//! Exportación de anotaciones en el modelo de anotación web del W3C.
//!
//! # Qué problema resuelve
//!
//! La exportación completa devuelve la biblioteca a NB Books. Esta
//! devuelve las anotaciones **a cualquiera**: a otra herramienta de anotación, a
//! un script de quince líneas, a la persona que abra el archivo dentro de diez
//! años sin saber qué programa lo escribió.
//!
//! Por eso el formato no es propio. Es JSON-LD con el contexto del W3C, y lo que
//! contiene son los selectores que ese modelo define, con sus nombres. Un
//! consumidor que solo sepa JSON encuentra la cita en `selector`, y uno que
//! entienda JSON-LD encuentra además lo que significa cada campo.
//!
//! # Lo que no cabe en el estándar
//!
//! El modelo del W3C no tiene ningún selector para la **progresión relativa**,
//! que es la tercera vía de la cascada de ADR-004. Se exporta bajo una clave con
//! espacio de nombres propio, fuera del vocabulario estándar: un consumidor que
//! no la conozca la ignora y sigue teniendo la cita y la posición, que es lo que
//! el criterio de aceptación pide. Meterla dentro del vocabulario estándar con
//! un nombre inventado sería peor: haría que un archivo nuestro pareciera decir
//! algo que el estándar no dice.
//!
//! Contrato: `docs/architecture/api/annotation-export.md`.

use serde::{Deserialize, Serialize};

use super::Annotation;
use crate::db::catalog::AnnotationKind;
use crate::error::NbResult;
use crate::reader::Locator;

/// Contexto JSON-LD del modelo de anotación web.
pub const CONTEXTO: &str = "http://www.w3.org/ns/anno.jsonld";

/// Espacio de nombres para lo que el estándar no cubre.
///
/// Un dominio `.invalid` a propósito: no resuelve y no puede resolver nunca, así
/// que nadie va a pedirle una definición a un servidor que no existe, y queda
/// claro que es una extensión local y no vocabulario publicado.
pub const NS: &str = "https://nbdigitallabs.lat/books/ns#";

/// Especificación a la que se ajusta un identificador de fragmento de EPUB.
const CFI: &str = "http://www.idpf.org/epub/linking/cfi/epub-cfi.html";

/// La de los fragmentos de PDF, que sirve también para los cómics paginados:
/// `page=N` significa lo mismo en los dos.
const PAGINA: &str = "http://tools.ietf.org/rfc/rfc3778";

/// Una página de anotaciones, tal y como la define el modelo del W3C.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnnotationPage {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub items: Vec<WebAnnotation>,
}

/// Una anotación en el modelo del W3C.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebAnnotation {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Para qué se anotó: `highlighting`, `commenting` o `bookmarking`.
    pub motivation: String,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<TextualBody>,
    pub target: Target,
}

/// El texto de una nota.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextualBody {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    pub format: String,
}

/// A qué apunta la anotación.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Target {
    /// El recurso dentro de la publicación.
    pub source: String,
    /// Los selectores, todos los que se sepan.
    ///
    /// Varios y no uno: es lo que el modelo llama un conjunto de selectores
    /// alternativos, y es exactamente la cascada de ADR-004 escrita en el
    /// vocabulario de otro. Quien lea el archivo elige el que sepa aplicar.
    pub selector: Vec<Selector>,
}

/// Un selector del modelo de anotación web.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Selector {
    /// La cita exacta, con lo que va antes y después.
    TextQuoteSelector {
        exact: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        suffix: Option<String>,
    },
    /// Desplazamiento en caracteres dentro del recurso.
    TextPositionSelector { start: u32, end: u32 },
    /// Un identificador de fragmento del formato.
    FragmentSelector {
        value: String,
        #[serde(rename = "conformsTo", skip_serializing_if = "Option::is_none")]
        conforms_to: Option<String>,
    },
}

/// Lo que el estándar no cubre, con espacio de nombres propio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    /// Progresión total dentro de la publicación, de 0 a 1.
    #[serde(rename = "totalProgression", skip_serializing_if = "Option::is_none")]
    pub total_progression: Option<f64>,
    /// Progresión dentro del recurso.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progression: Option<f64>,
    /// Posición absoluta, si el formato la tiene.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    /// La versión anterior de esta anotación, si la hay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
}

/// La motivación del modelo del W3C que corresponde a cada clase de marca.
///
/// # Por qué el subrayado y el trazo salen como resaltado
///
/// Porque el vocabulario de motivaciones del W3C es **cerrado** y no tiene
/// ninguna para un subrayado ni para tinta libre. Inventar una haría un documento
/// que dice ser del estándar y no lo es, y un consumidor que valide lo rechazaría
/// entero: se perdería la anotación, no solo su matiz.
///
/// `highlighting` es "la intención de destacar el recurso o parte de él", que es
/// lo que hacen las tres. Lo que se pierde al exportar **a otra herramienta** es
/// la distinción visual, y es una pérdida acotada y consciente.
///
/// No afecta a la exportación de la biblioteca: `portability.rs` no pasa por
/// aquí, así que llevarse la biblioteca entera y volver conserva las cinco
/// clases. Esto es solo la puerta hacia programas de terceros.
const fn motivacion(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Highlight | AnnotationKind::Underline | AnnotationKind::Drawing => {
            "highlighting"
        }
        AnnotationKind::Note => "commenting",
        AnnotationKind::Bookmark => "bookmarking",
    }
}

