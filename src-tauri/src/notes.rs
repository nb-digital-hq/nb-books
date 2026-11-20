//! Notas propias del usuario, enlazadas entre sí.
//!
//! Contrato: `docs/architecture/api/schemas.md`. Esquema:
//! `src-tauri/migrations/V12__linked_notes.sql`.
//!
//! # Qué las distingue de una anotación
//!
//! Una anotación cuelga de un documento y se ancla a un punto de él. Una nota no
//! ancla a nada: es del usuario y existe por sí sola. Sin ellas, lo único que
//! NB Books sabe guardar es lo que alguien escribió *sobre* un libro, y no lo que
//! piensa a partir de varios.
//!
//! Hay además un motivo que no es de simetría, y está medido: la investigación
//! de `docs/design/reading-experience/README.md`, sección 1.1, dice que
//! resaltar por sí solo no mejora el aprendizaje y que quien lee en digital casi
//! no subraya. Lo que funciona es escribir algo propio.
//!
//! # Por qué una nota sí se edita
//!
//! Una anotación es inmutable porque ancla a un texto que puede moverse, y
//! porque sobrescribirla pierde trabajo de lectura sin dejar rastro. Una nota es
//! un escrito propio, y un escrito propio se reescribe: imponerle inmutabilidad
//! convertiría cada errata corregida en una versión nueva.
//!
//! # Los retroenlaces no se guardan
//!
//! Un enlace es una fila; el retroenlace es esa misma fila leída del otro lado.
//! Guardarlo aparte permitiría que las dos direcciones dejaran de coincidir, y
//! esa clase de incoherencia no se detecta hasta que ya está extendida. Es
//! también lo que hace cierto el criterio de aceptación: enlazar A con B hace
//! aparecer A en los retroenlaces de B **sin ninguna escritura adicional**.

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// Una nota del usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Note {
    pub id: String,
    /// Es también el nombre por el que otras notas la enlazan, así que es único.
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

const COLUMNAS: &str = "id, title, body, created_at, updated_at";

/// Las mismas columnas, cualificadas.
///
/// Hacen falta en cuanto hay un `JOIN` con `note_links`, que también tiene
/// `created_at`: sin cualificar, SQLite rechaza la consulta por columna
/// ambigua. Lo descubrió la prueba de retroenlaces al primer intento.
const COLUMNAS_N: &str = "n.id, n.title, n.body, n.created_at, n.updated_at";

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo en el almacenamiento de notas");
    NbError::StorageFailure
}

