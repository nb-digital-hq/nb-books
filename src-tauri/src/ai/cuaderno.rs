//! El cuaderno de estudio: las tarjetas, las preguntas y los conceptos que alguien
//! decidió quedarse de lo generado.
//!
//! Contrato: `docs/architecture/ai/prompts.md` y `migrations/V34__estudio_generado.sql`.
//!
//!
//! # Lo que llega del webview no se cree
//!
//! Lo que se guarda lo manda la interfaz, que es zona no confiable, y lo que manda
//! salió de un modelo, que tampoco lo es. Así que al guardar se aplican las mismas
//! reglas que al leer la salida ([`super::salidas`]): campos no vacíos y con tope,
//! preguntas con opciones distintas y una clave que exista, como mucho
//! [`super::estudio::MAXIMO_DE_FRAGMENTOS`] fuentes, y una plantilla que exista.
//! Un valor que no cumple no se recorta en silencio: se rechaza, porque quien lo
//! mandó es el producto y un rechazo es un defecto que tiene que verse.

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::estudio::MAXIMO_DE_FRAGMENTOS;
use super::salidas::{normalizar, TipoDeConcepto, MAXIMO_DE_CAMPO};
use crate::error::{NbError, NbResult};
use crate::reader::Locator;

/// Cuántos elementos se guardan de una vez: los de una generación.
pub const MAXIMO_POR_GUARDADO: usize = super::salidas::MAXIMO_DE_ELEMENTOS;

/// Una tarjeta o una pregunta para guardar.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "NewStudyCard")]
pub struct NuevaTarjeta {
    pub edition_id: String,
    /// La respuesta de una tarjeta, o la explicación de una pregunta.
    pub front: String,
    pub back: String,
    /// Las opciones de una pregunta; ninguna en una tarjeta.
    pub options: Option<Vec<String>>,
    pub correct: Option<u32>,
    pub sources: Vec<Locator>,
    pub provider: String,
    pub template_id: String,
    pub template_version: u32,
}

/// Una tarjeta o una pregunta guardada.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "StudyCard")]
pub struct Tarjeta {
    pub id: String,
    pub edition_id: String,
    /// Título de la edición, para enseñarla fuera del libro.
    pub title: String,
    /// Archivo con el que abrir sus fuentes; nulo si no hay copia disponible.
    pub file_id: Option<String>,
    pub front: String,
    pub back: String,
    pub options: Option<Vec<String>>,
    pub correct: Option<u32>,
    pub sources: Vec<Locator>,
    /// Quién la generó. Sigue ahí aunque se edite.
    pub provider: String,
    /// `estudio.tarjetas@1`.
    pub template: String,
    pub edited_at: Option<String>,
    pub created_at: String,
    pub due_at: String,
    pub interval_days: u32,
    pub ease_factor: f64,
    pub repetitions: u32,
    pub last_reviewed_at: Option<String>,
}

/// Cantidad total de tarjetas y cuántas tocan hoy para repasar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyCardCounts"
)]
pub struct CuentasDeTarjetas {
    pub total: u32,
    pub due: u32,
}

/// Un concepto para guardar.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "NewConcept")]
pub struct NuevoConcepto {
    pub edition_id: String,
    pub name: String,
    pub kind: TipoDeConcepto,
    pub description: String,
    pub sources: Vec<Locator>,
    pub provider: String,
    pub template_id: String,
    pub template_version: u32,
}

/// Un concepto guardado.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "Concept")]
pub struct Concepto {
    pub id: String,
    pub edition_id: String,
    pub title: String,
    pub file_id: Option<String>,
    pub name: String,
    pub kind: TipoDeConcepto,
    pub description: String,
    pub sources: Vec<Locator>,
    pub provider: String,
    pub template: String,
    pub edited_at: Option<String>,
    pub created_at: String,
    /// En cuántos **otros** libros aparece un concepto con el mismo nombre. Es la
    /// arista del grafo de conocimiento personal, consultada y no guardada, por lo
    /// mismo que los retroenlaces de las notas.
    pub other_editions: u32,
}

/// Lo que se cambia al editar una tarjeta o una pregunta.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyCardEdit"
)]
pub struct EdicionDeTarjeta {
    pub front: String,
    pub back: String,
    pub options: Option<Vec<String>>,
    pub correct: Option<u32>,
}

/// Cuántas tarjetas y conceptos hay guardados, para las pestañas de Estudio
///. Dos `count(*)` sobre tablas que crecen al estudiar, no con la
/// biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct StudyCounts {
    pub cards: u32,
    pub concepts: u32,
}

