//! Criterio de aceptación de la. (Traer una biblioteca de Calibre).
//!
//! # Lo que exige el contrato
//!
//! 1. Seleccionar la carpeta raíz de una biblioteca de Calibre la importa **referenciada**
//!    (`StorageMode::Referenced`), leyendo su `metadata.db` en modo de solo lectura:
//!    - Título (`books.title`)
//!    - Autores (`authors`, `books_authors_link`, preservando nombre y `sort_name`)
//!    - Serie y número (`series`, `books_series_link`, `series_index`)
//!    - Etiquetas (`tags`, `books_tags_link` mapeadas a materias con esquema `calibre`)
//!    - Identificadores (`identifiers.type` -> `scheme`: `isbn`, `doi`, `asin`, `local`)
//!    - Portada (`cover.jpg` dentro de la carpeta del libro -> miniatura en caché de portadas)
//!    - Procedencia registrada como `"calibre"` (`Source::Calibre`)
//! 2. Nada se escribe dentro de la carpeta de Calibre (solo lectura estricta).
//! 3. Una biblioteca no legible o corrupta se rechaza limpiamente antes de importar a medias.
//! 4. Reimportar la misma biblioteca es idempotente (no duplica nada).
//! 5. Los metadatos de Calibre tienen precedencia mayor que la extracción interna de archivos,
//!    pero ceden ante la edición manual del usuario.
//!
//! Contrato: `docs/architecture/metadata-model.md`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nb_books_lib::calibre::{import_calibre_library, is_calibre_library, validar_biblioteca};
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::CancelToken;
use nb_books_lib::metadata::{self, Field, Source};

const MINIMAL_JPEG: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x48,
    0x00, 0x48, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08,
    0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0a, 0x0c, 0x14, 0x0d, 0x0c, 0x0b, 0x0b, 0x0c, 0x19, 0x12,
    0x13, 0x0f, 0x14, 0x1d, 0x1a, 0x1f, 0x1e, 0x1d, 0x1a, 0x1c, 0x1c, 0x20, 0x24, 0x2e, 0x27, 0x20,
    0x22, 0x2c, 0x23, 0x1c, 0x1c, 0x28, 0x37, 0x29, 0x2c, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1f, 0x27,
    0x39, 0x3d, 0x38, 0x32, 0x3c, 0x2e, 0x33, 0x34, 0x32, 0xff, 0xc0, 0x00, 0x0b, 0x08, 0x00, 0x01,
    0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xff, 0xc4, 0x00, 0x1f, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01,
    0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04,
    0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f,
    0x00, 0xbf, 0x00, 0xff, 0xd9,
];

struct Escenario {
    dir: PathBuf,
    calibre_dir: PathBuf,
    covers_dir: PathBuf,
    db: Database,
    bus: EventBus,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-calibre-{nombre}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let calibre_dir = dir.join("Biblioteca Calibre");
        let covers_dir = dir.join("cache/portadas");
        fs::create_dir_all(&calibre_dir).expect("crear directorio calibre");
        fs::create_dir_all(&covers_dir).expect("crear directorio portadas");

        let db = Database::open(&dir.join("nb-books.sqlite")).expect("abrir base de datos");
        let bus = EventBus::new();

