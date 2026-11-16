//! Catálogo bibliográfico: obra, edición y archivo.
//!
//! Es la cara tipada del esquema de `migrations/V2__bibliographic_core.sql`.
//! Vive bajo `db` y no bajo un servicio porque no pertenece a ninguno: la
//! biblioteca, los metadatos, el lector y la búsqueda leen y escriben las
//! mismas entidades. Ponerlo dentro de uno de ellos obligaría a los otros tres
//! a depender de él, que es exactamente el acoplamiento que
//! `docs/architecture/system-overview.md` prohíbe.
//!
//! Contratos: `docs/architecture/api/schemas.md` y
//! `docs/architecture/metadata-model.md`.
//!
//! # Qué hay aquí y qué no
//!
//! Aquí está la representación y su persistencia. **No** están las políticas,
//! que deciden otros módulos: la precedencia del metadato
//! manual la hace `collections.rs`, la forma del `Locator` que viaja en `Annotation::target`
//! la fija `reader::locator`, la identidad por contenido es de `library.rs` y los modos de
//! almacenamiento los decide la importación.

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{NbError, NbResult};

/// Identificador de una fila del catálogo.
///
/// Los genera la base con `randomblob`, no el código: así una fila tiene
/// identidad desde el momento en que existe, sin un viaje de ida y vuelta para
/// pedir un identificador antes de insertar.
pub type Id = String;

fn fallo_de_almacenamiento(error: &rusqlite::Error) -> NbError {
    // El error de la biblioteca no cruza la frontera: puede contener fragmentos
    // de la consulta y, con ellos, títulos o rutas del usuario.
    tracing::error!(error = %error, "consulta del catálogo fallida");
    NbError::StorageFailure
}

/// Una obra: el concepto, independiente de cualquier publicación concreta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub id: Id,
    pub canonical_title: String,
    pub original_language: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Datos necesarios para dar de alta una obra.
#[derive(Debug, Clone, Default)]
pub struct NewWork {
    pub canonical_title: String,
    pub original_language: Option<String>,
}

impl Work {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            canonical_title: row.get("canonical_title")?,
            original_language: row.get("original_language")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Da de alta una obra y devuelve la fila creada.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base rechaza la inserción; en
    /// particular, si el título canónico está vacío.
    pub fn create(conn: &Connection, nueva: &NewWork) -> NbResult<Self> {
        conn.query_row(
            "INSERT INTO works (canonical_title, original_language)
             VALUES (?1, ?2)
             RETURNING id, canonical_title, original_language, created_at, updated_at",
            (&nueva.canonical_title, &nueva.original_language),
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Busca una obra por identificador.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla. Que no exista no es un
    /// error: se devuelve `None`.
    pub fn find(conn: &Connection, id: &str) -> NbResult<Option<Self>> {
        conn.query_row(
            "SELECT id, canonical_title, original_language, created_at, updated_at
             FROM works WHERE id = ?1",
            [id],
            Self::from_row,
        )
        .optional()
        .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

/// Una edición: una publicación concreta de una obra.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edition {
    pub id: Id,
    pub work_id: Id,
    pub title: String,
    pub subtitle: Option<String>,
    pub publisher: Option<String>,
    /// Fecha de publicación, posiblemente parcial: `2019`, `2019-05` o
    /// `2019-05-14`. Se guarda la precisión que se conoce, no una inventada.
    pub published_date: Option<String>,
    pub edition_statement: Option<String>,
    pub language: Option<String>,
    pub series_title: Option<String>,
    pub series_index: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Datos necesarios para dar de alta una edición.
#[derive(Debug, Clone, Default)]
pub struct NewEdition {
    pub work_id: Id,
    pub title: String,
    pub subtitle: Option<String>,
    pub publisher: Option<String>,
    pub published_date: Option<String>,
    pub edition_statement: Option<String>,
    pub language: Option<String>,
    pub series_title: Option<String>,
    pub series_index: Option<f64>,
}

const COLUMNAS_EDITION: &str = "id, work_id, title, subtitle, publisher, published_date, \
                                edition_statement, language, series_title, series_index, \
                                created_at, updated_at";

impl Edition {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            work_id: row.get("work_id")?,
            title: row.get("title")?,
            subtitle: row.get("subtitle")?,
            publisher: row.get("publisher")?,
            published_date: row.get("published_date")?,
            edition_statement: row.get("edition_statement")?,
            language: row.get("language")?,
            series_title: row.get("series_title")?,
            series_index: row.get("series_index")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Da de alta una edición de una obra existente.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la obra no existe, si el título está
    /// vacío o si la fecha de publicación no es una fecha parcial válida.
    pub fn create(conn: &Connection, nueva: &NewEdition) -> NbResult<Self> {
        conn.query_row(
            &format!(
                "INSERT INTO editions (work_id, title, subtitle, publisher, published_date,
                                       edition_statement, language, series_title, series_index)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 RETURNING {COLUMNAS_EDITION}"
            ),
            rusqlite::params![
                &nueva.work_id,
                &nueva.title,
                &nueva.subtitle,
                &nueva.publisher,
                &nueva.published_date,
                &nueva.edition_statement,
                &nueva.language,
                &nueva.series_title,
                &nueva.series_index,
            ],
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Ediciones de una obra, en orden estable de creación.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list_for_work(conn: &Connection, work_id: &str) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNAS_EDITION} FROM editions
                 WHERE work_id = ?1 ORDER BY created_at, id"
            ))
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([work_id], Self::from_row)
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Añade un identificador externo a la edición.
    ///
    /// Es idempotente: repetir el mismo par esquema y valor no falla ni
    /// duplica, porque una reimportación del mismo metadato es lo normal, no un
    /// error.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el esquema no es uno de los admitidos o
    /// si la edición no existe.
    pub fn add_identifier(
        conn: &Connection,
        edition_id: &str,
        scheme: IdentifierScheme,
        value: &str,
    ) -> NbResult<()> {
        conn.execute(
            "INSERT OR IGNORE INTO edition_identifiers (edition_id, scheme, value)
             VALUES (?1, ?2, ?3)",
            (edition_id, scheme.as_str(), value),
        )
        .map(|_| ())
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Identificadores externos de una edición.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn identifiers(conn: &Connection, edition_id: &str) -> NbResult<Vec<(String, String)>> {
        let mut stmt = conn
            .prepare(
                "SELECT scheme, value FROM edition_identifiers
                 WHERE edition_id = ?1 ORDER BY scheme, value",
            )
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([edition_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

/// Esquemas de identificador externo admitidos por el esquema.
///
/// Es un tipo y no una cadena para que añadir uno obligue a tocar también la
/// migración: los dos sitios tienen que decir lo mismo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierScheme {
    Isbn,
    Issn,
    Doi,
    Asin,
    Oclc,
    Local,
}

impl IdentifierScheme {
    /// Valor tal y como se almacena. Coincide con la restricción `CHECK` de la
    /// tabla `edition_identifiers`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Isbn => "isbn",
            Self::Issn => "issn",
            Self::Doi => "doi",
            Self::Asin => "asin",
            Self::Oclc => "oclc",
            Self::Local => "local",
        }
    }
}

/// Identidad de un archivo por su contenido.
///
/// Es un tipo y no una cadena porque su forma es un contrato que sobrevive a la
/// biblioteca: si mañana hiciera falta otro algoritmo, hay que poder distinguir
/// los hashes viejos de los nuevos sin rehashear 150 000 archivos para
/// averiguarlo. De ahí el prefijo del algoritmo, como en las sumas de
/// integridad de subrecurso o en los digests de contenedor.
///
/// Forma: `sha256:` seguido de 64 caracteres hexadecimales en minúscula.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentHash(String);

impl ContentHash {
    /// Prefijo del único algoritmo en uso.
    pub const ALGORITMO: &'static str = "sha256";

    /// Longitud total de la forma canónica: `sha256:` más 64 hexadecimales.
    pub const LONGITUD: usize = 7 + 64;

    /// Construye la identidad a partir del resumen ya calculado.
    #[must_use]
    pub fn from_sha256(digest: &[u8; 32]) -> Self {
        let mut texto = String::with_capacity(Self::LONGITUD);
        texto.push_str(Self::ALGORITMO);
        texto.push(':');
        for byte in digest {
            use std::fmt::Write as _;
            // Ignorar el resultado es correcto: escribir en un String no falla.
            let _ = write!(texto, "{byte:02x}");
        }
        Self(texto)
    }

    /// Lee una identidad almacenada.
    ///
    /// # Errors
    ///
    /// [`NbError::DocumentCorrupted`] si la cadena no tiene la forma canónica.
    /// Una identidad mal formada en la base solo puede venir de una escritura
    /// que se saltó este tipo, y tratarla como válida propagaría el error.
    pub fn parse(texto: &str) -> NbResult<Self> {
        let valido = texto.len() == Self::LONGITUD
            && texto.starts_with("sha256:")
            && texto[7..]
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));

        if !valido {
            tracing::error!(
                longitud = texto.len(),
                "identidad de contenido con forma inesperada"
            );
            return Err(NbError::DocumentCorrupted);
        }
        Ok(Self(texto.to_owned()))
    }

    /// Forma canónica, tal y como se almacena.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentHash {
    type Error = NbError;

    fn try_from(texto: String) -> Result<Self, Self::Error> {
        Self::parse(&texto)
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.0
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Modo de almacenamiento de un archivo.
///
/// La política que decide cuál corresponde a cada archivo es de la importación; aquí solo
/// se representa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    /// El archivo vive dentro de la carpeta de biblioteca y la aplicación lo
    /// gobierna.
    Managed,
    /// El archivo vive donde el usuario lo dejó y solo se guarda su ruta.
    Referenced,
}

impl StorageMode {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Referenced => "referenced",
        }
    }

    fn from_str(valor: &str) -> Option<Self> {
        match valor {
            "managed" => Some(Self::Managed),
            "referenced" => Some(Self::Referenced),
            _ => None,
        }
    }
}

/// Disponibilidad de un archivo en disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum Availability {
    Available,
    /// La ruta ya no existe: volumen desconectado o archivo movido.
    Missing,
    /// La ruta existe pero no se puede leer: permisos o soporte dañado.
    Unreadable,
}

