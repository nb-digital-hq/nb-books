//! Registro de componentes y máquina de estados de salud.
//!
//! Es el mecanismo que hace **observable** la degradación elegante. Sin él, la
//! regla 1 de la Constitución sería una intención: la interfaz no tendría forma
//! de saber qué puede ofrecer y qué no.
//!
//! Contrato: `docs/architecture/component-model.md`.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use ts_rs::TS;

use crate::events::{DomainEvent, EventBus};

/// Estado de salud de un componente del runtime.
///
/// La distinción entre [`Self::NotInstalled`] y [`Self::NotSupported`] importa
/// para la interfaz: el primero invita a instalar, el segundo debe ocultar la
/// función sin generar frustración.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum ComponentState {
    /// Requiere binarios o configuración que no están presentes.
    NotInstalled,
    /// El hardware o el sistema no permiten ejecutarlo.
    NotSupported,
    /// Inicializando.
    Starting,
    /// Funcionamiento óptimo.
    Ready,
    /// Funciona con capacidades limitadas.
    Degraded,
    /// Fallo crítico aislado; el resto del sistema continúa.
    Failed,
    /// Detenido deliberadamente.
    Stopped,
}

impl ComponentState {
    /// Indica si el componente puede atender peticiones.
    #[must_use]
    pub const fn is_usable(self) -> bool {
        matches!(self, Self::Ready | Self::Degraded)
    }
}

/// Estado publicado de un componente.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ComponentStatus {
    pub component: &'static str,
    pub state: ComponentState,
    /// Código canónico del motivo, nunca un mensaje libre: la interfaz lo
    /// traduce. Ver `docs/architecture/i18n.md`.
    pub reason_code: Option<&'static str>,
}

/// Registro central de la salud de los componentes.
#[derive(Clone)]
pub struct Monitor {
    states: Arc<RwLock<BTreeMap<&'static str, ComponentStatus>>>,
    bus: EventBus,
}

impl Monitor {
    /// Crea el monitor asociado a un bus de eventos.
    #[must_use]
    pub fn new(bus: EventBus) -> Self {
        Self {
            states: Arc::new(RwLock::new(BTreeMap::new())),
            bus,
        }
    }

    /// Declara o actualiza el estado de un componente.
    ///
    /// Solo emite evento si el estado cambia realmente: repetir el mismo estado
    /// no debe generar ruido en el bus ni en la interfaz.
    pub fn set(
        &self,
        component: &'static str,
        state: ComponentState,
        reason_code: Option<&'static str>,
    ) {
        let cambio = {
            let mut states = match self.states.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            let previo = states.get(component).map(|s| (s.state, s.reason_code));
            let nuevo = (state, reason_code);

            if previo == Some(nuevo) {
                false
            } else {
                states.insert(
                    component,
                    ComponentStatus {
                        component,
                        state,
                        reason_code,
                    },
                );
                true
            }
        };

        if cambio {
            tracing::info!(
                component,
                state = ?state,
                reason = reason_code.unwrap_or("-"),
                "cambio de estado de componente"
            );
            self.bus.emit(DomainEvent::ComponentStateChanged {
                component,
                state,
                reason_code,
            });
        }
    }

    /// Estado actual de un componente, si está registrado.
    #[must_use]
    pub fn get(&self, component: &str) -> Option<ComponentState> {
        match self.states.read() {
            Ok(guard) => guard.get(component).map(|s| s.state),
            Err(poisoned) => poisoned.into_inner().get(component).map(|s| s.state),
        }
    }

    /// Instantánea de todos los componentes registrados, en orden estable.
    #[must_use]
    pub fn snapshot(&self) -> Vec<ComponentStatus> {
        match self.states.read() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(poisoned) => poisoned.into_inner().values().cloned().collect(),
        }
    }
}

/// Nombres canónicos de los componentes del runtime.
///
/// Son parte del contrato con la interfaz: renombrar uno rompe la lógica que
/// decide qué funciones mostrar.
pub mod components {
    pub const DATABASE: &str = "database";
    pub const READER: &str = "reader";
    pub const SEARCH: &str = "search";
    pub const METADATA: &str = "metadata";
    pub const AI: &str = "ai";
    /// El subsistema de extensiones: su almacén y su cajón de arena.
    ///
    /// Que **una** extensión se pase de tiempo o de memoria no cambia este
    /// estado: contenerla es el sistema funcionando. Lo que lo cambia es que el
    /// subsistema entero no esté, y entonces la interfaz oculta la sección en
    /// lugar de ofrecer botones que no hacen nada.
    pub const EXTENSIONS: &str = "extensions";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_componente_sin_declarar_no_tiene_estado() {
        let monitor = Monitor::new(EventBus::new());
        assert_eq!(monitor.get(components::AI), None);
    }

    #[test]
    fn declarar_un_estado_lo_hace_consultable() {
        let monitor = Monitor::new(EventBus::new());
        monitor.set(components::DATABASE, ComponentState::Ready, None);

        assert_eq!(
            monitor.get(components::DATABASE),
            Some(ComponentState::Ready)
        );
        assert_eq!(monitor.snapshot().len(), 1);
    }

    #[test]
    fn repetir_el_mismo_estado_no_genera_ruido() {
        let bus = EventBus::new();
        let monitor = Monitor::new(bus.clone());
        let recibidos = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let contador = Arc::clone(&recibidos);
        bus.subscribe("contador", move |_| {
            contador.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        monitor.set(components::DATABASE, ComponentState::Ready, None);
        monitor.set(components::DATABASE, ComponentState::Ready, None);
        monitor.set(
            components::DATABASE,
            ComponentState::Degraded,
            Some("DISK_SLOW"),
        );

        for _ in 0..200 {
            if recibidos.load(std::sync::atomic::Ordering::SeqCst) >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(
            recibidos.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "solo los cambios reales de estado se publican"
        );
    }

    /// La interfaz usa esta distinción para decidir qué muestra.
    #[test]
    fn solo_los_estados_operativos_se_consideran_utilizables() {
        assert!(ComponentState::Ready.is_usable());
        assert!(ComponentState::Degraded.is_usable());

        for estado in [
            ComponentState::NotInstalled,
            ComponentState::NotSupported,
            ComponentState::Starting,
            ComponentState::Failed,
            ComponentState::Stopped,
        ] {
            assert!(
                !estado.is_usable(),
                "{estado:?} no debe considerarse utilizable"
            );
        }
    }

    /// Los nombres de estado cruzan el puente IPC: cambiarlos rompe la interfaz
    /// sin que ningún compilador lo detecte.
    #[test]
    fn los_estados_se_serializan_con_su_nombre_canonico() {
        let json = serde_json::to_string(&ComponentState::NotInstalled).expect("serializable");
        assert_eq!(json, r#""NOT_INSTALLED""#);
    }
}
