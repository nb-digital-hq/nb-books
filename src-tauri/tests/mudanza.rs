//! Criterio de aceptación de la mudanza de la biblioteca.
//!
//! "Exportar una biblioteca con archivos referenciados, restaurarla sobre un
//! árbol con otra ruta base, comprobar que todo aparece como no localizado,
//! reapuntar la raíz **una vez** y comprobar que **todos** los documentos
//! vuelven a estar disponibles sin reimportar y sin perder ni una anotación."
//!
//! Es la prueba de la mudanza, y existe porque la ubicación de un
//! archivo era su ruta absoluta: restaurar en otro equipo, o en el mismo tras
//! reinstalar con otro nombre de usuario, dejaba la biblioteca entera como no
//! disponible. La regla es "ninguna pérdida", y eso era una
//! pérdida.

use std::path::{Path, PathBuf};

use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, Availability, File, LibraryRoot, NewAnnotation, RootKind,
    StorageMode,
};
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{
    self, import_document, refresh_all_availability, ImportContext, ImportOutcome, ImportRequest,
};
use nb_books_lib::portability;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Un equipo: su carpeta de datos, su biblioteca administrada y sus documentos.
struct Equipo {
    raiz: PathBuf,
    library: PathBuf,
    documentos: PathBuf,
    bus: EventBus,
}

impl Equipo {
    /// `nombre` hace las veces del nombre de usuario: es justo lo que cambia al
    /// mudarse, y lo que rompía las rutas absolutas.
    fn nuevo(prueba: &str, nombre: &str) -> Self {
        let raiz = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-mudanza-{prueba}-{}", std::process::id()))
            .join(nombre);
        let library = raiz.join("biblioteca");
        let documentos = raiz.join("Documentos").join("Libros");
        std::fs::create_dir_all(&library).expect("biblioteca");
        std::fs::create_dir_all(&documentos).expect("documentos");
        Self {
            raiz,
            library,
            documentos,
            bus: EventBus::new(),
        }
    }

    fn base(&self) -> PathBuf {
        self.raiz.join("nb-books.sqlite")
    }

    /// Copia una fixture al árbol de documentos de este equipo.
    fn documento(&self, origen: &str, destino: &str) -> PathBuf {
        let ruta = self.documentos.join(destino);
        std::fs::copy(fixtures().join(origen), &ruta).expect("copia de la fixture");
        ruta
    }

    fn importar(&self, db: &Database, ruta: &Path, modo: StorageMode) -> ImportOutcome {
        let mut conn = db.connection().expect("conexión");
        import_document(
            &mut conn,
            &ImportContext {
                scope: &self.raiz,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: ruta.to_path_buf(),
                storage_mode: modo,
                root: None,
            },
        )
        .expect("importación")
    }
}

impl Drop for Equipo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.raiz);
    }
}

fn anotar(conn: &rusqlite::Connection, edition_id: &str, texto: &str) -> Annotation {
    Annotation::create(
        conn,
        &NewAnnotation {
            payload: None,
            edition_id: edition_id.to_owned(),
            kind: AnnotationKind::Highlight,
            body: Some(texto.to_owned()),
            target: format!(
                r#"{{"version":1,"href":"OEBPS/c1.xhtml","mediaType":"application/xhtml+xml",
                    "locations":{{"progression":0.25}},"text":{{"highlight":"{texto}"}}}}"#
            ),
        },
    )
    .expect("anotación")
}

