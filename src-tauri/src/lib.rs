//! Runtime de NB Books.
//!
//! El runtime es pequeño y los dominios viven en componentes independientes que
//! se comunican por eventos, no por llamadas directas. Ver
//! `docs/architecture/system-overview.md`.

pub mod actualizacion;
pub mod annotations;
pub mod artefactos;
pub mod boards;
pub mod browse;
pub mod calibre;
pub mod collections;
pub mod covers;
pub mod db;
pub mod diagnostics;
pub mod entidades;
pub mod epub;
pub mod error;
pub mod events;
pub mod extensions;
pub mod home;
pub mod imports;
pub mod ipc;
pub mod paths;
pub mod portability;
pub mod power;
pub mod recovery;
pub mod repaso;
pub mod secrets;
pub mod security;
pub mod settings;
pub mod speech;
pub mod telemetry;
pub mod vocabulary;
pub mod voice;
pub mod zip;

// Componentes del runtime. Cada uno corresponde a un servicio descrito en
// docs/architecture/system-overview.md. Se implementan.
pub mod ai;
pub mod library;
pub mod lookup;
pub mod metadata;
pub mod metadata_online;
pub mod monitor;
pub mod net;
pub mod notes;
pub mod opds;
pub mod reader;
pub mod search;

use tauri::{Emitter, Manager};

use crate::db::Database;
use crate::error::NbResult;
use crate::events::EventBus;
use crate::monitor::{components, ComponentState, Monitor};
use crate::paths::AppPaths;

/// Estado compartido que el runtime inyecta en los comandos IPC.
pub struct AppState {
    pub paths: AppPaths,
    pub db: Database,
    /// Cómo terminó la sesión anterior, leído antes de marcar esta.
    pub cierre_anterior: crate::search::CierreAnterior,
    /// Qué pasó al abrir la biblioteca, cuando hubo que recuperarla.
    ///
    /// `None` en el arranque normal. Cuando lleva algo, la interfaz **tiene que
    /// decirlo**: la biblioteca está vacía y el usuario no lo pidió, y una
    /// biblioteca vacía sin explicación es la pérdida silenciosa que el producto
    /// existe para impedir.
    pub recovery: Option<crate::recovery::RecoveryReport>,
    pub bus: EventBus,
    pub monitor: Monitor,
    /// Proveedores de documento disponibles en **esta** instalación.
    ///
    /// Se construye una sola vez y refleja lo que hay de verdad: si falta el
    /// motor de PDF, aquí no hay proveedor de PDF, y abrir uno responde
    /// `COMPONENT_UNAVAILABLE` en lugar de fallar más adentro con un error que
    /// no distingue "falta un motor" de "el libro está roto".
    pub readers: std::sync::Arc<crate::reader::ProviderRegistry>,
    /// El subsistema de extensiones, si arrancó.
    ///
    /// `None` cuando no se pudo abrir su almacén o su motor. Es la misma
    /// decisión que con los proveedores de documento: si falta, la interfaz
    /// responde `COMPONENT_UNAVAILABLE` y **la aplicación arranca igual**. Una
    /// extensión rota no puede impedir leer un libro.
    pub extensions: Option<std::sync::Arc<crate::extensions::Subsistema>>,
    /// Documentos abiertos ahora mismo. Ver `reader::SessionStore`.
    pub sessions: crate::reader::SessionStore,
    /// La voz del runtime. Existe porque el webview de Linux no tiene la suya.
    pub voice: crate::voice::Narrator,
    /// La voz neuronal local, que **no carga nada hasta que se pide**.
    ///
    /// Va en el estado y no dentro del subsistema de IA porque leer en alto no es
    /// una capacidad de un proveedor de IA: no la ofrece ninguno de los externos y
    /// no entra en el registro de envíos. Comparte con ellos lo que sí es común:
    /// el tope de memoria y la disposición de instalación.
    pub voz_neuronal: std::sync::Arc<crate::voice::neuronal::VozNeuronal>,
    /// Quien prepara el audio de un capítulo o de un libro.
    ///
    /// Vive aquí y no dentro de la voz porque no es de la voz: es un trabajo de
    /// fondo con cola, cancelación y lo que guarda en la base, igual que la
    /// generación de vectores. Lo que comparten es el sintetizador.
    pub audio: std::sync::Arc<crate::voice::preparacion::Preparador>,
    /// Indexador de texto completo, con su hilo propio.
    ///
    /// Vive en el estado y no en una variable local del arranque porque su
    /// `Drop` es lo que espera al hilo: soltarlo antes de tiempo dejaría el hilo
    /// escribiendo sobre una base que se está cerrando.
    pub indexer: std::sync::Arc<crate::search::Indexer>,
    /// La importación en curso, si la hay.
    ///
    /// Existe porque una carpeta de ciento cincuenta mil documentos tarda horas:
    /// el comando no puede esperar a terminar, y quien la lanzó tiene que poder
    /// pararla. Sin esto, la única forma de cancelar sería cerrar la aplicación,
    /// que es exactamente lo que el runtime evita y no se puede perder
    /// al llegar a la interfaz.
    pub import: std::sync::Arc<crate::library::CancelToken>,
    /// Proveedores de consulta: diccionarios instalados y, los que
    /// responden por la red.
    ///
    /// Detrás de un `RwLock` porque instalar o desinstalar un diccionario lo
    /// **cambia** mientras la aplicación corre, y no hacerlo obligaría a
    /// reiniciar para poder consultar lo que se acaba de instalar. Se lee en
    /// cada consulta y se escribe solo al instalar y al desinstalar.
    pub lookup: std::sync::RwLock<crate::lookup::LookupRegistry>,
    /// La puerta de salida de red. Nada sale por otro sitio.
    ///
    /// Compartida con los proveedores de consulta en línea: un `Arc` y no una
    /// copia porque **la puerta tiene que ser una**. Con dos, el registro de
    /// salidas contaría la mitad de lo que ocurrió.
    pub net: std::sync::Arc<crate::net::Network>,
    /// El servicio de IA: proveedor local, catálogo e instalación en curso.
    pub ia: std::sync::Arc<crate::ai::subsistema::Subsistema>,
    /// La generación de vectores en segundo plano. Solo trabaja cuando
    /// alguien la pide, y entre tanto no hay ningún hilo vivo.
    pub vectores: std::sync::Arc<crate::ai::generacion::Generador>,
    /// La búsqueda de la barra superior que está corriendo, para que la
    /// siguiente la corte en lugar de esperar detrás de ella. Ver
    /// `search::BusquedaEnCurso`.
    pub busqueda: crate::search::BusquedaEnCurso,
    /// El repaso de las carpetas referenciadas, que es como se entera la
    /// biblioteca de lo que cambió en el disco sin vigilarlo.
    pub repaso: std::sync::Arc<crate::repaso::Repaso>,
}

