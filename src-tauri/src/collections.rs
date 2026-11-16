//! Colecciones y etiquetas.
//!
//! Contrato: `docs/architecture/api/schemas.md`. Esquema:
//! `src-tauri/migrations/V5__collections.sql`.
//!
//! # Por qué una colección por regla no guarda miembros
//!
//! La alternativa era materializarlos: recalcular la pertenencia al importar y
//! guardarla en una tabla. Se descarta porque una lista materializada **puede
//! quedarse obsoleta** —basta con que un recálculo falle, o con que alguien
//! cambie un metadato por otra vía— y una colección que miente sobre lo que
//! contiene es peor que una que tarda un milisegundo más en responder.
//!
//! Al evaluar la regla en cada consulta, "la colección se actualiza al importar
//! un documento que la cumple" no es un comportamiento que haya que implementar
//! y vigilar: es una consecuencia de cómo está definida.
//!
//! # Etiquetas
//!
//! Una etiqueta es una materia con esquema `tag`. No hace falta tabla propia
//! porque `subjects.scheme` existe justamente para distinguir vocabularios. La
//! diferencia está en el significado, y la interfaz la respeta: una **materia**
//! dice de qué trata la obra según un vocabulario; una **etiqueta** es el nombre
//! que el usuario le pone a algo suyo, como "para el examen" o "releer".

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::catalog::Subject;
use crate::error::{NbError, NbResult};

/// Esquema con el que se guardan las etiquetas del usuario.
pub const ESQUEMA_ETIQUETA: &str = "tag";

/// Cómo decide una colección quién pertenece a ella.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum CollectionKind {
    /// Los miembros se ponen y se quitan a mano.
    Manual,
    /// Los miembros son los que cumplen la regla, en cada momento.
    Rule,
}

impl CollectionKind {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Rule => "rule",
        }
    }
}

/// De quién es la colección.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum CollectionOrigin {
    /// Del usuario. Suya, y por tanto modificable.
    Personal,
}

impl CollectionOrigin {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Personal => "personal",
        }
    }

    fn from_str(valor: &str) -> Option<Self> {
        match valor {
            "personal" => Some(Self::Personal),
            _ => None,
        }
    }
}

/// Una condición de pertenencia.
///
/// La lista es cerrada y corta a propósito. Un lenguaje de consulta general
/// sería más potente y traería consigo un analizador, una superficie de
/// inyección y un conjunto de casos raros que nadie va a probar. Estas cinco
/// cubren lo que la gente hace de verdad con su biblioteca; añadir la sexta
/// cuando haga falta cuesta diez líneas y una migración de ninguna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    /// La edición tiene algún archivo de este tipo de medio.
    MediaType(String),
    Language(String),
    Publisher(String),
    /// La edición tiene esta materia o etiqueta.
    Subject {
        scheme: String,
        label: String,
    },
    /// El título la contiene, sin distinguir mayúsculas.
    TitleContains(String),
}

impl Condition {
    /// Fragmento de SQL y sus parámetros.
    ///
    /// El SQL sale de aquí, de una enumeración cerrada, y **los valores siempre
    /// van ligados**. Es lo que hace que una regla escrita por un tercero no pueda
    /// ser una inyección.
    fn a_sql(&self) -> (String, Vec<String>) {
        match self {
            Self::MediaType(v) => (
                "EXISTS (SELECT 1 FROM files f WHERE f.edition_id = e.id AND f.media_type = ?)"
                    .to_owned(),
                vec![v.clone()],
            ),
            Self::Language(v) => ("e.language = ?".to_owned(), vec![v.clone()]),
            Self::Publisher(v) => ("e.publisher = ?".to_owned(), vec![v.clone()]),
            Self::Subject { scheme, label } => (
                "EXISTS (SELECT 1 FROM edition_subjects es JOIN subjects s ON s.id = es.subject_id \
                 WHERE es.edition_id = e.id AND s.scheme = ? AND s.label = ?)"
                    .to_owned(),
                vec![scheme.clone(), label.clone()],
            ),
            // `instr` sobre minúsculas en lugar de `LIKE`: `LIKE` obligaría a
            // escapar `%` y `_`, y un título con un guion bajo dejaría de
            // encontrarse sin que nadie entendiera por qué.
            Self::TitleContains(v) => (
                "instr(lower(e.title), lower(?)) > 0".to_owned(),
                vec![v.clone()],
            ),
        }
    }
}

/// Criterio de una colección por regla.
///
/// Las condiciones se cumplen **todas**. La disyunción se añadirá cuando alguien
/// la necesite de verdad; hoy sería una forma de complicar el modelo por si
/// acaso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub all: Vec<Condition>,
}

