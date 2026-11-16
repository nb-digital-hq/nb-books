//! Un proveedor de IA **externo**, con la clave del usuario o el servidor de una
//! institución.
//!
//! Contrato: `docs/architecture/ai/privacy.md`.
//!
//! # Qué protocolos, y por qué dos
//!
//! **El compatible con OpenAI y el nativo de Anthropic**, decidido el
//! 2026-09-16. El primero no es "OpenAI": es la forma de `/v1/chat/completions`
//! que hablan también los servidores que se levantan en la propia máquina o en la
//! de una institución —Ollama, vLLM, LM Studio— y la mayoría de servicios de pago.
//! El segundo va aparte porque Anthropic no habla el primero de forma nativa, y
//! adaptarlo por un intermediario sería mandar el texto a un tercero más.
//!
//! # Qué hay en cada envío, y qué no
//!
//! * **La instrucción del producto, en el mensaje de sistema, y los fragmentos, en
//!   el del usuario**: separados por el protocolo y no mezclados en una cadena, que
//!   es donde se apoya la defensa frente a un documento hostil.
//! * **Nunca el documento entero**, que es la regla 1 de `privacy.md`. No se puede
//!   comprobar que un conjunto de fragmentos no sea "el documento entero" de un
//!   cuento corto, así que lo que se hace cumplir es un tope: [`MAXIMO_DE_TEXTO`].
//! * **Nunca el localizador de un fragmento.** El modelo no lo necesita para citar
//!   —se cita por número— y un localizador lleva identificadores internos de la
//!   biblioteca que no tienen por qué salir.
//! * **Nunca la credencial en el cuerpo**: va en una cabecera, y por eso el cuerpo
//!   se puede enseñar entero antes de enviarlo y guardar entero después.
//!
//! # Por qué nunca embeddings
//!
//! Porque los vectores se generan sobre la biblioteca, en segundo plano y por
//! cientos, y mandarlos fuera sería mandar libros enteros troceados: exactamente
//! lo que prohíbe la regla 1. [ADR-007] los decidió locales, y este proveedor no
//! declara la capacidad, así que no hay forma de pedírselos.
//!
//! [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md

use std::sync::Arc;

use serde_json::{json, Value};

use super::envios::{self, Anotacion};
use super::{AiProvider, Availability, Capability, Completion, Placement, ProviderInfo, Request};
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::net::{Envio, Network, Origin, Purpose};
use crate::settings::{self, SettingKey, SettingValue};

/// Identificador estable del proveedor externo. Es el que una función nombra con
/// `Choice::Provider` cuando el usuario lo ha elegido.
pub const ID: &str = "external";

/// Lo que atiende: todo lo que genera texto, y **nunca** embeddings.
const CAPACIDADES: &[Capability] = &[
    Capability::Summary,
    Capability::Flashcards,
    Capability::Questions,
    Capability::Concepts,
];

/// Cuánto texto de documento puede salir en un envío, en caracteres: **32.000**.
///
/// Son unos dieciséis fragmentos de búsqueda de 1.920 caracteres, que es más de lo
/// que una función de estudio necesita recuperar para responder, y menos que
/// cualquier libro. Es el mecanismo de la regla "nunca se envía el documento
/// completo": no puede saber qué es un documento completo, pero sí impedir que
/// salga uno.
pub const MAXIMO_DE_TEXTO: usize = 32_000;

/// Cuánto puede generar una respuesta, en tokens. Solo lo exige Anthropic.
///
/// Al compatible con OpenAI no se le manda: el nombre del campo cambió
/// —`max_tokens` pasó a `max_completion_tokens` en OpenAI y los servidores
/// compatibles no siempre aceptan el nuevo—, y un campo que la mitad de los
/// servidores rechaza rompe más de lo que acota.
const MAXIMO_DE_SALIDA: u32 = 2_048;

/// La versión de la API de Anthropic que se habla. Es obligatoria en cada petición.
const VERSION_DE_ANTHROPIC: &str = "2023-06-01";

