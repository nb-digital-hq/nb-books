//! Criterio de aceptación del borrado de datos.
//!
//! "Ajustes ofrece borrar el historial de lectura, las anotaciones y notas o la
//! biblioteca entera; cada acción dice antes qué se borra y qué no —los archivos
//! del usuario no se borran nunca—, pide confirmación y ejecuta en una sola
//! transacción inmediata. Una prueba por alcance."

use std::path::PathBuf;

use nb_books_lib::boards;
use nb_books_lib::collections::{Collection, CollectionOrigin};
use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, ContentHash, Contributor, ContributorRole, Edition, File,
    NewAnnotation, NewContributor, NewEdition, NewFile, NewWork, StorageMode, Subject, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::notes;
use nb_books_lib::portability;
use nb_books_lib::reader::progress;
use nb_books_lib::reader::{Locations, Locator, Precision};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-borrado-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn base(&self) -> PathBuf {
        self.0.join("biblioteca.sqlite")
    }

    fn libro_en_disco(&self) -> PathBuf {
        let carpeta = self.0.join("mis-libros");
        std::fs::create_dir_all(&carpeta).expect("carpeta de libros");
        let ruta = carpeta.join("pedro-paramo.epub");
        std::fs::write(&ruta, b"contenido del libro del usuario").expect("archivo en disco");
        ruta
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn contar(conn: &rusqlite::Connection, tabla: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {tabla}"), [], |f| f.get(0))
        .expect("cuenta")
}

/// Puebla una biblioteca con catálogo, archivo referenciado real en el disco,
/// anotaciones (con sucesión para ejercitar `supersedes`), notas enlazadas,
/// tablero, colección personal, progreso, sesión de lectura y preferencias.
fn poblar(db: &Database, ruta_libro: &std::path::Path) {
    let mut conn = db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Pedro Páramo".to_owned(),
            original_language: Some("es".to_owned()),
        },
    )
    .expect("obra");

    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Pedro Páramo".to_owned(),
            language: Some("es".to_owned()),
            published_date: Some("1955".to_owned()),
            ..NewEdition::default()
        },
    )
    .expect("edición");

    let autor = Contributor::create(
        &conn,
        &NewContributor {
            name: "Juan Rulfo".to_owned(),
            sort_name: Some("Rulfo, Juan".to_owned()),
            orcid: None,
        },
    )
    .expect("autor");
    Contributor::link(&conn, &edicion.id, &autor.id, ContributorRole::Author, 0).expect("autoría");

    let materia = Subject::ensure(&conn, "local", "Novela mexicana").expect("materia");
    Subject::link(&conn, &edicion.id, &materia.id).expect("materia enlazada");

    let (raiz_id, relativa) =
        nb_books_lib::library::ensure_root(&conn, ruta_libro, StorageMode::Referenced, None)
            .expect("raíz referenciada");

    File::create(
        &conn,
        &NewFile {
            edition_id: edicion.id.clone(),
            root_id: raiz_id,
            relative_path: relativa,
            content_hash: ContentHash::from_sha256(&[0x33; 32]),
            byte_size: 31,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )
    .expect("archivo");

    let primera = Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.id.clone(),
            kind: AnnotationKind::Note,
            body: Some("Vine a Comala".to_owned()),
            target: r#"{"version":1,"href":"OEBPS/c1.xhtml","mediaType":"application/xhtml+xml","locations":{"progression":0.05}}"#
                .to_owned(),
        },
    )
    .expect("primera anotación");
    Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.id.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: r#"{"version":1,"href":"OEBPS/c1.xhtml","mediaType":"application/xhtml+xml","locations":{"progression":0.10}}"#
                .to_owned(),
        },
    )
    .expect("segunda anotación");

    let nota_a = notes::create(&conn, "Comala", "Los murmullos en [[Susana]].").expect("nota A");
    let _nota_b = notes::create(&conn, "Susana", "El recuerdo.").expect("nota B");
    conn.execute(
        "INSERT INTO note_annotation_links (note_id, annotation_id) VALUES (?1, ?2)",
        rusqlite::params![nota_a.id, primera.id],
    )
    .expect("enlace nota-anotación");

    boards::crear(&mut conn, "Lectura de Rulfo", &["Ideas".to_owned()]).expect("tablero");

    let coleccion = Collection::create_manual(&conn, "Imprescindibles", CollectionOrigin::Personal)
        .expect("colección");
    Collection::add(&conn, &coleccion.id, &edicion.id).expect("miembro");

    let donde = Locator::new(
        "OEBPS/c1.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: Vec::new(),
            progression: Some(0.35),
            total_progression: Some(0.35),
            position: Some(12),
        },
        None,
    )
    .expect("locator");
    progress::save(
        &conn,
        &edicion.id,
        progress::PERFIL,
        &donde,
        Precision::Quote,
    )
    .expect("progreso");
    let sesion = progress::open_session(&conn, &edicion.id, &donde).expect("sesión");
    progress::close_session(&conn, &sesion, &donde).expect("cierre");

    settings::set(
        &conn,
        SettingKey::Theme,
        &SettingValue::Text("tinta".to_owned()),
    )
    .expect("tema");
}