impl Rule {
    /// Construye una regla.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si no tiene condiciones. Una regla vacía se
    /// cumpliría siempre, y una colección que contiene la biblioteca entera sin
    /// que nadie lo haya pedido es peor que una vacía: parece que funciona.
    pub fn new(all: Vec<Condition>) -> NbResult<Self> {
        if all.is_empty() {
            return Err(NbError::Internal);
        }
        Ok(Self { all })
    }

    fn a_sql(&self) -> (String, Vec<String>) {
        let mut fragmentos = Vec::with_capacity(self.all.len());
        let mut parametros = Vec::new();

        for condicion in &self.all {
            let (sql, valores) = condicion.a_sql();
            fragmentos.push(sql);
            parametros.extend(valores);
        }

        (fragmentos.join(" AND "), parametros)
    }
}

/// Una colección.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    pub origin: CollectionOrigin,
    pub rule: Option<Rule>,
    pub created_at: String,
    pub updated_at: String,
}

/// Una colección tal y como la enseña la interfaz: sin su regla y con cuántos
/// miembros tiene ahora.
///
/// La regla no viaja porque la interfaz no la edita todavía, y el recuento sí
/// porque es lo que dice de un vistazo si una colección está vacía sin abrirla.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct CollectionSummary {
    pub id: String,
    pub name: String,
    pub kind: CollectionKind,
    pub origin: CollectionOrigin,
    /// Cuántas ediciones tiene. En una por regla, las que la cumplen **ahora**.
    pub members: u32,
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(clase = %error, "consulta de colecciones fallida");
    NbError::StorageFailure
}

const COLUMNAS: &str = "id, name, kind, origin, rule, created_at, updated_at";

impl Collection {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let kind: String = row.get("kind")?;
        let origin: String = row.get("origin")?;
        let rule: Option<String> = row.get("rule")?;

