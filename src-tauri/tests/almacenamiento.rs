//! Criterio de aceptación de la.
//!
//! "Desconectar el volumen de un archivo referenciado deja el documento visible
//! en estado no disponible, conservando metadatos y anotaciones."
//!
//! Es el caso más cotidiano de todos: alguien desenchufa un disco externo. Si la
//! respuesta a eso fuera borrar el documento del catálogo, se perderían las
//! anotaciones de años por desconectar un cable. La respuesta correcta es la
//! degradación elegante de la regla 1 de la Constitución: el documento sigue
//! ahí, marcado, con todo lo suyo.
//!
//! Contrato: `docs/architecture/storage.md`, sección 2.

use std::path::{Path, PathBuf};

use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, Availability, Edition, File, NewAnnotation, StorageMode, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{
    import_document, refresh_all_availability, refresh_availability, ImportContext, ImportOutcome,
    ImportRequest,
};
use nb_books_lib::metadata::{self, Field};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    db: Database,
    bus: EventBus,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-almac-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        std::fs::create_dir_all(&scope).expect("ámbito");
        std::fs::create_dir_all(&library).expect("biblioteca");

        Self {
            dir,
            scope,
            library,
            db: Database::open_in_memory().expect("base"),
            bus: EventBus::new(),
        }
    }

    /// Ruta absoluta de un archivo. Ahora la ubicación son dos datos:
    /// su raíz y su ruta dentro de ella.
    fn ruta(&self, conn: &rusqlite::Connection, file: &nb_books_lib::db::catalog::File) -> PathBuf {
        nb_books_lib::library::resolve_path(conn, file)
            .expect("consulta de la raíz")
            .expect("raíz localizada")
    }

    /// Simula un volumen externo montado dentro del ámbito autorizado.
    fn volumen(&self, nombre: &str) -> PathBuf {
        let punto = self.scope.join(nombre);
        std::fs::create_dir_all(&punto).expect("punto de montaje");
        punto
    }

    fn importar(&self, ruta: &Path, modo: StorageMode) -> ImportOutcome {
        let mut conn = self.db.connection().expect("conexión");
        import_document(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: ruta.to_path_buf(),
                storage_mode: modo,
                root: None,
            },
        )
        .expect("importado")
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// **El criterio de aceptación.**
#[test]
fn desconectar_el_volumen_deja_el_documento_visible_y_no_disponible() {
    let esc = Escenario::nuevo("desconectado");

    // Un documento en un disco externo, referenciado: no se copia nada.
    let volumen = esc.volumen("disco-externo");
    let ruta = volumen.join("libro.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &ruta).expect("copia al volumen");

    let ImportOutcome::Imported(importado) = esc.importar(&ruta, StorageMode::Referenced) else {
        panic!("debía importarse");
    };

    // El usuario le pone título propio y le deja una anotación: es su trabajo, y
    // es lo que no puede perderse.
    let conn = esc.db.connection().expect("conexión");
    metadata::set_manual(
        &conn,
        &importado.edition.id,
        Field::Title,
        Some("El libro que tengo en el disco de fuera"),
    )
    .expect("título propio");
    let anotacion = Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: importado.edition.id.clone(),
            kind: AnnotationKind::Note,
            body: Some("Lo importante está en el capítulo tres".to_owned()),
            target: r#"{"version":1,"href":"c3.xhtml","mediaType":"application/xhtml+xml",
                        "locations":{"progression":0.4}}"#
                .to_owned(),
        },
    )
    .expect("anotación");

    // Se desconecta el disco.
    std::fs::remove_dir_all(&volumen).expect("desconectar el volumen");

    let estado = refresh_availability(&conn, &importado.file.id).expect("revisión");
    assert_eq!(estado, Availability::Missing);

    // --- El documento sigue visible, con todo lo suyo ---
    assert!(
        Work::find(&conn, &importado.work.id)
            .expect("consulta")
            .is_some(),
        "la obra no puede desaparecer porque se desconecte un disco"
    );

    let ediciones = Edition::list_for_work(&conn, &importado.work.id).expect("ediciones");
    assert_eq!(ediciones.len(), 1);
    assert_eq!(
        ediciones[0].title,
        "El libro que tengo en el disco de fuera"
    );

    let anotaciones = Annotation::list_for_edition(&conn, &importado.edition.id).expect("notas");
    assert_eq!(anotaciones.len(), 1);
    assert_eq!(anotaciones[0].id, anotacion.id);
    assert_eq!(
        anotaciones[0].body.as_deref(),
        Some("Lo importante está en el capítulo tres")
    );

    // Y el archivo sigue registrado, marcado, con su ruta y su hash: sin ellos
    // no habría forma de reconocerlo cuando vuelva.
    let archivos = File::list_for_edition(&conn, &importado.edition.id).expect("archivos");
    assert_eq!(archivos.len(), 1);
    assert_eq!(archivos[0].availability, Availability::Missing);
    assert_eq!(archivos[0].content_hash, importado.file.content_hash);
    assert_eq!(archivos[0].storage_mode, StorageMode::Referenced);
}

