//! Cliente de `speech-dispatcher` por su propio protocolo, SSIP.
//!
//! # Por qué se habla el protocolo en lugar de lanzar `spd-say`
//!
//! Porque `spd-say` **abre una conexión nueva en cada llamada**, y sobre una
//! conexión nueva las órdenes de callar alcanzan a todos los clientes del
//! servidor. Parar la lectura en alto de esta aplicación callaba también al
//! lector de pantalla de quien la estuviera usando, que en una aplicación que
//! presume de accesibilidad no es un detalle: es lo contrario de lo que se
//! promete. Era la pregunta I9.
//!
//! Con una conexión propia y sostenida, `STOP self` calla **lo nuestro y nada
//! más**. Y salen tres cosas de regalo, todas medidas contra el servidor real:
//!
//! 1. **Se sabe cuándo termina cada enunciado** por un evento (`702 END`), en
//!    lugar de sondear un proceso cada cincuenta milisegundos para ver si sigue
//!    vivo. Menos latencia y ningún hilo esperando.
//! 2. **Ya no se lanza un proceso por párrafo.** Antes eran dos: el que hablaba
//!    y el que lo mandaba callar.
//! 3. **El catálogo de voces llega tabulado** (`LIST SYNTHESIS_VOICES`), en vez
//!    de en columnas alineadas por espacios que había que adivinar dónde cortar.
//!
//! # El protocolo, en lo que aquí se usa
//!
//! Es texto sobre un socket de dominio Unix. Se manda una línea terminada en
//! `\r\n` y el servidor contesta con un código de tres cifras: `2xx` bien, `3xx`
//! "sigue mandando", `4xx` y `5xx` mal. Una respuesta de varias líneas lleva
//! `NNN-` en todas menos en la última, que lleva `NNN ` con un espacio; ese
//! espacio es la única marca de que la respuesta terminó.
//!
//! Además, y por el mismo socket, llegan **eventos** `7xx` sin que nadie los
//! pida: `701` empieza, `702` termina, `703` cancelado. Por eso hay un hilo
//! lector que separa las dos cosas: una respuesta que llegara mezclada con un
//! evento haría que cada orden leyera la contestación de la anterior.
//!
//! Contrato: `docs/architecture/voice-services.md`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use crate::error::{NbError, NbResult};

/// Cuánto se espera la contestación a una orden.
///
/// Dos segundos. El servidor contesta en milisegundos; si tarda más es que no
/// va a contestar, y quedarse esperando dejaría la voz colgada sin decirlo.
const ESPERA: Duration = Duration::from_secs(2);

// Los dos tipos de la interfaz viven en `tipos` porque los comparte la
// implementación de las plataformas sin `speech-dispatcher`, que no puede
// depender de este archivo: no compila fuera de Unix.
pub use super::tipos::{Evento, Voz};

/// Conexión con `speech-dispatcher`.
pub struct Ssip {
    /**
     * La conversación con el servidor, entera y bajo un solo cerrojo.
     *
     * **Un solo cerrojo y no dos**, y esto lo encontró una prueba. Con la salida
     * y las respuestas protegidas por separado, mandar una orden y leer su
     * contestación dejaban de ser una operación: entre lo uno y lo otro, otro
     * hilo podía escribir la suya y llevarse la contestación ajena. Ocurre de
     * verdad porque parar se manda desde el hilo de quien pulsa mientras el de
     * la voz está encolando el tramo siguiente.
     */
    conversacion: Mutex<Conversacion>,
}

/// Los dos extremos de la conversación: por donde se manda y por donde llega.
struct Conversacion {
    salida: UnixStream,
    respuestas: Receiver<Respuesta>,
}

/// Una contestación del servidor: su código y sus líneas de datos.
#[derive(Debug, Clone)]
struct Respuesta {
    codigo: u16,
    lineas: Vec<String>,
}

