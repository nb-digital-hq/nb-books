//! Exportación e importación completas: la garantía material de la posesión.
//!
//! "El usuario es dueño de sus datos" es una frase hasta que existe un botón que
//! los saca enteros en un formato que se puede leer sin NB Books. Este módulo es
//! ese botón, y su prueba de aceptación es la que de verdad lo demuestra:
//! exportar, borrar la base, importar y comparar campo a campo.
//!
//! Contratos: `docs/architecture/versioning.md` y
//! `docs/architecture/metadata-model.md`, sección 6.
//!
//! # Qué lleva el archivo y qué no
//!
//! Lleva **la identidad de los documentos, no sus bytes**: ruta, hash de
//! contenido, tamaño y tipo. Una biblioteca real ocupa cientos de gigabytes y un
//! archivo de exportación que los incluyera no sería un archivo, sería otra
//! copia de la biblioteca. Al importar en otra máquina, los documentos se
//! reconocen por su hash cuando están presentes, que es exactamente para lo que
//! sirve la identidad por contenido.
//!
//! # Restaurar, no fusionar
//!
//! La importación restaura sobre una base vacía y **se niega a operar sobre una
//! que ya tiene catálogo**. Fusionar dos bibliotecas es otro problema: exige
//! decidir qué gana cuando ambas versiones de una anotación difieren, y esa
//! decisión es [ADR-006](../../docs/adr/decisions/ADR-006-conflict-resolution.md),
//! que sigue en estado propuesto. Construir una fusión sobre una decisión no
//! tomada sería la forma más cara de tomarla.

use std::io::BufWriter;
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::catalog::{
    Annotation, ContentHash, Contributor, Edition, File, LibraryRoot, RootKind, StorageMode,
    Subject, Work,
};
use crate::error::{NbError, NbResult};
use crate::metadata::{Provenance, Suggestion};
use crate::notes::Note;
use crate::settings::{self, Setting};

/// Nombre del formato. Va dentro del archivo para que sea autodescriptivo: quien
/// se lo encuentre suelto dentro de cinco años tiene que poder saber qué es.
pub const FORMATO: &str = "nb-books-export";

/// Versión del formato de intercambio.
///
/// Sigue SemVer con la compatibilidad hacia atrás como prioridad
/// (`docs/architecture/versioning.md`). Aquí solo viaja el mayor:
/// los cambios compatibles añaden campos opcionales y no lo mueven.
pub const VERSION: u32 = 1;

/// Un archivo de exportación completo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    /// Siempre [`FORMATO`]. Un archivo sin esto no es una exportación nuestra.
    pub format: String,
    pub format_version: u32,
    pub exported_at: String,
    pub application: Application,

    // --- Catálogo -----------------------------------------------------------
    pub works: Vec<Work>,
    pub editions: Vec<Edition>,
    pub edition_identifiers: Vec<EditionIdentifier>,
    /// Raíces de biblioteca. Viajan **sin su ruta**: ver `exportar_raices`.
    #[serde(default)]
    pub library_roots: Vec<LibraryRoot>,
    pub files: Vec<File>,
    pub contributors: Vec<Contributor>,
    pub edition_contributors: Vec<EditionContributor>,
    pub subjects: Vec<Subject>,
    pub edition_subjects: Vec<EditionSubject>,
    pub annotations: Vec<Annotation>,

    // --- Metadatos ----------------------------------------------------------
    pub metadata_provenance: Vec<Provenance>,
    pub metadata_suggestions: Vec<Suggestion>,

    // --- Preferencias -------------------------------------------------------
    pub settings: Vec<Setting>,

    // --- Agrupaciones del usuario -------------------------------------------
    #[serde(default)]
    pub collections: Vec<CollectionRow>,
    /// Miembros de las colecciones manuales.
    ///
    /// Las colecciones por regla no tienen miembros que exportar: guardan el
    /// criterio, no la lista, y al importarlas se vuelven a evaluar solas.
    #[serde(default)]
    pub collection_items: Vec<CollectionItemRow>,

    // --- Notas propias -------------------------------------------------------
    /// Notas del usuario, que no cuelgan de ningún documento.
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Enlaces entre notas. **Los retroenlaces no se exportan**: son estos
    /// mismos leídos del otro lado, y exportarlos aparte permitiría que las dos
    /// direcciones dejaran de coincidir al restaurar.
    #[serde(default)]
    pub note_links: Vec<NoteLinkRow>,
    /// Enlaces de una nota a una anotación anclada.
    #[serde(default)]
    pub note_annotation_links: Vec<NoteAnnotationLinkRow>,

    // --- Lectura -------------------------------------------------------------
    #[serde(default)]
    pub progress: Vec<ProgressRow>,
    /// Sesiones de lectura registradas.
    ///
    /// Son del usuario como todo lo demás: cuánto y cuándo leyó es un dato suyo,
    /// y dejarlo fuera de la exportación lo convertiría en un dato que solo
    /// existe mientras use esta aplicación.
    #[serde(default)]
    pub reading_sessions: Vec<ReadingSessionRow>,

    // --- Comprensión ---------------------------------------------------------
    /// Palabras consultadas en el diccionario, con dónde aparecieron.
    ///
    /// Entran por el mismo motivo que las sesiones: es trabajo de lectura del
    /// usuario, y dejarlo fuera lo convertiría en un dato que solo existe
    /// mientras use esta instalación. Es además el criterio de aceptación de
    /// la exportación completa.
    #[serde(default)]
    pub vocabulary: Vec<VocabularyRow>,

    // --- Estudio con IA ---------------------------------------
    /// Tarjetas y preguntas **que alguien decidió guardar**, con su procedencia.
    ///
    /// Entran aunque las generara una máquina: guardarlas, editarlas y repasarlas es
    /// trabajo del usuario, y la marca de generado viaja con ellas.
    #[serde(default)]
    pub study_cards: Vec<StudyCardRow>,
    /// Conceptos guardados, con su procedencia.
    #[serde(default)]
    pub concepts: Vec<ConceptRow>,
    /// Lo que salió hacia un proveedor de IA externo (V32), byte a byte.
    ///
    /// `ai/privacy.md` sección 3 lo quiere local, **exportable** y borrable, y hasta
    /// antes solo era las otras dos cosas.
    #[serde(default)]
    pub ai_sends: Vec<AiSendRow>,
}