        Self {
            dir,
            calibre_dir,
            covers_dir,
            db,
            bus,
        }
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        // Restaurar permisos para permitir borrado si se cambiaron a solo lectura
        let _ = fs::set_permissions(&self.calibre_dir, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn crear_biblioteca_calibre_ejemplo(raiz: &Path) {
    let db_path = raiz.join("metadata.db");
    let conn = rusqlite::Connection::open(&db_path).expect("crear metadata.db de Calibre");

    conn.execute_batch(
        "CREATE TABLE books (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL DEFAULT 'Unknown',
            sort TEXT,
            author_sort TEXT,
            timestamp TEXT,
            pubdate TEXT,
            series_index REAL NOT NULL DEFAULT 1.0,
            path TEXT NOT NULL DEFAULT '',
            has_cover INTEGER DEFAULT 0
        );
        CREATE TABLE authors (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sort TEXT,
            link TEXT DEFAULT ''
        );
        CREATE TABLE books_authors_link (
            id INTEGER PRIMARY KEY,
            book INTEGER NOT NULL,
            author INTEGER NOT NULL
        );
        CREATE TABLE series (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            sort TEXT
        );
        CREATE TABLE books_series_link (
            id INTEGER PRIMARY KEY,
            book INTEGER NOT NULL,
            series INTEGER NOT NULL
        );
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE books_tags_link (
            id INTEGER PRIMARY KEY,
            book INTEGER NOT NULL,
            tag INTEGER NOT NULL
        );
        CREATE TABLE identifiers (
            id INTEGER PRIMARY KEY,
            book INTEGER NOT NULL,
            type TEXT NOT NULL DEFAULT 'isbn',
            val TEXT NOT NULL
        );
        CREATE TABLE data (
            id INTEGER PRIMARY KEY,
            book INTEGER NOT NULL,
            format TEXT NOT NULL,
            uncompressed_size INTEGER NOT NULL,
            name TEXT NOT NULL
        );
        CREATE TABLE comments (
            id INTEGER PRIMARY KEY,
            book INTEGER NOT NULL,
            text TEXT NOT NULL
        );",
    )
    .expect("esquema calibre");

    // Insertar libro
    conn.execute(
        "INSERT INTO books (id, title, sort, author_sort, pubdate, series_index, path, has_cover)
         VALUES (1, 'El Hobbit', 'Hobbit, El', 'Tolkien, J. R. R.', '1937-09-21 00:00:00+00:00', 1.0, 'J. R. R. Tolkien/El Hobbit (1)', 1)",
        [],
    )
    .expect("insertar libro");

    // Autor
    conn.execute(
        "INSERT INTO authors (id, name, sort) VALUES (1, 'J. R. R. Tolkien', 'Tolkien, J. R. R.')",
        [],
    )
    .expect("insertar autor");
    conn.execute(
        "INSERT INTO books_authors_link (book, author) VALUES (1, 1)",
        [],
    )
    .expect("insertar vinculo autor");

    // Serie
    conn.execute(
        "INSERT INTO series (id, name, sort) VALUES (1, 'Tierra Media', 'Tierra Media')",
        [],
    )
    .expect("insertar serie");
    conn.execute(
        "INSERT INTO books_series_link (book, series) VALUES (1, 1)",
        [],
    )
    .expect("insertar vinculo serie");

    // Etiquetas (Tags)
    conn.execute("INSERT INTO tags (id, name) VALUES (1, 'Fantasía')", [])
        .expect("tag 1");
    conn.execute("INSERT INTO tags (id, name) VALUES (2, 'Aventura')", [])
        .expect("tag 2");
    conn.execute(
        "INSERT INTO books_tags_link (book, tag) VALUES (1, 1), (1, 2)",
        [],
    )
    .expect("tags link");

    // Identificadores
    conn.execute(
        "INSERT INTO identifiers (book, type, val) VALUES
         (1, 'isbn', '9780261102217'),
         (1, 'doi', '10.1000/182'),
         (1, 'asin', 'B000FC0SJS'),
         (1, 'calibre_id', '42')",
        [],
    )
    .expect("identifiers");

    // Formato de archivo (data)
    conn.execute(
        "INSERT INTO data (book, format, uncompressed_size, name)
         VALUES (1, 'EPUB', 12345, 'El Hobbit - J. R. R. Tolkien')",
        [],
    )
    .expect("data");

    // Comentarios
    conn.execute(
        "INSERT INTO comments (book, text) VALUES (1, '<p>En un agujero en el suelo vivía un hobbit.</p>')",
        [],
    )
    .expect("comments");

    drop(conn);

    // Crear el archivo EPUB y la portada en el disco
    let book_dir = raiz.join("J. R. R. Tolkien/El Hobbit (1)");
    fs::create_dir_all(&book_dir).expect("crear subcarpeta de libro");

    let epub_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimo-valido.epub");
    let epub_dst = book_dir.join("El Hobbit - J. R. R. Tolkien.epub");
    fs::copy(&epub_src, &epub_dst).expect("copiar epub fixture");

    let cover_dst = book_dir.join("cover.jpg");
    fs::write(&cover_dst, MINIMAL_JPEG).expect("escribir portada");
}

#[test]
fn validar_biblioteca_rechaza_rutas_invalidas_o_sin_metadata() {
    let tmp = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-val-calibre-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).expect("crear tmp");