/// Prepara **lo que la ventana necesita para existir**: rutas, observabilidad,
/// base de datos y los componentes que la interfaz consulta al montarse.
///
/// Se separa de [`run`] para que las pruebas puedan construir el estado sin
/// levantar una ventana.
///
/// # Lo que deliberadamente no está aquí
///
/// Todo lo que crece con la biblioteca, que vive en [`arranque_diferido`] y
/// corre con la ventana ya abierta. La razón está medida: el 2026-09-09, con el
/// corpus real dentro, entre "proveedores de documento registrados" y
/// "documentos pendientes de indexar" pasaron **nueve minutos y trece
/// segundos**, y como `bootstrap` corre antes de `tauri::Builder`, durante esos
/// nueve minutos **no había ninguna ventana que pintar**. Lo que sí se queda
/// aquí costó 44 ms sobre esa misma biblioteca.
///
/// # Errors
///
/// Propaga el error de resolución de rutas o de apertura de la base de datos.
/// En particular, [`crate::error::NbError::SchemaMismatch`] cuando la base es
/// más nueva que esta versión de la aplicación.
pub fn bootstrap() -> NbResult<(AppState, telemetry::TelemetryGuard)> {
    let paths = AppPaths::resolve()?;
    let guard = telemetry::init(&paths.logs);

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "arrancando el runtime de NB Books"
    );

    let bus = EventBus::new();
    let monitor = Monitor::new(bus.clone());
    let monitor_para_sesiones = monitor.clone();
    let bus_para_voz = bus.clone();

    monitor.set(components::DATABASE, ComponentState::Starting, None);
    let (db, recuperacion) = match crate::recovery::open_or_recover(&paths.database()) {
        Ok((db, informe)) => {
            // Recuperado no es lo mismo que listo: la biblioteca está vacía y el
            // usuario tiene que enterarse. `Degraded` es lo que hace que la
            // interfaz lo enseñe en lugar de fingir que todo va bien.
            let estado = if informe.is_some() {
                ComponentState::Degraded
            } else {
                ComponentState::Ready
            };
            let motivo = informe.as_ref().map(|_| "LIBRARY_RECOVERED");
            monitor.set(components::DATABASE, estado, motivo);
            (db, informe)
        }
        Err(error) => {
            // Ni recuperando. Es el único componente cuyo fallo detiene el
            // arranque; se registra el estado antes de propagar para que quede
            // en el diagnóstico exportable, y `run` enseña un diálogo nativo en
            // lugar de morir en silencio.
            monitor.set(
                components::DATABASE,
                ComponentState::Failed,
                Some(error.code()),
            );
            return Err(error);
        }
    };

    tracing::info!(schema = db.schema_version()?, "base de datos lista");

    // Desde aquí y hasta que se salga bien, esta sesión cuenta como sucia. Es lo
    // que permite comprobar el índice solo cuando hay motivo, en vez de en cada
    // arranque: ver `search::repair_index_si_hace_falta`. **Cómo terminó la
    // anterior se lee antes**, y se guarda: el arranque diferido, que es quien lo
    // necesita, llega cuando esta marca ya está puesta.
    let cierre_anterior = match db.connection() {
        Ok(conn) => {
            let cierre = crate::search::cierre_anterior(&conn);
            crate::search::marcar_cierre(&conn, false);
            if let Some(ruta) = db.path() {
                crate::db::migrations::retirar_respaldo_si_ya_no_hace_falta(
                    &conn,
                    ruta,
                    cierre == crate::search::CierreAnterior::Limpio,
                );
            }
            cierre
        }
        Err(_) => crate::search::CierreAnterior::Sucio,
    };

    // El servicio de metadatos escucha el bus: extrae los metadatos internos y
    // la portada de cada documento que entra, **fuera de la ruta de importación**
    //. Ver docs/architecture/data-flows.md, sección 1.
    //
    // Suscribirse es inmediato y no depende del tamaño de la biblioteca, así que
    // se queda aquí; reconciliar las portadas ya extraídas recorre una carpeta
    // que sí crece con ella, y por eso está en `arranque_diferido`.
    crate::metadata::subscribe(&bus, db.clone(), paths.cache.join("portadas"));
    monitor.set(components::METADATA, ComponentState::Ready, None);

    // El lector depende de un motor nativo que puede no estar. Sin él se lee
    // EPUB pero no PDF, así que el componente queda **degradado**, no caído: la
    // interfaz oculta el PDF y ofrece el resto. Regla 1 de la Constitución.
    let (estado_lector, motivo) = match crate::reader::pdf::engine_state() {
        crate::reader::pdf::Engine::Available => (ComponentState::Ready, None),
        crate::reader::pdf::Engine::NotInstalled => {
            (ComponentState::Degraded, Some("PDF_ENGINE_MISSING"))
        }
    };
    monitor.set(components::READER, estado_lector, motivo);

    // El subsistema de extensiones, con su estado publicado.
    //
    // Se declara aquí y no dentro del estado porque el monitor tiene que
    // enterarse: si no arranca, la interfaz oculta la sección en lugar de
    // ofrecer botones que no hacen nada, y quien pregunte por qué tiene un
    // motivo en lugar de una pantalla vacía.
    let extensiones = match crate::extensions::Subsistema::arrancar(paths.extensions()) {
        Ok(subsistema) => {
            monitor.set(
                crate::monitor::components::EXTENSIONS,
                ComponentState::Ready,
                None,
            );
            Some(std::sync::Arc::new(subsistema))
        }
        Err(_) => {
            tracing::warn!("el subsistema de extensiones no arrancó; se sigue sin él");
            monitor.set(
                crate::monitor::components::EXTENSIONS,
                ComponentState::Failed,
                Some("EXTENSION_STORE_UNAVAILABLE"),
            );
            None
        }
    };

    // El registro de proveedores es la otra cara del estado del componente: lo
    // que el monitor declara degradado es exactamente lo que aquí no se
    // registra. Tenerlos separados dejaría que la interfaz ocultase el PDF
    // mientras el registro sigue aceptándolo, o al revés.
    let mut readers = crate::reader::ProviderRegistry::new();
    readers.register(Box::new(crate::reader::epub::EpubFactory::new(
        paths.cache.join("recursos"),
    )));
    readers.register(Box::new(crate::reader::comic::ComicFactory::new(
        paths.cache.join("paginas"),
    )));
    if estado_lector == ComponentState::Ready {
        readers.register(Box::new(crate::reader::pdf::PdfFactory::new(
            paths.cache.join("paginas"),
        )));
    }

    // Los formatos que aporten las extensiones instaladas. **Esto es lo que
    // hace que añadir un formato sea instalar una extensión y no tocar el
    // núcleo**: el registro no sabe de dónde sale cada fábrica, y una extensión
    // con un formato roto se omite en lugar de impedir el arranque.
    if let Some(subsistema) = &extensiones {
        match crate::extensions::format::registrar(
            &mut readers,
            subsistema,
            crate::extensions::Limites::default(),
        ) {
            Ok(0) => {}
            Ok(cuantos) => tracing::info!(cuantos, "formatos aportados por extensiones"),
            Err(_) => tracing::warn!("no se pudieron registrar los formatos de extensiones"),
        }
    }

    tracing::info!(
        formatos = readers.media_types().len(),
        "proveedores de documento registrados"
    );
    let readers = std::sync::Arc::new(readers);

    // El indexador de texto completo, en su propio hilo. Encola lo que entra por
    // el bus y, al arrancar, lo que quedó sin indexar la vez anterior: por eso no
    // hace falta guardar ningún cursor.
    let indexer = std::sync::Arc::new(crate::search::Indexer::start(
        db.clone(),
        std::sync::Arc::clone(&readers),
        bus.clone(),
        paths.clone(),
    ));
    crate::search::subscribe(&bus, db.clone(), std::sync::Arc::clone(&indexer));

    // La búsqueda existe y funciona, pero su índice todavía no se ha
    // comprobado: eso es `Starting`, no `Ready`. Lo pone en `Ready`
    // `arranque_diferido`, que es quien lo comprueba.
    //
    // Antes este componente se declaraba `NotInstalled` con motivo
    // `NOT_IMPLEMENTED`, heredado de cuando la búsqueda no existía. La interfaz
    // lo creía: `salud.ts` oculta lo que no está instalado y además lo anuncia
    // como algo que el usuario puede instalar. Anunciaba que faltaba un motor
    // que llevaba dentro.
    monitor.set(components::SEARCH, ComponentState::Starting, None);

    // El servicio de IA, con su proveedor local registrado. Su
    // estado **lo calcula él** y lo vuelve a publicar al instalar o quitar un
    // modelo; esta línea no fija nada. Sin ningún modelo instalado se declara
    // detenido y no "no instalado": la IA es opcional, y no descargarla no es algo
    // que necesite atención. Ver `LocalProvider::readiness`.
    //
    // Con el presupuesto guardado: sin él, la primera búsqueda por
    // significado tras arrancar usaría todos los núcleos hasta que alguien tocara
    // Ajustes.
    let presupuesto_de_ia = db.connection().map_or_else(
        |_| crate::ai::presupuesto::Presupuesto::de_fabrica(crate::ai::hardware::nucleos()),
        |conn| crate::ai::presupuesto::Presupuesto::guardado(&conn),
    );
    let ia = std::sync::Arc::new(crate::ai::subsistema::Subsistema::arrancar(
        &paths.models(),
        monitor.clone(),
        presupuesto_de_ia,
    ));
    // Con el mismo presupuesto que la IA: el tope de memoria es de la máquina, no
    // de cada subsistema, y dos topes distintos sumarían el doble sin que nadie lo
    // hubiera pedido.
    let voz_neuronal =
        crate::voice::neuronal::VozNeuronal::nueva(&paths.models(), presupuesto_de_ia);
    // El audio preparado vive en los datos y no en la caché -costó horas de
    // procesador, y una caché es justo lo que se puede borrar sin avisar- pero
    // cuenta contra el presupuesto de lo derivado, y por eso recibe las rutas
    // enteras y no solo su carpeta: para poder medirlo.
    let audio = crate::voice::preparacion::Preparador::nuevo(
        db.clone(),
        std::sync::Arc::new(crate::ai::generacion::DeLaBiblioteca {
            db: db.clone(),
            registry: std::sync::Arc::clone(&readers),
        }),
        std::sync::Arc::clone(&voz_neuronal)
            as std::sync::Arc<dyn crate::voice::neuronal::Sintetizador>,
        &paths,
        bus.clone(),
    );
    // Lo preparado con una voz que el catálogo ya no tiene se retira al arrancar:
    // ver `Preparador::retirar_de_voces_retiradas`.
    let voces: Vec<&str> = crate::voice::catalogo::KOKORO_82M
        .voces
        .iter()
        .map(|v| v.id)
        .collect();
    if let Err(error) = audio.retirar_de_voces_retiradas(&voces) {
        tracing::warn!(error = %error, "no se pudo revisar el audio preparado");
    }

    // Los diccionarios instalados, leídos de su carpeta. La lista de instalados
    // **es** la carpeta, así que aquí no hay ninguna preferencia que
    // consultar ni que pueda discrepar de lo que hay en disco.
    let mut lookup = crate::lookup::instalados(&paths.dictionaries());

    // La puerta de salida, una sola para toda la aplicación.
    let net = std::sync::Arc::new(crate::net::Network::new(Box::new(
        crate::net::UreqTransport::new(),
    )));

    // Los proveedores en línea. Se registran **siempre**: declaran que
    // necesitan red, y con el modo sin red puesto el registro los omite con su
    // motivo en lugar de esconderlos. Que aparezcan omitidos y no desaparecidos
    // es lo que permite decir por qué no están.
    registrar_consulta_en_linea(&mut lookup, &db, &net);

    // El proveedor de IA externo, si hay uno configurado. Aquí y no al
    // arrancar el subsistema porque necesita la puerta de red, que se crea después.
    // Que falle leerlo no impide arrancar: sin proveedor externo, la IA es la local.
    if ia.configurar_externo(&db, &net).is_err() {
        tracing::warn!("no se pudo leer la configuración del proveedor de IA externo");
    }

    // Y cuál de los modelos generativos del catálogo se usa: el que la
    // persona eligiera, o el que se recomienda para este equipo si no eligió.
    if ia.aplicar_modelo_generativo(&db).is_err() {
        tracing::warn!("no se pudo leer el modelo generativo elegido");
    }

    // Clon para el registrador de progreso: `Database` es un pool, así que
    // clonarlo comparte el mismo pool y no abre una base distinta.
    let db_para_progreso = db.clone();
    // Y el bus, para que cada escritura del progreso se anuncie.
    let bus_para_progreso = bus.clone();
    // La foto de las carpetas vive en la caché: es derivada y se regenera sola.
    let repaso_en = paths.cache.clone();

    // La generación de vectores: con el proveedor local del subsistema, el
    // índice de ADR-007 y los documentos por la misma consulta que el indexador.
    // No arranca ningún hilo hasta que alguien pida vectorizar algo.
    let vectores = std::sync::Arc::new(crate::ai::generacion::Generador::new(
        db.clone(),
        std::sync::Arc::new(crate::ai::indice::EnLaBase::new(db.clone())),
        std::sync::Arc::clone(&ia) as std::sync::Arc<dyn crate::ai::generacion::Vectorizador>,
        std::sync::Arc::new(crate::ai::generacion::DeLaBiblioteca {
            db: db.clone(),
            registry: std::sync::Arc::clone(&readers),
        }),
        bus.clone(),
        Some(paths.clone()),
    ));

    Ok((
        AppState {
            extensions: extensiones,
            paths,
            db,
            cierre_anterior,
            recovery: recuperacion,
            bus,
            monitor,
            readers,
            voice: crate::voice::Narrator::start(bus_para_voz),
            voz_neuronal,
            audio,
            sessions: crate::reader::SessionStore::new(std::sync::Arc::new(
                crate::reader::progress::ProgressTracker::new(db_para_progreso)
                    .con_bus(bus_para_progreso),
            ))
            // Con monitor: si el motor de formato entra en pánico, la interfaz
            // se entera de que está roto en lugar de quedarse esperando.
            .con_monitor(monitor_para_sesiones),
            indexer,
            import: std::sync::Arc::new(crate::library::CancelToken::default()),
            lookup: std::sync::RwLock::new(lookup),
            vectores,
            net,
            ia,
            busqueda: crate::search::BusquedaEnCurso::default(),
            repaso: std::sync::Arc::new(crate::repaso::Repaso::nuevo(repaso_en.join("repaso"))),
        },
        guard,
    ))
}