/// A qué protocolo se habla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocolo {
    /// `/v1/chat/completions`, el de OpenAI y los servidores compatibles.
    OpenAi,
    /// `/v1/messages`, el nativo de Anthropic.
    Anthropic,
}

impl Protocolo {
    /// El valor de la preferencia `ai_provider`, y el que se guarda en el registro.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }

    /// Desde el valor guardado. `none` y cualquier otra cosa son que no hay.
    #[must_use]
    pub fn desde(valor: &str) -> Option<Self> {
        match valor {
            "openai" => Some(Self::OpenAi),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    /// El final de la ruta del punto al que se manda.
    const fn ruta(self) -> &'static str {
        match self {
            Self::OpenAi => "/v1/chat/completions",
            Self::Anthropic => "/v1/messages",
        }
    }

    /// Las cabeceras que el protocolo exige además de la credencial.
    const fn cabeceras(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::OpenAi => &[],
            Self::Anthropic => &[("anthropic-version", VERSION_DE_ANTHROPIC)],
        }
    }
}

/// El proveedor externo que el usuario configuró.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuracion {
    pub protocolo: Protocolo,
    /// Lo que hay escrito en Ajustes, con o sin esquema y con o sin ruta.
    pub host: String,
    pub modelo: String,
}

impl Configuracion {
    /// La que hay guardada, si hay una completa: protocolo, dirección y modelo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si las preferencias no se pueden leer.
    pub fn guardada(conn: &rusqlite::Connection) -> NbResult<Option<Self>> {
        let texto = |clave| match settings::get(conn, clave) {
            Ok(SettingValue::Text(valor)) => Ok(valor.trim().to_owned()),
            Ok(_) => Ok(String::new()),
            Err(error) => Err(error),
        };
        let Some(protocolo) = Protocolo::desde(&texto(SettingKey::AiProvider)?) else {
            return Ok(None);
        };
        let host = texto(SettingKey::AiHost)?;
        let modelo = texto(SettingKey::AiModel)?;
        if host.is_empty() || modelo.is_empty() {
            return Ok(None);
        }
        Ok(Some(Self {
            protocolo,
            host,
            modelo,
        }))
    }

    /// La dirección exacta a la que se manda.
    ///
    /// Admite las formas que alguien copia de una guía —el anfitrión solo, con
    /// `/v1` o con la ruta entera—, igual que la voz: un `/v1` escrito
    /// por quien configura no puede acabar en `/v1/v1/...` y un 404 contra un
    /// servidor que funciona.
    #[must_use]
    pub fn punto(&self) -> String {
        let base = crate::net::direccion_escrita(&self.host);
        let minusculas = base.to_ascii_lowercase();
        let ruta = self.protocolo.ruta();
        let recortes: &[&str] = match self.protocolo {
            Protocolo::OpenAi => &["/v1/chat/completions", "/v1/chat", "/v1"],
            Protocolo::Anthropic => &["/v1/messages", "/v1"],
        };
        let recorte = recortes
            .iter()
            .find(|sufijo| minusculas.ends_with(*sufijo))
            .map_or(0, |sufijo| sufijo.len());
        format!(
            "{}{ruta}",
            base[..base.len() - recorte].trim_end_matches('/')
        )
    }

    /// El anfitrión al que se sale, canónico. Es el del consentimiento y el de la
    /// credencial.
    ///
    /// # Errors
    ///
    /// [`NbError::PathNotAllowed`] si la dirección no vale: sin anfitrión, o en
    /// claro hacia fuera de la red propia.
    pub fn anfitrion(&self) -> NbResult<String> {
        crate::net::host_de(&self.punto())
    }
}

/// Lo que se va a mandar: a dónde y el cuerpo entero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carta {
    pub url: String,
    pub cuerpo: String,
}

