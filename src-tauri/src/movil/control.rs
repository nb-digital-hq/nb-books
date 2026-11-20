//! Leer en el PC: el teléfono maneja el lector de esta ventana, en vivo.
//!
//! Contrato: `docs/architecture/mobile-contract.md`, sección 5, y ADR-016.
//!
//! # El teléfono es un teclado más
//!
//! El servidor no sabe leer. Recibe una orden, comprueba que es una de las que
//! el lector acepta y se la pasa a la interfaz, que la ejecuta por el mismo
//! registro de comandos que el teclado (`src/lib/ui/lector/comandos.ts`). Si el
//! servidor tuviera su propia idea de qué es "pasar página", habría dos, y algún
//! día dirían cosas distintas.
//!
//! En el otro sentido, la interfaz publica un resumen de dónde está el lector y
//! el servidor lo reenvía a quien lo maneja. Nunca al revés: el PC no controla
//! el teléfono.

use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::Extension;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, watch};
use ts_rs::TS;

use super::repositorio::Dispositivo;
use super::Contexto;
use crate::reader::Locator;

/// Las órdenes del registro de comandos del lector que se aceptan a distancia.
///
/// No están las que actúan sobre una selección de texto (resaltar, anotar,
/// consultar): a distancia no hay selección, y ADR-016 deja la anotación a
/// cada dispositivo. Una prueba de la interfaz comprueba que cada una existe en
/// el registro.
pub const COMANDOS: &[&str] = &[
    "pagina-siguiente",
    "pagina-anterior",
    "bloque-siguiente",
    "bloque-anterior",
    "marcador",
    "voz",
    "modo-de-lectura",
    "fondo-sonoro",
    "inmersion",
    "pantalla-completa",
    "acercar",
    "alejar",
    "letra-mayor",
    "letra-menor",
    "capitulo-siguiente",
    "capitulo-anterior",
    "avance-automatico",
    "salir",
];

/// Una orden que el teléfono pide al lector, tal como la recibe la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "tipo", rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "RemoteOrder")]
pub enum Orden {
    /// Un comando del registro, por su identificador.
    Comando { id: String },
    /// Abrir una edición en el lector.
    Abrir { edicion: String },
    /// Ir a una progresión total, de 0 a 1: la barra de posición del teléfono.
    IrA { progresion: f64 },
    /// Ir a un Locator concreto: una entrada del índice, una anotación, la
    /// posición del teléfono.
    IrAlLocator { locator: Locator },
    /// Desplazar en modo continuo, en píxeles lógicos. El teléfono es una
    /// superficie táctil.
    Desplazar { dy: f64 },
    /// Velocidad del avance automático: 0 lo detiene.
    Avance { velocidad: f64 },
}

/// Cota del desplazamiento por orden: un dedo no mueve más de una pantalla de
/// golpe, y una cifra absurda no puede mandar el lector al final del libro.
const DESPLAZAMIENTO_MAXIMO: f64 = 4_000.0;

impl Orden {
    /// Si la orden está dentro de lo que el lector acepta.
    pub fn valida(&self) -> bool {
        match self {
            Self::Comando { id } => COMANDOS.contains(&id.as_str()),
            Self::Abrir { edicion } => !edicion.is_empty() && edicion.len() <= 64,
            Self::IrA { progresion } => progresion.is_finite() && (0.0..=1.0).contains(progresion),
            // La forma ya la comprobó el tipo al leer la orden.
            Self::IrAlLocator { .. } => true,
            Self::Desplazar { dy } => dy.is_finite() && dy.abs() <= DESPLAZAMIENTO_MAXIMO,
            Self::Avance { velocidad } => velocidad.is_finite() && (0.0..=10.0).contains(velocidad),
        }
    }
}

/// A quién avisar cuando cambia el teléfono que maneja el lector.
type AvisoDeControlador = Arc<dyn Fn(Option<String>) + Send + Sync>;

/// Lo que comparte el servidor con la interfaz.
pub struct Puente {
    /// Hacia la interfaz: las órdenes, ya validadas.
    ordenes: broadcast::Sender<Orden>,
    /// Desde la interfaz: el último estado del lector, en JSON.
    estado: watch::Sender<Value>,
    /// Qué teléfono maneja el lector ahora. Con dos, manda el último que habló.
    controlador: Mutex<Option<String>>,
    /// Avisa a la interfaz de un cambio de controlador, para el aviso
    /// "Controlado desde".
    al_cambiar: Mutex<Option<AvisoDeControlador>>,
    /// Cierra las conexiones de un teléfono revocado o cortado.
    expulsiones: broadcast::Sender<Option<String>>,
}

impl Default for Puente {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Puente {
    pub fn nuevo() -> Self {
        Self {
            ordenes: broadcast::channel(64).0,
            estado: watch::channel(Value::Null).0,
            controlador: Mutex::new(None),
            al_cambiar: Mutex::new(None),
            expulsiones: broadcast::channel(8).0,
        }
    }

    /// Lo que la interfaz recibe. Cada suscripción ve todas las órdenes desde
    /// que se suscribe.
    pub fn ordenes(&self) -> broadcast::Receiver<Orden> {
        self.ordenes.subscribe()
    }

    /// La interfaz publica dónde está el lector.
    ///
    /// `send_replace` y no `send`: sin ningún teléfono conectado, `send` no
    /// guarda el valor, y el primero en conectarse recibía un lector vacío
    /// aunque hubiera un libro abierto.
    pub fn publicar(&self, estado: Value) {
        self.estado.send_replace(estado);
    }

    /// Quién maneja el lector ahora.
    pub fn controlador(&self) -> Option<String> {
        self.controlador
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn al_cambiar_controlador(&self, f: impl Fn(Option<String>) + Send + Sync + 'static) {
        *self
            .al_cambiar
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(f));
    }

