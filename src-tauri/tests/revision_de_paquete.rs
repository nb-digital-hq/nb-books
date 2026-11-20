//! La puerta de revisión del registro.
//!
//! # Qué exige el criterio de aceptación, y por qué las dos direcciones
//!
//! Que la segunda versión del módulo del banco —la que introduce carga remota
//! de código donde la primera no la tenía— **sea detenida**, y que **la primera
//! pase**. Las dos mitades importan lo mismo: una puerta que solo sabe rechazar
//! no demuestra nada, porque una que rechace todo cumpliría la primera mitad.
//!
//! # Por qué cada regla tiene además su propio paquete
//!
//! Porque el par del banco se diferencia en varias cosas a la vez —la versión
//! trae una dirección, una importación de socket y una capacidad declarada— y
//! con eso se prueba que la puerta detiene, pero no **por qué** detiene. Si una
//! regla dejara de funcionar, las otras dos seguirían tapándolo.
//!
//! Los paquetes de aquí abajo se construyen por eso a partir de un módulo que
//! pasa limpio, cambiando **una sola propiedad**: se le añade una sección
//! personalizada con una dirección, o con otro módulo dentro, o se le cambia el
//! manifiesto. Lo que falla es entonces exactamente lo que se rompió. Es la
//! misma disciplina que arregló cuatro pruebas que pasaban por el
//! motivo equivocado.

mod comun;

use std::path::{Path, PathBuf};

use comun::{banco_construido, componente_construido, raiz_repo};
use nb_books_lib::extensions::review::{revisar, Gravedad, Informe, Opciones};
use nb_books_lib::extensions::signature;

/// Un paquete recién montado en un directorio temporal.
///
/// Se **firma solo** antes de cada revisión, y con eso las pruebas corren en la
/// configuración de verdad del registro, donde un paquete sin firmar no se
/// distribuye. Si se hubieran escrito apagando la exigencia de firma, dieciséis
/// pruebas estarían midiendo una configuración que nadie usa.
struct Paquete {
    raiz: PathBuf,
    /// La clave del autor simulado, en PKCS#8. `None` para un paquete que a
    /// propósito no va firmado.
    clave: Option<Vec<u8>>,
}

impl Paquete {
    /// Un paquete que pasa la revisión entera. De aquí parten las variantes.
    ///
    /// El identificador **no es el del banco**: el del banco vive en el espacio
    /// reservado a las extensiones de primera parte, y usarlo aquí mezclaría el
    /// requisito R2 con lo que cada prueba quiere medir.
    fn limpio(nombre: &str, wasm: &[u8]) -> Self {
        let paquete = Self::vacio(nombre);
        paquete.manifiesto(
            r#"{
              "id": "com.ejemplo.revision",
              "name": "Paquete de prueba",
              "version": "1.0.0",
              "type": "core",
              "apiVersion": "1.0",
              "license": "Apache-2.0"
            }"#,
        );
        paquete.licencia();
        paquete.fuente();
        paquete.modulo("modulo.wasm", wasm);
        paquete
    }

    /// El código del que sale el módulo, que viaja con el paquete (R8).
    fn fuente(&self) {
        std::fs::create_dir_all(self.raiz.join("src")).expect("carpeta de fuente");
        std::fs::write(self.raiz.join("src/lib.rs"), "// fuente\n").expect("fuente");
        std::fs::write(
            self.raiz.join("Cargo.toml"),
            "[package]\nname = \"prueba\"\n",
        )
        .expect("receta");
    }

    /// El mismo paquete, sin firmar.
    fn sin_firmar(mut self) -> Self {
        self.clave = None;
        let _ = std::fs::remove_file(self.raiz.join(signature::ARCHIVO));
        self
    }

    fn vacio(nombre: &str) -> Self {
        let raiz = nb_books_lib::paths::temporal_de_pruebas().join(format!(
            "nb-revision-{nombre}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("reloj")
                .as_nanos()
        ));
        std::fs::create_dir_all(&raiz).expect("crear el paquete");
        Self {
            raiz,
            clave: Some(signature::crear_clave().expect("un par de claves")),
        }
    }

    /// Firma el paquete tal y como está ahora mismo.
    ///
    /// Se llama antes de cada revisión y no al montarlo, porque la firma cubre
    /// la huella del paquete entero: cualquier cambio posterior la dejaría sin
    /// corresponder, y entonces las pruebas de contenido fallarían por la firma
    /// en lugar de por lo que miden.
    fn sellar(&self) {
        let Some(clave) = &self.clave else {
            return;
        };
        let Ok(texto) = std::fs::read_to_string(self.raiz.join("nb-extension.json")) else {
            return;
        };
        let Ok(manifiesto) = nb_books_lib::extensions::manifest::parse(&texto) else {
            return;
        };
        let firma = signature::firmar(&self.raiz, &manifiesto.id, &manifiesto.version, clave)
            .expect("firmar el paquete");
        std::fs::write(
            self.raiz.join(signature::ARCHIVO),
            serde_json::to_string(&firma).expect("la firma es JSON"),
        )
        .expect("escribir la firma");
    }

    fn manifiesto(&self, json: &str) {
        std::fs::write(self.raiz.join("nb-extension.json"), json).expect("escribir el manifiesto");
    }

    fn licencia(&self) {
        std::fs::write(
            self.raiz.join("LICENSE"),
            "Apache License\nVersion 2.0, January 2004\n",
        )
        .expect("escribir la licencia");
    }

    fn modulo(&self, nombre: &str, bytes: &[u8]) {
        std::fs::write(self.raiz.join(nombre), bytes).expect("escribir el módulo");
    }

    fn revisar(&self) -> Informe {
        self.sellar();
        revisar(&self.raiz, Opciones::default()).expect("el paquete se puede leer")
    }

    fn revisar_como_primera_parte(&self) -> Informe {
        self.sellar();
        revisar(
            &self.raiz,
            Opciones {
                de_primera_parte: true,
                ..Opciones::default()
            },
        )
        .expect("el paquete se puede leer")
    }
}

