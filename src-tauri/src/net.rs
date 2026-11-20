//! La única puerta por la que algo sale del dispositivo.
//!
//! Contrato: `docs/audit/privacy.md`.
//!
//! # Por qué existe este módulo
//!
//! `AGENTS.md` declara innegociable que **nada sale del dispositivo sin
//! consentimiento explícito**. Antes ese innegociable no tenía
//! mecanismo: lo cumplía la ausencia de código de red, que no es una garantía
//! sino una casualidad que dura hasta la primera petición.
//!
//! Aquí pasa a ser una garantía comprobable, y con tres piezas:
//!
//! 1. **Una sola puerta.** `scripts/check-network.mjs` rechaza cualquier uso del
//!    cliente HTTP fuera de este archivo. Es el mismo mecanismo que mantiene el
//!    puente IPC dentro de `$core/ipc`: una regla que solo vive en la cabeza de
//!    quien programa no es una regla.
//! 2. **Consentimiento por anfitrión y propósito.** Autorizar un servidor no es
//!    autorizarlo para todo: a un catálogo al que se le permite servir libros no
//!    se le permite por ello recibir el texto de una selección para traducirlo.
//! 3. **Registro de lo que salió**, consultable por el usuario. Sin él,
//!    "observable desde la interfaz" sería una frase.
//!
//! De esas tres piezas sale la mitad estructural del requisito **R9** de la
//! política del registro: una extensión sin capacidad de red no puede hacer
//! telemetría, y no porque nadie la revise, sino porque no hay por dónde.
//!
//! # El interruptor general y el consentimiento no son lo mismo
//!
//! El interruptor (`offline_mode`) es un corte de emergencia: con él puesto no
//! sale nada, ni siquiera lo que ya tenía permiso. El consentimiento es la
//! autorización de verdad, y por eso el interruptor viene **apagado** de
//! fábrica: encenderlo de partida añadiría una segunda puerta que abrir sin
//! proteger de nada, porque sin consentimiento tampoco sale nada.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{NbError, NbResult};
use crate::settings::{self, SettingKey, SettingValue};

/// Cuántas líneas del registro se conservan.
///
/// Acotado porque un registro que crece sin fin acaba siendo un archivo que
/// nadie mira y que ocupa. Lo que importa es lo reciente.
const MAXIMO_REGISTRO: usize = 1_000;

/// Cuánto puede pesar una respuesta que se manda a leer entera.
///
/// Treinta y dos mebibytes. Un audio de un párrafo son unos cientos de
/// kilobytes; lo que pase de aquí no es una respuesta, es un servidor que no
/// piensa parar, y leerla entera sería quedarse sin memoria por obedecer.
const MAXIMO_RESPUESTA: u64 = 32 * 1024 * 1024;

/// Para qué se sale del dispositivo.
///
/// Es una lista cerrada a propósito. Un propósito libre convertiría el
/// consentimiento en un campo de texto que el usuario no puede evaluar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Purpose {
    /// Navegar un catálogo de publicaciones.
    Catalog,
    /// Navegar un catálogo **que solo se sirve en claro**, hacia internet.
    ///
    /// Va aparte de [`Purpose::Catalog`] porque no se consiente lo mismo: por
    /// `https`, quien está en el camino ve a qué servidor se habla; en claro, ve
    /// además qué se busca y qué libro se descarga. Antes esto no se
    /// admitía de ninguna forma, y se decidió ese día que se admita **con su
    /// propio consentimiento**, que dice exactamente eso. Tener permiso para un
    /// anfitrión por `https` no da permiso para hablarle en claro.
    CatalogCleartext,
    /// Obtener portada o metadatos de una obra.
    Metadata,
    /// Consultar una enciclopedia o un traductor.
    Lookup,
    /// Comprobar si hay una versión nueva.
    Update,
    /// Lo que una extensión decida hacer.
    ///
    /// Va aparte y no se reparte entre los otros propósitos porque **el código
    /// de una extensión no es nuestro**: no podemos prometer qué envía. Lo
    /// honesto es que el usuario lo sepa al conceder, y que el consentimiento
    /// sea de esa extensión y no del anfitrión.
    Extension,
    /// Preguntarle al registro qué extensiones ha retirado.
    ///
    /// Va aparte de [`Purpose::Update`] aunque las dos comprueben algo contra
    /// un servidor, porque lo que se envía no es lo mismo y el consentimiento
    /// se da leyendo eso: comprobar si hay versión nueva manda la versión
    /// instalada y la plataforma; esto **no manda nada** sobre lo que hay
    /// instalado.
    Registry,
    /// Sintetizar voz fuera del dispositivo.
    ///
    /// Es el propósito que más manda: el **texto del libro**, párrafo a párrafo,
    /// mientras se escucha. Por eso va aparte de `Lookup`, que envía lo que se
    /// selecciona, y por eso su consentimiento se concede solo.
    Speech,
    /// Descargar un modelo de IA del catálogo.
    ///
    /// Va aparte de [`Purpose::Update`] aunque las dos traigan algo del proyecto,
    /// porque lo que se consiente no es lo mismo: esto trae **cientos de megabytes**
    /// de un tercero, el autor del modelo, y quien lo sirve sabe que se descarga.
    /// Lo que no manda es nada de la biblioteca, y la inferencia después es local.
    Model,
    /// Pedirle algo a un proveedor de IA externo que el usuario configuró
    ///.
    ///
    /// Va aparte de [`Purpose::Speech`] aunque los dos manden texto del libro,
    /// porque no se manda lo mismo ni a quien se manda lo mismo: la voz manda el
    /// párrafo que se escucha a un sintetizador; esto manda **los fragmentos que
    /// una función necesite y lo que el usuario pida** a un modelo de lenguaje,
    /// que puede guardarlos según la política de quien lo sirve. Y es la regla 2 de
    /// `ai/philosophy.md`: salir del dispositivo con la IA es una elección
    /// consciente, nunca el efecto secundario de haber autorizado otra cosa.
    Ai,
}

impl Purpose {
    /// Todos los propósitos.
    ///
    /// Existe para que lo que depende de la lista entera -la etiqueta de cada uno
    /// en la interfaz, su conversión desde el puente- no pueda olvidarse de uno.
    /// Antes pasaba: cuatro de los ocho no tenían etiqueta, y la
    /// sección de Red pintaba la clave cruda, `proposito-speech`. La coincidencia
    /// exhaustiva de [`Purpose::as_str`] impide añadir una variante sin nombrarla,
    /// y la prueba `todo_proposito_se_puede_leer_desde_el_puente_y_tiene_etiqueta`
    /// impide que falte aquí.
    pub const TODOS: [Self; 10] = [
        Self::Catalog,
        Self::CatalogCleartext,
        Self::Metadata,
        Self::Lookup,
        Self::Update,
        Self::Extension,
        Self::Registry,
        Self::Speech,
        Self::Model,
        Self::Ai,
    ];
}

