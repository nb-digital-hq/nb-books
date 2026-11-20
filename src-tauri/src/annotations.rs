//! Anotaciones: inmutables por construcción.
//!
//! # La regla
//!
//! Una anotación **no se edita**. Editarla es darla de baja y crear otra, con
//! identificador nuevo, que apunta a la anterior. La anterior se conserva.
//!
//! # Por qué, si es más trabajo
//!
//! Porque una anotación es del usuario, y una edición que sobrescribe pierde lo
//! que había sin dejar rastro. Con este modelo:
//!
//! * "Me he cargado la nota buena" tiene arreglo: la anterior sigue ahí.
//! * La sincronización entre dispositivos no tiene que decidir quién
//!   escribió último sobre la misma fila. Dos versiones no compiten: conviven.
//! * Exportar el historial es exportar filas, no reconstruir un pasado que
//!   nunca se guardó.
//!
//! La regla la hacen cumplir además dos disparadores en la V7, así que tampoco
//! se puede saltar entrando por SQL directo.
//!
//! # Qué ancla una anotación
//!
//! Un [`Locator`], con todos sus selectores. No un desplazamiento de caracteres
//! ni un número de página: eso es lo que ADR-004 descarta, y es lo que hace que
//! el reanclaje sea posible.
//!
//! Contratos: `docs/adr/004-reader-locator-model.md`,
//! `docs/architecture/api/schemas.md`.

pub mod texto;
pub mod web;

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::catalog::AnnotationKind;
use crate::error::{NbError, NbResult};
use crate::reader::{DocumentProvider, Locator, NativePosition, Precision};

/// Una anotación del usuario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Annotation {
    pub id: String,
    pub edition_id: String,
    pub kind: AnnotationKind,
    /// Texto de la nota. Nulo en un resaltado sin comentario.
    pub body: Option<String>,
    /// Dónde está anclada, con todos sus selectores.
    pub target: Locator,
    /// Geometría del trazo, si la marca es un trazo libre.
    ///
    /// Objeto JSON serializado, y nulo en las demás clases. Aparte de `body`
    /// porque `body` es lo que escribe una persona.
    pub payload: Option<String>,
    pub created_at: String,
    /// Cuándo se dio de baja. Nulo mientras está viva.
    pub deleted_at: Option<String>,
    /// La versión anterior, si esta es la edición de otra.
    pub supersedes: Option<String>,
}

impl Annotation {
    /// Si sigue viva.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.deleted_at.is_none()
    }
}

/// Lo que hace falta para crear una anotación.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAnnotation {
    pub edition_id: String,
    pub kind: AnnotationKind,
    pub body: Option<String>,
    pub target: Locator,
    /// Geometría, solo para el trazo libre. La base rechaza un trazo sin ella.
    pub payload: Option<String>,
}

// `payload` va al final: las filas se leen por posición, y meterlo en medio
// habría desplazado en silencio a todos los de detrás.
const COLUMNAS: &str =
    "id, edition_id, kind, body, target, created_at, deleted_at, supersedes, payload";

fn desde_fila(row: &Row<'_>) -> rusqlite::Result<Annotation> {
    let kind: String = row.get(2)?;
    let target: String = row.get(4)?;

    let invalido = |que: &'static str| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            format!("anotación con {que} ilegible").into(),
        )
    };

    Ok(Annotation {
        id: row.get(0)?,
        edition_id: row.get(1)?,
        kind: AnnotationKind::parse(&kind).ok_or_else(|| invalido("tipo"))?,
        body: row.get(3)?,
        // Un ancla ilegible es un error, no una anotación sin ancla: colocarla
        // en el principio del documento sería mover el trabajo del usuario sin
        // decírselo.
        target: serde_json::from_str(&target).map_err(|_| invalido("ancla"))?,
        created_at: row.get(5)?,
        deleted_at: row.get(6)?,
        supersedes: row.get(7)?,
        payload: row.get(8)?,
    })
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo al operar con anotaciones");
    NbError::StorageFailure
}

