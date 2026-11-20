//! Procedencia del metadato y precedencia de la edición manual.
//!
//! Es la regla que separa un gestor en el que se puede confiar de uno que
//! corrompe el trabajo del usuario en silencio: **una actualización automática
//! nunca sobrescribe lo que alguien corrigió a mano.** Cuando discrepa, propone.
//!
//! Y lo que no puede aplicar tampoco se descarta: se guarda como propuesta, para
//! que la interfaz pueda ofrecerla. Tirarla en silencio sería perder información
//! igual que sobrescribir.
//!
//! Contrato: `docs/architecture/metadata-model.md`, sección 3.
//! Esquema: `src-tauri/migrations/V3__metadata_provenance.sql`.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::db::catalog::{Contributor, ContributorRole, Edition, IdentifierScheme, Subject};
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};

fn fallo_de_almacenamiento(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "consulta de procedencia fallida");
    NbError::StorageFailure
}

/// Entidad del catálogo a la que pertenece un campo con procedencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    Work,
    Edition,
    Contributor,
}

impl EntityType {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Edition => "edition",
            Self::Contributor => "contributor",
        }
    }

    const fn tabla(self) -> &'static str {
        match self {
            Self::Work => "works",
            Self::Edition => "editions",
            Self::Contributor => "contributors",
        }
    }
}

/// De dónde salió un valor.
///
/// El orden de precedencia es el de esta enumeración, y no un número suelto en
/// una tabla, para que añadir un origen obligue a decidir dónde encaja.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "MetadataSource"
)]
pub enum Source {
    /// Extraído de los metadatos internos del propio documento.
    File,
    /// Obtenido de un proveedor en línea.
    Online,
    /// Traído de una biblioteca de Calibre. Conserva lo que el usuario corrigió en ella.
    Calibre,
    /// Editado por el usuario. Gana siempre.
    Manual,
}

impl Source {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Online => "online",
            Self::Calibre => "calibre",
            Self::Manual => "manual",
        }
    }

    fn from_str(valor: &str) -> Option<Self> {
        match valor {
            "file" => Some(Self::File),
            "online" => Some(Self::Online),
            "calibre" => Some(Self::Calibre),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Campo con procedencia registrada.
///
/// Es una lista cerrada y no una cadena libre por dos motivos: el nombre acaba
/// interpolado en un `UPDATE`, y un campo escrito de tres formas distintas
/// produce tres procedencias que nunca se reconocen entre sí.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    CanonicalTitle,
    OriginalLanguage,
    Title,
    Subtitle,
    Publisher,
    PublishedDate,
    EditionStatement,
    Language,
    SeriesTitle,
    SeriesIndex,
    Description,
    Name,
    SortName,
}

impl Field {
    /// Nombre de la columna. Coincide con el nombre que se guarda en
    /// `metadata_provenance.field`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanonicalTitle => "canonical_title",
            Self::OriginalLanguage => "original_language",
            Self::Title => "title",
            Self::Subtitle => "subtitle",
            Self::Publisher => "publisher",
            Self::PublishedDate => "published_date",
            Self::EditionStatement => "edition_statement",
            Self::Language => "language",
            Self::SeriesTitle => "series_title",
            Self::SeriesIndex => "series_index",
            Self::Description => "description",
            Self::Name => "name",
            Self::SortName => "sort_name",
        }
    }

    /// Entidad a la que pertenece el campo.
    ///
    /// Impide escribir `title` en `works` o `canonical_title` en `editions`, que
    /// es un error que la base aceptaría como columna inexistente en tiempo de
    /// ejecución, no de compilación.
    #[must_use]
    pub const fn entity_type(self) -> EntityType {
        match self {
            Self::CanonicalTitle | Self::OriginalLanguage => EntityType::Work,
            Self::Title
            | Self::Subtitle
            | Self::Publisher
            | Self::PublishedDate
            | Self::EditionStatement
            | Self::Language
            | Self::SeriesTitle
            | Self::SeriesIndex
            | Self::Description => EntityType::Edition,
            Self::Name | Self::SortName => EntityType::Contributor,
        }
    }
}

/// Registro de qué aportó un origen a un campo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub field: String,
    pub source: Source,
    pub value: Option<String>,
    pub recorded_at: String,
}

impl Provenance {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let entity_type: String = row.get("entity_type")?;
        let source: String = row.get("source")?;

        let invalido = |columna: &'static str| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("valor fuera de la lista admitida en {columna}").into(),
            )
        };

        Ok(Self {
            entity_type: match entity_type.as_str() {
                "work" => EntityType::Work,
                "edition" => EntityType::Edition,
                "contributor" => EntityType::Contributor,
                _ => return Err(invalido("entity_type")),
            },
            entity_id: row.get("entity_id")?,
            field: row.get("field")?,
            source: Source::from_str(&source).ok_or_else(|| invalido("source"))?,
            value: row.get("value")?,
            recorded_at: row.get("recorded_at")?,
        })
    }
}

/// Estado de una propuesta de cambio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Rejected,
}

impl SuggestionStatus {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }

    fn from_str(valor: &str) -> Option<Self> {
        match valor {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// Cambio que un origen automático quiso hacer y no pudo aplicar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "MetadataSuggestion"
)]
pub struct Suggestion {
    pub id: String,
    pub entity_id: String,
    pub field: String,
    pub source: Source,
    pub proposed_value: Option<String>,
    pub status: SuggestionStatus,
    pub created_at: String,
}

const COLUMNAS_SUGGESTION: &str =
    "id, entity_type, entity_id, field, source, proposed_value, status, created_at";

impl Suggestion {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let source: String = row.get("source")?;
        let status: String = row.get("status")?;

        let invalido = |columna: &'static str| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("valor fuera de la lista admitida en {columna}").into(),
            )
        };

        Ok(Self {
            id: row.get("id")?,
            entity_id: row.get("entity_id")?,
            field: row.get("field")?,
            source: Source::from_str(&source).ok_or_else(|| invalido("source"))?,
            proposed_value: row.get("proposed_value")?,
            status: SuggestionStatus::from_str(&status).ok_or_else(|| invalido("status"))?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Qué ocurrió al intentar escribir un campo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Se escribió en la entidad y quedó registrada su procedencia.
    Applied,
    /// El campo ya tenía una decisión del usuario: el valor no se tocó y quedó
    /// registrado como propuesta.
    Suggested(Box<Suggestion>),
    /// El valor propuesto era el que ya había. No se toca nada ni se propone
    /// nada: repetir la misma consulta no puede generar ruido.
    Unchanged,
}

/// Escribe un valor decidido por el usuario.
///
/// Gana siempre y sin excepción, incluso sobre lo que un proveedor en línea
/// considere correcto. También retira las propuestas vivas de ese campo: una vez
/// que el usuario ha decidido, ofrecerle la alternativa que ya descartó es
/// ruido.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la entidad no existe o la escritura falla.
pub fn set_manual(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    value: Option<&str>,
) -> NbResult<Outcome> {
    escribir(conn, entity_id, field, value)?;
    registrar_procedencia(conn, entity_id, field, Source::Manual, value)?;

    conn.execute(
        "UPDATE metadata_suggestions
            SET status = 'rejected', resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3 AND status = 'pending'",
        (field.entity_type().as_str(), entity_id, field.as_str()),
    )
    .map_err(|e| fallo_de_almacenamiento(&e))?;

    Ok(Outcome::Applied)
}

/// Declara de dónde salió un valor que **ya está escrito**.
///
/// Existe para el caso de la importación: la fila se crea con su título en el
/// mismo `INSERT`, así que cuando se quiere registrar su procedencia el valor ya
/// está puesto y [`propose`] no haría nada, porque no habría discrepancia que
/// resolver. Sin esto, el título con el que un documento entra en la biblioteca
/// no tendría origen registrado, y la extracción de metadatos posterior no
/// podría mejorarlo sin parecer que pisa una decisión del usuario.
///
/// No escribe en la entidad: solo anota quién puso lo que ya hay.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn record_origin(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
    value: Option<&str>,
) -> NbResult<()> {
    registrar_procedencia(conn, entity_id, field, source, value)
}

/// Ofrece un valor obtenido de un origen automático.
///
/// Se aplica solo si el usuario no ha decidido ese campo. Si lo ha decidido, el
/// valor **no se escribe**: se guarda como propuesta.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la entidad no existe o la escritura falla.
///
/// # Panics
///
/// No entra en pánico; el `expect` interno solo se alcanzaría si la fila
/// insertada no se pudiera releer, que es un fallo de la base ya cubierto por el
/// error anterior.
pub fn propose(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
    value: Option<&str>,
) -> NbResult<Outcome> {
    if source == Source::Manual {
        // Un origen automático no puede hacerse pasar por el usuario. Se trata
        // como lo que es: una edición manual.
        return set_manual(conn, entity_id, field, value);
    }

    let actual = leer(conn, entity_id, field)?;
    if actual.as_deref() == value {
        return Ok(Outcome::Unchanged);
    }

    // Lo que el usuario ya rechazó no se vuelve a ofrecer **ni se aplica**.
    //
    // Es el criterio de aceptación, y vale para las dos ramas de
    // abajo a propósito: sin esto, consultar el mismo proveedor otra vez
    // volvería a proponer lo mismo cuando hay decisión manual, y lo escribiría
    // en silencio cuando no la hay. Rechazar es una decisión, y una decisión que
    // se olvida al siguiente clic no es una decisión.
    if fue_rechazada(conn, entity_id, field, source, value)? {
        tracing::debug!(
            field = field.as_str(),
            source = source.as_str(),
            "el valor propuesto ya se había rechazado; no se vuelve a ofrecer"
        );
        return Ok(Outcome::Unchanged);
    }

    if hay_decision_manual(conn, entity_id, field)? {
        let sugerencia = registrar_propuesta(conn, entity_id, field, source, value)?;
        tracing::debug!(
            field = field.as_str(),
            source = source.as_str(),
            "el valor automático discrepa de una edición manual; queda como propuesta"
        );
        return Ok(Outcome::Suggested(Box::new(sugerencia)));
    }

    if tiene_precedencia_mayor(conn, entity_id, field, source)? {
        let sugerencia = registrar_propuesta(conn, entity_id, field, source, value)?;
        tracing::debug!(
            field = field.as_str(),
            source = source.as_str(),
            "el valor automático discrepa de un origen de mayor precedencia; queda como propuesta"
        );
        return Ok(Outcome::Suggested(Box::new(sugerencia)));
    }

    escribir(conn, entity_id, field, value)?;
    registrar_procedencia(conn, entity_id, field, source, value)?;
    Ok(Outcome::Applied)
}