/// Compone el mensaje de una petición, sin mandarlo.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si el texto de los fragmentos pasa de
/// [`MAXIMO_DE_TEXTO`] o si la petición es de una capacidad que este proveedor no
/// atiende.
pub fn componer(config: &Configuracion, request: &Request) -> NbResult<Carta> {
    if !CAPACIDADES.contains(&request.capability) {
        tracing::info!(
            capacidad = request.capability.as_str(),
            "se pidió a un proveedor externo algo que no atiende"
        );
        return Err(NbError::PathNotAllowed);
    }
    let caracteres: usize = request
        .passages
        .iter()
        .map(|p| p.text.chars().count())
        .sum();
    if caracteres > MAXIMO_DE_TEXTO {
        // El tamaño sí se registra; el texto no.
        tracing::info!(
            caracteres,
            "se rechazó un envío con demasiado texto del documento"
        );
        return Err(NbError::PathNotAllowed);
    }

    let super::contexto::Mensajes {
        sistema,
        usuario: contenido,
    } = super::contexto::mensajes(request);

    let cuerpo = match config.protocolo {
        Protocolo::OpenAi => json!({
            "model": config.modelo,
            "messages": [
                { "role": "system", "content": sistema },
                { "role": "user", "content": contenido },
            ],
        }),
        Protocolo::Anthropic => json!({
            "model": config.modelo,
            "max_tokens": MAXIMO_DE_SALIDA,
            "system": sistema,
            "messages": [ { "role": "user", "content": contenido } ],
        }),
    };

    Ok(Carta {
        url: config.punto(),
        cuerpo: cuerpo.to_string(),
    })
}

/// El texto de una respuesta correcta.
///
/// # Errors
///
/// [`NbError::NetworkFailure`] si la respuesta no es lo que el protocolo dice que
/// es, o no trae texto. Un servicio que contesta otra cosa no ha contestado.
pub fn leer_respuesta(protocolo: Protocolo, cuerpo: &[u8]) -> NbResult<String> {
    let valor: Value = serde_json::from_slice(cuerpo).map_err(|_| {
        tracing::info!("un proveedor de IA devolvió algo que no es JSON");
        NbError::NetworkFailure
    })?;
    let texto = match protocolo {
        Protocolo::OpenAi => valor
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Protocolo::Anthropic => valor
            .get("content")
            .and_then(Value::as_array)
            .map(|bloques| {
                bloques
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            }),
    };
    match texto {
        Some(texto) if !texto.trim().is_empty() => Ok(texto),
        _ => {
            tracing::info!("un proveedor de IA respondió sin texto");
            Err(NbError::NetworkFailure)
        }
    }
}

/// Lo que respondió el servicio a un envío que salió.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Respuesta {
    pub status: u16,
    /// El texto, si la respuesta fue correcta y lo traía.
    pub texto: Option<String>,
}

/// El proveedor externo, con la puerta de red y la base.
pub struct ExternalProvider {
    config: Configuracion,
    anfitrion: String,
    etiqueta: String,
    db: Database,
    red: Arc<Network>,
}

impl ExternalProvider {
    /// Prepara el proveedor de una configuración.
    ///
    /// # Errors
    ///
    /// [`NbError::PathNotAllowed`] si la dirección no vale. Se comprueba aquí y no
    /// al enviar para que Ajustes pueda decirlo al escribirla.
    pub fn nuevo(config: Configuracion, db: Database, red: Arc<Network>) -> NbResult<Self> {
        let anfitrion = config.anfitrion()?;
        let etiqueta = format!("{} en {anfitrion}", config.modelo);
        Ok(Self {
            config,
            anfitrion,
            etiqueta,
            db,
            red,
        })
    }

    #[must_use]
    pub const fn configuracion(&self) -> &Configuracion {
        &self.config
    }

    /// El anfitrión canónico al que sale.
    #[must_use]
    pub fn anfitrion(&self) -> &str {
        &self.anfitrion
    }

