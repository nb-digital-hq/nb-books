//! Bus de eventos interno.
//!
//! Los componentes del runtime no se llaman entre sí: publican hechos y
//! reaccionan a los de otros. Esto es lo que permite añadir un componente sin
//! tocar los existentes, y lo que impide que el fallo de uno arrastre al resto.
//!
//! Reglas que este módulo hace cumplir (ver
//! `docs/architecture/event-bus.md`):
//!
//! - **Disparar y olvidar.** [`EventBus::emit`] no bloquea al emisor: encola y
//!   vuelve. La entrega ocurre en un hilo despachador propio.
//! - **Aislamiento del consumidor.** Un suscriptor que entra en pánico se
//!   registra y se desactiva; ni el emisor ni los demás suscriptores se ven
//!   afectados.
//! - **Orden no garantizado** entre eventos distintos.
//! - **Cargas útiles ligeras:** identificadores y referencias, nunca contenido.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};

use serde::Serialize;
use ts_rs::TS;

use crate::monitor::ComponentState;

/// Hecho ocurrido en el runtime.
///
/// El catálogo completo, con emisores y consumidores previstos, está en
/// `docs/architecture/api/events.md`. Las variantes solo pueden crecer con campos
/// opcionales: renombrar una es un cambio incompatible.
//
// El discriminante se serializa con el nombre de la variante en `PascalCase`,
// que es exactamente la grafía del catálogo de `docs/architecture/api/events.md` y la que
// devuelve [`DomainEvent::name`]. Una sola grafía en el código, en la
// documentación y en el puente hacia el webview.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum DomainEvent {
    /// Un componente del runtime cambió de estado de salud.
    ///
    /// Es el evento que hace observable la degradación elegante.
    #[serde(rename_all = "camelCase")]
    ComponentStateChanged {
        component: &'static str,
        state: ComponentState,
        /// Código canónico del motivo, nunca un mensaje libre.
        reason_code: Option<&'static str>,
    },

    /// Entró un documento nuevo en la biblioteca.
    ///
    /// Lo consumen el servicio de metadatos, que extrae los internos y genera la
    /// portada, y el de búsqueda, que indexa el texto. Ambos en segundo plano:
    /// la importación no espera a ninguno.
    ///
    /// La carga útil lleva identificadores y el tipo de medio, nunca el título
    /// ni la ruta: un evento que viaja al webview y acaba en un registro no
    /// puede llevar el contenido de la biblioteca de nadie.
    #[serde(rename_all = "camelCase")]
    DocumentImported {
        edition_id: String,
        file_id: String,
        media_type: String,
    },

    /// Un documento salió de la biblioteca.
    ///
    /// Lo consumen el índice de búsqueda, para retirar sus fragmentos, y la
    /// interfaz, para quitarlo de la cuadrícula sin recargarla entera.
    #[serde(rename_all = "camelCase")]
    DocumentDeleted { edition_id: String },

    /// Avance de una importación masiva.
    ///
    /// Lleva números, no nombres. Importar una carpeta de miles de documentos
    /// emitiría miles de eventos con el título de cada uno, y esos eventos
    /// acaban en el webview y, con él, en cualquier registro que alguien active.
    #[serde(rename_all = "camelCase")]
    BulkImportProgress {
        processed: u32,
        imported: u32,
        skipped: u32,
        rejected: u32,
        /// Copias exactas de algo que ya estaba, que no entran. Aparte de
        /// `skipped`, que es lo mismo visto otra vez.
        duplicates: u32,
        /// Cuántos archivos hay en total.
        ///
        /// Se recorre la carpeta entera **antes** de importar nada, así que este
        /// número se conoce desde el primer aviso y no crece por el camino. Sin
        /// él, la interfaz solo puede pintar un contador que sube: no puede
        /// decir cuánto falta, y quien importa una biblioteca de ciento
        /// cincuenta mil documentos necesita saberlo.
        total: u32,
    },

    /// La voz del runtime terminó de decir un bloque.
    ///
    /// Va por el bus y no como respuesta del comando porque hablar tarda lo que
    /// dura el párrafo: dejar la llamada abierta ocuparía un hilo del runtime
    /// asíncrono durante minutos.
    VoiceFinished { block: u32 },
    /// La voz del runtime no pudo hablar.
    VoiceFailed { reason_code: String },
    /// Una importación masiva terminó, y cómo.
    ///
    /// Existe porque el avance no dice cuándo parar de esperar: sin este evento,
    /// la interfaz solo sabría que la importación acabó al dejar de recibir
    /// avances, que es indistinguible de una que va lenta. `cancelled` distingue
    /// además terminar de que la parasen, que son dos finales distintos.
    #[serde(rename_all = "camelCase")]
    BulkImportFinished {
        processed: u32,
        imported: u32,
        skipped: u32,
        rejected: u32,
        duplicates: u32,
        cancelled: bool,
    },

    /// Se escribió en la base dónde va la lectura de una edición.
    ///
    /// Lo emite el registrador de progreso **cada vez que escribe**, no cada vez
    /// que se le apunta una posición: lo que se anuncia es lo que ya está en la
    /// base, así que quien lo reciba y vuelva a preguntar lee lo mismo que dice
    /// el evento. Anunciar lo apuntado y no escrito sería publicar un dato que
    /// ninguna consulta puede devolver todavía.
    ///
    /// Lleva la progresión porque es lo único que cambia en las pantallas que la
    /// pintan -la barra de la biblioteca, lo empezado en Inicio-, y con ella no
    /// hace falta una consulta para mover una barra.
    #[serde(rename_all = "camelCase")]
    ProgressSaved {
        edition_id: String,
        /// De 0 a 1, o nulo si el Locator no la declara.
        progression: Option<f64>,
    },

    /// Cambió lo que se sabe de una edición: título, autoría, portada.
    ///
    /// Lo emiten quienes escriben metadatos -el servicio que los extrae al
    /// importar, aplicar un candidato de un catálogo, aceptar una propuesta,
    /// traer una portada-, después de confirmar. Sin campos: quien pinta la
    /// edición la vuelve a pedir entera, y una lista de campos tocados solo
    /// serviría para que alguien decidiera no refrescar lo que sí cambió.
    #[serde(rename_all = "camelCase")]
    MetadataUpdated { edition_id: String },

    /// Cambió una colección: se creó, se renombró, se borró o cambió lo que
    /// tiene dentro.
    ///
    /// Nulo cuando lo que cambió es la lista entera, que es lo que pasa al
    /// crear o borrar: no hay una colección de antes a la que referirse.
    #[serde(rename_all = "camelCase")]
    CollectionsChanged { collection_id: Option<String> },

    /// Cambió lo que hay instalado: una extensión entró, salió, se apagó, se
    /// encendió, se retiró o se volvió a una versión anterior.
    ///
    /// Sin identificador a propósito: lo que aporta una extensión -temas,
    /// modos, órdenes, vistas, formatos de cita- se pide en listas enteras, y
    /// ninguna pantalla puede refrescar "solo lo de esta".
    ExtensionsChanged,

    /// Un archivo de una raíz referenciada dejó de estar, o volvió.
    ///
    /// Lo emite el repaso de las carpetas al ver que la base y el disco ya no dicen
    /// lo mismo. La edición no sale de la biblioteca: queda como no disponible,
    /// con sus anotaciones, y quien la pinta tiene que decirlo.
    #[serde(rename_all = "camelCase")]
    AvailabilityChanged { edition_id: String },

    /// Avance de la indexación de texto completo.
    ///
    /// Números, como el anterior y por el mismo motivo. `paused` viaja aquí
    /// porque la interfaz tiene que poder decir **por qué** no avanza: una barra
    /// de progreso detenida sin explicación se interpreta como que la aplicación
    /// se ha colgado, y quedarse callado mientras se espera a que alguien
    /// enchufe el portátil es la peor forma de esperar.
    #[serde(rename_all = "camelCase")]
    IndexingProgress {
        pending: u32,
        indexed: u32,
        skipped: u32,
        failed: u32,
        paused: bool,
        /// Código canónico de por qué está detenida, nunca un mensaje libre.
        pause_reason: Option<&'static str>,
    },

    /// Avance de la instalación de un modelo de IA.
    ///
    /// Bytes y no porcentaje: el porcentaje se calcula en la interfaz, y con los
    /// bytes puede decir además cuánto falta, que es lo que se quiere saber de
    /// doscientos megabytes.
    #[serde(rename_all = "camelCase")]
    AiModelProgress {
        model_id: String,
        // Doscientos megabytes caben de sobra en el entero exacto de JavaScript;
        // sin esto `ts-rs` genera `bigint`, y el webview recibe un número.
        #[ts(type = "number")]
        done: u64,
        #[ts(type = "number")]
        total: u64,
    },

    /// Una instalación de modelo terminó, y cómo.
    ///
    /// `outcome` es uno de `installed`, `cancelled`, `needsConsent` o `failed`.
    /// `needsConsent` trae el anfitrión, que puede ser la red de distribución a la
    /// que redirigió el del catálogo, y `failed` su código canónico: la vista pone
    /// el texto en contexto, porque `DOCUMENT_CORRUPTED` aquí significa "lo
    /// descargado no coincide con lo revisado".
    #[serde(rename_all = "camelCase")]
    AiModelFinished {
        model_id: String,
        outcome: &'static str,
        host: Option<String>,
        reason_code: Option<&'static str>,
    },

    /// Avance de la generación de vectores de un documento.
    ///
    /// Se emite tras cada lote y cada vez que el trabajo empieza o deja de
    /// esperar. `pauseReason` dice por qué espera, si espera: `USER_PAUSED`,
    /// `READER_ACTIVE`, `AI_SUSPENDED_BATTERY` o `AI_SUSPENDED_HEAT`.
    #[serde(rename_all = "camelCase")]
    VectorJobProgress {
        edition_id: String,
        #[ts(type = "number")]
        done: u64,
        #[ts(type = "number")]
        total: u64,
        /// Documentos que esperan después de este.
        queued: u32,
        pause_reason: Option<&'static str>,
    },

    /// La generación de vectores terminó con la cola vacía, y cómo.
    ///
    /// `outcome` es uno de `done`, `cancelled`, `budget` —se detuvo al llegar al
    /// tope de lo derivado— o `failed`, con el código del primer fallo.
    #[serde(rename_all = "camelCase")]
    VectorJobFinished {
        outcome: &'static str,
        reason_code: Option<&'static str>,
    },

    /// Se recuperaron portadas que la caché ya no tenía.
    ///
    /// Lo emite el runtime después de volver a sacarlas del documento, y sirve
    /// para una sola cosa: que la pantalla que las pidió vuelva a pedir su página
    /// y las pinte. Sin esto habría que salir de la biblioteca y volver.
    #[serde(rename_all = "camelCase")]
    CoversRecovered { count: u32 },

    /// Cómo va la preparación del audio de un capítulo o de un libro.
    ///
    /// `remaining_ms` es **lo que falta según el ritmo medido en esta pasada**, no
    /// una estimación hecha al empezar: quien espera cuarenta minutos merece que
    /// el número se corrija con lo que está pasando y no con lo que se supuso.
    /// Llega nulo hasta que hay bastante medido para decir algo.
    #[serde(rename_all = "camelCase")]
    AudioJobProgress {
        edition_id: String,
        /// Fragmentos dichos y fragmentos que hay.
        done: u32,
        total: u32,
        #[ts(type = "number | null")]
        remaining_ms: Option<u64>,
        /// Segundos de audio producidos hasta ahora.
        #[ts(type = "number")]
        seconds: u64,
        /// Libros que esperan detrás de este en la cola.
        queued: u32,
    },

    /// La preparación terminó, y cómo.
    ///
    /// `outcome` es uno de `done`, `cancelled` —lo hecho se queda y sirve—,
    /// `budget` o `failed`.
    #[serde(rename_all = "camelCase")]
    AudioJobFinished {
        edition_id: String,
        outcome: &'static str,
        reason_code: Option<&'static str>,
    },

    /// Cambió algo de lo que se ve en Ajustes, Teléfonos: un teléfono se
    /// emparejó, se revocó, o el servidor se encendió o se apagó.
    MobileDevicesChanged,

    /// Cambió el teléfono que maneja el lector con Leer en el PC. Nulo cuando
    /// ya no lo maneja ninguno.
    #[serde(rename_all = "camelCase")]
    RemoteControllerChanged {
        device_id: Option<String>,
        /// Su nombre, para el aviso: viaja con el anuncio para que la interfaz
        /// no tenga que pedir la lista de teléfonos para pintar una línea.
        device_name: Option<String>,
    },

    /// Una orden de Leer en el PC, ya validada por el servidor. La interfaz la
    /// ejecuta por el mismo registro de comandos que el teclado.
    ///
    /// `fileId` solo viaja con la orden de abrir: es la copia disponible de la
    /// edición pedida, resuelta aquí porque el teléfono habla de ediciones y el
    /// lector abre archivos. Nulo si la edición no tiene ninguna a mano.
    #[serde(rename_all = "camelCase")]
    RemoteOrder {
        order: crate::movil::control::Orden,
        file_id: Option<String>,
    },
}

