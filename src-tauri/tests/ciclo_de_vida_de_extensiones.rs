//! Ciclo de vida y activación perezosa.
//!
//! El criterio de aceptación dice que instalar veinte extensiones no degrada
//! el arranque. Aquí se mide, y se dice el número.
//!
//! # Dónde está la prueba de que la carga es perezosa
//!
//! **En la firma, no en el reloj.** `Registro::cargar` no recibe un `Motor`, así
//! que no puede compilar nada aunque quisiera; compilar exige el motor y el
//! motor solo llega en `invocar`. Una prueba de tiempo puede tener un día malo
//! en una máquina cargada; una firma no.
//!
//! Lo que sí se mide con reloj es el presupuesto: cuánto cuesta de verdad
//! arrancar con veinte extensiones instaladas.

mod comun;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use comun::{modulo, OBJETIVO};
use nb_books_lib::extensions::registry::Registro;
use nb_books_lib::extensions::store::Store;
use nb_books_lib::extensions::{Desenlace, Limites, Motor};

/// Cuánto le concede `performance-budgets.md` al arranque del runtime entero.
const PRESUPUESTO_DE_ARRANQUE: Duration = Duration::from_millis(200);

/// Lo que puede costar cargar el registro sin morder ese presupuesto.
///
/// Una cuarta parte, y sobra: leer veinte manifiestos es leer unos pocos
/// kilobytes de JSON. Si algún día esto se acerca al límite será porque alguien
/// metió trabajo de verdad en la carga, y eso es justo lo que hay que ver.
const PRESUPUESTO_DEL_REGISTRO: Duration = Duration::from_millis(50);

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-ciclo-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

