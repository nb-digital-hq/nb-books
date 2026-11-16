//! Un formato aportado por una extensión.
//!
//! El criterio de aceptación: *"una extensión de ejemplo añade soporte de un
//! formato nuevo **sin tocar el núcleo**, y su documento se lee, se ancla con
//! Locator y se busca como cualquier otro"*. Las cuatro cosas se comprueban
//! aquí, y la primera de todas es la que más importa: antes de instalarla, el
//! registro no sabe abrir ese tipo de medio, y después sí, **sin que nada del
//! núcleo haya cambiado**.

mod comun;

use std::path::{Path, PathBuf};

use nb_books_lib::extensions::{format, Limites, Subsistema};
use nb_books_lib::reader::provider::{NativePosition, ProviderRegistry};

use comun::{componente_construido, OBJETIVO_COMPONENTE};

/// El tipo de medio que declara la extensión de ejemplo en su manifiesto.
const TIPO: &str = "text/x-nb-secciones";

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-formato-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

/// Copia la extensión de ejemplo del banco a un paquete instalable.
fn paquete_del_formato(en: &Path) -> Option<PathBuf> {
    let construido = componente_construido()?;
    let origen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raíz")
        .join("extensiones")
        .join("banco")
        .join("formato-de-texto");

    let dir = en.join("paquete-formato");
    std::fs::create_dir_all(&dir).expect("paquete");
    std::fs::copy(
        origen.join("nb-extension.json"),
        dir.join("nb-extension.json"),
    )
    .expect("manifiesto");
    std::fs::copy(
        construido.join("formato_de_texto.wasm"),
        dir.join("formato_de_texto.wasm"),
    )
    .expect("módulo");
    Some(dir)
}

/// Un documento del formato nuevo.
fn documento(en: &Path) -> PathBuf {
    let ruta = en.join("obra.txt");
    std::fs::write(
        &ruta,
        "# Primer capítulo\n\
         Muchos años después, frente al pelotón de fusilamiento.\n\
         # Segundo capítulo\n\
         Macondo era entonces una aldea de veinte casas de barro.\n",
    )
    .expect("documento");
    ruta
}

/// **El criterio, entero.**
#[test]
fn una_extension_anade_un_formato_y_su_documento_se_lee_se_ancla_y_se_busca() {
    let raiz = temporal("completo");
    let Some(paquete) = paquete_del_formato(&raiz) else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");

    // Antes de instalar nada, el núcleo no sabe abrir esto. Es la mitad de la
    // afirmación que suele darse por supuesta.
    let mut registro = ProviderRegistry::new();
    assert!(!registro.supports(TIPO));

    subsistema.almacen.instalar(&paquete).expect("instala");
    let cuantos =
        format::registrar(&mut registro, &subsistema, Limites::default()).expect("registra");

    // Y ahora sí, **sin que nada del núcleo haya cambiado**: lo único que pasó
    // es que hay una extensión instalada.
    assert_eq!(cuantos, 1);
    assert!(registro.supports(TIPO));

    let ruta = documento(&raiz);
    let proveedor = registro.open(TIPO, &ruta).expect("abre");
    assert_eq!(proveedor.media_type(), TIPO);

    // Se lee: la navegación trae las secciones que la extensión declaró.
    let navegacion = proveedor.navigation().expect("navegación");
    assert_eq!(navegacion.entries.len(), 2);
    assert_eq!(navegacion.entries[0].title, "Primer capítulo");
    assert_eq!(navegacion.entries[1].title, "Segundo capítulo");

    // Se ancla con Locator, y la ida y vuelta es la misma posición. El modelo
    // de ADR-004 lo aplica el núcleo, no la extensión.
    let posicion = NativePosition::Fragment {
        href: "s2".to_owned(),
        offset: 10,
    };
    let locator = proveedor.locator_at(&posicion).expect("locator");
    let resuelto = proveedor.resolve(&locator).expect("resuelve");
    assert_eq!(resuelto.position, posicion);

    // Y se busca: el texto sale en trozos anclados, que es de lo que vive el
    // índice.
    let trozos = proveedor.text_chunks(4096).expect("trozos");
    assert!(!trozos.is_empty());
    assert!(trozos.iter().any(|(_, t)| t.contains("Macondo")));
}