impl Purpose {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Catalog => "catalog",
            Self::CatalogCleartext => "catalog-cleartext",
            Self::Metadata => "metadata",
            Self::Lookup => "lookup",
            Self::Update => "update",
            Self::Registry => "registry",
            Self::Speech => "speech",
            Self::Extension => "extension",
            Self::Model => "model",
            Self::Ai => "ai",
        }
    }

    /// Qué **se envía** con este propósito, en lenguaje llano.
    ///
    /// Vive aquí y no en la interfaz porque es lo que hay que enseñar antes de
    /// pedir el consentimiento, y tenerlo junto al propósito impide que la
    /// explicación se quede vieja cuando el propósito cambie.
    #[must_use]
    pub const fn what_is_sent(self) -> &'static str {
        match self {
            Self::Catalog => "la dirección del catálogo y lo que busques en él",
            Self::CatalogCleartext => {
                "la dirección del catálogo, lo que busques y lo que descargues, sin cifrar: quien comparta tu red o esté en el camino puede verlo"
            }
            Self::Metadata => "el título, el autor o el identificador de la obra",
            Self::Lookup => "el texto que selecciones",
            Self::Update => "la versión instalada y la plataforma",
            // La lista entera se descarga y se compara **aquí**. Decir qué
            // extensiones tienes sería justo lo que no hace falta enviar para
            // que esto funcione, y lo que ningún registro necesita saber.
            Self::Registry => "nada sobre lo que tienes instalado: solo se pide la lista",
            // Sin rodeos, porque es lo que hay que poder sopesar antes de
            // conceder: no es "algún dato", es el libro.
            Self::Speech => "el texto del libro que estés escuchando, párrafo a párrafo",
            // Decir "no lo sabemos" es más útil que inventar una lista. El
            // código de una extensión no lo escribimos nosotros.
            Self::Extension => "lo que la extensión decida enviar, que no podemos acotar",
            // Lo que importa aquí es lo que **no** se envía, y también lo que
            // quien sirve el archivo llega a saber: que se descarga ese modelo.
            Self::Model => {
                "nada de tu biblioteca; solo se pide el archivo del modelo, y quien lo sirve sabe que lo descargas"
            }
            // Lo que se envía y lo que no, y lo que ya no depende de nosotros: qué
            // hace con ello quien lo recibe.
            Self::Ai => {
                "los fragmentos del documento que la función necesite y lo que pidas, nunca el documento entero; quien lo recibe puede guardarlo según su propia política"
            }
        }
    }
}

/// El cuerpo de un envío, con su credencial si la lleva.
///
/// Se agrupa porque son las partes de lo mismo, y porque separadas hacían que la
/// función pasara del número de parámetros que el proyecto admite.
pub struct Envio<'a> {
    pub cuerpo: &'a str,
    pub credencial: Option<&'a str>,
    /// Cabeceras que el protocolo exige además de la credencial: la API
    /// de Anthropic no responde sin `anthropic-version`. **Nunca un secreto**: la
    /// credencial va en su campo, que es el único que el transporte trata como tal.
    pub cabeceras: &'a [(&'a str, &'a str)],
}

/// Quién quiere salir.
///
/// Es la tercera pregunta del consentimiento, junto a "a dónde" y "para qué".
/// Sin ella, conceder a una extensión salida a un anfitrión se la concedería a
/// todas las demás, presentes y futuras. Ver la migración V21.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// La propia aplicación.
    App,
    /// Una extensión, por el identificador que declara su manifiesto.
    Extension(String),
}

impl Origin {
    /// Valor tal y como se almacena. La aplicación es la cadena vacía.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::App => "",
            Self::Extension(id) => id,
        }
    }
}

/// Cómo terminó un intento de salir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "NetworkOutcome"
)]
pub enum Outcome {
    Allowed,
    /// Sin consentimiento para ese anfitrión y propósito.
    Denied,
    /// El interruptor general está puesto.
    Offline,
}

impl Outcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
            Self::Offline => "offline",
        }
    }
}

/// La dirección de un servicio tal y como alguien la escribió, con esquema y sin
/// barra final.
///
/// **Sin esquema, el esquema depende de a dónde apunta**: hacia la propia máquina o
/// la red propia se supone `http`, que es lo que sirven esos servidores; hacia
/// fuera, `https`. Estaba en `speech::Destino::direccion` y sale aquí en
/// Vive aquí porque el proveedor de IA externo se escribe igual, y dos copias de esta
/// regla acabarían diciendo cosas distintas. Quien decide si el resultado vale
/// sigue siendo [`host_de`], que rechaza el claro hacia internet.
#[must_use]
pub fn direccion_escrita(escrito: &str) -> String {
    let escrito = escrito.trim();
    let (esquema, resto) = match escrito.split_once("://") {
        Some((esquema, resto)) if esquema.eq_ignore_ascii_case("http") => ("http", resto),
        Some((esquema, resto)) if esquema.eq_ignore_ascii_case("https") => ("https", resto),
        Some((_, resto)) => ("https", resto),
        None if es_de_la_red_propia(escrito) => ("http", escrito),
        None => ("https", escrito),
    };
    format!("{esquema}://{}", resto.trim_end_matches('/'))
}

/// Una línea del registro de salidas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "NetworkLogEntry"
)]
pub struct LogEntry {
    pub at: String,
    /// Quién salió. Vacío es la propia aplicación.
    pub origin: String,
    pub host: String,
    pub purpose: String,
    pub outcome: Outcome,
    pub status: Option<u16>,
    pub bytes: u64,
}

/// Una respuesta. Deliberadamente pobre: aquí no se interpreta nada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    /// La cabecera `Location`, tal cual venga.
    ///
    /// Se recoge y no se sigue: seguirla es decisión de la puerta, no del
    /// transporte, porque seguirla puede significar salir a un anfitrión que
    /// nadie autorizó. Ver [`Network::get`].
    pub location: Option<String>,
}

/// Lo que sabe hacer un transporte HTTP.
///
/// Existe para poder comprobar la puerta **sin salir a la red**: las pruebas
/// registran un transporte que responde lo que se le diga, y así el criterio de
/// aceptación se demuestra sin depender de que haya internet ni de que un
/// servidor de terceros esté vivo.
pub trait HttpTransport: Send + Sync {
    /// Pide un recurso.
    ///
    /// # Errors
    ///
    /// [`NbError::NetworkFailure`] si no se pudo completar.
    fn get(&self, url: &str) -> NbResult<Response>;