/// Ver [`StudyCounts`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn cuentas(conn: &Connection) -> NbResult<StudyCounts> {
    conn.query_row(
        "SELECT (SELECT count(*) FROM study_cards), (SELECT count(*) FROM concepts)",
        [],
        |fila| {
            Ok(StudyCounts {
                cards: u32::try_from(fila.get::<_, i64>(0)?).unwrap_or(u32::MAX),
                concepts: u32::try_from(fila.get::<_, i64>(1)?).unwrap_or(u32::MAX),
            })
        },
    )
    .map_err(|e| fallo(&e))
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo en el almacenamiento del cuaderno de estudio");
    NbError::StorageFailure
}

fn rechazo(motivo: &'static str) -> NbError {
    tracing::warn!(motivo, "se rechazó guardar algo generado");
    NbError::Internal
}

/// Un campo de texto que cumple, limpio.
fn campo(texto: &str, vacio_admitido: bool) -> NbResult<String> {
    let limpio = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.is_empty() && !vacio_admitido {
        return Err(rechazo("campo vacío"));
    }
    if limpio.chars().count() > MAXIMO_DE_CAMPO {
        return Err(rechazo("campo demasiado largo"));
    }
    Ok(limpio)
}

fn plantilla_de(id: &str, version: u32) -> NbResult<String> {
    let plantilla = super::plantillas::plantilla(id)?;
    if version == 0 || version > plantilla.version {
        return Err(rechazo("versión de plantilla que no existe"));
    }
    Ok(format!("{id}@{version}"))
}

fn fuentes(locators: &[Locator]) -> NbResult<String> {
    if locators.len() > MAXIMO_DE_FRAGMENTOS {
        return Err(rechazo("demasiadas fuentes"));
    }
    serde_json::to_string(&serde_json::json!({ "locators": locators }))
        .map_err(|_| NbError::Internal)
}

fn leer_fuentes(bruto: &str) -> Vec<Locator> {
    // Una fuente ilegible no tumba la tarjeta: se enseña sin ella, que es lo mismo
    // que no verificable.
    serde_json::from_str::<serde_json::Value>(bruto)
        .ok()
        .and_then(|v| v.get("locators").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

/// Opciones y clave de una pregunta, comprobadas, como se guardan.
fn eleccion(options: Option<&Vec<String>>, correct: Option<u32>) -> NbResult<Option<String>> {
    match (options, correct) {
        (None, None) => Ok(None),
        (Some(opciones), Some(clave)) => {
            if !(3..=5).contains(&opciones.len()) {
                return Err(rechazo("número de opciones"));
            }
            let limpias = opciones
                .iter()
                .map(|o| campo(o, false))
                .collect::<NbResult<Vec<_>>>()?;
            let mut distintas: Vec<String> = limpias.iter().map(|o| normalizar(o)).collect();
            distintas.sort_unstable();
            distintas.dedup();
            if distintas.len() != limpias.len() || clave as usize >= limpias.len() {
                return Err(rechazo("opciones repetidas o clave fuera de rango"));
            }
            Ok(Some(
                serde_json::json!({ "options": limpias, "correct": clave }).to_string(),
            ))
        }
        _ => Err(rechazo("pregunta a medias")),
    }
}

/// Guarda tarjetas y preguntas, todas o ninguna.
///
/// # Errors
///
/// - [`NbError::Internal`] si alguna no cumple.
/// - [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
pub fn guardar_tarjetas(conn: &mut Connection, nuevas: &[NuevaTarjeta]) -> NbResult<Vec<String>> {
    guardar(conn, nuevas, |n| {
        plantilla_de(&n.template_id, n.template_version)
    })
}

/// Guarda tarjetas que **no** salen de una plantilla de estudio, con la procedencia
/// que se diga: las del vocabulario consultado, que se hacen con el
/// diccionario y no con un modelo. Devuelve cuántas se guardaron.
///
/// # Errors
///
/// Los de [`guardar_tarjetas`].
pub fn guardar_tarjetas_de(
    conn: &mut Connection,
    nuevas: &[NuevaTarjeta],
    plantilla: &str,
) -> NbResult<u32> {
    if nuevas.is_empty() {
        return Ok(0);
    }
    let ids = guardar(conn, nuevas, |_| Ok(plantilla.to_owned()))?;
    Ok(u32::try_from(ids.len()).unwrap_or(u32::MAX))
}

fn guardar(
    conn: &mut Connection,
    nuevas: &[NuevaTarjeta],
    plantilla: impl Fn(&NuevaTarjeta) -> NbResult<String>,
) -> NbResult<Vec<String>> {
    if nuevas.len() > MAXIMO_POR_GUARDADO {
        return Err(rechazo("demasiadas de una vez"));
    }
    let tx = crate::db::write_transaction(conn)?;
    let mut ids = Vec::with_capacity(nuevas.len());
    for nueva in nuevas {
        let choices = eleccion(nueva.options.as_ref(), nueva.correct)?;
        let kind = if choices.is_some() {
            "question"
        } else {
            "flashcard"
        };
        let front = campo(&nueva.front, false)?;
        let back = campo(&nueva.back, kind == "question")?;
        if kind == "flashcard" && normalizar(&front) == normalizar(&back) {
            return Err(rechazo("la respuesta repite la pregunta"));
        }
        let id: String = tx
            .query_row(
                "INSERT INTO study_cards (edition_id, kind, front, back, choices, sources, provider, template,
                                          due_at, interval_days, ease_factor, repetitions)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 0, 2.5, 0) RETURNING id",
                rusqlite::params![
                    nueva.edition_id,
                    kind,
                    front,
                    back,
                    choices,
                    fuentes(&nueva.sources)?,
                    campo(&nueva.provider, false)?,
                    plantilla(nueva)?,
                ],
                |fila| fila.get(0),
            )
            .map_err(|e| fallo(&e))?;
        ids.push(id);
    }
    tx.commit().map_err(|e| fallo(&e))?;
    Ok(ids)
}

