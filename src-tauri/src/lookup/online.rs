//! Los proveedores de consulta que salen del dispositivo.
//!
//! # Por qué entran por el mismo contrato que el diccionario
//!
//! Porque para quien lee son la misma operación: seleccionar algo y entender
//! qué es. El contrato se escribió pensando exactamente en esto, y la
//! prueba de que servía es que el panel no cambia ni una línea al
//! añadirlos: aparecen como dos respuestas más, con su procedencia y su clase.
//!
//! # Lo que estos dos no deciden
//!
//! **Si pueden salir.** Eso lo decide `net::Network`, que comprueba el
//! interruptor general y el consentimiento por anfitrión y propósito antes de
//! abrir nada. Un proveedor de aquí no tiene forma de saltárselo: no tiene
//! cliente HTTP propio, tiene la puerta.
//!
//! Y cuando la puerta dice que no, el registro **no lo llama fallo**: lo separa
//! en "el modo sin red está puesto" y "este destino todavía no está
//! autorizado", que son dos situaciones con dos acciones distintas.
//!
//! # Por qué la traducción no trae ningún servidor puesto
//!
//! Porque traducir manda **el texto que estás leyendo** a un tercero, y elegir
//! por el usuario a qué empresa se lo manda no es una decisión que corresponda
//! al programa. El traductor habla el protocolo de LibreTranslate, que es
//! software libre y se puede levantar en la propia máquina o en un servidor de
//! una universidad; sin anfitrión configurado, el proveedor sencillamente no se
//! registra. Es la misma regla que se aplica a los catálogos: ninguno
//! preconfigurado.
//!
//! La enciclopedia sí trae destino, y es Wikipedia, porque no hay una segunda
//! respuesta razonable a "una enciclopedia abierta": no es una elección
//! comercial, es la obra. Aun así no sale nada hasta que se autoriza el
//! anfitrión.

use std::sync::Arc;

use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::net::{Network, Purpose};

use super::{Entry, LookupKind, LookupProvider, Query};

/// Cuántos caracteres del cuerpo se conservan.
///
/// Un artículo entero no cabe en un panel lateral y nadie lo lee ahí. Se corta
/// por el final de una frase cuando se puede, para que no termine a mitad de
/// palabra.
const MAXIMO_CUERPO: usize = 1_200;

/// Enciclopedia: el resumen del artículo, en el idioma del documento.
pub struct Encyclopedia {
    db: Database,
    net: Arc<Network>,
}

impl Encyclopedia {
    #[must_use]
    pub const fn new(db: Database, net: Arc<Network>) -> Self {
        Self { db, net }
    }

    /// Subetiqueta primaria del idioma, que es lo que nombra al anfitrión.
    ///
    /// Un documento en `es-CO` se consulta en la Wikipedia en español: no hay
    /// una por variante regional, y exigirla dejaría sin enciclopedia a casi
    /// todo el mundo. Es la misma regla de coincidencia que ya usa el registro
    /// para los diccionarios.
    fn idioma_de(query: &Query) -> Option<String> {
        let etiqueta = query.language.as_deref()?;
        let primaria = etiqueta
            .split(['-', '_'])
            .next()
            .unwrap_or(etiqueta)
            .to_ascii_lowercase();

        // Solo letras: una etiqueta traída del webview no puede convertirse en
        // parte de un anfitrión sin comprobarla, o el destino lo elegiría quien
        // escriba los metadatos del libro.
        let valida =
            (2..=3).contains(&primaria.len()) && primaria.chars().all(|c| c.is_ascii_lowercase());
        valida.then_some(primaria)
    }

    fn anfitrion_de(query: &Query) -> Option<String> {
        Self::idioma_de(query).map(|idioma| format!("{idioma}.wikipedia.org"))
    }
}

/// Una palabra en mayúsculas, como la escribe un libro en un encabezado o en el
/// nombre de un personaje de teatro ("HAMLET"), se reintenta como título
/// ("Hamlet"): Wikipedia distingue mayúsculas más allá de la primera letra y
/// respondía que no había artículo.
fn como_titulo(termino: &str) -> String {
    let letras: Vec<char> = termino.chars().filter(|c| c.is_alphabetic()).collect();
    if letras.len() > 1 && letras.iter().all(|c| c.is_uppercase()) {
        let mut salida = String::with_capacity(termino.len());
        let mut primera = true;
        for c in termino.chars() {
            if c.is_alphabetic() {
                if primera {
                    salida.extend(c.to_uppercase());
                    primera = false;
                } else {
                    salida.extend(c.to_lowercase());
                }
            } else {
                if c.is_whitespace() {
                    primera = true;
                }
                salida.push(c);
            }
        }
        salida
    } else {
        termino.to_owned()
    }
}

