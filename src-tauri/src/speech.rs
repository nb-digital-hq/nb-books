//! Síntesis de voz **fuera del dispositivo**, cuando el usuario la autoriza.
//!
//! # Por qué existe, y por qué no contradice el innegociable
//!
//! La regla del proyecto nunca fue "nada sale". Es **"nada sale sin
//! consentimiento explícito"**, y ya hay tres salidas que el usuario concede una
//! a una: los catálogos, los metadatos y la consulta de
//! la consulta en línea. Esta es la cuarta, y es la que más pesa: lo que sale es el **texto
//! del libro**, párrafo a párrafo. Por eso tiene propósito propio —
//! [`crate::net::Purpose::Speech`]— y se concede sola, sin colarse detrás de
//! ningún otro permiso.
//!
//! Y existe porque la alternativa local es la que es. `espeak-ng` es un
//! sintetizador formántico: suena a máquina de los noventa porque lo es, y eso
//! no se arregla ajustando la velocidad ni eligiendo mejor la voz. Quien quiera
//! escuchar un libro entero necesita una voz que se pueda escuchar una hora.
//!
//! # Por qué un servidor que se configura, y no uno puesto de fábrica
//!
//! Es la misma decisión que se tomó con el traductor, y por los mismos
//! motivos, más uno nuevo.
//!
//! * **Elegir por el usuario a quién le manda el texto de sus libros no nos
//!   corresponde.** El campo viene vacío y sin él no se ofrece nada.
//! * **Los servicios "gratuitos" que usan otros lectores no son servicios.** El
//!   que hace sonar bien a Readest en Linux es un extremo interno del Edge de
//!   Microsoft al que se llega por ingeniería inversa. Funciona hoy; Microsoft
//!   ya ha empezado a exigirle credenciales antiabuso y a filtrar rangos de
//!   nube, y su uso comercial sin suscripción es al menos discutible. Un
//!   producto que quiere sentarse con una universidad no puede apoyar una
//!   función en algo así.
//! * **El mismo protocolo sirve para un servidor local.** Se habla el de
//!   `/v1/audio/speech`, que es el que implementan tanto los servicios de pago
//!   como los servidores que se levantan en la propia máquina. Apuntar el ajuste
//!   a `localhost` es tener voz neuronal sin que salga nada.
//!
//! # La credencial va al llavero del sistema, y por eso hay servicios
//!
//! Sin almacén de secretos esto no servía para lo que se construyó: **todos**
//! los servicios de síntesis con capa gratuita permanente piden credencial, sin
//! excepción, así que lo único alcanzable era un servidor sin clave en la propia
//! máquina, que es trabajo. La investigación que debió preceder a
//! esta función está en
//! [voice-services.md](../../docs/architecture/voice-services.md),
//! con las cifras de cada oferta.
//!
//! La clave se guarda en [`crate::secrets`] —el llavero del anfitrión— y nunca
//! en la base: los ajustes se exportan con la biblioteca, y un secreto que
//! viaja en la exportación es un secreto filtrado. Viaja además **en una
//! cabecera y jamás en la dirección**, porque la dirección es lo que queda
//! escrito en el registro de salidas que el usuario consulta.
//!
//! Y nada de esto quita que el destino que mejor cumple la regla siga siendo
//! el que corre en tu propia máquina: la credencial existe para los demás.
//!
//! Contrato: `docs/architecture/voice-services.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::error::{NbError, NbResult};
use crate::net::{Network, Purpose};

/// Cuánto puede durar el texto que se manda de una vez, en caracteres.
///
/// Cuatro mil. Es más de lo que ocupa cualquier párrafo y menos de lo que
/// ningún servicio acepta; sobre todo, acota lo que sale del dispositivo en una
/// sola petición a algo que se puede razonar.
const MAXIMO_TEXTO: usize = 4_000;

/// Formato que se pide. MP3 porque lo entienden todos los servicios y el webview
/// lo reproduce sin ayuda.
const FORMATO: &str = "mp3";

/// Lo que hace falta para sintetizar fuera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    /// Anfitrión del servicio, sin esquema ni ruta. Vacío significa que no hay.
    pub host: String,
    /// Modelo o motor, tal y como lo nombre el servicio.
    pub model: String,
    /// Voz, tal y como la nombre el servicio.
    pub voice: String,
}

impl Destino {
    /// Si hay servicio configurado.
    ///
    /// Sin anfitrión no se ofrece nada: no es un hueco por rellenar, es la
    /// decisión de fábrica.
    #[must_use]
    pub fn configurado(&self) -> bool {
        !self.host.trim().is_empty()
    }