impl Note {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            body: row.get("body")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// Los títulos que un cuerpo enlaza, en orden de aparición y sin repetir.
///
/// La sintaxis es `[[Título]]`. Se enlaza por **título** y no por identificador
/// porque el título es lo único que el usuario escribe y recuerda; un
/// identificador no lo teclea nadie. Es también el motivo de que el título sea
/// único, y lo que hace cierto el comentario de [`find_by_title`].
fn titulos_enlazados(body: &str) -> Vec<String> {
    let mut titulos: Vec<String> = Vec::new();
    let mut resto = body;

    while let Some(apertura) = resto.find("[[") {
        resto = &resto[apertura + 2..];
        let Some(cierre) = resto.find("]]") else {
            // Un corchete sin cerrar no es un enlace a medias: no es un enlace.
            break;
        };
        let titulo = resto[..cierre].trim();
        if !titulo.is_empty() && !titulos.iter().any(|t| t == titulo) {
            titulos.push(titulo.to_owned());
        }
        resto = &resto[cierre + 2..];
    }

    titulos
}

/// Rehace los enlaces que **salen** de una nota a partir de su cuerpo.
///
/// El cuerpo es la única declaración de un enlace, así que reescribirlo es
/// también reescribir el grafo: un `[[Título]]` que desaparece del texto tiene
/// que desaparecer de la tabla, o el retroenlace seguiría enseñando una relación
/// que el usuario ya borró.
///
/// Se calcula la diferencia en lugar de vaciar y rellenar porque un enlace tiene
/// fecha de creación: borrarlo y volverlo a insertar en cada guardado la
/// falsearía en cada pulsación de teclado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna de las consultas falla.
fn reconciliar_salientes(conn: &Connection, source_id: &str, body: &str) -> NbResult<()> {
    let mut deseados: Vec<String> = Vec::new();
    for titulo in titulos_enlazados(body) {
        // Un título que todavía no existe no es un error: es una nota que quizá
        // se escriba después, y `reconciliar_entrantes` la recogerá entonces.
        if let Some(destino) = find_by_title(conn, &titulo)? {
            // Enlazarse a sí misma no dice nada y ensuciaría los retroenlaces.
            if destino.id != source_id {
                deseados.push(destino.id);
            }
        }
    }

    let actuales: Vec<String> = outgoing(conn, source_id)?
        .into_iter()
        .map(|n| n.id)
        .collect();

    for sobrante in actuales.iter().filter(|id| !deseados.contains(id)) {
        unlink(conn, source_id, sobrante)?;
    }
    for nuevo in deseados.iter().filter(|id| !actuales.contains(id)) {
        link(conn, source_id, nuevo)?;
    }

    Ok(())
}

/// Vuelve a resolver los enlaces que **nombran** alguno de estos títulos.
///
/// Hace falta en dos momentos, y por el mismo motivo. Un `[[Título]]` escrito
/// antes de que esa nota exista no apunta a nada; y renombrar una nota deja
/// colgando a todas las que la nombraban. Sin esta pasada el enlace se pierde en
/// silencio y no vuelve nunca, ni siquiera cuando el destino aparece: es
/// exactamente la clase de pérdida que el producto existe para no tener.
///
/// Recorre todas las notas porque son las del usuario, no las del catálogo: un
/// índice invertido sobre cientos de filas costaría más de mantener que de
/// evitar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna de las consultas falla.
fn reconciliar_entrantes(conn: &Connection, titulos: &[&str]) -> NbResult<()> {
    for nota in todas(conn)? {
        let nombra = titulos_enlazados(&nota.body);
        if titulos.iter().any(|t| nombra.iter().any(|n| n == t)) {
            reconciliar_salientes(conn, &nota.id, &nota.body)?;
        }
    }
    Ok(())
}

/// Crea una nota, con los enlaces que su cuerpo declare.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si el título está vacío o ya existe, o si falla
/// la escritura de los enlaces. El título es único porque es el nombre con el
/// que se la enlaza: dos notas con el mismo nombre harían ambiguo cualquier
/// enlace.
pub fn create(conn: &Connection, title: &str, body: &str) -> NbResult<Note> {
    let note = conn
        .query_row(
            &format!("INSERT INTO notes (title, body) VALUES (?1, ?2) RETURNING {COLUMNAS}"),
            rusqlite::params![title, body],
            Note::from_row,
        )
        .map_err(|e| fallo(&e))?;

    reconciliar_salientes(conn, &note.id, body)?;
    // Y las que ya la nombraban antes de que existiera.
    reconciliar_entrantes(conn, &[&note.title])?;

    Ok(note)
}

/// Busca una nota por identificador.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla. Que no exista no es error.
pub fn find(conn: &Connection, id: &str) -> NbResult<Option<Note>> {
    conn.query_row(
        &format!("SELECT {COLUMNAS} FROM notes WHERE id = ?1"),
        [id],
        Note::from_row,
    )
    .optional()
    .map_err(|e| fallo(&e))
}

/// Busca una nota por su título, que es como la nombra un enlace escrito a mano.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn find_by_title(conn: &Connection, title: &str) -> NbResult<Option<Note>> {
    conn.query_row(
        &format!("SELECT {COLUMNAS} FROM notes WHERE title = ?1"),
        [title],
        Note::from_row,
    )
    .optional()
    .map_err(|e| fallo(&e))
}

/// Todas las notas, en orden estable.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn list(conn: &Connection, limite: u32, desde: u32) -> NbResult<Vec<Note>> {
    consultar(
        conn,
        // Los dos números son enteros sin signo del runtime, no texto de fuera.
        &format!("SELECT {COLUMNAS} FROM notes ORDER BY title, id LIMIT {limite} OFFSET {desde}"),
        &[],
    )
}

/// Todas las notas, sin cota. **No cruza el puente**, y por eso puede no
/// tenerla: la usa la reconciliación de enlaces, que necesita mirarlas todas
/// para saber cuáles nombran un título. Ver [`reconciliar_entrantes`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
fn todas(conn: &Connection) -> NbResult<Vec<Note>> {
    consultar(
        conn,
        &format!("SELECT {COLUMNAS} FROM notes ORDER BY title, id"),
        &[],
    )
}

/// Reescribe título y cuerpo, y con ellos los enlaces que el cuerpo declara.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la nota no existe.
/// - [`NbError::StorageFailure`] si el título nuevo choca con otro, o si falla
///   la escritura de los enlaces.
pub fn update(conn: &Connection, id: &str, title: &str, body: &str) -> NbResult<Note> {
    // El título de antes se lee ahora porque después ya no existe, y hace falta
    // para reenganchar a quien la nombraba por el nombre viejo.
    let anterior = find(conn, id)?.ok_or(NbError::DocumentNotFound)?;

    let note = conn
        .query_row(
            &format!(
                "UPDATE notes SET title = ?2, body = ?3,
                              updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 RETURNING {COLUMNAS}"
            ),
            rusqlite::params![id, title, body],
            Note::from_row,
        )
        .optional()
        .map_err(|e| fallo(&e))?
        .ok_or(NbError::DocumentNotFound)?;

    reconciliar_salientes(conn, id, body)?;
    if anterior.title != note.title {
        reconciliar_entrantes(conn, &[&anterior.title, &note.title])?;
    }

    Ok(note)
}