impl Drop for Paquete {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.raiz);
    }
}

/// Los requisitos que un informe señala, para comparar contra lo esperado.
fn requisitos(informe: &Informe) -> Vec<&str> {
    informe.requisitos().into_iter().collect()
}

/// Añade una sección personalizada a un módulo, que es la forma de cambiarle
/// **una sola cosa** sin recompilarlo.
///
/// Una sección personalizada no altera lo que el módulo hace: es carga que
/// viaja dentro del binario, exactamente igual que la que llevaría un módulo
/// que escondiera algo ahí.
fn con_seccion(wasm: &[u8], nombre: &str, datos: &[u8]) -> Vec<u8> {
    let mut carga = Vec::new();
    leb128(&mut carga, nombre.len() as u64);
    carga.extend_from_slice(nombre.as_bytes());
    carga.extend_from_slice(datos);

    let mut salida = wasm.to_vec();
    salida.push(0);
    leb128(&mut salida, carga.len() as u64);
    salida.extend_from_slice(&carga);
    salida
}

fn leb128(salida: &mut Vec<u8>, mut valor: u64) {
    loop {
        let mut byte = (valor & 0x7f) as u8;
        valor >>= 7;
        if valor != 0 {
            byte |= 0x80;
        }
        salida.push(byte);
        if valor == 0 {
            return;
        }
    }
}

fn leer_leb128(bytes: &[u8], cursor: &mut usize) -> u64 {
    let (mut valor, mut desplazamiento) = (0_u64, 0_u32);
    loop {
        let byte = bytes[*cursor];
        *cursor += 1;
        valor |= u64::from(byte & 0x7f) << desplazamiento;
        if byte & 0x80 == 0 {
            return valor;
        }
        desplazamiento += 7;
    }
}

/// Esconde `datos` en el primer módulo que lleva un componente, que es donde R4
/// tiene que mirar. Se reescribe el marco de la sección para que el
/// largo cuadre: el resto del componente queda byte a byte como estaba.
fn componente_con_escondite(componente: &[u8], datos: &[u8]) -> Vec<u8> {
    let mut salida = componente[..8].to_vec();
    let mut cursor = 8;
    let mut hecho = false;
    while cursor < componente.len() {
        let identificador = componente[cursor];
        cursor += 1;
        let largo = leer_leb128(componente, &mut cursor) as usize;
        let contenido = &componente[cursor..cursor + largo];
        cursor += largo;
        let contenido = if identificador == 1 && !hecho {
            hecho = true;
            con_seccion(contenido, "recurso", datos)
        } else {
            contenido.to_vec()
        };
        salida.push(identificador);
        leb128(&mut salida, contenido.len() as u64);
        salida.extend_from_slice(&contenido);
    }
    assert!(hecho, "el componente tiene que llevar algún módulo dentro");
    salida
}