/// El trabajo que puede ocurrir **con la ventana ya abierta**.
///
/// # Por qué existe
///
/// Porque `bootstrap` corre antes de `tauri::Builder`, así que todo lo que se
/// haga ahí ocurre **sin ninguna ventana en la pantalla**. Con ocho archivos de
/// prueba eso son milisegundos y no se nota. Con el corpus real se midió el
/// 2026-09-09: nueve minutos y trece segundos mirando el escritorio, sin
/// ventana, sin icono de carga y sin nada que dijera que la aplicación estaba
/// viva. Quien lo sufre concluye que no abre.
///
/// De ese tiempo, la práctica totalidad es el `integrity-check` de FTS5, que
/// recorre el índice entero; averiguar qué queda por indexar son 722 ms sobre
/// 130.724 documentos. Ninguna de las dos cosas la necesita la ventana para
/// existir, y ninguna la necesita la interfaz para pintar la biblioteca.
///
/// # Por qué no falla nunca
///
/// Nada de lo que hay aquí puede impedir usar la aplicación: el índice es caché
/// y las portadas ya extraídas son caché. Un fallo se registra y se publica como
/// estado del componente -que es lo que la interfaz sabe leer-, y la biblioteca
/// se abre igual. Es la regla 1 de la Constitución aplicada al arranque.
/// Lo que se hace al salir como se debe.
///
/// El progreso se guarda con antirrebote, así que al salir queda la última
/// posición apuntada y sin escribir, que es justo la que le importa al usuario
/// cuando vuelva: se vacía aquí y no se confía en que el `Drop` del estado llegue
/// a ejecutarse. Y se firma la salida: lo que no lleve esta firma la próxima vez
/// es una sesión que murió, y entonces sí hay que comprobar el índice. Va después
/// de vaciar el progreso a propósito: si eso falla, la sesión no cerró bien y no
/// debe decir que sí.
/// Cerrar sesión o apagar manda SIGTERM, y eso no pasaba por `ExitRequested`
///: el arranque siguiente creía que la aplicación se había caído y
/// comprobaba el índice entero. Ahora SIGTERM, SIGHUP y SIGINT piden la misma
/// salida que cerrar la ventana.
///
/// El manejador solo levanta una bandera, que es lo único seguro dentro de una
/// señal; un hilo la mira y sale por el camino normal.
#[cfg(unix)]
fn escuchar_senales_de_cierre(handle: tauri::AppHandle) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PEDIDO: AtomicBool = AtomicBool::new(false);
    extern "C" fn al_recibir(_: libc::c_int) {
        PEDIDO.store(true, Ordering::SeqCst);
    }
    let manejador = al_recibir as extern "C" fn(libc::c_int) as libc::sighandler_t;
    for senal in [libc::SIGTERM, libc::SIGHUP, libc::SIGINT] {
        // SAFETY: el manejador solo escribe en un atómico estático, que es
        // seguro desde una señal; `signal` no tiene más precondiciones.
        unsafe {
            libc::signal(senal, manejador);
        }
    }
    let lanzado = std::thread::Builder::new()
        .name("nb-senales".to_owned())
        .spawn(move || loop {
            if PEDIDO.load(Ordering::SeqCst) {
                tracing::info!("se pidió cerrar desde el sistema: se cierra bien");
                handle.exit(0);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        });
    if lanzado.is_err() {
        tracing::warn!("no se pudo vigilar las señales de cierre");
    }
}

