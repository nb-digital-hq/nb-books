//! Importación de bibliotecas de Calibre.
//!
//! # Por qué existe
//!
//! Quien viene de Calibre ha invertido horas o años corrigiendo metadatos:
//! títulos bien escritos, autores con su orden de apellidos, series con su
//! número exacto, etiquetas y portadas limpias. Importar esos libros como
//! archivos sueltos obligaría a rehacer ese trabajo desde cero.
//!
//! Calibre guarda esa información en una base de datos SQLite llamada
//! `metadata.db` en la raíz de su biblioteca. Este módulo lee esa base en **modo
//! estricto de solo lectura**, extrae la ficha bibliográfica completa con la
//! procedencia `Calibre`, y referencia los archivos originales donde están, sin
//! escribir jamás un solo byte dentro de la carpeta de Calibre.
//!
//! # Contratos y garantías
//!
//! 1. **Solo lectura garantizada:** `metadata.db` se abre con
//!    `SQLITE_OPEN_READ_ONLY` y se activa `PRAGMA query_only = ON` y
//!    `PRAGMA temp_store = MEMORY`. Ningún archivo de registro (WAL) ni temporal
//!    se crea en la carpeta de Calibre.
//! 2. **Modo referenciado:** Los libros no se copian a la biblioteca administrada.
//!    Se crea o reutiliza una raíz de biblioteca (`LibraryRoot`) referenciada.
//! 3. **Procedencia protegida:** Los metadatos importados llevan `Source::Calibre`,
//!    de modo que la extracción interna de metadatos de los archivos (`Source::File`)
//!    no pisa las correcciones de Calibre.
//! 4. **Idempotencia:** Reimportar la misma biblioteca no duplica obras, ediciones
//!    ni archivos; reutiliza los existentes gracias a la identidad por contenido.
//! 5. **Atomicidad:** Si `metadata.db` no se puede leer o está dañada, la operación
//!    falla explícitamente sin registrar nada a medias. Cada libro se registra en
//!    su propia transacción atómica.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::db::catalog::{
    Contributor, ContributorRole, Edition, IdentifierScheme, LibraryRoot, NewEdition, NewFile,
    NewWork, StorageMode, Subject, Work,
};
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};
use crate::library::{identify, sniff, BulkReport, CancelToken, Format, Known, Rejected};
use crate::metadata::{self, Field, Source};

/// Comprueba si un directorio parece ser una biblioteca de Calibre.
#[must_use]
pub fn is_calibre_library(dir: &Path) -> bool {
    dir.is_dir() && dir.join("metadata.db").is_file()
}

/// Valida que la biblioteca de Calibre sea legible antes de empezar la importación.
///
/// Abre `metadata.db` en solo lectura y verifica la existencia de las tablas
/// esenciales. Si no se puede abrir o está corrupta, devuelve un error claro.
///
/// # Errors
///
/// - [`NbError::FileNotFound`] si la ruta o `metadata.db` no existen.
/// - [`NbError::DocumentCorrupted`] si el archivo SQLite está dañado o faltan tablas.
/// - [`NbError::StorageFailure`] si no se puede abrir por permisos o error del sistema.
pub fn validar_biblioteca(calibre_path: &Path) -> NbResult<()> {
    let db_path = calibre_path.join("metadata.db");
    if !db_path.is_file() {
        return Err(NbError::FileNotFound);
    }

    let conn = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        tracing::warn!(
            code = e.to_string(),
            "no se pudo abrir metadata.db de Calibre"
        );
        match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::CannotOpen =>
            {
                NbError::StorageFailure
            }
            _ => NbError::DocumentCorrupted,
        }
    })?;

    conn.execute_batch(
        "PRAGMA query_only = ON;
         PRAGMA temp_store = MEMORY;",
    )
    .map_err(|_| NbError::DocumentCorrupted)?;

    let ok: bool = conn
        .query_row("SELECT count(*) >= 0 FROM books", [], |r| r.get(0))
        .map_err(|_| NbError::DocumentCorrupted)?;

    if !ok {
        return Err(NbError::DocumentCorrupted);
    }

    Ok(())
}

/// Registro básico de un libro leído de `metadata.db` de Calibre.
struct CalibreBook {
    id: i64,
    title: String,
    path: String,
    series_index: f64,
    pubdate: Option<String>,
    author_sort: Option<String>,
}

/// Autor de Calibre con su orden de enlace.
struct CalibreAuthor {
    name: String,
    sort: Option<String>,
}

/// Archivo de formato en Calibre.
struct CalibreData {
    format: String,
    name: String,
}