/// Si este mismo valor, de este mismo origen, ya se había rechazado.
///
/// Se compara **el valor** y no solo el campo: que alguien rechazara un título
/// no significa que rechace cualquier título que llegue después del mismo sitio,
/// y tratarlo así dejaría el campo condenado para siempre.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
fn fue_rechazada(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
    value: Option<&str>,
) -> NbResult<bool> {
    let Some(tipo) = entity_type_of_field(field.as_str()) else {
        return Ok(false);
    };

    conn.query_row(
        "SELECT 1 FROM metadata_suggestions
          WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3 AND source = ?4
            AND status = 'rejected'
            AND proposed_value IS ?5
          LIMIT 1",
        rusqlite::params![
            tipo.as_str(),
            entity_id,
            field.as_str(),
            source.as_str(),
            value
        ],
        |_| Ok(()),
    )
    .optional()
    .map(|encontrada| encontrada.is_some())
    .map_err(|e| fallo_de_almacenamiento(&e))
}

/// Aplica una propuesta pendiente como si el usuario la hubiera escrito.
///
/// Es lo que ocurre cuando alguien pulsa "aceptar" en la interfaz: la decisión
/// pasa a ser suya, y por eso queda registrada como manual.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la propuesta no existe o ya estaba resuelta.
pub fn accept(conn: &Connection, suggestion_id: &str) -> NbResult<String> {
    let (entity_id, campo, valor): (String, String, Option<String>) = conn
        .query_row(
            "SELECT entity_id, field, proposed_value FROM metadata_suggestions
              WHERE id = ?1 AND status = 'pending'",
            [suggestion_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| fallo_de_almacenamiento(&e))?
        .ok_or(NbError::DocumentNotFound)?;

    let field = campo_desde(&campo).ok_or(NbError::Internal)?;
    set_manual(conn, &entity_id, field, valor.as_deref())?;

    // `set_manual` marca como rechazadas las propuestas vivas del campo,
    // incluida esta. Se corrige a aceptada, que es lo que realmente pasó.
    conn.execute(
        "UPDATE metadata_suggestions SET status = 'accepted' WHERE id = ?1",
        [suggestion_id],
    )
    .map_err(|e| fallo_de_almacenamiento(&e))?;

    // Se devuelve a quién se aplicó: quien acepta tiene que poder anunciar qué
    // edición cambió, y sin esto tendría que volver a leer la propuesta.
    Ok(entity_id)
}

/// Descarta una propuesta pendiente sin tocar el valor.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la actualización falla.
/// [`NbError::DocumentNotFound`] si la propuesta no existe o ya estaba resuelta.
pub fn reject(conn: &Connection, suggestion_id: &str) -> NbResult<()> {
    let filas = conn
        .execute(
            "UPDATE metadata_suggestions
                SET status = 'rejected', resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              WHERE id = ?1 AND status = 'pending'",
            [suggestion_id],
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    if filas == 0 {
        return Err(NbError::DocumentNotFound);
    }
    Ok(())
}

/// Propuestas vivas de una entidad.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn pending(
    conn: &Connection,
    entity_type: EntityType,
    entity_id: &str,
) -> NbResult<Vec<Suggestion>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {COLUMNAS_SUGGESTION} FROM metadata_suggestions
              WHERE entity_type = ?1 AND entity_id = ?2 AND status = 'pending'
              ORDER BY created_at, id"
        ))
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    let filas = stmt
        .query_map((entity_type.as_str(), entity_id), Suggestion::from_row)
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo_de_almacenamiento(&e))
}