impl Availability {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
        }
    }

    fn from_str(valor: &str) -> Option<Self> {
        match valor {
            "available" => Some(Self::Available),
            "missing" => Some(Self::Missing),
            "unreadable" => Some(Self::Unreadable),
            _ => None,
        }
    }
}

/// Identificador fijo de la raíz administrada.
///
/// Fijo a propósito: la exportación y la importación tienen que reconocerla
/// entre equipos, y un aleatorio distinto en cada instalación lo impediría.
pub const MANAGED_ROOT_ID: &str = "00000000000000000000000000000001";

/// Clase de raíz de biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RootKind {
    /// La carpeta de biblioteca de esta instalación. Su ruta **nunca** se
    /// guarda: se resuelve en ejecución, porque es un dato de la instalación y
    /// no del catálogo.
    Managed,
    /// Un lugar declarado por el usuario. Su ruta se guarda, y es la única que
    /// hay que corregir al cambiar de equipo.
    Referenced,
}

impl RootKind {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Referenced => "referenced",
        }
    }

    fn from_str(valor: &str) -> Option<Self> {
        match valor {
            "managed" => Some(Self::Managed),
            "referenced" => Some(Self::Referenced),
            _ => None,
        }
    }
}

/// Un lugar declarado donde viven archivos de la biblioteca.
///
/// Existe para que ninguna ruta absoluta de archivo se guarde en el catálogo.
/// Ver `docs/architecture/storage.md`, sección 2.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRoot {
    pub id: Id,
    pub name: String,
    pub kind: RootKind,
    /// `None` en una raíz administrada significa "se resuelve en ejecución".
    /// `None` en una referenciada significa **no localizada**: es el estado en
    /// que quedan tras restaurar en otro equipo.
    pub path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const COLUMNAS_ROOT: &str = "id, name, kind, path, created_at, updated_at";