impl DomainEvent {
    /// Nombre estable del evento. Coincide con el discriminante serializado y
    /// con el catálogo de `docs/architecture/api/events.md`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ComponentStateChanged { .. } => "ComponentStateChanged",
            Self::DocumentImported { .. } => "DocumentImported",
            Self::DocumentDeleted { .. } => "DocumentDeleted",
            Self::BulkImportProgress { .. } => "BulkImportProgress",
            Self::VoiceFinished { .. } => "VoiceFinished",
            Self::VoiceFailed { .. } => "VoiceFailed",
            Self::BulkImportFinished { .. } => "BulkImportFinished",
            Self::IndexingProgress { .. } => "IndexingProgress",
            Self::AiModelProgress { .. } => "AiModelProgress",
            Self::AiModelFinished { .. } => "AiModelFinished",
            Self::VectorJobProgress { .. } => "VectorJobProgress",
            Self::VectorJobFinished { .. } => "VectorJobFinished",
            Self::CoversRecovered { .. } => "CoversRecovered",
            Self::AudioJobProgress { .. } => "AudioJobProgress",
            Self::AudioJobFinished { .. } => "AudioJobFinished",
            Self::ProgressSaved { .. } => "ProgressSaved",
            Self::MetadataUpdated { .. } => "MetadataUpdated",
            Self::CollectionsChanged { .. } => "CollectionsChanged",
            Self::ExtensionsChanged => "ExtensionsChanged",
            Self::AvailabilityChanged { .. } => "AvailabilityChanged",
            Self::MobileDevicesChanged => "MobileDevicesChanged",
            Self::RemoteControllerChanged { .. } => "RemoteControllerChanged",
            Self::RemoteOrder { .. } => "RemoteOrder",
        }
    }
}