/// Procedencia registrada de un campo, del origen más fuerte al más débil.
///
/// Es lo que permite a la interfaz responder "de dónde salió este dato".
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn history(conn: &Connection, entity_id: &str, field: Field) -> NbResult<Vec<Provenance>> {
    let mut stmt = conn
        .prepare(
            "SELECT entity_type, entity_id, field, source, value, recorded_at
               FROM metadata_provenance
              WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3",
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    let filas = stmt
        .query_map(
            (field.entity_type().as_str(), entity_id, field.as_str()),
            Provenance::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    let mut registros = filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    // Del más fuerte al más débil: la interfaz muestra primero el que manda.
    registros.sort_by_key(|registro| std::cmp::Reverse(registro.source));
    Ok(registros)
}

/// Valor con el que el propio documento llegó, si se registró.
///
/// Es el destino de "revertir al original" de la interfaz.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn original(conn: &Connection, entity_id: &str, field: Field) -> NbResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM metadata_provenance
          WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3 AND source = 'file'",
        (field.entity_type().as_str(), entity_id, field.as_str()),
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(|e| fallo_de_almacenamiento(&e))
}

// --- Interior ---------------------------------------------------------------

/// Entidad a la que pertenece un campo, a partir de su nombre almacenado.
///
/// La exportación no guarda el tipo de entidad en cada propuesta porque se
/// deduce de aquí, y guardarlo dos veces permitiría que los dos se
/// contradijeran.
#[must_use]
pub fn entity_type_of_field(nombre: &str) -> Option<EntityType> {
    campo_desde(nombre).map(Field::entity_type)
}

fn campo_desde(nombre: &str) -> Option<Field> {
    const TODOS: &[Field] = &[
        Field::CanonicalTitle,
        Field::OriginalLanguage,
        Field::Title,
        Field::Subtitle,
        Field::Publisher,
        Field::PublishedDate,
        Field::EditionStatement,
        Field::Language,
        Field::SeriesTitle,
        Field::SeriesIndex,
        Field::Description,
        Field::Name,
        Field::SortName,
    ];
    TODOS.iter().copied().find(|f| f.as_str() == nombre)
}

fn leer(conn: &Connection, entity_id: &str, field: Field) -> NbResult<Option<String>> {
    // La interpolación es segura porque tabla y columna salen de enumeraciones
    // cerradas, nunca de una cadena recibida. Ligar el nombre de una columna no
    // es posible en SQL.
    conn.query_row(
        &format!(
            // Como texto: `series_index` es un número, y leerlo como cadena sin
            // convertir fallaría con un tipo de columna inesperado.
            "SELECT CAST({} AS TEXT) FROM {} WHERE id = ?1",
            field.as_str(),
            field.entity_type().tabla()
        ),
        [entity_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(|e| fallo_de_almacenamiento(&e))
}

fn escribir(conn: &Connection, entity_id: &str, field: Field, value: Option<&str>) -> NbResult<()> {
    let filas = conn
        .execute(
            &format!(
                "UPDATE {} SET {} = ?2 WHERE id = ?1",
                field.entity_type().tabla(),
                field.as_str()
            ),
            rusqlite::params![entity_id, value],
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    if filas == 0 {
        // Escribir en una entidad inexistente no es un no-op inofensivo:
        // significa que quien llama tiene un identificador que ya no vale.
        return Err(NbError::DocumentNotFound);
    }
    Ok(())
}

fn registrar_procedencia(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
    value: Option<&str>,
) -> NbResult<()> {
    conn.execute(
        "INSERT INTO metadata_provenance (entity_type, entity_id, field, source, value)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (entity_type, entity_id, field, source) DO UPDATE
            SET value = excluded.value,
                recorded_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![
            field.entity_type().as_str(),
            entity_id,
            field.as_str(),
            source.as_str(),
            value,
        ],
    )
    .map(|_| ())
    .map_err(|e| fallo_de_almacenamiento(&e))
}

fn hay_decision_manual(conn: &Connection, entity_id: &str, field: Field) -> NbResult<bool> {
    conn.query_row(
        "SELECT 1 FROM metadata_provenance
          WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3 AND source = 'manual'",
        (field.entity_type().as_str(), entity_id, field.as_str()),
        |_| Ok(()),
    )
    .optional()
    .map(|encontrado| encontrado.is_some())
    .map_err(|e| fallo_de_almacenamiento(&e))
}

fn tiene_precedencia_mayor(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
) -> NbResult<bool> {
    let mut stmt = conn
        .prepare(
            "SELECT source FROM metadata_provenance
              WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3",
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;
    let mut filas = stmt
        .query(rusqlite::params![
            field.entity_type().as_str(),
            entity_id,
            field.as_str()
        ])
        .map_err(|e| fallo_de_almacenamiento(&e))?;

    while let Some(row) = filas.next().map_err(|e| fallo_de_almacenamiento(&e))? {
        let s: String = row.get(0).map_err(|e| fallo_de_almacenamiento(&e))?;
        if let Some(registrado) = Source::from_str(&s) {
            if registrado > source {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Registra una propuesta **sin escribir nunca el valor**.
///
/// Devuelve si quedó anotada. `false` cuando no había nada que proponer —el
/// valor ya es el que está guardado— o cuando ese mismo valor de ese mismo
/// origen ya se rechazó: una decisión que se olvida al siguiente clic no es una
/// decisión.
///
/// Existe, y la diferencia con [`propose`] es todo el módulo:
/// `propose` escribe cuando no hay decisión manual, y un lote que escribe mil
/// documentos en silencio es exactamente lo que no se puede deshacer. Aquí no
/// se escribe nunca, así que "reversible" no necesita nada que revertir.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn sugerir(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
    value: Option<&str>,
) -> NbResult<bool> {
    if source == Source::Manual {
        // Lo manual no se propone: se decide. Dejar que algo automático
        // registrara una propuesta "manual" sería hacerla pasar por el usuario.
        return Ok(false);
    }
    if leer(conn, entity_id, field)?.as_deref() == value {
        return Ok(false);
    }
    if fue_rechazada(conn, entity_id, field, source, value)? {
        return Ok(false);
    }
    registrar_propuesta(conn, entity_id, field, source, value)?;
    Ok(true)
}

fn registrar_propuesta(
    conn: &Connection,
    entity_id: &str,
    field: Field,
    source: Source,
    value: Option<&str>,
) -> NbResult<Suggestion> {
    conn.query_row(
        &format!(
            "INSERT INTO metadata_suggestions
                 (entity_type, entity_id, field, source, proposed_value)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (entity_type, entity_id, field, source) WHERE status = 'pending'
             DO UPDATE SET proposed_value = excluded.proposed_value,
                           created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             RETURNING {COLUMNAS_SUGGESTION}"
        ),
        rusqlite::params![
            field.entity_type().as_str(),
            entity_id,
            field.as_str(),
            source.as_str(),
            value,
        ],
        Suggestion::from_row,
    )
    .map_err(|e| fallo_de_almacenamiento(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::catalog::{Edition, NewEdition, NewWork, Work};
    use crate::db::Database;

    struct Escenario {
        db: Database,
        edicion: String,
    }

    fn escenario() -> Escenario {
        let db = Database::open_in_memory().expect("base en memoria");
        let edicion = {
            let conn = db.connection().expect("conexión");
            let obra = Work::create(
                &conn,
                &NewWork {
                    canonical_title: "Obra".to_owned(),
                    original_language: None,
                },
            )
            .expect("obra");
            Edition::create(
                &conn,
                &NewEdition {
                    work_id: obra.id,
                    title: "Titulo del archivo".to_owned(),
                    ..NewEdition::default()
                },
            )
            .expect("edición")
            .id
        };
        Escenario { db, edicion }
    }

    fn titulo(conn: &Connection, edicion: &str) -> String {
        conn.query_row("SELECT title FROM editions WHERE id = ?1", [edicion], |r| {
            r.get(0)
        })
        .expect("consulta")
    }

    #[test]
    fn limpiar_entidades_respeta_lo_manual_y_actualiza_la_procedencia_del_archivo() {
        let esc = escenario();
        let mut conn = esc.db.connection().unwrap();
        propose(
            &conn,
            &esc.edicion,
            Field::SeriesTitle,
            Source::File,
            Some("Áncora &amp; Delfín"),
        )
        .unwrap();
        set_manual(
            &conn,
            &esc.edicion,
            Field::Description,
            Some("Conservar &amp; como texto"),
        )
        .unwrap();
        limpiar_entidades_guardadas(&mut conn);
        let valores: (String, String) = conn
            .query_row(
                "SELECT series_title, description FROM editions WHERE id=?1",
                [&esc.edicion],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            valores,
            (
                "Áncora & Delfín".into(),
                "Conservar &amp; como texto".into()
            )
        );
        let origen: String = conn.query_row(
            "SELECT value FROM metadata_provenance WHERE entity_id=?1 AND field='series_title' AND source='file'",
            [&esc.edicion], |r| r.get(0),
        ).unwrap();
        assert_eq!(origen, "Áncora & Delfín");
    }

    #[test]
    fn la_limpieza_no_reinterpreta_metadatos_de_calibre() {
        let esc = escenario();
        let mut conn = esc.db.connection().unwrap();
        propose(
            &conn,
            &esc.edicion,
            Field::SeriesTitle,
            Source::Calibre,
            Some("A &amp; B"),
        )
        .unwrap();
        limpiar_entidades_guardadas(&mut conn);
        let actual: String = conn
            .query_row(
                "SELECT series_title FROM editions WHERE id=?1",
                [&esc.edicion],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, "A &amp; B");
    }

    #[test]
    fn una_lectura_antigua_de_metadatos_no_pisa_una_actualizacion_posterior() {
        let esc = escenario();
        let mut conn = esc.db.connection().unwrap();
        propose(
            &conn,
            &esc.edicion,
            Field::SeriesTitle,
            Source::File,
            Some("A &amp; B"),
        )
        .unwrap();
        conn.execute(
            "UPDATE editions SET series_title='Valor posterior' WHERE id=?1",
            [&esc.edicion],
        )
        .unwrap();
        assert!(
            !limpiar_entidad_guardada(&mut conn, &esc.edicion, Some("A &amp; B"), None).unwrap()
        );
        let actual: String = conn
            .query_row(
                "SELECT series_title FROM editions WHERE id=?1",
                [&esc.edicion],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, "Valor posterior");
    }

    #[test]
    fn una_limpieza_fallida_se_reintenta_sin_marcarla_como_terminada() {
        let esc = escenario();
        let mut conn = esc.db.connection().unwrap();
        propose(
            &conn,
            &esc.edicion,
            Field::SeriesTitle,
            Source::File,
            Some("A &amp; B"),
        )
        .unwrap();
        conn.execute_batch("CREATE TEMP TRIGGER impedir_limpieza BEFORE UPDATE ON editions BEGIN SELECT RAISE(ABORT, 'fallo de prueba'); END;").unwrap();
        limpiar_entidades_guardadas(&mut conn);
        let marcas: u32 = conn
            .query_row(
                "SELECT count(*) FROM app_metadata WHERE key='entidades_limpias_v2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(marcas, 0);
        conn.execute_batch("DROP TRIGGER impedir_limpieza").unwrap();
        limpiar_entidades_guardadas(&mut conn);
        let actual: String = conn
            .query_row(
                "SELECT series_title FROM editions WHERE id=?1",
                [&esc.edicion],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, "A & B");
        let marcas: u32 = conn
            .query_row(
                "SELECT count(*) FROM app_metadata WHERE key='entidades_limpias_v2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(marcas, 1);
    }

    /// **Criterio de aceptación.** Se edita un título a mano, llega
    /// una actualización automática con otro valor, y el valor manual
    /// sobrevive mientras el propuesto queda registrado como sugerencia.
    ///
    /// Si esta prueba falla, el proyecto pisa en silencio correcciones del
    /// usuario, que es el defecto que la sección 3 de metadata-model.md existe
    /// para impedir.
    #[test]
    fn una_actualizacion_automatica_no_pisa_una_edicion_manual() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        set_manual(
            &conn,
            &esc.edicion,
            Field::Title,
            Some("Título corregido a mano"),
        )
        .expect("edición manual");

        let resultado = propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Titulo Mal Capitalizado Del Proveedor"),
        )
        .expect("propuesta");

        // El valor manual sobrevive intacto.
        assert_eq!(titulo(&conn, &esc.edicion), "Título corregido a mano");

        // Y el automático no se pierde: queda como propuesta.
        let Outcome::Suggested(sugerencia) = resultado else {
            panic!("el valor automático debía quedar como propuesta: {resultado:?}");
        };
        assert_eq!(
            sugerencia.proposed_value.as_deref(),
            Some("Titulo Mal Capitalizado Del Proveedor")
        );
        assert_eq!(sugerencia.status, SuggestionStatus::Pending);
        assert_eq!(sugerencia.source, Source::Online);

        let vivas = pending(&conn, EntityType::Edition, &esc.edicion).expect("propuestas");
        assert_eq!(vivas.len(), 1);
        assert_eq!(vivas[0].id, sugerencia.id);
    }

    /// Sin decisión del usuario no hay nada que proteger: el valor automático se
    /// aplica. Lo contrario dejaría la biblioteca sin metadatos hasta que
    /// alguien los escribiera a mano uno por uno.
    #[test]
    fn sin_edicion_manual_previa_el_valor_automatico_se_aplica() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        let resultado = propose(
            &conn,
            &esc.edicion,
            Field::Publisher,
            Source::File,
            Some("Editorial del documento"),
        )
        .expect("propuesta");

        assert_eq!(resultado, Outcome::Applied);
        let editorial: Option<String> = conn
            .query_row(
                "SELECT publisher FROM editions WHERE id = ?1",
                [&esc.edicion],
                |r| r.get(0),
            )
            .expect("consulta");
        assert_eq!(editorial.as_deref(), Some("Editorial del documento"));
    }

    /// Consultar el mismo proveedor diez veces produce diez propuestas idénticas
    /// y ninguna información nueva. La segunda no debe generar ruido.
    #[test]
    fn proponer_el_valor_que_ya_esta_no_genera_ruido() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        propose(
            &conn,
            &esc.edicion,
            Field::Publisher,
            Source::Online,
            Some("Editorial"),
        )
        .expect("primera");

        let segunda = propose(
            &conn,
            &esc.edicion,
            Field::Publisher,
            Source::Online,
            Some("Editorial"),
        )
        .expect("segunda");

        assert_eq!(segunda, Outcome::Unchanged);
        assert!(pending(&conn, EntityType::Edition, &esc.edicion)
            .expect("propuestas")
            .is_empty());
    }

    #[test]
    fn dos_propuestas_del_mismo_origen_para_el_mismo_campo_no_se_acumulan() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Mío")).expect("manual");

        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Primera propuesta"),
        )
        .expect("primera");
        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Segunda propuesta"),
        )
        .expect("segunda");

        let vivas = pending(&conn, EntityType::Edition, &esc.edicion).expect("propuestas");
        assert_eq!(vivas.len(), 1, "una sola propuesta viva por campo y origen");
        assert_eq!(
            vivas[0].proposed_value.as_deref(),
            Some("Segunda propuesta")
        );
    }

    /// Orígenes distintos discrepan por separado: cada uno merece su propuesta.
    #[test]
    fn dos_origenes_distintos_dejan_dos_propuestas() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Mío")).expect("manual");

        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Del proveedor"),
        )
        .expect("en línea");
        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Calibre,
            Some("De Calibre"),
        )
        .expect("de Calibre");

        assert_eq!(
            pending(&conn, EntityType::Edition, &esc.edicion)
                .expect("propuestas")
                .len(),
            2
        );
    }

    #[test]
    fn aceptar_una_propuesta_la_escribe_y_pasa_a_ser_decision_del_usuario() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Mío")).expect("manual");

        let Outcome::Suggested(sugerencia) = propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("El del proveedor"),
        )
        .expect("propuesta") else {
            panic!("debía proponerse");
        };

        accept(&conn, &sugerencia.id).expect("aceptada");

        assert_eq!(titulo(&conn, &esc.edicion), "El del proveedor");
        assert!(pending(&conn, EntityType::Edition, &esc.edicion)
            .expect("propuestas")
            .is_empty());

        // Y queda como decisión del usuario, no del proveedor: una actualización
        // automática posterior tampoco puede pisarla.
        let posterior = propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Otro más"),
        )
        .expect("posterior");
        assert!(matches!(posterior, Outcome::Suggested(_)));
        assert_eq!(titulo(&conn, &esc.edicion), "El del proveedor");
    }

    #[test]
    fn rechazar_una_propuesta_no_toca_el_valor() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Mío")).expect("manual");

        let Outcome::Suggested(sugerencia) = propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("No, gracias"),
        )
        .expect("propuesta") else {
            panic!("debía proponerse");
        };

        reject(&conn, &sugerencia.id).expect("rechazada");

        assert_eq!(titulo(&conn, &esc.edicion), "Mío");
        assert!(pending(&conn, EntityType::Edition, &esc.edicion)
            .expect("propuestas")
            .is_empty());
        assert!(matches!(
            reject(&conn, &sugerencia.id),
            Err(NbError::DocumentNotFound)
        ));
    }

    /// Decidir a mano retira lo que ya se había propuesto para ese campo:
    /// ofrecer una alternativa que el usuario acaba de descartar es ruido.
    #[test]
    fn decidir_a_mano_retira_las_propuestas_vivas_de_ese_campo() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::File,
            Some("Del archivo"),
        )
        .expect("del archivo");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Primera decisión"))
            .expect("primera decisión");
        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Del proveedor"),
        )
        .expect("propuesta");

        assert_eq!(
            pending(&conn, EntityType::Edition, &esc.edicion)
                .expect("propuestas")
                .len(),
            1
        );

        set_manual(&conn, &esc.edicion, Field::Title, Some("Segunda decisión"))
            .expect("segunda decisión");

        assert!(pending(&conn, EntityType::Edition, &esc.edicion)
            .expect("propuestas")
            .is_empty());
        assert_eq!(titulo(&conn, &esc.edicion), "Segunda decisión");
    }

    /// La interfaz tiene que poder decir de dónde salió un dato y ofrecer volver
    /// al original del documento.
    #[test]
    fn la_procedencia_conserva_el_valor_original_del_documento() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::File,
            Some("Título tal cual venía"),
        )
        .expect("del archivo");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Título corregido")).expect("manual");

        let historial = history(&conn, &esc.edicion, Field::Title).expect("historial");

        assert_eq!(historial.len(), 2);
        assert_eq!(historial[0].source, Source::Manual, "manda el manual");
        assert_eq!(historial[1].source, Source::File);

        assert_eq!(
            original(&conn, &esc.edicion, Field::Title).expect("original"),
            Some("Título tal cual venía".to_owned())
        );
    }

    /// Un origen automático no puede hacerse pasar por el usuario para saltarse
    /// la precedencia.
    #[test]
    fn un_origen_automatico_no_puede_declararse_manual_para_colarse() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        // Pasar `Source::Manual` a `propose` se trata como lo que es: una
        // edición del usuario, no un atajo con otro nombre.
        let resultado = propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Manual,
            Some("Como si fuera yo"),
        )
        .expect("aplicada");

        assert_eq!(resultado, Outcome::Applied);
        let historial = history(&conn, &esc.edicion, Field::Title).expect("historial");
        assert_eq!(historial.len(), 1);
        assert_eq!(historial[0].source, Source::Manual);
    }

    #[test]
    fn escribir_en_una_entidad_inexistente_es_un_error_y_no_un_silencio() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        let resultado = set_manual(&conn, &"0".repeat(32), Field::Title, Some("Nada"));

        assert!(matches!(resultado, Err(NbError::DocumentNotFound)));
    }

    /// Borrar la entidad se lleva su procedencia y sus propuestas: sin clave
    /// foránea posible, lo hacen los disparadores.
    #[test]
    fn borrar_la_entidad_no_deja_procedencia_ni_propuestas_huerfanas() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");
        set_manual(&conn, &esc.edicion, Field::Title, Some("Mío")).expect("manual");
        propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Online,
            Some("Otro"),
        )
        .expect("propuesta");

        conn.execute("DELETE FROM editions WHERE id = ?1", [&esc.edicion])
            .expect("borrado");

        for tabla in ["metadata_provenance", "metadata_suggestions"] {
            let filas: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
                .expect("consulta");
            assert_eq!(filas, 0, "quedaron filas huérfanas en {tabla}");
        }
    }

    /// El nombre del campo acaba interpolado en un `UPDATE`. Que venga de una
    /// enumeración cerrada es lo que hace que eso sea seguro; esta prueba fija
    /// que cada campo apunta a su entidad y no a otra.
    #[test]
    fn cada_campo_pertenece_a_su_entidad() {
        assert_eq!(Field::CanonicalTitle.entity_type(), EntityType::Work);
        assert_eq!(Field::Title.entity_type(), EntityType::Edition);
        assert_eq!(Field::SortName.entity_type(), EntityType::Contributor);

        for nombre in [
            "canonical_title",
            "original_language",
            "title",
            "subtitle",
            "publisher",
            "published_date",
            "edition_statement",
            "language",
            "series_title",
            "series_index",
            "description",
            "name",
            "sort_name",
        ] {
            assert!(
                campo_desde(nombre).is_some(),
                "el campo {nombre} no se reconoce al releerlo de la base"
            );
        }
        assert!(campo_desde("title; DROP TABLE editions").is_none());
    }

    /// La precedencia es el orden de la enumeración, y el manual manda.
    #[test]
    fn el_origen_manual_tiene_la_maxima_precedencia() {
        assert!(Source::Manual > Source::Calibre);
        assert!(Source::Calibre > Source::Online);
        assert!(Source::Online > Source::File);
    }

    #[test]
    fn calibre_gana_sobre_archivo_y_manual_gana_sobre_calibre() {
        let esc = escenario();
        let conn = esc.db.connection().expect("conexión");

        // Calibre escribe el título
        record_origin(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::Calibre,
            Some("Título Calibre"),
        )
        .expect("calibre");
        escribir(&conn, &esc.edicion, Field::Title, Some("Título Calibre")).expect("escribir");

        // Archivo intenta proponer otro título: queda como propuesta porque Calibre tiene mayor precedencia
        let res = propose(
            &conn,
            &esc.edicion,
            Field::Title,
            Source::File,
            Some("Título Archivo"),
        )
        .expect("propose");
        assert!(matches!(res, Outcome::Suggested(_)));
        assert_eq!(
            leer(&conn, &esc.edicion, Field::Title)
                .expect("leer")
                .as_deref(),
            Some("Título Calibre")
        );

        // Manual edita: gana sobre Calibre
        let res_manual =
            set_manual(&conn, &esc.edicion, Field::Title, Some("Título Manual")).expect("manual");
        assert_eq!(res_manual, Outcome::Applied);
        assert_eq!(
            leer(&conn, &esc.edicion, Field::Title)
                .expect("leer")
                .as_deref(),
            Some("Título Manual")
        );
    }
}

