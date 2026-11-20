//! Buscar por significado en lo que se haya preparado.
//!
//! Contratos: `docs/architecture/ai/embeddings.md`, sección 4, y [ADR-007].
//!
//!
//! # Por qué es una búsqueda aparte y no un cambio en la léxica
//!
//! Porque el criterio es que **con la IA caída, la búsqueda léxica
//! sigue funcionando sin degradación**, y la forma de garantizarlo que no depende
//! de acordarse es no tocarla. La búsqueda léxica sigue siendo `search::search`, el
//! mismo comando y el mismo camino de siempre; esta es otra, que la interfaz pide
//! **después** y aparte, y cuyos resultados funde con los léxicos cuando llegan.
//! Si esta falla, tarda o no existe, lo léxico ya está en pantalla.
//!
//! # Lo que cuesta, y cuándo no se paga
//!
//! Calcular el vector de una consulta son unas décimas de segundo con el modelo
//! cargado, y un segundo y medio más si hay que cargarlo. **Si en el alcance no hay
//! ningún vector preparado, no se calcula nada**: se contesta vacío sin tocar el
//! modelo. Quien busca en un libro que nunca preparó no carga setecientos megabytes
//! para comparar la consulta con nada.
//!
//! [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md

use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use super::generacion::{Documentos, Vectorizador};
use super::indice::{self, Alcance, VectorIndex};
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::reader::Locator;
use crate::search::Scope;

/// Caracteres de la consulta que se usan. Una consulta es una pregunta, no un
/// documento: lo que pase de aquí es algo pegado por error.
const MAXIMO_DE_CONSULTA: usize = 1_000;

/// Caracteres del pasaje que se enseñan de un resultado.
const PASAJE: usize = 240;

/// Letras mínimas para buscar, las mismas que la búsqueda en el texto.
const MINIMO: usize = crate::search::MINIMO_PREFIJO;

/// Un fragmento cercano por significado a lo que se busca.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "SemanticHit")]
pub struct Cercania {
    pub edition_id: String,
    /// El archivo con el que se abre, si hay alguno disponible. Ver `search::Hit`.
    pub file_id: Option<String>,
    /// Dónde está. Es el mismo locator que tiene el fragmento en el índice léxico,
    /// y por eso la interfaz puede reconocer que un resultado sale de las dos
    /// búsquedas.
    pub locator: Locator,
    /// El principio del pasaje, **sin marcas**: aquí no hay palabras que marcar,
    /// porque no se ha encontrado por palabras.
    pub snippet: String,
    /// Coseno con la consulta: 1 es el mismo sentido.
    pub similarity: f32,
}

/// De dónde saca una búsqueda por significado lo que necesita.
///
/// Agrupado porque son las piezas de lo mismo —la base, el índice, quien calcula y
/// quien abre los documentos— y sueltas hacían pasar a la función del número de
/// parámetros que el proyecto admite.
pub struct Fuentes<'a> {
    pub db: &'a Database,
    pub indice: &'a dyn VectorIndex,
    pub vectorizador: &'a dyn Vectorizador,
    pub documentos: &'a dyn Documentos,
}

/// Busca por significado en un alcance.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si no hay modelo de embeddings listo: es lo
///   que la interfaz entiende como "no hay búsqueda por significado" y no enseña.
/// - [`NbError::DocumentNotFound`] si la colección no existe.
/// - [`NbError::StorageFailure`] si la base no responde.
pub fn por_significado(
    fuentes: &Fuentes<'_>,
    consulta: &str,
    alcance: &Scope,
    limite: usize,
) -> NbResult<Vec<Cercania>> {
    let consulta: String = consulta.trim().chars().take(MAXIMO_DE_CONSULTA).collect();
    if consulta.chars().count() < MINIMO {
        return Ok(Vec::new());
    }
    let Fuentes {
        db,
        indice: indice_vectorial,
        vectorizador,
        documentos,
    } = *fuentes;
    let modelo = vectorizador.modelo().ok_or(NbError::ComponentUnavailable)?;

    let conn = db.connection()?;
    let alcance = match alcance {
        Scope::Catalog => return Ok(Vec::new()),
        Scope::Document { edition_id } => Alcance::Ediciones(vec![edition_id.clone()]),
        Scope::Corpus {
            collection_id: Some(coleccion),
        } => Alcance::Ediciones(crate::collections::Collection::members(&conn, coleccion)?),
        Scope::Corpus {
            collection_id: None,
        } => Alcance::Todo,
    };
    drop(conn);

    if !indice_vectorial.hay(modelo, &alcance)? {
        return Ok(Vec::new());
    }

    let vector = vectorizador
        .vectores(&[consulta])?
        .into_iter()
        .next()
        .ok_or(NbError::Internal)?;
    let cercanos = indice_vectorial.buscar(modelo, &vector, &alcance, limite)?;

    let conn = db.connection()?;
    let mut abiertos: Vec<(String, Option<Box<dyn crate::reader::DocumentProvider>>)> = Vec::new();
    let mut resultados = Vec::with_capacity(cercanos.len());
    for cercano in cercanos {
        // Lo que devuelva el índice de un fragmento que ya no existe se descarta:
        // la fuente de verdad son los fragmentos, no los vectores.
        let Some((edition_id, serializado)) = indice::fragmento(&conn, &cercano.chunk_id)? else {
            continue;
        };
        let Ok(locator) = Locator::from_json(&serializado) else {
            continue;
        };
        let file_id: Option<String> = conn
            .query_row(
                "SELECT id FROM files WHERE edition_id = ?1 AND availability = 'available'
                  ORDER BY created_at LIMIT 1",
                [&edition_id],
                |fila| fila.get(0),
            )
            .optional()
            .map_err(|error| {
                tracing::error!(%error, "no se pudo leer el archivo de un resultado");
                NbError::StorageFailure
            })?;

        // Cada documento se abre una vez por búsqueda, aunque dé varios resultados.
        let posicion = match abiertos.iter().position(|(id, _)| *id == edition_id) {
            Some(i) => i,
            None => {
                abiertos.push((edition_id.clone(), documentos.abrir(&edition_id).ok()));
                abiertos.len() - 1
            }
        };
        let snippet = abiertos[posicion]
            .1
            .as_ref()
            .and_then(|doc| doc.text_range(&locator, PASAJE).ok())
            .map(|texto| pasaje(&texto))
            .unwrap_or_default();

        resultados.push(Cercania {
            edition_id,
            file_id,
            locator,
            snippet,
            similarity: cercano.similitud,
        });
    }
    Ok(resultados)
}

/// El principio de un pasaje, cortado en una palabra y con puntos suspensivos si
/// sigue.
fn pasaje(texto: &str) -> String {
    let limpio = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.chars().count() < PASAJE {
        return limpio;
    }
    let corte = limpio.rfind(' ').unwrap_or(limpio.len());
    format!("{}…", &limpio[..corte])
}

#[cfg(test)]
mod tests {
    use super::pasaje;

    #[test]
    fn el_pasaje_se_corta_en_una_palabra_y_dice_que_sigue() {
        assert_eq!(pasaje("corto  y\nlimpio"), "corto y limpio");
        let largo = "palabra ".repeat(40);
        let p = pasaje(&largo);
        assert!(p.ends_with("palabra…"), "{p}");
        assert!(!p.contains("  "));
    }
}