impl Ssip {
    /// Se conecta y deja listo el hilo que lee.
    ///
    /// Devuelve también el canal por el que llegan los eventos. Se devuelve en
    /// lugar de guardarse porque quien los consume es la máquina de la voz, y
    /// tenerlos aquí obligaría a esta a saber qué hacer con ellos.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si no hay servidor al que conectarse, que
    /// es lo que pasa cuando `speech-dispatcher` no está instalado o no ha
    /// arrancado en esta sesión.
    pub fn conectar(nombre_del_cliente: &str) -> NbResult<(Self, Receiver<Evento>)> {
        let flujo = UnixStream::connect(ruta_del_socket()?).map_err(|error| {
            tracing::info!(error = %error, "no hay servidor de síntesis al que conectarse");
            NbError::ComponentUnavailable
        })?;
        // Sin plazo de lectura: el hilo lector se queda esperando eventos, que
        // es justo lo que tiene que hacer. Los plazos los pone quien espera una
        // respuesta, sobre el canal.
        let copia = flujo
            .try_clone()
            .map_err(|_| NbError::ComponentUnavailable)?;

        let (emisor_r, receptor_r) = mpsc::channel::<Respuesta>();
        let (emisor_e, receptor_e) = mpsc::channel::<Evento>();

        std::thread::Builder::new()
            .name("nb-voz-ssip".to_owned())
            .spawn(move || leer_sin_parar(copia, &emisor_r, &emisor_e))
            .map_err(|_| NbError::ComponentUnavailable)?;

        let cliente = Self {
            conversacion: Mutex::new(Conversacion {
                salida: flujo,
                respuestas: receptor_r,
            }),
        };

        // El nombre identifica la conexión ante el servidor, y es lo que hace
        // que `self` signifique nosotros y no todo el mundo.
        cliente.orden(&format!("SET self CLIENT_NAME {nombre_del_cliente}"))?;
        // Sin esto no llegaría ningún `7xx` y habría que volver a sondear.
        cliente.orden("SET self NOTIFICATION ALL ON")?;

        Ok((cliente, receptor_e))
    }

    /// Manda una orden y espera su contestación.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si la conexión se rompió o si el servidor
    /// contestó con un código de error.
    fn orden(&self, linea: &str) -> NbResult<Respuesta> {
        let mut charla = self.conversacion.lock().unwrap_or_else(|p| p.into_inner());
        charla.escribir(linea)?;
        charla.esperar()
    }

    /// Velocidad, en el rango de `speech-dispatcher`: de -100 a 100.
    ///
    /// # Errors
    ///
    /// Los de [`Ssip::orden`].
    pub fn poner_velocidad(&self, valor: i32) -> NbResult<()> {
        self.orden(&format!("SET self RATE {}", valor.clamp(-100, 100)))
            .map(|_| ())
    }

    /// Voz de síntesis, por su nombre.
    ///
    /// # Errors
    ///
    /// Los de [`Ssip::orden`].
    pub fn poner_voz(&self, nombre: &str) -> NbResult<()> {
        // El nombre va entre comillas porque los hay con espacios —"Spanish
        // (Spain)"— y sin ellas el servidor lee solo la primera palabra. Las
        // comillas que trajera el propio nombre se quitan: no hay forma de
        // escaparlas en el protocolo, y un nombre con comillas no existe en
        // ningún sintetizador.
        let limpio = nombre.replace(['"', '\r', '\n'], "");
        self.orden(&format!("SET self SYNTHESIS_VOICE \"{limpio}\""))
            .map(|_| ())
    }

