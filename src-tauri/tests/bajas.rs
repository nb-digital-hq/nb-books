//! Criterio de aceptación de la.
//!
//! "Tras eliminar un documento no quedan filas huérfanas en ninguna tabla ni
//! archivos de caché asociados."
//!
//! Una baja que deja restos es peor que no borrar: la biblioteca parece limpia y
//! la base engorda con filas que nadie vuelve a mirar, hasta que una exportación
//! las arrastra sin dueño o una consulta las cuenta.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use nb_books_lib::collections::{Collection, CollectionOrigin};
use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, Contributor, ContributorRole, Edition, IdentifierScheme,
    NewAnnotation, NewContributor, NewEdition, NewWork, StorageMode, Subject, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::library::{delete_document, FilePolicy};
use nb_books_lib::metadata::{self, Field, Source};

struct Escenario {
    dir: PathBuf,
    library: PathBuf,
    covers: PathBuf,
    db: Database,
    bus: EventBus,
    bajas: Arc<Mutex<Vec<String>>>,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-baja-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let library = dir.join("biblioteca");
        let covers = dir.join("cache/portadas");
        std::fs::create_dir_all(&library).expect("biblioteca");
        std::fs::create_dir_all(&covers).expect("portadas");

        let bus = EventBus::new();
        let bajas = Arc::new(Mutex::new(Vec::new()));
        let destino = Arc::clone(&bajas);
        bus.subscribe("espia", move |evento| {
            if let DomainEvent::DocumentDeleted { edition_id } = evento {
                destino
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(edition_id.clone());
            }
        });

