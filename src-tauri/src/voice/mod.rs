//! Voz del sistema **desde el runtime**, para cuando el webview no la tiene.
//!
//! # Por qué existe esto si ya había un proveedor de voz
//!
//! Porque en Linux el webview no tiene voz. WebKitGTK 2.52 declara la
//! característica `SpeechSynthesisAPI` activada por defecto y aun así
//! `window.speechSynthesis` llega **indefinido**: está en la lista de
//! características y no en el binario. Comprobado sobre el WebKitGTK 4.1 de esta
//! máquina, con el sintetizador del sistema instalado y funcionando.
//!
//! Ningún paquete arregla eso, porque no falta un paquete: falta la API. Así que
//! la voz tiene que salir de donde sí se puede: del runtime.
//!
//! **Y por esto el contrato es un contrato.** Se escribió pensando en
//! que el motor neuronal entraría por ahí; el primero en entrar acabó
//! siendo este. La interfaz no ha cambiado ni una línea de su lógica de
//! narración: solo hay otro proveedor detrás.
//!
//! # Por qué el protocolo y no `spd-say`
//!
//! `speech-dispatcher` es la capa estándar de síntesis en Linux: es lo que usan
//! Orca y el resto del escritorio, respeta lo que el usuario ya haya configurado
//! en su sistema, y habla por el servidor de sonido que tenga.
//!
//! Se le hablaba lanzando `spd-say`, y eso costaba dos cosas que solo se ven
//! usándolo. Una, que **matar el proceso no calla al servidor**: `spd-say -w`
//! espera, y matar a quien espera no cancela lo que ya está encolado, así que
//! parar no paraba. Y dos, que la orden de callar sobre una conexión nueva
//! alcanza a **todos** los clientes del servidor, así que parar aquí callaba
//! también al lector de pantalla de quien lo estuviera usando.
//!
//! Con una conexión propia y sostenida —[`ssip`]— `STOP self` calla lo nuestro y
//! nada más, el fin de cada enunciado llega por un evento en lugar de sondeando
//! un proceso, y el catálogo de voces viene tabulado. No añade dependencias:
//! SSIP es texto sobre un socket de dominio Unix.
//!
//! Lo que se pierde sigue siendo la pausa: pausar corta y reanudar vuelve a
//! empezar el tramo, que es lo mismo que ya hacía la máquina de narración al
//! cambiar la velocidad.
//!
//! Contrato: `docs/architecture/voice-services.md`.

pub mod audio;
pub mod catalogo;
pub mod fonemas;
pub mod kokoro;
pub mod neuronal;
pub mod preparacion;
pub mod tipos;
pub mod wav;

// La voz del sistema tiene dos implementaciones detrás del mismo nombre, y por
// eso nada de lo que hay debajo sabe en qué plataforma corre. En Unix habla con
// `speech-dispatcher` por su socket; donde no lo hay, declara que no está
// disponible. El archivo `sin_ssip.rs` explica por qué se hizo así y no con
// condicionales repartidos.
#[cfg(unix)]
pub mod ssip;
#[cfg(not(unix))]
#[path = "sin_ssip.rs"]
pub mod ssip;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use ts_rs::TS;

use crate::events::{DomainEvent, EventBus};
use ssip::{Evento, Ssip};

/// Cómo se identifica esta aplicación ante el servidor de síntesis.
///
/// El formato es el que espera el protocolo: usuario, aplicación y conexión.
/// Aparece en la configuración de `speech-dispatcher`, así que quien quiera
/// darle a esta aplicación otra voz u otra velocidad por defecto puede hacerlo
/// desde su sistema, como con cualquier otro cliente.
const CLIENTE: &str = "nbbooks:nbbooks:main";

/// Una voz que el sistema ofrece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct SystemVoice {
    /// Identificador con el que se pide. Es el nombre de `speech-dispatcher`.
    pub id: String,
    pub name: String,
    /// Etiqueta de idioma tal y como la declara el sintetizador.
    pub language: String,
}

/// Qué puede hacer la voz del runtime en esta máquina.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct VoiceStatus {
    pub available: bool,
    /// Código del motivo cuando no está disponible. Lo traduce la interfaz.
    pub reason: Option<String>,
    pub voices: Vec<SystemVoice>,
}

/// Lo que se le pide al hilo que habla.
enum Orden {
    Hablar {
        /// En qué generación se pidió. Ver [`Narrator::generacion`].
        generacion: u64,
        bloque: u32,
        texto: String,
        velocidad: f32,
        voz: Option<String>,
    },
    Parar,
}