/// La ventana abre al tamaño de la pantalla y no a 1100×720 fijos.
///
/// En una pantalla de 1920 la aplicación ocupaba poco más de la mitad, con cuatro
/// portadas por fila. Se abre al 85 % del área útil, centrada, y nunca por debajo
/// del tamaño de fábrica: en una pantalla pequeña se queda como estaba.
fn ajustar_la_ventana_a_la_pantalla(app: &tauri::App) {
    use tauri::Manager;
    let Some(ventana) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(monitor)) = ventana.current_monitor() else {
        return;
    };
    let escala = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(escala);
    let ancho = (area.width * 0.85).max(1100.0);
    let alto = (area.height * 0.85).max(720.0);
    if ancho <= area.width && alto <= area.height {
        let _ = ventana.set_size(tauri::LogicalSize::new(ancho, alto));
        let _ = ventana.center();
    }
}

/// Fuera de Unix el sistema cierra la ventana, que ya pasa por `ExitRequested`.
#[cfg(not(unix))]
fn escuchar_senales_de_cierre(_handle: tauri::AppHandle) {}

pub fn cerrar_bien(estado: &AppState) {
    if let Err(error) = estado.sessions.tracker().flush_all() {
        tracing::error!(code = error.code(), "quedó progreso sin escribir al salir");
        return;
    }
    if let Ok(conn) = estado.db.connection() {
        crate::search::marcar_cierre(&conn, true);
    }
}