    /// Manda una petición **si hay consentimiento**, y la anota si salió.
    ///
    /// # Errors
    ///
    /// - Los de [`componer`].
    /// - [`NbError::NetworkDisabled`] o [`NbError::ConsentRequired`] si la puerta
    ///   no deja salir. Entonces no sale nada y no se anota nada: no hay contenido
    ///   que auditar, y el intento ya queda en el registro de salidas.
    /// - [`NbError::NetworkFailure`] si no se pudo completar. **Ese sí se anota**,
    ///   porque el mensaje pudo llegar aunque la respuesta no volviera.
    /// - [`NbError::StorageFailure`] si la base no responde.
    pub fn enviar(&self, request: &Request) -> NbResult<Respuesta> {
        let carta = componer(&self.config, request)?;
        let credencial = credencial_de(&self.anfitrion);
        let conn = self.db.connection()?;

        let resultado = self.red.post_json_from(
            &conn,
            &Origin::App,
            &carta.url,
            Purpose::Ai,
            Envio {
                cuerpo: &carta.cuerpo,
                credencial: credencial.as_deref(),
                cabeceras: self.config.protocolo.cabeceras(),
            },
        );

        let status = match &resultado {
            Ok(respuesta) => Some(respuesta.status),
            Err(NbError::NetworkFailure) => None,
            Err(_) => {
                return resultado.map(|_| Respuesta {
                    status: 0,
                    texto: None,
                })
            }
        };
        envios::anotar(
            &conn,
            &Anotacion {
                protocolo: self.config.protocolo.as_str(),
                host: &self.anfitrion,
                modelo: &self.config.modelo,
                capacidad: request.capability,
                cuerpo: &carta.cuerpo,
                status,
            },
        )?;

        let respuesta = resultado?;
        let texto = if (200..300).contains(&respuesta.status) {
            Some(leer_respuesta(self.config.protocolo, &respuesta.body)?)
        } else {
            tracing::info!(
                status = respuesta.status,
                "un proveedor de IA rechazó la petición"
            );
            None
        };
        Ok(Respuesta {
            status: respuesta.status,
            texto,
        })
    }
}

impl AiProvider for ExternalProvider {
    fn id(&self) -> &str {
        ID
    }

    fn label(&self) -> &str {
        &self.etiqueta
    }

    fn placement(&self) -> Placement {
        Placement::External
    }

    fn capabilities(&self) -> &[Capability] {
        CAPACIDADES
    }

    fn host(&self) -> Option<&str> {
        Some(&self.anfitrion)
    }

    /// Listo si está configurado, que es lo único que existir ya garantiza.
    ///
    /// **El consentimiento no entra aquí**, y es a propósito: si faltar el permiso
    /// dejara al proveedor "no disponible", la interfaz escondería la función en
    /// lugar de enseñar qué se enviaría y pedirlo, que es el criterio. La
    /// puerta de red lo hace cumplir al enviar.
    fn readiness(&self) -> Availability {
        Availability::Ready(ProviderInfo {
            id: ID.to_owned(),
            label: self.etiqueta.clone(),
            placement: Placement::External,
            host: Some(self.anfitrion.clone()),
        })
    }

    fn input_chars(&self) -> usize {
        MAXIMO_DE_TEXTO
    }

    fn generate(&self, request: &Request) -> NbResult<Completion> {
        let respuesta = self.enviar(request)?;
        let text = respuesta.texto.ok_or(NbError::NetworkFailure)?;
        Ok(Completion {
            text,
            // Vacío es "no verificable", y es lo que es hasta que la generación enlace
            // cada afirmación a su fragmento.
            grounding: Vec::new(),
            provider_id: ID.to_owned(),
        })
    }
}