/// Importa una biblioteca de Calibre completa en modo referenciado.
///
/// # Errors
///
/// Devuelve error si la biblioteca no se puede validar o si ocurre un fallo fatal
/// de base de datos.
pub fn import_calibre_library(
    conn: &mut Connection,
    calibre_path: &Path,
    covers_dir: &Path,
    bus: &EventBus,
    cancel: &CancelToken,
) -> NbResult<BulkReport> {
    // 1. Validación previa estricta. Una biblioteca rota no empieza a importarse.
    validar_biblioteca(calibre_path)?;

    // 2. Apuntar la ejecución de importación para histórico y seguimiento.
    let ejecucion = match crate::imports::abrir(
        conn,
        &calibre_path.to_string_lossy(),
        StorageMode::Referenced,
    ) {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(
                code = error.code(),
                "la importación de Calibre no se pudo registrar"
            );
            None
        }
    };

    // 3. Abrir la base de Calibre en solo lectura estricta.
    let calibre_db = Connection::open_with_flags(
        calibre_path.join("metadata.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| NbError::DocumentCorrupted)?;

    calibre_db
        .execute_batch(
            "PRAGMA query_only = ON;
             PRAGMA temp_store = MEMORY;",
        )
        .map_err(|_| NbError::DocumentCorrupted)?;

    // 4. Asegurar la raíz referenciada en el catálogo.
    let carpeta_str = calibre_path.to_string_lossy().replace('\\', "/");
    let root = match LibraryRoot::find_by_path(conn, &carpeta_str)? {
        Some(r) => r,
        None => {
            let nombre = calibre_path
                .file_name()
                .map_or_else(|| carpeta_str.clone(), |n| n.to_string_lossy().into_owned());
            LibraryRoot::create_referenced(conn, &nombre, &carpeta_str)?
        }
    };

    // 5. Cargar la lista de libros de Calibre.
    let mut stmt = calibre_db
        .prepare(
            "SELECT b.id, b.title, b.path, b.series_index, b.pubdate, b.author_sort
               FROM books b
              ORDER BY b.id",
        )
        .map_err(|_| NbError::DocumentCorrupted)?;

    let libros_iter = stmt
        .query_map([], |row| {
            Ok(CalibreBook {
                id: row.get(0)?,
                title: row.get(1)?,
                path: row.get(2)?,
                series_index: row.get::<_, Option<f64>>(3)?.unwrap_or(1.0),
                pubdate: row.get(4)?,
                author_sort: row.get(5)?,
            })
        })
        .map_err(|_| NbError::DocumentCorrupted)?;

    let libros: Vec<CalibreBook> = libros_iter
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| NbError::DocumentCorrupted)?;

    let total = libros.len();
    let mut informe = BulkReport::default();

    emitir_avance(bus, &informe, total);

    for book in libros {
        if cancel.is_cancelled() {
            informe.cancelled = true;
            break;
        }

        informe.processed += 1;

        // Buscar formatos disponibles en la tabla `data`.
        let formatos = match obtener_datos_libro(&calibre_db, book.id) {
            Ok(f) => f,
            Err(_) => {
                informe.rejected.push(Rejected {
                    path: calibre_path.join(&book.path).to_string_lossy().into_owned(),
                    code: NbError::DocumentCorrupted.code(),
                });
                continue;
            }
        };

        // Encontrar un archivo admitido por NB Books en disco.
        let mut archivo_encontrado: Option<(PathBuf, Format)> = None;
        for data in &formatos {
            if let Some(ruta) = resolver_archivo_en_disco(calibre_path, &book.path, data) {
                if let Ok(formato) = sniff(&ruta) {
                    archivo_encontrado = Some((ruta, formato));
                    break;
                }
            }
        }

        let Some((archivo_ruta, formato)) = archivo_encontrado else {
            // Libro sin archivo soportado en disco: se anota rechazo
            let carpeta_libro = calibre_path.join(&book.path);
            if let Some(id) = &ejecucion {
                let rel = carpeta_libro
                    .strip_prefix(calibre_path)
                    .unwrap_or(&carpeta_libro);
                let _ = crate::imports::anotar_rechazo(
                    conn,
                    id,
                    &rel.to_string_lossy(),
                    NbError::FormatUnsupported.code(),
                );
            }
            informe.rejected.push(Rejected {
                path: carpeta_libro.to_string_lossy().into_owned(),
                code: NbError::FormatUnsupported.code(),
            });
            continue;
        };

        // Identidad por contenido: detección de ya importados y copias.
        let identidad = match identify(conn, &archivo_ruta) {
            Ok(id) => id,
            Err(err) => {
                informe.rejected.push(Rejected {
                    path: archivo_ruta.to_string_lossy().into_owned(),
                    code: err.code(),
                });
                continue;
            }
        };

        match &identidad.known {
            Known::AlreadyRegistered(_) => {
                informe.already_in_library += 1;
                continue;
            }
            Known::Copy(original) => {
                informe.duplicates += 1;
                if let Some(id) = &ejecucion {
                    let rel = archivo_ruta
                        .strip_prefix(calibre_path)
                        .unwrap_or(&archivo_ruta);
                    let _ = crate::imports::anotar_copia(
                        conn,
                        id,
                        &rel.to_string_lossy(),
                        &original.id,
                    );
                }
                continue;
            }
            Known::Moved(movido) => {
                let movido = movido.clone();
                let _ = crate::library::relocate_within(
                    conn,
                    &movido.id,
                    &archivo_ruta,
                    Some(calibre_path),
                );
                informe.already_in_library += 1;
                continue;
            }
            Known::New => {}
        }

        // Leer datos complementarios de Calibre para este libro.
        let autores = leer_autores_libro(&calibre_db, book.id).unwrap_or_default();
        let serie = leer_serie_libro(&calibre_db, book.id).ok().flatten();
        let etiquetas = leer_etiquetas_libro(&calibre_db, book.id).unwrap_or_default();
        let identificadores = leer_identificadores_libro(&calibre_db, book.id).unwrap_or_default();

        // Ruta relativa respecto a la raíz de Calibre.
        let ruta_relativa = archivo_ruta
            .strip_prefix(calibre_path)
            .unwrap_or(&archivo_ruta)
            .to_string_lossy()
            .replace('\\', "/");

        // Transacción de escritura para el libro. Todo o nada para cada obra.
        let tx = match crate::db::write_transaction(conn) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(error = %e, "no se pudo abrir transacción para importar libro de Calibre");
                informe.rejected.push(Rejected {
                    path: archivo_ruta.to_string_lossy().into_owned(),
                    code: e.code(),
                });
                continue;
            }
        };

        let resultado = registrar_libro_calibre(
            &tx,
            &book,
            &root.id,
            &ruta_relativa,
            &identidad,
            formato,
            &autores,
            serie.as_deref(),
            &etiquetas,
            &identificadores,
            calibre_path,
            covers_dir,
        );

        match resultado {
            Ok((edition_id, file_id)) => {
                if let Err(e) = tx.commit() {
                    tracing::error!(error = %e, "no se pudo confirmar libro de Calibre");
                    informe.rejected.push(Rejected {
                        path: archivo_ruta.to_string_lossy().into_owned(),
                        code: NbError::StorageFailure.code(),
                    });
                    continue;
                }

                bus.emit(DomainEvent::DocumentImported {
                    edition_id,
                    file_id,
                    media_type: formato.media_type().to_owned(),
                });

                informe.imported += 1;
            }
            Err(err) => {
                // Al salir tx sin commit, se revierte automáticamente
                informe.rejected.push(Rejected {
                    path: archivo_ruta.to_string_lossy().into_owned(),
                    code: err.code(),
                });
            }
        }

        if informe.processed % 25 == 0 {
            emitir_avance(bus, &informe, total);
        }
    }

    // 6. Cerrar el informe de ejecución.
    if let Some(id) = &ejecucion {
        let resumen = crate::imports::Resumen {
            procesados: u32::try_from(informe.processed).unwrap_or(u32::MAX),
            importados: u32::try_from(informe.imported).unwrap_or(u32::MAX),
            ya_estaban: u32::try_from(informe.already_in_library).unwrap_or(u32::MAX),
            rechazados: u32::try_from(informe.rejected.len()).unwrap_or(u32::MAX),
            copias: u32::try_from(informe.duplicates).unwrap_or(u32::MAX),
            cancelada: informe.cancelled,
        };
        let _ = crate::imports::cerrar(conn, id, resumen);
    }

    bus.emit(DomainEvent::BulkImportFinished {
        processed: u32::try_from(informe.processed).unwrap_or(u32::MAX),
        imported: u32::try_from(informe.imported).unwrap_or(u32::MAX),
        skipped: u32::try_from(informe.already_in_library).unwrap_or(u32::MAX),
        rejected: u32::try_from(informe.rejected.len()).unwrap_or(u32::MAX),
        duplicates: u32::try_from(informe.duplicates).unwrap_or(u32::MAX),
        cancelled: informe.cancelled,
    });

    tracing::info!(
        procesados = informe.processed,
        importados = informe.imported,
        ya_estaban = informe.already_in_library,
        rechazados = informe.rejected.len(),
        cancelada = informe.cancelled,
        "importación de biblioteca de Calibre finalizada"
    );

    Ok(informe)
}