/// Crea una anotación.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si un resaltado no lleva la cita. Sin cita no
///   hay segundo escalón de la cascada de ADR-004, así que ese resaltado se
///   perdería en cuanto el documento cambiase, y aceptarlo sería prometer un
///   reanclaje que no puede ocurrir.
/// - [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
pub fn create(conn: &Connection, nueva: &NewAnnotation) -> NbResult<Annotation> {
    if nueva.kind == AnnotationKind::Highlight && nueva.target.text.is_none() {
        tracing::warn!("se intentó crear un resaltado sin cita");
        return Err(NbError::PathNotAllowed);
    }

    let target = serde_json::to_string(&nueva.target).map_err(|_| NbError::Internal)?;

    conn.query_row(
        &format!(
            "INSERT INTO annotations (edition_id, kind, body, target, payload)
             VALUES (?1, ?2, ?3, ?4, ?5) RETURNING {COLUMNAS}"
        ),
        rusqlite::params![
            nueva.edition_id,
            nueva.kind.as_str(),
            nueva.body,
            target,
            nueva.payload
        ],
        desde_fila,
    )
    .map_err(|e| fallo(&e))
}

/// Lo que se quiere cambiar de una anotación.
///
/// Los campos ausentes se heredan de la versión anterior. Una edición que solo
/// cambia el comentario conserva el ancla sin tener que repetirla, que es lo que
/// se hace casi siempre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edit {
    pub body: Option<Option<String>>,
    pub target: Option<Locator>,
}

/// Edita una anotación: la da de baja y crea su sucesora.
///
/// Devuelve la **nueva**, con identificador nuevo. La anterior sigue en la base,
/// dada de baja, y se puede consultar con [`history`].
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no existe o si ya estaba dada de baja.
///   Editar una anotación borrada no es editar: es resucitarla, y eso se hace a
///   propósito o no se hace.
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn edit(conn: &mut Connection, id: &str, cambio: &Edit) -> NbResult<Annotation> {
    let tx = crate::db::write_transaction(conn)?;

    let anterior = find_in(&tx, id)?.ok_or(NbError::DocumentNotFound)?;
    if !anterior.is_live() {
        tracing::info!("se intentó editar una anotación dada de baja");
        return Err(NbError::DocumentNotFound);
    }

    let body = cambio.body.clone().unwrap_or(anterior.body.clone());
    let target = cambio.target.clone().unwrap_or(anterior.target.clone());

    if anterior.kind == AnnotationKind::Highlight && target.text.is_none() {
        return Err(NbError::PathNotAllowed);
    }

    let serializado = serde_json::to_string(&target).map_err(|_| NbError::Internal)?;

    // La baja va antes que el alta: si el alta falla, la transacción deshace las
    // dos y la anotación original sigue viva. Al revés, un fallo dejaría dos
    // vivas apuntando al mismo sitio.
    tx.execute(
        "UPDATE annotations SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ?1",
        [id],
    )
    .map_err(|e| fallo(&e))?;

    let nueva = tx
        .query_row(
            &format!(
                "INSERT INTO annotations (edition_id, kind, body, target, supersedes)
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING {COLUMNAS}"
            ),
            rusqlite::params![
                anterior.edition_id,
                anterior.kind.as_str(),
                body,
                serializado,
                anterior.id
            ],
            desde_fila,
        )
        .map_err(|e| fallo(&e))?;

    tx.commit().map_err(|e| fallo(&e))?;
    Ok(nueva)
}

/// Da de baja una anotación.
///
/// La fila se conserva. Un borrado de verdad llega con la purga explícita del
/// usuario, que es otra cosa y no es este módulo.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si no existe, o [`NbError::StorageFailure`] si
/// la escritura falla. Dar de baja lo que ya estaba de baja no es un error.
pub fn delete(conn: &Connection, id: &str) -> NbResult<()> {
    let filas = conn
        .execute(
            "UPDATE annotations SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              WHERE id = ?1 AND deleted_at IS NULL",
            [id],
        )
        .map_err(|e| fallo(&e))?;

    if filas == 0 && find(conn, id)?.is_none() {
        return Err(NbError::DocumentNotFound);
    }

    Ok(())
}

/// Busca una anotación por identificador, viva o no.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn find(conn: &Connection, id: &str) -> NbResult<Option<Annotation>> {
    find_in(conn, id)
}

fn find_in(conn: &Connection, id: &str) -> NbResult<Option<Annotation>> {
    conn.query_row(
        &format!("SELECT {COLUMNAS} FROM annotations WHERE id = ?1"),
        [id],
        desde_fila,
    )
    .optional()
    .map_err(|e| fallo(&e))
}