pub fn arranque_diferido(estado: &AppState) {
    let inicio = std::time::Instant::now();

    // Lo primero, y no por importancia: una importación que murió a mitad de
    // copiar deja un `.parcial` que nadie se lleva, y esto solo es correcto
    // mientras no haya ninguna copia en vuelo. La ventana acaba de abrirse, así
    // que ese margen son milisegundos; ponerlo detrás de una comprobación de
    // minutos lo convertiría en una carrera de verdad.
    crate::library::limpiar_copias_a_medias(&estado.paths.library());

    // Las portadas que ya estaban extraídas antes de que hubiera dónde
    // apuntarlas se registran una vez. Ver `reconciliar_portadas`.
    if let Ok(conn) = estado.db.connection() {
        let portadas = estado.paths.cache.join("portadas");
        crate::metadata::reconciliar_portadas(&conn, &portadas);

        // Y los títulos que todavía son el nombre de su archivo, con sus guiones
        // bajos a la vista en la rejilla: 973 de 152.080 en la instalación real.
        crate::metadata::normalizar_titulos_de_archivo(&conn);

        // Y lo que se quedó sin autor porque su extracción falló y nadie la
        // volvió a intentar: el 20 % del catálogo en la instalación real. Va
        // acotada, así que converge en unas cuantas sesiones en lugar de
        // convertir un arranque en una reimportación.
        crate::metadata::reconciliar_metadatos(&conn, &portadas);
    }

    /*
     * Las portadas originales que quedan de se convierten en
     * miniaturas, en su propio hilo y sin tocar la base (ver `covers::compactar`).
     *
     * En un hilo aparte y no aquí porque sobre la instalación real son 152.064
     * archivos y minutos de trabajo, y el resto del arranque diferido no tiene
     * por qué esperarlos. Al terminar avisa al indexador: lo que libera -22 GB en
     * la instalación real- es justo lo que le faltaba para seguir.
     */
    compactar_portadas_en_segundo_plano(estado);

    // La serie y la sinopsis de lo ya importado, que no se leían.
    completar_series_en_segundo_plano(estado);

    /*
     * El hueco que dejó la migración vuelve al disco antes de indexar nada.
     *
     * Va aquí -y no en la migración, que corre dentro de una transacción donde
     * `VACUUM` no cabe- y **antes** del indexador, porque es el indexador quien
     * mira lo que ocupa lo derivado para frenar: medir primero y compactar
     * después le haría frenar contando espacio que ya no se usa. Es justo lo que
     * pasaba el 2026-09-10 con la instalación real, donde 77 de los 111 MB de la
     * base eran hueco tras quitar el texto.
     */
    if let Ok(conn) = estado.db.connection() {
        crate::db::compactar_si_sobra_hueco(&conn);
    }

    // El índice se comprueba y se repara solo. Es caché: un índice corrupto no
    // puede impedir abrir la biblioteca, y detener el arranque por algo que se
    // rehace sería confundir lo regenerable con los datos del usuario.
    //
    // Se cronometra aparte porque es, con diferencia, lo más caro que ocurre al
    // arrancar, y porque el registro es lo único que permite verlo en la máquina
    // de alguien: es un número agregado, sin ninguna ruta ni ningún título.
    let reloj = std::time::Instant::now();
    let (salud, motivo) =
        match estado.db.connection().and_then(|conn| {
            crate::search::repair_index_si_hace_falta(&conn, estado.cierre_anterior)
        }) {
            // La sesión anterior cerró bien, así que no había nada que comprobar.
            Ok(None) | Ok(Some(crate::search::Repair::NotNeeded)) => (ComponentState::Ready, None),
            Ok(Some(crate::search::Repair::Rebuilt)) => {
                tracing::warn!("el índice de búsqueda estaba dañado y se ha rehecho");
                (ComponentState::Ready, None)
            }
            Ok(Some(crate::search::Repair::NeedsFullReindex)) => {
                // Buscar responderá de menos hasta que la indexación de fondo
                // recorra la biblioteca otra vez. Degradado y no listo: la
                // diferencia es exactamente lo que la interfaz tiene que poder
                // decirle a quien busque algo y no lo encuentre.
                tracing::warn!("el índice hay que rehacerlo entero; la búsqueda queda incompleta");
                (ComponentState::Degraded, Some("SEARCH_INDEX_REBUILDING"))
            }
            Err(error) => {
                tracing::error!(
                    code = error.code(),
                    "no se pudo comprobar el índice de búsqueda"
                );
                (ComponentState::Degraded, Some("SEARCH_INDEX_UNCHECKED"))
            }
        };

    tracing::info!(
        ms = reloj.elapsed().as_millis(),
        "índice de búsqueda comprobado"
    );

    // La cola se ceba con lo que quedó sin indexar la vez anterior; por eso el
    // indexador no guarda ningún cursor.
    match estado
        .db
        .connection()
        .and_then(|conn| estado.indexer.enqueue_missing(&conn))
    {
        Ok(pendientes) => tracing::info!(pendientes, "documentos pendientes de indexar"),
        Err(error) => tracing::warn!(
            code = error.code(),
            "no se pudo averiguar qué queda por indexar"
        ),
    }

    estado.monitor.set(components::SEARCH, salud, motivo);

    // Lo que cambió en las carpetas referenciadas mientras la aplicación estaba
    // cerrada. Al final, porque importa lo que encuentre y compite por la
    // base con lo de arriba; y aquí y no en un hilo aparte porque este ya lo es.
    if let Err(error) = estado.repaso.si_toca(
        &estado.db,
        &estado.bus,
        &estado.paths.library(),
        &estado.import,
    ) {
        tracing::warn!(code = error.code(), "el repaso del arranque falló");
    }

    tracing::info!(
        ms = inicio.elapsed().as_millis(),
        "terminó el trabajo diferido del arranque"
    );
}