/// El módulo benigno del par: la primera versión, la que tiene que pasar.
fn modulo_benigno() -> Option<Vec<u8>> {
    let banco = banco_construido()?;
    std::fs::read(banco.join("carga-remota-v1.wasm")).ok()
}

fn modulo_del_banco(nombre: &str) -> Option<Vec<u8>> {
    let banco = banco_construido()?;
    std::fs::read(banco.join(nombre)).ok()
}

// ---------------------------------------------------------------------------
// El criterio de aceptación
// ---------------------------------------------------------------------------

#[test]
fn la_version_que_introduce_carga_remota_se_detiene_y_la_anterior_pasa() {
    let (Some(primera), Some(segunda)) =
        (modulo_benigno(), modulo_del_banco("carga-remota-v2.wasm"))
    else {
        return;
    };

    let buena = Paquete::limpio("v1", &primera);
    let informe = buena.revisar();
    assert!(
        informe.pasa(),
        "la primera versión no lleva nada objetable y tiene que publicarse: {:?}",
        informe.hallazgos
    );

    let mala = Paquete::limpio("v2", &segunda);
    let informe = mala.revisar();
    assert!(
        !informe.pasa(),
        "la segunda versión trae carga remota de código y no puede publicarse"
    );
    assert!(
        requisitos(&informe).contains(&"R3"),
        "y tiene que decir que es por R3: {:?}",
        informe.hallazgos
    );
}

// ---------------------------------------------------------------------------
// Una regla por paquete, con una sola diferencia cada uno
// ---------------------------------------------------------------------------

#[test]
fn una_direccion_de_la_que_traer_codigo_detiene_el_paquete() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let con_direccion = con_seccion(
        &benigno,
        "aviso",
        b"https://ejemplo.invalid/carga-util.wasm",
    );

    let paquete = Paquete::limpio("direccion", &con_direccion);
    let informe = paquete.revisar();

    assert_eq!(
        requisitos(&informe),
        vec!["R3"],
        "lo único que cambia respecto al paquete limpio es la dirección: {:?}",
        informe.hallazgos
    );
    assert!(informe.hallazgos[0]
        .que_pasa
        .contains("https://ejemplo.invalid/carga-util.wasm"));
}

#[test]
fn una_direccion_que_no_apunta_a_codigo_no_detiene_nada() {
    // El requisito es sobre código, no sobre red. Una extensión que consulta un
    // catálogo lleva su dirección dentro y es exactamente lo que se le permite
    // hacer con su permiso; detenerla enseñaría a ignorar los hallazgos.
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let con_direccion = con_seccion(&benigno, "aviso", b"https://openlibrary.org/api/books");

    let paquete = Paquete::limpio("direccion-benigna", &con_direccion);
    assert!(paquete.revisar().pasa());
}

#[test]
fn un_modulo_escondido_dentro_de_otro_detiene_el_paquete() {
    let Some(benigno) = benigno_sin_modulo_dentro() else {
        return;
    };
    // La cabecera de un módulo vacío: ocho bytes que no significan nada más que
    // "aquí empieza WebAssembly". Es lo que delata a un módulo que viaja como
    // datos dentro de otro.
    let con_escondite = con_seccion(&benigno, "recurso", b"\0asm\x01\0\0\0");

    let paquete = Paquete::limpio("escondite", &con_escondite);
    let informe = paquete.revisar();

    assert_eq!(
        requisitos(&informe),
        vec!["R4"],
        "solo se le añadió un módulo dentro: {:?}",
        informe.hallazgos
    );
}

/// El módulo benigno, comprobando antes que no lleva ya otro módulo dentro.
///
/// Sin esta comprobación, la prueba de R4 podría pasar por el motivo
/// equivocado: si el módulo de partida ya llevara la cabecera repetida, el
/// hallazgo saldría igual sin haber añadido nada.
fn benigno_sin_modulo_dentro() -> Option<Vec<u8>> {
    let benigno = modulo_benigno()?;
    let paquete = Paquete::limpio("control-r4", &benigno);
    assert!(
        paquete.revisar().pasa(),
        "el módulo de partida tiene que estar limpio para que R4 signifique algo"
    );
    Some(benigno)
}