impl LibraryRoot {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let kind: String = row.get("kind")?;
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            kind: RootKind::from_str(&kind).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    "valor fuera de la lista admitida en kind".into(),
                )
            })?,
            path: row.get("path")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Declara una raíz referenciada.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la inserción falla.
    pub fn create_referenced(conn: &Connection, name: &str, path: &str) -> NbResult<Self> {
        conn.query_row(
            &format!(
                "INSERT INTO library_roots (name, kind, path) VALUES (?1, 'referenced', ?2)
                 RETURNING {COLUMNAS_ROOT}"
            ),
            rusqlite::params![name, path],
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Busca una raíz por identificador.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn find(conn: &Connection, id: &str) -> NbResult<Option<Self>> {
        conn.query_row(
            &format!("SELECT {COLUMNAS_ROOT} FROM library_roots WHERE id = ?1"),
            [id],
            Self::from_row,
        )
        .optional()
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Busca una raíz referenciada por su ruta exacta.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn find_by_path(conn: &Connection, path: &str) -> NbResult<Option<Self>> {
        conn.query_row(
            &format!(
                "SELECT {COLUMNAS_ROOT} FROM library_roots
                 WHERE kind = 'referenced' AND path = ?1"
            ),
            [path],
            Self::from_row,
        )
        .optional()
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Todas las raíces, en orden estable.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list(conn: &Connection) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNAS_ROOT} FROM library_roots ORDER BY kind, name, id"
            ))
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([], Self::from_row)
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Vuelve a apuntar una raíz referenciada.
    ///
    /// Es la operación entera de la mudanza: una llamada por raíz devuelve
    /// todos sus documentos, sin reimportar y sin tocar una sola anotación.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la raíz no existe o es administrada.
    pub fn relocate(conn: &Connection, id: &str, path: &str) -> NbResult<Self> {
        let destino = Self::resolver_carpeta_de_raiz(conn, id, path)?;
        conn.query_row(
            &format!(
                "UPDATE library_roots
                 SET path = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1 AND kind = 'referenced'
                 RETURNING {COLUMNAS_ROOT}"
            ),
            rusqlite::params![id, &destino],
            Self::from_row,
        )
        .optional()
        .map_err(|e| fallo_de_almacenamiento(&e))?
        .ok_or_else(|| {
            tracing::error!("se intentó reapuntar una raíz inexistente o administrada");
            crate::error::NbError::StorageFailure
        })
    }

    /// Si el usuario elige la carpeta de encima de la raíz —por ejemplo
    /// `.../Biblioteca` cuando la raíz era `.../Biblioteca/biblioteca`—, baja al
    /// subdirectorio donde viven de verdad las rutas relativas registradas.
    fn resolver_carpeta_de_raiz(conn: &Connection, id: &str, path: &str) -> NbResult<String> {
        let mut stmt = conn
            .prepare("SELECT relative_path FROM files WHERE root_id = ?1 LIMIT 8")
            .map_err(|e| fallo_de_almacenamiento(&e))?;
        let muestra: Vec<String> = stmt
            .query_map([id], |row| row.get(0))
            .and_then(|filas| filas.collect())
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let base = std::path::Path::new(path);
        if muestra.is_empty() || muestra.iter().any(|rel| base.join(rel).exists()) {
            return Ok(path.to_owned());
        }

        if let Some(raiz) = Self::find(conn, id)? {
            let sub = base.join(&raiz.name);
            if sub.is_dir() && muestra.iter().any(|rel| sub.join(rel).exists()) {
                let resuelta = sub.canonicalize().unwrap_or(sub);
                return Ok(resuelta.to_string_lossy().replace('\\', "/"));
            }
        }

        if let Ok(entradas) = std::fs::read_dir(base) {
            for entrada in entradas.filter_map(Result::ok).take(32) {
                let sub = entrada.path();
                if sub.is_dir() && muestra.iter().any(|rel| sub.join(rel).exists()) {
                    let resuelta = sub.canonicalize().unwrap_or(sub);
                    return Ok(resuelta.to_string_lossy().replace('\\', "/"));
                }
            }
        }

        Ok(path.to_owned())
    }

    /// Reescribe la ruta de la raíz administrada con la de esta instalación.
    ///
    /// Se llama al abrir la base. La ruta de esa raíz es una **caché**, no una
    /// fuente de verdad: mover la aplicación, reinstalarla o restaurar en otro
    /// equipo la corrigen solos, sin que el usuario tenga que hacer nada.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la actualización falla.
    pub fn sync_managed(conn: &Connection, path: &str) -> NbResult<()> {
        // Se comprueba antes de escribir, y no se escribe si ya coincide. No es
        // una optimización: abrir la base es lo primero que hace la aplicación,
        // y una escritura ahí bloquearía ante cualquier otra conexión que
        // estuviera escribiendo. El caso normal, que es que no haya cambiado
        // nada, tiene que ser una lectura.
        let actual: Option<Option<String>> = conn
            .query_row(
                "SELECT path FROM library_roots WHERE id = ?1",
                [MANAGED_ROOT_ID],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        if actual.as_ref().and_then(Option::as_deref) == Some(path) {
            return Ok(());
        }

        conn.execute(
            "UPDATE library_roots
             SET path = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?2",
            rusqlite::params![path, MANAGED_ROOT_ID],
        )
        .map(|_| ())
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Marca una raíz como no localizada, que es como quedan al restaurar en
    /// otro equipo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la actualización falla.
    pub fn forget_path(conn: &Connection, id: &str) -> NbResult<()> {
        conn.execute(
            "UPDATE library_roots
             SET path = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND kind = 'referenced'",
            [id],
        )
        .map(|_| ())
        .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

/// Un archivo en disco que materializa una edición.
///
/// Su ubicación son **dos** datos y no uno: la raíz y la ruta dentro de ella.
/// Ninguna ruta absoluta de archivo se guarda en el catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: Id,
    pub edition_id: Id,
    pub root_id: Id,
    pub relative_path: String,
    pub content_hash: ContentHash,
    pub byte_size: i64,
    pub media_type: String,
    pub storage_mode: StorageMode,
    pub availability: Availability,
    pub created_at: String,
    pub updated_at: String,
}

/// Datos necesarios para dar de alta un archivo.
#[derive(Debug, Clone)]
pub struct NewFile {
    pub edition_id: Id,
    pub root_id: Id,
    pub relative_path: String,
    pub content_hash: ContentHash,
    pub byte_size: i64,
    pub media_type: String,
    pub storage_mode: StorageMode,
}

const COLUMNAS_FILE: &str = "id, edition_id, root_id, relative_path, content_hash, byte_size, \
                             media_type, storage_mode, availability, created_at, updated_at";

/// Expresión SQL que resuelve la ruta absoluta de un archivo de la tabla `files f`.
///
/// Vale `NULL` cuando la raíz **no está localizada**, que es exactamente lo que
/// hay que saber: un archivo cuya raíz no se ha reapuntado todavía no tiene
/// dirección, y tratarlo como ausente es la respuesta correcta.
///
/// Se une con barra normal también en Windows, que la acepta sin problema.
pub const SQL_RUTA_ABSOLUTA: &str =
    "((SELECT r.path FROM library_roots r WHERE r.id = f.root_id) || '/' || f.relative_path)";

impl File {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let storage_mode: String = row.get("storage_mode")?;
        let availability: String = row.get("availability")?;

        // Un valor fuera de la lista solo puede venir de una base manipulada a
        // mano: se trata como error, no se adivina un valor por defecto.
        let invalido = |columna: &'static str| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("valor fuera de la lista admitida en {columna}").into(),
            )
        };

        let content_hash: String = row.get("content_hash")?;

        Ok(Self {
            id: row.get("id")?,
            edition_id: row.get("edition_id")?,
            root_id: row.get("root_id")?,
            relative_path: row.get("relative_path")?,
            content_hash: ContentHash::parse(&content_hash)
                .map_err(|_| invalido("content_hash"))?,
            byte_size: row.get("byte_size")?,
            media_type: row.get("media_type")?,
            storage_mode: StorageMode::from_str(&storage_mode)
                .ok_or_else(|| invalido("storage_mode"))?,
            availability: Availability::from_str(&availability)
                .ok_or_else(|| invalido("availability"))?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Da de alta un archivo asociado a una edición.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la edición no existe o si la ruta ya está
    /// registrada.
    pub fn create(conn: &Connection, nuevo: &NewFile) -> NbResult<Self> {
        conn.query_row(
            &format!(
                "INSERT INTO files (edition_id, root_id, relative_path, content_hash, byte_size,
                                    media_type, storage_mode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 RETURNING {COLUMNAS_FILE}"
            ),
            rusqlite::params![
                &nuevo.edition_id,
                &nuevo.root_id,
                &nuevo.relative_path,
                nuevo.content_hash.as_str(),
                nuevo.byte_size,
                &nuevo.media_type,
                nuevo.storage_mode.as_str(),
            ],
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Busca un archivo por identificador.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla. Que no exista no es un
    /// error: se devuelve `None`.
    pub fn find(conn: &Connection, id: &str) -> NbResult<Option<Self>> {
        conn.query_row(
            &format!("SELECT {COLUMNAS_FILE} FROM files WHERE id = ?1"),
            [id],
            Self::from_row,
        )
        .optional()
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Archivos de una edición, en orden estable.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list_for_edition(conn: &Connection, edition_id: &str) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNAS_FILE} FROM files
                 WHERE edition_id = ?1 ORDER BY created_at, id"
            ))
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([edition_id], Self::from_row)
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Archivos con un hash de contenido dado, en cualquier edición.
    ///
    /// Es la consulta sobre la que la importación decide si un archivo es un
    /// duplicado o un archivo movido.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn find_by_content_hash(
        conn: &Connection,
        content_hash: &ContentHash,
    ) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {COLUMNAS_FILE} FROM files
                 WHERE content_hash = ?1 ORDER BY created_at, id"
            ))
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([content_hash.as_str()], Self::from_row)
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Cambia la disponibilidad de un archivo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la actualización falla.
    pub fn set_availability(
        conn: &Connection,
        id: &str,
        availability: Availability,
    ) -> NbResult<()> {
        conn.execute(
            "UPDATE files SET availability = ?2 WHERE id = ?1",
            (id, availability.as_str()),
        )
        .map(|_| ())
        .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

/// Persona o entidad que contribuye a una edición.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub id: Id,
    pub name: String,
    pub sort_name: Option<String>,
    pub orcid: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Datos necesarios para dar de alta un contribuidor.
#[derive(Debug, Clone, Default)]
pub struct NewContributor {
    pub name: String,
    pub sort_name: Option<String>,
    pub orcid: Option<String>,
}

/// Letras que la clave de un nombre pliega, con lo que las sustituye.
///
/// Lo que se decidió el 2026-09-24: dos autores son el
/// mismo si su nombre coincide **sin distinguir mayúsculas ni tildes**. `lower()` de
/// SQLite solo pliega ASCII, así que las mayúsculas acentuadas van aquí también.
/// Cubre el latín de Europa, que es lo que trae el corpus; un nombre en cirílico o
/// en griego se compara tal cual, y eso se dice en `metadata-model.md`.
#[rustfmt::skip]
const PLIEGUES: &[(&str, &str)] = &[
    ("á", "a"), ("à", "a"), ("â", "a"), ("ä", "a"), ("ã", "a"), ("å", "a"), ("ā", "a"), ("ą", "a"), ("ă", "a"),
    ("Á", "a"), ("À", "a"), ("Â", "a"), ("Ä", "a"), ("Ã", "a"), ("Å", "a"), ("Ā", "a"), ("Ą", "a"), ("Ă", "a"),
    ("é", "e"), ("è", "e"), ("ê", "e"), ("ë", "e"), ("ē", "e"), ("ę", "e"), ("ě", "e"), ("ė", "e"),
    ("É", "e"), ("È", "e"), ("Ê", "e"), ("Ë", "e"), ("Ē", "e"), ("Ę", "e"), ("Ě", "e"), ("Ė", "e"),
    ("í", "i"), ("ì", "i"), ("î", "i"), ("ï", "i"), ("ī", "i"), ("į", "i"), ("ı", "i"),
    ("Í", "i"), ("Ì", "i"), ("Î", "i"), ("Ï", "i"), ("Ī", "i"), ("Į", "i"), ("İ", "i"),
    ("ó", "o"), ("ò", "o"), ("ô", "o"), ("ö", "o"), ("õ", "o"), ("ø", "o"), ("ō", "o"), ("ő", "o"),
    ("Ó", "o"), ("Ò", "o"), ("Ô", "o"), ("Ö", "o"), ("Õ", "o"), ("Ø", "o"), ("Ō", "o"), ("Ő", "o"),
    ("ú", "u"), ("ù", "u"), ("û", "u"), ("ü", "u"), ("ū", "u"), ("ů", "u"), ("ű", "u"), ("ų", "u"),
    ("Ú", "u"), ("Ù", "u"), ("Û", "u"), ("Ü", "u"), ("Ū", "u"), ("Ů", "u"), ("Ű", "u"), ("Ų", "u"),
    ("ç", "c"), ("ć", "c"), ("č", "c"), ("Ç", "c"), ("Ć", "c"), ("Č", "c"),
    ("ñ", "n"), ("ń", "n"), ("ň", "n"), ("Ñ", "n"), ("Ń", "n"), ("Ň", "n"),
    ("ś", "s"), ("š", "s"), ("ş", "s"), ("ș", "s"), ("Ś", "s"), ("Š", "s"), ("Ş", "s"), ("Ș", "s"),
    ("ź", "z"), ("ż", "z"), ("ž", "z"), ("Ź", "z"), ("Ż", "z"), ("Ž", "z"),
    ("ý", "y"), ("ÿ", "y"), ("Ý", "y"), ("Ÿ", "y"),
    ("ł", "l"), ("ľ", "l"), ("ĺ", "l"), ("Ł", "l"), ("Ľ", "l"), ("Ĺ", "l"),
    ("ř", "r"), ("ŕ", "r"), ("Ř", "r"), ("Ŕ", "r"),
    ("ď", "d"), ("đ", "d"), ("Ď", "d"), ("Đ", "d"),
    ("ť", "t"), ("ţ", "t"), ("ț", "t"), ("Ť", "t"), ("Ţ", "t"), ("Ț", "t"),
    ("ğ", "g"), ("Ğ", "g"),
    ("ß", "ss"), ("æ", "ae"), ("Æ", "ae"), ("œ", "oe"), ("Œ", "oe"),
];

/// La expresión SQL que da la clave de un nombre: sin tildes, en minúsculas, sin
/// espacios de sobra. Es también la del título (V39), que es con la que se
/// encuentra lo duplicado: mismo título y mismo autor.
///
/// # Por qué es SQL y no Rust
///
/// Porque la clave es una **columna generada** de `contributors` con un índice
/// único encima (V38): así la base es quien impide dos filas para el mismo autor,
/// venga la escritura de la importación, de una restauración o de una migración
/// futura, y cualquier `sqlite3` puede abrirla. Una función registrada desde Rust
/// dejaría la columna ilegible fuera de la aplicación. El precio es que la misma
/// expresión vive aquí y en `V38__un_autor_una_fila.sql`, y una prueba exige que
/// sean la misma cadena, como con `search::CREAR_INDICE`.
#[must_use]
pub fn clave_de_nombre_sql(columna: &str) -> String {
    let mut expresion = format!("trim({columna})");
    for _ in 0..2 {
        expresion = format!("replace({expresion}, '  ', ' ')");
    }
    for (de, a) in PLIEGUES {
        expresion = format!("replace({expresion}, '{de}', '{a}')");
    }
    format!("lower({expresion})")
}

/// Papel de un contribuidor en una edición.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributorRole {
    Author,
    Editor,
    Translator,
    Illustrator,
}

impl ContributorRole {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Editor => "editor",
            Self::Translator => "translator",
            Self::Illustrator => "illustrator",
        }
    }
}

