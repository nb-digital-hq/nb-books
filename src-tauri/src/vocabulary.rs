//! El vocabulario consultado: qué palabras miró alguien, dónde y cuándo.
//!
//! Contrato: `docs/architecture/api/schemas.md`. Esquema:
//! `src-tauri/migrations/V15__vocabulary.sql`.
//!
//! # Por qué esto sale gratis
//!
//! Consultar una palabra ya ocurre: lo hace `lookup`. Este módulo no añade una
//! función que el usuario tenga que usar, apunta lo que la que ya existe
//! produjo. Ese es el motivo de que el vocabulario esté en el núcleo y no junto a
//! las tarjetas de estudio: las tarjetas necesitan saber de dónde
//! salen las palabras, y esta lista es esa respuesta.
//!
//! # Qué se apunta y qué no
//!
//! Se apunta **una consulta que encontró algo**, y solo de un diccionario. Una
//! consulta sin resultado no es vocabulario: es una palabra que no está en
//! ningún diccionario instalado, y llenar la lista de esas convertiría el
//! vocabulario en un registro de fallos. Una traducción tampoco entra: traducir
//! una frase entera no es aprender una palabra, y la lista dejaría de ser
//! estudiable en cuanto tuviera párrafos dentro.
//!
//! # El rebote
//!
//! Consultar dos veces seguidas la misma palabra en el mismo punto es un rebote
//! —se cerró el panel y se volvió a abrir—, no un dato. Se descarta al escribir
//! y no con un `UNIQUE`, porque la misma palabra en **otro** punto sí es un dato,
//! y es justo el que dice que la palabra cuesta.

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::reader::Locator;

/// Una palabra consultada, con dónde apareció.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct VocabularyEntry {
    pub id: String,
    /// El lema con el que respondió el diccionario.
    pub headword: String,
    /// La forma que se seleccionó en el texto. Casi nunca es el lema.
    pub term: String,
    pub edition_id: String,
    /// Quién respondió, para la procedencia.
    pub provider_id: String,
    /// Dónde apareció. Nunca un número: ver ADR-004.
    pub locator: Locator,
    pub created_at: String,
}

/// Una entrada con lo que hace falta para enseñarla fuera del documento.
///
/// Es la misma decisión que `LibraryAnnotation`: en la vista de Estudio no hay
/// un documento abierto, así que el título tiene que viajar con la entrada.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct VocabularyItem {
    pub entry: VocabularyEntry,
    /// Título de la edición donde apareció la palabra.
    pub title: String,
    /// Archivo con el que abrirla. **Nulo si ninguna copia está disponible**, y
    /// entonces la interfaz no ofrece el salto en lugar de fallar al pulsarlo.
    pub file_id: Option<String>,
}

const COLUMNAS: &str = "id, headword, term, edition_id, provider_id, locator, created_at";

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo en el almacenamiento del vocabulario");
    NbError::StorageFailure
}