#[test]
fn una_importacion_que_ninguna_capacidad_concede_detiene_el_paquete() {
    let Some(conexion) = modulo_del_banco("abre-una-conexion.wasm") else {
        return;
    };
    let paquete = Paquete::limpio("socket", &conexion);
    let informe = paquete.revisar();

    assert_eq!(requisitos(&informe), vec!["R5"], "{:?}", informe.hallazgos);
    assert!(
        informe
            .hallazgos
            .iter()
            .any(|h| h.que_pasa.contains("sock_accept")),
        "el hallazgo nombra la función, que es lo que su autor tiene que quitar: {:?}",
        informe.hallazgos
    );
}

#[test]
fn leer_archivos_del_anfitrion_tambien_se_detiene_al_revisar() {
    // El cajón de arena ya lo impide al ejecutarlo. Que además lo
    // detenga la revisión no es redundancia: es que su autor se entera antes de
    // publicarlo en vez de después.
    let Some(lectura) = modulo_del_banco("lee-un-archivo.wasm") else {
        return;
    };
    let informe = Paquete::limpio("archivo", &lectura).revisar();

    assert!(informe
        .hallazgos
        .iter()
        .any(|h| h.requisito == "R5" && h.que_pasa.contains("path_open")));
}

#[test]
fn declarar_una_capacidad_que_nada_puede_ejercer_detiene_el_paquete() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("sobra", &benigno);
    paquete.manifiesto(
        r#"{
          "id": "com.ejemplo.revision",
          "name": "Paquete de prueba",
          "version": "1.0.0",
          "type": "core",
          "apiVersion": "1.0",
          "license": "Apache-2.0",
          "capabilities": ["network:fetch"],
          "permissions": { "network:fetch": { "urls": ["https://ejemplo.invalid/*"] } }
        }"#,
    );
    let informe = paquete.revisar();

    assert_eq!(
        requisitos(&informe),
        vec!["R5"],
        "lo único que cambia es la capacidad declarada: {:?}",
        informe.hallazgos
    );
}

#[test]
fn un_componente_que_llega_al_anfitrion_si_puede_declarar_la_capacidad() {
    // Es el control de la prueba anterior. Sin él, "declara de más" podría estar
    // implementado como "declarar siempre sobra", que rechazaría a toda
    // extensión que de verdad necesita salir a la red.
    let Some(componentes) = componente_construido() else {
        return;
    };
    let Ok(bytes) = std::fs::read(componentes.join("usa_el_mundo.wasm")) else {
        return;
    };

    let paquete = Paquete::limpio("componente", &bytes);
    paquete.manifiesto(
        r#"{
          "id": "com.ejemplo.revision",
          "name": "Paquete de prueba",
          "version": "1.0.0",
          "type": "core",
          "apiVersion": "1.0",
          "license": "Apache-2.0",
          "capabilities": ["network:fetch"],
          "permissions": { "network:fetch": { "urls": ["https://ejemplo.invalid/*"] } }
        }"#,
    );
    let informe = paquete.revisar();

    assert!(
        !requisitos(&informe).contains(&"R5"),
        "un componente que importa la interfaz del anfitrión sí ejerce lo que declara: {:?}",
        informe.hallazgos
    );
    assert!(
        informe.pasa(),
        "y nada más lo detiene: {:?}",
        informe.hallazgos
    );
}