/// **El criterio de aceptación.**
#[test]
fn mudarse_de_equipo_no_pierde_la_biblioteca() {
    // ---------------------------------------------------------------- origen
    let ana = Equipo::nuevo("criterio", "ana");
    let origen = Database::open(&ana.base()).expect("base de origen");

    let uno = ana.documento("minimo-valido.epub", "uno.epub");
    let dos = ana.documento("minimo-valido.pdf", "dos.pdf");
    let tres = ana.documento("minimo-valido.cbz", "tres.cbz");

    let ImportOutcome::Imported(referenciado_uno) =
        ana.importar(&origen, &uno, StorageMode::Referenced)
    else {
        panic!("debía importarse");
    };
    let ImportOutcome::Imported(referenciado_dos) =
        ana.importar(&origen, &dos, StorageMode::Referenced)
    else {
        panic!("debía importarse");
    };
    let ImportOutcome::Imported(administrado) = ana.importar(&origen, &tres, StorageMode::Managed)
    else {
        panic!("debía importarse");
    };

    let anotaciones_esperadas = {
        let mut conn = origen.connection().expect("conexión");
        let a = anotar(&conn, &referenciado_uno.edition.id, "algo que no se pierde");
        let b = anotar(&conn, &referenciado_dos.edition.id, "otra que tampoco");
        let c = anotar(&conn, &administrado.edition.id, "y una del administrado");

        // Todo disponible antes de mudarse: si no, la prueba no demostraría nada.
        let informe = refresh_all_availability(&mut conn).expect("revisión");
        assert_eq!(informe.available, 3, "el punto de partida debe estar sano");

        vec![a.id, b.id, c.id]
    };

    // La copia administrada, para llevársela: es lo que hace una copia de
    // seguridad completa, y sin ella el documento administrado no estaría en el
    // equipo nuevo por mucho que el catálogo lo recuerde.
    let copia_administrada = {
        let conn = origen.connection().expect("conexión");
        library::resolve_path(&conn, &administrado.file)
            .expect("consulta")
            .expect("raíz localizada")
    };

    let exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };

    // La exportación **no** puede llevar ninguna ruta absoluta de este equipo:
    // llevarla es exactamente el defecto que este módulo corrige.
    let json = portability::to_json(&exportado).expect("json");
    assert!(
        !json.contains(&ana.raiz.to_string_lossy().into_owned()),
        "la exportación se llevó una ruta del equipo de origen"
    );
    for raiz in &exportado.library_roots {
        assert!(raiz.path.is_none(), "una raíz viajó con su ruta puesta");
    }

    drop(origen);

    // --------------------------------------------------------------- destino
    // Otro equipo: otro nombre de usuario, otra ruta base. Los mismos documentos
    // con la misma estructura relativa, que es lo que el usuario se lleva.
    let beatriz = Equipo::nuevo("criterio", "beatriz");
    beatriz.documento("minimo-valido.epub", "uno.epub");
    beatriz.documento("minimo-valido.pdf", "dos.pdf");
    std::fs::copy(
        &copia_administrada,
        beatriz
            .library
            .join(copia_administrada.file_name().expect("nombre")),
    )
    .expect("copia de la biblioteca administrada");

    let destino = Database::open(&beatriz.base()).expect("base de destino");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &exportado).expect("importación");
    }

    let mut conn = destino.connection().expect("conexión");

    // 1. Nada se ha reimportado: los mismos tres archivos, con los mismos ids.
    let archivos: i64 = conn
        .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .expect("recuento");
    assert_eq!(archivos, 3, "restaurar no puede crear registros nuevos");

    // 2. Las raíces referenciadas llegan **no localizadas**, y sus documentos
    //    aparecen ausentes. La administrada no: se resuelve sola con la carpeta
    //    de biblioteca de esta instalación.
    let raices = LibraryRoot::list(&conn).expect("raíces");
    let referenciadas: Vec<_> = raices
        .iter()
        .filter(|r| r.kind == RootKind::Referenced)
        .collect();
    assert_eq!(
        referenciadas.len(),
        1,
        "los dos documentos compartían carpeta"
    );
    assert!(
        referenciadas[0].path.is_none(),
        "una raíz restaurada no puede llegar apuntando a otro equipo"
    );

    let informe = refresh_all_availability(&mut conn).expect("revisión");
    assert_eq!(informe.checked, 3);
    assert_eq!(
        informe.missing, 2,
        "los referenciados están ausentes hasta reapuntar su raíz"
    );

    // 3. **Una** llamada por raíz. Eso es toda la mudanza.
    LibraryRoot::relocate(
        &conn,
        &referenciadas[0].id,
        &beatriz.documentos.to_string_lossy(),
    )
    .expect("raíz reapuntada");

    // 4. Todo vuelve, y vuelve al equipo nuevo.
    let informe = refresh_all_availability(&mut conn).expect("revisión");
    assert_eq!(informe.checked, 3);
    assert_eq!(
        informe.available, 3,
        "reapuntar una raíz devuelve sus documentos"
    );
    assert_eq!(informe.missing, 0);

    for archivo in [&referenciado_uno.file, &referenciado_dos.file] {
        let resuelta = library::resolve_path(&conn, archivo)
            .expect("consulta")
            .expect("raíz localizada");
        assert!(
            resuelta.starts_with(&beatriz.raiz),
            "el documento sigue apuntando al equipo anterior: {}",
            resuelta.display()
        );
        assert!(resuelta.exists());
    }

    // 5. Ni una anotación perdida, y siguen siendo las mismas.
    let vivas: Vec<String> = conn
        .prepare("SELECT id FROM annotations WHERE deleted_at IS NULL ORDER BY id")
        .expect("consulta")
        .query_map([], |r| r.get(0))
        .expect("filas")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("anotaciones");
    let mut esperadas = anotaciones_esperadas;
    esperadas.sort();
    assert_eq!(vivas, esperadas, "la mudanza perdió o alteró anotaciones");
}