/// Un paquete cuyo manifiesto coloca sus órdenes en un sitio de la interfaz.
fn paquete_con_menu(en: &Path, id: &str, sitio: Option<&str>) -> PathBuf {
    let dir = en.join(format!("menu-{}", id.replace('.', "-")));
    std::fs::create_dir_all(&dir).expect("paquete");
    let mut contribuciones = serde_json::json!({
        "commands": [{ "id": format!("{id}.orden"), "title": "Hacer algo" }],
        "providers": { "command": "modulo.wasm" }
    });
    if let Some(sitio) = sitio {
        contribuciones["contextMenus"] = serde_json::json!({ sitio: [format!("{id}.orden")] });
    }
    let manifiesto = serde_json::json!({
        "id": id,
        "name": "Extensión con menú",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": contribuciones
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    std::fs::write(dir.join("modulo.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    dir
}

/// Un paquete con un módulo de verdad y los comandos que se le digan.
fn paquete(en: &Path, id: &str, wasm: &[u8], comandos: usize) -> PathBuf {
    let dir = en.join(format!("paquete-{}", id.replace('.', "-")));
    std::fs::create_dir_all(&dir).expect("paquete");
    let contribuciones: Vec<serde_json::Value> = (0..comandos)
        .map(|n| serde_json::json!({ "id": format!("{id}.orden{n}"), "title": "Hacer algo" }))
        .collect();
    let manifiesto = serde_json::json!({
        "id": id,
        "name": "Extensión de prueba",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": {
            "commands": contribuciones,
            "providers": { "metadata": "modulo.wasm" }
        }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    std::fs::write(dir.join("modulo.wasm"), wasm).expect("módulo");
    dir
}

#[test]
fn los_comandos_de_una_extension_estan_antes_de_cargar_su_codigo() {
    let raiz = temporal("comandos");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.aporta", b"\0asm\x01\0\0\0", 2))
        .expect("instala");

    let registro = Registro::cargar(&almacen).expect("registro");
    let comandos = registro.comandos();

    // Salen del manifiesto. En este punto no se ha compilado nada, y no se
    // podría: `cargar` no tiene motor con el que hacerlo.
    assert_eq!(comandos.len(), 2);
    assert_eq!(comandos[0].extension, "org.ejemplo.aporta");
    assert_eq!(comandos[0].comando.id, "org.ejemplo.aporta.orden0");
}

#[test]
fn una_extension_desactivada_no_aporta_pero_sigue_instalada() {
    let raiz = temporal("desactivar");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.apaga", b"\0asm\x01\0\0\0", 1))
        .expect("instala");

    assert!(almacen
        .habilitar("org.ejemplo.apaga", false)
        .expect("apaga"));

    let registro = Registro::cargar(&almacen).expect("registro");
    assert!(registro.comandos().is_empty());
    // Pero sigue instalada, con su sitio y su versión: desactivar no es
    // desinstalar.
    assert_eq!(registro.instaladas().len(), 1);
    assert!(!registro.instaladas()[0].enabled);
    assert!(!registro.esta_activa("org.ejemplo.apaga"));

    // Y se vuelve a encender sin reinstalar nada.
    assert!(almacen
        .habilitar("org.ejemplo.apaga", true)
        .expect("enciende"));
    assert_eq!(
        Registro::cargar(&almacen)
            .expect("registro")
            .comandos()
            .len(),
        1
    );
}

#[test]
fn desactivar_dos_veces_no_deja_una_generacion_por_cada_intento() {
    let raiz = temporal("idempotente");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.dos", b"\0asm\x01\0\0\0", 0))
        .expect("instala");

    assert!(almacen.habilitar("org.ejemplo.dos", false).expect("apaga"));
    let tras_apagar = almacen.perfil().expect("perfil").generation;
    // La segunda no cambia nada, así que no crea generación: un histórico lleno
    // de generaciones idénticas hace inservible volver atrás.
    assert!(!almacen
        .habilitar("org.ejemplo.dos", false)
        .expect("otra vez"));
    assert_eq!(almacen.perfil().expect("perfil").generation, tras_apagar);
}

#[test]
fn una_extension_con_el_manifiesto_roto_no_impide_arrancar() {
    let raiz = temporal("rota");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let buena = paquete(&raiz, "org.ejemplo.buena", b"\0asm\x01\0\0\0", 1);
    let mala = paquete(&raiz, "org.ejemplo.mala", b"\0asm\x01\0\0\0", 1);
    almacen.instalar(&buena).expect("buena");
    let instalada = almacen.instalar(&mala).expect("mala");

    // Se estropea su manifiesto **dentro del almacén**, que es lo que pasaría
    // con un disco con errores. El almacén es inmutable para nosotros, no para
    // el hardware.
    std::fs::write(
        almacen.ruta_de(&instalada).join("nb-extension.json"),
        "{ esto ya no es json",
    )
    .expect("se estropea");

    // Que una extensión rota impidiera abrir la aplicación sería el fallo de un
    // componente tumbando a otro, que es un innegociable.
    let registro = Registro::cargar(&almacen).expect("carga igualmente");
    assert_eq!(registro.instaladas().len(), 2);
    assert_eq!(
        registro.comandos().len(),
        1,
        "solo aporta la que se puede leer"
    );
}

#[test]
fn un_manifiesto_que_apunta_fuera_de_su_carpeta_no_se_sirve() {
    let raiz = temporal("escape");
    let Some(wasm) = modulo("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");

    // Un módulo de verdad, colocado **exactamente donde el escape lo
    // alcanzaría**: tres niveles por encima de `<raíz>/extensiones/store/<hash>`.
    //
    // Se construye así porque la primera versión de esta prueba apuntaba a
    // `../../../../etc/hostname` y pasaba **por el motivo equivocado**: esos
    // cuatro niveles no llegaban a `/etc` sino a `/tmp/etc`, que no existe, así
    // que fallaba por archivo no encontrado y no por contención. Quitar el
    // acotado la dejaba en verde. Lo encontró una mutación.
    std::fs::write(raiz.join("fuera.wasm"), &wasm).expect("el objetivo del escape");

    let dir = raiz.join("paquete-escapista");
    std::fs::create_dir_all(&dir).expect("paquete");
    let manifiesto = serde_json::json!({
        "id": "org.ejemplo.escapista",
        "name": "Se sale de su carpeta",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        // La ruta la escribe quien publica la extensión: es una ruta de fuera.
        "contributes": { "providers": { "metadata": "../../../fuera.wasm" } }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    /*
     * **Ahora no llega ni a instalarse**: un módulo que no está dentro de la
     * carpeta del paquete es un módulo que falta, y la instalación lo rechaza
     * antes de copiar nada.
     */
    assert_eq!(
        almacen.instalar(&dir).map(|_| ()),
        Err(nb_books_lib::error::NbError::ExtensionManifestInvalid)
    );

    /*
     * Y la contención al invocar sigue haciendo falta, por lo que entró antes de
     * esa comprobación o por un almacén tocado a mano. Se instala un paquete
     * sano y después se reescribe su manifiesto guardado para que apunte fuera:
     * es el mismo estado que dejaría una instalación anterior.
     */
    std::fs::write(dir.join("modulo.wasm"), &wasm).expect("módulo dentro");
    let sano = serde_json::json!({
        "id": "org.ejemplo.escapista",
        "name": "Se sale de su carpeta",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": { "providers": { "metadata": "modulo.wasm" } }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&sano).expect("json"),
    )
    .expect("manifiesto sano");
    let instalada = almacen.instalar(&dir).expect("el paquete sano entra");
    std::fs::write(
        almacen.ruta_de(&instalada).join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("el manifiesto guardado apunta fuera");

    let motor = Motor::nuevo().expect("motor");
    let registro = Registro::cargar(&almacen).expect("registro");
    let intento = registro.invocar(
        &motor,
        &almacen,
        "org.ejemplo.escapista",
        "metadata",
        Limites::default(),
    );

    // Sin acotar la ruta, el almacén serviría cualquier archivo del sistema con
    // solo publicar un manifiesto. Y aquí no fallaría: ese archivo es un módulo
    // válido y se ejecutaría.
    match intento {
        Err(_) => {}
        Ok(ejecucion) => panic!(
            "se ejecutó un módulo de fuera de la carpeta, y dijo: {}",
            ejecucion.salida
        ),
    }
}

#[test]
fn invocar_una_desactivada_no_ejecuta_nada() {
    let raiz = temporal("invocar-apagada");
    let Some(wasm) = modulo("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.apagada", &wasm, 0))
        .expect("instala");
    almacen
        .habilitar("org.ejemplo.apagada", false)
        .expect("apaga");

    let motor = Motor::nuevo().expect("motor");
    let registro = Registro::cargar(&almacen).expect("registro");
    assert!(registro
        .invocar(
            &motor,
            &almacen,
            "org.ejemplo.apagada",
            "metadata",
            Limites::default()
        )
        .is_err());
}

#[test]
fn invocar_compila_y_ejecuta_el_modulo_que_declara_el_manifiesto() {
    let raiz = temporal("invocar");
    let Some(wasm) = modulo("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.corre", &wasm, 0))
        .expect("instala");

    let motor = Motor::nuevo().expect("motor");
    let registro = Registro::cargar(&almacen).expect("registro");
    let ejecucion = registro
        .invocar(
            &motor,
            &almacen,
            "org.ejemplo.corre",
            "metadata",
            Limites::default(),
        )
        .expect("corre");

    assert_eq!(ejecucion.desenlace, Desenlace::Termino);
    assert!(ejecucion.salida.contains("suma = 55"));
}

/// **El criterio de aceptación, medido.**
#[test]
fn veinte_extensiones_instaladas_no_se_comen_el_arranque() {
    let raiz = temporal("veinte");
    let Some(wasm) = modulo("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    for n in 0..20 {
        almacen
            .instalar(&paquete(&raiz, &format!("org.ejemplo.n{n:02}"), &wasm, 3))
            .expect("instala");
    }

    let reloj = Instant::now();
    let registro = Registro::cargar(&almacen).expect("registro");
    let carga = reloj.elapsed();

    assert_eq!(registro.instaladas().len(), 20);
    assert_eq!(registro.comandos().len(), 60);

    // Lo que costaría de verdad no ser perezoso, para tener con qué comparar.
    let motor = Motor::nuevo().expect("motor");
    let reloj = Instant::now();
    registro
        .invocar(
            &motor,
            &almacen,
            "org.ejemplo.n00",
            "metadata",
            Limites::default(),
        )
        .expect("corre");
    let una_invocacion = reloj.elapsed();

    println!(
        "cargar veinte extensiones: {carga:?}; compilar y ejecutar una sola: {una_invocacion:?}"
    );
    assert!(
        carga < PRESUPUESTO_DEL_REGISTRO,
        "cargar veinte costó {carga:?}, y el presupuesto del registro es {PRESUPUESTO_DEL_REGISTRO:?} \
         de los {PRESUPUESTO_DE_ARRANQUE:?} que tiene el arranque entero"
    );
}

#[test]
fn una_orden_solo_aparece_donde_su_manifiesto_la_coloca() {
    let raiz = temporal("menus");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete_con_menu(
            &raiz,
            "org.ejemplo.colocada",
            Some("library/document"),
        ))
        .expect("colocada");
    almacen
        .instalar(&paquete_con_menu(&raiz, "org.ejemplo.suelta", None))
        .expect("suelta");

    let registro = Registro::cargar(&almacen).expect("registro");

    // Las dos declaran una orden, y las dos están en `comandos()`.
    assert_eq!(registro.comandos().len(), 2);

    // Pero solo una dice dónde va. Declarar una orden no es ponerla: eso
    // permite que una extensión tenga órdenes que solo invoca ella.
    let en_la_ficha = registro.comandos_en("library/document");
    assert_eq!(en_la_ficha.len(), 1);
    assert_eq!(en_la_ficha[0].extension, "org.ejemplo.colocada");
    assert!(registro.comandos_en("otro/sitio").is_empty());
}

#[test]
fn ejecutar_una_orden_que_el_manifiesto_no_declara_se_rechaza_antes_de_compilar() {
    // Con un **componente de verdad**, y eso importa: un módulo falso falla
    // igual al instanciarse, así que la prueba podría pasar por ese fallo en
    // lugar de por la comprobación. Con un componente que sí se instancia, la
    // única razón de que devuelva error es que el manifiesto no declara esa
    // orden. Comprobado quitando la comprobación: falla.
    let Some(componente) = comun::componente("usa_el_mundo") else {
        eprintln!("omitida: falta el objetivo {}", comun::OBJETIVO_COMPONENTE);
        return;
    };
    let raiz = temporal("orden-ajena");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");

    let dir = raiz.join("paquete-real");
    std::fs::create_dir_all(&dir).expect("paquete");
    let manifiesto = serde_json::json!({
        "id": "org.ejemplo.solouna",
        "name": "Aporta una sola orden",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": {
            "commands": [{ "id": "org.ejemplo.solouna.esta", "title": "Esta sí" }],
            "contextMenus": { "library/document": ["org.ejemplo.solouna.esta"] },
            "providers": { "command": "modulo.wasm" }
        }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    std::fs::write(dir.join("modulo.wasm"), &componente).expect("módulo");
    almacen.instalar(&dir).expect("instala");

    let motor = Motor::nuevo().expect("motor");
    let db = nb_books_lib::db::Database::open_in_memory().expect("base");
    let red = std::sync::Arc::new(nb_books_lib::net::Network::new(Box::new(
        nb_books_lib::net::UreqTransport::new(),
    )));
    let registro = Registro::cargar(&almacen).expect("registro");

    // Lo que el usuario aprobó al instalar es lo que el manifiesto decía. Una
    // orden que no está ahí no se ejecuta, y se rechaza **sin compilar nada**.
    assert!(registro
        .ejecutar_orden(
            &nb_books_lib::extensions::registry::Entorno {
                motor: &motor,
                almacen: &almacen,
                db: &db,
                red: &red,
            },
            "org.ejemplo.solouna",
            "org.ejemplo.otra.orden",
            Limites::default()
        )
        .is_err());
}