/// R4 mira dentro de un componente, en cada módulo que lleva.
///
/// Antes el informe avisaba de que no llegaba: un componente lleva módulos
/// dentro por construcción, y buscar una cabecera en cualquier parte encontraba las
/// suyas. Ahora se recorre su formato y se busca **dentro de cada módulo**, donde no
/// tiene por qué haber otro. Se comprueba con el mismo componente limpio y con un
/// módulo escondido en su primer módulo, que es lo único que cambia.
#[test]
fn un_modulo_escondido_dentro_de_un_componente_se_detiene_y_el_limpio_pasa() {
    let Some(componentes) = componente_construido() else {
        return;
    };
    let Ok(bytes) = std::fs::read(componentes.join("formato_de_texto.wasm")) else {
        return;
    };

    let limpio = Paquete::limpio("componente-limpio", &bytes).revisar();
    assert!(
        !requisitos(&limpio).contains(&"R4"),
        "un componente limpio no tiene nada que decir de R4: {:?}",
        limpio.hallazgos
    );
    assert!(limpio.pasa(), "{:?}", limpio.hallazgos);

    let escondido = componente_con_escondite(&bytes, b"\0asm\x01\0\0\0");
    let informe = Paquete::limpio("componente-escondite", &escondido).revisar();
    let hallazgo = informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R4")
        .expect("el módulo escondido en el componente se ve");
    assert_eq!(hallazgo.gravedad, Gravedad::Detiene);
    assert!(!informe.pasa());
}

#[test]
fn un_paquete_sin_el_texto_de_la_licencia_no_se_publica() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::vacio("sin-licencia");
    paquete.manifiesto(
        r#"{
          "id": "com.ejemplo.revision",
          "name": "Paquete de prueba",
          "version": "1.0.0",
          "type": "core",
          "apiVersion": "1.0",
          "license": "Apache-2.0"
        }"#,
    );
    paquete.fuente();
    paquete.modulo("modulo.wasm", &benigno);

    let informe = paquete.revisar();
    assert_eq!(
        requisitos(&informe),
        vec!["R6"],
        "declarar la licencia en el manifiesto no es traerla: {:?}",
        informe.hallazgos
    );

    paquete.licencia();
    assert!(paquete.revisar().pasa());
}

#[test]
fn el_espacio_de_nombres_reservado_es_de_primera_parte() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("reservado", &benigno);
    paquete.manifiesto(
        r#"{
          "id": "lat.nbdigitallabs.books.metadata.ejemplo",
          "name": "Paquete de prueba",
          "version": "1.0.0",
          "type": "core",
          "apiVersion": "1.0",
          "license": "Apache-2.0"
        }"#,
    );

    let informe = paquete.revisar();
    assert_eq!(requisitos(&informe), vec!["R2"], "{:?}", informe.hallazgos);

    // La misma extensión enviada por el propio proyecto sí puede llamarse así.
    // Que la diferencia esté en cómo se invoca la revisión y no en el paquete es
    // deliberado: si se dedujera del contenido, el paquete decidiría si es
    // oficial.
    assert!(paquete.revisar_como_primera_parte().pasa());
}

/// El nombre del producto en el nombre visible también se hace pasar
/// por oficial, escrito como se escriba: el ejemplo de la política es "NB Books
/// Sync", que no comparte ni un carácter con el prefijo reservado.
#[test]
fn un_nombre_visible_con_el_nombre_del_producto_se_detiene() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    for nombre in [
        "NB Books Sync",
        "Sincronizador nb-books",
        "N.B. BOOKS extra",
    ] {
        let paquete = Paquete::limpio("nombre-oficial", &benigno);
        paquete.manifiesto(&format!(
            r#"{{
              "id": "com.ejemplo.sincronizador",
              "name": "{nombre}",
              "version": "1.0.0",
              "type": "core",
              "apiVersion": "1.0",
              "license": "Apache-2.0"
            }}"#
        ));
        let informe = paquete.revisar();
        assert_eq!(
            requisitos(&informe),
            vec!["R2"],
            "{nombre}: {:?}",
            informe.hallazgos
        );
        assert!(paquete.revisar_como_primera_parte().pasa(), "{nombre}");
    }
}

#[test]
fn una_version_de_api_que_este_anfitrion_no_habla_se_detiene() {
    let Some(futura) = modulo_del_banco("version-futura.wasm") else {
        return;
    };
    let informe = Paquete::limpio("futura", &futura).revisar();

    assert!(
        informe.hallazgos.iter().any(|h| h.requisito == "R11"),
        "{:?}",
        informe.hallazgos
    );
    let dicho = &informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R11")
        .expect("hay hallazgo de R11")
        .que_pasa;
    assert!(
        dicho.contains("admite"),
        "el aviso lleva el rango con el que su autor puede arreglarlo, no el nombre \
         de una variante de Rust: {dicho}"
    );
}