/// Borra una nota y, con ella, sus enlaces en los dos sentidos.
///
/// La cascada la hace la base, no este código: es lo que garantiza que borrar
/// una nota no deje ningún retroenlace apuntando al vacío aunque alguien entre
/// por SQL directo.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si la nota no existe.
pub fn delete(conn: &Connection, id: &str) -> NbResult<()> {
    let filas = conn
        .execute("DELETE FROM notes WHERE id = ?1", [id])
        .map_err(|e| fallo(&e))?;

    if filas == 0 {
        return Err(NbError::DocumentNotFound);
    }
    Ok(())
}

/// Enlaza una nota con otra. Repetir el enlace no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna de las dos no existe, o si se intenta
/// enlazar una nota consigo misma.
pub fn link(conn: &Connection, source_id: &str, target_id: &str) -> NbResult<()> {
    conn.execute(
        "INSERT INTO note_links (source_id, target_id) VALUES (?1, ?2)
         ON CONFLICT DO NOTHING",
        [source_id, target_id],
    )
    .map(|_| ())
    .map_err(|e| fallo(&e))
}

/// Deshace un enlace. Que no existiera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la operación falla.
pub fn unlink(conn: &Connection, source_id: &str, target_id: &str) -> NbResult<()> {
    conn.execute(
        "DELETE FROM note_links WHERE source_id = ?1 AND target_id = ?2",
        [source_id, target_id],
    )
    .map(|_| ())
    .map_err(|e| fallo(&e))
}

/// Notas a las que esta apunta.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn outgoing(conn: &Connection, note_id: &str) -> NbResult<Vec<Note>> {
    consultar(
        conn,
        &format!(
            "SELECT {COLUMNAS_N} FROM notes n
              JOIN note_links l ON l.target_id = n.id
             WHERE l.source_id = ?1 ORDER BY n.title, n.id"
        ),
        &[&note_id],
    )
}

/// Notas que apuntan a esta. **No se guardan: se consultan.**
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn backlinks(conn: &Connection, note_id: &str) -> NbResult<Vec<Note>> {
    consultar(
        conn,
        &format!(
            "SELECT {COLUMNAS_N} FROM notes n
              JOIN note_links l ON l.source_id = n.id
             WHERE l.target_id = ?1 ORDER BY n.title, n.id"
        ),
        &[&note_id],
    )
}

/// Enlaza una nota con una anotación anclada a un documento.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la nota o la anotación no existen.
pub fn link_annotation(conn: &Connection, note_id: &str, annotation_id: &str) -> NbResult<()> {
    conn.execute(
        "INSERT INTO note_annotation_links (note_id, annotation_id) VALUES (?1, ?2)
         ON CONFLICT DO NOTHING",
        [note_id, annotation_id],
    )
    .map(|_| ())
    .map_err(|e| fallo(&e))
}

/// Deshace el enlace con una anotación.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la operación falla.
pub fn unlink_annotation(conn: &Connection, note_id: &str, annotation_id: &str) -> NbResult<()> {
    conn.execute(
        "DELETE FROM note_annotation_links WHERE note_id = ?1 AND annotation_id = ?2",
        [note_id, annotation_id],
    )
    .map(|_| ())
    .map_err(|e| fallo(&e))
}

/// Notas que apuntan a una anotación. Es el retroenlace desde el documento.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn notes_for_annotation(conn: &Connection, annotation_id: &str) -> NbResult<Vec<Note>> {
    consultar(
        conn,
        &format!(
            "SELECT {COLUMNAS_N} FROM notes n
              JOIN note_annotation_links l ON l.note_id = n.id
             WHERE l.annotation_id = ?1 ORDER BY n.title, n.id"
        ),
        &[&annotation_id],
    )
}

/// Identificadores de las anotaciones que una nota enlaza, en orden estable.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn linked_annotations(conn: &Connection, note_id: &str) -> NbResult<Vec<String>> {
    let mut stmt = conn
        .prepare(
            "SELECT annotation_id FROM note_annotation_links
              WHERE note_id = ?1 ORDER BY annotation_id",
        )
        .map_err(|e| fallo(&e))?;

    stmt.query_map([note_id], |row| row.get(0))
        .and_then(Iterator::collect)
        .map_err(|e| fallo(&e))
}

fn consultar(
    conn: &Connection,
    sql: &str,
    parametros: &[&dyn rusqlite::ToSql],
) -> NbResult<Vec<Note>> {
    let mut stmt = conn.prepare(sql).map_err(|e| fallo(&e))?;
    stmt.query_map(parametros, Note::from_row)
        .and_then(Iterator::collect)
        .map_err(|e| fallo(&e))
}