/// Un envío a un proveedor de IA externo, tal y como viaja.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSendRow {
    pub id: i64,
    pub sent_at: String,
    pub protocol: String,
    pub host: String,
    pub model: String,
    pub capability: String,
    pub body: String,
    pub bytes: i64,
    pub status: Option<i64>,
}

/// Una tarjeta o una pregunta guardada, tal y como viaja: las columnas, sin
/// interpretar, por lo mismo que `VocabularyRow`. Incluye el estado de repaso
/// espaciado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyCardRow {
    pub id: String,
    pub edition_id: String,
    pub kind: String,
    pub front: String,
    pub back: String,
    pub choices: Option<String>,
    pub sources: String,
    pub provider: String,
    pub template: String,
    pub edited_at: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub interval_days: u32,
    #[serde(default = "facilidad_por_defecto")]
    pub ease_factor: f64,
    #[serde(default)]
    pub repetitions: u32,
    #[serde(default)]
    pub last_reviewed_at: Option<String>,
}

fn facilidad_por_defecto() -> f64 {
    2.5
}

/// Un concepto guardado, tal y como viaja.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConceptRow {
    pub id: String,
    pub edition_id: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub sources: String,
    pub provider: String,
    pub template: String,
    pub edited_at: Option<String>,
    pub created_at: String,
}

/// Una colección, tal y como viaja.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub origin: String,
    pub rule: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Un enlace entre dos notas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteLinkRow {
    pub source_id: String,
    pub target_id: String,
    pub created_at: String,
}

/// Un enlace de una nota a una anotación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteAnnotationLinkRow {
    pub note_id: String,
    pub annotation_id: String,
    pub created_at: String,
}

/// Un miembro de una colección manual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionItemRow {
    pub collection_id: String,
    pub edition_id: String,
    pub position: i64,
    pub added_at: String,
}

/// El progreso de una edición.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressRow {
    pub id: String,
    pub edition_id: String,
    pub profile: String,
    /// El Locator, tal cual. Viaja como texto JSON y no interpretado porque la
    /// exportación tiene que poder llevarse un locator que esta versión no
    /// entienda: rechazarlo aquí perdería la posición del usuario para siempre.
    pub locator: String,
    pub precision: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Una sesión de lectura registrada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingSessionRow {
    pub id: String,
    pub edition_id: String,
    pub profile: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub start_locator: String,
    pub end_locator: Option<String>,
}

/// Una palabra consultada, tal y como viaja.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VocabularyRow {
    pub id: String,
    pub headword: String,
    pub term: String,
    pub edition_id: String,
    pub provider_id: String,
    /// El Locator, tal cual. Como en `ProgressRow`, viaja como texto y sin
    /// interpretar: la exportación tiene que poder llevarse un locator que esta
    /// versión no entienda en lugar de perderlo.
    pub locator: String,
    pub created_at: String,
}

/// Qué versión de qué aplicación escribió el archivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub name: String,
    pub version: String,
    /// Versión del esquema de base de datos de origen. No se usa para decidir
    /// nada al importar; está para poder diagnosticar un archivo raro sin
    /// adivinar de dónde salió.
    pub schema_version: u32,
}

/// Identificador externo de una edición, tal y como viaja en el archivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionIdentifier {
    pub edition_id: String,
    pub scheme: String,
    pub value: String,
}

/// Relación entre una edición y quien contribuyó a ella.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionContributor {
    pub edition_id: String,
    pub contributor_id: String,
    pub role: String,
    pub position: i64,
}

/// Relación entre una edición y una materia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditionSubject {
    pub edition_id: String,
    pub subject_id: String,
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(clase = %error, "consulta de exportación fallida");
    NbError::StorageFailure
}

/// Recorre una consulta y construye el vector correspondiente.
fn recolectar<T, F>(conn: &Connection, sql: &str, mapear: F) -> NbResult<Vec<T>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| fallo(&e))?;
    let filas = stmt.query_map([], mapear).map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Vuelca la biblioteca entera a una estructura exportable.