#[test]
fn un_componente_sin_seccion_de_version_se_detiene_por_r11() {
    let componente_vacio = b"\0asm\x0d\x00\x01\x00";
    let informe = Paquete::limpio("sin-version-comp", componente_vacio).revisar();

    assert!(
        informe.hallazgos.iter().any(|h| h.requisito == "R11"),
        "falta R11: {:?}",
        informe.hallazgos
    );
    let hallazgo = informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R11")
        .expect("hay hallazgo de R11");
    assert_eq!(hallazgo.gravedad, Gravedad::Detiene);
    assert!(
        hallazgo.que_pasa.contains("nb:api-version"),
        "debe señalar la sección que falta: {}",
        hallazgo.que_pasa
    );
}

#[test]
fn un_componente_cuya_version_discrepa_del_manifiesto_se_detiene_por_r1() {
    let Some(componentes) = componente_construido() else {
        return;
    };
    let Ok(bytes) = std::fs::read(componentes.join("formato_de_texto.wasm")) else {
        return;
    };
    let paquete = Paquete::limpio("discrepante", &bytes);
    // formato_de_texto declara API 1 (1.0.0); el manifiesto declara 2.0 para forzar la discrepancia
    paquete.manifiesto(
        r#"{
          "id": "com.ejemplo.revision",
          "name": "Paquete discrepante",
          "version": "1.0.0",
          "type": "core",
          "apiVersion": "2.0",
          "license": "Apache-2.0"
        }"#,
    );

    let informe = paquete.revisar();
    assert!(
        informe.hallazgos.iter().any(|h| h.requisito == "R1"),
        "debe detectar discrepancia R1: {:?}",
        informe.hallazgos
    );
    let hallazgo = informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R1")
        .expect("hay hallazgo de R1");
    assert_eq!(hallazgo.gravedad, Gravedad::Detiene);
    assert!(
        hallazgo.que_pasa.contains("manifiesto declara la API 2.0")
            && hallazgo.que_pasa.contains("módulo trae la 1"),
        "debe contrastar ambas declaraciones: {}",
        hallazgo.que_pasa
    );
}

#[test]
fn un_componente_con_version_fuera_de_rango_se_detiene_por_r11() {
    let Some(componentes) = componente_construido() else {
        return;
    };
    let Ok(bytes) = std::fs::read(componentes.join("formato_de_texto.wasm")) else {
        return;
    };
    let con_futura = con_seccion(&bytes, "nb:api-version", b"99.0.0");
    let paquete = Paquete::limpio("futura-comp", &con_futura);
    paquete.manifiesto(
        r#"{
          "id": "com.ejemplo.revision",
          "name": "Paquete futuro",
          "version": "1.0.0",
          "type": "core",
          "apiVersion": "99.0",
          "license": "Apache-2.0"
        }"#,
    );

    let informe = paquete.revisar();
    assert!(
        informe.hallazgos.iter().any(|h| h.requisito == "R11"),
        "debe detenerse por versión no soportada R11: {:?}",
        informe.hallazgos
    );
    let hallazgo = informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R11")
        .expect("hay hallazgo de R11");
    assert_eq!(hallazgo.gravedad, Gravedad::Detiene);
    assert!(hallazgo.que_pasa.contains("admite"));
}

#[test]
fn un_manifiesto_invalido_se_detiene_diciendo_el_campo() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("manifiesto", &benigno);
    paquete.manifiesto(r#"{ "id": "no-es-un-dominio", "name": "", "version": "1.0" }"#);

    let informe = paquete.revisar();
    assert!(!informe.pasa());
    assert!(informe
        .hallazgos
        .iter()
        .any(|h| h.requisito == "R1" && h.donde.contains("id")));
    assert!(informe
        .hallazgos
        .iter()
        .any(|h| h.donde.contains("version")));
}

#[test]
fn un_archivo_que_no_es_webassembly_no_pasa_por_llamarse_wasm() {
    // Dar por limpio lo que no se ha podido leer es peor que rechazarlo: el
    // informe saldría sin hallazgos, que es exactamente lo que un envío
    // malicioso querría conseguir.
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("impostor", &benigno);
    paquete.modulo("recursos.wasm", b"esto no es un modulo");

    let informe = paquete.revisar();
    assert_eq!(informe.modulos, 2);
    assert!(
        informe
            .hallazgos
            .iter()
            .any(|h| h.requisito == "R12" && h.donde == "recursos.wasm"),
        "{:?}",
        informe.hallazgos
    );
    assert!(!informe.pasa());
}