/// Y cuando el disco vuelve, el documento vuelve. Un estado que solo sabe
/// empeorar obligaría al usuario a reimportar su biblioteca cada vez.
#[test]
fn volver_a_conectar_el_volumen_devuelve_el_documento_a_disponible() {
    let esc = Escenario::nuevo("reconectado");
    let volumen = esc.volumen("disco-externo");
    let ruta = volumen.join("libro.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &ruta).expect("copia");

    let ImportOutcome::Imported(importado) = esc.importar(&ruta, StorageMode::Referenced) else {
        panic!("debía importarse");
    };
    let conn = esc.db.connection().expect("conexión");

    std::fs::remove_dir_all(&volumen).expect("desconectar");
    assert_eq!(
        refresh_availability(&conn, &importado.file.id).expect("revisión"),
        Availability::Missing
    );

    // Se vuelve a conectar.
    std::fs::create_dir_all(&volumen).expect("reconectar");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &ruta).expect("el archivo vuelve");

    assert_eq!(
        refresh_availability(&conn, &importado.file.id).expect("revisión"),
        Availability::Available
    );
}

/// Existir no es lo mismo que poder leerse. Un montaje sin permisos o un soporte
/// con un sector dañado existen y no se dejan abrir, y la interfaz tiene que
/// poder decir cuál de las dos cosas pasa.
#[cfg(unix)]
#[test]
fn un_archivo_que_existe_pero_no_se_puede_leer_se_distingue_del_que_falta() {
    use std::os::unix::fs::PermissionsExt;

    let esc = Escenario::nuevo("ilegible");
    let ruta = esc.scope.join("libro.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &ruta).expect("copia");

    let ImportOutcome::Imported(importado) = esc.importar(&ruta, StorageMode::Referenced) else {
        panic!("debía importarse");
    };
    let conn = esc.db.connection().expect("conexión");

    std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o000)).expect("sin permisos");
    let estado = refresh_availability(&conn, &importado.file.id);
    let _ = std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o600));

    assert_eq!(estado, Ok(Availability::Unreadable));
}

/// El modo administrado copia; el referenciado, no. Es la diferencia entre
/// duplicar ciento cincuenta gigabytes y no duplicarlos.
#[test]
fn el_modo_administrado_copia_y_el_referenciado_conserva_la_ruta() {
    let esc = Escenario::nuevo("modos");

    let uno = esc.scope.join("administrado.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &uno).expect("copia");
    let ImportOutcome::Imported(administrado) = esc.importar(&uno, StorageMode::Managed) else {
        panic!("debía importarse");
    };

    let dos = esc.scope.join("referenciado.pdf");
    std::fs::copy(fixtures().join("minimo-valido.pdf"), &dos).expect("copia");
    let ImportOutcome::Imported(referenciado) = esc.importar(&dos, StorageMode::Referenced) else {
        panic!("debía importarse");
    };

    let conn = esc.db.connection().expect("conexión");
    let ruta_administrado = esc.ruta(&conn, &administrado.file);
    let ruta_referenciado = esc.ruta(&conn, &referenciado.file);

    // El administrado vive dentro de la biblioteca, con su identidad por nombre.
    assert!(ruta_administrado.starts_with(&esc.library));
    assert!(
        administrado
            .file
            .relative_path
            .contains(&administrado.file.content_hash.as_str()[7..]),
        "el nombre en la biblioteca administrada es la identidad del contenido"
    );

    // El referenciado se quedó donde estaba.
    assert_eq!(
        ruta_referenciado.canonicalize().expect("canónica"),
        dos.canonicalize().expect("canónica")
    );
    assert!(!ruta_referenciado.starts_with(&esc.library));

    // Y en la biblioteca solo hay una copia: la del administrado.
    assert_eq!(
        std::fs::read_dir(&esc.library).expect("biblioteca").count(),
        1
    );
}

/// Un administrado que falta no es lo mismo que un referenciado que falta. Del
/// primero respondemos nosotros, así que su ausencia significa que algo lo borró
/// por detrás y merece contarse aparte.
#[test]
fn la_revision_distingue_lo_que_falta_de_la_biblioteca_administrada() {
    let esc = Escenario::nuevo("informe");

    let uno = esc.scope.join("administrado.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &uno).expect("copia");
    let ImportOutcome::Imported(administrado) = esc.importar(&uno, StorageMode::Managed) else {
        panic!("debía importarse");
    };

    let volumen = esc.volumen("disco");
    let dos = volumen.join("referenciado.pdf");
    std::fs::copy(fixtures().join("minimo-valido.pdf"), &dos).expect("copia");
    esc.importar(&dos, StorageMode::Referenced);

    let mut conn = esc.db.connection().expect("conexión");

    // Todo en su sitio.
    let informe = refresh_all_availability(&mut conn).expect("revisión");
    assert_eq!(informe.checked, 2);
    assert_eq!(informe.available, 2);
    assert_eq!(informe.missing, 0);

    // Se desconecta el disco y, además, algo borra la copia administrada.
    std::fs::remove_dir_all(&volumen).expect("desconectar");
    std::fs::remove_file(esc.ruta(&conn, &administrado.file)).expect("borrado por detrás");

    let informe = refresh_all_availability(&mut conn).expect("revisión");
    assert_eq!(informe.checked, 2);
    assert_eq!(informe.missing, 2);
    assert_eq!(
        informe.managed_missing, 1,
        "un administrado que falta se cuenta aparte"
    );
}

#[test]
fn revisar_un_archivo_que_no_existe_en_el_catalogo_es_un_error_y_no_un_silencio() {
    let esc = Escenario::nuevo("inexistente");
    let conn = esc.db.connection().expect("conexión");

    assert_eq!(
        refresh_availability(&conn, &"0".repeat(32)),
        Err(NbError::DocumentNotFound)
    );
}