/// Traduce un locator a los selectores del modelo del W3C.
///
/// El orden importa: primero el más preciso. El modelo no lo exige, pero un
/// consumidor sencillo aplica el primero que entiende, y darle el peor primero
/// lo dejaría en un sitio peor del que podíamos ofrecerle.
fn selectores(locator: &Locator) -> Vec<Selector> {
    let mut salida = Vec::new();

    for fragmento in &locator.locations.fragments {
        if let Some(inicio) = fragmento
            .strip_prefix("#char=")
            .and_then(|n| n.parse::<u32>().ok())
        {
            // El desplazamiento por sí solo no dice dónde acaba: el final sale de
            // la longitud de la cita, que es lo que se resaltó.
            let largo = locator.text.as_ref().map_or(0, |t| {
                u32::try_from(t.highlight.chars().count()).unwrap_or(0)
            });
            salida.push(Selector::TextPositionSelector {
                start: inicio,
                end: inicio.saturating_add(largo),
            });
            continue;
        }

        let conforms_to = if fragmento.starts_with("epubcfi(") {
            Some(CFI.to_owned())
        } else if fragmento.starts_with("page=") {
            Some(PAGINA.to_owned())
        } else {
            None
        };

        salida.push(Selector::FragmentSelector {
            value: fragmento.clone(),
            conforms_to,
        });
    }

    if let Some(contexto) = &locator.text {
        salida.push(Selector::TextQuoteSelector {
            exact: contexto.highlight.clone(),
            prefix: contexto.before.clone(),
            suffix: contexto.after.clone(),
        });
    }

    salida
}

/// Convierte una anotación al modelo del W3C, con lo que no cabe en él aparte.
#[must_use]
pub fn to_web(anotacion: &Annotation) -> (WebAnnotation, Extension) {
    let cuerpo = anotacion.body.as_ref().map(|texto| TextualBody {
        kind: "TextualBody".to_owned(),
        value: texto.clone(),
        format: "text/plain".to_owned(),
    });

    let web = WebAnnotation {
        context: CONTEXTO.to_owned(),
        // Un URN y no una URL: la anotación no se sirve por HTTP en ningún
        // sitio, y darle una dirección que no responde sería mentir sobre dónde
        // vive. `urn:uuid:` es la forma canónica de decir "esto es un
        // identificador, no una dirección".
        id: format!("urn:uuid:{}", con_guiones(&anotacion.id)),
        kind: "Annotation".to_owned(),
        motivation: motivacion(anotacion.kind).to_owned(),
        created: anotacion.created_at.clone(),
        body: cuerpo,
        target: Target {
            source: anotacion.target.href.clone(),
            selector: selectores(&anotacion.target),
        },
    };

    let extension = Extension {
        total_progression: anotacion.target.locations.total_progression,
        progression: anotacion.target.locations.progression,
        position: anotacion.target.locations.position,
        supersedes: anotacion
            .supersedes
            .as_ref()
            .map(|id| format!("urn:uuid:{}", con_guiones(id))),
    };

    (web, extension)
}

/// Da forma canónica a un identificador de 32 hexadecimales.
///
/// La base los guarda sin guiones porque los genera con `randomblob`; `urn:uuid`
/// los exige con ellos. Se convierte al salir y no al guardar para no tocar
/// treinta y dos caracteres en cada fila por un formato que solo importa aquí.
fn con_guiones(id: &str) -> String {
    if id.len() != 32 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return id.to_owned();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &id[0..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    )
}

/// Exporta las anotaciones vivas de una edición.
///
/// Las dadas de baja no salen: este archivo sirve para llevarse las anotaciones
/// a otra herramienta, y el historial de versiones no significa nada fuera de
/// aquí. El historial completo viaja en la exportación, que es la que
/// sirve para volver.
///
/// # Errors
///
/// [`crate::error::NbError::StorageFailure`] si la consulta falla, o
/// [`crate::error::NbError::Internal`] si la serialización falla.
pub fn export(conn: &rusqlite::Connection, edition_id: &str) -> NbResult<serde_json::Value> {
    let vivas = super::list(conn, edition_id)?;
    let mut items = Vec::with_capacity(vivas.len());

    for anotacion in &vivas {
        let (web, extension) = to_web(anotacion);
        let mut valor = serde_json::to_value(&web).map_err(|_| crate::error::NbError::Internal)?;
        let extra =
            serde_json::to_value(&extension).map_err(|_| crate::error::NbError::Internal)?;

        if let (Some(objeto), Some(campos)) = (valor.as_object_mut(), extra.as_object()) {
            if !campos.is_empty() {
                objeto.insert(format!("{NS}locations"), extra.clone());
            }
        }
        items.push(valor);
    }

    Ok(serde_json::json!({
        "@context": CONTEXTO,
        "type": "AnnotationPage",
        "items": items,
    }))
}