        let invalido = |c: &'static str| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("valor fuera de la lista admitida en {c}").into(),
            )
        };

        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            kind: match kind.as_str() {
                "manual" => CollectionKind::Manual,
                "rule" => CollectionKind::Rule,
                _ => return Err(invalido("kind")),
            },
            origin: CollectionOrigin::from_str(&origin).ok_or_else(|| invalido("origin"))?,
            rule: rule
                .map(|json| serde_json::from_str(&json).map_err(|_| invalido("rule")))
                .transpose()?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Crea una colección manual.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el nombre está vacío o ya existe otra
    /// colección del mismo origen con ese nombre.
    pub fn create_manual(
        conn: &Connection,
        name: &str,
        origin: CollectionOrigin,
    ) -> NbResult<Self> {
        conn.query_row(
            &format!(
                "INSERT INTO collections (name, kind, origin) VALUES (?1, 'manual', ?2)
                 RETURNING {COLUMNAS}"
            ),
            (name, origin.as_str()),
            Self::from_row,
        )
        .map_err(|e| fallo(&e))
    }

    /// Crea una colección por regla.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el nombre está vacío o repetido, o
    /// [`NbError::Internal`] si la regla no se puede serializar.
    pub fn create_rule(
        conn: &Connection,
        name: &str,
        origin: CollectionOrigin,
        rule: &Rule,
    ) -> NbResult<Self> {
        let json = serde_json::to_string(rule).map_err(|_| NbError::Internal)?;

        conn.query_row(
            &format!(
                "INSERT INTO collections (name, kind, origin, rule) VALUES (?1, 'rule', ?2, ?3)
                 RETURNING {COLUMNAS}"
            ),
            (name, origin.as_str(), json),
            Self::from_row,
        )
        .map_err(|e| fallo(&e))
    }

    /// Busca una colección por identificador.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn find(conn: &Connection, id: &str) -> NbResult<Option<Self>> {
        conn.query_row(
            &format!("SELECT {COLUMNAS} FROM collections WHERE id = ?1"),
            [id],
            Self::from_row,
        )
        .optional()
        .map_err(|e| fallo(&e))
    }

    /// Todas las colecciones, en orden estable.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list(conn: &Connection) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNAS} FROM collections ORDER BY origin, name"
            ))
            .map_err(|e| fallo(&e))?;
        let filas = stmt.query_map([], Self::from_row).map_err(|e| fallo(&e))?;
        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo(&e))
    }

    /// Añade una edición a una colección manual.
    ///
    /// Es idempotente: añadir dos veces lo mismo no falla ni duplica.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si la colección no existe.
    /// - [`NbError::PathNotAllowed`] si es una colección por regla: sus miembros
    ///   los decide el criterio, no la mano.
    /// - [`NbError::StorageFailure`] si la edición no existe.
    pub fn add(conn: &Connection, collection_id: &str, edition_id: &str) -> NbResult<()> {
        let coleccion = Self::find(conn, collection_id)?.ok_or(NbError::DocumentNotFound)?;

        if coleccion.kind == CollectionKind::Rule {
            tracing::warn!("se intentó añadir a mano a una colección por regla");
            return Err(NbError::PathNotAllowed);
        }

        conn.execute(
            "INSERT OR IGNORE INTO collection_items (collection_id, edition_id, position)
             VALUES (?1, ?2,
                     (SELECT coalesce(max(position) + 1, 0) FROM collection_items
                       WHERE collection_id = ?1))",
            (collection_id, edition_id),
        )
        .map(|_| ())
        .map_err(|e| fallo(&e))
    }

    /// Quita una edición de una colección manual.
    ///
    /// # Errors
    ///
    /// Los mismos que [`Self::add`].
    pub fn remove(conn: &Connection, collection_id: &str, edition_id: &str) -> NbResult<()> {
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND edition_id = ?2",
            (collection_id, edition_id),
        )
        .map(|_| ())
        .map_err(|e| fallo(&e))
    }

    /// Identificadores de las ediciones que pertenecen a la colección.
    ///
    /// En una colección manual son los que alguien puso, en su orden. En una por
    /// regla son los que la cumplen **ahora**, evaluado en la base y no
    /// filtrando en memoria: filtrar en memoria obligaría a traer la biblioteca
    /// entera para descartar el noventa y nueve por ciento.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn members(conn: &Connection, collection_id: &str) -> NbResult<Vec<String>> {
        let coleccion = Self::find(conn, collection_id)?.ok_or(NbError::DocumentNotFound)?;

        match (&coleccion.kind, &coleccion.rule) {
            (CollectionKind::Manual, _) => {
                let mut stmt = conn
                    .prepare(
                        "SELECT edition_id FROM collection_items WHERE collection_id = ?1
                          ORDER BY position, added_at, edition_id",
                    )
                    .map_err(|e| fallo(&e))?;
                let filas = stmt
                    .query_map([collection_id], |row| row.get(0))
                    .map_err(|e| fallo(&e))?;
                filas
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|e| fallo(&e))
            }
            (CollectionKind::Rule, Some(regla)) => matching(conn, regla),
            // El `CHECK` de la tabla lo impide, así que llegar aquí significa que
            // alguien escribió por debajo del esquema.
            (CollectionKind::Rule, None) => Err(NbError::DocumentCorrupted),
        }
    }

    /// Cambia el nombre de una colección personal.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si la colección no existe.
    /// - [`NbError::StorageFailure`] si el nombre queda vacío o ya lo lleva otra
    ///   colección del mismo origen.
    pub fn rename(conn: &Connection, id: &str, name: &str) -> NbResult<Self> {
        Self::find(conn, id)?.ok_or(NbError::DocumentNotFound)?;
        conn.query_row(
            &format!("UPDATE collections SET name = ?2 WHERE id = ?1 RETURNING {COLUMNAS}"),
            (id, name.trim()),
            Self::from_row,
        )
        .map_err(|e| fallo(&e))
    }

    /// Borra una colección personal y devuelve si existía.
    ///
    /// **Los documentos siguen en la biblioteca**: lo que se borra es la
    /// agrupación, y sus filas de pertenencia caen con ella por la clave ajena.
    ///
    /// # Errors
    ///
    /// - [`NbError::StorageFailure`] si no se puede escribir.
    pub fn delete(conn: &Connection, id: &str) -> NbResult<bool> {
        conn.execute("DELETE FROM collections WHERE id = ?1", [id])
            .map(|filas| filas > 0)
            .map_err(|e| fallo(&e))
    }

    /// Añade varias ediciones a una colección manual: todas o ninguna.
    ///
    /// Devuelve cuántas entraron **nuevas**. Añadir es idempotente, así que las
    /// que ya estaban no cuentan, y es ese número el que la interfaz puede decir
    /// sin mentir.
    ///
    /// # Errors
    ///
    /// Los de [`Self::add`]. Si falla una, no queda añadida ninguna: una
    /// selección que entra a medias deja al usuario sin saber cuáles faltan.
    pub fn add_many(
        conn: &mut Connection,
        collection_id: &str,
        edition_ids: &[String],
    ) -> NbResult<u32> {
        let tx = crate::db::write_transaction(conn)?;
        let antes = contar_elementos(&tx, collection_id)?;
        for edicion in edition_ids {
            Self::add(&tx, collection_id, edicion)?;
        }
        let despues = contar_elementos(&tx, collection_id)?;
        tx.commit().map_err(|e| fallo(&e))?;

        Ok(u32::try_from(despues.saturating_sub(antes)).unwrap_or(u32::MAX))
    }

    /// Las colecciones manuales en las que está una edición, por nombre.
    ///
    /// Solo las manuales: en una por regla la pertenencia la decide el criterio,
    /// y la ficha enseña esta lista como algo que se pone y se quita a mano.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn containing(conn: &Connection, edition_id: &str) -> NbResult<Vec<String>> {
        let mut stmt = conn
            .prepare(
                "SELECT c.id FROM collection_items ci
                   JOIN collections c ON c.id = ci.collection_id
                  WHERE ci.edition_id = ?1 AND c.kind = 'manual'
                  ORDER BY c.name COLLATE NOCASE, c.id",
            )
            .map_err(|e| fallo(&e))?;
        let filas = stmt
            .query_map([edition_id], |row| row.get(0))
            .map_err(|e| fallo(&e))?;
        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo(&e))
    }

    /// Todas las colecciones con cuántos miembros tienen, ordenadas por nombre.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn summaries(conn: &Connection) -> NbResult<Vec<CollectionSummary>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNAS},
                        (SELECT count(*) FROM collection_items ci WHERE ci.collection_id = collections.id)
                          AS miembros
                   FROM collections
                  ORDER BY name COLLATE NOCASE, id"
            ))
            .map_err(|e| fallo(&e))?;
        let filas = stmt
            .query_map([], |row| {
                Ok((Self::from_row(row)?, row.get::<_, i64>("miembros")?))
            })
            .map_err(|e| fallo(&e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo(&e))?;

        filas
            .into_iter()
            .map(|(coleccion, manuales)| {
                let miembros = match (&coleccion.kind, &coleccion.rule) {
                    (CollectionKind::Manual, _) => manuales,
                    (CollectionKind::Rule, Some(regla)) => contar_los_que_cumplen(conn, regla)?,
                    (CollectionKind::Rule, None) => return Err(NbError::DocumentCorrupted),
                };
                Ok(CollectionSummary {
                    id: coleccion.id,
                    name: coleccion.name,
                    kind: coleccion.kind,
                    origin: coleccion.origin,
                    members: u32::try_from(miembros).unwrap_or(u32::MAX),
                })
            })
            .collect()
    }
}