    // 1. Ruta que no existe
    let no_existe = tmp.join("inexistente");
    assert!(!is_calibre_library(&no_existe));
    assert!(validar_biblioteca(&no_existe).is_err());

    // 2. Ruta que es un archivo
    let archivo = tmp.join("archivo.txt");
    fs::write(&archivo, "hola").expect("escribir");
    assert!(!is_calibre_library(&archivo));
    assert!(validar_biblioteca(&archivo).is_err());

    // 3. Directorio vacío sin metadata.db
    let vacio = tmp.join("vacio");
    fs::create_dir_all(&vacio).expect("vacio");
    assert!(!is_calibre_library(&vacio));
    assert!(validar_biblioteca(&vacio).is_err());

    // 4. metadata.db corrupto (archivo plano de texto)
    let corrupto = tmp.join("corrupto");
    fs::create_dir_all(&corrupto).expect("corrupto");
    fs::write(corrupto.join("metadata.db"), "no es sqlite").expect("escribir corrupto");
    assert!(is_calibre_library(&corrupto));
    assert!(validar_biblioteca(&corrupto).is_err());

    // 5. metadata.db válido pero sin tablas requeridas de Calibre
    let sin_tablas = tmp.join("sin_tablas");
    fs::create_dir_all(&sin_tablas).expect("sin tablas");
    let conn = rusqlite::Connection::open(sin_tablas.join("metadata.db")).expect("abrir");
    conn.execute_batch("CREATE TABLE otra_cosa (id INT);")
        .expect("crear");
    drop(conn);
    assert!(is_calibre_library(&sin_tablas));
    assert!(validar_biblioteca(&sin_tablas).is_err());

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn importar_biblioteca_calibre_completa() {
    let escenario = Escenario::nuevo("completa");
    crear_biblioteca_calibre_ejemplo(&escenario.calibre_dir);

    assert!(is_calibre_library(&escenario.calibre_dir));
    validar_biblioteca(&escenario.calibre_dir).expect("debe ser válida");

    let cancel = CancelToken::new();
    let mut conn = escenario.db.connection().expect("conexión");
    let informe = import_calibre_library(
        &mut conn,
        &escenario.calibre_dir,
        &escenario.covers_dir,
        &escenario.bus,
        &cancel,
    )
    .expect("importación exitosa");

    assert_eq!(informe.imported, 1);
    assert_eq!(informe.already_in_library, 0);
    assert!(informe.rejected.is_empty());

    // 1. Raíz de biblioteca registrada en modo referenciado
    let (root_id, kind): (String, String) = conn
        .query_row(
            "SELECT id, kind FROM library_roots WHERE path = ?1",
            [escenario.calibre_dir.to_string_lossy()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("raíz de biblioteca");
    assert_eq!(kind, "referenced");

    // 2. Archivo en modo referenciado con ruta relativa
    let (_file_id, edition_id, storage_mode, rel_path): (String, String, String, String) = conn
        .query_row(
            "SELECT id, edition_id, storage_mode, relative_path FROM files WHERE root_id = ?1",
            [&root_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("archivo importado");
    assert_eq!(storage_mode, "referenced");
    assert_eq!(
        rel_path,
        "J. R. R. Tolkien/El Hobbit (1)/El Hobbit - J. R. R. Tolkien.epub"
    );

    // 3. Edición con metadatos de Calibre
    struct DatosEdicion {
        work_id: String,
        title: String,
        series_title: Option<String>,
        series_index: Option<f64>,
        author_sort: Option<String>,
        pubdate: Option<String>,
        cover_path: Option<String>,
    }

    let ed = conn
        .query_row(
            "SELECT work_id, title, series_title, series_index, author_sort, published_date, cover_path
             FROM editions WHERE id = ?1",
            [&edition_id],
            |row| {
                Ok(DatosEdicion {
                    work_id: row.get(0)?,
                    title: row.get(1)?,
                    series_title: row.get(2)?,
                    series_index: row.get(3)?,
                    author_sort: row.get(4)?,
                    pubdate: row.get(5)?,
                    cover_path: row.get(6)?,
                })
            },
        )
        .expect("edición");

    assert_eq!(ed.title, "El Hobbit");
    assert_eq!(ed.series_title.as_deref(), Some("Tierra Media"));
    assert_eq!(ed.series_index, Some(1.0));
    assert_eq!(ed.author_sort.as_deref(), Some("Tolkien, J. R. R."));
    assert_eq!(ed.pubdate.as_deref(), Some("1937-09-21"));
    assert!(ed.cover_path.is_some());

    // 4. Portada guardada en la caché de portadas de NB Books
    let cp = ed.cover_path.expect("ruta de portada");
    assert!(escenario.covers_dir.join(&cp).exists());

    // 5. Obra con título canónico
    let work_title: String = conn
        .query_row(
            "SELECT canonical_title FROM works WHERE id = ?1",
            [&ed.work_id],
            |row| row.get(0),
        )
        .expect("obra");
    assert_eq!(work_title, "El Hobbit");

    // 6. Autor con forma de presentación y ordenación
    let (autor_name, autor_sort): (String, Option<String>) = conn
        .query_row(
            "SELECT c.name, c.sort_name
             FROM contributors c
             JOIN edition_contributors ec ON ec.contributor_id = c.id
             WHERE ec.edition_id = ?1",
            [&edition_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("autor");
    assert_eq!(autor_name, "J. R. R. Tolkien");
    assert_eq!(autor_sort.as_deref(), Some("Tolkien, J. R. R."));

    // 7. Materias / Etiquetas con esquema 'calibre'
    let etiquetas: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT s.label
                 FROM subjects s
                 JOIN edition_subjects es ON es.subject_id = s.id
                 WHERE es.edition_id = ?1 AND s.scheme = 'calibre'
                 ORDER BY s.label ASC",
            )
            .expect("prepare materias");
        let filas = stmt
            .query_map([&edition_id], |row| row.get(0))
            .expect("query materias");
        filas.collect::<Result<Vec<_>, _>>().expect("leer materias")
    };
    assert_eq!(
        etiquetas,
        vec!["Aventura".to_string(), "Fantasía".to_string()]
    );

    // 8. Identificadores mapeados
    let mut stmt = conn
        .prepare(
            "SELECT scheme, value FROM edition_identifiers WHERE edition_id = ?1 ORDER BY scheme ASC",
        )
        .expect("prepare ids");
    let ids: Vec<(String, String)> = stmt
        .query_map([&edition_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query ids")
        .collect::<Result<Vec<_>, _>>()
        .expect("leer ids");

    assert!(ids.contains(&("asin".to_string(), "B000FC0SJS".to_string())));
    assert!(ids.contains(&("doi".to_string(), "10.1000/182".to_string())));
    assert!(ids.contains(&("isbn".to_string(), "9780261102217".to_string())));
    assert!(ids.contains(&("local".to_string(), "42".to_string())));

    // 9. Procedencia registrada como 'calibre'
    let mut stmt_prov = conn
        .prepare(
            "SELECT entity_type, field, source, value
             FROM metadata_provenance
             WHERE source = 'calibre'
             ORDER BY entity_type, field",
        )
        .expect("prepare provenance");
    let provs: Vec<(String, String, String, Option<String>)> = stmt_prov
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query provenance")
        .collect::<Result<Vec<_>, _>>()
        .expect("leer provenance");

    assert!(provs
        .iter()
        .any(|(t, f, s, _)| t == "work" && f == "canonical_title" && s == "calibre"));
    assert!(provs
        .iter()
        .any(|(t, f, s, _)| t == "edition" && f == "title" && s == "calibre"));
    assert!(provs
        .iter()
        .any(|(t, f, s, _)| t == "edition" && f == "series_title" && s == "calibre"));
    assert!(provs
        .iter()
        .any(|(t, f, s, _)| t == "edition" && f == "published_date" && s == "calibre"));
    assert!(provs
        .iter()
        .any(|(t, f, s, _)| t == "contributor" && f == "name" && s == "calibre"));
    assert!(provs
        .iter()
        .any(|(t, f, s, _)| t == "contributor" && f == "sort_name" && s == "calibre"));
}

#[test]
fn importar_biblioteca_calibre_es_de_solo_lectura_y_no_toca_el_directorio() {
    let escenario = Escenario::nuevo("solo-lectura");
    crear_biblioteca_calibre_ejemplo(&escenario.calibre_dir);

    // 1. Registrar mtimes y tamaños antes de importar
    let metadata_db = escenario.calibre_dir.join("metadata.db");
    let epub_file = escenario
        .calibre_dir
        .join("J. R. R. Tolkien/El Hobbit (1)/El Hobbit - J. R. R. Tolkien.epub");
    let cover_file = escenario
        .calibre_dir
        .join("J. R. R. Tolkien/El Hobbit (1)/cover.jpg");

    let mtime_db_antes = fs::metadata(&metadata_db)
        .expect("meta db")
        .modified()
        .expect("mtime");
    let mtime_epub_antes = fs::metadata(&epub_file)
        .expect("meta epub")
        .modified()
        .expect("mtime");
    let mtime_cover_antes = fs::metadata(&cover_file)
        .expect("meta cover")
        .modified()
        .expect("mtime");

    // 2. Establecer permisos de solo lectura (0o555 en carpetas, 0o444 en archivos)
    fs::set_permissions(&metadata_db, fs::Permissions::from_mode(0o444)).expect("chmod db");
    fs::set_permissions(&epub_file, fs::Permissions::from_mode(0o444)).expect("chmod epub");
    fs::set_permissions(&cover_file, fs::Permissions::from_mode(0o444)).expect("chmod cover");
    fs::set_permissions(
        escenario.calibre_dir.join("J. R. R. Tolkien/El Hobbit (1)"),
        fs::Permissions::from_mode(0o555),
    )
    .expect("chmod subfolder");
    fs::set_permissions(
        escenario.calibre_dir.join("J. R. R. Tolkien"),
        fs::Permissions::from_mode(0o555),
    )
    .expect("chmod author folder");
    fs::set_permissions(&escenario.calibre_dir, fs::Permissions::from_mode(0o555))
        .expect("chmod root");

    let cancel = CancelToken::new();
    let mut conn = escenario.db.connection().expect("conexión");
    let informe = import_calibre_library(
        &mut conn,
        &escenario.calibre_dir,
        &escenario.covers_dir,
        &escenario.bus,
        &cancel,
    )
    .expect("la importación en solo lectura debe tener éxito sin escribir nada");

    assert_eq!(informe.imported, 1);
    assert!(informe.rejected.is_empty());

    // 3. Restaurar permisos para revisar mtimes
    let _ = fs::set_permissions(&escenario.calibre_dir, fs::Permissions::from_mode(0o755));

    let mtime_db_despues = fs::metadata(&metadata_db)
        .expect("meta db")
        .modified()
        .expect("mtime");
    let mtime_epub_despues = fs::metadata(&epub_file)
        .expect("meta epub")
        .modified()
        .expect("mtime");
    let mtime_cover_despues = fs::metadata(&cover_file)
        .expect("meta cover")
        .modified()
        .expect("mtime");

    assert_eq!(
        mtime_db_antes, mtime_db_despues,
        "metadata.db no debe haber sido tocado"
    );
    assert_eq!(
        mtime_epub_antes, mtime_epub_despues,
        "epub no debe haber sido tocado"
    );
    assert_eq!(
        mtime_cover_antes, mtime_cover_despues,
        "cover no debe haber sido tocada"
    );

    // Comprobar que no hay archivos temporales ni residuos de wal/shm en la carpeta
    assert!(!escenario.calibre_dir.join("metadata.db-wal").exists());
    assert!(!escenario.calibre_dir.join("metadata.db-shm").exists());
}

#[test]
fn importar_biblioteca_calibre_es_idempotente() {
    let escenario = Escenario::nuevo("idempotente");
    crear_biblioteca_calibre_ejemplo(&escenario.calibre_dir);

    let cancel = CancelToken::new();
    let mut conn = escenario.db.connection().expect("conexión");

    // Primera pasada: importa 1
    let primer_informe = import_calibre_library(
        &mut conn,
        &escenario.calibre_dir,
        &escenario.covers_dir,
        &escenario.bus,
        &cancel,
    )
    .expect("primera importación");
    assert_eq!(primer_informe.imported, 1);
    assert_eq!(primer_informe.already_in_library, 0);

    // Segunda pasada: detecta que ya está por content_hash y lo salta
    let segundo_informe = import_calibre_library(
        &mut conn,
        &escenario.calibre_dir,
        &escenario.covers_dir,
        &escenario.bus,
        &cancel,
    )
    .expect("segunda importación");
    assert_eq!(segundo_informe.imported, 0);
    assert_eq!(segundo_informe.already_in_library, 1);
    assert!(segundo_informe.rejected.is_empty());

    // Verificar en base de datos que solo existe 1 obra, 1 edición y 1 archivo
    let obras: i64 = conn
        .query_row("SELECT count(*) FROM works", [], |r| r.get(0))
        .unwrap();
    let ediciones: i64 = conn
        .query_row("SELECT count(*) FROM editions", [], |r| r.get(0))
        .unwrap();
    let archivos: i64 = conn
        .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .unwrap();
    let autores: i64 = conn
        .query_row("SELECT count(*) FROM contributors", [], |r| r.get(0))
        .unwrap();

    assert_eq!(obras, 1);
    assert_eq!(ediciones, 1);
    assert_eq!(archivos, 1);
    assert_eq!(autores, 1);
}

#[test]
fn metadato_calibre_gana_sobre_archivo_y_cede_ante_manual() {
    let escenario = Escenario::nuevo("precedencia");
    crear_biblioteca_calibre_ejemplo(&escenario.calibre_dir);

    let cancel = CancelToken::new();
    let mut conn = escenario.db.connection().expect("conexión");
    import_calibre_library(
        &mut conn,
        &escenario.calibre_dir,
        &escenario.covers_dir,
        &escenario.bus,
        &cancel,
    )
    .expect("importar");

    let (edition_id, _): (String, String) = conn
        .query_row("SELECT id, title FROM editions LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .expect("edición");

    // Intentar aplicar metadatos de archivo (inferior a Calibre).
    // Con precedencia: File < Online < Calibre < Manual.
    // metadata::propose con Source::File debe registrar propuesta y no pisar el valor.
    let outcome = metadata::propose(
        &conn,
        &edition_id,
        Field::Title,
        Source::File,
        Some("Título del Archivo"),
    )
    .expect("proponer desde archivo");

    match outcome {
        metadata::Outcome::Suggested(s) => {
            assert_eq!(s.proposed_value.as_deref(), Some("Título del Archivo"));
            assert_eq!(s.source, Source::File);
        }
        _ => panic!("debe ser una propuesta porque Calibre tiene mayor precedencia que File"),
    }

    // Verificar que el título de la edición sigue siendo el de Calibre
    let titulo_actual: String = conn
        .query_row(
            "SELECT title FROM editions WHERE id = ?1",
            [&edition_id],
            |r| r.get(0),
        )
        .expect("leer título");
    assert_eq!(titulo_actual, "El Hobbit");

    // Ahora probar que la edición manual sí sobrescribe a Calibre
    let manual_outcome = metadata::set_manual(
        &conn,
        &edition_id,
        Field::Title,
        Some("El Hobbit (Edición Anotada)"),
    )
    .expect("editar manual");
    assert_eq!(manual_outcome, metadata::Outcome::Applied);

    let titulo_manual: String = conn
        .query_row(
            "SELECT title FROM editions WHERE id = ?1",
            [&edition_id],
            |r| r.get(0),
        )
        .expect("leer título modificado");
    assert_eq!(titulo_manual, "El Hobbit (Edición Anotada)");
}