/// El archivo administrado no necesita que nadie reapunte nada: su raíz se
/// resuelve con la carpeta de biblioteca de la instalación que lo abre.
#[test]
fn la_biblioteca_administrada_se_resuelve_sola_en_el_equipo_nuevo() {
    let ana = Equipo::nuevo("administrada", "ana");
    let origen = Database::open(&ana.base()).expect("base de origen");
    let uno = ana.documento("minimo-valido.epub", "uno.epub");
    let ImportOutcome::Imported(administrado) = ana.importar(&origen, &uno, StorageMode::Managed)
    else {
        panic!("debía importarse");
    };

    let copia = {
        let conn = origen.connection().expect("conexión");
        library::resolve_path(&conn, &administrado.file)
            .expect("consulta")
            .expect("raíz localizada")
    };
    assert!(copia.starts_with(&ana.library));

    let exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };
    drop(origen);

    let beatriz = Equipo::nuevo("administrada", "beatriz");
    // El usuario se lleva su carpeta de biblioteca administrada, que es lo que
    // hace una copia de seguridad completa.
    std::fs::copy(
        &copia,
        beatriz.library.join(copia.file_name().expect("nombre")),
    )
    .expect("copia de la biblioteca administrada");

    let destino = Database::open(&beatriz.base()).expect("base de destino");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &exportado).expect("importación");
    }

    let mut conn = destino.connection().expect("conexión");
    let informe = refresh_all_availability(&mut conn).expect("revisión");
    assert_eq!(
        informe.available, 1,
        "el administrado debía volver sin que nadie reapuntara nada"
    );

    let resuelta = library::resolve_path(&conn, &administrado.file)
        .expect("consulta")
        .expect("raíz localizada");
    assert!(resuelta.starts_with(&beatriz.library));
}

/// Una raíz no localizada no puede impedir el arranque ni ocultar el documento:
/// sigue en el catálogo, con sus metadatos, marcado como ausente.
#[test]
fn una_raiz_sin_localizar_deja_el_documento_visible_y_ausente() {
    let ana = Equipo::nuevo("sin-localizar", "ana");
    let db = Database::open(&ana.base()).expect("base");
    let uno = ana.documento("minimo-valido.epub", "uno.epub");
    let ImportOutcome::Imported(importado) = ana.importar(&db, &uno, StorageMode::Referenced)
    else {
        panic!("debía importarse");
    };

    let conn = db.connection().expect("conexión");
    LibraryRoot::forget_path(&conn, &importado.file.root_id).expect("raíz olvidada");

    // Sin ruta no hay nada que comprobar en disco, y eso es exactamente ausente.
    let estado = library::refresh_availability(&conn, &importado.file.id).expect("revisión");
    assert_eq!(estado, Availability::Missing);

    // Pero el documento sigue ahí, con su edición y sus metadatos.
    let archivos = File::list_for_edition(&conn, &importado.edition.id).expect("archivos");
    assert_eq!(archivos.len(), 1);
    assert!(library::resolve_path(&conn, &archivos[0])
        .expect("consulta")
        .is_none());
}

