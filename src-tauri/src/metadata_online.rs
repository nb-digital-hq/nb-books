//! Portada y metadatos obtenidos de un catálogo abierto.
//!
//! # Qué promesa cierra
//!
//! Una escrita en `docs/adr/product-decisions.md`, sección 1, que
//! no tenía módulo propio: que se pueda completar la ficha de un libro sin
//! escribirla a mano. Lo que **no** cierra, y a propósito, es la promesa de
//! hacerlo solo: la consulta es explícita, nunca automática al importar.
//!
//! # Por qué nunca sobrescribe
//!
//! Porque lo que llega de fuera no sabe lo que el usuario decidió. Todo entra
//! por [`crate::metadata::propose`] con origen `online`, y esa función ya hace
//! cumplir la regla de procedencia: si hay una edición manual del
//! campo, el valor **no se escribe**, queda como propuesta. Aquí no hay una
//! segunda copia de esa regla, que es exactamente el motivo de que la regla
//! viva ahí.
//!
//! Y lo que se rechaza no vuelve a ofrecerse: también es de `propose`, desde
//! este módulo.
//!
//! # Por qué Open Library
//!
//! Porque sus datos son **abiertos** y su interfaz no pide credencial: ninguna
//! de las dos cosas es un detalle. Una credencial obligaría a guardar un secreto
//! —que el esquema prohíbe— o a distribuir uno nuestro en el binario, y unos
//! datos cerrados convertirían la ficha del usuario en algo que solo se puede
//! completar mientras un tercero lo permita.
//!
//! Aun así **no sale nada sin consentimiento**: son dos anfitriones distintos
//! —el catálogo y las portadas— y cada uno se autoriza por separado, porque
//! pedir un dato de texto y descargar una imagen no son la misma decisión.
//!
//! # Sin red, la vía normal sigue siendo la de siempre
//!
//! La extracción local no cambia ni depende de esto. Esto es lo que se
//! hace cuando aquella no encontró nada, que con EPUB mal etiquetados es a
//! menudo.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::net::{Network, Purpose};

/// Catálogo. Se autoriza por separado del de portadas.
pub const HOST_CATALOGO: &str = "openlibrary.org";

/// Portadas. Otro anfitrión, otro permiso: pedir un dato de texto y descargar
/// una imagen no son la misma decisión.
pub const HOST_PORTADAS: &str = "covers.openlibrary.org";

/// Cuántos candidatos se ofrecen.
///
/// Cinco. Una lista de veinte no es una elección: es un trabajo. Y quien busca
/// su libro lo reconoce en los primeros o no lo va a reconocer.
const MAXIMO_CANDIDATOS: usize = 5;

/// Cuánto puede pesar una portada.
///
/// Ocho mebibytes. Una portada de tapa es medio mega; lo que pase de aquí no es
/// una portada, y el límite existe porque lo que llega de fuera no se cree.
const MAXIMO_PORTADA: usize = 8 * 1024 * 1024;