    /// La dirección del servicio, con esquema y sin barra final.
    ///
    /// # Qué se admite escribir, y por qué tantas formas
    ///
    /// Porque cada guía lo escribe de una manera y quien configura copia la suya:
    /// `localhost:8880`, `http://localhost:8880/`, `HTTP://localhost:8880/v1/`,
    /// `https://api.example/v1/audio/speech`. Todas llevan al mismo sitio.
    ///
    /// **Sin esquema, el esquema depende de a dónde apunta.** Antes
    /// se suponía siempre `https`, y el ejemplo que enseñaba Ajustes era
    /// `localhost:8880`: escribir exactamente el ejemplo llevaba a hablar cifrado
    /// con un servidor local que no lo está, y la voz no respondía. Hacia la
    /// propia máquina o la red propia se supone `http`, que es lo que sirven esos
    /// servidores; hacia fuera, `https`. Quien decide si el resultado vale sigue
    /// siendo `net::host_de`, que rechaza el claro hacia internet.
    #[must_use]
    pub fn direccion(&self) -> String {
        crate::net::direccion_escrita(&self.host)
    }

    /// La dirección exacta a la que se manda a sintetizar.
    ///
    /// Existe porque **la app y su servidor no se ponían de acuerdo en la ruta**.
    /// El protocolo se documenta en todas partes como `base_url` = `.../v1`, que es
    /// lo que piden el SDK de OpenAI y las guías de cualquier servidor compatible,
    /// así que quien copia su configuración escribe `http://192.168.100.10:8880/v1`
    /// —y era lo razonable— y aquí se le añadía otro `/v1` detrás. El resultado era
    /// un 404 contra un servidor que funcionaba perfectamente.
    ///
    /// Se aceptan las formas que alguien escribe de verdad: el anfitrión solo, con
    /// `/v1`, con `/v1/audio` y la dirección entera del punto de síntesis, con o sin
    /// barra final y con la ruta en mayúsculas.
    #[must_use]
    pub fn punto_de_sintesis(&self) -> String {
        let base = self.direccion();
        let minusculas = base.to_ascii_lowercase();
        let recorte = ["/v1/audio/speech", "/v1/audio", "/v1"]
            .iter()
            .find(|sufijo| minusculas.ends_with(*sufijo))
            .map_or(0, |sufijo| sufijo.len());
        let base = &base[..base.len() - recorte];
        format!("{}/v1/audio/speech", base.trim_end_matches('/'))
    }
}

/// Sintetiza un texto y deja el audio en la caché.
///
/// Devuelve la ruta del archivo. Se guarda en un archivo y no se devuelve por el
/// puente porque son cientos de kilobytes por párrafo: cruzarlos como datos
/// obligaría a codificarlos en base64 y a moverlos por el canal de mensajes, y
/// el webview ya sabe leer de la caché por el protocolo de assets.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay servicio configurado o el texto pasa
///   del límite.
/// - Los de la puerta de red: [`NbError::NetworkDisabled`],
///   [`NbError::ConsentRequired`] o [`NbError::NetworkFailure`].
/// - [`NbError::StorageFailure`] si no se puede escribir el audio.
pub fn sintetizar(
    db: &crate::db::Database,
    net: &Arc<Network>,
    carpeta: &Path,
    destino: &Destino,
    texto: &str,
    velocidad: f32,
) -> NbResult<PathBuf> {
    if !destino.configurado() {
        tracing::info!("se pidió voz remota sin servicio configurado");
        return Err(NbError::PathNotAllowed);
    }

    let limpio = texto.trim();
    if limpio.is_empty() || limpio.len() > MAXIMO_TEXTO {
        tracing::info!(
            largo = limpio.len(),
            "se rechazó un texto fuera de límites para la voz remota"
        );
        return Err(NbError::PathNotAllowed);
    }

    // La caché se indexa por el contenido de la petición, no por el texto: dos
    // párrafos iguales con voces distintas suenan distinto, y servir uno por el
    // otro sería el mismo defecto que servir la portada de otro libro.
    let huella = crate::library::hash_reader(
        format!(
            "{}|{}|{}|{velocidad}|{limpio}",
            destino.host, destino.model, destino.voice
        )
        .as_bytes(),
    )?;
    let archivo = carpeta.join(format!("{}.{FORMATO}", huella.0.as_str().replace(':', "-")));
    if archivo.is_file() {
        return Ok(archivo);
    }

    let url = destino.punto_de_sintesis();
    let cuerpo = cuerpo_de(destino, limpio, velocidad);

    // La credencial se busca por el anfitrión canónico y no por lo que hay
    // escrito en el ajuste, para que `localhost:8880` y `http://localhost:8880`
    // sean el mismo servicio y no dos. Ver `secrets::cuenta_de_voz`.
    let credencial = credencial_de(&url);

    let conn = db.connection()?;
    let respuesta = net.post_json(&conn, &url, Purpose::Speech, &cuerpo, credencial.as_deref())?;
    drop(conn);

    if respuesta.status != 200 {
        tracing::info!(status = respuesta.status, "la síntesis remota no respondió");
        return Err(NbError::NetworkFailure);
    }
    if respuesta.body.is_empty() {
        tracing::info!("la síntesis remota devolvió un audio vacío");
        return Err(NbError::NetworkFailure);
    }

    std::fs::create_dir_all(carpeta).map_err(|error| {
        tracing::error!(error = %error, "no se pudo preparar la caché de voz");
        NbError::StorageFailure
    })?;
    crate::paths::write_atomic(&archivo, |temporal| {
        std::fs::write(temporal, &respuesta.body).map_err(|error| {
            tracing::error!(error = %error, "no se pudo escribir el audio");
            NbError::StorageFailure
        })
    })?;

    Ok(archivo)
}