    /// Manda algo y espera la respuesta.
    ///
    /// Hace falta: sintetizar voz fuera del dispositivo es
    /// **enviar** el texto, y eso no cabe en una petición que solo pide. El
    /// cuerpo va en JSON, que es lo que hablan los servicios de síntesis.
    ///
    /// # Errors
    ///
    /// [`NbError::NetworkFailure`] si no se pudo completar.
    ///
    /// La implementación por defecto **dice que no puede**, y así los
    /// transportes que solo saben pedir —los simulados de las pruebas de
    /// catálogos y de consulta— no tienen que escribir un método que nunca se
    /// les llama. Un transporte que sí deba mandar lo implementa, y uno que no
    /// falla diciéndolo en lugar de reventar.
    fn post_json(
        &self,
        _url: &str,
        _cuerpo: &str,
        _credencial: Option<&str>,
    ) -> NbResult<Response> {
        tracing::info!("se pidió enviar por un transporte que solo sabe pedir");
        Err(NbError::NetworkFailure)
    }

    /// Como [`HttpTransport::post_json`], con cabeceras que el protocolo exige
    ///.
    ///
    /// # Errors
    ///
    /// [`NbError::NetworkFailure`] si no se pudo completar.
    ///
    /// La implementación por defecto **dice que no puede**, y no manda sin las
    /// cabeceras: hacerlo daría un rechazo del servidor que parecería un fallo de
    /// la clave, cuando lo que falla es el transporte.
    fn post_json_con_cabeceras(
        &self,
        _url: &str,
        _cuerpo: &str,
        _credencial: Option<&str>,
        _cabeceras: &[(&str, &str)],
    ) -> NbResult<Response> {
        tracing::info!("se pidió enviar con cabeceras por un transporte que no sabe hacerlo");
        Err(NbError::NetworkFailure)
    }

    /// Pide un recurso y lo escribe **según llega**, sin tenerlo entero en
    /// memoria.
    ///
    /// Hace falta. [`HttpTransport::get`] lee la respuesta entera
    /// con un límite y un plazo de un minuto, que es lo correcto para un
    /// catálogo y lo imposible para un modelo de doscientos megabytes: ni cabe
    /// sensatamente en memoria ni llega en un minuto por una red corriente.
    ///
    /// El cuerpo se escribe **solo si la respuesta es 2xx**. Una redirección no
    /// trae nada que guardar, y escribir su cuerpo en el archivo del modelo lo
    /// contaminaría antes de llegar a los bytes de verdad. `seguir` recibe los
    /// bytes escritos hasta ese momento y devuelve `false` para cortar.
    ///
    /// # Errors
    ///
    /// - [`NbError::NetworkFailure`] si no se pudo completar.
    /// - [`NbError::StorageFailure`] si no se pudo escribir.
    /// - [`NbError::Cancelled`] si `seguir` pidió cortar.
    ///
    /// La implementación por defecto dice que no sabe, por el mismo motivo que
    /// [`HttpTransport::post_json`].
    fn descargar(
        &self,
        _url: &str,
        _plazo: std::time::Duration,
        _destino: &mut dyn std::io::Write,
        _seguir: &mut dyn FnMut(u64) -> bool,
    ) -> NbResult<Cabecera> {
        tracing::info!("se pidió descargar por un transporte que no sabe hacerlo");
        Err(NbError::NetworkFailure)
    }
}

/// Lo que se sabe de una descarga sin su cuerpo, que ya se escribió.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cabecera {
    pub status: u16,
    /// La cabecera `Location`, tal cual venga. Ver [`Response::location`].
    pub location: Option<String>,
}

/// Transporte real.
pub struct UreqTransport {
    agente: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    /// Crea el transporte con los límites puestos.
    #[must_use]
    pub fn new() -> Self {
        Self {
            agente: ureq::Agent::new_with_config(
                ureq::Agent::config_builder()
                    // El transporte no sigue redirecciones: las sigue la
                    // puerta, para que el anfitrión de destino vuelva a pasar
                    // por la comprobación de consentimiento. Que las siguiera
                    // aquí sería seguirlas en silencio, y una redirección puede
                    // llevar a un servidor que nadie autorizó. Ver
                    // `redireccion_de` y `Network::get`.
                    .max_redirects(0)
                    /*
                     * Con plazo. Sin él, un servidor que acepta la
                     * conexión y no contesta deja la petición abierta
                     * indefinidamente: medido el 2026-09-12, el catálogo de
                     * Internet Archive no respondía en 25 s, y Descubrir se
                     * quedaba cargando sin decir nada. Diez segundos para
                     * conectar y un minuto para todo: un libro grande por una red
                     * lenta cabe; un servidor muerto, no.
                     */
                    .timeout_connect(Some(std::time::Duration::from_secs(10)))
                    .timeout_global(Some(std::time::Duration::from_secs(60)))
                    .user_agent(concat!("NBBooks/", env!("CARGO_PKG_VERSION")))
                    .build(),
            ),
        }
    }
}

impl HttpTransport for UreqTransport {
    fn post_json(&self, url: &str, cuerpo: &str, credencial: Option<&str>) -> NbResult<Response> {
        self.post_json_con_cabeceras(url, cuerpo, credencial, &[])
    }

    fn post_json_con_cabeceras(
        &self,
        url: &str,
        cuerpo: &str,
        credencial: Option<&str>,
        cabeceras: &[(&str, &str)],
    ) -> NbResult<Response> {
        let mut peticion = self
            .agente
            .post(url)
            .header("content-type", "application/json");
        for (nombre, valor) in cabeceras {
            peticion = peticion.header(*nombre, *valor);
        }
        if let Some(clave) = credencial {
            // La credencial va en la cabecera y **nunca** en la dirección: una
            // dirección acaba en el registro de salidas, y ahí no puede haber un
            // secreto. Por eso `net.rs` la recibe aparte del `url`.
            peticion = peticion.header("authorization", &format!("Bearer {clave}"));
        }

        // Un 401 o un 429 son respuestas, no fallos de red: quien manda tiene que
        // poder decir "la clave no vale" en lugar de "no hay conexión".
        // Con la opción de fábrica de `ureq`, los dos llegaban como el mismo error.
        let mut respuesta = peticion
            .config()
            .http_status_as_error(false)
            .build()
            .send(cuerpo)
            .map_err(|error| {
                tracing::info!(error = %error, "una petición de red no se pudo completar");
                NbError::NetworkFailure
            })?;

        let status = respuesta.status().as_u16();
        let content_type = respuesta
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = respuesta
            .body_mut()
            .with_config()
            .limit(MAXIMO_RESPUESTA)
            .read_to_vec()
            .map_err(|_| NbError::NetworkFailure)?;

        Ok(Response {
            status,
            content_type,
            body,
            location: None,
        })
    }

