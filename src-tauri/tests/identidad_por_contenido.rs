//! Criterio de aceptación de la.
//!
//! "Importar dos veces el mismo archivo no crea dos registros; mover un archivo
//! referenciado y volver a importarlo lo reconoce como el mismo."
//!
//! Los dos casos son cotidianos y los dos duelen: el primero llena la biblioteca
//! de duplicados, y el segundo, si se resuelve mal, crea un registro nuevo y
//! deja las anotaciones colgando del viejo. Que es perder el trabajo del
//! usuario sin borrar nada.
//!
//! Contrato: `docs/architecture/metadata-model.md`, sección 4.

use std::path::{Path, PathBuf};

use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, Availability, Edition, File, NewAnnotation, NewEdition, NewFile,
    NewWork, StorageMode, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::library::{hash_file, identify, relocate, Known};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas().join(format!(
            "nb-identidad-{nombre}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn escribir(&self, nombre: &str, contenido: &[u8]) -> PathBuf {
        let ruta = self.0.join(nombre);
        std::fs::write(&ruta, contenido).expect("archivo de prueba");
        ruta
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Contexto mínimo: una obra con una edición a la que colgar archivos.
fn edicion(db: &Database) -> String {
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
            title: "Edición".to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición")
    .id
}

fn registrar(conn: &rusqlite::Connection, edicion: &str, ruta: &Path) -> File {
    let (hash, tamano) = hash_file(ruta).expect("identidad");
    let (root_id, relative_path) =
        nb_books_lib::library::ensure_root(conn, ruta, StorageMode::Referenced, None)
            .expect("raíz");
    File::create(
        conn,
        &NewFile {
            edition_id: edicion.to_owned(),
            root_id,
            relative_path,
            content_hash: hash,
            byte_size: tamano as i64,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )
    .expect("archivo registrado")
}

fn cuantos_archivos(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .expect("consulta")
}

/// **Primera mitad del criterio.** Importar dos veces el mismo archivo no crea
/// dos registros.
#[test]
fn importar_dos_veces_el_mismo_archivo_no_crea_dos_registros() {
    let tmp = Temporal::nuevo("duplicado");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let ruta = tmp.escribir("libro.epub", b"contenido del libro");

    // Primera vez: nadie lo conoce.
    let primera = identify(&conn, &ruta).expect("identidad");
    assert_eq!(primera.known, Known::New);
    assert_eq!(primera.byte_size, 19);
    let registrado = registrar(&conn, &edicion, &ruta);

    // Segunda vez: la biblioteca lo reconoce y quien importa sabe que no debe
    // crear nada.
    let segunda = identify(&conn, &ruta).expect("identidad");
    let Known::AlreadyRegistered(ya) = segunda.known else {
        panic!("el mismo archivo en la misma ruta debe reconocerse: {segunda:?}");
    };
    assert_eq!(ya.id, registrado.id);
    assert_eq!(segunda.hash, primera.hash);
    assert_eq!(cuantos_archivos(&conn), 1);
}

/// **Segunda mitad del criterio.** Mover un archivo referenciado y volver a
/// importarlo lo reconoce como el mismo, y reconciliarlo conserva lo que colgaba
/// de él.
#[test]
fn mover_un_archivo_y_reimportarlo_lo_reconoce_como_el_mismo() {
    let tmp = Temporal::nuevo("movido");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let original = tmp.escribir("libro.epub", b"contenido del libro");
    let registrado = registrar(&conn, &edicion, &original);

    // Una anotación del usuario, que es lo que de verdad hay que no perder.
    let anotacion = Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Note,
            body: Some("Lo importante de este libro".to_owned()),
            target: r#"{"type":"TextQuoteSelector","exact":"contenido"}"#.to_owned(),
        },
    )
    .expect("anotación");

    // El usuario reorganiza sus carpetas.
    let destino = tmp.0.join("subcarpeta");
    std::fs::create_dir_all(&destino).expect("subcarpeta");
    let nueva = destino.join("libro.epub");
    std::fs::rename(&original, &nueva).expect("mover");

    // Se vuelve a importar desde la ruta nueva.
    let identidad = identify(&conn, &nueva).expect("identidad");
    let Known::Moved(mismo) = identidad.known else {
        panic!("un archivo movido debe reconocerse como el mismo: {identidad:?}");
    };
    assert_eq!(mismo.id, registrado.id);

    relocate(&conn, &mismo.id, &nueva).expect("reubicado");

    // Ni un registro nuevo, ni una anotación perdida.
    assert_eq!(cuantos_archivos(&conn), 1);
    let archivos = File::list_for_edition(&conn, &edicion).expect("archivos");
    assert_eq!(
        nb_books_lib::library::resolve_path(&conn, &archivos[0])
            .expect("consulta de la raíz")
            .expect("raíz localizada"),
        nueva
    );
    assert_eq!(archivos[0].availability, Availability::Available);

    let anotaciones = Annotation::list_for_edition(&conn, &edicion).expect("anotaciones");
    assert_eq!(anotaciones.len(), 1);
    assert_eq!(anotaciones[0].id, anotacion.id);
}

/// La distinción que evita el falso positivo: si el archivo original **sigue
/// ahí**, no es una mudanza, son dos copias. Reubicar aquí borraría del registro
/// una copia que existe.
#[test]
fn dos_copias_del_mismo_contenido_no_se_confunden_con_una_mudanza() {
    let tmp = Temporal::nuevo("copia");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let original = tmp.escribir("libro.epub", b"contenido del libro");
    registrar(&conn, &edicion, &original);

    let copia = tmp.escribir("libro-copia.epub", b"contenido del libro");
    let identidad = identify(&conn, &copia).expect("identidad");

    assert!(
        matches!(identidad.known, Known::Copy(_)),
        "con el original todavía en su sitio no es una mudanza: {identidad:?}"
    );
}

/// La ruta se compara canonicalizada: `./libro.epub` y su ruta absoluta son el
/// mismo archivo, y tratarlos como distintos duplicaría el registro.
#[test]
fn la_misma_ruta_escrita_de_otra_forma_no_duplica() {
    let tmp = Temporal::nuevo("canonica");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let ruta = tmp.escribir("libro.epub", b"contenido");
    registrar(&conn, &edicion, &ruta);

    let rodeada = tmp.0.join("subdir").join("..").join("libro.epub");
    std::fs::create_dir_all(tmp.0.join("subdir")).expect("subdir");

    let identidad = identify(&conn, &rodeada).expect("identidad");
    assert!(
        matches!(identidad.known, Known::AlreadyRegistered(_)),
        "la misma ruta por otro camino no puede parecer otro archivo: {identidad:?}"
    );
}

/// Contenido distinto, mismo nombre: no es el mismo archivo aunque lo parezca.
#[test]
fn el_mismo_nombre_con_otro_contenido_es_otro_archivo() {
    let tmp = Temporal::nuevo("homonimo");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let primera = tmp.escribir("libro.epub", b"primera edicion");
    registrar(&conn, &edicion, &primera);

    let otra_carpeta = tmp.0.join("otra");
    std::fs::create_dir_all(&otra_carpeta).expect("carpeta");
    let segunda = otra_carpeta.join("libro.epub");
    std::fs::write(&segunda, b"segunda edicion revisada").expect("archivo");

    let identidad = identify(&conn, &segunda).expect("identidad");
    assert_eq!(identidad.known, Known::New);
}

/// Un archivo vacío tiene identidad, y es siempre la misma. No es un caso
/// teórico: una descarga interrumpida deja archivos de cero bytes, y todos
/// colisionarían entre sí si no se distinguieran por otra vía.
#[test]
fn un_archivo_vacio_tiene_identidad_y_es_siempre_la_misma() {
    let tmp = Temporal::nuevo("vacio");
    let a = tmp.escribir("a.epub", b"");
    let b = tmp.escribir("b.epub", b"");

    let (ha, ta) = hash_file(&a).expect("identidad");
    let (hb, tb) = hash_file(&b).expect("identidad");

    assert_eq!(ha, hb);
    assert_eq!(ta, 0);
    assert_eq!(tb, 0);
    assert_eq!(
        ha.as_str(),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