// ---------------------------------------------------------------------------
// Enriquecimiento desde el propio documento
// ---------------------------------------------------------------------------

/// Qué se sacó del documento.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Enrichment {
    /// Campos que se aplicaron porque nadie los había decidido a mano.
    pub applied: Vec<String>,
    /// Campos que discreparon de una decisión del usuario y quedaron propuestos.
    pub suggested: Vec<String>,
    pub contributors: usize,
    pub subjects: usize,
    pub identifiers: usize,
    /// Ruta de la portada extraída, si el documento traía una.
    pub cover: Option<PathBuf>,
}

/// Traduce el papel leído del archivo al que guarda el catálogo.
///
/// Son dos listas cerradas y equivalentes, en dos capas distintas: la de `epub`
/// habla de lo que dice el archivo y la de `catalog` de lo que guarda la base.
/// Tenerlas separadas es lo que permite que una crezca sin arrastrar a la otra.
const fn papel_de(papel: crate::epub::Papel) -> ContributorRole {
    match papel {
        crate::epub::Papel::Autor => ContributorRole::Author,
        crate::epub::Papel::Editor => ContributorRole::Editor,
        crate::epub::Papel::Traductor => ContributorRole::Translator,
        crate::epub::Papel::Ilustrador => ContributorRole::Illustrator,
    }
}