    fn get(&self, url: &str) -> NbResult<Response> {
        // Un 4xx o un 5xx es una respuesta, no un fallo de red: con la
        // opción de fábrica de ureq llegaban como error, así que un artículo que
        // no existe (404) se enseñaba como "No respondió" y un catálogo que pide
        // usuario (401) como "No se pudo alcanzar", aunque quien llama ya sabía
        // distinguir los dos. Quien llama mira el estado.
        let mut respuesta = self
            .agente
            .get(url)
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .map_err(|error| {
                // El error se registra redactado: una URL puede llevar dentro el
                // texto que el usuario seleccionó.
                tracing::info!(error = %error, "una petición de red no se pudo completar");
                NbError::NetworkFailure
            })?;

        let status = respuesta.status().as_u16();
        let cabecera = |nombre: &str| {
            respuesta
                .headers()
                .get(nombre)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let content_type = cabecera("content-type");
        let location = cabecera("location");

        let body = respuesta
            .body_mut()
            .read_to_vec()
            .map_err(|_| NbError::NetworkFailure)?;

        Ok(Response {
            status,
            content_type,
            body,
            location,
        })
    }

    fn descargar(
        &self,
        url: &str,
        plazo: std::time::Duration,
        destino: &mut dyn std::io::Write,
        seguir: &mut dyn FnMut(u64) -> bool,
    ) -> NbResult<Cabecera> {
        use std::io::Read;

        /*
         * El plazo del agente es de un minuto, y aquí se sustituye por el de esta
         * descarga. **Lo que ureq no ofrece es un plazo de inactividad**: su plazo
         * es para la descarga entera, así que una conexión que se queda muda solo
         * se detecta al agotarse el total. Por eso quien llama lo calcula con el
         * tamaño esperado y un ritmo lento, y por eso cancelar no puede
         * desbloquear una lectura detenida: se comprueba entre bloques. Se dice
         * aquí en lugar de prometer que "cancelar corta al instante".
         */
        let mut respuesta = self
            .agente
            .get(url)
            .config()
            .timeout_global(Some(plazo))
            .build()
            .call()
            .map_err(|error| {
                tracing::info!(error = %error, "una descarga no se pudo completar");
                NbError::NetworkFailure
            })?;

        let status = respuesta.status().as_u16();
        let location = respuesta
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        if !(200..300).contains(&status) {
            return Ok(Cabecera { status, location });
        }

        let mut lector = respuesta.body_mut().as_reader();
        // Bloques de 64 KiB: lo bastante grandes para no hacer una llamada al
        // sistema por paquete, y lo bastante pequeños para que cancelar se
        // atienda enseguida cuando llegan datos.
        let mut bloque = vec![0_u8; 64 * 1024];
        let mut escritos: u64 = 0;
        loop {
            let leidos = lector.read(&mut bloque).map_err(|_| {
                tracing::info!("una descarga se cortó a medias");
                NbError::NetworkFailure
            })?;
            if leidos == 0 {
                break;
            }
            destino
                .write_all(&bloque[..leidos])
                .map_err(|_| NbError::StorageFailure)?;
            escritos += leidos as u64;
            if !seguir(escritos) {
                return Err(NbError::Cancelled);
            }
        }

        Ok(Cabecera { status, location })
    }
}

/// La puerta.
pub struct Network {
    transporte: Box<dyn HttpTransport>,
}

impl Network {
    #[must_use]
    pub fn new(transporte: Box<dyn HttpTransport>) -> Self {
        Self { transporte }
    }

    /// Manda algo a un servidor, si se puede.
    ///
    /// Las mismas tres comprobaciones que [`Network::get`] —interruptor,
    /// consentimiento, registro—, porque son de la puerta y no del verbo. Lo que
    /// cambia es que aquí **sale contenido**, y por eso el propósito importa
    /// más: `what_is_sent` es lo que hay que poder leer antes de conceder.
    ///
    /// No sigue redirecciones. Una redirección de un `POST` es ambigua —según el
    /// código, unos clientes reenvían el cuerpo y otros lo convierten en `GET`—
    /// y reenviar a ciegas el texto de un libro a un servidor distinto del
    /// autorizado es justo lo que la puerta existe para impedir.
    ///
    /// # Errors
    ///
    /// - [`NbError::NetworkDisabled`] si el modo sin red está puesto.
    /// - [`NbError::ConsentRequired`] si no hay consentimiento para ese
    ///   anfitrión y propósito.
    /// - [`NbError::NetworkFailure`] si la petición no se pudo completar.
    pub fn post_json(
        &self,
        conn: &Connection,
        url: &str,
        purpose: Purpose,
        cuerpo: &str,
        credencial: Option<&str>,
    ) -> NbResult<Response> {
        self.post_json_from(
            conn,
            &Origin::App,
            url,
            purpose,
            Envio {
                cuerpo,
                credencial,
                cabeceras: &[],
            },
        )
    }

    /// Como [`Network::post_json`], diciendo quién sale.
    ///
    /// # Errors
    ///
    /// Las mismas que [`Network::post_json`].
    pub fn post_json_from(
        &self,
        conn: &Connection,
        origin: &Origin,
        url: &str,
        purpose: Purpose,
        envio: Envio<'_>,
    ) -> NbResult<Response> {
        let Envio {
            cuerpo,
            credencial,
            cabeceras,
        } = envio;
        let host = host_de(url)?;

        if is_offline(conn)? {
            registrar(
                conn,
                Anotacion {
                    origin,
                    host: &host,
                    purpose,
                    outcome: Outcome::Offline,
                    status: None,
                    bytes: 0,
                },
            )?;
            tracing::info!(purpose = purpose.as_str(), "modo sin red: no se sale");
            return Err(NbError::NetworkDisabled);
        }

        if !has_consent_from(conn, origin, &host, purpose)? {
            registrar(
                conn,
                Anotacion {
                    origin,
                    host: &host,
                    purpose,
                    outcome: Outcome::Denied,
                    status: None,
                    bytes: 0,
                },
            )?;
            tracing::info!(
                purpose = purpose.as_str(),
                "se rechazó una salida sin consentimiento"
            );
            return Err(NbError::ConsentRequired);
        }

        let respuesta = if cabeceras.is_empty() {
            self.transporte.post_json(url, cuerpo, credencial)
        } else {
            self.transporte
                .post_json_con_cabeceras(url, cuerpo, credencial, cabeceras)
        };
        match &respuesta {
            Ok(r) => registrar(
                conn,
                Anotacion {
                    origin,
                    host: &host,
                    purpose,
                    outcome: Outcome::Allowed,
                    status: Some(r.status),
                    bytes: r.body.len() as u64,
                },
            )?,
            Err(_) => registrar(
                conn,
                Anotacion {
                    origin,
                    host: &host,
                    purpose,
                    outcome: Outcome::Allowed,
                    status: None,
                    bytes: 0,
                },
            )?,
        }
        respuesta
    }