impl LookupProvider for Encyclopedia {
    fn id(&self) -> &str {
        "wikipedia"
    }

    fn label(&self) -> &str {
        "Wikipedia"
    }

    fn kind(&self) -> LookupKind {
        LookupKind::Encyclopedia
    }

    /// Cualquiera: hay Wikipedia en trescientos idiomas, y mantener aquí la
    /// lista de cuáles sería una lista que envejece sola. Un idioma sin
    /// artículo devuelve una respuesta vacía, que es la respuesta correcta.
    fn languages(&self) -> &[String] {
        &[]
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn destination(&self, query: &Query) -> Option<String> {
        Self::anfitrion_de(query)
    }

    fn owns_host(&self, host: &str) -> bool {
        // El anfitrión que la interfaz proponga tiene que ser uno que este
        // proveedor pueda construir: `es.wikipedia.org` sí, cualquier otra cosa
        // terminada en un dominio parecido, no.
        let Some(prefijo) = host.strip_suffix(".wikipedia.org") else {
            return false;
        };
        (2..=3).contains(&prefijo.len()) && prefijo.chars().all(|c| c.is_ascii_lowercase())
    }

    fn lookup(&self, query: &Query) -> NbResult<Vec<Entry>> {
        let Some(anfitrion) = Self::anfitrion_de(query) else {
            // Sin idioma no se sabe a qué Wikipedia preguntar. No es un fallo:
            // es una consulta que este proveedor no puede atender.
            return Ok(Vec::new());
        };

        let url_de = |termino: &str| {
            format!(
                "https://{anfitrion}/api/rest_v1/page/summary/{}",
                codificar(termino)
            )
        };

        let conn = self.db.connection()?;
        let mut respuesta = self.net.get(&conn, &url_de(&query.term), Purpose::Lookup)?;
        // Sin artículo con las mayúsculas del libro, se prueba como título: "ONU"
        // existe tal cual y "HAMLET" solo como "Hamlet".
        let titulo = como_titulo(&query.term);
        if respuesta.status == 404 && titulo != query.term {
            respuesta = self.net.get(&conn, &url_de(&titulo), Purpose::Lookup)?;
        }

        // Un artículo que no existe responde 404, y eso no es un fallo del
        // proveedor: es que la palabra no tiene artículo.
        if respuesta.status == 404 {
            return Ok(Vec::new());
        }
        if respuesta.status != 200 {
            tracing::info!(
                status = respuesta.status,
                "la enciclopedia no respondió bien"
            );
            return Err(NbError::NetworkFailure);
        }

        let cuerpo = String::from_utf8_lossy(&respuesta.body);
        let documento: serde_json::Value = serde_json::from_str(&cuerpo).map_err(|_| {
            tracing::info!("la enciclopedia respondió algo que no es JSON");
            NbError::NetworkFailure
        })?;

        let extracto = documento
            .get("extract")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        if extracto.is_empty() {
            return Ok(Vec::new());
        }

        let titulo = documento
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&query.term);

        Ok(vec![Entry {
            provider_id: self.id().to_owned(),
            kind: LookupKind::Encyclopedia,
            headword: titulo.to_owned(),
            body: recortar(extracto),
            // La atribución no es cortesía: el contenido es de terceros y su
            // licencia la exige.
            source: Some(format!("Wikipedia ({anfitrion}), CC BY-SA")),
        }])
    }
}

/// Traducción por un servidor que **elige el usuario**.
pub struct Translation {
    db: Database,
    net: Arc<Network>,
    /// Anfitrión configurado. Sin él, este proveedor no se construye.
    host: String,
    /// Idioma al que se traduce, en subetiqueta primaria.
    destino: String,
}

impl Translation {
    /// Construye el traductor si hay un servidor configurado y es utilizable.
    ///
    /// Devuelve `None` cuando no lo hay, y entonces **no se registra**: un
    /// proveedor que aparece en el panel y nunca puede responder es peor que
    /// ninguno.
    #[must_use]
    pub fn new(db: Database, net: Arc<Network>, host: &str, destino: &str) -> Option<Self> {
        let host = host.trim().to_ascii_lowercase();
        // Se acepta el anfitrión pelado, no una URL: así no hay forma de colar
        // un esquema ni una ruta, y la puerta recibe siempre `https://`.
        let utilizable = !host.is_empty()
            && !host.contains('/')
            && !host.contains(':')
            && host.contains('.')
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        if !utilizable {
            return None;
        }

        let destino = destino.split(['-', '_']).next().unwrap_or(destino);
        let destino = destino.to_ascii_lowercase();
        if destino.len() < 2 || !destino.chars().all(|c| c.is_ascii_lowercase()) {
            return None;
        }

        Some(Self {
            db,
            net,
            host,
            destino,
        })
    }
}