/// Edita una tarjeta o una pregunta. **No cambia su procedencia**: marca que se
/// editó. Que no exista es [`NbError::DocumentNotFound`].
///
/// Una tarjeta no se convierte en pregunta ni al revés: son dos formas de repasar,
/// y cambiar de una a otra es crear otra cosa.
///
/// # Errors
///
/// - [`NbError::Internal`] si no cumple.
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn editar_tarjeta(conn: &Connection, id: &str, edicion: &EdicionDeTarjeta) -> NbResult<()> {
    let kind: Option<String> = conn
        .query_row("SELECT kind FROM study_cards WHERE id = ?1", [id], |f| {
            f.get(0)
        })
        .optional()
        .map_err(|e| fallo(&e))?;
    let kind = kind.ok_or(NbError::DocumentNotFound)?;
    let choices = eleccion(edicion.options.as_ref(), edicion.correct)?;
    if (kind == "question") != choices.is_some() {
        return Err(rechazo("una tarjeta no cambia de clase al editarla"));
    }
    let front = campo(&edicion.front, false)?;
    let back = campo(&edicion.back, kind == "question")?;
    conn.execute(
        "UPDATE study_cards
            SET front = ?2, back = ?3, choices = ?4,
                edited_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ?1",
        rusqlite::params![id, front, back, choices],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Borra una tarjeta o una pregunta. Que no existiera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn borrar_tarjeta(conn: &Connection, id: &str) -> NbResult<()> {
    conn.execute("DELETE FROM study_cards WHERE id = ?1", [id])
        .map_err(|e| fallo(&e))?;
    Ok(())
}

fn tarjeta_de(fila: &Row<'_>) -> rusqlite::Result<Tarjeta> {
    let choices: Option<String> = fila.get("choices")?;
    let eleccion = choices.and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok());
    Ok(Tarjeta {
        id: fila.get("id")?,
        edition_id: fila.get("edition_id")?,
        title: fila.get("title")?,
        file_id: fila.get("file_id")?,
        front: fila.get("front")?,
        back: fila.get("back")?,
        options: eleccion
            .as_ref()
            .and_then(|e| serde_json::from_value(e.get("options")?.clone()).ok()),
        correct: eleccion
            .as_ref()
            .and_then(|e| e.get("correct")?.as_u64())
            .and_then(|n| u32::try_from(n).ok()),
        sources: leer_fuentes(&fila.get::<_, String>("sources")?),
        provider: fila.get("provider")?,
        template: fila.get("template")?,
        edited_at: fila.get("edited_at")?,
        created_at: fila.get("created_at")?,
        due_at: fila.get("due_at")?,
        interval_days: fila.get("interval_days")?,
        ease_factor: fila.get("ease_factor")?,
        repetitions: fila.get("repetitions")?,
        last_reviewed_at: fila.get("last_reviewed_at")?,
    })
}

const ARCHIVO: &str = "(SELECT min(f.id) FROM files f
                         WHERE f.edition_id = e.id AND f.availability = 'available')";

/// Una tarjeta o pregunta concreta por su identificador.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si la consulta falla.
pub fn tarjeta(conn: &Connection, id: &str) -> NbResult<Tarjeta> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT c.*, e.title, {ARCHIVO} AS file_id
               FROM study_cards c JOIN editions e ON e.id = c.edition_id
              WHERE c.id = ?1"
        ))
        .map_err(|e| fallo(&e))?;
    stmt.query_row([id], tarjeta_de)
        .optional()
        .map_err(|e| fallo(&e))?
        .ok_or(NbError::DocumentNotFound)
}