impl Contributor {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            sort_name: row.get("sort_name")?,
            orcid: row.get("orcid")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    /// Da de alta un contribuidor.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el nombre está vacío, si el ORCID no
    /// tiene la forma correcta o si ya existe otro contribuidor con ese ORCID.
    pub fn create(conn: &Connection, nuevo: &NewContributor) -> NbResult<Self> {
        conn.query_row(
            "INSERT INTO contributors (name, sort_name, orcid)
             VALUES (?1, ?2, ?3)
             RETURNING id, name, sort_name, orcid, created_at, updated_at",
            (&nuevo.name, &nuevo.sort_name, &nuevo.orcid),
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// El contribuidor con este nombre, **el que ya existe si lo hay**.
    ///
    /// Es lo que usa la importación. Hasta entonces cada obra daba de
    /// alta a su autor otra vez: en la biblioteca real, 207.282 filas para 61.187
    /// nombres. El índice único de V38 ya impide la segunda fila; esto es lo que
    /// hace que no haga falta chocar con él.
    ///
    /// Primero se inserta sin pisar nada y después se busca, en ese orden y no al
    /// revés: dos importaciones que traen al mismo autor nuevo a la vez no pueden
    /// crearlo dos veces, porque la segunda inserción no hace nada.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el nombre está vacío o la base falla.
    pub fn ensure(conn: &Connection, name: &str) -> NbResult<Self> {
        conn.execute(
            "INSERT INTO contributors (name) VALUES (?1) ON CONFLICT DO NOTHING",
            [name],
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;
        conn.query_row(
            &format!(
                "SELECT id, name, sort_name, orcid, created_at, updated_at
                   FROM contributors
                  WHERE name_key = {} AND orcid IS NULL",
                clave_de_nombre_sql("?1")
            ),
            [name],
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Asocia un contribuidor a una edición con un papel y una posición.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la edición o el contribuidor no existen.
    pub fn link(
        conn: &Connection,
        edition_id: &str,
        contributor_id: &str,
        role: ContributorRole,
        position: i64,
    ) -> NbResult<()> {
        /*
         * `ON CONFLICT DO UPDATE` y no `INSERT OR REPLACE`.
         *
         * Con `OR REPLACE`, volver a enlazar la misma autoría -lo que hace una
         * reimportación o un refresco de metadatos- **borra la fila y la vuelve
         * a insertar**. Y en SQLite, el borrado de un `REPLACE` no dispara los
         * disparadores de baja salvo que `recursive_triggers` esté activo, que
         * no lo está: se disparaba el alta y no la baja, así que el recuento de
         * la estantería del autor crecía uno cada vez sobre una biblioteca que
         * no había cambiado.
         *
         * Aquí el efecto es el mismo -las otras tres columnas son la clave
         * primaria, así que lo único que puede cambiar es la posición- sin
         * borrar nada, y ningún disparador ve un movimiento que no hubo.
         */
        conn.execute(
            "INSERT INTO edition_contributors
                 (edition_id, contributor_id, role, position)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (edition_id, contributor_id, role)
             DO UPDATE SET position = excluded.position",
            (edition_id, contributor_id, role.as_str(), position),
        )
        .map(|_| ())
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Contribuidores de una edición con su papel, en el orden declarado.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list_for_edition(
        conn: &Connection,
        edition_id: &str,
    ) -> NbResult<Vec<(Self, String, i64)>> {
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.name, c.sort_name, c.orcid, c.created_at, c.updated_at,
                        ec.role, ec.position
                 FROM edition_contributors ec
                 JOIN contributors c ON c.id = ec.contributor_id
                 WHERE ec.edition_id = ?1
                 ORDER BY ec.role, ec.position, c.name",
            )
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([edition_id], |row| {
                Ok((Self::from_row(row)?, row.get("role")?, row.get("position")?))
            })
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

/// Materia o entrada de clasificación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    pub id: Id,
    /// Vocabulario del que procede la etiqueta. `local` es la materia que
    /// escribe el usuario y no viene de ningún esquema publicado.
    pub scheme: String,
    pub label: String,
}

impl Subject {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            scheme: row.get("scheme")?,
            label: row.get("label")?,
        })
    }

    /// Devuelve la materia, creándola si no existía.
    ///
    /// Es idempotente porque la misma materia llega una y otra vez desde los
    /// metadatos de documentos distintos.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la etiqueta está vacía o la consulta
    /// falla.
    pub fn ensure(conn: &Connection, scheme: &str, label: &str) -> NbResult<Self> {
        conn.execute(
            "INSERT OR IGNORE INTO subjects (scheme, label) VALUES (?1, ?2)",
            (scheme, label),
        )
        .map_err(|e| fallo_de_almacenamiento(&e))?;

        conn.query_row(
            "SELECT id, scheme, label FROM subjects WHERE scheme = ?1 AND label = ?2",
            (scheme, label),
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Asocia una materia a una edición.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la edición o la materia no existen.
    pub fn link(conn: &Connection, edition_id: &str, subject_id: &str) -> NbResult<()> {
        conn.execute(
            "INSERT OR IGNORE INTO edition_subjects (edition_id, subject_id) VALUES (?1, ?2)",
            (edition_id, subject_id),
        )
        .map(|_| ())
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Materias de una edición.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list_for_edition(conn: &Connection, edition_id: &str) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.scheme, s.label
                 FROM edition_subjects es
                 JOIN subjects s ON s.id = es.subject_id
                 WHERE es.edition_id = ?1
                 ORDER BY s.scheme, s.label",
            )
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([edition_id], Self::from_row)
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

/// Clase de anotación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum AnnotationKind {
    Highlight,
    Note,
    Bookmark,
    /// Subrayado de palabras sueltas.
    ///
    /// Clase propia y no una variante del resaltado, decidido: el
    /// gesto es otro —"esta palabra la quiero buscar o traducir"— y las dos
    /// tienen que poder convivir sobre las mismas palabras.
    Underline,
    /// Trazo libre. Su geometría viaja en `payload`, no en el cuerpo.
    Drawing,
}

impl AnnotationKind {
    /// Valor tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Highlight => "highlight",
            Self::Note => "note",
            Self::Bookmark => "bookmark",
            Self::Underline => "underline",
            Self::Drawing => "drawing",
        }
    }

    /// Interpreta el valor almacenado.
    ///
    /// Se llama `parse` y no `from_str` para no chocar con el rasgo estándar del
    /// mismo nombre, que devolvería `Result` y obligaría a un tipo de error que
    /// aquí no aporta nada: o el valor está en la lista o no está.
    #[must_use]
    pub fn parse(valor: &str) -> Option<Self> {
        match valor {
            "highlight" => Some(Self::Highlight),
            "note" => Some(Self::Note),
            "bookmark" => Some(Self::Bookmark),
            "underline" => Some(Self::Underline),
            "drawing" => Some(Self::Drawing),
            _ => None,
        }
    }
}

/// Una anotación anclada a una edición.
///
/// Se ancla a la edición y no al archivo a propósito: sustituir el archivo por
/// otra copia del mismo texto, o moverlo de sitio, no puede desanclar el
/// trabajo del usuario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: Id,
    pub edition_id: Id,
    pub kind: AnnotationKind,
    pub body: Option<String>,
    /// Selectores serializados como objeto JSON. Su forma la fija el Locator
    /// (ADR-004); la base solo garantiza que es un objeto, nunca un entero de
    /// posición.
    pub target: String,
    /// Geometría del trazo, si la marca es un trazo libre.
    ///
    /// Objeto JSON, y nulo en todas las demás clases. Va aparte de `body` porque
    /// `body` es lo que escribe una persona: mezclar coordenadas ahí obligaría a
    /// adivinar cuál de las dos cosas es al leerlo.
    #[serde(default)]
    pub payload: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Cuándo se dio de baja. Nula mientras está viva.
    #[serde(default)]
    pub deleted_at: Option<String>,
    /// La versión anterior, si esta es la edición de otra.
    #[serde(default)]
    pub supersedes: Option<String>,
}

/// Datos necesarios para crear una anotación.
#[derive(Debug, Clone)]
pub struct NewAnnotation {
    pub edition_id: Id,
    pub kind: AnnotationKind,
    pub body: Option<String>,
    pub target: String,
    /// Geometría, solo para el trazo libre. La base rechaza un trazo sin ella.
    pub payload: Option<String>,
}

const COLUMNAS_ANNOTATION: &str = "id, edition_id, kind, body, target, payload, created_at, \
                                   updated_at, deleted_at, supersedes";

impl Annotation {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let kind: String = row.get("kind")?;

        Ok(Self {
            id: row.get("id")?,
            edition_id: row.get("edition_id")?,
            kind: AnnotationKind::parse(&kind).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    "clase de anotación fuera de la lista admitida".into(),
                )
            })?,
            body: row.get("body")?,
            target: row.get("target")?,
            payload: row.get("payload")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            deleted_at: row.get("deleted_at")?,
            supersedes: row.get("supersedes")?,
        })
    }

    /// Crea una anotación.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la edición no existe o si `target` no es
    /// un objeto JSON. Un entero de posición se rechaza aquí.
    pub fn create(conn: &Connection, nueva: &NewAnnotation) -> NbResult<Self> {
        conn.query_row(
            &format!(
                "INSERT INTO annotations (edition_id, kind, body, target, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 RETURNING {COLUMNAS_ANNOTATION}"
            ),
            rusqlite::params![
                &nueva.edition_id,
                nueva.kind.as_str(),
                &nueva.body,
                &nueva.target,
                &nueva.payload,
            ],
            Self::from_row,
        )
        .map_err(|e| fallo_de_almacenamiento(&e))
    }

    /// Anotaciones de una edición, en orden estable.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn list_for_edition(conn: &Connection, edition_id: &str) -> NbResult<Vec<Self>> {
        let mut stmt = conn
            .prepare(&format!(
                // Solo las vivas. Desde la V7 una anotación editada o borrada
                // se conserva dada de baja, y devolverla aquí le mostraría al
                // usuario versiones que él ya había sustituido.
                "SELECT {COLUMNAS_ANNOTATION} FROM annotations
                 WHERE edition_id = ?1 AND deleted_at IS NULL ORDER BY created_at, id"
            ))
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        let filas = stmt
            .query_map([edition_id], Self::from_row)
            .map_err(|e| fallo_de_almacenamiento(&e))?;

        filas
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo_de_almacenamiento(&e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Base con el esquema aplicado y la configuración real del pool, no una
    /// conexión desnuda: las claves foráneas están desactivadas por defecto en
    /// SQLite, y probar sobre una conexión sin configurar daría por buenas
    /// inserciones que en producción fallan.
    fn base() -> Database {
        Database::open_in_memory().expect("base en memoria")
    }

    fn obra(conn: &Connection, titulo: &str) -> Work {
        Work::create(
            conn,
            &NewWork {
                canonical_title: titulo.to_owned(),
                original_language: Some("es".to_owned()),
            },
        )
        .expect("obra creada")
    }

    fn edicion(conn: &Connection, work_id: &str, titulo: &str) -> Edition {
        Edition::create(
            conn,
            &NewEdition {
                work_id: work_id.to_owned(),
                title: titulo.to_owned(),
                ..NewEdition::default()
            },
        )
        .expect("edición creada")
    }

    /// Identidad de contenido fabricada, determinista y con la forma canónica.
    /// Las pruebas de este módulo comprueban el esquema, no el algoritmo: eso es
    /// de `library.rs`.
    fn hash(semilla: u8) -> ContentHash {
        ContentHash::from_sha256(&[semilla; 32])
    }

    /// Traduce una ruta absoluta a raíz más ruta relativa, que es como se
    /// guarda. Las pruebas siguen escribiendo rutas legibles.
    fn ubicacion(conn: &Connection, ruta: &str) -> (String, String) {
        crate::library::ensure_root(
            conn,
            std::path::Path::new(ruta),
            StorageMode::Referenced,
            None,
        )
        .expect("raíz declarada")
    }

    fn archivo(conn: &Connection, edition_id: &str, ruta: &str, semilla: u8) -> File {
        let (root_id, relative_path) = ubicacion(conn, ruta);
        File::create(
            conn,
            &NewFile {
                edition_id: edition_id.to_owned(),
                root_id,
                relative_path,
                content_hash: hash(semilla),
                byte_size: 1024,
                media_type: "application/epub+zip".to_owned(),
                storage_mode: StorageMode::Managed,
            },
        )
        .expect("archivo creado")
    }

    fn anotacion(conn: &Connection, edition_id: &str, texto: &str) -> Annotation {
        Annotation::create(
            conn,
            &NewAnnotation {
                payload: None,
                edition_id: edition_id.to_owned(),
                kind: AnnotationKind::Note,
                body: Some(texto.to_owned()),
                target: r#"{"type":"FragmentSelector","value":"epubcfi(/6/4!/2)"}"#.to_owned(),
            },
        )
        .expect("anotación creada")
    }

    // --- Identidad y alta ---------------------------------------------------

    #[test]
    fn una_obra_recien_creada_se_recupera_igual() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let creada = obra(&conn, "Rayuela");
        let recuperada = Work::find(&conn, &creada.id).expect("consulta");

        assert_eq!(recuperada, Some(creada));
    }

    /// Los identificadores tienen que sobrevivir a exportar y reimportar en otra
    /// máquina. Un entero local obligaría a reasignar claves en cada fusión.
    #[test]
    fn los_identificadores_no_son_correlativos() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let primera = obra(&conn, "Primera");
        let segunda = obra(&conn, "Segunda");

        assert_ne!(primera.id, segunda.id);
        assert_eq!(primera.id.len(), 32, "id inesperado: {}", primera.id);
        assert!(primera.id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(
            primera.id.parse::<u32>().is_err(),
            "un identificador correlativo rompería la fusión de bibliotecas"
        );
    }

    #[test]
    fn una_obra_sin_titulo_se_rechaza() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let resultado = Work::create(
            &conn,
            &NewWork {
                canonical_title: "   ".to_owned(),
                original_language: None,
            },
        );

        assert!(matches!(resultado, Err(NbError::StorageFailure)));
    }

    /// Sin claves foráneas activas esta inserción pasaría y dejaría una edición
    /// huérfana que nadie volvería a encontrar.
    #[test]
    fn una_edicion_de_una_obra_inexistente_se_rechaza() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let resultado = Edition::create(
            &conn,
            &NewEdition {
                work_id: "0".repeat(32),
                title: "Huérfana".to_owned(),
                ..NewEdition::default()
            },
        );

        assert!(matches!(resultado, Err(NbError::StorageFailure)));
    }

    // --- Fechas parciales ---------------------------------------------------

    /// Muchos catálogos solo conocen el año. Guardar una fecha completa
    /// inventada es peor que guardar la precisión que se tiene.
    #[test]
    fn se_admite_la_fecha_de_publicacion_con_la_precision_que_se_conoce() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");

        for fecha in ["1963", "1963-06", "1963-06-28"] {
            let creada = Edition::create(
                &conn,
                &NewEdition {
                    work_id: w.id.clone(),
                    title: format!("Edición {fecha}"),
                    published_date: Some(fecha.to_owned()),
                    ..NewEdition::default()
                },
            )
            .expect("fecha parcial admitida");
            assert_eq!(creada.published_date.as_deref(), Some(fecha));
        }
    }

    #[test]
    fn una_fecha_de_publicacion_con_forma_invalida_se_rechaza() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");

        for fecha in ["28/06/1963", "1963-6-28", "junio de 1963", "1963-13-01x"] {
            let resultado = Edition::create(
                &conn,
                &NewEdition {
                    work_id: w.id.clone(),
                    title: "Edición".to_owned(),
                    published_date: Some(fecha.to_owned()),
                    ..NewEdition::default()
                },
            );
            assert!(
                matches!(resultado, Err(NbError::StorageFailure)),
                "la fecha {fecha:?} no debería admitirse"
            );
        }
    }

    // --- El innegociable del Locator ---------------------------------------

    /// ADR-004: ninguna posición se guarda como entero. La restricción vive en
    /// la propia base, de modo que no depende de que el código de turno la
    /// recuerde. Si esta prueba falla, el modelo que destruye anotaciones al
    /// cambiar la tipografía ha vuelto a entrar.
    #[test]
    fn una_anotacion_anclada_a_un_entero_se_rechaza_en_la_base() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");

        for objetivo in ["42", "0.75", "\"pagina 42\"", "[1,2]", "no es json"] {
            let resultado = Annotation::create(
                &conn,
                &NewAnnotation {
                    payload: None,
                    edition_id: e.id.clone(),
                    kind: AnnotationKind::Highlight,
                    body: None,
                    target: objetivo.to_owned(),
                },
            );
            assert!(
                matches!(resultado, Err(NbError::StorageFailure)),
                "el objetivo {objetivo:?} no puede admitirse: no es un selector"
            );
        }
    }

    #[test]
    fn una_anotacion_con_selectores_se_admite() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");

        let creada = anotacion(&conn, &e.id, "Nota al margen");

        assert_eq!(creada.kind, AnnotationKind::Note);
        assert_eq!(creada.body.as_deref(), Some("Nota al margen"));
    }

    // --- Archivos -----------------------------------------------------------

    #[test]
    fn no_se_registra_dos_veces_la_misma_ruta() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        archivo(&conn, &e.id, "/biblioteca/libro.epub", 0xAA);

        let (root_id, relative_path) = ubicacion(&conn, "/biblioteca/libro.epub");
        let resultado = File::create(
            &conn,
            &NewFile {
                edition_id: e.id.clone(),
                root_id,
                relative_path,
                content_hash: hash(0xBB),
                byte_size: 10,
                media_type: "application/epub+zip".to_owned(),
                storage_mode: StorageMode::Referenced,
            },
        );

        assert!(matches!(resultado, Err(NbError::StorageFailure)));
    }

    /// El índice por hash es la base sobre la que la importación decide si una
    /// importación es un duplicado o un archivo movido. El esquema no decide esa
    /// política: la hace consultable.
    #[test]
    fn los_archivos_con_el_mismo_contenido_se_encuentran_por_hash() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");

        archivo(&conn, &e.id, "/original/libro.epub", 0xCC);
        archivo(&conn, &e.id, "/copia/libro.epub", 0xCC);

        let encontrados =
            File::find_by_content_hash(&conn, &hash(0xCC)).expect("consulta por hash");

        assert_eq!(encontrados.len(), 2);
    }

    #[test]
    fn el_modo_y_la_disponibilidad_sobreviven_a_la_ida_y_vuelta() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");

        let (root_id, relative_path) = ubicacion(&conn, "/externo/libro.pdf");
        let creado = File::create(
            &conn,
            &NewFile {
                edition_id: e.id.clone(),
                root_id,
                relative_path,
                content_hash: hash(0xDD),
                byte_size: 2048,
                media_type: "application/pdf".to_owned(),
                storage_mode: StorageMode::Referenced,
            },
        )
        .expect("archivo creado");

        assert_eq!(creado.storage_mode, StorageMode::Referenced);
        assert_eq!(creado.availability, Availability::Available);

        File::set_availability(&conn, &creado.id, Availability::Missing).expect("actualización");

        let recuperados = File::list_for_edition(&conn, &e.id).expect("consulta");
        assert_eq!(recuperados[0].availability, Availability::Missing);
        assert_eq!(recuperados[0].storage_mode, StorageMode::Referenced);
    }

    // --- Contribuidores y materias ------------------------------------------

    // --- La clave de orden por autor ------------------------------
    //
    // Una columna desnormalizada que solo se mantiene en el caso feliz es peor
    // que no tenerla: el orden miente y nadie se entera. Por eso hay una prueba
    // por cada disparador de `V19__author_sort.sql`, y una que comprueba que un
    // rol que no es autor **no** la toca.

    fn clave_de_orden(conn: &Connection, edition_id: &str) -> Option<String> {
        conn.query_row(
            "SELECT author_sort FROM editions WHERE id = ?1",
            [edition_id],
            |fila| fila.get(0),
        )
        .expect("clave de orden")
    }

    fn autor(conn: &Connection, nombre: &str, orden: Option<&str>) -> Contributor {
        Contributor::create(
            conn,
            &NewContributor {
                name: nombre.to_owned(),
                sort_name: orden.map(str::to_owned),
                orcid: None,
            },
        )
        .expect("contribuidor")
    }

    #[test]
    fn la_clave_de_orden_sale_de_la_forma_ordenable_del_primer_autor() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let a = autor(&conn, "Ana Autora", Some("Autora, Ana"));

        Contributor::link(&conn, &e.id, &a.id, ContributorRole::Author, 0).expect("enlace");

        assert_eq!(clave_de_orden(&conn, &e.id).as_deref(), Some("Autora, Ana"));
    }

    /// Sin forma ordenable se usa el nombre, que es peor orden pero es un orden.
    /// Dejarla nula mandaría la obra al final de la lista sin decir por qué.
    #[test]
    fn sin_forma_ordenable_la_clave_es_el_nombre() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let a = autor(&conn, "Ana Autora", None);

        Contributor::link(&conn, &e.id, &a.id, ContributorRole::Author, 0).expect("enlace");

        assert_eq!(clave_de_orden(&conn, &e.id).as_deref(), Some("Ana Autora"));
    }

    /// Un autor que entra por delante manda, y uno que entra detrás no.
    #[test]
    fn manda_el_autor_de_posicion_mas_baja_entre_como_entre() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let segunda = autor(&conn, "Bea Segunda", Some("Segunda, Bea"));
        let primera = autor(&conn, "Ana Primera", Some("Primera, Ana"));

        Contributor::link(&conn, &e.id, &segunda.id, ContributorRole::Author, 1).expect("enlace");
        assert_eq!(
            clave_de_orden(&conn, &e.id).as_deref(),
            Some("Segunda, Bea")
        );

        // Entra por delante: pasa a mandar.
        Contributor::link(&conn, &e.id, &primera.id, ContributorRole::Author, 0).expect("enlace");
        assert_eq!(
            clave_de_orden(&conn, &e.id).as_deref(),
            Some("Primera, Ana")
        );

        // Y uno que entra detrás no cambia nada.
        let tercera = autor(&conn, "Carmen Tercera", Some("Tercera, Carmen"));
        Contributor::link(&conn, &e.id, &tercera.id, ContributorRole::Author, 2).expect("enlace");
        assert_eq!(
            clave_de_orden(&conn, &e.id).as_deref(),
            Some("Primera, Ana")
        );
    }

    /// Quitar al principal devuelve la clave al siguiente, y quitarlos a todos la
    /// deja nula en lugar de conservar el nombre de alguien que ya no está.
    #[test]
    fn quitar_al_autor_principal_pasa_la_clave_al_siguiente() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let primera = autor(&conn, "Ana Primera", Some("Primera, Ana"));
        let segunda = autor(&conn, "Bea Segunda", Some("Segunda, Bea"));
        Contributor::link(&conn, &e.id, &primera.id, ContributorRole::Author, 0).expect("enlace");
        Contributor::link(&conn, &e.id, &segunda.id, ContributorRole::Author, 1).expect("enlace");

        conn.execute(
            "DELETE FROM edition_contributors WHERE edition_id = ?1 AND contributor_id = ?2",
            [&e.id, &primera.id],
        )
        .expect("baja");
        assert_eq!(
            clave_de_orden(&conn, &e.id).as_deref(),
            Some("Segunda, Bea")
        );

        conn.execute(
            "DELETE FROM edition_contributors WHERE edition_id = ?1",
            [&e.id],
        )
        .expect("baja");
        assert_eq!(clave_de_orden(&conn, &e.id), None);
    }

    /// Renombrar a una persona reordena sus obras.
    ///
    /// Sin este disparador se quedarían con el nombre viejo hasta que alguien las
    /// reimportara, que es la mentira silenciosa que trae una columna
    /// desnormalizada sin mantener.
    #[test]
    fn renombrar_a_una_persona_reordena_sus_obras() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let a = autor(&conn, "Ana Autora", Some("Autora, Ana"));
        Contributor::link(&conn, &e.id, &a.id, ContributorRole::Author, 0).expect("enlace");

        conn.execute(
            "UPDATE contributors SET sort_name = ?2 WHERE id = ?1",
            [&a.id, &"Casada, Ana".to_owned()],
        )
        .expect("renombrado");

        assert_eq!(clave_de_orden(&conn, &e.id).as_deref(), Some("Casada, Ana"));
    }

    /// Un traductor no cambia el orden por autor.
    ///
    /// Es lo que comprueba que el `WHEN` de los disparadores hace su trabajo: sin
    /// él, cada traductor, editor o ilustrador dispararía un recálculo entero
    /// para no cambiar nada, y eso se paga una vez por documento importado.
    /// Enlazar una autoría no marca la edición como modificada.
    ///
    /// `author_sort` es una copia materializada de algo que vive en
    /// `contributors`: mantenerla no es editar la obra. Y enlazar
    /// un autor no tocaba `editions` para nada, así que esto no decide nada nuevo
    /// sobre qué cuenta como modificar; impide cambiarlo por accidente.
    ///
    /// Importa más allá de la fecha: ADR-006 resuelve conflictos comparando
    /// marcas de tiempo, y exportar y reimportar tiene que devolver una
    /// biblioteca equivalente. Lo avisó la prueba de exportación.
    #[test]
    fn enlazar_una_autoria_no_marca_la_edicion_como_modificada() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let antes: String = conn
            .query_row(
                "SELECT updated_at FROM editions WHERE id = ?1",
                [&e.id],
                |f| f.get(0),
            )
            .expect("fecha");

        let a = autor(&conn, "Ana Autora", Some("Autora, Ana"));
        Contributor::link(&conn, &e.id, &a.id, ContributorRole::Author, 0).expect("enlace");

        let despues: String = conn
            .query_row(
                "SELECT updated_at FROM editions WHERE id = ?1",
                [&e.id],
                |f| f.get(0),
            )
            .expect("fecha");

        assert_eq!(clave_de_orden(&conn, &e.id).as_deref(), Some("Autora, Ana"));
        assert_eq!(
            antes, despues,
            "materializar la clave de orden marcó la edición como editada"
        );
    }

    #[test]
    fn un_traductor_no_toca_la_clave_de_orden() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        let a = autor(&conn, "Ana Autora", Some("Autora, Ana"));
        let t = autor(&conn, "Tomás Traductor", Some("Traductor, Tomás"));

        Contributor::link(&conn, &e.id, &a.id, ContributorRole::Author, 0).expect("enlace");
        Contributor::link(&conn, &e.id, &t.id, ContributorRole::Translator, 0).expect("enlace");

        assert_eq!(clave_de_orden(&conn, &e.id).as_deref(), Some("Autora, Ana"));
    }

    #[test]
    fn los_contribuidores_conservan_su_papel_y_su_orden() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");

        let autora = Contributor::create(
            &conn,
            &NewContributor {
                name: "Ana Autora".to_owned(),
                sort_name: Some("Autora, Ana".to_owned()),
                orcid: None,
            },
        )
        .expect("contribuidora");
        let traductor = Contributor::create(
            &conn,
            &NewContributor {
                name: "Tomás Traductor".to_owned(),
                ..NewContributor::default()
            },
        )
        .expect("contribuidor");

        Contributor::link(&conn, &e.id, &autora.id, ContributorRole::Author, 0).expect("enlace");
        Contributor::link(&conn, &e.id, &traductor.id, ContributorRole::Translator, 0)
            .expect("enlace");

        let listados = Contributor::list_for_edition(&conn, &e.id).expect("consulta");

        assert_eq!(listados.len(), 2);
        assert_eq!(listados[0].1, "author");
        assert_eq!(listados[0].0.name, "Ana Autora");
        assert_eq!(listados[1].1, "translator");
    }

    #[test]
    fn un_orcid_con_forma_invalida_se_rechaza() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let resultado = Contributor::create(
            &conn,
            &NewContributor {
                name: "Quien Sea".to_owned(),
                sort_name: None,
                orcid: Some("1234-5678".to_owned()),
            },
        );

        assert!(matches!(resultado, Err(NbError::StorageFailure)));
    }

    #[test]
    fn dos_personas_no_pueden_compartir_orcid() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let orcid = Some("0000-0002-1825-0097".to_owned());

        Contributor::create(
            &conn,
            &NewContributor {
                name: "Primera".to_owned(),
                sort_name: None,
                orcid: orcid.clone(),
            },
        )
        .expect("primera");

        let resultado = Contributor::create(
            &conn,
            &NewContributor {
                name: "Segunda".to_owned(),
                sort_name: None,
                orcid,
            },
        );

        assert!(matches!(resultado, Err(NbError::StorageFailure)));
    }

    /// La misma materia llega una y otra vez desde documentos distintos: pedirla
    /// dos veces devuelve la misma fila, no una duplicada.
    #[test]
    fn pedir_dos_veces_la_misma_materia_devuelve_la_misma_fila() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let primera = Subject::ensure(&conn, "local", "Literatura hispanoamericana").expect("alta");
        let segunda = Subject::ensure(&conn, "local", "Literatura hispanoamericana").expect("alta");

        assert_eq!(primera, segunda);
    }

    #[test]
    fn una_edicion_admite_varios_identificadores_externos_a_la_vez() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");

        Edition::add_identifier(&conn, &e.id, IdentifierScheme::Isbn, "9788437604947")
            .expect("isbn 13");
        Edition::add_identifier(&conn, &e.id, IdentifierScheme::Isbn, "8437604940")
            .expect("isbn 10");
        Edition::add_identifier(&conn, &e.id, IdentifierScheme::Doi, "10.1000/ejemplo")
            .expect("doi");
        // Repetir el mismo par no duplica: una reimportación del mismo metadato
        // es lo normal, no un error.
        Edition::add_identifier(&conn, &e.id, IdentifierScheme::Isbn, "9788437604947")
            .expect("repetido");

        let identificadores = Edition::identifiers(&conn, &e.id).expect("consulta");

        assert_eq!(identificadores.len(), 3);
    }

    // --- Integridad referencial y marcas de tiempo --------------------------

    /// Borrar una obra no puede dejar ediciones, archivos ni anotaciones
    /// colgando: serían filas que nadie vuelve a encontrar y que la exportación
    /// no recogería.
    #[test]
    fn borrar_una_obra_no_deja_filas_huerfanas() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let w = obra(&conn, "Obra");
        let e = edicion(&conn, &w.id, "Edición");
        archivo(&conn, &e.id, "/biblioteca/libro.epub", 0xEE);
        anotacion(&conn, &e.id, "Nota");

        conn.execute("DELETE FROM works WHERE id = ?1", [&w.id])
            .expect("borrado");

        for tabla in ["editions", "files", "annotations"] {
            let filas: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
                .expect("consulta");
            assert_eq!(filas, 0, "quedaron filas huérfanas en {tabla}");
        }
    }

    /// Una marca de tiempo que el código olvida actualizar es un dato en el que
    /// después no se puede confiar. La mantiene un disparador, no la disciplina.
    #[test]
    fn actualizar_una_fila_mueve_updated_at_y_conserva_created_at() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let creada = obra(&conn, "Antes");

        // La marca tiene precisión de milisegundo: sin esperar, la actualización
        // podría caer en el mismo instante y la prueba no demostraría nada.
        std::thread::sleep(std::time::Duration::from_millis(5));
        conn.execute(
            "UPDATE works SET canonical_title = 'Después' WHERE id = ?1",
            [&creada.id],
        )
        .expect("actualización");

        let actualizada = Work::find(&conn, &creada.id)
            .expect("consulta")
            .expect("existe");

        assert_eq!(actualizada.created_at, creada.created_at);
        assert!(
            actualizada.updated_at > creada.updated_at,
            "updated_at no avanzó: {} -> {}",
            creada.updated_at,
            actualizada.updated_at
        );
    }

    /// Las marcas se guardan en ISO-8601 UTC para que ordenar como cadena sea lo
    /// mismo que ordenar como instante.
    #[test]
    fn las_marcas_de_tiempo_son_iso_8601_en_utc() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let creada = obra(&conn, "Obra");

        assert!(
            creada.created_at.contains('T') && creada.created_at.ends_with('Z'),
            "marca inesperada: {}",
            creada.created_at
        );
        assert_eq!(creada.created_at.len(), 24, "{}", creada.created_at);
    }

    /// La clave de un nombre vive en Rust **y** en V38, y tiene que ser la misma
    /// cadena: la columna generada de la base y la consulta de `ensure` comparan
    /// una con la otra, y si se separaran, `ensure` no encontraría a nadie y la
    /// importación chocaría con el índice único en cada libro.
    #[test]
    fn la_clave_de_nombre_es_la_misma_en_v38_y_aqui() {
        let v38 = include_str!("../../migrations/V38__un_autor_una_fila.sql");
        assert!(
            v38.contains(&format!(
                "GENERATED ALWAYS AS ({}) VIRTUAL",
                clave_de_nombre_sql("name")
            )),
            "V38 y clave_de_nombre_sql tienen que escribir la misma expresión"
        );
    }

    /// Y V39 guarda con la misma expresión la clave del título, que es con la que
    /// se encuentra lo duplicado. Tres veces: al migrar, al crear y al cambiar.
    #[test]
    fn la_clave_del_titulo_de_v39_es_la_misma_de_aqui() {
        let v39 = include_str!("../../migrations/V39__titulo_comparable.sql");
        for columna in ["title", "NEW.title", "NEW.title"] {
            assert!(
                v39.contains(&clave_de_nombre_sql(columna)),
                "V39 no escribe la clave de {columna} como clave_de_nombre_sql"
            );
        }
        assert_eq!(v39.matches(&clave_de_nombre_sql("NEW.title")).count(), 2);
    }

    /// Lo que se decidió: sin distinguir mayúsculas ni
    /// tildes. Y nada más: un nombre distinto sigue siendo otra persona.
    #[test]
    fn el_mismo_nombre_sin_mayusculas_ni_tildes_es_el_mismo_autor() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let id = |nombre: &str| Contributor::ensure(&conn, nombre).expect("autor").id;

        let gabo = id("Gabriel García Márquez");
        assert_eq!(id("GABRIEL GARCIA MARQUEZ"), gabo);
        assert_eq!(id("  gabriel  garcía márquez "), gabo);
        assert_eq!(id("Slavoj Žižek"), id("Slavoj Zizek"));
        assert_eq!(id("ÉMILE ZOLA"), id("Émile Zola"));
        assert_eq!(id("Allan Pease"), id("Allan pease"));
        assert_ne!(id("Gabriel García"), gabo, "otro nombre es otra persona");

        // La forma que se queda es la primera que entró.
        let fila = Contributor::ensure(&conn, "gabriel garcia marquez").expect("autor");
        assert_eq!(fila.name, "Gabriel García Márquez");
    }

    /// El índice único es de la base, no de `ensure`: una segunda fila con el
    /// mismo nombre no entra por ningún camino. Y el ORCID, que es lo único que
    /// distingue a dos homónimos, queda fuera de la regla.
    #[test]
    fn la_base_no_admite_dos_filas_para_el_mismo_autor_salvo_con_orcid() {
        let db = base();
        let conn = db.connection().expect("conexión");
        Contributor::ensure(&conn, "Ana Autora").expect("autora");

        let repetida = Contributor::create(
            &conn,
            &NewContributor {
                name: "ANA AUTORA".to_owned(),
                ..NewContributor::default()
            },
        );
        assert!(
            repetida.is_err(),
            "la base tiene que rechazar la segunda fila"
        );

        let con_orcid = Contributor::create(
            &conn,
            &NewContributor {
                name: "Ana Autora".to_owned(),
                sort_name: None,
                orcid: Some("0000-0002-1825-0097".to_owned()),
            },
        );
        assert!(con_orcid.is_ok(), "un homónimo con ORCID es otra persona");
    }
}