fn emitir_avance(bus: &EventBus, informe: &BulkReport, total: usize) {
    bus.emit(DomainEvent::BulkImportProgress {
        processed: u32::try_from(informe.processed).unwrap_or(u32::MAX),
        imported: u32::try_from(informe.imported).unwrap_or(u32::MAX),
        skipped: u32::try_from(informe.already_in_library).unwrap_or(u32::MAX),
        rejected: u32::try_from(informe.rejected.len()).unwrap_or(u32::MAX),
        duplicates: u32::try_from(informe.duplicates).unwrap_or(u32::MAX),
        total: u32::try_from(total).unwrap_or(u32::MAX),
    });
}

fn obtener_datos_libro(conn: &Connection, book_id: i64) -> rusqlite::Result<Vec<CalibreData>> {
    let mut stmt = conn.prepare("SELECT format, name FROM data WHERE book = ?1")?;
    let filas = stmt.query_map([book_id], |row| {
        Ok(CalibreData {
            format: row.get(0)?,
            name: row.get(1)?,
        })
    })?;
    filas.collect()
}

fn resolver_archivo_en_disco(
    calibre_path: &Path,
    book_path: &str,
    data: &CalibreData,
) -> Option<PathBuf> {
    let dir = calibre_path.join(book_path);
    // 1. Probar nombre exacto con formato en minúsculas (lo habitual en Calibre)
    let min = dir.join(format!("{}.{}", data.name, data.format.to_lowercase()));
    if min.is_file() {
        return Some(min);
    }
    // 2. Probar con formato en mayúsculas
    let may = dir.join(format!("{}.{}", data.name, data.format));
    if may.is_file() {
        return Some(may);
    }
    None
}