        let db = Database::open(&dir.join("nb-books.sqlite")).expect("base");
        Self {
            dir,
            library,
            covers,
            db,
            bus,
            bajas,
        }
    }

    fn filas(&self, tabla: &str) -> i64 {
        self.db
            .connection()
            .expect("conexión")
            .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
            .expect("consulta")
    }

    /// Un documento **completo**: con todo lo que puede colgar de él, para que
    /// la prueba de "no quedan huérfanos" signifique algo.
    fn documento_completo(&self, titulo: &str, modo: StorageMode) -> (String, String, PathBuf) {
        let conn = self.db.connection().expect("conexión");

        let obra = Work::create(
            &conn,
            &NewWork {
                canonical_title: titulo.to_owned(),
                original_language: Some("es".to_owned()),
            },
        )
        .expect("obra");
        let edicion = Edition::create(
            &conn,
            &NewEdition {
                work_id: obra.id.clone(),
                title: titulo.to_owned(),
                publisher: Some("Una editorial".to_owned()),
                ..NewEdition::default()
            },
        )
        .expect("edición");

        // Archivo, en el modo que se pida.
        let ruta = if modo == StorageMode::Managed {
            self.library.join(format!("{titulo}.epub"))
        } else {
            self.dir.join(format!("fuera-{titulo}.epub"))
        };
        std::fs::write(&ruta, format!("contenido de {titulo}")).expect("archivo");
        let (hash, tamano) = nb_books_lib::library::hash_file(&ruta).expect("identidad");
        let (raiz, relativa) =
            nb_books_lib::library::ensure_root(&conn, &ruta, modo, None).expect("raíz");
        nb_books_lib::db::catalog::File::create(
            &conn,
            &nb_books_lib::db::catalog::NewFile {
                edition_id: edicion.id.clone(),
                root_id: raiz,
                relative_path: relativa,
                content_hash: hash,
                byte_size: tamano as i64,
                media_type: "application/epub+zip".to_owned(),
                storage_mode: modo,
            },
        )
        .expect("archivo registrado");

        // Anotaciones, identificadores, contribuidor, materia, procedencia,
        // propuesta viva y pertenencia a una colección.
        Annotation::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.id.clone(),
                kind: AnnotationKind::Note,
                body: Some("Una nota".to_owned()),
                target: r#"{"version":1,"href":"c1.xhtml","mediaType":"application/xhtml+xml",
                            "locations":{"progression":0.1}}"#
                    .to_owned(),
            },
        )
        .expect("anotación");

        Edition::add_identifier(&conn, &edicion.id, IdentifierScheme::Isbn, "9788437604947")
            .expect("isbn");

        let quien = Contributor::create(
            &conn,
            &NewContributor {
                name: format!("Autor de {titulo}"),
                ..NewContributor::default()
            },
        )
        .expect("contribuidor");
        Contributor::link(&conn, &edicion.id, &quien.id, ContributorRole::Author, 0)
            .expect("autoría");

        let materia =
            Subject::ensure(&conn, "local", &format!("Materia de {titulo}")).expect("materia");
        Subject::link(&conn, &edicion.id, &materia.id).expect("materia enlazada");

        metadata::propose(&conn, &edicion.id, Field::Title, Source::File, Some(titulo))
            .expect("procedencia");
        metadata::set_manual(&conn, &edicion.id, Field::Title, Some("Como yo lo llamo"))
            .expect("manual");
        metadata::propose(
            &conn,
            &edicion.id,
            Field::Title,
            Source::Online,
            Some("Lo que dice el proveedor"),
        )
        .expect("propuesta viva");

        let coleccion = Collection::create_manual(
            &conn,
            &format!("Lista de {titulo}"),
            CollectionOrigin::Personal,
        )
        .expect("colección");
        Collection::add(&conn, &coleccion.id, &edicion.id).expect("añadido");

        // Y su portada en la caché.
        std::fs::write(self.covers.join(format!("{}.png", edicion.id)), b"portada")
            .expect("portada");

        (obra.id, edicion.id, ruta)
    }

    fn dar_de_baja(
        &self,
        edicion: &str,
        policy: FilePolicy,
    ) -> Result<nb_books_lib::library::Deleted, NbError> {
        let mut conn = self.db.connection().expect("conexión");
        delete_document(&mut conn, &self.bus, edicion, &self.covers, policy)
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Todas las tablas del esquema que pueden guardar algo de un documento.
const TABLAS: &[&str] = &[
    "works",
    "editions",
    "edition_identifiers",
    "files",
    "contributors",
    "edition_contributors",
    "subjects",
    "edition_subjects",
    "annotations",
    "metadata_provenance",
    "metadata_suggestions",
    "collection_items",
];

/// **El criterio de aceptación.**
#[test]
fn tras_dar_de_baja_no_quedan_filas_huerfanas_ni_archivos_de_cache() {
    let esc = Escenario::nuevo("completo");
    let (_, edicion, ruta) = esc.documento_completo("Unico", StorageMode::Managed);

    // Todo está poblado antes de borrar: si no, la comprobación no dice nada.
    for tabla in TABLAS {
        assert!(esc.filas(tabla) > 0, "{tabla} debía tener algo antes");
    }
    assert!(esc.covers.join(format!("{edicion}.png")).exists());

    let informe = esc
        .dar_de_baja(&edicion, FilePolicy::DeleteManagedCopy)
        .expect("baja");

    // --- Ni una fila en ninguna tabla ---
    for tabla in TABLAS {
        assert_eq!(
            esc.filas(tabla),
            0,
            "quedaron filas huérfanas en {tabla} tras la baja"
        );
    }
    // La colección sigue existiendo: es del usuario, no del documento.
    assert_eq!(esc.filas("collections"), 1);

    // --- Ni un archivo de caché ---
    assert_eq!(informe.covers_removed, 1);
    assert!(!esc.covers.join(format!("{edicion}.png")).exists());
    assert_eq!(
        std::fs::read_dir(&esc.covers).expect("caché").count(),
        0,
        "quedaron portadas huérfanas"
    );

    // --- Y lo que se pidió del disco ---
    assert_eq!(informe.file_deleted.as_deref(), Some(ruta.as_path()));
    assert!(!ruta.exists());
    assert!(informe.work_removed);
    assert_eq!(informe.annotations_removed, 1);
}

/// La política es explícita porque borrar el archivo de alguien es
/// irreversible. Por defecto no se toca nada del disco.
#[test]
fn con_la_politica_de_conservar_el_archivo_se_queda_donde_esta() {
    let esc = Escenario::nuevo("conservar");
    let (_, edicion, ruta) = esc.documento_completo("Conservado", StorageMode::Managed);

    let informe = esc.dar_de_baja(&edicion, FilePolicy::Keep).expect("baja");

    assert_eq!(informe.file_deleted, None);
    assert!(ruta.exists(), "el archivo no debía tocarse");
    assert_eq!(esc.filas("editions"), 0, "pero sí sale del catálogo");
}

/// **Un archivo referenciado no se borra jamás**, aunque se pida.
///
/// Vive fuera de la biblioteca porque el usuario decidió que viviera ahí, y
/// puede estar en una carpeta compartida o dentro de otra aplicación. Del que
/// respondemos es del que copiamos nosotros.
#[test]
fn un_archivo_referenciado_no_se_borra_ni_pidiendolo() {
    let esc = Escenario::nuevo("referenciado");
    let (_, edicion, ruta) = esc.documento_completo("Referenciado", StorageMode::Referenced);

    let informe = esc
        .dar_de_baja(&edicion, FilePolicy::DeleteManagedCopy)
        .expect("baja");

    assert_eq!(informe.file_deleted, None);
    assert!(
        ruta.exists(),
        "un archivo referenciado es del usuario, no nuestro"
    );
    assert_eq!(esc.filas("editions"), 0);
}

/// Dar de baja una edición no puede llevarse por delante las otras ediciones de
/// la misma obra, ni lo que comparten.
#[test]
fn dar_de_baja_una_edicion_no_toca_a_las_hermanas() {
    let esc = Escenario::nuevo("hermanas");
    let conn = esc.db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "La obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");

    let compartido = Contributor::create(
        &conn,
        &NewContributor {
            name: "La autora".to_owned(),
            ..NewContributor::default()
        },
    )
    .expect("contribuidora");

    let mut ediciones = Vec::new();
    for titulo in ["Original", "Traducción"] {
        let e = Edition::create(
            &conn,
            &NewEdition {
                work_id: obra.id.clone(),
                title: titulo.to_owned(),
                ..NewEdition::default()
            },
        )
        .expect("edición");
        Contributor::link(&conn, &e.id, &compartido.id, ContributorRole::Author, 0)
            .expect("autoría");
        ediciones.push(e.id);
    }
    drop(conn);

    let informe = esc
        .dar_de_baja(&ediciones[0], FilePolicy::Keep)
        .expect("baja");

    assert!(!informe.work_removed, "la obra tiene otra edición viva");
    assert_eq!(
        informe.contributors_swept, 0,
        "la autora sigue firmando la traducción"
    );
    assert_eq!(esc.filas("editions"), 1);
    assert_eq!(esc.filas("works"), 1);
    assert_eq!(esc.filas("contributors"), 1);
}

