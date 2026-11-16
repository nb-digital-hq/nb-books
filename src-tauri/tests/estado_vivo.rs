//! Quien escribe, lo anuncia.
//!
//! Es la mitad del runtime de los principios 1 y 2 de la sección 7 de la
//! filosofía de producto: un cambio se ve sin reiniciar y en todas partes a la
//! vez. La interfaz no puede enterarse de lo que nadie publica, así que aquí se
//! comprueba que **cada escritura que cambia algo que se pinta sale al bus**
//! después de confirmarse. La otra mitad -que la interfaz reaccione- la
//! comprueba el recorrido de `e2e/flujo.spec.ts`.
//!
//! Los comandos se piden por el mismo camino que los pediría el webview, con la
//! aplicación simulada de Tauri: lo que se prueba es el comando registrado, no
//! una función de debajo que podría anunciar mientras el comando no lo hace.

mod comun;

use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use comun::estado::{estado_aislado_con, invocar};
use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::reader::progress::ProgressTracker;
use nb_books_lib::reader::{Locator, Precision};
use serde_json::json;
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

/*
 * `estado_aislado` fija la carpeta de pruebas en una variable de entorno, que es
 * del proceso entero. Dos pruebas en paralelo se pisarían la carpeta, y la de
 * extensiones escribe en ella. Se hacen de una en una.
 */
static EN_SERIE: Mutex<()> = Mutex::new(());

/// Lo que el bus publica, en orden, para poder esperar a un evento concreto.
fn escuchar(bus: &EventBus) -> Receiver<DomainEvent> {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    bus.subscribe("prueba-estado-vivo", move |evento| {
        let _ = tx.lock().expect("canal").send(evento.clone());
    });
    rx
}