fn leer_autores_libro(conn: &Connection, book_id: i64) -> rusqlite::Result<Vec<CalibreAuthor>> {
    let mut stmt = conn.prepare(
        "SELECT a.name, a.sort
           FROM books_authors_link l
           JOIN authors a ON a.id = l.author
          WHERE l.book = ?1
          ORDER BY l.id ASC",
    )?;
    let filas = stmt.query_map([book_id], |row| {
        Ok(CalibreAuthor {
            name: row.get(0)?,
            sort: row.get(1)?,
        })
    })?;
    filas.collect()
}

fn leer_serie_libro(conn: &Connection, book_id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT s.name
           FROM books_series_link l
           JOIN series s ON s.id = l.series
          WHERE l.book = ?1
          LIMIT 1",
        [book_id],
        |row| row.get(0),
    )
    .optional()
}

fn leer_etiquetas_libro(conn: &Connection, book_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name
           FROM books_tags_link l
           JOIN tags t ON t.id = l.tag
          WHERE l.book = ?1
          ORDER BY l.id ASC",
    )?;
    let filas = stmt.query_map([book_id], |row| row.get(0))?;
    filas.collect()
}

fn leer_identificadores_libro(
    conn: &Connection,
    book_id: i64,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT type, val FROM identifiers WHERE book = ?1")?;
    let filas = stmt.query_map([book_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    filas.collect()
}

fn mapear_esquema_identificador(tipo: &str) -> IdentifierScheme {
    match tipo.to_ascii_lowercase().as_str() {
        "isbn" => IdentifierScheme::Isbn,
        "issn" => IdentifierScheme::Issn,
        "doi" => IdentifierScheme::Doi,
        "asin" | "amazon" | "mobi-asin" => IdentifierScheme::Asin,
        "oclc" => IdentifierScheme::Oclc,
        _ => IdentifierScheme::Local,
    }
}

fn normalizar_fecha(fecha: Option<&str>) -> Option<&str> {
    let texto = fecha?.trim();
    for largo in [10, 7, 4] {
        if texto.len() >= largo {
            let recorte = &texto[..largo];
            if recorte.chars().all(|c| c.is_ascii_digit() || c == '-') {
                return Some(recorte);
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn registrar_libro_calibre(
    tx: &Connection,
    book: &CalibreBook,
    root_id: &str,
    ruta_relativa: &str,
    identidad: &crate::library::FileIdentity,
    formato: Format,
    autores: &[CalibreAuthor],
    serie: Option<&str>,
    etiquetas: &[String],
    identificadores: &[(String, String)],
    calibre_path: &Path,
    covers_dir: &Path,
) -> NbResult<(String, String)> {
    // 1. Obra
    let work = Work::create(
        tx,
        &NewWork {
            canonical_title: book.title.clone(),
            original_language: None,
        },
    )?;

    // 2. Edición
    let fecha_pub = normalizar_fecha(book.pubdate.as_deref());
    let edition = Edition::create(
        tx,
        &NewEdition {
            work_id: work.id.clone(),
            title: book.title.clone(),
            series_title: serie.map(ToOwned::to_owned),
            series_index: if serie.is_some() {
                Some(book.series_index)
            } else {
                None
            },
            published_date: fecha_pub.map(ToOwned::to_owned),
            ..NewEdition::default()
        },
    )?;

    // Apuntar author_sort en la edición si Calibre lo proporciona
    if let Some(ref asort) = book.author_sort {
        if !asort.trim().is_empty() {
            tx.execute(
                "UPDATE editions SET author_sort = ?1 WHERE id = ?2",
                rusqlite::params![asort.trim(), edition.id],
            )
            .map_err(|_| NbError::StorageFailure)?;
        }
    }

    // 3. Archivo
    let file = File::create(
        tx,
        &NewFile {
            edition_id: edition.id.clone(),
            root_id: root_id.to_owned(),
            relative_path: ruta_relativa.to_owned(),
            content_hash: identidad.hash.clone(),
            byte_size: i64::try_from(identidad.byte_size).unwrap_or(i64::MAX),
            media_type: formato.media_type().to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )?;

    // 4. Procedencia del título
    metadata::record_origin(
        tx,
        &work.id,
        Field::CanonicalTitle,
        Source::Calibre,
        Some(&book.title),
    )?;

    metadata::record_origin(
        tx,
        &edition.id,
        Field::Title,
        Source::Calibre,
        Some(&book.title),
    )?;

    if let Some(fpub) = fecha_pub {
        metadata::record_origin(
            tx,
            &edition.id,
            Field::PublishedDate,
            Source::Calibre,
            Some(fpub),
        )?;
    }

    // Procedencia de la serie
    if let Some(s) = serie {
        metadata::record_origin(
            tx,
            &edition.id,
            Field::SeriesTitle,
            Source::Calibre,
            Some(s),
        )?;
    }

    // 5. Autores
    for (pos, autor) in autores.iter().enumerate() {
        let contribuidor = Contributor::ensure(tx, &autor.name)?;

        if let Some(ref sort) = autor.sort {
            if !sort.trim().is_empty() {
                let _ = tx.execute(
                    "UPDATE contributors SET sort_name = ?1 WHERE id = ?2 AND sort_name IS NULL",
                    rusqlite::params![sort.trim(), contribuidor.id],
                );
                let _ = metadata::record_origin(
                    tx,
                    &contribuidor.id,
                    Field::SortName,
                    Source::Calibre,
                    Some(sort.trim()),
                );
            }
        }

        let _ = Contributor::link(
            tx,
            &edition.id,
            &contribuidor.id,
            ContributorRole::Author,
            pos as i64,
        );

        let _ = metadata::record_origin(
            tx,
            &contribuidor.id,
            Field::Name,
            Source::Calibre,
            Some(&autor.name),
        );
    }

    // 6. Etiquetas (Subjects con esquema "calibre")
    for etiqueta in etiquetas {
        let trimmed = etiqueta.trim();
        if !trimmed.is_empty() {
            if let Ok(materia) = Subject::ensure(tx, "calibre", trimmed) {
                let _ = Subject::link(tx, &edition.id, &materia.id);
            }
        }
    }

    // 7. Identificadores
    for (tipo, valor) in identificadores {
        let trimmed_val = valor.trim();
        if !trimmed_val.is_empty() {
            let esquema = mapear_esquema_identificador(tipo);
            let _ = Edition::add_identifier(tx, &edition.id, esquema, trimmed_val);
        }
    }

    // 8. Portada: leer cover.jpg de Calibre (solo lectura) y guardar miniatura en caché
    let cover_path = calibre_path.join(&book.path).join("cover.jpg");
    if cover_path.is_file() {
        if let Ok(bytes) = crate::paths::read_bytes_noatime(&cover_path) {
            let nombre = format!("{}.jpg", edition.id);
            if crate::covers::guardar(covers_dir, &nombre, &bytes).is_ok() {
                let _ = tx.execute(
                    "UPDATE editions SET cover_path = ?1 WHERE id = ?2",
                    rusqlite::params![nombre, edition.id],
                );
            }
        }
    }

    Ok((edition.id, file.id))
}

use crate::db::catalog::File;