#[test]
fn un_paquete_sin_manifiesto_no_se_publica() {
    let paquete = Paquete::vacio("sin-manifiesto");
    paquete.licencia();
    let informe = paquete.revisar();

    assert!(!informe.pasa());
    assert_eq!(informe.extension, None);
}

#[test]
fn revisar_algo_que_no_es_un_directorio_es_un_error_y_no_un_hallazgo() {
    // La diferencia importa: un hallazgo dice que el paquete incumple, y aquí no
    // hay paquete. Confundirlos haría que un fallo de la herramienta se leyera
    // como una extensión maliciosa.
    assert!(revisar(Path::new("/no/existe/este/paquete"), Opciones::default()).is_err());
}

#[test]
fn se_revisan_todos_los_modulos_del_paquete_y_no_solo_los_que_el_manifiesto_nombra() {
    // Lo que viaja dentro del paquete se distribuye, lo cite alguien o no.
    let (Some(benigno), Some(malo)) = (modulo_benigno(), modulo_del_banco("carga-remota-v2.wasm"))
    else {
        return;
    };
    let paquete = Paquete::limpio("polizon", &benigno);
    std::fs::create_dir_all(paquete.raiz.join("recursos")).expect("crear subcarpeta");
    paquete.modulo("recursos/polizon.wasm", &malo);

    let informe = paquete.revisar();
    assert_eq!(informe.modulos, 2);
    assert!(
        informe
            .hallazgos
            .iter()
            .any(|h| h.requisito == "R3" && h.donde == "recursos/polizon.wasm"),
        "el hallazgo señala el archivo, no el paquete entero: {:?}",
        informe.hallazgos
    );
}

// ---------------------------------------------------------------------------
// La puerta y la política hablan del mismo mundo
// ---------------------------------------------------------------------------

#[test]
fn todo_requisito_que_la_puerta_nombra_existe_en_la_politica() {
    // `scripts/check-politica.mjs` comprueba la dirección contraria: que el
    // mecanismo que la política declara cite su requisito. Esta comprueba que
    // la puerta no invente identificadores, que es como se llegaría a un aviso
    // citando un requisito que el autor no puede ir a leer.
    let fuente = std::fs::read_to_string(
        raiz_repo()
            .join("src-tauri/src/extensions")
            .join("review.rs"),
    )
    .expect("la puerta existe");
    let politica = std::fs::read_to_string(
        raiz_repo()
            .join("docs/legal/governance")
            .join("extension-policy.md"),
    )
    .expect("la política existe");

    let mut citados: Vec<String> = fuente
        .match_indices("requisito: \"")
        .map(|(posicion, _)| {
            let resto = &fuente[posicion + "requisito: \"".len()..];
            resto
                .split('"')
                .next()
                .expect("el literal está cerrado")
                .to_owned()
        })
        .collect();
    citados.sort();
    citados.dedup();

    assert!(!citados.is_empty(), "la puerta no cita ningún requisito");
    for requisito in citados {
        assert!(
            politica.contains(&format!("### {requisito} —")),
            "la puerta señala {requisito} y la política no lo define"
        );
    }
}

// ---------------------------------------------------------------------------
// R13: la firma del autor
// ---------------------------------------------------------------------------

#[test]
fn un_paquete_sin_firmar_no_se_distribuye() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("sin-firma", &benigno).sin_firmar();
    let informe = paquete.revisar();

    assert_eq!(
        requisitos(&informe),
        vec!["R13"],
        "lo único que le falta es la firma: {:?}",
        informe.hallazgos
    );
}

#[test]
fn su_autor_puede_revisar_el_contenido_antes_de_firmarlo() {
    // Quien está escribiendo su extensión todavía no la ha firmado, y exigirle
    // la firma para poder mirar el contenido le obligaría a firmar borradores.
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("borrador", &benigno).sin_firmar();
    let informe = revisar(
        &paquete.raiz,
        Opciones {
            exige_firma: false,
            ..Opciones::default()
        },
    )
    .expect("el paquete se puede leer");

    assert!(informe.pasa(), "{:?}", informe.hallazgos);
}