/// Un libro encontrado en el catálogo.
///
/// Todos los campos son opcionales menos el título porque el catálogo lo es: hay
/// fichas sin editorial, sin año y sin idioma, y exigirlos dejaría fuera
/// precisamente las obras menos conocidas, que son las que más falta hace
/// completar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "MetadataCandidate"
)]
pub struct Candidate {
    pub title: String,
    /// Autores, separados por coma, tal y como los da el catálogo.
    pub authors: Option<String>,
    /// Año de la primera publicación, como texto: una fecha parcial es un dato,
    /// no un defecto. Es la misma decisión que el esquema toma en `editions`.
    pub published_date: Option<String>,
    pub publisher: Option<String>,
    /// Etiqueta BCP-47, cuando el catálogo la da.
    pub language: Option<String>,
    /// Dirección de la portada, o nulo si esa ficha no tiene ninguna.
    pub cover_url: Option<String>,
    /// Qué extensión lo propuso, o nulo si viene del catálogo del núcleo.
    ///
    /// Quien elige entre varias fichas tiene que poder saber cuál viene del
    /// catálogo de siempre y cuál de una extensión que instaló la semana
    /// pasada. Sin esto la decisión se toma a ciegas, y el candidato de un
    /// tercero se vería exactamente igual que el nuestro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Busca candidatos por título y autor.
///
/// # Errors
///
/// - [`NbError::NetworkDisabled`] con el modo sin red puesto.
/// - [`NbError::ConsentRequired`] si el catálogo no está autorizado.
/// - [`NbError::NetworkFailure`] si no responde o responde algo ilegible.
pub fn buscar(
    db: &Database,
    net: &Arc<Network>,
    title: &str,
    author: Option<&str>,
) -> NbResult<Vec<Candidate>> {
    let titulo = title.trim();
    if titulo.is_empty() {
        // Sin título no hay nada que buscar. No es un fallo: es una edición de
        // la que no se sabe lo suficiente para preguntar.
        return Ok(Vec::new());
    }

    let mut url = format!(
        "https://{HOST_CATALOGO}/search.json?limit={MAXIMO_CANDIDATOS}\
         &fields=title,author_name,first_publish_year,publisher,language,cover_i\
         &title={}",
        codificar(titulo)
    );
    if let Some(autor) = author.map(str::trim).filter(|a| !a.is_empty()) {
        url.push_str(&format!("&author={}", codificar(autor)));
    }

    let conn = db.connection()?;
    let respuesta = net.get(&conn, &url, Purpose::Metadata)?;
    if respuesta.status != 200 {
        tracing::info!(status = respuesta.status, "el catálogo no respondió bien");
        return Err(NbError::NetworkFailure);
    }

    let cuerpo = String::from_utf8_lossy(&respuesta.body);
    let documento: serde_json::Value = serde_json::from_str(&cuerpo).map_err(|_| {
        tracing::info!("el catálogo respondió algo que no es JSON");
        NbError::NetworkFailure
    })?;

    let fichas = documento
        .get("docs")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    Ok(fichas
        .iter()
        .take(MAXIMO_CANDIDATOS)
        .filter_map(candidato_de)
        .collect())
}

/// Convierte una ficha del catálogo en un candidato, o la descarta.
///
/// Una ficha sin título se descarta entera: es el único campo del que depende
/// todo lo demás, y una propuesta sin título no se puede ni enseñar.
fn candidato_de(ficha: &serde_json::Value) -> Option<Candidate> {
    let title = ficha.get("title")?.as_str()?.trim();
    if title.is_empty() {
        return None;
    }

    let primero = |clave: &str| {
        ficha
            .get(clave)
            .and_then(serde_json::Value::as_array)
            .and_then(|lista| lista.first())
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };

    let authors = ficha
        .get("author_name")
        .and_then(serde_json::Value::as_array)
        .map(|lista| {
            lista
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|a| !a.is_empty());

    let cover_url = ficha
        .get("cover_i")
        .and_then(serde_json::Value::as_i64)
        // El identificador se interpola en una dirección, así que se comprueba
        // que sea un número positivo en lugar de confiar en que lo sea.
        .filter(|id| *id > 0)
        .map(|id| format!("https://{HOST_PORTADAS}/b/id/{id}-L.jpg"));

    Some(Candidate {
        title: title.to_owned(),
        authors,
        published_date: ficha
            .get("first_publish_year")
            .and_then(serde_json::Value::as_i64)
            .map(|anio| anio.to_string()),
        publisher: primero("publisher"),
        language: primero("language"),
        cover_url,
        // Del catálogo del núcleo: sin extensión detrás.
        source: None,
    })
}

/// Descarga una portada y la deja en la caché con el nombre de la edición.
///
/// Devuelve el **nombre del archivo**, que es lo que guarda `editions.cover_path`
/// —nunca una ruta absoluta, por la regla—.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la dirección no es del anfitrión de
///   portadas. Se comprueba aquí y no se confía en quien la propuso: la
///   dirección viene del webview, que es zona no confiable.
/// - [`NbError::NetworkDisabled`], [`NbError::ConsentRequired`] o
///   [`NbError::NetworkFailure`] según lo que diga la puerta.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn descargar_portada(
    db: &Database,
    net: &Arc<Network>,
    covers_dir: &Path,
    edition_id: &str,
    url: &str,
) -> NbResult<String> {
    let anfitrion = crate::net::host_de(url)?;
    if anfitrion != HOST_PORTADAS {
        tracing::warn!("se intentó descargar una portada de un anfitrión ajeno");
        return Err(NbError::PathNotAllowed);
    }

    let conn = db.connection()?;
    let respuesta = net.get(&conn, url, Purpose::Metadata)?;
    if respuesta.status != 200 || respuesta.body.is_empty() {
        tracing::info!(status = respuesta.status, "la portada no llegó");
        return Err(NbError::NetworkFailure);
    }
    if respuesta.body.len() > MAXIMO_PORTADA {
        tracing::warn!(bytes = respuesta.body.len(), "la portada pasa del límite");
        return Err(NbError::NetworkFailure);
    }

    // La extensión sale del **contenido**, no de la dirección: un archivo se
    // llama como quiera y eso no decide lo que es. Es la misma regla que la
    // importación aplica al formato de un documento.
    let extension = extension_de(&respuesta.body).ok_or_else(|| {
        tracing::info!("lo que llegó como portada no es una imagen conocida");
        NbError::NetworkFailure
    })?;

    let nombre = format!("{edition_id}.{extension}");
    // Solo la miniatura, como al importar. Una descarga anterior de la
    // misma edición pudo dejar su miniatura: se retira antes, o `para_rejilla`
    // seguiría sirviendo la portada vieja.
    let _ = std::fs::remove_file(covers_dir.join(format!("{nombre}.min.jpg")));
    crate::covers::guardar(covers_dir, &nombre, &respuesta.body)?;

    conn.execute(
        "UPDATE editions SET cover_path = ?1 WHERE id = ?2",
        rusqlite::params![&nombre, edition_id],
    )
    .map_err(|error| {
        tracing::error!(error = %error, "no se pudo registrar la portada");
        NbError::StorageFailure
    })?;

    Ok(nombre)
}

/// Qué imagen es, mirando su firma.
///
/// Cuatro formatos y no una lista abierta: son los que un catálogo sirve y los
/// que un webview pinta sin ayuda. Un SVG **no** está, y no es un olvido: un SVG
/// puede traer script dentro.
fn extension_de(bytes: &[u8]) -> Option<&'static str> {
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF];
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G'];
    const GIF: &[u8] = b"GIF8";

    if bytes.starts_with(JPEG) {
        return Some("jpg");
    }
    if bytes.starts_with(PNG) {
        return Some("png");
    }
    if bytes.starts_with(GIF) {
        return Some("gif");
    }
    // WebP es RIFF con la marca `WEBP` en el byte 8.
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    None
}