    /// Manda decir un texto. Devuelve el identificador del mensaje.
    ///
    /// El identificador es lo que permite saber **cuál** de los enunciados
    /// terminó cuando llega un evento, y por tanto encadenar el siguiente sin
    /// confundirse con el aviso tardío de uno que ya se canceló.
    ///
    /// # Errors
    ///
    /// Los de [`Ssip::orden`], o si el servidor no devuelve identificador.
    pub fn hablar(&self, texto: &str) -> NbResult<u64> {
        // El protocolo termina el texto con una línea que es solo un punto, así
        // que una línea del texto que fuera un punto lo cortaría por la mitad.
        // Se escapa duplicándolo, que es lo que dice el protocolo.
        let cuerpo: String = texto
            .lines()
            .map(|linea| {
                if linea.trim_end() == "." {
                    "..".to_owned()
                } else {
                    linea.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\r\n");

        // Las cuatro líneas son **una sola operación**: si otro hilo colara una
        // orden entre el `SPEAK` y el punto final, el servidor tomaría esa
        // orden por parte del texto y diría en voz alta lo que era una orden.
        let respuesta = {
            let mut charla = self.conversacion.lock().unwrap_or_else(|p| p.into_inner());
            charla.escribir("SPEAK")?;
            charla.esperar()?;
            charla.escribir(&cuerpo)?;
            charla.escribir(".")?;
            charla.esperar()?
        };

        respuesta
            .lineas
            .iter()
            .find_map(|l| l.trim().parse::<u64>().ok())
            .ok_or_else(|| {
                tracing::info!("la síntesis aceptó un enunciado sin decir cuál es");
                NbError::ComponentUnavailable
            })
    }

    /// Calla lo que **esta** conexión esté diciendo, y solo eso.
    ///
    /// # Errors
    ///
    /// Los de [`Ssip::orden`].
    pub fn callar(&self) -> NbResult<()> {
        // `STOP` calla lo que suena; `CANCEL` vacía además la cola. Se mandan
        // los dos porque parar tiene que dejarlo todo callado, incluido lo que
        // se hubiera encolado y aún no ha empezado.
        self.orden("STOP self")?;
        self.orden("CANCEL self")?;
        Ok(())
    }

    /// Las voces del sintetizador.
    ///
    /// # Errors
    ///
    /// Los de [`Ssip::orden`].
    pub fn voces(&self) -> NbResult<Vec<Voz>> {
        let respuesta = self.orden("LIST SYNTHESIS_VOICES")?;
        Ok(respuesta.lineas.iter().filter_map(|l| voz_de(l)).collect())
    }
}

impl Conversacion {
    fn escribir(&mut self, linea: &str) -> NbResult<()> {
        self.salida
            .write_all(format!("{linea}\r\n").as_bytes())
            .and_then(|()| self.salida.flush())
            .map_err(|error| {
                tracing::info!(error = %error, "se perdió la conexión con la síntesis");
                NbError::ComponentUnavailable
            })
    }

    fn esperar(&mut self) -> NbResult<Respuesta> {
        match self.respuestas.recv_timeout(ESPERA) {
            Ok(respuesta) if (200..300).contains(&respuesta.codigo) => Ok(respuesta),
            Ok(respuesta) => {
                tracing::info!(codigo = respuesta.codigo, "la síntesis rechazó una orden");
                Err(NbError::ComponentUnavailable)
            }
            Err(RecvTimeoutError::Timeout) => {
                tracing::info!("la síntesis no contestó a tiempo");
                Err(NbError::ComponentUnavailable)
            }
            Err(RecvTimeoutError::Disconnected) => Err(NbError::ComponentUnavailable),
        }
    }
}

/// Lee una línea del listado de voces: `nombre\tidioma\tvariante`.
///
/// Tabuladores y no espacios: el listado por la línea de órdenes venía en
/// columnas alineadas, y partir por espacios descartaba en silencio todas las
/// voces con nombre compuesto, que son la mitad e incluyen el español.
fn voz_de(linea: &str) -> Option<Voz> {
    let mut campos = linea.split('\t');
    let nombre = campos.next()?.trim();
    let idioma = campos.next()?.trim();
    let variante = campos.next().unwrap_or("none").trim();
    if nombre.is_empty() || idioma.is_empty() {
        return None;
    }
    Some(Voz {
        nombre: nombre.to_owned(),
        idioma: idioma.to_owned(),
        variante: variante.to_owned(),
    })
}

/// El hilo que lee: separa contestaciones de eventos y reparte cada cosa.
fn leer_sin_parar(flujo: UnixStream, respuestas: &Sender<Respuesta>, eventos: &Sender<Evento>) {
    let lector = BufReader::new(flujo);
    let mut acumuladas: Vec<String> = Vec::new();
    let mut de_evento: Vec<String> = Vec::new();

    for linea in lector.lines() {
        let Ok(linea) = linea else { break };
        let linea = linea.trim_end_matches(['\r', '\n']);
        let Some((codigo, resto, terminada)) = partir(linea) else {
            continue;
        };

        // `7xx` son avisos que nadie pidió; el resto contesta a una orden.
        if (700..800).contains(&codigo) {
            de_evento.push(resto.to_owned());
            if terminada {
                if let Some(evento) = evento_de(codigo, &de_evento) {
                    // Que nadie escuche los eventos no es un error: significa
                    // que la voz se apagó y este hilo aún no lo sabe.
                    let _ = eventos.send(evento);
                }
                de_evento.clear();
            }
            continue;
        }

        acumuladas.push(resto.to_owned());
        if terminada {
            let respuesta = Respuesta {
                codigo,
                lineas: std::mem::take(&mut acumuladas),
            };
            if respuestas.send(respuesta).is_err() {
                break;
            }
        }
    }
}

/// Parte una línea en código, contenido y si cierra la respuesta.
///
/// El cuarto carácter es la marca: un espacio cierra, un guion dice que sigue.
fn partir(linea: &str) -> Option<(u16, &str, bool)> {
    if linea.len() < 3 {
        return None;
    }
    let codigo = linea.get(..3)?.parse::<u16>().ok()?;
    let separador = linea.chars().nth(3);
    let resto = linea.get(4..).unwrap_or("");
    match separador {
        Some(' ') | None => Some((codigo, resto, true)),
        Some('-') => Some((codigo, resto, false)),
        _ => None,
    }
}

/// Convierte un aviso `7xx` en un evento, con el mensaje al que se refiere.
///
/// Las líneas de un aviso son el identificador del mensaje y el del cliente, en
/// ese orden. Solo interesa el primero.
fn evento_de(codigo: u16, lineas: &[String]) -> Option<Evento> {
    let id = lineas.first()?.trim().parse::<u64>().ok()?;
    match codigo {
        701 => Some(Evento::Empezado(id)),
        702 => Some(Evento::Terminado(id)),
        703 => Some(Evento::Cortado(id)),
        // 704 y 705 son pausa y reanudación, que este proveedor no usa: pausar
        // corta y reanudar vuelve a empezar el tramo.
        _ => None,
    }
}

/// Dónde escucha el servidor.
///
/// `SPEECHD_SOCK` gana si está puesta, que es como se conecta a un servidor que
/// no es el de la sesión. Si no, el sitio estándar dentro del directorio de
/// ejecución del usuario.
fn ruta_del_socket() -> NbResult<std::path::PathBuf> {
    if let Ok(ruta) = std::env::var("SPEECHD_SOCK") {
        if !ruta.trim().is_empty() {
            return Ok(std::path::PathBuf::from(ruta));
        }
    }
    let base = std::env::var("XDG_RUNTIME_DIR").map_err(|_| {
        tracing::info!("sin XDG_RUNTIME_DIR no se sabe dónde escucha la síntesis");
        NbError::ComponentUnavailable
    })?;
    Ok(std::path::PathBuf::from(base)
        .join("speech-dispatcher")
        .join("speechd.sock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_linea_con_espacio_cierra_la_respuesta_y_una_con_guion_no() {
        assert_eq!(
            partir("208 OK CLIENT NAME SET"),
            Some((208, "OK CLIENT NAME SET", true))
        );
        assert_eq!(
            partir("249-Spanish (Spain)\tes\tnone"),
            Some((249, "Spanish (Spain)\tes\tnone", false))
        );
    }

    /// Es la única marca de que una respuesta terminó. Sin distinguirla, cada
    /// orden acabaría leyendo la contestación de la anterior.
    #[test]
    fn lo_que_no_empieza_por_un_codigo_no_es_una_linea_del_protocolo() {
        assert_eq!(partir(""), None);
        assert_eq!(partir("ab"), None);
        assert_eq!(partir("abc def"), None);
        assert_eq!(partir("225x1"), None);
    }

    #[test]
    fn una_voz_se_lee_por_tabuladores_y_no_por_espacios() {
        // El nombre lleva espacios dentro. Partir por espacios descartaba en
        // silencio la mitad de los idiomas, español incluido.
        assert_eq!(
            voz_de("Spanish (Spain)\tes\tnone"),
            Some(Voz {
                nombre: "Spanish (Spain)".to_owned(),
                idioma: "es".to_owned(),
                variante: "none".to_owned(),
            })
        );
    }

    #[test]
    fn una_linea_de_voz_incompleta_no_inventa_una_voz() {
        assert_eq!(voz_de("solo-un-campo"), None);
        assert_eq!(voz_de("\tes\tnone"), None);
        assert_eq!(voz_de(""), None);
    }

    #[test]
    fn los_avisos_se_reconocen_por_su_codigo_y_traen_el_mensaje() {
        assert_eq!(
            evento_de(701, &["7".to_owned(), "3".to_owned()]),
            Some(Evento::Empezado(7))
        );
        assert_eq!(
            evento_de(702, &["7".to_owned()]),
            Some(Evento::Terminado(7))
        );
        assert_eq!(evento_de(703, &["7".to_owned()]), Some(Evento::Cortado(7)));
        // Pausa y reanudación no se usan: pausar corta y reanudar reempieza.
        assert_eq!(evento_de(704, &["7".to_owned()]), None);
        assert_eq!(evento_de(702, &["no es un numero".to_owned()]), None);
    }
}
