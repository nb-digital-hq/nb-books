//! Las anotaciones de un libro en texto que se lee sin NB Books.
//!
//! La exportación es W3C Web Annotation en JSON: la entienden otras
//! herramientas, y no la lee nadie. Foliate saca además un archivo que se abre y se
//! lee, y eso es lo que faltaba: **Markdown** y **texto plano**, con cada anotación en
//! el orden del libro, su cita, su nota y dónde está.
//!
//! "Dónde está" va dos veces a propósito: como lo lee una persona -el capítulo y el
//! porcentaje- y como lo lee una máquina -el recurso y el fragmento del Locator-, que
//! es lo que permite volver al sitio exacto con otra herramienta o con esta misma
//! después de reinstalar. Nunca un número de página inventado: ver ADR-004.

use rusqlite::Connection;

use super::{list, Annotation};
use crate::db::catalog::AnnotationKind;
use crate::error::{NbError, NbResult};

/// En qué se exporta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    Markdown,
    Texto,
}

impl Formato {
    /// Lo que pide la interfaz: `markdown` o `text`. Cualquier otra cosa no es de
    /// este módulo.
    #[must_use]
    pub fn de(nombre: &str) -> Option<Self> {
        match nombre {
            "markdown" => Some(Self::Markdown),
            "text" => Some(Self::Texto),
            _ => None,
        }
    }
}

/// Los nombres que se escriben, en los tres idiomas de la interfaz. Viven aquí y no
/// en la interfaz porque el archivo lo escribe el runtime, y porque un archivo con
/// "highlight" dentro de una exportación en español es un defecto.
struct Nombres {
    anotaciones: &'static str,
    nota: &'static str,
    sin_capitulo: &'static str,
    clase: fn(AnnotationKind) -> &'static str,
}

fn nombres(idioma: &str) -> Nombres {
    match idioma.split('-').next().unwrap_or(idioma) {
        "en" => Nombres {
            anotaciones: "annotations",
            nota: "Note",
            sin_capitulo: "Untitled section",
            clase: |k| match k {
                AnnotationKind::Highlight => "Highlight",
                AnnotationKind::Note => "Note",
                AnnotationKind::Bookmark => "Bookmark",
                AnnotationKind::Underline => "Underline",
                AnnotationKind::Drawing => "Drawing",
            },
        },
        "pt" => Nombres {
            anotaciones: "anotações",
            nota: "Nota",
            sin_capitulo: "Secção sem título",
            clase: |k| match k {
                AnnotationKind::Highlight => "Destaque",
                AnnotationKind::Note => "Nota",
                AnnotationKind::Bookmark => "Marcador",
                AnnotationKind::Underline => "Sublinhado",
                AnnotationKind::Drawing => "Traço",
            },
        },
        _ => Nombres {
            anotaciones: "anotaciones",
            nota: "Nota",
            sin_capitulo: "Sección sin título",
            clase: |k| match k {
                AnnotationKind::Highlight => "Resaltado",
                AnnotationKind::Note => "Nota",
                AnnotationKind::Bookmark => "Marcador",
                AnnotationKind::Underline => "Subrayado",
                AnnotationKind::Drawing => "Trazo",
            },
        },
    }
}

/// Dónde está, como lo lee una máquina: el recurso y su fragmento.
fn localizador(anotacion: &Annotation) -> String {
    let fragmento = anotacion
        .target
        .locations
        .fragments
        .first()
        .cloned()
        .unwrap_or_default();
    format!("{}{fragmento}", anotacion.target.href)
}

/// Dónde está, como lo lee una persona: cuánto del libro.
fn porcentaje(anotacion: &Annotation) -> Option<String> {
    anotacion
        .target
        .locations
        .total_progression
        .map(|p| format!("{} %", (p * 100.0).round()))
}

/// Las anotaciones vivas de una edición en texto, en el orden del libro.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn exportar(
    conn: &Connection,
    edition_id: &str,
    formato: Formato,
    idioma: &str,
) -> NbResult<String> {
    let titulo: String = conn
        .query_row(
            "SELECT title FROM editions WHERE id = ?1",
            [edition_id],
            |f| f.get(0),
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo leer el título para exportar");
            NbError::StorageFailure
        })?;
    let mut lista = list(conn, edition_id)?;
    // El orden del libro, que es el de quien lee lo exportado; a igualdad de sitio, el
    // de creación, que es el de `list`.
    lista.sort_by(|a, b| {
        let pa = a.target.locations.total_progression.unwrap_or(f64::MAX);
        let pb = b.target.locations.total_progression.unwrap_or(f64::MAX);
        pa.total_cmp(&pb)
    });
    let n = nombres(idioma);

    let mut salida = String::new();
    match formato {
        Formato::Markdown => {
            salida.push_str(&format!(
                "# {titulo}\n\n*{} {}*\n",
                lista.len(),
                n.anotaciones
            ));
        }
        Formato::Texto => {
            salida.push_str(&format!("{titulo}\n{} {}\n", lista.len(), n.anotaciones));
        }
    }

    let mut capitulo_anterior: Option<String> = None;
    for anotacion in &lista {
        let capitulo = anotacion
            .target
            .title
            .clone()
            .unwrap_or_else(|| n.sin_capitulo.to_owned());
        if capitulo_anterior.as_ref() != Some(&capitulo) {
            match formato {
                Formato::Markdown => salida.push_str(&format!("\n## {capitulo}\n")),
                Formato::Texto => salida.push_str(&format!("\n— {capitulo} —\n")),
            }
            capitulo_anterior = Some(capitulo);
        }
        let cita = anotacion
            .target
            .text
            .as_ref()
            .map(|t| t.highlight.trim().to_owned())
            .filter(|c| !c.is_empty());
        let nota = anotacion
            .body
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty());
        let sitio = [
            Some((n.clase)(anotacion.kind).to_owned()),
            porcentaje(anotacion),
            Some(localizador(anotacion)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        salida.push('\n');
        match formato {
            Formato::Markdown => {
                if let Some(cita) = &cita {
                    for linea in cita.lines() {
                        salida.push_str(&format!("> {linea}\n"));
                    }
                    salida.push('\n');
                }
                if let Some(nota) = nota {
                    salida.push_str(&format!("{nota}\n\n"));
                }
                // El localizador entre acentos graves: es un identificador, y en
                // Markdown así no se convierte en un enlace ni se le cambia nada.
                let (texto, maquina) = sitio.split_at(sitio.len() - 1);
                salida.push_str(&format!("*{}* · `{}`\n", texto.join(" · "), maquina[0]));
            }
            Formato::Texto => {
                if let Some(cita) = &cita {
                    salida.push_str(&format!("«{cita}»\n"));
                }
                if let Some(nota) = nota {
                    salida.push_str(&format!("{}: {nota}\n", n.nota));
                }
                salida.push_str(&format!("[{}]\n", sitio.join(" · ")));
            }
        }
    }
    Ok(salida)
}