impl LookupProvider for Translation {
    fn id(&self) -> &str {
        "traductor"
    }

    fn label(&self) -> &str {
        "Traducción"
    }

    fn kind(&self) -> LookupKind {
        LookupKind::Translation
    }

    /// Cualquiera: el servidor detecta el idioma de origen.
    fn languages(&self) -> &[String] {
        &[]
    }

    fn requires_network(&self) -> bool {
        true
    }

    fn destination(&self, _query: &Query) -> Option<String> {
        Some(self.host.clone())
    }

    fn owns_host(&self, host: &str) -> bool {
        host.eq_ignore_ascii_case(&self.host)
    }

    fn lookup(&self, query: &Query) -> NbResult<Vec<Entry>> {
        // Se traduce **el contexto** cuando lo hay: traducir una palabra suelta
        // sin la frase da la acepción equivocada la mitad de las veces, y el
        // contexto es justo el párrafo del que salió.
        let texto = query.context.as_deref().unwrap_or(&query.term);
        if texto.trim().is_empty() {
            return Ok(Vec::new());
        }

        // El protocolo de LibreTranslate acepta la petición por GET con los
        // parámetros en la dirección, que es lo único que sabe hacer la puerta:
        // un cliente con POST sería una segunda forma de salir, y el
        // innegociable dice que solo hay una.
        let url = format!(
            "https://{}/translate?q={}&source=auto&target={}&format=text",
            self.host,
            codificar(texto),
            self.destino
        );

        let conn = self.db.connection()?;
        let respuesta = self.net.get(&conn, &url, Purpose::Lookup)?;
        if respuesta.status != 200 {
            tracing::info!(status = respuesta.status, "el traductor no respondió bien");
            return Err(NbError::NetworkFailure);
        }

        let cuerpo = String::from_utf8_lossy(&respuesta.body);
        let documento: serde_json::Value = serde_json::from_str(&cuerpo).map_err(|_| {
            tracing::info!("el traductor respondió algo que no es JSON");
            NbError::NetworkFailure
        })?;

        let traducido = documento
            .get("translatedText")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        if traducido.is_empty() {
            return Ok(Vec::new());
        }

        Ok(vec![Entry {
            provider_id: self.id().to_owned(),
            kind: LookupKind::Translation,
            headword: query.term.clone(),
            body: recortar(traducido),
            // Se dice quién tradujo. Una traducción automática y una definición
            // de diccionario no valen lo mismo, y el usuario tiene derecho a
            // distinguirlas sin adivinar.
            source: Some(format!("Traducción automática ({})", self.host)),
        }])
    }
}

/// Codifica un texto para que quepa en una dirección.
///
/// A mano y no con un paquete: son quince líneas, y la política de dependencias
/// dice que no se añade uno por menos de cien. Se escapa **todo** lo que no sea
/// alfanumérico ASCII o uno de los cuatro que el estándar declara seguros, así
/// que un término con `?`, `&` o `/` dentro no puede cambiar la petición.
fn codificar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    for byte in texto.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                salida.push(*byte as char);
            }
            _ => salida.push_str(&format!("%{byte:02X}")),
        }
    }
    salida
}

/// Recorta un texto largo por el final de una frase, si lo hay cerca.
fn recortar(texto: &str) -> String {
    if texto.chars().count() <= MAXIMO_CUERPO {
        return texto.to_owned();
    }

    let corte: String = texto.chars().take(MAXIMO_CUERPO).collect();
    match corte.rfind(". ") {
        // Solo si el punto está en la última cuarta parte: si no, cortar por él
        // tiraría más texto del que sobra.
        Some(i) if i > corte.len() * 3 / 4 => format!("{}.", &corte[..i]),
        _ => format!("{}…", corte.trim_end()),
    }
}

#[cfg(test)]
mod tests {