/// Las tarjetas y preguntas, de las más recientes a las más antiguas, de un libro
/// o de todos. Si `solo_pendientes` es verdadero, filtra por las que ya alcanzaron
/// su fecha de repaso (`due_at <= now`) y las ordena por vencimiento más urgente.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn tarjetas(
    conn: &Connection,
    edition_id: Option<&str>,
    solo_pendientes: bool,
    limite: u32,
    desde: u32,
) -> NbResult<Vec<Tarjeta>> {
    let orden = if solo_pendientes {
        "c.due_at ASC, c.created_at ASC, c.id ASC"
    } else {
        "c.created_at DESC, c.id DESC"
    };
    let sql = format!(
        "SELECT c.*, e.title, {ARCHIVO} AS file_id
           FROM study_cards c JOIN editions e ON e.id = c.edition_id
          WHERE (?1 IS NULL OR c.edition_id = ?1)
            AND (?2 = 0 OR c.due_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
          ORDER BY {orden}
          LIMIT ?3 OFFSET ?4"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| fallo(&e))?;
    let pendientes_int: i32 = if solo_pendientes { 1 } else { 0 };
    let filas = stmt
        .query_map(
            rusqlite::params![edition_id, pendientes_int, limite, desde],
            tarjeta_de,
        )
        .map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| fallo(&e))
}

/// Cuenta el total de tarjetas y cuántas tocan hoy para repasar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn cuentas_de_tarjetas(
    conn: &Connection,
    edition_id: Option<&str>,
) -> NbResult<CuentasDeTarjetas> {
    let (total, due) = conn
        .query_row(
            "SELECT count(*),
                    coalesce(sum(CASE WHEN due_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now') THEN 1 ELSE 0 END), 0)
               FROM study_cards
              WHERE ?1 IS NULL OR edition_id = ?1",
            [edition_id],
            |f| Ok((f.get(0)?, f.get(1)?)),
        )
        .map_err(|e| fallo(&e))?;
    Ok(CuentasDeTarjetas { total, due })
}

/// Registra la respuesta a una tarjeta o pregunta, recalculando su próximo repaso
/// según SM-2 determinista.
///
/// Usa una transacción inmediata según el contrato de concurrencia de la base de datos.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no existe la tarjeta.
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn responder_tarjeta(
    conn: &mut Connection,
    id: &str,
    calificacion: super::repaso_espaciado::CalificacionRepaso,
) -> NbResult<Tarjeta> {
    let tx = crate::db::write_transaction(conn)?;
    let actual: Option<(u32, u32, f64)> = tx
        .query_row(
            "SELECT repetitions, interval_days, ease_factor FROM study_cards WHERE id = ?1",
            [id],
            |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)),
        )
        .optional()
        .map_err(|e| fallo(&e))?;
    let (repetitions, interval_days, ease_factor) = actual.ok_or(NbError::DocumentNotFound)?;

    let estado_actual = super::repaso_espaciado::EstadoRepaso {
        repetitions,
        interval_days,
        ease_factor,
    };
    let nuevo_estado = super::repaso_espaciado::calcular_repaso(estado_actual, calificacion);

    let ahora: String = tx
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |f| {
            f.get(0)
        })
        .map_err(|e| fallo(&e))?;
    let proximo_due = super::repaso_espaciado::sumar_dias(&ahora, nuevo_estado.interval_days)?;

    tx.execute(
        "UPDATE study_cards
            SET repetitions = ?2,
                interval_days = ?3,
                ease_factor = ?4,
                due_at = ?5,
                last_reviewed_at = ?6
          WHERE id = ?1",
        rusqlite::params![
            id,
            nuevo_estado.repetitions,
            nuevo_estado.interval_days,
            nuevo_estado.ease_factor,
            proximo_due,
            ahora,
        ],
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))?;

    tarjeta(conn, id)
}