/// Anotaciones vivas de una edición, en orden estable.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn list(conn: &Connection, edition_id: &str) -> NbResult<Vec<Annotation>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNAS} FROM annotations
              WHERE edition_id = ?1 AND deleted_at IS NULL
              ORDER BY created_at, id"
        ))
        .map_err(|e| fallo(&e))?;

    let filas = stmt
        .query_map([edition_id], desde_fila)
        .map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Todas las versiones de una anotación, de la más antigua a la más reciente.
///
/// Se puede pedir por cualquiera de sus identificadores: la cadena se recorre
/// hacia atrás hasta la primera versión y después hacia delante. Quien tiene un
/// identificador viejo guardado en una exportación de hace un año sigue pudiendo
/// llegar a la versión actual.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si el identificador no existe, o
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn history(conn: &Connection, id: &str) -> NbResult<Vec<Annotation>> {
    let punto = find(conn, id)?.ok_or(NbError::DocumentNotFound)?;

    // Hacia atrás hasta la primera. El tope no es paranoia gratuita: un ciclo
    // en la cadena, por una importación manipulada, colgaría el proceso.
    let mut primera = punto;
    for _ in 0..1_000 {
        let Some(anterior) = primera.supersedes.clone() else {
            break;
        };
        let Some(fila) = find(conn, &anterior)? else {
            break;
        };
        primera = fila;
    }

    let mut cadena = vec![primera];
    for _ in 0..1_000 {
        let ultima = cadena.last().expect("siempre hay una").id.clone();
        let siguiente: Option<Annotation> = conn
            .query_row(
                &format!("SELECT {COLUMNAS} FROM annotations WHERE supersedes = ?1"),
                [&ultima],
                desde_fila,
            )
            .optional()
            .map_err(|e| fallo(&e))?;

        match siguiente {
            Some(fila) => cadena.push(fila),
            None => break,
        }
    }

    Ok(cadena)
}

/// Una anotación con la posición donde ha quedado al abrir el documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct AnchoredAnnotation {
    pub annotation: Annotation,
    pub position: NativePosition,
    pub precision: Precision,
    /// La posición ya no es donde se anotó. La interfaz lo marca.
    pub relocated: bool,
    /// Se resolvió por progresión: puede estar en otro párrafo. Se avisa.
    pub warn: bool,
}

/// Lo que quedó al reanclar las anotaciones de un documento.
///
/// Las que no se resolvieron **no se descartan**: salen en `lost`. Es la
/// diferencia entre "esta anotación no se puede mostrar aquí" y "esta anotación
/// no existe", y esa diferencia es todo el módulo: ADR-004 prohíbe perder una
/// anotación en silencio, no solo colocarla mal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct AnchorReport {
    pub anchored: Vec<AnchoredAnnotation>,
    /// Anotaciones que ninguna vía de la cascada supo colocar.
    pub lost: Vec<Annotation>,
}

impl AnchorReport {
    /// Cuántas quedaron en un sitio distinto de donde se anotaron.
    #[must_use]
    pub fn relocated(&self) -> usize {
        self.anchored.iter().filter(|a| a.relocated).count()
    }

    /// Cuántas hay que avisar por estar colocadas por progresión.
    #[must_use]
    pub fn warned(&self) -> usize {
        self.anchored.iter().filter(|a| a.warn).count()
    }
}

/// Reancla todas las anotaciones vivas de una edición sobre el documento abierto.
///
/// Aplica la cascada de ADR-004 que implementa cada proveedor: fragmento
/// canónico corroborado con la cita, después la cita con su contexto, y por
/// último la progresión. Lo que este nivel añade es que **el resultado se
/// conserva entero**: ninguna anotación se cae por el camino, ni siquiera la que
/// no se pudo colocar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla o si alguna anotación tiene
/// el ancla ilegible.
pub fn anchor(
    conn: &Connection,
    provider: &dyn DocumentProvider,
    edition_id: &str,
) -> NbResult<AnchorReport> {
    let vivas = list(conn, edition_id)?;
    let mut anchored = Vec::with_capacity(vivas.len());
    let mut lost = Vec::new();

    for anotacion in vivas {
        match provider.resolve(&anotacion.target) {
            Ok(resolucion) => anchored.push(AnchoredAnnotation {
                annotation: anotacion,
                position: resolucion.position,
                precision: resolucion.precision,
                relocated: resolucion.precision.is_relocated(),
                warn: resolucion.precision.requires_warning(),
            }),
            Err(error) => {
                // Se registra el código, no el contenido: el título de la
                // anotación y la cita son datos del usuario.
                tracing::info!(
                    code = error.code(),
                    "una anotación no se pudo reanclar en este documento"
                );
                lost.push(anotacion);
            }
        }
    }

    if !lost.is_empty() {
        tracing::warn!(
            perdidas = lost.len(),
            "hay anotaciones que este documento no sabe colocar"
        );
    }

    Ok(AnchorReport { anchored, lost })
}