/// Extrae los metadatos internos de un documento y los incorpora.
///
/// **Nunca sobrescribe una decisión del usuario:** todo entra por [`propose`]
/// con origen `File`, de modo que lo que alguien corrigió a mano sigue ganando y
/// lo que el documento diga distinto queda como propuesta.
///
/// La portada se extrae aparte y su fallo no afecta a lo demás: un documento sin
/// portada es un documento, y uno cuya portada no se pudo escribir también.
///
/// # Errors
///
/// - [`NbError::FormatUnsupported`] si todavía no sabemos leer ese tipo de medio.
/// - [`NbError::DocumentCorrupted`] si el documento no se deja analizar.
/// - [`NbError::StorageFailure`] si falla la escritura en la base.
pub fn enrich(
    conn: &Connection,
    edition_id: &str,
    file_path: &Path,
    media_type: &str,
    covers_dir: &Path,
) -> NbResult<Enrichment> {
    // Cada formato dice lo suyo con su propio lector: el EPUB su paquete OPF, el
    // cómic su `ComicInfo.xml` y su primera página, y el PDF su diccionario de
    // información y su primera página pintada por el motor.
    let (interno, portada_pintada) = match media_type {
        "application/epub+zip" => (crate::epub::read_metadata(file_path)?, None),
        "application/vnd.comicbook+zip" => (crate::reader::comic::read_metadata(file_path)?, None),
        "application/pdf" => leer_pdf(file_path, covers_dir)?,
        _ => return Err(NbError::FormatUnsupported),
    };
    let mut informe = Enrichment::default();

    for (campo, valor) in [
        (Field::Title, interno.title.as_deref()),
        (Field::Language, interno.language.as_deref()),
        (Field::Publisher, interno.publisher.as_deref()),
        (
            Field::PublishedDate,
            normalizar_fecha(interno.date.as_deref()),
        ),
        // La serie y la sinopsis, que el paquete traía y nadie leía.
        (Field::SeriesTitle, interno.series.as_deref()),
        (Field::SeriesIndex, interno.series_index.as_deref()),
        (Field::Description, interno.description.as_deref()),
    ] {
        let Some(valor) = valor else { continue };
        match propose(conn, edition_id, campo, Source::File, Some(valor))? {
            Outcome::Applied => informe.applied.push(campo.as_str().to_owned()),
            Outcome::Suggested(_) => informe.suggested.push(campo.as_str().to_owned()),
            Outcome::Unchanged => {}
        }
    }

    // El título canónico de la obra sigue al de la edición mientras nadie lo
    // haya tocado: una obra que se llama como su archivo y una edición que se
    // llama bien es una biblioteca que se busca mal.
    if let Some(titulo) = interno.title.as_deref() {
        if let Some(work_id) = obra_de(conn, edition_id)? {
            propose(
                conn,
                &work_id,
                Field::CanonicalTitle,
                Source::File,
                Some(titulo),
            )?;
        }
    }

    // El papel sale del archivo y no se supone: ver `epub::Creador`. Antes todos
    // entraban como autores, y con ello entraban las herramientas que empaquetan
    // el EPUB.
    //
    // Y el autor que ya existe se reutiliza: cada obra lo daba de alta
    // otra vez, y la biblioteca real tenía 207.282 filas para 61.187 nombres.
    for (posicion, creador) in interno.creators.iter().enumerate() {
        let quien = Contributor::ensure(conn, &creador.name)?;
        Contributor::link(
            conn,
            edition_id,
            &quien.id,
            papel_de(creador.role),
            i64::try_from(posicion).unwrap_or(i64::MAX),
        )?;
        informe.contributors += 1;
    }

    for materia in &interno.subjects {
        let sujeto = Subject::ensure(conn, "local", materia)?;
        Subject::link(conn, edition_id, &sujeto.id)?;
        informe.subjects += 1;
    }

    for (esquema, valor) in &interno.identifiers {
        if let Some(conocido) = esquema_conocido(esquema) {
            Edition::add_identifier(conn, edition_id, conocido, valor)?;
            informe.identifiers += 1;
        }
    }

    // La portada, al final y sin poder tumbar nada de lo anterior.
    let extraida = match (&portada_pintada, interno.cover_path.as_deref()) {
        (Some(png), _) => {
            let nombre = format!("{edition_id}.png");
            Some(crate::covers::guardar(covers_dir, &nombre, png).map(|ruta| (nombre, ruta)))
        }
        (None, Some(interna)) => Some(extraer_portada(file_path, interna, edition_id, covers_dir)),
        (None, None) => None,
    };
    if let Some(extraida) = extraida {
        match extraida {
            Ok((nombre, ruta)) => {
                // Se apunta **el nombre** en el catálogo, no la ruta: la caché
                // cambia de sitio entre equipos y versiones del sistema, y una
                // ruta absoluta guardada aquí sería una ruta rota. Sin este
                // apunte, la portada se extraía y no la encontraba nadie: la
                // interfaz enseñaba la generada aunque el libro trajera la suya.
                if let Err(error) = conn.execute(
                    "UPDATE editions SET cover_path = ?1 WHERE id = ?2",
                    rusqlite::params![nombre, edition_id],
                ) {
                    tracing::warn!(error = %error, "no se pudo apuntar la portada");
                }
                informe.cover = Some(ruta);
            }
            Err(_) => {
                tracing::warn!("no se pudo extraer la portada; el documento no se ve afectado")
            }
        }
    }

    Ok(informe)
}

/// Lo que un PDF dice de sí mismo, y su primera página como portada.
///
/// Un título que es un nombre de archivo o la firma del programa que generó el
/// PDF no es un título: se descarta para que no sustituya al del archivo, que
/// suele ser mejor.
fn leer_pdf(
    file_path: &Path,
    covers_dir: &Path,
) -> NbResult<(crate::epub::EpubMetadata, Option<Vec<u8>>)> {
    let paginas = covers_dir.with_file_name("paginas");
    let documento = crate::reader::pdf::PdfDocument::open(file_path, &paginas)?;
    let (titulo, autor) = documento.declared_metadata();
    let titulo = titulo.filter(|t| {
        let minusculas = t.to_lowercase();
        ![".pdf", ".doc", ".docx", ".tex", ".dvi", ".indd"]
            .iter()
            .any(|ext| minusculas.ends_with(ext))
            && !minusculas.starts_with("microsoft word")
            && !minusculas.starts_with("untitled")
    });
    let mut datos = crate::epub::EpubMetadata {
        title: titulo,
        ..Default::default()
    };
    if let Some(autor) = autor {
        for nombre in autor
            .split([';', '&'])
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            datos.creators.push(crate::epub::Creador {
                name: nombre.to_owned(),
                role: crate::epub::Papel::Autor,
            });
        }
    }
    let portada = documento
        .render_page(1)
        .ok()
        .and_then(|ruta| std::fs::read(ruta).ok());
    Ok((datos, portada))
}

/// Deja la fecha en una de las tres formas que el esquema admite.
///
/// Los EPUB traen fechas en todos los formatos imaginables. Quedarse con la
/// parte que se entiende es mejor que descartar el campo entero: saber el año es
/// saber algo.
fn normalizar_fecha(fecha: Option<&str>) -> Option<&str> {
    let texto = fecha?.trim();
    for largo in [10, 7, 4] {
        if texto.len() >= largo {
            let recorte = &texto[..largo];
            let valido = match largo {
                4 => recorte.chars().all(|c| c.is_ascii_digit()),
                7 => es_fecha_parcial(recorte, 1),
                _ => es_fecha_parcial(recorte, 2),
            };
            if valido {
                return Some(recorte);
            }
        }
    }
    None
}