/// La voz del runtime: un hilo que habla y avisa cuando termina.
///
/// El hilo existe porque hablar tarda lo que dura el párrafo. Hacerlo dentro del
/// comando del puente dejaría la llamada abierta minutos y ocuparía un hilo del
/// runtime asíncrono; con un hilo propio, el comando vuelve enseguida y el fin de
/// cada tramo llega por el bus, que es por donde ya llega todo lo demás.
pub struct Narrator {
    orden: Sender<Orden>,
    /**
     * Cuántas veces se ha mandado parar.
     *
     * Parar no puede ser solo callar al servidor: entre que se manda hablar y
     * que el hilo llega a encolarlo hay un hueco en el que **no hay nada que
     * callar**, y la orden de parar espera detrás de la de hablar en la misma
     * cola. El resultado era que parar justo después de empezar no paraba nada:
     * el hilo encolaba después, se leía el párrafo entero y se anunciaba el
     * tramo como terminado.
     *
     * Con un contador, parar invalida lo que hubiera pedido: el hilo compara la
     * generación con la que venía la orden y, si ya no es la de ahora, ni
     * siquiera lo manda.
     */
    generacion: Arc<AtomicU64>,
    /// La conexión, para poder callar sin esperar al hilo.
    conexion: Arc<Mutex<Option<Arc<Ssip>>>>,
}

impl Narrator {
    /// Arranca el hilo de la voz.
    #[must_use]
    pub fn start(bus: EventBus) -> Self {
        let (emisor, receptor) = mpsc::channel::<Orden>();
        let generacion = Arc::new(AtomicU64::new(0));
        let suya = Arc::clone(&generacion);
        let conexion: Arc<Mutex<Option<Arc<Ssip>>>> = Arc::new(Mutex::new(None));
        let suya_conexion = Arc::clone(&conexion);

        std::thread::Builder::new()
            .name("nb-voz".to_owned())
            .spawn(move || {
                /*
                 * La conexión se abre en el hilo y a la primera orden, no al
                 * arrancar. Arrancar la aplicación no es querer hablar, y en una
                 * máquina sin `speech-dispatcher` conectar falla: hacerlo aquí
                 * deja ese fallo donde se puede contar, en lugar de en el
                 * arranque de todo.
                 */
                let mut eventos: Option<std::sync::mpsc::Receiver<Evento>> = None;

                for orden in receptor {
                    match orden {
                        Orden::Parar => {
                            if let Some(ssip) = suya_conexion
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .as_ref()
                            {
                                let _ = ssip.callar();
                            }
                        }
                        Orden::Hablar {
                            generacion,
                            bloque,
                            texto,
                            velocidad,
                            voz,
                        } => {
                            // Lo que se pidió antes de un `stop` no se dice:
                            // esta orden ya no es de esta generación.
                            if generacion != suya.load(Ordering::SeqCst) {
                                continue;
                            }

                            let ssip = match conectar_si_hace_falta(&suya_conexion, &mut eventos) {
                                Some(ssip) => ssip,
                                None => {
                                    bus.emit(DomainEvent::VoiceFailed {
                                        reason_code: "SIN_SINTETIZADOR".to_owned(),
                                    });
                                    continue;
                                }
                            };

                            // Hablar es sustituir, no encolar: dos párrafos
                            // sonando a la vez no se entienden.
                            let _ = ssip.callar();
                            let _ = ssip.poner_velocidad(tasa(velocidad));
                            if let Some(voz) = voz.as_deref() {
                                let _ = ssip.poner_voz(voz);
                            }

                            let Ok(id) = ssip.hablar(&texto) else {
                                tracing::warn!("la síntesis no aceptó un enunciado");
                                bus.emit(DomainEvent::VoiceFailed {
                                    reason_code: "SIN_SINTETIZADOR".to_owned(),
                                });
                                continue;
                            };

                            if let Some(canal) = eventos.as_ref() {
                                esperar_a_que_termine(canal, id, &bus, bloque, &suya, generacion);
                            }
                        }
                    }
                }
            })
            .ok();

        Self {
            orden: emisor,
            generacion,
            conexion,
        }
    }

    /// Manda decir un tramo. Vuelve enseguida.
    pub fn speak(&self, bloque: u32, texto: String, velocidad: f32, voz: Option<String>) {
        // Un canal roto significa que el hilo murió. No es motivo para fallar:
        // la lectura sigue, sin voz.
        let _ = self.orden.send(Orden::Hablar {
            generacion: self.generacion.load(Ordering::SeqCst),
            bloque,
            texto,
            velocidad,
            voz,
        });
    }