/// Si el usuario elige la carpeta contenedora al reapuntar una raíz —por ejemplo
/// `.../Biblioteca` cuando la raíz era `.../Biblioteca/biblioteca`—, la raíz se
/// resuelve al subdirectorio donde viven de verdad sus rutas relativas.
#[test]
fn reapuntar_al_directorio_padre_resuelve_la_subcarpeta_de_la_raiz() {
    let ana = Equipo::nuevo("padre-raiz", "ana");
    let db = Database::open(&ana.base()).expect("base");
    let origen = ana.documentos.join("biblioteca");
    let autor = origen.join("Cervantes");
    std::fs::create_dir_all(&autor).expect("carpeta");
    let libro = autor.join("uno.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &libro).expect("copia");

    let ImportOutcome::Imported(importado) = ({
        let mut conn = db.connection().expect("conexión");
        import_document(
            &mut conn,
            &ImportContext {
                scope: &ana.raiz,
                library: &ana.library,
                bus: &ana.bus,
            },
            &ImportRequest {
                source: libro,
                storage_mode: StorageMode::Referenced,
                root: Some(origen.clone()),
            },
        )
        .expect("importación")
    }) else {
        panic!("debía importarse");
    };

    // El usuario mueve `Biblioteca/biblioteca` a otra carpeta contenedora y en
    // el selector elige la carpeta contenedora, no la subcarpeta `biblioteca`.
    let contenedora = ana.raiz.join("NuevoSitio").join("Biblioteca");
    let subcarpeta = contenedora.join("biblioteca").join("Cervantes");
    std::fs::create_dir_all(&subcarpeta).expect("nueva carpeta");
    std::fs::copy(
        fixtures().join("minimo-valido.epub"),
        subcarpeta.join("uno.epub"),
    )
    .expect("copia en destino");
    std::fs::remove_dir_all(&origen).expect("mudanza");

    let mut conn = db.connection().expect("conexión");
    let reapuntada = LibraryRoot::relocate(
        &conn,
        &importado.file.root_id,
        &contenedora.to_string_lossy(),
    )
    .expect("raíz reapuntada");
    assert_eq!(
        PathBuf::from(reapuntada.path.expect("ruta")),
        contenedora
            .join("biblioteca")
            .canonicalize()
            .expect("canónica")
    );

    let informe = refresh_all_availability(&mut conn).expect("revisión");
    assert_eq!(informe.available, 1);
    assert_eq!(informe.missing, 0);
}

/// Reconciliar archivos movidos dentro de una carpeta elegida —al reimportar o
/// repasar— conserva una sola raíz y no crea una raíz nueva por cada subcarpeta
/// de autor.
#[test]
fn reconciliar_una_carpeta_movida_conserva_una_sola_raiz() {
    let ana = Equipo::nuevo("una-raiz-movida", "ana");
    let db = Database::open(&ana.base()).expect("base");
    let origen = ana.documentos.join("antes");
    std::fs::create_dir_all(origen.join("AutorA")).expect("carpeta");
    std::fs::create_dir_all(origen.join("AutorB")).expect("carpeta");
    let uno = origen.join("AutorA").join("uno.epub");
    let dos = origen.join("AutorB").join("dos.pdf");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &uno).expect("copia");
    std::fs::copy(fixtures().join("minimo-valido.pdf"), &dos).expect("copia");

    {
        let mut conn = db.connection().expect("conexión");
        for archivo in [&uno, &dos] {
            import_document(
                &mut conn,
                &ImportContext {
                    scope: &ana.raiz,
                    library: &ana.library,
                    bus: &ana.bus,
                },
                &ImportRequest {
                    source: archivo.clone(),
                    storage_mode: StorageMode::Referenced,
                    root: Some(origen.clone()),
                },
            )
            .expect("importación inicial");
        }
    }

    let destino = ana.documentos.join("despues");
    std::fs::rename(&origen, &destino).expect("mover carpeta entera");
    let destino_canonico = destino.canonicalize().expect("canónica");

    {
        let mut conn = db.connection().expect("conexión");
        for relativo in ["AutorA/uno.epub", "AutorB/dos.pdf"] {
            let res = import_document(
                &mut conn,
                &ImportContext {
                    scope: &ana.raiz,
                    library: &ana.library,
                    bus: &ana.bus,
                },
                &ImportRequest {
                    source: destino_canonico.join(relativo),
                    storage_mode: StorageMode::Referenced,
                    root: Some(destino_canonico.clone()),
                },
            )
            .expect("reconciliación");
            assert!(matches!(res, ImportOutcome::AlreadyInLibrary(_)));
        }
    }

    let conn = db.connection().expect("conexión");
    let con_archivos: Vec<(String, i64)> = conn
        .prepare("SELECT root_id, count(*) FROM files GROUP BY root_id")
        .expect("consulta")
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("filas")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("raíces con archivos");
    assert_eq!(
        con_archivos.len(),
        1,
        "todos los archivos movidos deben colgar de la única raíz elegida, no de una por autor"
    );
    assert_eq!(con_archivos[0].1, 2);
}