fn es_fecha_parcial(texto: &str, guiones: usize) -> bool {
    texto.matches('-').count() == guiones
        && texto
            .split('-')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn esquema_conocido(esquema: &str) -> Option<IdentifierScheme> {
    match esquema.to_ascii_lowercase().as_str() {
        "isbn" => Some(IdentifierScheme::Isbn),
        "issn" => Some(IdentifierScheme::Issn),
        "doi" => Some(IdentifierScheme::Doi),
        "asin" => Some(IdentifierScheme::Asin),
        "oclc" => Some(IdentifierScheme::Oclc),
        // Un `urn:uuid:` o cualquier esquema propio del generador entra como
        // local: descartarlo perdería la única identidad que trae el archivo.
        _ => Some(IdentifierScheme::Local),
    }
}

fn obra_de(conn: &Connection, edition_id: &str) -> NbResult<Option<String>> {
    conn.query_row(
        "SELECT work_id FROM editions WHERE id = ?1",
        [edition_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| fallo_de_almacenamiento(&e))
}

/// Extrae la portada tal cual viene, **sin recodificarla**.
///
/// Redimensionar necesitaría un decodificador de imágenes, que es una
/// dependencia grande, y sobre todo necesitaría saber a qué tamaño, cosa que
/// depende de una interfaz que todavía no existe. Guardar los bytes originales
/// no cierra ninguna puerta y no cuesta nada.
///
/// Va a la **caché**, no a los datos: es regenerable a partir del documento, y
/// la regla 5 de `docs/architecture/api/schemas.md` manda que lo regenerable viva fuera del
/// directorio que se respalda.
/// Registra en el catálogo las portadas que ya estaban extraídas en la caché.
///
/// # Por qué hace falta esto y no basta la migración
///
/// La portada se extrae desde la primera versión y se guarda como
/// `<edition_id>.<extensión>`, pero hasta la V14 no había dónde apuntar cuál era.
/// El resultado es que una biblioteca ya importada tiene sus portadas en el
/// disco y ninguna registrada: la migración añade la columna y la deja vacía,
/// así que la interfaz seguiría pintando la generada aunque la buena esté ahí al
/// lado. Una migración no puede mirar el sistema de archivos; esto sí.
///
/// # Por qué una sola vez
///
/// Recorrer el directorio cuesta lo que cuesta con ciento cincuenta mil
/// documentos, y a partir del primer arranque no hay nada que reconciliar: las
/// importaciones nuevas apuntan su portada al extraerla. Se deja constancia en
/// `app_metadata` para no volver a mirar.
///
/// Un fallo aquí **no impide arrancar**: se pierde la portada de los documentos
/// antiguos, que es exactamente lo que ya pasaba, y no vale la pena cambiar eso
/// por no abrir la aplicación.
pub fn reconciliar_portadas(conn: &Connection, covers_dir: &Path) {
    const MARCA: &str = "covers_reconciled_v14";

    let hecho: bool = conn
        .query_row(
            "SELECT count(*) FROM app_metadata WHERE key = ?1",
            [MARCA],
            |fila| fila.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if hecho {
        return;
    }

    let Ok(entradas) = std::fs::read_dir(covers_dir) else {
        // Sin directorio de portadas no hay nada que reconciliar, y eso es lo
        // normal en una instalación recién estrenada.
        let _ = marcar(conn, MARCA);
        return;
    };

    let mut registradas = 0_usize;
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let (Some(nombre), Some(edicion)) = (
            ruta.file_name().and_then(|n| n.to_str()),
            ruta.file_stem().and_then(|n| n.to_str()),
        ) else {
            continue;
        };

        match conn.execute(
            "UPDATE editions SET cover_path = ?1 WHERE id = ?2 AND cover_path IS NULL",
            rusqlite::params![nombre, edicion],
        ) {
            Ok(filas) => registradas += filas,
            Err(error) => tracing::warn!(error = %error, "no se pudo registrar una portada"),
        }
    }

    tracing::info!(
        registradas,
        "portadas de la caché registradas en el catálogo"
    );
    let _ = marcar(conn, MARCA);
}

/// Cuántas ediciones reprocesa una pasada de reconciliación.
///
/// Acotada porque cada una abre su archivo: sin tope, una biblioteca de ciento
/// cincuenta mil documentos convertiría cada arranque en una reimportación. Con
/// tope, converge en unas cuantas sesiones y ninguna se nota.
const POR_PASADA: u32 = 200;

/// Vuelve a extraer los metadatos de lo que se quedó sin autor.
///
/// # Por qué hace falta, y por qué no basta con arreglar la causa
///
/// La extracción de metadatos ocurre **fuera de la ruta de importación**: escucha
/// el bus y trabaja después. Si falla -por contención de escritura, por un
/// archivo que en ese momento no se dejaba abrir, o porque la aplicación se
/// cerró antes de que le tocara-, el documento se queda con el nombre de su
/// archivo por título y sin nadie a quien atribuirlo, y **nadie lo vuelve a
/// intentar**: el evento ya pasó. En la instalación real eso fue el **20 % del
/// catálogo**, treinta mil ediciones.
///
/// Arreglar la causa evita que vuelva a pasar, pero no repara lo que ya
/// está dentro. Esto sí, y por eso es una pasada y no un arreglo: se ejecuta
/// mientras la aplicación está abierta, acotada, y cada vez alcanza a unas
/// cuantas más.
///
/// # Qué se considera "sin metadatos"
///
/// No tener **ninguna autoría** y tener un archivo disponible que leer; en un
/// PDF o un cómic, además, no tener todavía portada. El
/// título no sirve como señal: un libro puede llamarse igual que su archivo
/// legítimamente. La portada tampoco: hay EPUB que no traen ninguna, y volver a
/// abrirlos en cada arranque para no encontrarla sería trabajo garantizado sin
/// resultado posible.
///
/// Devuelve cuántas quedaron con autor tras la pasada.
pub fn reconciliar_metadatos(conn: &Connection, covers_dir: &Path) -> u32 {
    let pendientes: Vec<(String, String, String)> = {
        let Ok(mut stmt) = conn.prepare(&format!(
            "SELECT e.id, f.media_type, {ruta}
               FROM editions e
               JOIN files f ON f.edition_id = e.id
              WHERE f.availability = 'available'
                AND NOT EXISTS (
                    SELECT 1 FROM edition_contributors ec WHERE ec.edition_id = e.id
                )
                -- Un PDF o un cómic sin autor declarado es lo normal: se lee una
                -- vez, y la portada que deja es la marca de que ya se leyó. Sin
                -- esto se volvería a pintar su primera página en cada pasada.
                AND (f.media_type = 'application/epub+zip' OR e.cover_path IS NULL)
              GROUP BY e.id
              LIMIT {POR_PASADA}",
            ruta = crate::db::catalog::SQL_RUTA_ABSOLUTA
        )) else {
            return 0;
        };

        let filas = stmt.query_map([], |fila| {
            Ok((
                fila.get::<_, String>(0)?,
                fila.get::<_, String>(1)?,
                fila.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        });

        match filas {
            Ok(filas) => filas.filter_map(Result::ok).collect(),
            Err(_) => return 0,
        }
    };

    if pendientes.is_empty() {
        return 0;
    }

    let mut recuperadas = 0;
    for (edition_id, media_type, ruta) in &pendientes {
        if ruta.is_empty() {
            continue;
        }
        if let Ok(informe) = enrich(conn, edition_id, Path::new(ruta), media_type, covers_dir) {
            if informe.contributors > 0 {
                recuperadas += 1;
            }
        }
    }

    // Números agregados y ninguna ruta: es material del usuario.
    tracing::info!(
        miradas = pendientes.len(),
        recuperadas,
        "pasada de reconciliación de metadatos"
    );
    recuperadas
}

/// Vuelve a sacar del documento las portadas que la caché ya no tiene.
///
/// # Por qué hace falta
///
/// Porque la caché es caché: un limpiador del sistema la vacía y el catálogo
/// sigue apuntando el nombre lógico de cada portada. Se encontró el
/// 2026-09-18 al vaciar la del sistema: la biblioteca entera se quedó con
/// imágenes rotas, una por obra, y nada volvía a generarlas. Lo derivado que no
/// se puede rehacer no es derivado, es un dato perdido.
///
/// # Por qué por lista y no por pasada entera
///
/// Porque quien la llama sabe **qué se está mirando ahora**: la página de la
/// rejilla que acaba de pedirse. Con ciento cincuenta mil obras, recorrer el
/// catálogo entero al arrancar sería una reimportación; recuperar las cien que
/// se ven converge mientras se navega y no se nota.
///
/// Devuelve cuántas quedaron con portada.
pub fn recuperar_portadas(conn: &Connection, covers_dir: &Path, ediciones: &[String]) -> u32 {
    if ediciones.is_empty() {
        return 0;
    }

    let mut recuperadas = 0;
    for edition_id in ediciones {
        let fila: Option<(String, String)> = conn
            .query_row(
                &format!(
                    "SELECT f.media_type, {ruta}
                       FROM files f
                      WHERE f.edition_id = ?1 AND f.availability = 'available'
                      LIMIT 1",
                    ruta = crate::db::catalog::SQL_RUTA_ABSOLUTA
                ),
                [edition_id],
                |f| {
                    Ok((
                        f.get(0)?,
                        f.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    ))
                },
            )
            .ok();
        let Some((media_type, ruta)) = fila else {
            continue;
        };
        // Sin archivo no hay de dónde sacarla, y no es un fallo: es un documento
        // que no está disponible. La rejilla ya sabe pintar eso.
        if ruta.is_empty() || media_type != "application/epub+zip" {
            continue;
        }

        let documento = Path::new(&ruta);
        let Ok(interno) = crate::epub::read_metadata(documento) else {
            continue;
        };
        let Some(interna) = interno.cover_path.as_deref() else {
            continue;
        };
        if let Ok((nombre, _)) = extraer_portada(documento, interna, edition_id, covers_dir) {
            // El nombre lógico puede no ser el que había apuntado -la extensión
            // del recurso puede haber cambiado entre versiones-, así que se
            // vuelve a apuntar en lugar de darlo por bueno.
            let _ = conn.execute(
                "UPDATE editions SET cover_path = ?1 WHERE id = ?2",
                rusqlite::params![nombre, edition_id],
            );
            recuperadas += 1;
            ceder_el_paso(PAUSA_ENTRE_ESCRITURAS_DE_FONDO);
        }
    }

    // Números agregados y ninguna ruta: es material del usuario.
    tracing::info!(
        miradas = ediciones.len(),
        recuperadas,
        "portadas recuperadas del documento"
    );
    recuperadas
}

/// Si ya hay una recuperación de portadas en marcha.
///
/// Una a la vez en todo el proceso: cada página de la rejilla pediría la suya, y
/// desplazarse deprisa lanzaría veinte hilos abriendo EPUB a la vez. Con una
/// bandera, la que está en marcha termina y la siguiente página vuelve a pedir
/// lo que le falte.
static RECUPERANDO_PORTADAS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Cuántas portadas se recuperan de una vez.
///
/// Una página de la rejilla, que es lo que se está mirando. Cada una abre su
/// EPUB y reescribe su miniatura: unos 40 ms, así que cien son cuatro segundos
/// de trabajo en un hilo de fondo.
const PORTADAS_POR_TANDA: usize = 100;

/// Recupera portadas en segundo plano y avisa cuando hay algo nuevo que pintar.
///
/// No espera: la llamada vuelve enseguida y el trabajo ocurre en su hilo. Si ya
/// hay una tanda en marcha, no hace nada; la página siguiente volverá a pedirlo.
pub fn recuperar_portadas_en_segundo_plano(
    db: crate::db::Database,
    covers_dir: std::path::PathBuf,
    bus: EventBus,
    ediciones: Vec<String>,
) {
    use std::sync::atomic::Ordering;

    if ediciones.is_empty() || RECUPERANDO_PORTADAS.swap(true, Ordering::SeqCst) {
        return;
    }

    let hilo = std::thread::Builder::new()
        .name("nb-portadas".to_owned())
        .spawn(move || {
            let recuperadas = db
                .connection()
                .map(|conn| {
                    let tanda: Vec<String> =
                        ediciones.into_iter().take(PORTADAS_POR_TANDA).collect();
                    recuperar_portadas(&conn, &covers_dir, &tanda)
                })
                .unwrap_or(0);
            RECUPERANDO_PORTADAS.store(false, Ordering::SeqCst);
            if recuperadas > 0 {
                bus.emit(DomainEvent::CoversRecovered { count: recuperadas });
            }
        });

    if hilo.is_err() {
        RECUPERANDO_PORTADAS.store(false, Ordering::SeqCst);
    }
}

/// Presenta como título lo que todavía es el nombre de un archivo.
///
/// # Por qué hace falta una pasada, y no basta con arreglar la importación
///
/// Porque lo ya importado no se vuelve a importar. Con el corpus real dentro, el
/// 2026-09-12, **973 ediciones de 152.080** seguían mostrando en la rejilla el
/// nombre de su archivo con los guiones bajos puestos.
///
/// # Por qué solo las que coinciden con el nombre del archivo
///
/// Porque la procedencia no sirve para elegir: `Source::File` significa "de los
/// metadatos internos del documento", y el título provisional que sale del nombre
/// del archivo **se registra con esa misma procedencia**, así que las dos cosas
/// son indistinguibles en la base. Lo que sí distingue es comparar: si el título
/// es exactamente el nombre del archivo sin extensión, es el provisional y nadie
/// lo ha corregido. Un título que viene de los metadatos y además lleva un guion
/// bajo no se toca, y por eso se compara en lugar de reemplazar a ciegas.
///
/// Un fallo aquí no impide arrancar: se queda el título de antes.
pub fn normalizar_titulos_de_archivo(conn: &Connection) -> u32 {
    const MARCA: &str = "filename_titles_normalized_v1";

    let hecho: bool = conn
        .query_row(
            "SELECT count(*) FROM app_metadata WHERE key = ?1",
            [MARCA],
            |fila| fila.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if hecho {
        return 0;
    }

    let candidatas: Vec<(String, String, String)> = {
        let Ok(mut stmt) = conn.prepare(
            r"SELECT e.id, e.title, f.relative_path
                FROM editions e
                JOIN files f ON f.edition_id = e.id
               WHERE e.title LIKE '%\_%' ESCAPE '\'
               GROUP BY e.id",
        ) else {
            return 0;
        };
        let filas = stmt.query_map([], |fila| {
            Ok((
                fila.get::<_, String>(0)?,
                fila.get::<_, String>(1)?,
                fila.get::<_, String>(2)?,
            ))
        });
        match filas {
            Ok(filas) => filas.filter_map(Result::ok).collect(),
            Err(_) => return 0,
        }
    };

    let cambios: Vec<(String, String)> = candidatas
        .into_iter()
        .filter_map(|(id, titulo, ruta)| {
            let nombre = std::path::Path::new(&ruta)
                .file_stem()
                .and_then(|s| s.to_str())?;
            if nombre != titulo {
                return None;
            }
            let nuevo = crate::library::titulo_normalizado(&titulo);
            (nuevo != titulo).then_some((id, nuevo))
        })
        .collect();

    if cambios.is_empty() {
        let _ = marcar(conn, MARCA);
        return 0;
    }

    /*
     * **Fila a fila y sin transacción propia, y esto se aprendió midiendo.**
     *
     * La primera versión metía los 973 cambios en una transacción inmediata. Con
     * el indexador trabajando **no consiguió el bloqueo ni una vez en diez
     * minutos**: lo sostiene casi todo el tiempo, así que una transacción larga no
     * encuentra hueco. Cada `UPDATE` suelto, en cambio, se cuela entre dos commits
     * suyos respetando `busy_timeout`, que es justo lo que hacen las otras dos
     * pasadas de este módulo.
     *
     * La marca se pone solo si **todos** entraron. Si alguno se quedó fuera por
     * contención, el siguiente arranque lo reintenta en lugar de dar por hecho lo
     * que no se hizo, y mientras tanto se dice en el registro.
     */
    let mut hechos = 0_u32;
    let mut fallidos = 0_u32;
    for (id, titulo) in &cambios {
        let actualizado = conn
            .execute(
                "UPDATE editions SET title = ?1 WHERE id = ?2",
                rusqlite::params![titulo, id],
            )
            .and_then(|filas| {
                // La procedencia guarda el valor que se registró: con el nombre
                // viejo, el historial diría que el título es otro del que se ve.
                conn.execute(
                    "UPDATE metadata_provenance SET value = ?1
                      WHERE entity_id = ?2 AND field = 'title'",
                    rusqlite::params![titulo, id],
                )
                .map(|_| filas)
            });
        match actualizado {
            Ok(filas) => hechos += u32::try_from(filas).unwrap_or(0),
            Err(error) if base_ocupada(&error) => {
                /*
                 * **Se rinde entera al primer bloqueo, y esto se aprendió en vivo.**
                 *
                 * La primera versión seguía con la fila siguiente. Cada intento
                 * espera el `busy_timeout` -cinco segundos- y escribe su aviso: con
                 * otro escritor delante, el 2026-09-12 salieron **dieciocho avisos
                 * idénticos en noventa segundos**, y con las novecientas filas de esa
                 * pasada habrían sido más de una hora de lo mismo. Eso no es
                 * información: es ruido que tapa el registro justo cuando hay algo
                 * que leer en él.
                 *
                 * Si la base está ocupada ahora, lo estará para la fila siguiente.
                 * Se para, se dice una vez cuántas quedan, y el próximo arranque lo
                 * reintenta: no se pone la marca.
                 */
                fallidos = u32::try_from(cambios.len()).unwrap_or(u32::MAX) - hechos;
                tracing::warn!(
                    presentados = hechos,
                    pendientes = fallidos,
                    "la base está ocupada: los títulos que quedan se presentan en el próximo arranque"
                );
                break;
            }
            Err(error) => {
                fallidos += 1;
                tracing::warn!(error = %error, "no se pudo presentar un título");
            }
        }
    }

    if fallidos == 0 {
        let _ = marcar(conn, MARCA);
    } else {
        tracing::warn!(
            fallidos,
            "quedaron títulos por presentar; se reintenta en el próximo arranque"
        );
    }
    tracing::info!(
        presentados = hechos,
        "títulos de archivo presentados como títulos"
    );
    hechos
}

/// Si el error es "la base está ocupada" y no un fallo de verdad.
///
/// Se distingue porque las consecuencias son opuestas: un bloqueo se reintenta
/// más tarde y no dice nada del dato, mientras que cualquier otro error es un
/// problema de esa fila y hay que verlo.
fn base_ocupada(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(fallo, _)
            if matches!(
                fallo.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

fn marcar(conn: &Connection, clave: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, 'true')",
        [clave],
    )
}

fn extraer_portada(
    documento: &Path,
    interna: &str,
    edition_id: &str,
    covers_dir: &Path,
) -> NbResult<(String, PathBuf)> {
    let mut contenedor = crate::zip::Archive::open(documento)?;
    let bytes = contenedor.read(interna)?;

    let extension = Path::new(interna)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("bin");

    // Solo la miniatura: la original es una copia de lo que ya lleva el libro, y
    // pesaba el 70 % de lo derivado en la instalación real. Ver `covers`.
    let nombre = format!("{edition_id}.{extension}");
    let guardada = crate::covers::guardar(covers_dir, &nombre, &bytes)?;
    // Lo que se apunta en el catálogo es el nombre lógico, del que cuelga la
    // miniatura; quien lo lee lo resuelve con `covers::para_rejilla`. Lo que se
    // devuelve, en cambio, es lo que de verdad quedó en el disco.
    Ok((nombre, guardada))
}

/// Conecta el servicio de metadatos al bus.
///
/// Es lo que hace que la extracción ocurra **en segundo plano y disparada por un
/// evento**, y no dentro de la importación: el bus despacha en su propio hilo,
/// así que quien importa no espera a que se lea el documento ni a que se escriba
/// la portada.
///
/// Y por eso mismo un fallo aquí no puede impedir que el documento exista: para
/// cuando esto corre, la transacción de la importación ya está confirmada.
pub fn subscribe(bus: &EventBus, db: Database, covers_dir: PathBuf) {
    let anunciar = bus.clone();
    bus.subscribe("metadata-service", move |evento| {
        let DomainEvent::DocumentImported {
            edition_id,
            file_id,
            media_type,
        } = evento
        else {
            return;
        };

        let Ok(conn) = db.connection() else {
            tracing::warn!("sin conexión para enriquecer metadatos");
            return;
        };

        let ruta: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT {} FROM files f WHERE f.id = ?1",
                    crate::db::catalog::SQL_RUTA_ABSOLUTA
                ),
                [file_id],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten();

        let Some(ruta) = ruta else { return };

        match enrich(&conn, edition_id, Path::new(&ruta), media_type, &covers_dir) {
            Ok(informe) => {
                tracing::info!(
                    aplicados = informe.applied.len(),
                    propuestos = informe.suggested.len(),
                    contribuidores = informe.contributors,
                    portada = informe.cover.is_some(),
                    "metadatos internos incorporados"
                );
                // El documento entró con el nombre de su archivo y ahora tiene
                // título, autoría y portada: quien lo esté pintando tiene que
                // enterarse sin que nadie recargue.
                if !informe.applied.is_empty()
                    || informe.contributors > 0
                    || informe.cover.is_some()
                {
                    anunciar.emit(DomainEvent::MetadataUpdated {
                        edition_id: edition_id.clone(),
                    });
                }
            }
            // Un documento cuyos metadatos no se dejan leer sigue siendo un
            // documento: entró con el nombre de su archivo y ahí se queda.
            Err(error) => tracing::info!(
                codigo = error.code(),
                "no se pudieron extraer los metadatos internos; el documento no se ve afectado"
            ),
        }
    });
}

/// Lo que una pasada de fondo espera después de escribir, para que lo que hace
/// el usuario -guardar una posición, una nota, una preferencia- entre sin cola.
pub const PAUSA_ENTRE_ESCRITURAS_DE_FONDO: std::time::Duration =
    std::time::Duration::from_millis(15);

/// Cede el paso después de una escritura de fondo que tardó `tardo`.
///
/// La pausa es al menos tan larga como la escritura. Cambiar la serie de una
/// edición rehace su fila del índice del catálogo, y sobre la biblioteca real eso
/// son unos 300 ms con el cerrojo cogido: con una pausa fija de 15 ms detrás, el
/// cerrojo seguía ocupado el 95 % del tiempo. Así nunca pasa de la mitad.
pub fn ceder_el_paso(tardo: std::time::Duration) {
    std::thread::sleep(tardo.max(PAUSA_ENTRE_ESCRITURAS_DE_FONDO));
}

/// Cuántas ediciones mira cada tanda de [`completar_serie_y_sinopsis`].
pub const TANDA_DE_SERIES: u32 = 400;

/// Donde se quedó la pasada de serie y sinopsis, para retomarla en el siguiente
/// arranque en lugar de empezar de cero.
const MARCA_DE_SERIES: &str = "serie_y_sinopsis_desde";

/// Limpia las entidades que quedaron escritas en series y sinopsis (una vez).
///
/// Antes la serie se guardaba con las entidades del atributo sin
/// resolver y la sinopsis con las de su segunda capa: "Áncora &amp; Delfín",
/// "siglo&#160;XX". El lector de metadatos ya las resuelve; esto arregla lo que
/// entró antes. No va en una migración porque una que actualiza filas obliga a
/// comprobar las claves ajenas de la base entera, que sobre la real son minutos.
///
/// Cota: las filas con una entidad a la vista, unas mil trescientas en la base
/// real.
pub fn limpiar_entidades_guardadas(conn: &mut Connection) {
    const MARCA: &str = "entidades_limpias_v2";
    let hecha: bool = conn
        .query_row("SELECT 1 FROM app_metadata WHERE key = ?1", [MARCA], |_| {
            Ok(true)
        })
        .unwrap_or(false);
    if hecha {
        return;
    }
    let filas: Vec<(String, Option<String>, Option<String>)> = {
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, series_title, description FROM editions
              WHERE series_title LIKE '%&%;%' OR description LIKE '%&%;%'
                 OR description LIKE '%<%>%'",
        ) else {
            return;
        };
        let Ok(leidas) = stmt.query_map([], |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?))) else {
            return;
        };
        let Ok(filas) = leidas.collect::<rusqlite::Result<Vec<_>>>() else {
            return;
        };
        filas
    };
    let mut limpias = 0_u32;
    for (id, serie, sinopsis) in filas {
        let antes = std::time::Instant::now();
        match limpiar_entidad_guardada(conn, &id, serie.as_deref(), sinopsis.as_deref()) {
            Ok(true) => {
                limpias += 1;
                ceder_el_paso(antes.elapsed());
            }
            Ok(false) => {}
            Err(_) => {
                // Sin marca de completado: una escritura fallida se reintenta al arrancar.
                tracing::warn!("no se pudo completar la limpieza de entidades guardadas");
                return;
            }
        }
    }
    if conn
        .execute(
            "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, '1')",
            [MARCA],
        )
        .is_ok()
    {
        tracing::info!(
            limpias,
            "entidades resueltas en series y sinopsis guardadas"
        );
    }
}

fn limpiar_entidad_guardada(
    conn: &mut Connection,
    id: &str,
    serie: Option<&str>,
    sinopsis: Option<&str>,
) -> NbResult<bool> {
    let valores = [
        (
            Field::SeriesTitle,
            serie,
            serie.map(crate::epub::sin_entidades),
        ),
        (
            Field::Description,
            sinopsis,
            sinopsis.map(crate::epub::sinopsis_limpia),
        ),
    ];
    if valores
        .iter()
        .all(|(_, anterior, nuevo)| *anterior == nuevo.as_deref())
    {
        return Ok(false);
    }
    let tx = crate::db::write_transaction(conn)?;
    let mut cambiada = false;
    for (campo, anterior, nuevo) in valores {
        if anterior == nuevo.as_deref() {
            continue;
        }
        // Un origen de mayor precedencia gana, aunque contenga entidades a propósito.
        // La comparación protege también cambios posteriores a la lectura inicial.
        let sql = format!(
            "UPDATE editions SET {0} = ?1 WHERE id = ?2 AND {0} IS ?3
               AND NOT EXISTS (SELECT 1 FROM metadata_provenance
                 WHERE entity_type = 'edition' AND entity_id = ?2
                   AND field = ?4 AND source != 'file')",
            campo.as_str()
        );
        let escritas = tx
            .execute(&sql, rusqlite::params![nuevo, id, anterior, campo.as_str()])
            .map_err(|e| fallo_de_almacenamiento(&e))?;
        if escritas > 0 {
            tx.execute(
                "UPDATE metadata_provenance SET value = ?1
                   WHERE entity_type = 'edition' AND entity_id = ?2
                     AND field = ?3 AND source = 'file' AND value IS ?4",
                rusqlite::params![nuevo, id, campo.as_str(), anterior],
            )
            .map_err(|e| fallo_de_almacenamiento(&e))?;
            cambiada = true;
        }
    }
    tx.commit().map_err(|e| fallo_de_almacenamiento(&e))?;
    Ok(cambiada)
}

/// Rellena la serie y la sinopsis de lo ya importado, una tanda cada vez.
///
/// # Por qué hace falta
///
/// Antes no se leían: ninguna de las 152.080 ediciones de
/// la instalación real tenía serie, y ninguna sinopsis. Lo que entra a partir de
/// ahora las trae con [`enrich`]; esto es para lo que ya estaba. Solo lee el
/// paquete de cada EPUB, que son unos kilobytes, y no toca nada que el usuario
/// haya escrito a mano: pasa por [`propose`] como cualquier otro origen.
///
/// Devuelve cuántas miró. Cero es que terminó.
pub fn completar_serie_y_sinopsis(conn: &Connection) -> u32 {
    let desde: i64 = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM app_metadata WHERE key = ?1",
            [MARCA_DE_SERIES],
            |f| f.get(0),
        )
        .unwrap_or(0);
    let pendientes: Vec<(i64, String, String)> = {
        let Ok(mut stmt) = conn.prepare(&format!(
            "SELECT e.rowid, e.id, {ruta}
               FROM editions e
               JOIN files f ON f.edition_id = e.id
              WHERE e.rowid > ?1
                AND f.media_type = 'application/epub+zip'
                AND f.availability = 'available'
              GROUP BY e.id
              ORDER BY e.rowid
              LIMIT {TANDA_DE_SERIES}",
            ruta = crate::db::catalog::SQL_RUTA_ABSOLUTA
        )) else {
            return 0;
        };
        let filas = stmt.query_map([desde], |f| {
            Ok((
                f.get::<_, i64>(0)?,
                f.get::<_, String>(1)?,
                f.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        });
        match filas {
            Ok(filas) => filas.filter_map(Result::ok).collect(),
            Err(_) => return 0,
        }
    };
    let Some(ultima) = pendientes.last().map(|(rowid, _, _)| *rowid) else {
        return 0;
    };
    for (_, edition_id, ruta) in &pendientes {
        let Ok(interno) = crate::epub::read_metadata(Path::new(ruta)) else {
            continue;
        };
        let mut escribio = false;
        let antes = std::time::Instant::now();
        for (campo, valor) in [
            (Field::SeriesTitle, interno.series.as_deref()),
            (Field::SeriesIndex, interno.series_index.as_deref()),
            (Field::Description, interno.description.as_deref()),
        ] {
            if let Some(valor) = valor {
                let _ = propose(conn, edition_id, campo, Source::File, Some(valor));
                escribio = true;
            }
        }
        /*
         * Tras escribir, se suelta el paso. Sin esta pausa la pasada encadena
         * escrituras sin hueco durante minutos, y SQLite no reparte el cerrojo
         * por turnos: quien espera vuelve a mirar cada tantos milisegundos y casi
         * nunca acierta el instante libre. Medido el 2026-11-13 en la aplicación
         * instalada, guardar una preferencia esperó los cinco segundos enteros y
         * falló con "database is locked" mientras esto corría -y lo mismo le
         * habría pasado a guardar la posición de lectura o una nota-. Con la
         * pausa, el cerrojo está libre casi todo el tiempo.
         */
        if escribio {
            ceder_el_paso(antes.elapsed());
        }
    }
    let _ = conn.execute(
        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, ?2)",
        rusqlite::params![MARCA_DE_SERIES, ultima.to_string()],
    );
    u32::try_from(pendientes.len()).unwrap_or(u32::MAX)
}