    /// Corta lo que esté diciendo.
    pub fn stop(&self) {
        // Primero se invalida lo pedido, y después se calla. En este orden: al
        // revés, un tramo que estuviera arrancando entre las dos líneas todavía
        // se vería a sí mismo como vigente.
        self.generacion.fetch_add(1, Ordering::SeqCst);

        /*
         * Y se calla **desde aquí**, no solo mandándoselo al hilo.
         *
         * El hilo puede estar esperando el fin de un enunciado, y la orden no le
         * llegaría hasta que terminara: eso es lo contrario de parar. La
         * conexión aguanta que se le hable desde dos hilos porque cada orden
         * toma su cerrojo, así que callar desde aquí es seguro y es inmediato.
         */
        if let Some(ssip) = self
            .conexion
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            let _ = ssip.callar();
        }
        let _ = self.orden.send(Orden::Parar);
    }
}

/// Abre la conexión si aún no la hay, y la deja guardada.
fn conectar_si_hace_falta(
    donde: &Arc<Mutex<Option<Arc<Ssip>>>>,
    eventos: &mut Option<std::sync::mpsc::Receiver<Evento>>,
) -> Option<Arc<Ssip>> {
    if let Some(ssip) = donde.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        return Some(Arc::clone(ssip));
    }
    let (ssip, canal) = Ssip::conectar(CLIENTE).ok()?;
    let ssip = Arc::new(ssip);
    *donde.lock().unwrap_or_else(|p| p.into_inner()) = Some(Arc::clone(&ssip));
    *eventos = Some(canal);
    Some(ssip)
}

/// Cada cuánto se comprueba si alguien mandó parar mientras se esperaba.
///
/// Cincuenta milisegundos. No es un sondeo del proceso —eso es lo que se quitó
/// al dejar `spd-say`— sino un plazo sobre la espera de eventos: sin él, parar
/// no se notaría hasta que el servidor avisara del fin del párrafo, que puede
/// tardar medio minuto.
const REVISION: std::time::Duration = std::time::Duration::from_millis(50);

/// Cuánto se espera un aviso antes de dar al servidor por perdido.
///
/// Cinco minutos, que es más de lo que dura cualquier párrafo dicho despacio. Es
/// un plazo y no una espera indefinida porque si el servidor muriera sin avisar,
/// el hilo se quedaría ahí para siempre y la voz no volvería a hablar.
const PACIENCIA: std::time::Duration = std::time::Duration::from_secs(300);

/// Espera a que el servidor diga qué pasó con un enunciado.
///
/// Se espera **por identificador**: un aviso de un enunciado que ya no es el
/// nuestro es un aviso que venía de camino cuando se canceló, y tomarlo por
/// bueno anunciaría como terminado un tramo que se cortó.
///
/// # Y se deja de esperar en cuanto alguien para
///
/// Esto lo encontró una prueba, y en la aplicación se notaba así: pulsar parar y
/// volver a dar a leer tardaba **hasta un párrafo entero** en arrancar. La causa
/// es que el hilo es uno solo: mientras espera el fin del tramo que ya nadie
/// quiere, no atiende la orden de parar ni la de volver a hablar, que están
/// detrás en la cola.
///
/// Parar invalida la generación, así que comprobarla es saber que lo que se
/// espera ya no le importa a nadie. Se comprueba entre plazos cortos en lugar de
/// esperar sin fin.
fn esperar_a_que_termine(
    eventos: &std::sync::mpsc::Receiver<Evento>,
    id: u64,
    bus: &EventBus,
    bloque: u32,
    generacion_actual: &Arc<AtomicU64>,
    generacion: u64,
) {
    let limite = std::time::Instant::now() + PACIENCIA;
    loop {
        if generacion != generacion_actual.load(Ordering::SeqCst) {
            tracing::debug!(bloque, "se dejó de esperar un tramo que ya se paró");
            return;
        }
        if std::time::Instant::now() > limite {
            tracing::warn!(bloque, "la síntesis dejó de avisar");
            return;
        }

        match eventos.recv_timeout(REVISION) {
            Ok(Evento::Terminado(recibido)) if recibido == id => {
                // Y solo si nadie paró mientras tanto: parar invalida la
                // generación, y un tramo cancelado no continúa.
                if generacion == generacion_actual.load(Ordering::SeqCst) {
                    bus.emit(DomainEvent::VoiceFinished { block: bloque });
                }
                return;
            }
            Ok(Evento::Cortado(recibido)) if recibido == id => {
                tracing::debug!(bloque, "la voz se cortó antes de terminar el tramo");
                return;
            }
            // Avisos de otros enunciados, o de que este empezó: no cambian nada.
            Ok(_) => {}
            // Que no llegue nada en cincuenta milisegundos es lo normal: se
            // vuelve arriba, que es donde se mira si alguien paró.
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                tracing::warn!(bloque, "se perdió el aviso de la síntesis");
                return;
            }
        }
    }
}