    /// "HAMLET" se pide como "Hamlet"; lo que no está todo en mayúsculas,
    /// tal cual.
    #[test]
    fn una_palabra_en_mayusculas_se_pide_como_titulo() {
        assert_eq!(super::como_titulo("HAMLET"), "Hamlet");
        assert_eq!(super::como_titulo("REY LEAR"), "Rey Lear");
        assert_eq!(super::como_titulo("iPhone"), "iPhone");
        assert_eq!(super::como_titulo("ONU"), "Onu");
    }

    use super::*;

    fn base() -> Database {
        Database::open_in_memory().expect("base en memoria")
    }

    fn puerta() -> Arc<Network> {
        // Un transporte que nunca se usa: todas las pruebas de aquí paran antes
        // de llegar a él, y las que salen viven en `tests/consulta_en_linea.rs`.
        Arc::new(Network::new(Box::new(crate::net::UreqTransport::new())))
    }

    fn consulta(idioma: Option<&str>) -> Query {
        Query {
            term: "alquimia".to_owned(),
            context: None,
            language: idioma.map(str::to_owned),
        }
    }

    /// El anfitrión sale del idioma del documento, y el idioma del documento lo
    /// escribe quien hizo el EPUB. Sin comprobarlo, el destino de la petición lo
    /// elegiría un tercero.
    #[test]
    fn un_idioma_con_pinta_rara_no_se_convierte_en_un_anfitrion() {
        for veneno in [
            "es.evil.com",
            "../../etc",
            "e",
            "spanish",
            "es/algo",
            "ES ",
            "",
        ] {
            assert_eq!(
                Encyclopedia::anfitrion_de(&consulta(Some(veneno))),
                None,
                "se aceptó {veneno:?} como idioma"
            );
        }
    }

    #[test]
    fn una_variante_regional_consulta_la_enciclopedia_de_su_idioma() {
        assert_eq!(
            Encyclopedia::anfitrion_de(&consulta(Some("es-CO"))).as_deref(),
            Some("es.wikipedia.org")
        );
        assert_eq!(
            Encyclopedia::anfitrion_de(&consulta(Some("pt"))).as_deref(),
            Some("pt.wikipedia.org")
        );
    }

    /// Conceder un permiso pasa por aquí: si esto aceptara cualquier cosa, el
    /// webview podría autorizar el destino que quisiera.
    #[test]
    fn la_enciclopedia_solo_reconoce_como_suyos_sus_propios_anfitriones() {
        let wiki = Encyclopedia::new(base(), puerta());
        assert!(wiki.owns_host("es.wikipedia.org"));
        assert!(wiki.owns_host("pt.wikipedia.org"));

        assert!(!wiki.owns_host("wikipedia.org"));
        assert!(!wiki.owns_host("es.wikipedia.org.evil.com"));
        assert!(!wiki.owns_host("malicioso.wikipedia.org"));
        assert!(!wiki.owns_host("es.wikipedia.com"));
    }

    #[test]
    fn sin_servidor_configurado_no_hay_traductor() {
        for invalido in [
            "",
            "   ",
            "https://libre.example",
            "libre.example/api",
            "sinpunto",
        ] {
            assert!(
                Translation::new(base(), puerta(), invalido, "es").is_none(),
                "se aceptó {invalido:?} como servidor de traducción"
            );
        }
    }

    #[test]
    fn el_traductor_solo_reconoce_el_servidor_que_le_configuraron() {
        let traductor =
            Translation::new(base(), puerta(), "libre.example", "es").expect("configurado");
        assert!(traductor.owns_host("libre.example"));
        assert!(traductor.owns_host("LIBRE.EXAMPLE"));
        assert!(!traductor.owns_host("otro.example"));
    }

    /// Un término con `&` o `?` dentro no puede cambiar la petición. Es entrada
    /// del documento, así que es entrada no confiable.
    #[test]
    fn el_termino_se_codifica_entero_antes_de_entrar_en_la_direccion() {
        assert_eq!(codificar("a&b?c/d"), "a%26b%3Fc%2Fd");
        assert_eq!(codificar("año"), "a%C3%B1o");
        assert_eq!(codificar("simple-1_2.3~"), "simple-1_2.3~");
    }

    #[test]
    fn un_texto_corto_no_se_recorta() {
        assert_eq!(recortar("Una frase."), "Una frase.");
    }

    #[test]
    fn un_texto_largo_se_recorta_y_lo_dice() {
        let largo = "palabra ".repeat(400);
        let recortado = recortar(&largo);
        assert!(recortado.chars().count() <= MAXIMO_CUERPO + 1);
        assert!(recortado.ends_with('…'));
    }
}