///
/// El orden de cada sección es estable —por identificador— para que dos
/// exportaciones del mismo estado produzcan el mismo archivo. Un archivo que
/// cambia sin que cambien los datos no se puede versionar ni comparar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn export(conn: &Connection) -> NbResult<Export> {
    let works = recolectar(
        conn,
        "SELECT id, canonical_title, original_language, created_at, updated_at
           FROM works ORDER BY id",
        |row| {
            Ok(Work {
                id: row.get(0)?,
                canonical_title: row.get(1)?,
                original_language: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )?;

    let editions = recolectar(
        conn,
        "SELECT id, work_id, title, subtitle, publisher, published_date, edition_statement,
                language, series_title, series_index, created_at, updated_at
           FROM editions ORDER BY id",
        |row| {
            Ok(Edition {
                id: row.get(0)?,
                work_id: row.get(1)?,
                title: row.get(2)?,
                subtitle: row.get(3)?,
                publisher: row.get(4)?,
                published_date: row.get(5)?,
                edition_statement: row.get(6)?,
                language: row.get(7)?,
                series_title: row.get(8)?,
                series_index: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        },
    )?;

    let edition_identifiers = recolectar(
        conn,
        "SELECT edition_id, scheme, value FROM edition_identifiers
          ORDER BY edition_id, scheme, value",
        |row| {
            Ok(EditionIdentifier {
                edition_id: row.get(0)?,
                scheme: row.get(1)?,
                value: row.get(2)?,
            })
        },
    )?;

    // Las raíces viajan **sin ruta**, siempre. Una ruta es la dirección de un
    // equipo concreto: llevarla al equipo de destino es exactamente el defecto
    // que la restauración corrige. Al restaurar, toda raíz llega no localizada y el
    // usuario la apunta una vez.
    let library_roots = recolectar(
        conn,
        "SELECT id, name, kind, created_at, updated_at FROM library_roots ORDER BY id",
        |row| {
            let kind: String = row.get(2)?;
            Ok(LibraryRoot {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: serde_json::from_value(serde_json::Value::String(kind)).map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        "valor fuera de la lista admitida en kind".into(),
                    )
                })?,
                path: None,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )?;

    let files = recolectar(
        conn,
        "SELECT id, edition_id, root_id, relative_path, content_hash, byte_size, media_type,
                storage_mode, availability, created_at, updated_at
           FROM files ORDER BY id",
        |row| {
            let hash: String = row.get(4)?;
            let modo: String = row.get(7)?;
            let disponibilidad: String = row.get(8)?;
            let invalido = |c: &'static str| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    format!("valor fuera de la lista admitida en {c}").into(),
                )
            };

            Ok(File {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                root_id: row.get(2)?,
                relative_path: row.get(3)?,
                content_hash: ContentHash::parse(&hash).map_err(|_| invalido("content_hash"))?,
                byte_size: row.get(5)?,
                media_type: row.get(6)?,
                storage_mode: serde_json::from_value(serde_json::Value::String(modo))
                    .map_err(|_| invalido("storage_mode"))?,
                availability: serde_json::from_value(serde_json::Value::String(disponibilidad))
                    .map_err(|_| invalido("availability"))?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        },
    )?;

    let contributors = recolectar(
        conn,
        "SELECT id, name, sort_name, orcid, created_at, updated_at
           FROM contributors ORDER BY id",
        |row| {
            Ok(Contributor {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_name: row.get(2)?,
                orcid: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    )?;

    let edition_contributors = recolectar(
        conn,
        "SELECT edition_id, contributor_id, role, position FROM edition_contributors
          ORDER BY edition_id, contributor_id, role",
        |row| {
            Ok(EditionContributor {
                edition_id: row.get(0)?,
                contributor_id: row.get(1)?,
                role: row.get(2)?,
                position: row.get(3)?,
            })
        },
    )?;

    let subjects = recolectar(
        conn,
        "SELECT id, scheme, label FROM subjects ORDER BY id",
        |row| {
            Ok(Subject {
                id: row.get(0)?,
                scheme: row.get(1)?,
                label: row.get(2)?,
            })
        },
    )?;

    let edition_subjects = recolectar(
        conn,
        "SELECT edition_id, subject_id FROM edition_subjects ORDER BY edition_id, subject_id",
        |row| {
            Ok(EditionSubject {
                edition_id: row.get(0)?,
                subject_id: row.get(1)?,
            })
        },
    )?;

    let collections = recolectar(
        conn,
        "SELECT id, name, kind, origin, rule, created_at, updated_at
           FROM collections ORDER BY id",
        |row| {
            Ok(CollectionRow {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                origin: row.get(3)?,
                rule: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )?;

    let collection_items = recolectar(
        conn,
        "SELECT collection_id, edition_id, position, added_at
           FROM collection_items ORDER BY collection_id, position, edition_id",
        |row| {
            Ok(CollectionItemRow {
                collection_id: row.get(0)?,
                edition_id: row.get(1)?,
                position: row.get(2)?,
                added_at: row.get(3)?,
            })
        },
    )?;

    let progress = recolectar(
        conn,
        "SELECT id, edition_id, profile, locator, precision, created_at, updated_at
           FROM progress ORDER BY id",
        |row| {
            Ok(ProgressRow {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                profile: row.get(2)?,
                locator: row.get(3)?,
                precision: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )?;

    let reading_sessions = recolectar(
        conn,
        "SELECT id, edition_id, profile, started_at, ended_at, start_locator, end_locator
           FROM reading_sessions ORDER BY started_at, id",
        |row| {
            Ok(ReadingSessionRow {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                profile: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                start_locator: row.get(5)?,
                end_locator: row.get(6)?,
            })
        },
    )?;

    let annotations = recolectar(
        conn,
        // `payload` va al final a propósito: los campos se leen por posición, y
        // meterlo en medio habría desplazado en silencio a todos los de detrás.
        "SELECT id, edition_id, kind, body, target, created_at, updated_at,
                deleted_at, supersedes, payload
           FROM annotations ORDER BY id",
        |row| {
            let kind: String = row.get(2)?;
            Ok(Annotation {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                kind: serde_json::from_value(serde_json::Value::String(kind)).map_err(|_| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        "clase de anotación fuera de la lista admitida".into(),
                    )
                })?,
                body: row.get(3)?,
                target: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                deleted_at: row.get(7)?,
                supersedes: row.get(8)?,
                payload: row.get(9)?,
            })
        },
    )?;

    let metadata_provenance = recolectar(
        conn,
        "SELECT entity_type, entity_id, field, source, value, recorded_at
           FROM metadata_provenance ORDER BY entity_type, entity_id, field, source",
        |row| {
            let json = serde_json::json!({
                "entityType": row.get::<_, String>(0)?,
                "entityId": row.get::<_, String>(1)?,
                "field": row.get::<_, String>(2)?,
                "source": row.get::<_, String>(3)?,
                "value": row.get::<_, Option<String>>(4)?,
                "recordedAt": row.get::<_, String>(5)?,
            });
            serde_json::from_value(json).map_err(|_| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    "procedencia con valores fuera de la lista admitida".into(),
                )
            })
        },
    )?;

    let metadata_suggestions = recolectar(
        conn,
        "SELECT id, entity_id, field, source, proposed_value, status, created_at
           FROM metadata_suggestions ORDER BY id",
        |row| {
            let json = serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "entityId": row.get::<_, String>(1)?,
                "field": row.get::<_, String>(2)?,
                "source": row.get::<_, String>(3)?,
                "proposedValue": row.get::<_, Option<String>>(4)?,
                "status": row.get::<_, String>(5)?,
                "createdAt": row.get::<_, String>(6)?,
            });
            serde_json::from_value(json).map_err(|_| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    "propuesta con valores fuera de la lista admitida".into(),
                )
            })
        },
    )?;

    let notes = recolectar(
        conn,
        "SELECT id, title, body, created_at, updated_at FROM notes ORDER BY id",
        |row| {
            Ok(Note {
                id: row.get(0)?,
                title: row.get(1)?,
                body: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )?;

    let note_links = recolectar(
        conn,
        "SELECT source_id, target_id, created_at FROM note_links
          ORDER BY source_id, target_id",
        |row| {
            Ok(NoteLinkRow {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                created_at: row.get(2)?,
            })
        },
    )?;

    let note_annotation_links = recolectar(
        conn,
        "SELECT note_id, annotation_id, created_at FROM note_annotation_links
          ORDER BY note_id, annotation_id",
        |row| {
            Ok(NoteAnnotationLinkRow {
                note_id: row.get(0)?,
                annotation_id: row.get(1)?,
                created_at: row.get(2)?,
            })
        },
    )?;

    let vocabulary = recolectar(
        conn,
        "SELECT id, headword, term, edition_id, provider_id, locator, created_at
           FROM vocabulary ORDER BY id",
        |row| {
            Ok(VocabularyRow {
                id: row.get(0)?,
                headword: row.get(1)?,
                term: row.get(2)?,
                edition_id: row.get(3)?,
                provider_id: row.get(4)?,
                locator: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )?;

    let study_cards = recolectar(
        conn,
        "SELECT id, edition_id, kind, front, back, choices, sources, provider, template,
                edited_at, created_at, due_at, interval_days, ease_factor, repetitions,
                last_reviewed_at
           FROM study_cards ORDER BY id",
        |row| {
            Ok(StudyCardRow {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                kind: row.get(2)?,
                front: row.get(3)?,
                back: row.get(4)?,
                choices: row.get(5)?,
                sources: row.get(6)?,
                provider: row.get(7)?,
                template: row.get(8)?,
                edited_at: row.get(9)?,
                created_at: row.get(10)?,
                due_at: row.get(11)?,
                interval_days: row.get(12)?,
                ease_factor: row.get(13)?,
                repetitions: row.get(14)?,
                last_reviewed_at: row.get(15)?,
            })
        },
    )?;

    let concepts = recolectar(
        conn,
        "SELECT id, edition_id, name, kind, description, sources, provider, template,
                edited_at, created_at
           FROM concepts ORDER BY id",
        |row| {
            Ok(ConceptRow {
                id: row.get(0)?,
                edition_id: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                description: row.get(4)?,
                sources: row.get(5)?,
                provider: row.get(6)?,
                template: row.get(7)?,
                edited_at: row.get(8)?,
                created_at: row.get(9)?,
            })
        },
    )?;

    let ai_sends = recolectar(
        conn,
        "SELECT id, sent_at, protocol, host, model, capability, body, bytes, status
           FROM ai_sends ORDER BY id",
        |row| {
            Ok(AiSendRow {
                id: row.get(0)?,
                sent_at: row.get(1)?,
                protocol: row.get(2)?,
                host: row.get(3)?,
                model: row.get(4)?,
                capability: row.get(5)?,
                body: row.get(6)?,
                bytes: row.get(7)?,
                status: row.get(8)?,
            })
        },
    )?;

    Ok(Export {
        format: FORMATO.to_owned(),
        format_version: VERSION,
        exported_at: ahora(conn)?,
        application: Application {
            name: "NB Books".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            schema_version: crate::db::migrations::current_version(conn)?,
        },
        library_roots,
        notes,
        note_links,
        note_annotation_links,
        works,
        editions,
        edition_identifiers,
        files,
        contributors,
        edition_contributors,
        subjects,
        edition_subjects,
        annotations,
        metadata_provenance,
        metadata_suggestions,
        // Si este equipo escucha a los teléfonos no viaja: es de este equipo, y
        // restaurarlo en otro abriría un puerto sin ningún teléfono emparejado.
        settings: settings::snapshot(conn)?
            .into_iter()
            .filter(|s| s.key != settings::SettingKey::MobileServer.as_str())
            .collect(),
        collections,
        collection_items,
        progress,
        reading_sessions,
        vocabulary,
        study_cards,
        concepts,
        ai_sends,
    })
}

fn ahora(conn: &Connection) -> NbResult<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
        row.get(0)
    })
    .map_err(|e| fallo(&e))
}

/// Serializa una exportación con sangría, para que sea legible sin herramientas.
///
/// # Errors
///
/// [`NbError::Internal`] si la serialización falla.
pub fn to_json(export: &Export) -> NbResult<String> {
    serde_json::to_string_pretty(export).map_err(|error| {
        tracing::error!(error = %error, "no se pudo serializar la exportación");
        NbError::Internal
    })
}

/// Escribe una exportación en un archivo, sin dejarlo nunca a medias.
///
/// Se serializa **directamente al disco** y no a una cadena que después se
/// escribe: sobre ciento cincuenta mil documentos esa cadena eran 177 MB en
/// memoria, además de las estructuras que la produjeron
/// (`performance-budgets.md`, sección 4.8). [`to_json`] sigue para quien
/// necesite el texto.
///
/// Y se escribe al lado, con otro nombre, y se renombra al terminar. Una
/// exportación que falla a mitad —el disco lleno, un disco externo que se
/// desconecta— no puede dejar un archivo truncado **encima de la anterior**: la
/// copia buena se perdería justo al intentar hacer otra.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el destino no nombra un archivo.
/// - [`NbError::StorageFailure`] si no se puede escribir o renombrar.
/// - [`NbError::Internal`] si la serialización falla.
pub fn write_json(export: &Export, destino: &Path) -> NbResult<()> {
    let Some(nombre) = destino.file_name() else {
        return Err(NbError::PathNotAllowed);
    };
    let parcial = destino.with_file_name(format!(".{}.parcial", nombre.to_string_lossy()));

    let resultado = escribir_y_renombrar(export, &parcial, destino);
    if resultado.is_err() {
        // Lo que quedó a medias no es de nadie. Si ni siquiera llegó a crearse no
        // hay nada que borrar, y eso no es un fallo más.
        let _ = std::fs::remove_file(&parcial);
    }
    resultado
}

fn escribir_y_renombrar(export: &Export, parcial: &Path, destino: &Path) -> NbResult<()> {
    let disco = |error: std::io::Error| {
        tracing::error!(error = %error, "no se pudo escribir la exportación");
        NbError::StorageFailure
    };

    let mut escritor = BufWriter::new(std::fs::File::create(parcial).map_err(disco)?);
    serde_json::to_writer_pretty(&mut escritor, export).map_err(|error| {
        tracing::error!(error = %error, "no se pudo serializar la exportación");
        if error.is_io() {
            NbError::StorageFailure
        } else {
            NbError::Internal
        }
    })?;
    let archivo = escritor
        .into_inner()
        .map_err(|error| disco(error.into_error()))?;
    // Sin esto, un apagón justo después de renombrar podría dejar el nombre nuevo
    // apuntando a datos que nunca llegaron al disco.
    archivo.sync_all().map_err(disco)?;
    drop(archivo);
    std::fs::rename(parcial, destino).map_err(disco)
}

/// Lee un archivo de exportación.
///
/// # Errors
///
/// - [`NbError::DocumentCorrupted`] si el JSON no tiene la forma esperada o no
///   declara ser una exportación de NB Books.
/// - [`NbError::SchemaMismatch`] si lo escribió una versión posterior del
///   formato: interpretarlo con reglas antiguas restauraría datos incompletos, y
///   una restauración incompleta es peor que ninguna, porque parece haber
///   funcionado.
pub fn from_json(json: &str) -> NbResult<Export> {
    let export: Export = serde_json::from_str(json).map_err(|error| {
        // El mensaje de serde cita el fragmento que falló, y ese fragmento puede
        // ser el título de un libro: no se registra.
        tracing::warn!(
            linea = error.line(),
            columna = error.column(),
            "archivo de exportación con forma inesperada"
        );
        NbError::DocumentCorrupted
    })?;

    if export.format != FORMATO {
        tracing::error!("el archivo no declara ser una exportación de NB Books");
        return Err(NbError::DocumentCorrupted);
    }

    if export.format_version > VERSION {
        tracing::error!(
            version = export.format_version,
            soportada = VERSION,
            "exportación escrita por una versión posterior del formato"
        );
        return Err(NbError::SchemaMismatch);
    }

    Ok(export)
}

/// Restaura una exportación sobre una base **vacía**.
///
/// Todo dentro de una única transacción: una restauración a medias dejaría una
/// biblioteca que parece la del usuario y no lo es, que es peor que un fallo
/// limpio.
///
/// Los identificadores se conservan tal cual venían. Es lo que hace que la
/// restauración sea equivalente y no solo parecida: una anotación que apunta a
/// una edición tiene que seguir apuntando a la misma.
///
/// # Errors
///
/// - [`NbError::SchemaMismatch`] si la base ya tiene catálogo. Fusionar dos
///   bibliotecas exige decidir qué gana ante un conflicto, y esa decisión es
///   ADR-006, todavía propuesto.
/// - [`NbError::StorageFailure`] si alguna escritura falla; en ese caso no se
///   escribe nada.
pub fn import(conn: &mut Connection, export: &Export) -> NbResult<()> {
    let ocupada: i64 = conn
        .query_row(
            // Las notas cuentan: una base con notas y sin catálogo no está
            // vacía, y dejarla pasar acabaría en un choque de títulos a mitad
            // de la restauración en lugar de en un rechazo limpio.
            "SELECT (SELECT count(*) FROM works) + (SELECT count(*) FROM editions)
                  + (SELECT count(*) FROM files) + (SELECT count(*) FROM annotations)
                  + (SELECT count(*) FROM notes)",
            [],
            |row| row.get(0),
        )
        .map_err(|e| fallo(&e))?;

    if ocupada > 0 {
        tracing::error!("se rechazó importar sobre una biblioteca que ya tiene contenido");
        return Err(NbError::SchemaMismatch);
    }

    let tx = crate::db::write_transaction(conn)?;

    for w in &export.works {
        tx.execute(
            "INSERT INTO works (id, canonical_title, original_language, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                w.id,
                w.canonical_title,
                w.original_language,
                w.created_at,
                w.updated_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for e in &export.editions {
        tx.execute(
            "INSERT INTO editions (id, work_id, title, subtitle, publisher, published_date,
                                   edition_statement, language, series_title, series_index,
                                   created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                e.id,
                e.work_id,
                e.title,
                e.subtitle,
                e.publisher,
                e.published_date,
                e.edition_statement,
                e.language,
                e.series_title,
                e.series_index,
                e.created_at,
                e.updated_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for i in &export.edition_identifiers {
        tx.execute(
            "INSERT INTO edition_identifiers (edition_id, scheme, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![i.edition_id, i.scheme, i.value],
        )
        .map_err(|e| fallo(&e))?;
    }

    /*
     * Un archivo exportado puede traer al mismo autor en varias
     * filas, y desde V38 la base solo admite una. Se funden al restaurar igual
     * que los fundió la migración: la primera fila de cada nombre se queda, y las
     * demás apuntan a ella. Sin esto, restaurar una exportación vieja fallaría
     * entera por el índice único, que es perder la biblioteca el día que hace
     * falta.
     */
    let mut fundidos: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for c in &export.contributors {
        let entro = tx
            .execute(
                "INSERT INTO contributors (id, name, sort_name, orcid, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT DO NOTHING",
                rusqlite::params![
                    c.id,
                    c.name,
                    c.sort_name,
                    c.orcid,
                    c.created_at,
                    c.updated_at
                ],
            )
            .map_err(|e| fallo(&e))?;
        if entro == 0 {
            // Con el mismo ORCID es la misma persona aunque se escriba distinto;
            // sin él, lo que decide es el nombre, como en V38.
            let ya: String = match &c.orcid {
                Some(orcid) => tx.query_row(
                    "SELECT id FROM contributors WHERE orcid = ?1",
                    [orcid],
                    |fila| fila.get(0),
                ),
                None => tx.query_row(
                    &format!(
                        "SELECT id FROM contributors WHERE name_key = {} AND orcid IS NULL",
                        crate::db::catalog::clave_de_nombre_sql("?1")
                    ),
                    [&c.name],
                    |fila| fila.get(0),
                ),
            }
            .map_err(|e| fallo(&e))?;
            fundidos.insert(c.id.as_str(), ya);
        }
    }

    for ec in &export.edition_contributors {
        let quien = fundidos
            .get(ec.contributor_id.as_str())
            .map_or(ec.contributor_id.as_str(), String::as_str);
        tx.execute(
            "INSERT INTO edition_contributors (edition_id, contributor_id, role, position)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT DO NOTHING",
            rusqlite::params![ec.edition_id, quien, ec.role, ec.position],
        )
        .map_err(|e| fallo(&e))?;
    }

    for s in &export.subjects {
        tx.execute(
            "INSERT INTO subjects (id, scheme, label) VALUES (?1, ?2, ?3)",
            rusqlite::params![s.id, s.scheme, s.label],
        )
        .map_err(|e| fallo(&e))?;
    }

    for es in &export.edition_subjects {
        tx.execute(
            "INSERT INTO edition_subjects (edition_id, subject_id) VALUES (?1, ?2)",
            rusqlite::params![es.edition_id, es.subject_id],
        )
        .map_err(|e| fallo(&e))?;
    }

    // Las raíces entran antes que los archivos, que las referencian. La
    // administrada ya existe desde la migración: se conserva la de destino, que
    // es la de esta instalación, en lugar de pisarla con la del origen.
    for r in &export.library_roots {
        tx.execute(
            "INSERT INTO library_roots (id, name, kind, path, created_at, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5)
             ON CONFLICT(id) DO NOTHING",
            rusqlite::params![
                r.id,
                r.name,
                root_kind_texto(r.kind),
                r.created_at,
                r.updated_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for f in &export.files {
        tx.execute(
            "INSERT INTO files (id, edition_id, root_id, relative_path, content_hash, byte_size,
                                media_type, storage_mode, availability, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                f.id,
                f.edition_id,
                f.root_id,
                f.relative_path,
                f.content_hash.as_str(),
                f.byte_size,
                f.media_type,
                storage_mode_texto(f.storage_mode),
                availability_texto(f.availability),
                f.created_at,
                f.updated_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for n in &export.notes {
        tx.execute(
            "INSERT INTO notes (id, title, body, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![n.id, n.title, n.body, n.created_at, n.updated_at],
        )
        .map_err(|e| fallo(&e))?;
    }

    for l in &export.note_links {
        tx.execute(
            "INSERT INTO note_links (source_id, target_id, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![l.source_id, l.target_id, l.created_at],
        )
        .map_err(|e| fallo(&e))?;
    }

    for a in &export.annotations {
        tx.execute(
            "INSERT INTO annotations (id, edition_id, kind, body, target, created_at,
                                      updated_at, deleted_at, supersedes, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                a.id,
                a.edition_id,
                a.kind.as_str(),
                a.body,
                a.target,
                a.created_at,
                a.updated_at,
                a.deleted_at,
                a.supersedes,
                a.payload
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    // Después de las anotaciones, que es a lo que apuntan.
    for l in &export.note_annotation_links {
        tx.execute(
            "INSERT INTO note_annotation_links (note_id, annotation_id, created_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![l.note_id, l.annotation_id, l.created_at],
        )
        .map_err(|e| fallo(&e))?;
    }

    for c in &export.collections {
        tx.execute(
            "INSERT INTO collections (id, name, kind, origin, rule, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                c.id,
                c.name,
                c.kind,
                c.origin,
                c.rule,
                c.created_at,
                c.updated_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for i in &export.collection_items {
        tx.execute(
            "INSERT INTO collection_items (collection_id, edition_id, position, added_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![i.collection_id, i.edition_id, i.position, i.added_at],
        )
        .map_err(|e| fallo(&e))?;
    }

    for p in &export.progress {
        tx.execute(
            "INSERT INTO progress (id, edition_id, profile, locator, precision,
                                   created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                p.id,
                p.edition_id,
                p.profile,
                p.locator,
                p.precision,
                p.created_at,
                p.updated_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for r in &export.reading_sessions {
        tx.execute(
            "INSERT INTO reading_sessions (id, edition_id, profile, started_at, ended_at,
                                           start_locator, end_locator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                r.id,
                r.edition_id,
                r.profile,
                r.started_at,
                r.ended_at,
                r.start_locator,
                r.end_locator
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for v in &export.vocabulary {
        tx.execute(
            "INSERT INTO vocabulary (id, headword, term, edition_id, provider_id, locator,
                                     created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                v.id,
                v.headword,
                v.term,
                v.edition_id,
                v.provider_id,
                v.locator,
                v.created_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for c in &export.study_cards {
        let due_at = c
            .due_at
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&c.created_at);
        let ease = if c.ease_factor < 1.3 {
            2.5
        } else {
            c.ease_factor
        };
        tx.execute(
            "INSERT INTO study_cards (id, edition_id, kind, front, back, choices, sources,
                                      provider, template, edited_at, created_at,
                                      due_at, interval_days, ease_factor, repetitions,
                                      last_reviewed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            rusqlite::params![
                c.id,
                c.edition_id,
                c.kind,
                c.front,
                c.back,
                c.choices,
                c.sources,
                c.provider,
                c.template,
                c.edited_at,
                c.created_at,
                due_at,
                c.interval_days,
                ease,
                c.repetitions,
                c.last_reviewed_at,
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for e in &export.ai_sends {
        tx.execute(
            "INSERT INTO ai_sends (id, sent_at, protocol, host, model, capability, body, bytes,
                                   status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                e.id,
                e.sent_at,
                e.protocol,
                e.host,
                e.model,
                e.capability,
                e.body,
                e.bytes,
                e.status
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for c in &export.concepts {
        tx.execute(
            "INSERT INTO concepts (id, edition_id, name, kind, description, sources, provider,
                                   template, edited_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                c.id,
                c.edition_id,
                c.name,
                c.kind,
                c.description,
                c.sources,
                c.provider,
                c.template,
                c.edited_at,
                c.created_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for p in &export.metadata_provenance {
        tx.execute(
            "INSERT INTO metadata_provenance (entity_type, entity_id, field, source, value,
                                              recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                p.entity_type.as_str(),
                p.entity_id,
                p.field,
                p.source.as_str(),
                p.value,
                p.recorded_at
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for s in &export.metadata_suggestions {
        // El tipo de entidad no viaja en la propuesta porque se deduce del
        // campo, que es una lista cerrada.
        let entidad = crate::metadata::entity_type_of_field(&s.field).ok_or_else(|| {
            tracing::error!("propuesta con un campo que esta versión no conoce");
            NbError::DocumentCorrupted
        })?;

        tx.execute(
            "INSERT INTO metadata_suggestions (id, entity_type, entity_id, field, source,
                                               proposed_value, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                s.id,
                entidad.as_str(),
                s.entity_id,
                s.field,
                s.source.as_str(),
                s.proposed_value,
                s.status.as_str(),
                s.created_at,
                // Una propuesta resuelta necesita fecha de resolución; la del
                // archivo no la lleva, así que se usa la de creación. Perder el
                // instante exacto de una resolución es aceptable; perder la
                // propuesta, no.
                if s.status == crate::metadata::SuggestionStatus::Pending {
                    None
                } else {
                    Some(s.created_at.clone())
                }
            ],
        )
        .map_err(|e| fallo(&e))?;
    }

    for s in &export.settings {
        if let Some(clave) = settings::SettingKey::from_name(&s.key)
            .filter(|c| *c != settings::SettingKey::MobileServer)
        {
            let json = serde_json::to_string(&s.value).map_err(|_| NbError::Internal)?;
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET
                   value = excluded.value,
                   updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')",
                rusqlite::params![clave.as_str(), json],
            )
            .map_err(|e| fallo(&e))?;
        } else {
            // Una preferencia que esta versión no conoce viene de una posterior.
            // Se ignora en lugar de fallar: no restaurar una preferencia
            // desconocida es un inconveniente; no restaurar la biblioteca por
            // ella sería absurdo.
            tracing::warn!("preferencia desconocida en la exportación; se omite");
        }
    }

    tx.commit().map_err(|e| fallo(&e))?;

    tracing::info!(
        obras = export.works.len(),
        ediciones = export.editions.len(),
        archivos = export.files.len(),
        anotaciones = export.annotations.len(),
        "biblioteca restaurada desde una exportación"
    );
    Ok(())
}

/// Borra el historial de lectura y por dónde iba cada documento.
///
/// Quita las sesiones cerradas (`reading_sessions`) y la última posición de cada
/// archivo (`progress`) en una sola transacción inmediata. No toca el catálogo, ni
/// las anotaciones, ni las notas, ni las preferencias, ni los archivos del usuario
/// en disco.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si SQLite rechaza la transacción.
pub fn clear_reading_history(conn: &mut Connection) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    tx.execute_batch(
        "DELETE FROM reading_sessions;
         DELETE FROM progress;",
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))?;
    tracing::info!("historial de lectura y progreso borrados por el usuario");
    Ok(())
}

/// Borra todas las anotaciones, las notas, sus enlaces y los tableros.
///
/// Ejecuta el borrado en una sola transacción inmediata. `DELETE FROM note_boards`
/// arrastra columnas y fichas por clave foránea; `DELETE FROM notes` arrastra los
/// enlaces entre notas y con anotaciones; `DELETE FROM annotations` vacía todas las
/// generaciones sin activar el disparador de inmutabilidad (`BEFORE UPDATE`). No
/// toca el catálogo, ni el historial de lectura, ni el vocabulario, ni las
/// preferencias, ni los archivos del usuario en disco.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si SQLite rechaza la transacción.
pub fn clear_annotations_and_notes(conn: &mut Connection) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    tx.execute_batch(
        "DELETE FROM note_boards;
         DELETE FROM notes;
         DELETE FROM annotations;",
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))?;
    tracing::info!("anotaciones, notas y tableros borrados por el usuario");
    Ok(())
}

/// Vacía la biblioteca entera en la base de datos.
///
/// Borra en una sola transacción inmediata las obras, ediciones, archivos
/// registrados, raíces referenciadas, colecciones personales, anotaciones, notas,
/// tableros, historial de lectura, progreso, vocabulario, tarjetas de estudio,
/// conceptos, propuestas de metadatos e índices de búsqueda.
///
/// Conserva la raíz gestionada vacía (`kind = 'managed'`), las colecciones de
/// sistema, los ajustes (`settings`), los catálogos OPDS, los diccionarios y los
/// permisos de red. **Nunca borra los archivos del usuario en el disco.**
///
/// # Errors
///
/// [`NbError::StorageFailure`] si SQLite rechaza la transacción.
pub fn clear_library(conn: &mut Connection) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    tx.execute_batch(
        "DELETE FROM note_boards;
         DELETE FROM notes;
         DELETE FROM annotations;
         DELETE FROM reading_sessions;
         DELETE FROM progress;
         DELETE FROM vocabulary;
         DELETE FROM study_cards;
         DELETE FROM concepts;
         DELETE FROM collection_items
           WHERE collection_id IN (SELECT id FROM collections WHERE origin = 'personal');
         DELETE FROM collections WHERE origin = 'personal';
         DELETE FROM works;
         DELETE FROM contributors;
         DELETE FROM subjects;
         DELETE FROM metadata_provenance;
         DELETE FROM metadata_suggestions;
         DELETE FROM shelf_counts;
         DELETE FROM catalog_fts;
         DELETE FROM import_duplicates;
         DELETE FROM import_rejections;
         DELETE FROM import_runs;
         DELETE FROM vector_excluded_collections;
         DELETE FROM library_roots WHERE kind = 'referenced';",
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))?;
    tracing::info!("biblioteca entera vaciada en la base de datos por el usuario");
    Ok(())
}

const fn storage_mode_texto(modo: StorageMode) -> &'static str {
    modo.as_str()
}

const fn availability_texto(a: crate::db::catalog::Availability) -> &'static str {
    a.as_str()
}

const fn root_kind_texto(k: RootKind) -> &'static str {
    k.as_str()
}