    fn fijar_controlador(&self, nuevo: Option<String>) {
        let cambio = {
            let mut actual = self
                .controlador
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if *actual == nuevo {
                false
            } else {
                actual.clone_from(&nuevo);
                true
            }
        };
        if cambio {
            let aviso = self
                .al_cambiar
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(f) = aviso {
                f(nuevo);
            }
        }
    }

    /// Entrega una orden de un teléfono. Devuelve si se aceptó.
    pub fn ordenar(&self, telefono: &str, orden: Orden) -> bool {
        if !orden.valida() {
            return false;
        }
        self.fijar_controlador(Some(telefono.to_owned()));
        // Sin la ventana suscrita no hay a quién dárselo; la orden se pierde, que
        // es lo mismo que pulsar una tecla con la ventana cerrada.
        let _ = self.ordenes.send(orden);
        true
    }

    /// Cierra las conexiones de un teléfono.
    pub fn expulsar(&self, telefono: &str) {
        let _ = self.expulsiones.send(Some(telefono.to_owned()));
        if self.controlador().as_deref() == Some(telefono) {
            self.fijar_controlador(None);
        }
    }

    /// Corta el control de todos: lo pide la persona desde el PC.
    pub fn soltar(&self) {
        let _ = self.expulsiones.send(None);
        self.fijar_controlador(None);
    }
}

#[derive(Serialize)]
struct Mensaje<'a> {
    tipo: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    estado: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aceptada: Option<bool>,
}

fn texto(m: &Mensaje<'_>) -> Message {
    Message::Text(serde_json::to_string(m).unwrap_or_default().into())
}

/// `GET /api/v1/control`: abre el WebSocket de Leer en el PC.
pub async fn abrir(
    State(ctx): State<Arc<Contexto>>,
    Extension(telefono): Extension<Dispositivo>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| sesion(ctx, telefono, socket))
}

async fn sesion(ctx: Arc<Contexto>, telefono: Dispositivo, mut socket: WebSocket) {
    let puente = &ctx.puente;
    let mut estado = puente.estado.subscribe();
    let mut expulsiones = puente.expulsiones.subscribe();

    let actual = estado.borrow_and_update().clone();
    if socket
        .send(texto(&Mensaje {
            tipo: "estado",
            estado: Some(&actual),
            aceptada: None,
        }))
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            entrante = socket.recv() => {
                let Some(Ok(mensaje)) = entrante else { break };
                let Message::Text(cuerpo) = mensaje else { continue };
                let aceptada = serde_json::from_str::<Orden>(&cuerpo)
                    .is_ok_and(|orden| puente.ordenar(&telefono.id, orden));
                if aceptada {
                    (ctx.avisar)();
                }
                let respuesta = texto(&Mensaje { tipo: "ack", estado: None, aceptada: Some(aceptada) });
                if socket.send(respuesta).await.is_err() {
                    break;
                }
            }
            cambio = estado.changed() => {
                if cambio.is_err() {
                    break;
                }
                let valor = estado.borrow_and_update().clone();
                let mensaje = texto(&Mensaje { tipo: "estado", estado: Some(&valor), aceptada: None });
                if socket.send(mensaje).await.is_err() {
                    break;
                }
            }
            expulsado = expulsiones.recv() => {
                match expulsado {
                    Ok(None) => break,
                    Ok(Some(id)) if id == telefono.id => break,
                    Ok(Some(_)) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
    if puente.controlador().as_deref() == Some(telefono.id.as_str()) {
        puente.fijar_controlador(None);
        (ctx.avisar)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_orden_fuera_de_lo_que_acepta_el_lector_se_rechaza() {
        let puente = Puente::nuevo();
        let mut recibidas = puente.ordenes();
        assert!(!puente.ordenar(
            "t",
            Orden::Comando {
                id: "resaltar".to_owned()
            }
        ));
        assert!(!puente.ordenar("t", Orden::IrA { progresion: 1.5 }));
        assert!(!puente.ordenar("t", Orden::Desplazar { dy: f64::NAN }));
        assert!(recibidas.try_recv().is_err(), "ninguna llegó a la interfaz");
        assert_eq!(puente.controlador(), None);
    }

    #[test]
    fn manda_el_ultimo_telefono_que_dio_una_orden() {
        let puente = Puente::nuevo();
        let mut recibidas = puente.ordenes();
        assert!(puente.ordenar(
            "a",
            Orden::Comando {
                id: "pagina-siguiente".to_owned()
            }
        ));
        assert!(puente.ordenar("b", Orden::IrA { progresion: 0.5 }));
        assert_eq!(puente.controlador().as_deref(), Some("b"));
        assert_eq!(
            recibidas.try_recv().ok(),
            Some(Orden::Comando {
                id: "pagina-siguiente".to_owned()
            })
        );
    }

    #[test]
    fn revocar_o_cortar_deja_el_lector_sin_controlador() {
        let puente = Puente::nuevo();
        let avisos = Arc::new(Mutex::new(Vec::new()));
        let copia = Arc::clone(&avisos);
        puente.al_cambiar_controlador(move |c| copia.lock().unwrap().push(c));
        puente.ordenar(
            "a",
            Orden::Comando {
                id: "marcador".to_owned(),
            },
        );
        puente.expulsar("a");
        assert_eq!(puente.controlador(), None);
        puente.ordenar(
            "b",
            Orden::Comando {
                id: "marcador".to_owned(),
            },
        );
        puente.soltar();
        assert_eq!(puente.controlador(), None);
        assert_eq!(
            *avisos.lock().unwrap(),
            vec![Some("a".to_owned()), None, Some("b".to_owned()), None]
        );
    }
}