/// La credencial del servicio al que se va a hablar, si la hay.
///
/// **Un fallo del llavero no impide sintetizar**, y es a propósito: la mitad de
/// los destinos útiles no piden credencial —un servidor en la propia máquina, el
/// de una institución— y en un escritorio sin Secret Service el llavero falla
/// siempre. Negar la voz ahí sería castigar a quien no necesitaba clave por un
/// almacén que no iba a usar. Si el servicio sí la pedía, responderá 401 y eso
/// se ve; lo que no puede pasar es que no suene nada sin motivo visible.
fn credencial_de(url: &str) -> Option<String> {
    let host = crate::net::host_de(url).ok()?;
    match crate::secrets::leer(&crate::secrets::cuenta_de_voz(&host)) {
        Ok(secreto) => secreto,
        Err(error) => {
            tracing::info!(%error, "no se pudo consultar el llavero; se sale sin credencial");
            None
        }
    }
}

/// Cuerpo de la petición, en el formato de `/v1/audio/speech`.
///
/// Se compone a mano y no con un serializador porque son cinco campos y hay que
/// escapar uno: traer una derivación para esto sería más código, no menos.
fn cuerpo_de(destino: &Destino, texto: &str, velocidad: f32) -> String {
    format!(
        r#"{{"model":"{}","voice":"{}","input":"{}","response_format":"{FORMATO}","speed":{}}}"#,
        escapar(&destino.model),
        escapar(&destino.voice),
        escapar(texto),
        // Acotada al rango que el protocolo admite: fuera de él, los servicios
        // devuelven un error en lugar de recortar.
        (velocidad.clamp(0.25, 4.0) * 100.0).round() / 100.0
    )
}