    /// Pide un recurso y, si falta un permiso, **dice cuál**.
    ///
    /// Es [`Network::get_from`] sin convertir la falta de permiso en un error
    /// mudo. Existe por un defecto que se encontró el 2026-09-12: casi todo
    /// catálogo redirige a otro anfitrión -`epublibre.gratis` a
    /// `ww2.epublibre.app`-, la puerta pedía permiso para el segundo, y la
    /// interfaz, que solo recibía `CONSENT_REQUIRED`, volvía a autorizar el
    /// primero. Autorizado y sin sincronizar, en bucle. El registro de salidas lo
    /// enseñaba: diecisiete autorizaciones y dieciocho rechazos, todos del otro.
    ///
    /// # Errors
    ///
    /// Los de [`Network::get`], salvo la falta de permiso, que vuelve como
    /// [`Intento::FaltaPermiso`].
    pub fn intentar(&self, conn: &Connection, url: &str, purpose: Purpose) -> NbResult<Intento> {
        match self.salir(conn, &Origin::App, url, purpose) {
            Err(Salida::FaltaPermiso { host, purpose }) => Ok(Intento::FaltaPermiso {
                host,
                cleartext: purpose == Purpose::CatalogCleartext,
            }),
            Err(Salida::Error(error)) => Err(error),
            Ok(respuesta) => Ok(Intento::Respondio(respuesta)),
        }
    }

    /// Pide un recurso, si se puede.
    ///
    /// Comprueba el interruptor, después el consentimiento, y solo entonces
    /// sale. **Cada intento queda registrado**, salga o no: un intento denegado
    /// es justo lo que el usuario querría ver.
    ///
    /// # Errors
    ///
    /// - [`NbError::NetworkDisabled`] si el modo sin red está puesto.
    /// - [`NbError::ConsentRequired`] si no hay consentimiento para ese
    ///   anfitrión y propósito.
    /// - [`NbError::NetworkFailure`] si la petición no se pudo completar.
    pub fn get(&self, conn: &Connection, url: &str, purpose: Purpose) -> NbResult<Response> {
        self.get_from(conn, &Origin::App, url, purpose)
    }

    /// Como [`Network::get`], diciendo quién sale.
    ///
    /// # Errors
    ///
    /// Las mismas que [`Network::get`].
    pub fn get_from(
        &self,
        conn: &Connection,
        origin: &Origin,
        url: &str,
        purpose: Purpose,
    ) -> NbResult<Response> {
        self.salir(conn, origin, url, purpose)
            .map_err(|salida| match salida {
                Salida::FaltaPermiso { .. } => NbError::ConsentRequired,
                Salida::Error(error) => error,
            })
    }

    fn salir(
        &self,
        conn: &Connection,
        origin: &Origin,
        url: &str,
        purpose: Purpose,
    ) -> Result<Response, Salida> {
        let mut destino = url.to_owned();

        for _ in 0..=MAXIMO_SALTOS {
            let (host, purpose) = destino_de(&destino, purpose)?;

            if is_offline(conn)? {
                registrar(
                    conn,
                    Anotacion {
                        origin,
                        host: &host,
                        purpose,
                        outcome: Outcome::Offline,
                        status: None,
                        bytes: 0,
                    },
                )?;
                tracing::info!(purpose = purpose.as_str(), "modo sin red: no se sale");
                return Err(NbError::NetworkDisabled.into());
            }

            if !has_consent_from(conn, origin, &host, purpose)? {
                registrar(
                    conn,
                    Anotacion {
                        origin,
                        host: &host,
                        purpose,
                        outcome: Outcome::Denied,
                        status: None,
                        bytes: 0,
                    },
                )?;
                tracing::info!(
                    purpose = purpose.as_str(),
                    "se rechazó una salida sin consentimiento"
                );
                return Err(Salida::FaltaPermiso { host, purpose });
            }

            let respuesta = self.transporte.get(&destino);
            match &respuesta {
                Ok(r) => registrar(
                    conn,
                    Anotacion {
                        origin,
                        host: &host,
                        purpose,
                        outcome: Outcome::Allowed,
                        status: Some(r.status),
                        bytes: r.body.len() as u64,
                    },
                )?,
                // Un intento fallido también salió del dispositivo: la conexión
                // se abrió. Ocultarlo del registro sería mentir sobre lo que
                // ocurrió.
                Err(_) => registrar(
                    conn,
                    Anotacion {
                        origin,
                        host: &host,
                        purpose,
                        outcome: Outcome::Allowed,
                        status: None,
                        bytes: 0,
                    },
                )?,
            }

            let respuesta = respuesta?;
            let Some(siguiente) = redireccion_de(&respuesta, &destino) else {
                return Ok(respuesta);
            };

            // La vuelta del bucle comprueba el consentimiento del anfitrión
            // nuevo, que es lo que hace que seguir una redirección no salte la
            // puerta: si el destino es otro servidor, hace falta autorizarlo.
            destino = siguiente;
        }

        tracing::info!("una redirección encadenó más saltos de los admitidos");
        Err(NbError::NetworkFailure.into())
    }
}

impl Network {
    /// Descarga un recurso por partes, si se puede.
    ///
    /// Las mismas comprobaciones que [`Network::get`] y en el mismo orden —el
    /// interruptor, después el consentimiento—, repetidas en cada salto de una
    /// redirección, y cada salto anotado en el registro con los bytes que movió.
    ///
    /// # Por qué recibe la base y no una conexión
    ///
    /// Porque una descarga dura minutos y el pool tiene cuatro conexiones.
    /// [`Network::get`] recibe una conexión y la usa antes y después de salir, y
    /// en una petición de un segundo eso no importa. Aquí la conexión se pediría
    /// antes de empezar y se soltaría al terminar, **reteniendo una de las cuatro
    /// durante toda la transferencia**, que es media biblioteca sin poder guardar
    /// el progreso de lectura si coinciden dos. Es la lección del 2026-09-12: un
    /// trabajo de fondo que retiene un recurso compartido no es trabajo de fondo.
    /// Así que la conexión se pide para comprobar y para anotar, y durante la
    /// transferencia no se tiene ninguna.
    ///
    /// # Errors
    ///
    /// - [`NbError::NetworkDisabled`] si el modo sin red está puesto.
    /// - [`NbError::NetworkFailure`] si no se pudo completar, o si terminó con un
    ///   estado que no es ni éxito ni redirección.
    /// - [`NbError::StorageFailure`] si no se pudo escribir o anotar.
    /// - [`NbError::Cancelled`] si `seguir` pidió cortar.
    ///
    /// Que falte el permiso **no es un error**: es [`Descarga::FaltaPermiso`], con
    /// el anfitrión que lo necesita, que puede no ser el pedido sino el de la red
    /// de distribución a la que se redirigió. Es lo que permite a la interfaz
    /// pedir el permiso de ese anfitrión y no uno genérico.
    pub fn descargar(
        &self,
        db: &crate::db::Database,
        encargo: &Encargo<'_>,
        destino: &mut dyn std::io::Write,
        seguir: &mut dyn FnMut(u64) -> bool,
    ) -> NbResult<Descarga> {
        let Encargo {
            url,
            purpose,
            plazo,
        } = *encargo;
        let origin = Origin::App;
        let mut direccion = url.to_owned();

        for _ in 0..=MAXIMO_SALTOS {
            let (host, purpose) = destino_de(&direccion, purpose)?;

            {
                let conn = db.connection()?;
                if is_offline(&conn)? {
                    registrar(
                        &conn,
                        Anotacion {
                            origin: &origin,
                            host: &host,
                            purpose,
                            outcome: Outcome::Offline,
                            status: None,
                            bytes: 0,
                        },
                    )?;
                    tracing::info!(purpose = purpose.as_str(), "modo sin red: no se descarga");
                    return Err(NbError::NetworkDisabled);
                }
                if !has_consent_from(&conn, &origin, &host, purpose)? {
                    registrar(
                        &conn,
                        Anotacion {
                            origin: &origin,
                            host: &host,
                            purpose,
                            outcome: Outcome::Denied,
                            status: None,
                            bytes: 0,
                        },
                    )?;
                    tracing::info!(
                        purpose = purpose.as_str(),
                        "se rechazó una descarga sin consentimiento"
                    );
                    return Ok(Descarga::FaltaPermiso { host });
                }
            }

            // Sin conexión en la mano: ver la documentación de esta función.
            let mut movidos: u64 = 0;
            let resultado = self
                .transporte
                .descargar(&direccion, plazo, destino, &mut |bytes| {
                    movidos = bytes;
                    seguir(bytes)
                });

            {
                let conn = db.connection()?;
                // Un intento fallido o cortado también salió del dispositivo, con
                // lo que moviera hasta ese momento. Ocultarlo del registro sería
                // mentir sobre lo que ocurrió.
                registrar(
                    &conn,
                    Anotacion {
                        origin: &origin,
                        host: &host,
                        purpose,
                        outcome: Outcome::Allowed,
                        status: resultado.as_ref().ok().map(|c| c.status),
                        bytes: movidos,
                    },
                )?;
            }

            let cabecera = resultado?;
            if (200..300).contains(&cabecera.status) {
                return Ok(Descarga::Completa { bytes: movidos });
            }

            let como_respuesta = Response {
                status: cabecera.status,
                location: cabecera.location,
                ..Response::default()
            };
            let Some(siguiente) = redireccion_de(&como_respuesta, &direccion) else {
                tracing::info!(
                    status = cabecera.status,
                    "una descarga terminó sin éxito ni redirección"
                );
                return Err(NbError::NetworkFailure);
            };
            direccion = siguiente;
        }

        tracing::info!("una descarga encadenó más redirecciones de las admitidas");
        Err(NbError::NetworkFailure)
    }
}

/// Qué descargar, para qué y con cuánto plazo.
///
/// Se agrupa por el mismo motivo que [`Envio`]: son tres datos de lo mismo, y
/// sueltos la función pasaba del número de parámetros que el proyecto admite.
#[derive(Debug, Clone, Copy)]
pub struct Encargo<'a> {
    pub url: &'a str,
    pub purpose: Purpose,
    pub plazo: std::time::Duration,
}