/// Lanza la compactación de portadas si no se hizo ya. Ver `arranque_diferido`.
/// Rellena serie y sinopsis de lo ya importado, por tandas y en su hilo.
///
/// Sobre la instalación real son 152.080 paquetes de unos kilobytes: minutos de
/// lectura que no tienen por qué retrasar nada. Se para con la batería baja, como
/// el indexador, y retoma donde lo dejó en el siguiente arranque.
fn completar_series_en_segundo_plano(estado: &AppState) {
    let db = estado.db.clone();
    let lanzado = std::thread::Builder::new()
        .name("nb-series".to_owned())
        .spawn(move || {
            if !crate::power::state().allows_background_work() {
                return;
            }
            if let Ok(mut conn) = db.connection() {
                crate::metadata::limpiar_entidades_guardadas(&mut conn);
            }
            let mut total = 0_u64;
            loop {
                if !crate::power::state().allows_background_work() {
                    break;
                }
                let Ok(conn) = db.connection() else { break };
                let miradas = crate::metadata::completar_serie_y_sinopsis(&conn);
                drop(conn);
                if miradas == 0 {
                    break;
                }
                total += u64::from(miradas);
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            if total > 0 {
                tracing::info!(miradas = total, "pasada de serie y sinopsis");
            }
        });
    if lanzado.is_err() {
        tracing::warn!("no se pudo lanzar la pasada de serie y sinopsis");
    }
}

fn compactar_portadas_en_segundo_plano(estado: &AppState) {
    const MARCA: &str = "covers_compacted_v30";

    let Ok(conn) = estado.db.connection() else {
        return;
    };
    let hecha = conn
        .query_row(
            "SELECT count(*) FROM app_metadata WHERE key = ?1",
            [MARCA],
            |fila| fila.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    drop(conn);
    if hecha {
        return;
    }

    let portadas = estado.paths.cache.join("portadas");
    let db = estado.db.clone();
    let indexador = std::sync::Arc::clone(&estado.indexer);
    let lanzado = std::thread::Builder::new()
        .name("nb-portadas".to_owned())
        .spawn(move || {
            let reloj = std::time::Instant::now();
            // Con la batería baja se para, igual que el indexador: convertir
            // ciento cincuenta mil imágenes no puede gastarle la tarde a nadie.
            // Lo que quede se retoma en el siguiente arranque, porque la marca
            // solo se pone al terminar.
            let seguir = || crate::power::state().allows_background_work();
            let hecho = crate::covers::compactar(&portadas, &seguir);
            tracing::info!(
                retiradas = hecho.retiradas,
                conservadas = hecho.conservadas,
                mb_liberados = hecho.bytes_liberados / (1024 * 1024),
                ms = reloj.elapsed().as_millis(),
                "portadas originales convertidas en miniaturas"
            );
            if seguir() {
                if let Ok(conn) = db.connection() {
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, 'true')",
                        [MARCA],
                    );
                }
            }
            indexador.revisar_presupuesto();
        });
    if lanzado.is_err() {
        tracing::warn!("no se pudo lanzar la compactación de portadas");
    }
}

/// Registra la enciclopedia y, si hay servidor configurado, el traductor.
///
/// Vive aparte del arranque porque también hace falta al **cambiar la
/// preferencia**: poner un servidor de traducción tiene que registrar el
/// proveedor sin reiniciar, y quitarlo tiene que darlo de baja. Con el código
/// dentro de `bootstrap`, lo segundo no podría ocurrir nunca.
pub fn registrar_consulta_en_linea(
    registro: &mut crate::lookup::LookupRegistry,
    db: &Database,
    net: &std::sync::Arc<crate::net::Network>,
) {
    registro.register(Box::new(crate::lookup::online::Encyclopedia::new(
        db.clone(),
        std::sync::Arc::clone(net),
    )));

    let Ok(conn) = db.connection() else {
        return;
    };
    let leer = |clave| match crate::settings::get(&conn, clave) {
        Ok(crate::settings::SettingValue::Text(valor)) => valor,
        _ => String::new(),
    };

    let anfitrion = leer(crate::settings::SettingKey::TranslationHost);
    let destino = {
        let elegido = leer(crate::settings::SettingKey::TranslationTarget);
        // Vacío significa el idioma de la interfaz, que es el que la persona
        // entiende. Traducir al idioma del documento no tendría ningún sentido.
        if elegido.is_empty() {
            leer(crate::settings::SettingKey::Language)
        } else {
            elegido
        }
    };

    match crate::lookup::online::Translation::new(
        db.clone(),
        std::sync::Arc::clone(net),
        &anfitrion,
        &destino,
    ) {
        Some(traductor) => registro.register(Box::new(traductor)),
        None => tracing::info!("sin servidor de traducción configurado: no se ofrece traducir"),
    }
}

/// Enseña el diálogo nativo cuando la biblioteca no se pudo abrir.
///
/// Bloqueante a propósito, y desde el hilo de arranque, no el de la ventana: si
/// no lo fuera, el proceso saldría antes de que nadie llegara a leerlo, que es
/// exactamente el problema que se arregló.
fn avisar_de_que_no_se_pudo_abrir(handle: &tauri::AppHandle, codigo: &str) {
    use tauri_plugin_dialog::{DialogExt as _, MessageDialogKind};

    let mensaje = format!(
        "NB Books no ha podido abrir tu biblioteca.\n\nMotivo: {codigo}\n\n\
         No se ha borrado nada: tus archivos y tu base de datos siguen donde \
         estaban. Si el problema continúa, guarda este código y escribe a soporte."
    );
    handle
        .dialog()
        .message(mensaje)
        .title("No se pudo abrir la biblioteca")
        .kind(MessageDialogKind::Error)
        .blocking_show();
}

/// Si el estado del runtime ya está a disposición de los comandos. Ver
/// [`ipc::runtime_ready`].
static LISTO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// La telemetría tiene que vivir hasta el final del proceso, y nace
/// en un hilo que termina: se guarda aquí y se suelta al salir, que es cuando
/// vuelca lo que le quede.
static TELEMETRIA: std::sync::Mutex<Option<telemetry::TelemetryGuard>> =
    std::sync::Mutex::new(None);

#[must_use]
pub fn runtime_listo() -> bool {
    LISTO.load(std::sync::atomic::Ordering::SeqCst)
}