/// Espera a que llegue un evento con ese nombre y lo devuelve. Los demás se
/// descartan: el bus no garantiza orden entre eventos distintos.
fn esperar(rx: &Receiver<DomainEvent>, nombre: &str) -> Option<DomainEvent> {
    let limite = std::time::Instant::now() + Duration::from_secs(5);
    while let Some(queda) = limite.checked_duration_since(std::time::Instant::now()) {
        match rx.recv_timeout(queda) {
            Ok(evento) if evento.name() == nombre => return Some(evento),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

/// Que no llegue ninguno con ese nombre en un rato razonable.
fn nada(rx: &Receiver<DomainEvent>, nombre: &str) -> bool {
    let limite = std::time::Instant::now() + Duration::from_millis(300);
    while let Some(queda) = limite.checked_duration_since(std::time::Instant::now()) {
        match rx.recv_timeout(queda) {
            Ok(evento) if evento.name() == nombre => return false,
            Ok(_) => {}
            Err(_) => return true,
        }
    }
    true
}

/// La aplicación simulada con los comandos que escriben, y el estado dentro.
fn aplicacion(estado: nb_books_lib::AppState) -> tauri::WebviewWindow<MockRuntime> {
    let app = mock_builder()
        .manage(estado)
        .invoke_handler(tauri::generate_handler![
            nb_books_lib::ipc::collections_create,
            nb_books_lib::ipc::collections_rename,
            nb_books_lib::ipc::collections_add,
            nb_books_lib::ipc::collections_remove,
            nb_books_lib::ipc::collections_delete,
            nb_books_lib::ipc::accept_suggestion,
            nb_books_lib::ipc::reject_suggestion,
            nb_books_lib::ipc::extensions_install,
            nb_books_lib::ipc::extensions_set_enabled,
            nb_books_lib::ipc::extensions_uninstall,
            nb_books_lib::ipc::open_document,
            nb_books_lib::ipc::save_progress,
            nb_books_lib::ipc::flush_progress,
        ])
        .build(mock_context(noop_assets()))
        .expect("la aplicación de prueba se construye");
    tauri::WebviewWindowBuilder::new(&app, "principal", Default::default())
        .build()
        .expect("la ventana de prueba se crea")
}

fn fixtures() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Importa un documento del corpus de pruebas y devuelve archivo y edición.
fn importar(estado: &nb_books_lib::AppState, fixture: &str) -> (String, String) {
    let origen = estado.paths.library().join("origen");
    std::fs::create_dir_all(&origen).expect("origen");
    let destino = origen.join(fixture);
    std::fs::copy(fixtures().join(fixture), &destino).expect("copia");
    let mut conn = estado.db.connection().expect("conexión");
    match import_document(
        &mut conn,
        &ImportContext {
            scope: &origen,
            library: &estado.paths.library(),
            bus: &estado.bus,
        },
        &ImportRequest {
            source: destino,
            storage_mode: StorageMode::Referenced,
            root: None,
        },
    )
    .expect("importación")
    {
        ImportOutcome::Imported(datos) => (datos.file.id.clone(), datos.edition.id.clone()),
        ImportOutcome::AlreadyInLibrary(archivo) | ImportOutcome::Duplicate(archivo) => {
            (archivo.id.clone(), archivo.edition_id.clone())
        }
    }
}

// --- Colecciones ----------------------------------------------------------

#[test]
fn cada_cambio_de_una_coleccion_sale_al_bus_despues_de_escribirse() {
    let _serie = EN_SERIE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (estado, _guarda) = estado_aislado_con("vivo-colecciones", false);
    let (_archivo, edicion) = importar(&estado, "minimo-valido.epub");
    let rx = escuchar(&estado.bus);
    let ventana = aplicacion(estado);

    let creada =
        invocar(&ventana, "collections_create", json!({ "name": "Tesis" })).expect("se crea");
    let id = creada["id"].as_str().expect("identificador").to_owned();
    assert!(
        matches!(
            esperar(&rx, "CollectionsChanged"),
            Some(DomainEvent::CollectionsChanged {
                collection_id: None
            })
        ),
        "crear cambia la lista entera, y eso se anuncia"
    );

    invocar(
        &ventana,
        "collections_add",
        json!({ "id": id, "editionIds": [edicion] }),
    )
    .expect("se añade");
    match esperar(&rx, "CollectionsChanged") {
        Some(DomainEvent::CollectionsChanged { collection_id }) => {
            assert_eq!(collection_id.as_deref(), Some(id.as_str()));
        }
        otro => panic!("añadir una obra tiene que anunciar su colección: {otro:?}"),
    }

    // Añadir lo que ya estaba no cambia nada, y un anuncio que no corresponde a
    // un cambio hace que todas las pantallas vuelvan a preguntar por nada.
    invocar(
        &ventana,
        "collections_add",
        json!({ "id": id, "editionIds": [edicion] }),
    )
    .expect("se añade otra vez");
    assert!(nada(&rx, "CollectionsChanged"));

    invocar(
        &ventana,
        "collections_rename",
        json!({ "id": id, "name": "Tesis Doctoral" }),
    )
    .expect("se renombra");
    assert!(esperar(&rx, "CollectionsChanged").is_some());

    invocar(
        &ventana,
        "collections_remove",
        json!({ "id": id, "editionId": edicion }),
    )
    .expect("se quita");
    assert!(esperar(&rx, "CollectionsChanged").is_some());

    invocar(&ventana, "collections_delete", json!({ "id": id })).expect("se borra");
    assert!(esperar(&rx, "CollectionsChanged").is_some());
}

// --- Metadatos ------------------------------------------------------------

#[test]
fn aceptar_una_propuesta_anuncia_la_edicion_que_cambio() {
    let _serie = EN_SERIE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (estado, _guarda) = estado_aislado_con("vivo-metadatos", false);
    let (_archivo, edicion) = importar(&estado, "minimo-valido.epub");

    // Una propuesta viva sobre el título: con una decisión del usuario puesta,
    // un catálogo no sobrescribe, propone.
    let proponer = |db: &Database, valor: &str| -> String {
        let conn = db.connection().expect("conexión");
        match nb_books_lib::metadata::propose(
            &conn,
            &edicion,
            nb_books_lib::metadata::Field::Title,
            nb_books_lib::metadata::Source::Online,
            Some(valor),
        )
        .expect("se propone")
        {
            nb_books_lib::metadata::Outcome::Suggested(propuesta) => propuesta.id,
            otro => panic!("con decisión manual, un catálogo propone: {otro:?}"),
        }
    };
    {
        let conn = estado.db.connection().expect("conexión");
        nb_books_lib::metadata::set_manual(
            &conn,
            &edicion,
            nb_books_lib::metadata::Field::Title,
            Some("El Título que Puse"),
        )
        .expect("decisión manual");
    }
    let db = estado.db.clone();
    let propuesta = proponer(&db, "Un Título Propuesto");

    let rx = escuchar(&estado.bus);
    let ventana = aplicacion(estado);

    invocar(&ventana, "reject_suggestion", json!({ "id": propuesta })).expect("se rechaza");
    // Rechazar deja el dato como estaba: no hay nada que anunciar.
    assert!(nada(&rx, "MetadataUpdated"));

    let otra = proponer(&db, "Otro Título Propuesto");
    invocar(&ventana, "accept_suggestion", json!({ "id": otra })).expect("se acepta");
    match esperar(&rx, "MetadataUpdated") {
        Some(DomainEvent::MetadataUpdated { edition_id }) => assert_eq!(edition_id, edicion),
        otro => panic!("aceptar cambia el título, y eso se anuncia con su edición: {otro:?}"),
    }
}

#[test]
fn lo_que_el_servicio_de_metadatos_extrae_al_importar_tambien_se_anuncia() {
    let _serie = EN_SERIE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (estado, _guarda) = estado_aislado_con("vivo-extraccion", false);
    nb_books_lib::metadata::subscribe(
        &estado.bus,
        estado.db.clone(),
        estado.paths.cache.join("portadas"),
    );
    let rx = escuchar(&estado.bus);

    let (_archivo, edicion) = importar(&estado, "minimo-valido.epub");

    // El documento entra con el nombre de su archivo; el título de verdad llega
    // después, por el bus, y quien lo pinta tiene que enterarse.
    match esperar(&rx, "MetadataUpdated") {
        Some(DomainEvent::MetadataUpdated { edition_id }) => assert_eq!(edition_id, edicion),
        otro => panic!("lo extraído al importar se anuncia: {otro:?}"),
    }
}

// --- Extensiones ----------------------------------------------------------

#[test]
fn instalar_apagar_y_desinstalar_una_extension_se_anuncia() {
    let _serie = EN_SERIE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (estado, _guarda) = estado_aislado_con("vivo-extensiones", true);
    let rx = escuchar(&estado.bus);
    let ventana = aplicacion(estado);

    // El tema de ejemplo: no trae código ni pide permisos, así que se instala
    // sin consentimiento y sin compilar nada.
    let tema = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raíz")
        .join("extensiones/tema-de-prueba");
    let instalada = invocar(
        &ventana,
        "extensions_install",
        json!({ "path": tema.to_string_lossy() }),
    )
    .expect("se instala");
    let id = instalada["detalle"]["id"]
        .as_str()
        .expect("se instaló de verdad, no pidió nada")
        .to_owned();
    assert!(
        esperar(&rx, "ExtensionsChanged").is_some(),
        "instalar se anuncia"
    );

    invocar(
        &ventana,
        "extensions_set_enabled",
        json!({ "id": id, "enabled": false }),
    )
    .expect("se apaga");
    assert!(
        esperar(&rx, "ExtensionsChanged").is_some(),
        "apagar se anuncia"
    );

    invocar(&ventana, "extensions_uninstall", json!({ "id": id })).expect("se desinstala");
    assert!(
        esperar(&rx, "ExtensionsChanged").is_some(),
        "desinstalar se anuncia"
    );
}

// --- Progreso -------------------------------------------------------------

/// Una edición de verdad en la base, para que el progreso tenga a quién colgarse.
fn edicion_de_prueba(db: &Database, dir: &std::path::Path) -> String {
    let origen = dir.join("origen");
    let biblioteca = dir.join("biblioteca");
    std::fs::create_dir_all(&origen).expect("origen");
    std::fs::create_dir_all(&biblioteca).expect("biblioteca");
    let destino = origen.join("minimo-valido.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &destino).expect("copia");
    let mut conn = db.connection().expect("conexión");
    match import_document(
        &mut conn,
        &ImportContext {
            scope: &origen,
            library: &biblioteca,
            bus: &EventBus::new(),
        },
        &ImportRequest {
            source: destino,
            storage_mode: StorageMode::Referenced,
            root: None,
        },
    )
    .expect("importación")
    {
        ImportOutcome::Imported(datos) => datos.edition.id.clone(),
        ImportOutcome::AlreadyInLibrary(archivo) | ImportOutcome::Duplicate(archivo) => {
            archivo.edition_id.clone()
        }
    }
}

fn locator_con(progresion: f64) -> Locator {
    let mut locator: Locator = serde_json::from_value(json!({
        "version": 1,
        "href": "OEBPS/c1.xhtml",
        "mediaType": "application/xhtml+xml",
        "locations": { "progression": progresion, "totalProgression": progresion },
        "text": { "highlight": "algo" }
    }))
    .expect("locator");
    locator.locations.total_progression = Some(progresion);
    locator
}

#[test]
fn cada_escritura_del_progreso_se_anuncia_con_su_cifra_y_lo_apuntado_no() {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-vivo-progreso-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("carpeta");
    let db = Database::open(&dir.join("nb.sqlite")).expect("base");
    let bus = EventBus::new();
    let rx = escuchar(&bus);

    // Una hora de intervalo: solo la primera posición se escribe por tiempo, y
    // el resto queda apuntado. Así se distingue anunciar lo escrito de anunciar
    // lo apuntado, que es la diferencia que importa.
    let tracker = Arc::new(
        ProgressTracker::with_interval(db.clone(), Duration::from_secs(3600)).con_bus(bus),
    );
    let edicion = edicion_de_prueba(&db, &dir);

    tracker
        .note(&edicion, &locator_con(0.1), Precision::Exact)
        .expect("primera");
    match esperar(&rx, "ProgressSaved") {
        Some(DomainEvent::ProgressSaved {
            edition_id,
            progression,
        }) => {
            assert_eq!(edition_id, edicion);
            assert_eq!(progression, Some(0.1));
        }
        otro => panic!("la primera posición se escribe y se anuncia: {otro:?}"),
    }

    tracker
        .note(&edicion, &locator_con(0.4), Precision::Exact)
        .expect("segunda");
    assert!(
        nada(&rx, "ProgressSaved"),
        "lo apuntado y no escrito no se anuncia: quien volviera a preguntar leería otra cifra"
    );

    // Salir del lector sin cerrarlo escribe lo apuntado, y lo anuncia.
    tracker
        .write_pending(&edicion)
        .expect("se escribe lo pendiente");
    match esperar(&rx, "ProgressSaved") {
        Some(DomainEvent::ProgressSaved { progression, .. }) => {
            assert_eq!(progression, Some(0.4));
        }
        otro => panic!("escribir lo pendiente se anuncia: {otro:?}"),
    }
    let guardado = {
        let conn = db.connection().expect("conexión");
        nb_books_lib::reader::progress::load(&conn, &edicion, "default")
            .expect("consulta")
            .expect("hay progreso")
    };
    assert_eq!(
        guardado.locator.locations.total_progression,
        Some(0.4),
        "lo que se anuncia es lo que ya está en la base"
    );

    // Y sigue siguiéndolo: no es un cierre. Lo siguiente vuelve a quedar
    // apuntado, y cerrar lo escribe.
    tracker
        .note(&edicion, &locator_con(0.7), Precision::Exact)
        .expect("tercera");
    assert!(nada(&rx, "ProgressSaved"));
    assert_eq!(tracker.pending().expect("pendientes"), 1);
    tracker.flush(&edicion).expect("cierre");
    assert!(
        esperar(&rx, "ProgressSaved").is_some(),
        "cerrar también se anuncia"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn salir_del_lector_sin_cerrarlo_deja_en_la_base_la_misma_cifra_que_el_lector() {
    let _serie = EN_SERIE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (estado, _guarda) = estado_aislado_con("vivo-lector", false);
    let (archivo, edicion) = importar(&estado, "minimo-valido.epub");
    let db = estado.db.clone();
    let rx = escuchar(&estado.bus);
    let ventana = aplicacion(estado);

    let abierta =
        invocar(&ventana, "open_document", json!({ "fileId": archivo })).expect("se abre");
    let mut destino: Locator =
        serde_json::from_value(abierta["navigation"]["entries"][0]["locator"].clone())
            .expect("el índice trae un locator");
    // Dos movimientos seguidos: el primero se escribe y el segundo queda apuntado
    // por el antirrebote de cinco segundos.
    invocar(
        &ventana,
        "save_progress",
        json!({ "fileId": archivo, "locator": destino }),
    )
    .expect("primer movimiento");
    let _ = esperar(&rx, "ProgressSaved");
    destino.locations.total_progression = Some(0.63);
    invocar(
        &ventana,
        "save_progress",
        json!({ "fileId": archivo, "locator": destino }),
    )
    .expect("segundo movimiento");

    invocar(&ventana, "flush_progress", json!({ "fileId": archivo })).expect("se escribe");
    match esperar(&rx, "ProgressSaved") {
        Some(DomainEvent::ProgressSaved { progression, .. }) => {
            assert_eq!(progression, Some(0.63));
        }
        otro => panic!("salir del lector escribe y anuncia lo último: {otro:?}"),
    }
    let conn = db.connection().expect("conexión");
    let guardado = nb_books_lib::reader::progress::load(&conn, &edicion, "default")
        .expect("consulta")
        .expect("hay progreso");
    assert_eq!(guardado.locator.locations.total_progression, Some(0.63));
    drop(conn);

    // El documento sigue abierto: salir del lector no es cerrarlo.
    invocar(
        &ventana,
        "save_progress",
        json!({ "fileId": archivo, "locator": destino }),
    )
    .expect("la sesión sigue viva");
}