/// Cómo terminó una descarga que no falló.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Descarga {
    /// Llegó entera, y estos son los bytes que se escribieron.
    Completa { bytes: u64 },
    /// Falta el permiso de este anfitrión, que puede ser el de una redirección.
    FaltaPermiso { host: String },
}

/// Cómo terminó intentar salir, cuando no hubo un error de otra clase.
#[derive(Debug)]
pub enum Intento {
    Respondio(Response),
    /// Falta el permiso de este anfitrión, que puede no ser el pedido: es el de
    /// la redirección que se estaba siguiendo.
    FaltaPermiso {
        host: String,
        /// Si lo que falta es el permiso de hablarle **en claro**.
        cleartext: bool,
    },
}

/// Lo que puede cortar una salida por dentro: un error, o un permiso que falta
/// y del que hay que saber el anfitrión.
enum Salida {
    FaltaPermiso { host: String, purpose: Purpose },
    Error(NbError),
}

impl From<NbError> for Salida {
    fn from(error: NbError) -> Self {
        Self::Error(error)
    }
}

/// El anfitrión de una dirección y el propósito con el que hay que consentirla.
///
/// Es [`host_de`] con una sola excepción, y es de catálogos: **un catálogo en
/// claro hacia internet se admite**, con su propio propósito, que es otro
/// consentimiento. Para todo lo demás -la voz, las consultas, las
/// extensiones- el claro hacia fuera sigue sin admitirse, porque lo que saldría
/// es el texto del libro o lo que se seleccionó.
fn destino_de(url: &str, purpose: Purpose) -> NbResult<(String, Purpose)> {
    if purpose == Purpose::Catalog {
        if let Some(anfitrion) = catalogo_en_claro(url) {
            return Ok((anfitrion, Purpose::CatalogCleartext));
        }
    }
    Ok((host_de(url)?, purpose))
}

/// El anfitrión de un catálogo en claro hacia internet, si la dirección lo es.
///
/// Nulo para `https`, para la red propia -que ya se admitía en claro sin
/// consentimiento aparte- y para lo que no tenga anfitrión.
#[must_use]
pub fn catalogo_en_claro(url: &str) -> Option<String> {
    let resto = url
        .get(..7)
        .filter(|esquema| esquema.eq_ignore_ascii_case("http://"))
        .map(|_| &url[7..])?;
    if es_de_la_red_propia(resto) {
        return None;
    }
    let host = anfitrion_en(resto);
    (!host.is_empty() && !host.contains(' ')).then_some(host)
}

/// Valida la dirección de un catálogo por el mismo camino que la puerta.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si no es `https`, `http` o no tiene anfitrión.
pub fn anfitrion_de_catalogo(url: &str) -> NbResult<(String, bool)> {
    let (host, purpose) = destino_de(url.trim(), Purpose::Catalog)?;
    Ok((host, purpose == Purpose::CatalogCleartext))
}

/// Anfitrión de una URL, sin puerto, sin credenciales y en minúsculas.
///
/// Se interpreta a mano y no con un analizador de URL: son diez líneas, y la
/// política de dependencias dice que no se añade un paquete por menos de cien.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si no es una URL con anfitrión, o si es `http`
/// hacia fuera de la propia máquina.
///
/// # `https`, salvo dentro de la propia red
///
/// `http` en claro deja el contenido y el destino a la vista de cualquiera por
/// donde pase, así que **hacia internet no se admite y no se va a admitir**.
///
/// Se admite hacia la propia máquina y hacia **direcciones de red privada**
/// (RFC 1918, enlace local y las equivalentes de IPv6). El motivo es que ahí
/// vive el caso real: un servidor de síntesis en una máquina virtual, en otro
/// equipo de la casa o en la red de una institución. Exigirle `https` significa
/// exigir que se monte una autoridad certificadora propia y se instale su raíz
/// en el sistema, para hablar con una máquina que está a un metro. Nadie lo
/// hace; lo que se consigue es que la función no se use.
///
/// **La diferencia se dice, no se esconde.** Hacia `localhost` no hay red que
/// escuche. Hacia `192.168.x.x` sí la hay: el texto del libro viaja sin cifrar
/// por esa red, y quien comparta la red puede leerlo. Por eso Ajustes lo
/// advierte al escribir la dirección, en lugar de que la puerta lo permita en
/// silencio.
///
/// Solo se admite por **dirección numérica**, nunca por nombre. Un nombre lo
/// resuelve el DNS, y lo que hoy resuelve a una dirección privada mañana
/// resuelve a otra: aceptar `mi-servidor.example` en claro sería dejar que
/// quien controle esa zona decida a dónde va el texto.
pub fn host_de(url: &str) -> NbResult<String> {
    let resto = url
        .strip_prefix("https://")
        .or_else(|| {
            url.strip_prefix("http://")
                .filter(|resto| es_de_la_red_propia(resto))
        })
        .ok_or_else(|| {
            tracing::info!("se rechazó una dirección que no es https ni de la red propia");
            NbError::PathNotAllowed
        })?;

    let host = anfitrion_en(resto);

    if host.is_empty() || host.contains(' ') {
        tracing::info!("se rechazó una dirección sin anfitrión");
        return Err(NbError::PathNotAllowed);
    }

    Ok(host)
}