/// Una entrada del panel de anotaciones.
///
/// Una sola lista y no dos, con las que no se pudieron colocar al final y
/// marcadas: un panel que esconde las perdidas en otra pestaña las convierte en
/// algo que el usuario descubre por casualidad, y la mitad del objetivo de
/// el objetivo era que no se perdieran de vista.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct OutlineEntry {
    pub annotation: Annotation,
    /// Dónde ha quedado. Nula si ninguna vía supo colocarla.
    pub position: Option<NativePosition>,
    pub precision: Option<Precision>,
    pub relocated: bool,
    pub warn: bool,
    /// Ninguna vía de la cascada supo colocarla en este documento.
    pub lost: bool,
}

/// Panel de anotaciones de un documento, **en orden de lectura**.
///
/// En orden de lectura y no por fecha de creación: el panel acompaña a quien
/// está leyendo, y una lista por fecha obliga a buscar en ella cada vez que se
/// pasa de capítulo. La fecha decide solo entre dos anotaciones que caen en el
/// mismo punto.
///
/// El orden sale de la **progresión total** del locator, que es lo único que
/// significa lo mismo en los tres formatos: una página de cómic, un
/// desplazamiento de EPUB y una página de PDF no se pueden comparar entre sí, y
/// la progresión sí.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn outline(
    conn: &Connection,
    provider: &dyn DocumentProvider,
    edition_id: &str,
) -> NbResult<Vec<OutlineEntry>> {
    let informe = anchor(conn, provider, edition_id)?;

    let mut entradas: Vec<OutlineEntry> = informe
        .anchored
        .into_iter()
        .map(|a| OutlineEntry {
            annotation: a.annotation,
            position: Some(a.position),
            precision: Some(a.precision),
            relocated: a.relocated,
            warn: a.warn,
            lost: false,
        })
        .collect();

    entradas.sort_by(|a, b| {
        let clave = |e: &OutlineEntry| {
            (
                e.annotation.target.locations.total_progression,
                e.annotation.target.locations.position,
            )
        };
        let (pa, oa) = clave(a);
        let (pb, ob) = clave(b);

        // `total_cmp` y no `partial_cmp`: una progresión no puede ser NaN porque
        // el tipo lo impide, pero ordenar con un comparador que puede devolver
        // "no comparable" deja el orden a merced de esa suposición.
        pa.unwrap_or(f64::MAX)
            .total_cmp(&pb.unwrap_or(f64::MAX))
            .then(oa.unwrap_or(u32::MAX).cmp(&ob.unwrap_or(u32::MAX)))
            .then_with(|| a.annotation.created_at.cmp(&b.annotation.created_at))
            .then_with(|| a.annotation.id.cmp(&b.annotation.id))
    });

    // Las que no se pudieron colocar, al final: no tienen sitio en el orden de
    // lectura porque no tienen sitio en el documento.
    entradas.extend(informe.lost.into_iter().map(|anotacion| OutlineEntry {
        annotation: anotacion,
        position: None,
        precision: None,
        relocated: false,
        warn: false,
        lost: true,
    }));

    Ok(entradas)
}

/// Anotaciones vivas de una edición de una clase concreta.
///
/// Existe para los marcadores, que la interfaz lista aparte de los resaltados y
/// las notas: son de otra naturaleza, no llevan texto y se usan para volver, no
/// para recordar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn list_of_kind(
    conn: &Connection,
    edition_id: &str,
    kind: AnnotationKind,
) -> NbResult<Vec<Annotation>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNAS} FROM annotations
              WHERE edition_id = ?1 AND kind = ?2 AND deleted_at IS NULL
              ORDER BY created_at, id"
        ))
        .map_err(|e| fallo(&e))?;

    let filas = stmt
        .query_map(rusqlite::params![edition_id, kind.as_str()], desde_fila)
        .map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Una anotación con el documento del que salió.
