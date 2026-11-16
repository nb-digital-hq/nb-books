//! Metadatos académicos por identificador.
//!
//! # Qué demuestra estar aquí y no en el núcleo
//!
//! Que una fuente de metadatos nueva **entra sin tocar el núcleo**. El núcleo
//! trae un proveedor general de portadas y fichas; este añade lo
//! que ese no cubre —la ficha bibliográfica de un artículo por su DOI— y lo
//! hace por la misma puerta que usaría un tercero: manifiesto, permiso de red
//! por anfitrión y origen, y candidatos que **se proponen**.
//!
//! # Por qué solo por identificador
//!
//! Porque es lo que distingue a una fuente académica de una general. Buscar por
//! título en Crossref devuelve coincidencias aproximadas que hay que desempatar
//! a ojo, y proponer una ficha aproximada sobre un documento que el usuario ya
//! tenía bien catalogado es peor que no proponer nada. Con un DOI la respuesta
//! es exacta o no hay respuesta.
//!
//! Sin DOI devuelve lista vacía, que no es un error: es no saber.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "metadata-provider",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

use crate::nb::extension::host::{fetch, FetchError};

/// La API pública de Crossref. Se declara igual en el manifiesto, y sin ese
/// permiso concedido **a esta extensión** la salida se deniega antes de llegar
/// al transporte.
const CROSSREF: &str = "https://api.crossref.org/works/";

struct Academico;

impl Guest for Academico {
    fn lookup(q: Query) -> Result<Vec<Candidate>, LookupError> {
        let Some(doi) = q.identifiers.iter().find_map(|id| doi_de(id)) else {
            return Ok(Vec::new());
        };

        let respuesta = fetch(&format!("{CROSSREF}{doi}")).map_err(|motivo| match motivo {
            // Los tres se conservan tal cual: "pide permiso", "no hay red" y
            // "se intentó y falló" piden acciones distintas de quien lo lee.
            FetchError::Denied => LookupError::Denied,
            FetchError::Offline => LookupError::Offline,
            FetchError::Failed => LookupError::Failed,
        })?;

        // Un 404 de Crossref significa que ese DOI no está, y eso es no saber,
        // no fallar. Distinguirlo importa: un error haría que la interfaz
        // enseñara un aviso rojo por un documento que sencillamente no es un
        // artículo indexado.
        if respuesta.status == 404 {
            return Ok(Vec::new());
        }
        if respuesta.status != 200 {
            return Err(LookupError::Failed);
        }

        let cuerpo = String::from_utf8(respuesta.body).map_err(|_| LookupError::Failed)?;
        let documento: serde_json::Value =
            serde_json::from_str(&cuerpo).map_err(|_| LookupError::Failed)?;

        Ok(ficha(&documento["message"]).into_iter().collect())
    }
}

/// El DOI que lleva dentro un identificador, si lo lleva.
///
/// Se admiten las tres formas con las que llega un DOI en la vida real: pelado,
/// con el prefijo `doi:` y como dirección de `doi.org`. Quien cataloga no
/// escribe siempre la misma, y rechazar dos de las tres convertiría la
/// extensión en algo que "a veces no funciona".
fn doi_de(identificador: &str) -> Option<String> {
    let limpio = identificador.trim();
    let sin_prefijo = limpio
        .strip_prefix("doi:")
        .or_else(|| limpio.strip_prefix("DOI:"))
        .or_else(|| limpio.strip_prefix("https://doi.org/"))
        .or_else(|| limpio.strip_prefix("http://doi.org/"))
        .or_else(|| limpio.strip_prefix("https://dx.doi.org/"))
        .unwrap_or(limpio)
        .trim();

    // Todo DOI empieza por "10." y lleva una barra. Sin las dos cosas no es un
    // DOI, y preguntar por cualquier cadena gastaría una petición y el permiso
    // del usuario en algo que no puede responder.
    if sin_prefijo.starts_with("10.") && sin_prefijo.contains('/') {
        Some(sin_prefijo.to_owned())
    } else {
        None
    }
}

/// La ficha que sale de la respuesta, si trae al menos un título.
///
/// Cada campo va por separado y `None` cuando la fuente no lo sabe: proponer un
/// campo vacío borraría lo que sí se sabía, y eso es justo lo que el modelo de
/// propuestas existe para impedir.
fn ficha(mensaje: &serde_json::Value) -> Option<Candidate> {
    let title = primera_cadena(&mensaje["title"])?;
    Some(Candidate {
        title,
        authors: autores(&mensaje["author"]),
        published_date: anio(&mensaje["issued"]),
        publisher: mensaje["publisher"].as_str().map(str::to_owned),
        // Crossref da el idioma en ISO 639-1 cuando lo sabe, y a menudo no lo
        // sabe.
        language: mensaje["language"].as_str().map(str::to_owned),
    })
}

fn primera_cadena(valor: &serde_json::Value) -> Option<String> {
    match valor {
        serde_json::Value::Array(lista) => lista.first()?.as_str().map(str::to_owned),
        serde_json::Value::String(texto) => Some(texto.clone()),
        _ => None,
    }
}

/// Los autores, en el orden en el que vienen y separados por punto y coma.
///
/// El punto y coma y no la coma: un apellido compuesto lleva comas dentro, y
/// separando por comas "García Márquez, Gabriel" se leería como dos personas.
fn autores(valor: &serde_json::Value) -> Option<String> {
    let lista = valor.as_array()?;
    let nombres: Vec<String> = lista
        .iter()
        .filter_map(|autor| {
            let familia = autor["family"].as_str();
            let dado = autor["given"].as_str();
            match (familia, dado) {
                (Some(familia), Some(dado)) => Some(format!("{familia}, {dado}")),
                (Some(familia), None) => Some(familia.to_owned()),
                // Una entidad colectiva viene con `name` en lugar de nombre y
                // apellido: un consorcio también firma artículos.
                _ => autor["name"].as_str().map(str::to_owned),
            }
        })
        .collect();

    (!nombres.is_empty()).then(|| nombres.join("; "))
}

/// El año de publicación, de `date-parts`.
///
/// Solo el año, y no la fecha completa: Crossref da partes de fecha con
/// precisión variable —a veces solo el año, a veces año y mes—, y componer una
/// fecha que aparente más precisión de la que hay es inventarla.
fn anio(valor: &serde_json::Value) -> Option<String> {
    let partes = valor["date-parts"].as_array()?.first()?.as_array()?;
    partes.first()?.as_i64().map(|anio| anio.to_string())
}

export!(Academico);