/// La credencial del proveedor, si la hay.
///
/// Un fallo del llavero no impide enviar, por lo mismo que en la voz: un servidor
/// en la propia máquina o el de una institución no la piden, y en un escritorio sin
/// llavero fallaría siempre. Si el servicio la pedía, responderá 401, y eso se ve.
fn credencial_de(anfitrion: &str) -> Option<String> {
    match crate::secrets::leer(&crate::secrets::cuenta_de_ia(anfitrion)) {
        Ok(secreto) => secreto,
        Err(error) => {
            tracing::info!(%error, "no se pudo consultar el llavero; se sale sin credencial");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Passage;
    use crate::reader::Locator;

    fn config(protocolo: Protocolo, host: &str) -> Configuracion {
        Configuracion {
            protocolo,
            host: host.to_owned(),
            modelo: "un-modelo".to_owned(),
        }
    }

    fn peticion(fragmentos: &[&str]) -> Request {
        Request {
            capability: Capability::Summary,
            instruction: "Resume.".to_owned(),
            passages: fragmentos
                .iter()
                .map(|t| Passage {
                    locator: Locator::new(
                        "OEBPS/capitulo-secreto.xhtml",
                        "application/xhtml+xml",
                        crate::reader::locator::Locations {
                            progression: Some(0.5),
                            ..crate::reader::locator::Locations::default()
                        },
                        None,
                    )
                    .expect("localizador"),
                    text: (*t).to_owned(),
                })
                .collect(),
            task: String::new(),
            language: Some("es".to_owned()),
        }
    }

    /// Cualquier forma de escribir la dirección lleva al mismo punto, en los dos
    /// protocolos. Un `/v1` que alguien copió de una guía no puede acabar en
    /// `/v1/v1`.
    #[test]
    fn cualquier_forma_de_escribir_el_servidor_lleva_al_mismo_punto() {
        for escrito in [
            "api.openai.com",
            "https://api.openai.com/",
            "https://api.openai.com/v1",
            "HTTPS://api.openai.com/v1/chat/completions",
        ] {
            assert_eq!(
                config(Protocolo::OpenAi, escrito).punto(),
                "https://api.openai.com/v1/chat/completions",
                "{escrito}"
            );
        }
        for escrito in [
            "api.anthropic.com",
            "https://api.anthropic.com/v1/",
            "api.anthropic.com/v1/messages",
        ] {
            assert_eq!(
                config(Protocolo::Anthropic, escrito).punto(),
                "https://api.anthropic.com/v1/messages",
                "{escrito}"
            );
        }
        // Un servidor en la propia máquina, sin esquema, se habla en claro.
        assert_eq!(
            config(Protocolo::OpenAi, "localhost:11434/v1").punto(),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    /// Instrucción y fragmentos van **separados por el protocolo**, y lo que no debe
    /// salir no sale: ni el localizador ni la credencial.
    #[test]
    fn la_instruccion_y_los_fragmentos_van_separados_y_sin_localizador() {
        for protocolo in [Protocolo::OpenAi, Protocolo::Anthropic] {
            let carta = componer(
                &config(protocolo, "servidor.example"),
                &peticion(&["Uno.", "Dos."]),
            )
            .expect("carta");
            let cuerpo: Value = serde_json::from_str(&carta.cuerpo).expect("JSON");
            assert_eq!(cuerpo["model"], "un-modelo");

            let (sistema, usuario) = match protocolo {
                Protocolo::OpenAi => {
                    assert_eq!(cuerpo["messages"][0]["role"], "system");
                    assert_eq!(cuerpo["messages"][1]["role"], "user");
                    (
                        cuerpo["messages"][0]["content"].clone(),
                        cuerpo["messages"][1]["content"].clone(),
                    )
                }
                Protocolo::Anthropic => {
                    assert_eq!(cuerpo["max_tokens"], MAXIMO_DE_SALIDA);
                    assert_eq!(cuerpo["messages"].as_array().map(Vec::len), Some(1));
                    (
                        cuerpo["system"].clone(),
                        cuerpo["messages"][0]["content"].clone(),
                    )
                }
            };
            let sistema = sistema.as_str().expect("texto");
            let usuario = usuario.as_str().expect("texto");
            assert!(sistema.starts_with("Resume."));
            assert!(
                !sistema.contains("Uno."),
                "el documento se coló en la instrucción"
            );
            assert!(usuario.contains("<fragmento id=\"F1\">\nUno.\n</fragmento>"));
            assert!(usuario.contains("<fragmento id=\"F2\">\nDos.\n</fragmento>"));
            assert!(
                !usuario.contains("Resume."),
                "la instrucción se coló en el contenido"
            );
            assert!(
                !carta.cuerpo.contains("capitulo-secreto"),
                "el localizador salió en el envío"
            );
        }
    }

    /// **Nunca el documento entero**: pasado el tope, no se compone nada. Y nunca
    /// embeddings, que no se pueden ni pedir.
    #[test]
    fn no_se_compone_un_envio_con_demasiado_texto_ni_de_embeddings() {
        let largo = "a".repeat(MAXIMO_DE_TEXTO + 1);
        assert_eq!(
            componer(
                &config(Protocolo::OpenAi, "servidor.example"),
                &peticion(&[&largo])
            ),
            Err(NbError::PathNotAllowed)
        );
        let justo = "a".repeat(MAXIMO_DE_TEXTO);
        assert!(componer(
            &config(Protocolo::OpenAi, "servidor.example"),
            &peticion(&[&justo])
        )
        .is_ok());

        let mut vectores = peticion(&["Uno."]);
        vectores.capability = Capability::Embedding;
        assert_eq!(
            componer(&config(Protocolo::Anthropic, "servidor.example"), &vectores),
            Err(NbError::PathNotAllowed)
        );
    }

    /// Se lee el texto de cada protocolo, y lo que no es una respuesta no se toma
    /// por una: JSON roto, sin texto o con la forma del otro protocolo.
    #[test]
    fn solo_se_toma_por_respuesta_lo_que_el_protocolo_dice_que_lo_es() {
        let openai = br#"{"choices":[{"message":{"role":"assistant","content":"listo"}}]}"#;
        let anthropic = br#"{"content":[{"type":"thinking","thinking":"x"},{"type":"text","text":"lis"},{"type":"text","text":"to"}]}"#;
        assert_eq!(
            leer_respuesta(Protocolo::OpenAi, openai).as_deref(),
            Ok("listo")
        );
        assert_eq!(
            leer_respuesta(Protocolo::Anthropic, anthropic).as_deref(),
            Ok("listo")
        );

        assert!(leer_respuesta(Protocolo::OpenAi, anthropic).is_err());
        assert!(leer_respuesta(Protocolo::Anthropic, openai).is_err());
        assert!(leer_respuesta(Protocolo::OpenAi, b"no es json").is_err());
        assert!(leer_respuesta(
            Protocolo::OpenAi,
            br#"{"choices":[{"message":{"content":"  "}}]}"#
        )
        .is_err());
        assert!(leer_respuesta(
            Protocolo::Anthropic,
            br#"{"error":{"type":"x","message":"y"}}"#
        )
        .is_err());
    }

    /// Anthropic exige su versión en cada petición; el compatible con OpenAI no
    /// lleva ninguna cabecera propia. Y ninguna es un secreto.
    #[test]
    fn cada_protocolo_lleva_sus_cabeceras_y_ninguna_es_la_credencial() {
        assert_eq!(
            Protocolo::Anthropic.cabeceras(),
            &[("anthropic-version", VERSION_DE_ANTHROPIC)]
        );
        assert!(Protocolo::OpenAi.cabeceras().is_empty());
        for protocolo in [Protocolo::OpenAi, Protocolo::Anthropic] {
            for (nombre, _) in protocolo.cabeceras() {
                assert!(!nombre.eq_ignore_ascii_case("authorization") && !nombre.contains("key"));
            }
            assert_eq!(Protocolo::desde(protocolo.as_str()), Some(protocolo));
        }
        assert_eq!(Protocolo::desde("none"), None);
    }

    /// Una dirección en claro hacia internet no llega a ser un proveedor: la regla
    /// de la puerta de red, comprobada al configurarlo y no al enviar.
    #[test]
    fn una_direccion_en_claro_hacia_fuera_no_llega_a_ser_proveedor() {
        let db = Database::open_in_memory().expect("base");
        let red = Arc::new(Network::new(Box::new(crate::net::UreqTransport::new())));
        assert!(matches!(
            ExternalProvider::nuevo(
                config(Protocolo::OpenAi, "http://api.example.com"),
                db.clone(),
                Arc::clone(&red)
            ),
            Err(NbError::PathNotAllowed)
        ));
        let proveedor =
            ExternalProvider::nuevo(config(Protocolo::Anthropic, "api.anthropic.com"), db, red)
                .expect("proveedor");
        assert_eq!(proveedor.anfitrion(), "api.anthropic.com");
        assert_eq!(proveedor.placement(), Placement::External);
        assert!(!proveedor.capabilities().contains(&Capability::Embedding));
    }
}