///
/// # Por qué no es un [`OutlineEntry`]
///
/// Un `OutlineEntry` dice **dónde ha quedado** la anotación, y saberlo exige
/// abrir el documento y recorrer la cascada de reanclaje. En la vista de Estudio
/// hay anotaciones de toda la biblioteca: abrir cada documento para pintar una
/// lista costaría segundos y no aportaría nada, porque desde ahí no se está
/// leyendo ninguno.
///
/// Así que aquí **no se afirma nada sobre el anclaje**. Un tipo con `lost` en
/// falso sin haberlo comprobado sería exactamente la clase de afirmación que
/// este proyecto no se permite. Lo que se sabe se dice; lo que no, no se
/// inventa: el reanclaje se resuelve al abrir el documento.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct LibraryAnnotation {
    pub annotation: Annotation,
    /// Título de la edición: de qué libro es, sin tener que abrirlo.
    pub title: String,
    /// Autores, separados por coma. Nulo si la edición no tiene ninguno.
    pub authors: Option<String>,
    /// Archivo con el que abrirla. **Nulo si ninguna copia está disponible**,
    /// y entonces la interfaz no ofrece el salto en lugar de fallar al pulsarlo.
    pub file_id: Option<String>,
}

/// Todas las anotaciones vivas de la biblioteca, de la más reciente a la más
/// antigua.
///
/// # Por qué por fecha y no en orden de lectura
///
/// El panel del lector va en orden de lectura porque acompaña a quien está
/// leyendo un documento concreto. Aquí no hay un documento: hay sesenta, y la
/// progresión total de uno no es comparable con la de otro. Lo único que
/// significa lo mismo entre libros es cuándo se escribió, y "lo último que
/// anoté" es además la pregunta que trae a alguien a esta vista.
///
/// `kind` filtra por clase cuando se pasa. Sin filtro salen las tres.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn library(
    conn: &Connection,
    kind: Option<AnnotationKind>,
    limite: u32,
    desde: u32,
) -> NbResult<Vec<LibraryAnnotation>> {
    con_contexto_acotado(
        conn,
        "",
        "WHERE a.deleted_at IS NULL AND (?1 IS NULL OR a.kind = ?1)",
        rusqlite::params![kind.map(AnnotationKind::as_str)],
        Some((limite, desde)),
    )
}

/// Cuántas anotaciones hay de cada clase en la biblioteca, contando las mismas que
/// [`library`].
///
/// Existe para no ofrecer un filtro que no lleva a nada: Observación de uso del
/// 2026-09-12, "si no tiene ítems no se muestra". Contar es una lectura de las
/// anotaciones que escribió el usuario, que son miles como mucho, y no de la
/// biblioteca.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn counts(conn: &Connection) -> NbResult<AnnotationCounts> {
    let mut cuentas = AnnotationCounts::default();
    let mut stmt = conn
        .prepare(
            "SELECT a.kind, count(*) FROM annotations a
               JOIN editions e ON e.id = a.edition_id
              WHERE a.deleted_at IS NULL
              GROUP BY a.kind",
        )
        .map_err(|e| fallo(&e))?;
    let filas = stmt
        .query_map([], |f| Ok((f.get::<_, String>(0)?, f.get::<_, i64>(1)?)))
        .map_err(|e| fallo(&e))?;
    for fila in filas {
        let (clase, cuantas) = fila.map_err(|e| fallo(&e))?;
        let cuantas = u32::try_from(cuantas).unwrap_or(u32::MAX);
        cuentas.all = cuentas.all.saturating_add(cuantas);
        match clase.as_str() {
            "note" => cuentas.note = cuantas,
            "highlight" => cuentas.highlight = cuantas,
            "bookmark" => cuentas.bookmark = cuantas,
            "underline" => cuentas.underline = cuantas,
            "drawing" => cuentas.drawing = cuantas,
            _ => {}
        }
    }
    Ok(cuentas)
}

/// Cuántas anotaciones hay de cada clase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct AnnotationCounts {
    pub all: u32,
    pub note: u32,
    pub highlight: u32,
    pub bookmark: u32,
    pub underline: u32,
    pub drawing: u32,
}