/// Aplica un candidato a una edición, campo por campo.
///
/// Devuelve qué pasó con cada uno. **Nada de esto sobrescribe una edición
/// manual**: lo impide `metadata::propose`, y por eso aquí no hay ninguna
/// comprobación de procedencia.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna escritura falla.
pub fn aplicar(
    conn: &rusqlite::Connection,
    edition_id: &str,
    candidato: &Candidate,
) -> NbResult<Vec<(crate::metadata::Field, crate::metadata::Outcome)>> {
    use crate::metadata::{propose, Field, Source};

    let campos: [(Field, Option<&str>); 4] = [
        (Field::Title, Some(candidato.title.as_str())),
        (Field::Publisher, candidato.publisher.as_deref()),
        (Field::PublishedDate, candidato.published_date.as_deref()),
        (Field::Language, candidato.language.as_deref()),
    ];

    let mut resultados = Vec::with_capacity(campos.len());
    for (campo, valor) in campos {
        // Un campo que el catálogo no trae no se propone como vacío: proponer
        // "no se sabe" borraría lo que sí se sabía.
        let Some(valor) = valor.map(str::trim).filter(|v| !v.is_empty()) else {
            continue;
        };
        resultados.push((
            campo,
            propose(conn, edition_id, campo, Source::Online, Some(valor))?,
        ));
    }

    Ok(resultados)
}

/// Ruta de la caché donde viven las portadas.
#[must_use]
pub fn carpeta_de_portadas(cache: &Path) -> PathBuf {
    cache.join("portadas")
}

/// Codifica un texto para que quepa en una dirección.
///
/// La misma de `lookup::online` y por el mismo motivo: son quince líneas, y la
/// política de dependencias dice que no se añade un paquete por menos de cien.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_ficha_sin_titulo_se_descarta_entera() {
        let ficha = serde_json::json!({ "author_name": ["Alguien"] });
        assert!(candidato_de(&ficha).is_none());
    }

    #[test]
    fn los_autores_llegan_juntos_y_en_orden() {
        let ficha = serde_json::json!({
            "title": "Cien años de soledad",
            "author_name": ["Gabriel García Márquez", "Otro"]
        });
        let candidato = candidato_de(&ficha).expect("candidato");
        assert_eq!(
            candidato.authors.as_deref(),
            Some("Gabriel García Márquez, Otro")
        );
    }

    /// El identificador de portada se interpola en una dirección: si llegara un
    /// valor raro, la dirección iría a otro sitio.
    #[test]
    fn una_portada_sin_identificador_valido_no_produce_direccion() {
        for id in [
            serde_json::json!(0),
            serde_json::json!(-4),
            serde_json::json!("12; rm -rf"),
            serde_json::json!(null),
        ] {
            let ficha = serde_json::json!({ "title": "Obra", "cover_i": id });
            assert_eq!(candidato_de(&ficha).expect("candidato").cover_url, None);
        }
    }

    #[test]
    fn una_portada_con_identificador_produce_la_direccion_del_anfitrion_de_portadas() {
        let ficha = serde_json::json!({ "title": "Obra", "cover_i": 12345 });
        assert_eq!(
            candidato_de(&ficha)
                .expect("candidato")
                .cover_url
                .as_deref(),
            Some("https://covers.openlibrary.org/b/id/12345-L.jpg")
        );
    }

    /// Lo que llega se mira por dentro. Un archivo se llama como quiera y eso no
    /// decide lo que es.
    #[test]
    fn la_extension_sale_del_contenido_y_no_de_la_direccion() {
        assert_eq!(extension_de(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(extension_de(b"\x89PNG\r\n\x1a\n"), Some("png"));
        assert_eq!(extension_de(b"GIF89a...."), Some("gif"));
        assert_eq!(extension_de(b"RIFF____WEBPVP8 "), Some("webp"));
    }

    /// Un SVG no se acepta, y no es un olvido: puede traer script dentro.
    #[test]
    fn un_svg_no_pasa_por_portada() {
        assert_eq!(
            extension_de(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"),
            None
        );
        assert_eq!(extension_de(b"<?xml version=\"1.0\"?><svg>"), None);
    }

    #[test]
    fn el_titulo_se_codifica_antes_de_entrar_en_la_direccion() {
        assert_eq!(
            codificar("Cien años & más"),
            "Cien%20a%C3%B1os%20%26%20m%C3%A1s"
        );
    }
}