/// Guarda conceptos. Uno que ya estaba en ese libro **junta sus fuentes** en vez de
/// duplicarse, y conserva su descripción si ya la tenía editada.
///
/// # Errors
///
/// - [`NbError::Internal`] si alguno no cumple.
/// - [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
pub fn guardar_conceptos(conn: &mut Connection, nuevos: &[NuevoConcepto]) -> NbResult<Vec<String>> {
    if nuevos.len() > MAXIMO_POR_GUARDADO {
        return Err(rechazo("demasiados de una vez"));
    }
    let tx = crate::db::write_transaction(conn)?;
    let mut ids = Vec::with_capacity(nuevos.len());
    for nuevo in nuevos {
        let name = campo(&nuevo.name, false)?;
        if name.chars().count() > 120 {
            return Err(rechazo("nombre de concepto demasiado largo"));
        }
        let description = campo(&nuevo.description, true)?;
        let template = plantilla_de(&nuevo.template_id, nuevo.template_version)?;
        let provider = campo(&nuevo.provider, false)?;

        let previo: Option<(String, String, Option<String>)> = tx
            .query_row(
                "SELECT id, sources, edited_at FROM concepts WHERE edition_id = ?1 AND name = ?2",
                rusqlite::params![nuevo.edition_id, name],
                |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)),
            )
            .optional()
            .map_err(|e| fallo(&e))?;

        let id = if let Some((id, bruto, editado)) = previo {
            let mut todas = leer_fuentes(&bruto);
            for locator in &nuevo.sources {
                if !todas.contains(locator) {
                    todas.push(locator.clone());
                }
            }
            todas.truncate(MAXIMO_DE_FRAGMENTOS);
            tx.execute(
                "UPDATE concepts
                    SET sources = ?2,
                        description = CASE WHEN ?4 THEN description ELSE ?3 END
                  WHERE id = ?1",
                rusqlite::params![id, fuentes(&todas)?, description, editado.is_some()],
            )
            .map_err(|e| fallo(&e))?;
            id
        } else {
            tx.query_row(
                "INSERT INTO concepts (edition_id, name, kind, description, sources, provider, template)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id",
                rusqlite::params![
                    nuevo.edition_id,
                    name,
                    nuevo.kind.as_str(),
                    description,
                    fuentes(&nuevo.sources)?,
                    provider,
                    template,
                ],
                |f| f.get(0),
            )
            .map_err(|e| fallo(&e))?
        };
        ids.push(id);
    }
    tx.commit().map_err(|e| fallo(&e))?;
    Ok(ids)
}

/// Edita el nombre, la clase y la descripción de un concepto.
///
/// # Errors
///
/// - [`NbError::Internal`] si no cumple.
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si la escritura falla, o si el nombre nuevo ya lo
///   tiene otro concepto del mismo libro.
pub fn editar_concepto(
    conn: &Connection,
    id: &str,
    name: &str,
    kind: TipoDeConcepto,
    description: &str,
) -> NbResult<()> {
    let name = campo(name, false)?;
    let description = campo(description, true)?;
    let cambiadas = conn
        .execute(
            "UPDATE concepts
                SET name = ?2, kind = ?3, description = ?4,
                    edited_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              WHERE id = ?1",
            rusqlite::params![id, name, kind.as_str(), description],
        )
        .map_err(|e| fallo(&e))?;
    if cambiadas == 0 {
        return Err(NbError::DocumentNotFound);
    }
    Ok(())
}

/// Borra un concepto. Que no existiera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn borrar_concepto(conn: &Connection, id: &str) -> NbResult<()> {
    conn.execute("DELETE FROM concepts WHERE id = ?1", [id])
        .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Los conceptos, por nombre, de un libro o de todos.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn conceptos(
    conn: &Connection,
    edition_id: Option<&str>,
    limite: u32,
    desde: u32,
) -> NbResult<Vec<Concepto>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT c.*, e.title, {ARCHIVO} AS file_id,
                    (SELECT count(DISTINCT o.edition_id) FROM concepts o
                      WHERE o.name = c.name AND o.edition_id <> c.edition_id) AS otros
               FROM concepts c JOIN editions e ON e.id = c.edition_id
              WHERE ?1 IS NULL OR c.edition_id = ?1
              ORDER BY c.name, e.title
              LIMIT ?2 OFFSET ?3"
        ))
        .map_err(|e| fallo(&e))?;
    let filas = stmt
        .query_map(rusqlite::params![edition_id, limite, desde], |f| {
            let kind: String = f.get("kind")?;
            Ok(Concepto {
                id: f.get("id")?,
                edition_id: f.get("edition_id")?,
                title: f.get("title")?,
                file_id: f.get("file_id")?,
                name: f.get("name")?,
                kind: TipoDeConcepto::de(&kind).unwrap_or(TipoDeConcepto::Concepto),
                description: f.get("description")?,
                sources: leer_fuentes(&f.get::<_, String>("sources")?),
                provider: f.get("provider")?,
                template: f.get("template")?,
                edited_at: f.get("edited_at")?,
                created_at: f.get("created_at")?,
                other_editions: f.get("otros")?,
            })
        })
        .map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| fallo(&e))
}