/// Las anotaciones que una nota cita, con el libro del que salió cada una.
///
/// # Por qué devuelve lo mismo que [`library`] y no una lista de
/// identificadores
///
/// Porque una cita que solo dice el identificador no se puede enseñar: quien
/// mira una nota necesita ver **de qué pasaje habla**, y eso es la cita y el
/// libro. `notes::linked_annotations` da los identificadores y sirve para la
/// exportación, que sí los quiere pelados.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn for_note(conn: &Connection, note_id: &str) -> NbResult<Vec<LibraryAnnotation>> {
    con_contexto(
        conn,
        "JOIN note_annotation_links l ON l.annotation_id = a.id",
        "WHERE a.deleted_at IS NULL AND l.note_id = ?1",
        rusqlite::params![note_id],
    )
}

/// Anotaciones con su contexto: de qué libro son y con qué archivo se abren.
///
/// Lo comparten las dos consultas que devuelven [`LibraryAnnotation`] y se
/// escribe una sola vez, porque dos copias del mismo `SELECT` acaban divergiendo
/// justo en la columna que una de las dos añadió.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
fn con_contexto(
    conn: &Connection,
    union: &str,
    donde: &str,
    parametros: impl rusqlite::Params,
) -> NbResult<Vec<LibraryAnnotation>> {
    con_contexto_acotado(conn, union, donde, parametros, None)
}

/// Lo mismo, con un tramo. `None` trae todas: solo lo usan las consultas cuya
/// cota es un documento o una nota, no la biblioteca entera.
fn con_contexto_acotado(
    conn: &Connection,
    union: &str,
    donde: &str,
    parametros: impl rusqlite::Params,
    tramo: Option<(u32, u32)>,
) -> NbResult<Vec<LibraryAnnotation>> {
    // El archivo se elige con `min` entre los disponibles, igual que en
    // `browse`: dos copias del mismo documento abren la misma edición, así que
    // cuál de las dos se use no cambia nada y elegir siempre igual sí importa.
    let consulta = format!(
        "SELECT {columnas},
                e.title,
                (SELECT group_concat(c.name, ', ')
                   FROM (SELECT ec.edition_id, c2.name
                           FROM edition_contributors ec
                           JOIN contributors c2 ON c2.id = ec.contributor_id
                          WHERE ec.edition_id = e.id AND ec.role = 'author'
                          ORDER BY ec.position) c
                ) AS autores,
                (SELECT min(f.id) FROM files f
                  WHERE f.edition_id = e.id AND f.availability = 'available'
                ) AS archivo
           FROM annotations a
           JOIN editions e ON e.id = a.edition_id
           {union}
          {donde}
          ORDER BY a.created_at DESC, a.id DESC
          {tramo}",
        tramo = match tramo {
            // Interpolado y no parametrizado porque los dos son enteros sin
            // signo del propio runtime: no hay ninguna cadena de fuera aquí.
            Some((limite, desde)) => format!("LIMIT {limite} OFFSET {desde}"),
            None => String::new(),
        },
        columnas = COLUMNAS
            .split(", ")
            .map(|c| format!("a.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    /*
     * Dónde empiezan las columnas añadidas, **contado y no escrito**.
     *
     * Estaban escritas a mano —8, 9 y 10— y al añadir `payload` al final de
     * `COLUMNAS` se desplazaron todas: la consulta seguía siendo válida, así que
     * no falló al prepararse; falló al leer un texto donde ahora hay otra cosa, y
     * salió como `StorageFailure` sin decir por qué. Cuatro pruebas lo vieron.
     *
     * Contando, añadir una columna a `COLUMNAS` no puede volver a romper esto.
     */
    let añadidas = COLUMNAS.split(", ").count();

    let mut stmt = conn.prepare(&consulta).map_err(|e| fallo(&e))?;
    let filas = stmt
        .query_map(parametros, |row| {
            Ok(LibraryAnnotation {
                annotation: desde_fila(row)?,
                title: row.get(añadidas)?,
                authors: row.get(añadidas + 1)?,
                file_id: row.get(añadidas + 2)?,
            })
        })
        .map_err(|e| fallo(&e))?;

    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Pone un marcador en una posición.
///
/// Un marcador es una anotación sin cuerpo: no dice nada, solo señala. Tiene
/// función propia porque crear uno es lo que más se hace y pedir la clase y un
/// cuerpo vacío en cada llamada invita a equivocarse con la clase.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
pub fn bookmark(conn: &Connection, edition_id: &str, target: &Locator) -> NbResult<Annotation> {
    create(
        conn,
        &NewAnnotation {
            payload: None,
            edition_id: edition_id.to_owned(),
            kind: AnnotationKind::Bookmark,
            body: None,
            target: target.clone(),
        },
    )
}