/// Prepara el runtime con la ventana ya abierta.
///
/// Antes `bootstrap` corría antes de construir la ventana, y una migración sobre
/// la base real dejaba minutos sin nada en pantalla: la aplicación parecía no
/// abrir. Ahora la ventana sale enseguida, la interfaz dice que está poniendo al
/// día la biblioteca, y el estado se entrega a los comandos cuando existe.
fn preparar_con_la_ventana_abierta(handle: &tauri::AppHandle) {
    let (state, guardia) = match bootstrap() {
        Ok(listo) => listo,
        Err(error) => {
            /*
             * **Nunca una pantalla en blanco.** Aquí estaba el defecto que cierra
             * un `expect` mataba el proceso sin mensaje. El diálogo es el
             * del sistema y bloquea este hilo, no el de la ventana. Dice qué pasó,
             * con su código, y que no se ha borrado nada.
             */
            tracing::error!(code = error.code(), "el runtime no pudo prepararse");
            avisar_de_que_no_se_pudo_abrir(handle, error.code());
            handle.exit(1);
            return;
        }
    };
    if let Ok(mut hueco) = TELEMETRIA.lock() {
        *hueco = Some(guardia);
    }

    // Puente del bus interno hacia el webview. El bus no conoce Tauri: es este
    // suscriptor el que traduce, de modo que el runtime sigue siendo probable sin
    // levantar una ventana.
    let para_bus = handle.clone();
    state.bus.subscribe("puente-webview", move |event| {
        if let Err(error) = para_bus.emit(event.name(), event) {
            tracing::warn!(
                event = event.name(),
                error = %error,
                "no se pudo entregar el evento al webview"
            );
        }
    });
    handle.manage(state);
    LISTO.store(true, std::sync::atomic::Ordering::SeqCst);

    // El trabajo que crece con la biblioteca, en este mismo hilo y con la ventana
    // ya construida: es trabajo bloqueante de disco de varios minutos, y meterlo
    // en un hilo de trabajo de tokio se lo quitaría a los comandos, que desde
    // las tareas de fondo corren justo ahí.
    arranque_diferido(&handle.state::<AppState>());
}