/// Traduce el multiplicador de velocidad al rango de `speech-dispatcher`.
///
/// Va de -100 a 100 con 0 en la velocidad que el usuario tenga configurada en su
/// sistema, así que 1 tiene que ser 0: quien ya ajustó su lector de pantalla no
/// quiere que esta aplicación le cambie la velocidad por su cuenta.
fn tasa(velocidad: f32) -> i32 {
    let bruta = ((velocidad - 1.0) * 100.0).round();
    bruta.clamp(-100.0, 100.0) as i32
}

/// Qué puede hacer la voz del runtime aquí y ahora.
///
/// Se pregunta al servidor en lugar de suponerlo: `speech-dispatcher` puede
/// estar instalado y no tener ninguna voz, y son dos situaciones distintas para
/// quien lo lee.
#[must_use]
pub fn status() -> VoiceStatus {
    let Ok((ssip, _eventos)) = Ssip::conectar(CLIENTE) else {
        return VoiceStatus {
            available: false,
            reason: Some("sin-sintetizador".to_owned()),
            voices: Vec::new(),
        };
    };

    let voices = ssip.voces().map(|v| una_por_idioma(&v)).unwrap_or_default();
    if voices.is_empty() {
        return VoiceStatus {
            available: false,
            reason: Some("sin-voces".to_owned()),
            voices,
        };
    }

    VoiceStatus {
        available: true,
        reason: None,
        voices,
    }
}

/// Se queda con las voces **sin variante**, una por idioma.
///
/// El listado completo de espeak son casi quince mil líneas —cada idioma con
/// cien nombres de fantasía— y un desplegable con quince mil entradas no es un
/// desplegable: es una forma de que nadie elija nada. Quien quiera una variante
/// concreta la tiene en la configuración de su sistema, que es donde vive y donde
/// la respeta todo lo demás.
fn una_por_idioma(voces: &[ssip::Voz]) -> Vec<SystemVoice> {
    voces
        .iter()
        .filter(|v| v.variante == "none")
        .map(|v| SystemVoice {
            id: v.nombre.clone(),
            name: v.nombre.clone(),
            language: v.idioma.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_velocidad_del_sistema_es_el_punto_medio() {
        // Quien ya ajustó la velocidad de su lector de pantalla no quiere que
        // esta aplicación se la cambie por su cuenta.
        assert_eq!(tasa(1.0), 0);
        assert_eq!(tasa(1.5), 50);
        assert_eq!(tasa(0.75), -25);
        assert_eq!(tasa(0.5), -50);
    }

    #[test]
    fn una_velocidad_desmedida_se_acota_en_lugar_de_desbordar() {
        assert_eq!(tasa(100.0), 100);
        assert_eq!(tasa(-100.0), -100);
    }

    #[test]
    fn del_listado_se_toma_una_voz_por_idioma() {
        let voces = vec![
            ssip::Voz {
                nombre: "Afrikaans".to_owned(),
                idioma: "af".to_owned(),
                variante: "none".to_owned(),
            },
            ssip::Voz {
                nombre: "Afrikaans+Adam".to_owned(),
                idioma: "af".to_owned(),
                variante: "Adam".to_owned(),
            },
            ssip::Voz {
                nombre: "Spanish (Spain)".to_owned(),
                idioma: "es".to_owned(),
                variante: "none".to_owned(),
            },
        ];
        let salida = una_por_idioma(&voces);
        assert_eq!(salida.len(), 2, "las variantes no entran: {salida:?}");
        assert_eq!(salida[0].language, "af");
        assert_eq!(salida[1].id, "Spanish (Spain)");
    }

    #[test]
    fn un_listado_vacio_no_inventa_voces() {
        assert!(una_por_idioma(&[]).is_empty());
    }
}
