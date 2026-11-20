//! El estado del runtime, montado como en el arranque pero aislado.
//!
//! Vive aquí y no en cada prueba porque lo necesitan varias, y una copia de un
//! constructor de cien líneas que se queda vieja construye un estado que ya no se
//! parece al de verdad sin que nadie lo vea.

use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::monitor::{components, ComponentState, Monitor};
use nb_books_lib::paths::AppPaths;
use nb_books_lib::AppState;

/// Construye un estado equivalente al del arranque, pero aislado en un
/// directorio temporal, con su base en un archivo como en el arranque.
pub fn estado_aislado(nombre: &str) -> (AppState, tempdir::Guard) {
    estado_aislado_con(nombre, false)
}

/// Lo mismo, con el subsistema de extensiones arrancado sobre la carpeta de
/// pruebas si se pide: sin él, sus comandos responden `COMPONENT_UNAVAILABLE`, y
/// para comprobar qué anuncian al instalar hace falta que instalen.
pub fn estado_aislado_con(nombre: &str, con_extensiones: bool) -> (AppState, tempdir::Guard) {
    let guard = tempdir::Guard::nuevo(nombre);
    // SAFETY: cada prueba usa su propio directorio y lo limpia al terminar.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, guard.path()) };

    let paths = AppPaths::resolve().expect("rutas resolubles");
    let paths_indexador = paths.clone();
    let bus = EventBus::new();
    let monitor = Monitor::new(bus.clone());
    // En archivo y en WAL, como la aplicación: la base en memoria comparte caché
    // entre conexiones, y ahí un escritor que se cruza con otro (la extracción
    // de metadatos que sigue a una importación) recibe `SQLITE_LOCKED` en el
    // acto, sin `busy_timeout`. Es un bloqueo que la aplicación no puede tener,
    // y en un ejecutor lento hacía fallar a quien guardaba el progreso.
    let db = Database::open(&guard.path().join("nb-books.sqlite")).expect("base");
    let db_progreso = db.clone();
    let bus_progreso = bus.clone();
    let db_indice = db.clone();
    let bus_indice = bus.clone();
    let readers = std::sync::Arc::new({
        let mut registro = nb_books_lib::reader::ProviderRegistry::new();
        registro.register(Box::new(
            nb_books_lib::reader::epub::EpubFactory::solo_texto(),
        ));
        registro
    });
    let bus_para_voz = bus.clone();
    let monitor_ia = monitor.clone();
    let paths_modelos = paths.models();
    monitor.set(components::DATABASE, ComponentState::Ready, None);

    // El generador de vectores con el índice de la base y sin vectorizador que
    // calcule: el subsistema de IA de esta prueba no tiene modelo instalado.
    let ia_de_prueba = std::sync::Arc::new(nb_books_lib::ai::subsistema::Subsistema::arrancar(
        &paths_modelos,
        monitor_ia.clone(),
        nb_books_lib::ai::presupuesto::Presupuesto::de_fabrica(8),
    ));
    let vectores_de_prueba = std::sync::Arc::new(nb_books_lib::ai::generacion::Generador::new(
        db.clone(),
        std::sync::Arc::new(nb_books_lib::ai::indice::EnLaBase::new(db.clone())),
        std::sync::Arc::clone(&ia_de_prueba)
            as std::sync::Arc<dyn nb_books_lib::ai::generacion::Vectorizador>,
        std::sync::Arc::new(nb_books_lib::ai::generacion::DeLaBiblioteca {
            db: db.clone(),
            registry: std::sync::Arc::clone(&readers),
        }),
        bus.clone(),
        None,
    ));

    let modelos = paths.models();
    let paths_audio = paths.clone();
    let voz_neuronal = nb_books_lib::voice::neuronal::VozNeuronal::nueva(
        &modelos,
        nb_books_lib::ai::presupuesto::Presupuesto::de_fabrica(4),
    );
    let extensiones = con_extensiones.then(|| {
        std::sync::Arc::new(
            nb_books_lib::extensions::Subsistema::arrancar(paths.extensions())
                .expect("el almacén de extensiones de prueba se abre"),
        )
    });
    let repaso_en = paths.cache.join("repaso");
    let db_para_audio = db.clone();
    let db_para_documentos = db.clone();
    let bus_para_audio = bus.clone();
    let (puente, movil) = nb_books_lib::preparar_movil(&db, &paths, &bus, &readers);
    (
        AppState {
            movil,
            puente,
            paths,
            db,
            // Una base recién creada, como la de un perfil nuevo.
            cierre_anterior: nb_books_lib::search::CierreAnterior::PrimeraVez,
            // Arranque limpio: no hubo nada que recuperar.
            recovery: None,
            voice: nb_books_lib::voice::Narrator::start(bus_para_voz),
            voz_neuronal: std::sync::Arc::clone(&voz_neuronal),
            audio: nb_books_lib::voice::preparacion::Preparador::nuevo(
                db_para_audio,
                std::sync::Arc::new(nb_books_lib::ai::generacion::DeLaBiblioteca {
                    db: db_para_documentos,
                    registry: std::sync::Arc::clone(&readers),
                }),
                voz_neuronal as std::sync::Arc<dyn nb_books_lib::voice::neuronal::Sintetizador>,
                &paths_audio,
                bus_para_audio,
            ),
            bus,
            monitor,
            readers: std::sync::Arc::clone(&readers),
            // Con el bus, como en el arranque: cada escritura del progreso se
            // anuncia.
            sessions: nb_books_lib::reader::SessionStore::new(std::sync::Arc::new(
                nb_books_lib::reader::progress::ProgressTracker::new(db_progreso)
                    .con_bus(bus_progreso),
            )),
            indexer: std::sync::Arc::new(nb_books_lib::search::Indexer::start(
                db_indice,
                readers,
                bus_indice,
                paths_indexador,
            )),
            import: std::sync::Arc::new(nb_books_lib::library::CancelToken::default()),
            // Sin subsistema de extensiones salvo que se pida, que es lo que pasa
            // cuando su almacén no se puede abrir. La aplicación arranca igual y
            // sus comandos responden `COMPONENT_UNAVAILABLE`: una extensión rota
            // no puede impedir leer un libro.
            extensions: extensiones,
            // Sin diccionarios instalados: consultar responde
            // `COMPONENT_UNAVAILABLE`, que es justo el caso que la interfaz usa
            // para ocultar la función.
            lookup: std::sync::RwLock::new(nb_books_lib::lookup::LookupRegistry::new()),
            // La puerta real. Ninguna prueba de aquí sale: sin consentimiento
            // concedido, la puerta rechaza antes de llegar al transporte.
            net: std::sync::Arc::new(nb_books_lib::net::Network::new(Box::new(
                nb_books_lib::net::UreqTransport::new(),
            ))),
            // El servicio de IA de verdad, sobre la carpeta de modelos del
            // directorio de pruebas, que está vacía: nadie ha instalado nada.
            ia: ia_de_prueba,
            vectores: vectores_de_prueba,
            busqueda: nb_books_lib::search::BusquedaEnCurso::default(),
            repaso: std::sync::Arc::new(nb_books_lib::repaso::Repaso::nuevo(repaso_en)),
        },
        guard,
    )
}

/// Pide un comando como lo pediría el webview.
pub fn invocar(
    ventana: &tauri::WebviewWindow<tauri::test::MockRuntime>,
    comando: &str,
    argumentos: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    tauri::test::get_ipc_response(
        ventana,
        tauri::webview::InvokeRequest {
            cmd: comando.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .expect("la url del webview es válida"),
            body: tauri::ipc::InvokeBody::Json(argumentos),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|r| {
        r.deserialize::<serde_json::Value>()
            .expect("respuesta en JSON")
    })
}

pub mod tempdir {
    use std::path::{Path, PathBuf};

    pub struct Guard(PathBuf);

    impl Guard {
        pub fn nuevo(nombre: &str) -> Self {
            let dir = nb_books_lib::paths::temporal_de_pruebas().join(format!(
                "nb-books-it-{nombre}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("directorio temporal");
            Self(dir)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