/// Arranca la aplicación.
///
/// # Panics
///
/// Si Tauri no puede construir el contexto: sin él no hay ni ventana ni diálogo.
pub fn run() {
    tauri::Builder::default()
        .setup(move |app| {
            escuchar_senales_de_cierre(app.handle().clone());
            ajustar_la_ventana_a_la_pantalla(app);
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("nb-arranque".to_owned())
                .spawn(move || preparar_con_la_ventana_abierta(&handle))?;
            Ok(())
        })
        // El diálogo nativo: es la única vía por la que una ruta del disco entra
        // al proyecto habiéndola elegido el usuario en el sistema, y no el
        // webview. Sin él, importar exige escribir la ruta a mano.
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            ipc::runtime_ready,
            ipc::health_check,
            ipc::update_check,
            ipc::update_managed_by_launcher,
            ipc::edit_edition_field,
            ipc::retired_audio_notice,
            ipc::browse_letter_anchors,
            ipc::delete_document,
            ipc::update_install,
            ipc::extensions_list,
            ipc::extensions_api,
            ipc::extensions_context_commands,
            ipc::extensions_study_views,
            ipc::extensions_themes,
            ipc::extensions_reading_modes,
            ipc::extensions_soundscapes,
            ipc::board_limits,
            ipc::boards_list,
            ipc::boards_create,
            ipc::boards_delete,
            ipc::collections_list,
            ipc::collections_create,
            ipc::collections_rename,
            ipc::collections_delete,
            ipc::collections_add,
            ipc::collections_remove,
            ipc::collections_containing,
            ipc::find_note,
            ipc::board_columns,
            ipc::board_add_column,
            ipc::board_rename_column,
            ipc::board_delete_column,
            ipc::board_move_column,
            ipc::board_add_note,
            ipc::board_add_link,
            ipc::board_create_note,
            ipc::board_move_card,
            ipc::board_remove_card,
            ipc::annotation_counts,
            ipc::study_counts,
            ipc::extensions_citation_formats,
            ipc::review_metadata_in_bulk,
            ipc::save_citations,
            ipc::extensions_render_view,
            ipc::extensions_run_command,
            ipc::extensions_install,
            ipc::extensions_grant,
            ipc::extensions_revoke,
            ipc::extensions_set_enabled,
            ipc::extensions_uninstall,
            ipc::extensions_generations,
            ipc::extensions_withdrawals,
            ipc::extensions_check_withdrawals,
            ipc::extensions_rollback,
            ipc::component_states,
            ipc::settings_snapshot,
            ipc::update_setting,
            ipc::open_document,
            ipc::close_document,
            ipc::save_progress,
            ipc::flush_progress,
            ipc::synchronized_audio,
            ipc::book_entities,
            ipc::reading_pace,
            ipc::entity_places,
            ipc::rescan_roots,
            ipc::render_document,
            ipc::page_size,
            ipc::text_layer,
            ipc::locator_for_selection,
            ipc::create_annotation,
            ipc::edit_annotation,
            ipc::library_annotations,
            ipc::diagnostics_preview,
            ipc::save_diagnostics,
            ipc::recovery_report,
            ipc::voice_status,
            ipc::voice_speak,
            ipc::voice_stop,
            ipc::synthesize_speech,
            ipc::neural_voice,
            ipc::synthesize_neural_speech,
            ipc::estimate_audio,
            ipc::sample_audio,
            ipc::start_audio,
            ipc::start_audio_collection,
            ipc::audio_queue,
            ipc::cancel_audio,
            ipc::audio_job,
            ipc::prepared_audio,
            ipc::forget_audio,
            ipc::allow_speech_host,
            ipc::speech_endpoint,
            ipc::speech_credential_status,
            ipc::set_speech_credential,
            ipc::clear_speech_credential,
            ipc::annotation_history,
            ipc::delete_annotation,
            ipc::annotation_outline,
            ipc::create_bookmark,
            ipc::indexing_progress,
            ipc::set_indexing_paused,
            ipc::import_path,
            ipc::cancel_import,
            ipc::pending_import,
            ipc::last_import,
            ipc::import_rejections,
            ipc::import_duplicates,
            ipc::discard_pending_import,
            ipc::reading_stats,
            ipc::home_rows,
            ipc::list_notes,
            ipc::create_note,
            ipc::update_note,
            ipc::delete_note,
            ipc::note_backlinks,
            ipc::cite_annotation,
            ipc::uncite_annotation,
            ipc::cited_annotations,
            ipc::notes_citing,
            ipc::browse_library,
            ipc::library_facets,
            ipc::library_shelves,
            ipc::search_status,
            ipc::search,
            ipc::reindex,
            ipc::lookup_selection,
            ipc::list_dictionaries,
            ipc::install_dictionary,
            ipc::uninstall_dictionary,
            ipc::list_vocabulary,
            ipc::delete_vocabulary,
            ipc::clear_vocabulary,
            ipc::save_annotations,
            ipc::save_vocabulary,
            ipc::vocabulary_to_cards,
            ipc::save_library_export,
            ipc::clear_reading_history,
            ipc::clear_annotations_and_notes,
            ipc::clear_library,
            ipc::library_roots,
            ipc::relocate_library_root,
            ipc::refresh_availability,
            ipc::network_report,
            ipc::revoke_network_consent,
            ipc::allow_lookup_host,
            ipc::allow_catalog_host,
            ipc::ai_models,
            ipc::install_ai_model,
            ipc::cancel_ai_model_install,
            ipc::uninstall_ai_model,
            ipc::allow_model_host,
            ipc::ai_provider_status,
            ipc::probe_ai_provider,
            ipc::allow_ai_provider,
            ipc::set_ai_credential,
            ipc::clear_ai_credential,
            ipc::ai_sends,
            ipc::clear_ai_sends,
            ipc::report_ai_output,
            ipc::ai_reports,
            ipc::forget_ai_report,
            ipc::clear_ai_reports,
            ipc::save_ai_reports,
            ipc::estimate_vectors,
            ipc::start_vectors,
            ipc::pause_vectors,
            ipc::cancel_vectors,
            ipc::vector_job,
            ipc::reader_activity,
            ipc::vector_exclusion,
            ipc::set_vector_exclusion,
            ipc::semantic_search,
            ipc::study_availability,
            ipc::estimate_study_scope,
            ipc::generate_study,
            ipc::cancel_study,
            ipc::save_study_cards,
            ipc::study_cards,
            ipc::review_study_card,
            ipc::study_card_counts,
            ipc::edit_study_card,
            ipc::delete_study_card,
            ipc::save_concepts,
            ipc::concepts,
            ipc::edit_concept,
            ipc::delete_concept,
            ipc::what_is_sent,
            ipc::search_online_metadata,
            ipc::apply_online_metadata,
            ipc::fetch_online_cover,
            ipc::pending_suggestions,
            ipc::edition_details,
            ipc::allow_metadata_host,
            ipc::accept_suggestion,
            ipc::reject_suggestion,
            ipc::list_catalogs,
            ipc::add_catalog,
            ipc::remove_catalog,
            ipc::browse_catalog,
            ipc::download_from_catalog,
            ipc::legal_notices
        ])
        .build(tauri::generate_context!())
        .expect("el runtime de Tauri debe poder arrancar")
        .run(|handle, evento| {
            if matches!(evento, tauri::RunEvent::ExitRequested { .. }) {
                if let Some(estado) = handle.try_state::<AppState>() {
                    cerrar_bien(&estado);
                }
            }
            if matches!(evento, tauri::RunEvent::Exit) {
                if let Ok(mut hueco) = TELEMETRIA.lock() {
                    hueco.take();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    /// Un archivo `.rs` que ningún `mod` declara no se compila, y por tanto no
    /// se lint ni se prueba: parece código vivo y no lo es.
    ///
    /// No es hipotético. Durante la. se escribió `settings.rs`
    /// entero, `cargo build` pasó en verde y el archivo no se había compilado
    /// ni una vez, porque faltaba su declaración aquí.
    #[test]
    fn ningun_archivo_de_codigo_queda_sin_declarar() {
        let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let declaraciones = recolectar_declaraciones(&raiz);

        for archivo in archivos_rust(&raiz) {
            let nombre = archivo
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("nombre de archivo");

            // `main.rs` es la raíz del binario y `mod.rs` es la del directorio
            // que lo contiene: ninguno se declara a sí mismo.
            if matches!(nombre, "lib" | "main" | "mod") {
                continue;
            }

            // `src/bin/` es la excepción, y no por convenio: Cargo descubre ahí
            // un binario por archivo, así que se compila y `cargo clippy
            // --all-targets` lo lint. Comprobado el 2026-09-07 metiéndole un
            // aviso a `revisar.rs`: clippy falló. La premisa de esta prueba
            // -que sin `mod` el archivo no se compila- no se cumple ahí, y una
            // prueba que exige lo que no hace falta obliga a saltársela.
            if archivo.parent().is_some_and(|p| p.ends_with("bin")) {
                continue;
            }

            assert!(
                declaraciones.contains(nombre),
                "{} no lo declara ningún `mod`: no se compila, no se lint y no se prueba",
                archivo.display()
            );
        }
    }

    fn archivos_rust(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut salida = Vec::new();
        let Ok(entradas) = std::fs::read_dir(dir) else {
            return salida;
        };
        for entrada in entradas.filter_map(Result::ok) {
            let ruta = entrada.path();
            if ruta.is_dir() {
                salida.extend(archivos_rust(&ruta));
            } else if ruta.extension().is_some_and(|e| e == "rs") {
                salida.push(ruta);
            }
        }
        salida
    }

    fn recolectar_declaraciones(dir: &std::path::Path) -> std::collections::BTreeSet<String> {
        let mut nombres = std::collections::BTreeSet::new();
        for archivo in archivos_rust(dir) {
            let Ok(texto) = std::fs::read_to_string(&archivo) else {
                continue;
            };
            for linea in texto.lines() {
                let linea = linea.trim();

                // Un módulo puede venir de otro archivo: `#[path = "x.rs"]` es
                // como se sirve una implementación distinta detrás del mismo
                // nombre, que es lo que hace la voz del sistema para las
                // plataformas sin `speech-dispatcher`. Sin esto, el archivo de
                // la otra plataforma parecería código muerto.
                if let Some(resto) = linea.strip_prefix("#[path = \"") {
                    if let Some(ruta) = resto.split('"').next() {
                        let archivo = std::path::Path::new(ruta);
                        if let Some(nombre) = archivo.file_stem().and_then(|s| s.to_str()) {
                            nombres.insert(nombre.to_owned());
                        }
                    }
                }

                let resto = linea
                    .strip_prefix("pub mod ")
                    .or_else(|| linea.strip_prefix("mod "));
                if let Some(resto) = resto {
                    if let Some(nombre) = resto.split(&[';', ' ', '{'][..]).next() {
                        nombres.insert(nombre.to_owned());
                    }
                }
            }
        }
        nombres
    }
}