#[test]
fn una_firma_que_no_corresponde_al_contenido_se_detiene() {
    // Es el caso que importa: el paquete va firmado, y lo que se firmó no es lo
    // que llega. Se consigue firmando y cambiando algo después, que es
    // exactamente lo que haría quien intercepta un paquete por el camino.
    let (Some(benigno), Some(otro)) = (modulo_benigno(), modulo_del_banco("entra-en-bucle.wasm"))
    else {
        return;
    };
    let paquete = Paquete::limpio("manipulado", &benigno);
    paquete.sellar();
    paquete.modulo("modulo.wasm", &otro);

    // Sin `sellar` de por medio: se revisa la firma vieja contra el contenido
    // nuevo.
    let informe = revisar(&paquete.raiz, Opciones::default()).expect("el paquete se puede leer");
    let hallazgo = informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R13")
        .expect("la firma no corresponde");
    assert!(
        hallazgo.que_pasa.contains("no corresponde"),
        "{}",
        hallazgo.que_pasa
    );
}

#[test]
fn una_firma_ilegible_se_distingue_de_una_que_no_corresponde() {
    // Un archivo mal escrito y una firma que no cuadra son problemas distintos
    // y se arreglan distinto: uno es del autor y el otro es de lo que llegó.
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("firma-rota", &benigno).sin_firmar();
    std::fs::write(paquete.raiz.join(signature::ARCHIVO), "esto no es json")
        .expect("escribir la firma");

    let informe = paquete.revisar();
    let hallazgo = informe
        .hallazgos
        .iter()
        .find(|h| h.requisito == "R13")
        .expect("la firma no se puede leer");
    assert!(hallazgo.que_pasa.contains("JSON"), "{}", hallazgo.que_pasa);
}

/// R2: un nombre que se lee como el del producto se detiene aunque no lo escriba igual.
#[test]
fn un_nombre_que_imita_al_producto_se_detiene() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    for nombre in ["NB Bookz", "N8 Books Lector", "nb-bo0ks"] {
        let paquete = Paquete::limpio(&format!("imita-{}", nombre.len()), &benigno);
        paquete.manifiesto(&format!(
            r#"{{"id":"com.ejemplo.revision","name":"{nombre}","version":"1.0.0","type":"core","apiVersion":"1.0","license":"Apache-2.0"}}"#
        ));
        assert_eq!(requisitos(&paquete.revisar()), vec!["R2"], "{nombre}");
    }
    // Un nombre que solo dice "books" no imita nada.
    let paquete = Paquete::limpio("libros", &benigno);
    paquete.manifiesto(
        r#"{"id":"com.ejemplo.revision","name":"My Books Tools","version":"1.0.0","type":"core","apiVersion":"1.0","license":"Apache-2.0"}"#,
    );
    assert!(paquete.revisar().pasa());
}

/// R6: el texto de la licencia tiene que ser el de la licencia declarada.
#[test]
fn una_licencia_declarada_con_otro_texto_se_detiene() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("licencia-equivocada", &benigno);
    paquete.manifiesto(
        r#"{"id":"com.ejemplo.revision","name":"Paquete de prueba","version":"1.0.0","type":"core","apiVersion":"1.0","license":"MIT"}"#,
    );
    assert_eq!(requisitos(&paquete.revisar()), vec!["R6"]);
}

/// R8: un paquete con código trae su fuente y su receta.
#[test]
fn un_paquete_con_codigo_sin_su_fuente_se_detiene() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("sin-fuente", &benigno);
    std::fs::remove_dir_all(paquete.raiz.join("src")).expect("quitar la fuente");
    assert_eq!(requisitos(&paquete.revisar()), vec!["R8"]);
}

/// R10: una herramienta conocida para quitar DRM no entra.
#[test]
fn un_paquete_con_una_herramienta_para_quitar_drm_se_detiene() {
    let Some(benigno) = modulo_benigno() else {
        return;
    };
    let paquete = Paquete::limpio("drm", &benigno);
    std::fs::write(paquete.raiz.join("src/dedrm_plugin.rs"), "// \n").expect("archivo");
    assert_eq!(requisitos(&paquete.revisar()), vec!["R10"]);
}