#[test]
fn borrar_el_historial_de_lectura_conserva_el_catalogo_las_notas_y_los_archivos() {
    let tmp = Temporal::nuevo("historial");
    let libro = tmp.libro_en_disco();
    let db = Database::open(&tmp.base()).expect("base");
    poblar(&db, &libro);

    let mut conn = db.connection().expect("conexión");
    assert_eq!(contar(&conn, "progress"), 1);
    assert_eq!(contar(&conn, "reading_sessions"), 1);

    portability::clear_reading_history(&mut conn).expect("borra el historial");

    assert_eq!(contar(&conn, "progress"), 0, "el progreso se borra");
    assert_eq!(
        contar(&conn, "reading_sessions"),
        0,
        "las sesiones de lectura se borran"
    );
    assert_eq!(contar(&conn, "works"), 1, "el catálogo se conserva");
    assert_eq!(contar(&conn, "editions"), 1, "las ediciones se conservan");
    assert_eq!(
        contar(&conn, "files"),
        1,
        "los archivos registrados se conservan"
    );
    assert_eq!(
        contar(&conn, "annotations"),
        2,
        "las anotaciones se conservan"
    );
    assert_eq!(contar(&conn, "notes"), 2, "las notas se conservan");
    assert_eq!(contar(&conn, "note_boards"), 1, "los tableros se conservan");
    assert_eq!(
        settings::get(&conn, SettingKey::Theme).expect("ajuste"),
        SettingValue::Text("tinta".to_owned()),
        "los ajustes se conservan"
    );
    assert!(
        libro.exists(),
        "el archivo del usuario en el disco no se toca nunca"
    );
}

#[test]
fn borrar_las_anotaciones_y_notas_conserva_el_catalogo_el_historial_y_los_archivos() {
    let tmp = Temporal::nuevo("anotaciones");
    let libro = tmp.libro_en_disco();
    let db = Database::open(&tmp.base()).expect("base");
    poblar(&db, &libro);

    let mut conn = db.connection().expect("conexión");
    assert_eq!(contar(&conn, "annotations"), 2);
    assert_eq!(contar(&conn, "notes"), 2);
    assert_eq!(contar(&conn, "note_links"), 1);
    assert_eq!(contar(&conn, "note_annotation_links"), 1);
    assert_eq!(contar(&conn, "note_boards"), 1);

    portability::clear_annotations_and_notes(&mut conn).expect("borra anotaciones y notas");

    assert_eq!(contar(&conn, "annotations"), 0, "las anotaciones se borran");
    assert_eq!(contar(&conn, "notes"), 0, "las notas se borran");
    assert_eq!(
        contar(&conn, "note_links"),
        0,
        "los enlaces entre notas se borran"
    );
    assert_eq!(
        contar(&conn, "note_annotation_links"),
        0,
        "los enlaces con anotaciones se borran"
    );
    assert_eq!(contar(&conn, "note_boards"), 0, "los tableros se borran");
    assert_eq!(
        contar(&conn, "note_board_columns"),
        0,
        "las columnas de los tableros se borran"
    );
    assert_eq!(contar(&conn, "works"), 1, "el catálogo se conserva");
    assert_eq!(
        contar(&conn, "progress"),
        1,
        "el progreso de lectura se conserva"
    );
    assert_eq!(
        contar(&conn, "reading_sessions"),
        1,
        "el historial de lectura se conserva"
    );
    assert!(
        libro.exists(),
        "el archivo del usuario en el disco no se toca nunca"
    );
}

#[test]
fn borrar_la_biblioteca_entera_vacia_la_base_conserva_ajustes_y_no_toca_el_disco() {
    let tmp = Temporal::nuevo("biblioteca");
    let libro = tmp.libro_en_disco();
    let db = Database::open(&tmp.base()).expect("base");
    poblar(&db, &libro);

    let mut conn = db.connection().expect("conexión");
    let respaldo = portability::export(&conn).expect("exporta antes de vaciar");

    portability::clear_library(&mut conn).expect("vacía la biblioteca");

    assert_eq!(contar(&conn, "works"), 0, "sin obras");
    assert_eq!(contar(&conn, "editions"), 0, "sin ediciones");
    assert_eq!(contar(&conn, "files"), 0, "sin archivos");
    assert_eq!(contar(&conn, "contributors"), 0, "sin autores");
    assert_eq!(contar(&conn, "subjects"), 0, "sin materias");
    assert_eq!(contar(&conn, "annotations"), 0, "sin anotaciones");
    assert_eq!(contar(&conn, "notes"), 0, "sin notas");
    assert_eq!(contar(&conn, "note_boards"), 0, "sin tableros");
    assert_eq!(contar(&conn, "progress"), 0, "sin progreso");
    assert_eq!(contar(&conn, "reading_sessions"), 0, "sin sesiones");
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM collections WHERE origin = 'personal'",
            [],
            |f| f.get::<_, i64>(0),
        )
        .expect("colecciones personales"),
        0,
        "sin colecciones personales"
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM library_roots WHERE kind = 'managed'",
            [],
            |f| f.get::<_, i64>(0),
        )
        .expect("raíz gestionada"),
        1,
        "la raíz gestionada vacía sigue existiendo"
    );
    assert_eq!(
        settings::get(&conn, SettingKey::Theme).expect("ajuste"),
        SettingValue::Text("tinta".to_owned()),
        "los ajustes de la aplicación se conservan"
    );
    assert!(
        libro.exists(),
        "el archivo del usuario en el disco no se borra nunca"
    );

    // Y una biblioteca vaciada desde la aplicación admite restaurar una exportación.
    portability::import(&mut conn, &respaldo).expect("restaura sobre la biblioteca vaciada");
    assert_eq!(contar(&conn, "works"), 1, "la obra vuelve al restaurar");
}