/// Escapa lo que va dentro de una cadena JSON.
///
/// El texto es un párrafo de un libro: lleva comillas, guiones largos, saltos de
/// línea y lo que el autor quisiera. Sin escapar, una comilla parte el mensaje y
/// el servidor recibe algo que no es lo que se le mandó.
fn escapar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len() + 8);
    for c in texto.chars() {
        match c {
            '"' => salida.push_str("\\\""),
            '\\' => salida.push_str("\\\\"),
            '\n' => salida.push_str("\\n"),
            '\r' => salida.push_str("\\r"),
            '\t' => salida.push_str("\\t"),
            // Los de control no se pueden escribir tal cual dentro de una
            // cadena JSON, y en un libro aparecen más de lo que parece.
            c if (c as u32) < 0x20 => salida.push_str(&format!("\\u{:04x}", c as u32)),
            c => salida.push(c),
        }
    }
    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn destino() -> Destino {
        Destino {
            host: "voz.example".to_owned(),
            model: "tts-1".to_owned(),
            voice: "alloy".to_owned(),
        }
    }

    /// Sin anfitrión no hay servicio, y eso es la decisión de fábrica y no un
    /// hueco por rellenar.
    #[test]
    fn sin_anfitrion_no_hay_servicio() {
        let vacio = Destino {
            host: "  ".to_owned(),
            ..destino()
        };
        assert!(!vacio.configurado());
        assert!(destino().configurado());
    }

    /// Un párrafo de un libro lleva comillas, y sin escaparlas parte el mensaje:
    /// el servidor recibiría algo que no es lo que se le mandó.
    /// Un servidor local se apunta con su esquema, y así se puede tener voz
    /// neuronal sin que salga nada del dispositivo. Quien decide si vale es la
    /// puerta de red, que rechaza el claro hacia fuera.
    #[test]
    fn un_anfitrion_con_esquema_se_respeta_y_uno_sin_el_se_asume_seguro() {
        let local = Destino {
            host: "http://localhost:8880/".to_owned(),
            ..destino()
        };
        assert_eq!(local.direccion(), "http://localhost:8880");
        assert_eq!(destino().direccion(), "https://voz.example");
    }

    /// **Regresión.** Todas las formas en que alguien escribe el mismo
    /// servidor llevan al mismo punto de síntesis.
    ///
    /// Antes escribir `localhost:8880` -el ejemplo que enseñaba
    /// Ajustes- suponía `https` y la voz no respondía, porque un servidor local no
    /// habla cifrado.
    #[test]
    fn cualquier_forma_de_escribir_el_servidor_lleva_al_mismo_punto() {
        let punto = |escrito: &str| {
            Destino {
                host: escrito.to_owned(),
                ..destino()
            }
            .punto_de_sintesis()
        };
        for escrito in [
            "localhost:8880",
            "localhost:8880/",
            "http://localhost:8880",
            "http://localhost:8880/",
            "http://localhost:8880/v1",
            "http://localhost:8880/v1/",
            "HTTP://localhost:8880/V1/",
            "http://localhost:8880/v1/audio",
            "http://localhost:8880/v1/audio/speech",
            "http://localhost:8880/v1/audio/speech/",
            "  localhost:8880/v1  ",
        ] {
            assert_eq!(
                punto(escrito),
                "http://localhost:8880/v1/audio/speech",
                "{escrito:?}"
            );
        }
        assert_eq!(
            punto("192.168.100.10:8880/v1"),
            "http://192.168.100.10:8880/v1/audio/speech"
        );
        // Hacia fuera, sin esquema, cifrado: el claro hacia internet no se supone.
        assert_eq!(
            punto("api.example/v1/"),
            "https://api.example/v1/audio/speech"
        );
    }

    #[test]
    fn el_texto_se_escapa_para_caber_en_el_mensaje() {
        let cuerpo = cuerpo_de(&destino(), "Dijo \"hola\"\ny se fue\\", 1.0);
        assert!(cuerpo.contains(r#"\"hola\""#), "{cuerpo}");
        assert!(cuerpo.contains(r"\n"), "{cuerpo}");
        assert!(cuerpo.contains(r"\\"), "{cuerpo}");
        // Y sigue siendo un mensaje de una sola línea.
        assert_eq!(cuerpo.lines().count(), 1);
    }

    #[test]
    fn los_caracteres_de_control_se_escriben_como_escapes() {
        // En un libro aparecen más de lo que parece: separadores de línea,
        // marcas de formato, restos de la conversión desde otro formato. Sin
        // escaparlos el mensaje deja de ser JSON válido y el servicio lo rechaza
        // entero, así que se pierde el párrafo por un carácter que nadie ve.
        assert_eq!(escapar("a\u{0}b\u{7}c"), "a\\u0000b\\u0007c");
    }

    /// La velocidad se acota antes de salir: fuera de rango los servicios
    /// devuelven un error en lugar de recortar, y eso sería una salida gastada
    /// para nada.
    #[test]
    fn la_velocidad_se_acota_al_rango_del_protocolo() {
        assert!(cuerpo_de(&destino(), "hola", 99.0).contains(r#""speed":4"#));
        assert!(cuerpo_de(&destino(), "hola", 0.01).contains(r#""speed":0.25"#));
        assert!(cuerpo_de(&destino(), "hola", 1.5).contains(r#""speed":1.5"#));
    }

    /// Un anfitrión con ruta la conserva, y eso hace falta de verdad: no todos
    /// los servicios cuelgan el protocolo de la raíz. OpenRouter, por ejemplo, lo
    /// sirve bajo `/api`. Escribiéndolo con su ruta funciona sin un ajuste más.
    #[test]
    fn un_anfitrion_con_ruta_la_conserva() {
        let con_prefijo = Destino {
            host: "servicio.example/api/".to_owned(),
            ..destino()
        };
        assert_eq!(con_prefijo.direccion(), "https://servicio.example/api");
    }

    #[test]
    fn el_cuerpo_lleva_lo_que_el_protocolo_espera() {
        let cuerpo = cuerpo_de(&destino(), "hola", 1.0);
        for campo in ["\"model\"", "\"voice\"", "\"input\"", "\"response_format\""] {
            assert!(cuerpo.contains(campo), "falta {campo} en {cuerpo}");
        }
    }
}
