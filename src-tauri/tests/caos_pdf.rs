//! Escenario de caos: ausencia de la biblioteca nativa de PDF.
//!
//! Es el tercero de los tres que exigía el criterio y el único que no
//! se pudo escribir, porque no había motor de PDF cuya ausencia
//! simular. Con el enlazado en tiempo de ejecución, simularla es
//! apuntar a una ruta que no existe.
//!
//! Lo que se comprueba es la regla 1 de la Constitución: **el fallo de un
//! componente produce consecuencias locales**. Sin motor de PDF se pierde el
//! PDF, no la aplicación.
//!
//! Este archivo es su propio binario de pruebas a propósito: el enlace con la
//! biblioteca se resuelve una sola vez por proceso, así que simular su ausencia
//! exige un proceso donde nadie la haya cargado antes.

use std::path::PathBuf;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::pdf::{engine_state, Engine, PdfFactory, RUTA_ENV};
use nb_books_lib::reader::ProviderRegistry;

/// Declara que en este proceso no hay biblioteca de PDF.
fn sin_biblioteca() {
    // SAFETY: este binario de pruebas no comparte proceso con ningún otro, y
    // todas sus pruebas quieren exactamente este estado.
    unsafe { std::env::set_var(RUTA_ENV, "/no/existe/libpdfium.so") };
}

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-caos-pdf-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio");
    dir
}

/// Sin biblioteca, el motor se declara no instalado. No falla: no está.
#[test]
fn caos_sin_biblioteca_el_motor_se_declara_no_instalado() {
    sin_biblioteca();
    assert_eq!(engine_state(), Engine::NotInstalled);
}

/// **El escenario.** Falta el motor de PDF y la biblioteca sigue funcionando:
/// se importan documentos, se abren los EPUB, y solo el PDF se declara no
/// disponible.
#[test]
fn caos_sin_biblioteca_de_pdf_el_resto_de_la_aplicacion_sigue_entera() {
    sin_biblioteca();

    let dir = temporal("degradacion");
    let scope = dir.join("origen");
    let library = dir.join("biblioteca");
    std::fs::create_dir_all(&scope).expect("ámbito");
    std::fs::create_dir_all(&library).expect("biblioteca");

    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let db = Database::open_in_memory().expect("base");
    let bus = EventBus::new();

    // --- La biblioteca funciona: entra un EPUB y entra un PDF ---
    let mut importados = Vec::new();
    for (fixture, nombre) in [
        ("minimo-valido.epub", "libro.epub"),
        ("minimo-valido.pdf", "documento.pdf"),
    ] {
        let ruta = scope.join(nombre);
        std::fs::copy(fixtures.join(fixture), &ruta).expect("copia");

        let mut conn = db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &scope,
                library: &library,
                bus: &bus,
            },
            &ImportRequest {
                source: ruta.clone(),
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("la importación no depende del motor de PDF");

        let ImportOutcome::Imported(importado) = resultado else {
            panic!("debía importarse");
        };
        importados.push((importado.file.media_type.clone(), ruta));
    }

    assert_eq!(importados.len(), 2, "los dos entran en el catálogo");

    // --- El lector: el EPUB se abre, el PDF se declara no disponible ---
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(EpubFactory::solo_texto()));
    registro.register(Box::new(PdfFactory::new(dir.join("cache"))));

    let epub = importados
        .iter()
        .find(|(tipo, _)| tipo == "application/epub+zip")
        .expect("epub");
    let doc = registro
        .open(&epub.0, &epub.1)
        .expect("el EPUB se abre sin motor de PDF");
    assert!(!doc.navigation().expect("navegación").entries.is_empty());

    let pdf = importados
        .iter()
        .find(|(tipo, _)| tipo == "application/pdf")
        .expect("pdf");
    assert_eq!(
        registro.open(&pdf.0, &pdf.1).err(),
        Some(NbError::ComponentUnavailable),
        "sin motor, el PDF no está disponible; no es que esté roto"
    );

    // Y el documento **sigue en el catálogo**: perder el motor no puede perder
    // los libros, ni sus metadatos, ni sus anotaciones.
    let filas: i64 = db
        .connection()
        .expect("conexión")
        .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .expect("consulta");
    assert_eq!(filas, 2);

    let _ = std::fs::remove_dir_all(&dir);
}

/// La distinción que la interfaz necesita: "esta instalación no puede abrir
/// esto" no es lo mismo que "esto no es un documento". Con la primera se oculta
/// la función; con la segunda se le dice al usuario que su archivo está mal.
#[test]
fn caos_la_ausencia_del_motor_no_se_confunde_con_un_documento_roto() {
    sin_biblioteca();

    let dir = temporal("distincion");
    let roto = dir.join("no-es.pdf");
    std::fs::write(&roto, b"desde luego esto no es un pdf").expect("archivo");

    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(PdfFactory::new(dir.join("cache"))));

    // Sin motor, hasta un archivo evidentemente roto responde "no disponible":
    // no se puede afirmar que un documento está mal sin poder mirarlo.
    assert_eq!(
        registro.open("application/pdf", &roto).err(),
        Some(NbError::ComponentUnavailable)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// El registro del arranque y el estado del componente dicen lo mismo.
///
/// Es la trampa que podía quedar abierta: que el monitor declare el lector
/// degradado mientras el registro sigue aceptando PDF, o al revés. Entonces la
/// interfaz ocultaría un formato que el runtime abre, o mostraría uno que no
/// puede abrir, y las dos versiones de la verdad tardarían meses en encontrarse.
#[test]
fn caos_el_registro_del_arranque_y_el_estado_del_componente_no_se_contradicen() {
    sin_biblioteca();

    let dir = temporal("arranque");
    // SAFETY: mismo motivo que en `sin_biblioteca`: este binario no comparte
    // proceso y todas sus pruebas quieren este estado.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &dir) };

    let (estado, _guard) = nb_books_lib::bootstrap().expect("el runtime arranca sin motor de PDF");

    assert!(
        !estado.readers.supports("application/pdf"),
        "sin motor no puede haber proveedor de PDF registrado"
    );
    assert!(
        estado.readers.supports("application/epub+zip"),
        "lo que no depende del motor sigue entero"
    );
    assert!(
        estado.readers.supports("application/vnd.comicbook+zip"),
        "el cómic tampoco depende del motor de PDF"
    );

    let salud = estado.monitor.snapshot();
    let lector = salud
        .iter()
        .find(|c| c.component == nb_books_lib::monitor::components::READER)
        .expect("el lector se declara");
    assert_eq!(
        lector.state,
        nb_books_lib::monitor::ComponentState::Degraded,
        "degradado, no caído: se lee EPUB y cómic igual"
    );
    assert_eq!(lector.reason_code, Some("PDF_ENGINE_MISSING"));
}