/// Una obra sin ediciones no es nada: no se puede abrir, ni buscar, ni mostrar.
/// Se va con la última.
#[test]
fn la_obra_se_va_con_su_ultima_edicion() {
    let esc = Escenario::nuevo("ultima");
    let (obra, edicion, _) = esc.documento_completo("Unica", StorageMode::Referenced);

    let informe = esc.dar_de_baja(&edicion, FilePolicy::Keep).expect("baja");

    assert!(informe.work_removed);
    let conn = esc.db.connection().expect("conexión");
    assert!(Work::find(&conn, &obra).expect("consulta").is_none());
}

/// Los contribuidores y las materias que dejan de estar enlazados a nada se
/// barren. Es distinto de quitar una etiqueta a mano, que no las barre: dar de
/// baja un documento es decir "esto y todo lo suyo se va", y quitar una etiqueta
/// no lo es.
#[test]
fn se_barren_los_contribuidores_y_las_materias_que_quedan_sin_nada() {
    let esc = Escenario::nuevo("barrido");
    let (_, edicion, _) = esc.documento_completo("Con vocabulario", StorageMode::Referenced);

    assert_eq!(esc.filas("contributors"), 1);
    assert_eq!(esc.filas("subjects"), 1);

    let informe = esc.dar_de_baja(&edicion, FilePolicy::Keep).expect("baja");

    assert_eq!(informe.contributors_swept, 1);
    assert_eq!(informe.subjects_swept, 1);
    assert_eq!(esc.filas("contributors"), 0);
    assert_eq!(esc.filas("subjects"), 0);
}

/// El resto del sistema se entera por el bus: el índice de búsqueda tendrá que
/// retirar sus fragmentos, y la interfaz, quitarlo de la cuadrícula.
#[test]
fn la_baja_se_publica_en_el_bus() {
    let esc = Escenario::nuevo("evento");
    let (_, edicion, _) = esc.documento_completo("Anunciado", StorageMode::Referenced);

    esc.dar_de_baja(&edicion, FilePolicy::Keep).expect("baja");

    let mut vistos = Vec::new();
    for _ in 0..200 {
        vistos = esc.bajas.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if !vistos.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(vistos, vec![edicion]);
}

#[test]
fn dar_de_baja_algo_que_no_existe_es_un_error_y_no_un_silencio() {
    let esc = Escenario::nuevo("inexistente");

    assert_eq!(
        esc.dar_de_baja(&"0".repeat(32), FilePolicy::Keep).err(),
        Some(NbError::DocumentNotFound)
    );
}

/// Si la copia administrada ya no está en el disco, la baja sigue adelante: el
/// registro tiene que poder irse aunque el archivo se haya perdido antes.
#[test]
fn una_copia_que_ya_no_esta_no_impide_la_baja() {
    let esc = Escenario::nuevo("sin-archivo");
    let (_, edicion, ruta) = esc.documento_completo("Perdido", StorageMode::Managed);
    std::fs::remove_file(&ruta).expect("borrado por detrás");

    let informe = esc
        .dar_de_baja(&edicion, FilePolicy::DeleteManagedCopy)
        .expect("la baja debe completarse igual");

    assert_eq!(informe.file_deleted, None);
    assert_eq!(esc.filas("editions"), 0);
}