type Handler = Box<dyn Fn(&DomainEvent) + Send + Sync>;

struct Subscriber {
    name: &'static str,
    handler: Handler,
    /// Un suscriptor que entra en pánico se desactiva en lugar de repetir el
    /// fallo en cada evento.
    alive: bool,
}

/// Bus de publicación y suscripción del runtime.
#[derive(Clone)]
pub struct EventBus {
    sender: Sender<DomainEvent>,
    subscribers: Arc<RwLock<Vec<Subscriber>>>,
}

impl EventBus {
    /// Crea el bus y levanta su hilo despachador.
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<DomainEvent>();
        let subscribers: Arc<RwLock<Vec<Subscriber>>> = Arc::new(RwLock::new(Vec::new()));

        let dispatch_subscribers = Arc::clone(&subscribers);
        std::thread::Builder::new()
            .name("nb-event-bus".into())
            .spawn(move || {
                for event in receiver {
                    dispatch(&dispatch_subscribers, &event);
                }
                tracing::debug!("el bus de eventos se detuvo");
            })
            .expect("el hilo del bus de eventos debe poder crearse");

        Self {
            sender,
            subscribers,
        }
    }

    /// Registra un consumidor.
    ///
    /// El consumidor debe ser **idempotente**: puede recibir el mismo evento
    /// más de una vez sin corromper su estado.
    pub fn subscribe<F>(&self, name: &'static str, handler: F)
    where
        F: Fn(&DomainEvent) + Send + Sync + 'static,
    {
        let mut subscribers = match self.subscribers.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        subscribers.push(Subscriber {
            name,
            handler: Box::new(handler),
            alive: true,
        });
        tracing::debug!(subscriber = name, "suscriptor registrado");
    }

    /// Publica un evento sin bloquear al emisor.
    ///
    /// Si el hilo despachador ya no existe, el evento se descarta con un aviso:
    /// no poder notificar nunca puede impedir que la operación que lo originó
    /// termine correctamente.
    pub fn emit(&self, event: DomainEvent) {
        if self.sender.send(event).is_err() {
            tracing::warn!("no hay despachador de eventos; el evento se descarta");
        }
    }

    /// Número de suscriptores vivos. Solo para diagnóstico y pruebas.
    #[must_use]
    pub fn live_subscribers(&self) -> usize {
        match self.subscribers.read() {
            Ok(guard) => guard.iter().filter(|s| s.alive).count(),
            Err(poisoned) => poisoned.into_inner().iter().filter(|s| s.alive).count(),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

fn dispatch(subscribers: &Arc<RwLock<Vec<Subscriber>>>, event: &DomainEvent) {
    let mut guard = match subscribers.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    for subscriber in guard.iter_mut().filter(|s| s.alive) {
        let result = catch_unwind(AssertUnwindSafe(|| (subscriber.handler)(event)));

        if result.is_err() {
            // El fallo de un consumidor es local: se aísla y el resto continúa.
            tracing::error!(
                subscriber = subscriber.name,
                event = event.name(),
                "un suscriptor entró en pánico; queda desactivado"
            );
            subscriber.alive = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn evento(component: &'static str) -> DomainEvent {
        DomainEvent::ComponentStateChanged {
            component,
            state: ComponentState::Ready,
            reason_code: None,
        }
    }

    /// Espera activa acotada: el despacho es asíncrono y las pruebas no pueden
    /// depender de un `sleep` fijo.
    /// Espera holgada: con `RUST_BACKTRACE=1`, como en la CI, el pánico
    /// deliberado tarda en escribir su traza, y un segundo no siempre bastaba.
    fn esperar_hasta(condicion: impl Fn() -> bool) -> bool {
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < limite {
            if condicion() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn un_suscriptor_recibe_los_eventos_publicados() {
        let bus = EventBus::new();
        let recibidos = Arc::new(AtomicUsize::new(0));

        let contador = Arc::clone(&recibidos);
        bus.subscribe("contador", move |_| {
            contador.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit(evento("db"));
        bus.emit(evento("reader"));

        assert!(esperar_hasta(|| recibidos.load(Ordering::SeqCst) == 2));
    }

    /// Regla 1 de la Constitución: un fallo local produce consecuencias locales.
    /// Si esta prueba falla, el pánico de una extensión o de un servicio podría
    /// tumbar la aplicación entera.
    #[test]
    fn el_panico_de_un_suscriptor_no_afecta_a_los_demas() {
        let bus = EventBus::new();
        let sano = Arc::new(AtomicUsize::new(0));

        bus.subscribe("roto", |_| panic!("fallo deliberado en un consumidor"));

        let contador = Arc::clone(&sano);
        bus.subscribe("sano", move |_| {
            contador.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit(evento("db"));
        bus.emit(evento("db"));

        assert!(
            esperar_hasta(|| sano.load(Ordering::SeqCst) == 2),
            "el suscriptor sano debe seguir recibiendo eventos"
        );
        assert_eq!(
            bus.live_subscribers(),
            1,
            "el suscriptor roto debe quedar desactivado"
        );
    }

    #[test]
    fn emitir_sin_suscriptores_no_falla() {
        let bus = EventBus::new();
        bus.emit(evento("db"));
        assert_eq!(bus.live_subscribers(), 0);
    }

    /// El nombre del evento forma parte del contrato con el webview.
    #[test]
    fn los_nombres_de_evento_son_estables() {
        assert_eq!(evento("db").name(), "ComponentStateChanged");
    }

    #[test]
    fn el_evento_se_serializa_con_su_discriminante() {
        let json = serde_json::to_string(&evento("db")).expect("serializable");
        assert!(
            json.contains(r#""type":"ComponentStateChanged""#),
            "json: {json}"
        );
        assert!(json.contains(r#""component":"db""#), "json: {json}");
    }
}