#[test]
fn un_documento_que_no_es_de_ese_formato_se_rechaza_como_corrupto_y_no_como_averia() {
    let raiz = temporal("corrupto");
    let Some(paquete) = paquete_del_formato(&raiz) else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");
    subsistema.almacen.instalar(&paquete).expect("instala");
    let mut registro = ProviderRegistry::new();
    format::registrar(&mut registro, &subsistema, Limites::default()).expect("registra");

    // Bytes que no son UTF-8. Que la extensión lo diga es un documento
    // corrupto, no un fallo de la extensión: son cosas distintas y la interfaz
    // reacciona distinto a cada una.
    let ruta = raiz.join("roto.txt");
    std::fs::write(&ruta, [0xff, 0xfe, 0x00]).expect("roto");
    // `Box<dyn DocumentProvider>` no implementa `Debug`, así que el resultado
    // se desarma a mano en vez de con `expect_err`.
    let Err(error) = registro.open(TIPO, &ruta) else {
        panic!("un archivo que no es de ese formato no se puede abrir");
    };
    assert_eq!(error.code(), "DOCUMENT_CORRUPTED");
}

#[test]
fn una_extension_desactivada_no_aporta_su_formato() {
    let raiz = temporal("apagada");
    let Some(paquete) = paquete_del_formato(&raiz) else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");
    subsistema.almacen.instalar(&paquete).expect("instala");
    subsistema
        .almacen
        .habilitar("lat.nbdigitallabs.books.banco.formato-de-texto", false)
        .expect("apaga");

    let mut registro = ProviderRegistry::new();
    let cuantos =
        format::registrar(&mut registro, &subsistema, Limites::default()).expect("registra");

    // Apagarla tiene que quitar lo que aporta. Si no, desactivar sería un
    // adorno: la extensión seguiría corriendo cuando alguien abriera un
    // documento de su formato.
    assert_eq!(cuantos, 0);
    assert!(!registro.supports(TIPO));
}

#[test]
fn un_formato_que_apunta_fuera_de_su_carpeta_no_se_registra() {
    let raiz = temporal("escape");
    let Some(construido) = componente_construido() else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");

    // Un componente **de verdad**, colocado exactamente donde el escape lo
    // alcanzaría: tres niveles por encima de `<raíz>/extensiones/store/<hash>`.
    //
    // La primera versión de esta prueba apuntaba a un archivo que no existía,
    // así que pasaba **por el motivo equivocado**: se omitía por "no está" y no
    // por contención, y quitar el acotado la dejaba en verde. Lo destapó una
    // mutación, y es la cuarta vez que ocurre lo mismo en esta fase.
    std::fs::copy(
        construido.join("formato_de_texto.wasm"),
        raiz.join("fuera.wasm"),
    )
    .expect("el objetivo del escape");

    let dir = raiz.join("paquete-escapista");
    std::fs::create_dir_all(&dir).expect("paquete");
    let manifiesto = serde_json::json!({
        "id": "org.ejemplo.formato-escapista",
        "name": "Se sale de su carpeta",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": { "providers": { "format:text/x-escape": "../../../fuera.wasm" } }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    // Ahora no llega ni a instalarse: fuera de su carpeta, el módulo falta.
    assert_eq!(
        subsistema.almacen.instalar(&dir).map(|_| ()),
        Err(nb_books_lib::error::NbError::ExtensionManifestInvalid)
    );

    // Y el registro sigue acotando, por lo que entró antes de esa comprobación:
    // se instala sano y se reescribe el manifiesto guardado para que apunte fuera.
    std::fs::copy(
        construido.join("formato_de_texto.wasm"),
        dir.join("modulo.wasm"),
    )
    .expect("módulo dentro");
    let mut sano = manifiesto.clone();
    sano["contributes"]["providers"]["format:text/x-escape"] = "modulo.wasm".into();
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&sano).expect("json"),
    )
    .expect("manifiesto sano");
    let instalada = subsistema
        .almacen
        .instalar(&dir)
        .expect("el paquete sano entra");
    std::fs::write(
        subsistema
            .almacen
            .ruta_de(&instalada)
            .join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("el manifiesto guardado apunta fuera");

    let mut registro = ProviderRegistry::new();
    let cuantos =
        format::registrar(&mut registro, &subsistema, Limites::default()).expect("registra");

    // La ruta la escribe quien publica la extensión, así que se acota antes de
    // leer nada. Y omitir un formato roto no puede impedir que se registren los
    // demás ni que la aplicación arranque.
    assert_eq!(cuantos, 0);
    assert!(!registro.supports("text/x-escape"));
}