impl VocabularyEntry {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let bruto: String = row.get("locator")?;
        Ok(Self {
            id: row.get("id")?,
            headword: row.get("headword")?,
            term: row.get("term")?,
            edition_id: row.get("edition_id")?,
            provider_id: row.get("provider_id")?,
            // Un locator ilegible es una fila corrupta, no una fila con la
            // posición a cero: se propaga como error de tipo en lugar de
            // devolver una posición inventada que llevaría al usuario a otro
            // sitio del libro sin decírselo.
            locator: serde_json::from_str(&bruto).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Apunta una palabra consultada.
///
/// Devuelve `None` cuando la consulta era un rebote de la anterior —la misma
/// palabra, la misma obra y el mismo punto—, que es lo que ocurre al cerrar y
/// volver a abrir el panel sin moverse.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
pub fn record(
    conn: &Connection,
    edition_id: &str,
    provider_id: &str,
    headword: &str,
    term: &str,
    locator: &Locator,
) -> NbResult<Option<VocabularyEntry>> {
    let serializado = serde_json::to_string(locator).map_err(|error| {
        tracing::error!(error = %error, "un locator no se dejó serializar");
        NbError::StorageFailure
    })?;

    let repetida: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM vocabulary
              WHERE edition_id = ?1 AND headword = ?2 AND locator = ?3
              ORDER BY created_at DESC LIMIT 1",
            rusqlite::params![edition_id, headword, &serializado],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| fallo(&e))?;
    if repetida.is_some() {
        return Ok(None);
    }

    conn.query_row(
        &format!(
            "INSERT INTO vocabulary (headword, term, edition_id, provider_id, locator)
             VALUES (?1, ?2, ?3, ?4, ?5) RETURNING {COLUMNAS}"
        ),
        rusqlite::params![headword, term, edition_id, provider_id, &serializado],
        VocabularyEntry::from_row,
    )
    .optional()
    .map_err(|e| fallo(&e))
}

/// Todo el vocabulario, de lo más reciente a lo más antiguo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn list(conn: &Connection, limite: u32, desde: u32) -> NbResult<Vec<VocabularyItem>> {
    let consulta = format!(
        "SELECT {columnas},
                e.title,
                (SELECT min(f.id) FROM files f
                  WHERE f.edition_id = e.id AND f.availability = 'available'
                ) AS archivo
           FROM vocabulary v
           JOIN editions e ON e.id = v.edition_id
          ORDER BY v.created_at DESC, v.id DESC
          LIMIT {limite} OFFSET {desde}",
        columnas = COLUMNAS
            .split(", ")
            .map(|c| format!("v.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut stmt = conn.prepare(&consulta).map_err(|e| fallo(&e))?;
    let filas = stmt
        .query_map([], |row| {
            Ok(VocabularyItem {
                entry: VocabularyEntry::from_row(row)?,
                title: row.get(7)?,
                file_id: row.get(8)?,
            })
        })
        .map_err(|e| fallo(&e))?;

    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Borra el vocabulario entero.
///
/// Es el criterio de aceptación: la lista es un registro de lo que el
/// usuario ha leído, así que tiene que poder deshacerse de ella de una vez y sin
/// ir palabra por palabra.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn clear(conn: &Connection) -> NbResult<usize> {
    let borradas = conn
        .execute("DELETE FROM vocabulary", [])
        .map_err(|e| fallo(&e))?;
    tracing::info!(borradas, "se borró el vocabulario consultado");
    Ok(borradas)
}

/// Borra una entrada. Que no existiera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn delete(conn: &Connection, id: &str) -> NbResult<()> {
    conn.execute("DELETE FROM vocabulary WHERE id = ?1", [id])
        .map_err(|e| fallo(&e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Lo que se consultó queda: exportarlo y repasarlo
// ---------------------------------------------------------------------------

/// Cuántas palabras como mucho salen en una exportación o en una tanda de
/// tarjetas. Cota: la exportación entera de quien consulta a diario durante años
/// cabe de sobra; más allá, se pide otra vez.
pub const MAXIMO_EXPORTADO: u32 = 20_000;

/// Cuántas tarjetas se hacen de una vez: las de una generación de estudio, para
/// que una tanda no pese más que las que ya se guardan.
pub const MAXIMO_DE_TARJETAS: u32 = crate::ai::cuaderno::MAXIMO_POR_GUARDADO as u32;

/// La plantilla con la que quedan apuntadas las tarjetas del vocabulario. No es un
/// texto que se le mande a un modelo: es cómo se sabe después de dónde salieron.
pub const PLANTILLA: &str = "vocabulario@1";

/// Lo que se ve de alrededor de una palabra: "…la tarde remota en que «su padre»
/// lo llevó…". Sale de la cita que guarda el Locator, que es donde se consultó.
#[must_use]
pub fn contexto(entrada: &VocabularyEntry) -> String {
    let Some(cita) = &entrada.locator.text else {
        return format!("«{}»", entrada.term);
    };
    let antes = cita.before.as_deref().unwrap_or("").trim_start();
    let despues = cita.after.as_deref().unwrap_or("").trim_end();
    let puntos_antes = if antes.is_empty() { "" } else { "…" };
    let puntos_despues = if despues.is_empty() { "" } else { "…" };
    format!(
        "{puntos_antes}{antes}«{}»{despues}{puntos_despues}",
        cita.highlight.trim()
    )
}

/// Un campo de CSV, entre comillas y con las de dentro dobladas, que es lo que
/// entiende cualquier hoja de cálculo y lo que Anki importa sin configurar nada.
fn csv(campo: &str) -> String {
    format!("\"{}\"", campo.replace('"', "\"\""))
}

/// El vocabulario entero en CSV, de lo más reciente a lo más antiguo.
///
/// Una fila por consulta, con la palabra como se seleccionó, el lema, el libro, el
/// contexto y la fecha. **Sin el Locator**: un archivo para una hoja de cálculo o
/// para Anki no sabe qué hacer con él, y la exportación de la biblioteca entera ya
/// lo lleva.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn exportar(conn: &Connection) -> NbResult<(String, usize)> {
    let entradas = list(conn, MAXIMO_EXPORTADO, 0)?;
    let mut salida = String::from("palabra,lema,libro,contexto,fecha,diccionario\n");
    for item in &entradas {
        let e = &item.entry;
        salida.push_str(
            &[
                csv(&e.term),
                csv(&e.headword),
                csv(&item.title),
                csv(&contexto(e)),
                csv(&e.created_at),
                csv(&e.provider_id),
            ]
            .join(","),
        );
        salida.push('\n');
    }
    Ok((salida, entradas.len()))
}

/// Qué pasó al hacer tarjetas con el vocabulario.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct VocabularyCards {
    /// Tarjetas nuevas.
    pub created: u32,
    /// Palabras que ya tenían su tarjeta: hacerlas otra vez sería repetirlas.
    pub already: u32,
    /// Palabras que el diccionario de aquí dentro ya no sabe definir -se consultaron
    /// en uno que se desinstaló, o en uno en línea-: sin definición no hay reverso.
    pub without_definition: u32,
}

/// Hace una tarjeta por palabra consultada, con la definición del diccionario
/// local en el reverso.
///
/// **Sin que el usuario copie nada y sin un modelo**: el anverso es la palabra en
/// el contexto donde se consultó, y el reverso lo que dice el mismo diccionario
/// que respondió entonces, preguntado otra vez **sin salir a la red**. La fuente de
/// la tarjeta es el Locator de la consulta, así que desde la tarjeta se vuelve al
/// párrafo donde apareció. Una palabra que ya tiene su tarjeta no se repite.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
pub fn a_tarjetas(
    conn: &mut Connection,
    registro: &crate::lookup::LookupRegistry,
) -> NbResult<VocabularyCards> {
    let entradas = list(conn, MAXIMO_EXPORTADO, 0)?;
    let mut informe = VocabularyCards::default();
    let mut nuevas = Vec::new();
    for item in entradas {
        if u32::try_from(nuevas.len()).unwrap_or(u32::MAX) >= MAXIMO_DE_TARJETAS {
            break;
        }
        let e = &item.entry;
        let anverso = contexto(e);
        let ya: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM study_cards
                   WHERE edition_id = ?1 AND template = ?2 AND front = ?3)",
                rusqlite::params![e.edition_id, PLANTILLA, anverso],
                |f| f.get(0),
            )
            .map_err(|error| fallo(&error))?;
        // La misma palabra en el mismo sitio, consultada dos veces, es una tarjeta.
        if ya
            || nuevas.iter().any(|n: &crate::ai::cuaderno::NuevaTarjeta| {
                n.edition_id == e.edition_id && n.front == anverso
            })
        {
            informe.already += 1;
            continue;
        }
        let consulta = crate::lookup::Query {
            term: e.headword.clone(),
            context: None,
            language: None,
        };
        let definicion = registro.lookup(&consulta, false).ok().and_then(|r| {
            r.entries
                .into_iter()
                .filter(|d| d.kind == crate::lookup::LookupKind::Dictionary)
                .min_by_key(|d| u8::from(d.provider_id != e.provider_id))
        });
        let Some(definicion) = definicion else {
            informe.without_definition += 1;
            continue;
        };
        let reverso: String = definicion.body.trim().chars().take(600).collect();
        if reverso.is_empty() {
            informe.without_definition += 1;
            continue;
        }
        nuevas.push(crate::ai::cuaderno::NuevaTarjeta {
            edition_id: e.edition_id.clone(),
            front: anverso,
            back: format!("{}: {reverso}", definicion.headword),
            options: None,
            correct: None,
            sources: vec![e.locator.clone()],
            provider: definicion
                .source
                .unwrap_or_else(|| definicion.provider_id.clone()),
            template_id: String::new(),
            template_version: 0,
        });
    }
    informe.created = crate::ai::cuaderno::guardar_tarjetas_de(conn, &nuevas, PLANTILLA)?;
    Ok(informe)
}