/// El anfitrión que hay en lo que sigue al esquema, sin puerto, sin
/// credenciales y en minúsculas.
///
/// El puerto de una dirección IPv6 **no** es lo que sigue al último `:`, que ahí
/// es parte de la propia dirección: es lo que sigue al corchete de cierre. Sin
/// esa distinción, `[::1]` sin puerto se partía por la mitad y no coincidía con
/// nada, así que la excepción de claro hacia uno mismo nunca se aplicaba por
/// IPv6 aunque la lista la nombrara.
fn anfitrion_en(resto: &str) -> String {
    let autoridad = resto
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        // Credenciales incrustadas: no se admiten, y tampoco se registran.
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .trim();

    let host = match autoridad
        .strip_prefix('[')
        .and_then(|_| autoridad.find(']'))
    {
        Some(cierre) => &autoridad[1..cierre],
        None => autoridad
            .rsplit_once(':')
            .map_or(autoridad, |(anfitrion, _)| anfitrion),
    };
    host.to_ascii_lowercase()
}

/// Si lo que sigue al esquema apunta a la propia máquina o a la propia red.
///
/// Se compara el anfitrión entero y no un prefijo: `localhost.example.com` no es
/// `localhost`, y aceptarlo por empezar igual sería abrir en claro hacia un
/// dominio de cualquiera.
///
/// Y solo por dirección numérica: un nombre lo resuelve el DNS, y quien controle
/// esa zona decidiría a dónde va el texto. Ver la nota de [`host_de`].
pub(crate) fn es_de_la_red_propia(resto: &str) -> bool {
    let host = anfitrion_en(resto);
    if host == "localhost" {
        return true;
    }

    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        // `is_unique_local` y `is_unicast_link_local` siguen sin estabilizarse,
        // así que se miran los bits: `fc00::/7` son las direcciones locales
        // únicas y `fe80::/10` las de enlace local.
        Ok(std::net::IpAddr::V6(v6)) => {
            let primero = v6.segments()[0];
            v6.is_loopback() || (primero & 0xfe00) == 0xfc00 || (primero & 0xffc0) == 0xfe80
        }
        Err(_) => false,
    }
}

/// Cuántas redirecciones se siguen antes de rendirse.
///
/// Cinco es lo que usan los navegadores para lo mismo. El límite existe porque
/// una cadena sin fin es un servidor que te tiene dando vueltas, y cada vuelta
/// es una salida del dispositivo.
const MAXIMO_SALTOS: u8 = 5;

/// La dirección a la que redirige una respuesta, si redirige.
///
/// # Por qué se sigue una redirección, habiendo dicho que no se seguían
///
/// Porque no seguirlas no protegía de nada: protegía **de funcionar**. Casi
/// todo catálogo real redirige —de `http` a `https`, de `www` al dominio pelado,
/// de una ruta sin barra final a la que la lleva— y la respuesta se convertía en
/// `NetworkFailure`, es decir, en "no se pudo alcanzar el catálogo" cuando el
/// catálogo había contestado perfectamente. Un mensaje falso sobre el que además
/// no se puede actuar.
///
/// Lo que protege de verdad es **volver a pasar por la puerta**: quien sigue la
/// redirección es [`Network::get`], y lo hace dando otra vuelta a su bucle, así
/// que el anfitrión nuevo pasa por la misma comprobación de consentimiento que
/// el primero. Redirigir a otro servidor no autoriza a ese servidor; lo deja
/// anotado como denegado, con su nombre, en el registro que el usuario consulta.
///
/// Solo se admite `https` absoluto o una ruta del mismo anfitrión: una
/// redirección a `http` es una degradación, y [`host_de`] la rechaza.
fn redireccion_de(respuesta: &Response, actual: &str) -> Option<String> {
    if !(300..400).contains(&respuesta.status) {
        return None;
    }

    let destino = respuesta.location.as_deref()?.trim();
    if destino.is_empty() {
        return None;
    }

    let en_claro = actual
        .get(..7)
        .is_some_and(|esquema| esquema.eq_ignore_ascii_case("http://"));

    if destino.starts_with("https://") {
        return Some(destino.to_owned());
    }
    // De claro a claro, sí: es el mismo trato que ya se había consentido, y la
    // vuelta del bucle vuelve a mirar el permiso del anfitrión nuevo. **De
    // cifrado a claro, nunca**: sería una degradación que nadie pidió.
    if en_claro
        && destino
            .get(..7)
            .is_some_and(|esquema| esquema.eq_ignore_ascii_case("http://"))
    {
        return Some(destino.to_owned());
    }

    // Relativa a la raíz del mismo anfitrión. Se compone con el esquema y el
    // anfitrión actuales y nada más: quedarse con la ruta de partida obligaría a
    // resolver `..` y `.`, y equivocarse ahí es pedir un recurso que no es.
    let (esquema, raiz) = if en_claro {
        ("http", &actual[7..])
    } else {
        ("https", actual.strip_prefix("https://")?)
    };
    let anfitrion = raiz.split(['/', '?', '#']).next()?;
    if let Some(ruta) = destino.strip_prefix('/') {
        return Some(format!("{esquema}://{anfitrion}/{ruta}"));
    }

    tracing::info!("no se siguió una redirección que no era absoluta ni de raíz");
    None
}

/// Indica si el modo sin red está puesto.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer la preferencia.
pub fn is_offline(conn: &Connection) -> NbResult<bool> {
    Ok(matches!(
        settings::get(conn, SettingKey::OfflineMode)?,
        SettingValue::Flag(true)
    ))
}

/// Concede consentimiento para un anfitrión y un propósito.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn grant(conn: &Connection, host: &str, purpose: Purpose) -> NbResult<()> {
    grant_from(conn, &Origin::App, host, purpose)
}