fn contar_elementos(conn: &Connection, collection_id: &str) -> NbResult<i64> {
    conn.query_row(
        "SELECT count(*) FROM collection_items WHERE collection_id = ?1",
        [collection_id],
        |row| row.get(0),
    )
    .map_err(|e| fallo(&e))
}

/// Cuántas ediciones cumplen una regla, contadas en la base.
fn contar_los_que_cumplen(conn: &Connection, rule: &Rule) -> NbResult<i64> {
    let (donde, parametros) = rule.a_sql();
    conn.query_row(
        &format!("SELECT count(*) FROM editions e WHERE {donde}"),
        rusqlite::params_from_iter(parametros.iter()),
        |row| row.get(0),
    )
    .map_err(|e| fallo(&e))
}

/// Ediciones que cumplen una regla, en orden estable.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn matching(conn: &Connection, rule: &Rule) -> NbResult<Vec<String>> {
    let (donde, parametros) = rule.a_sql();

    let sql = format!("SELECT e.id FROM editions e WHERE {donde} ORDER BY e.created_at, e.id");
    let mut stmt = conn.prepare(&sql).map_err(|e| fallo(&e))?;

    let filas = stmt
        .query_map(rusqlite::params_from_iter(parametros.iter()), |row| {
            row.get(0)
        })
        .map_err(|e| fallo(&e))?;

    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Pone una etiqueta a una edición, creándola si no existía.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la etiqueta está vacía o la edición no existe.
pub fn tag(conn: &Connection, edition_id: &str, label: &str) -> NbResult<Subject> {
    let etiqueta = Subject::ensure(conn, ESQUEMA_ETIQUETA, label)?;
    Subject::link(conn, edition_id, &etiqueta.id)?;
    Ok(etiqueta)
}

/// Quita una etiqueta de una edición.
///
/// La etiqueta en sí no se borra: puede seguir puesta en otros documentos, y
/// borrarla al quitar la última haría desaparecer de la interfaz una etiqueta
/// que el usuario acaba de usar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn untag(conn: &Connection, edition_id: &str, label: &str) -> NbResult<()> {
    conn.execute(
        "DELETE FROM edition_subjects
          WHERE edition_id = ?1
            AND subject_id IN (SELECT id FROM subjects WHERE scheme = ?2 AND label = ?3)",
        (edition_id, ESQUEMA_ETIQUETA, label),
    )
    .map(|_| ())
    .map_err(|e| fallo(&e))
}

/// Etiquetas de una edición, en orden alfabético.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn tags_of(conn: &Connection, edition_id: &str) -> NbResult<Vec<String>> {
    Ok(Subject::list_for_edition(conn, edition_id)?
        .into_iter()
        .filter(|s| s.scheme == ESQUEMA_ETIQUETA)
        .map(|s| s.label)
        .collect())
}