/// Concede consentimiento a un origen concreto.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn grant_from(
    conn: &Connection,
    origin: &Origin,
    host: &str,
    purpose: Purpose,
) -> NbResult<()> {
    conn.execute(
        "INSERT INTO network_consents (origin, host, purpose) VALUES (?1, ?2, ?3)
         ON CONFLICT DO NOTHING",
        [
            origin.as_str(),
            &host.to_ascii_lowercase(),
            purpose.as_str(),
        ],
    )
    .map(|_| ())
    .map_err(fallo)
}

/// Revoca un consentimiento. Que no existiera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn revoke(conn: &Connection, host: &str, purpose: Purpose) -> NbResult<()> {
    revoke_from(conn, &Origin::App, host, purpose)
}

/// Revoca el consentimiento de un origen concreto.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn revoke_from(
    conn: &Connection,
    origin: &Origin,
    host: &str,
    purpose: Purpose,
) -> NbResult<()> {
    conn.execute(
        "DELETE FROM network_consents WHERE origin = ?1 AND host = ?2 AND purpose = ?3",
        [
            origin.as_str(),
            &host.to_ascii_lowercase(),
            purpose.as_str(),
        ],
    )
    .map(|_| ())
    .map_err(fallo)
}

/// Revoca **todo** lo concedido a un origen.
///
/// Es lo que hace desinstalar una extensión: sus permisos no pueden
/// sobrevivirla, porque entonces reinstalarla más tarde saldría a la red sin
/// volver a preguntar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn revoke_all_from(conn: &Connection, origin: &Origin) -> NbResult<usize> {
    conn.execute(
        "DELETE FROM network_consents WHERE origin = ?1",
        [origin.as_str()],
    )
    .map_err(fallo)
}

/// Un consentimiento concedido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Consent {
    /// Quién lo tiene. Vacío es la propia aplicación.
    pub origin: String,
    pub host: String,
    /// Para qué. Ver [`Purpose`].
    pub purpose: String,
    pub granted_at: String,
}

/// Todos los consentimientos concedidos, en orden estable.
///
/// Existe para que el usuario pueda **ver lo que ha autorizado**, que es la
/// mitad del innegociable de red que no se cumple con impedir la salida: un
/// permiso concedido hace seis meses y olvidado es indistinguible de uno que
/// nunca se pidió si no hay dónde mirarlo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn consents(conn: &Connection) -> NbResult<Vec<Consent>> {
    let mut stmt = conn
        .prepare(
            "SELECT origin, host, purpose, granted_at FROM network_consents
              ORDER BY origin, host, purpose",
        )
        .map_err(fallo)?;

    stmt.query_map([], |row| {
        Ok(Consent {
            origin: row.get(0)?,
            host: row.get(1)?,
            purpose: row.get(2)?,
            granted_at: row.get(3)?,
        })
    })
    .and_then(Iterator::collect)
    .map_err(fallo)
}

/// Convierte el propósito tal y como viaja por el puente.
///
/// Se interpreta en Rust y no en la interfaz porque el webview es zona no
/// confiable: un propósito inventado no puede convertirse en un consentimiento
/// que la puerta luego acepte.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si no es uno de los que existen.
///
/// Se busca en [`Purpose::TODOS`] en lugar de repetir la lista aquí. Repetida,
/// se quedó vieja: `registry` no se podía leer desde el puente, así que su
/// permiso no se podía revocar desde Ajustes.
pub fn purpose_de(bruto: &str) -> NbResult<Purpose> {
    Purpose::TODOS
        .into_iter()
        .find(|p| p.as_str() == bruto)
        .ok_or_else(|| {
            tracing::info!("se pidió un propósito de red que no existe");
            NbError::PathNotAllowed
        })
}

/// Indica si hay consentimiento.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn has_consent(conn: &Connection, host: &str, purpose: Purpose) -> NbResult<bool> {
    has_consent_from(conn, &Origin::App, host, purpose)
}

/// Indica si un origen concreto tiene consentimiento.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn has_consent_from(
    conn: &Connection,
    origin: &Origin,
    host: &str,
    purpose: Purpose,
) -> NbResult<bool> {
    conn.query_row(
        "SELECT 1 FROM network_consents WHERE origin = ?1 AND host = ?2 AND purpose = ?3",
        [
            origin.as_str(),
            &host.to_ascii_lowercase(),
            purpose.as_str(),
        ],
        |_| Ok(()),
    )
    .optional()
    .map(|encontrado| encontrado.is_some())
    .map_err(fallo)
}

/// Todo lo que salió, de lo más reciente a lo más antiguo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn log(conn: &Connection, limite: usize) -> NbResult<Vec<LogEntry>> {
    let mut stmt = conn
        .prepare(
            "SELECT at, origin, host, purpose, outcome, status, bytes FROM network_log
              ORDER BY id DESC LIMIT ?1",
        )
        .map_err(fallo)?;

    let limite = i64::try_from(limite).unwrap_or(i64::MAX);
    stmt.query_map([limite], |row| {
        let outcome: String = row.get(4)?;
        Ok(LogEntry {
            at: row.get(0)?,
            origin: row.get(1)?,
            host: row.get(2)?,
            purpose: row.get(3)?,
            outcome: match outcome.as_str() {
                "denied" => Outcome::Denied,
                "offline" => Outcome::Offline,
                _ => Outcome::Allowed,
            },
            status: row
                .get::<_, Option<i64>>(5)?
                .and_then(|s| u16::try_from(s).ok()),
            bytes: row.get::<_, i64>(6)?.unsigned_abs(),
        })
    })
    .and_then(Iterator::collect)
    .map_err(fallo)
}

/// Una línea del registro, antes de escribirse.
///
/// Se agrupa en un tipo y no se pasa suelta porque son seis datos del mismo
/// hecho: quién salió, a dónde, para qué, cómo terminó y cuánto movió.
struct Anotacion<'a> {
    origin: &'a Origin,
    host: &'a str,
    purpose: Purpose,
    outcome: Outcome,
    status: Option<u16>,
    bytes: u64,
}

fn registrar(conn: &Connection, anotacion: Anotacion) -> NbResult<()> {
    let Anotacion {
        origin,
        host,
        purpose,
        outcome,
        status,
        bytes,
    } = anotacion;
    conn.execute(
        "INSERT INTO network_log (origin, host, purpose, outcome, status, bytes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            origin.as_str(),
            host,
            purpose.as_str(),
            outcome.as_str(),
            status.map(i64::from),
            i64::try_from(bytes).unwrap_or(i64::MAX)
        ],
    )
    .map_err(fallo)?;

    // Se poda aquí y no en un trabajo de fondo: una tabla que solo crece es una
    // deuda que nadie recuerda hasta que ocupa.
    conn.execute(
        "DELETE FROM network_log WHERE id <= (
             SELECT max(id) - ?1 FROM network_log
         )",
        [i64::try_from(MAXIMO_REGISTRO).unwrap_or(i64::MAX)],
    )
    .map_err(fallo)?;

    Ok(())
}

fn fallo(error: rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo al registrar una salida de red");
    NbError::StorageFailure
}
