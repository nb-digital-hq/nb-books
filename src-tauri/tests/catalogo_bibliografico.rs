//! Criterio de aceptación de la.
//!
//! "Una prueba crea dos ediciones distintas de la misma obra y ambas coexisten
//! con sus archivos y anotaciones separados."
//!
//! El caso no es académico: es el que hace inservible un modelo de "título y
//! autor" en cuanto alguien tiene el original y su traducción, o dos ediciones
//! con paginación distinta. Sin la distinción entre obra, edición y archivo, una
//! de las dos sobrescribe a la otra o las anotaciones se mezclan.
//!
//! Contratos: `docs/architecture/api/schemas.md` y
//! `docs/architecture/metadata-model.md`.

use std::path::PathBuf;

use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, ContentHash, Contributor, ContributorRole, Edition, File,
    IdentifierScheme, NewAnnotation, NewContributor, NewEdition, NewFile, NewWork, StorageMode,
    Subject, Work,
};
use nb_books_lib::db::Database;

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-catalogo-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn base(&self) -> PathBuf {
        self.0.join("nb-books.sqlite")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Identificadores de lo creado, para poder comprobarlo tras reabrir la base.
struct Creado {
    obra: String,
    original: String,
    traduccion: String,
}

/// Puebla la base con una obra y dos ediciones suyas: el original y su
/// traducción, cada una con su archivo, sus contribuidores y sus anotaciones.
fn poblar(db: &Database) -> Creado {
    let conn = db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Cien años de soledad".to_owned(),
            original_language: Some("es".to_owned()),
        },
    )
    .expect("obra");

    let original = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id.clone(),
            title: "Cien años de soledad".to_owned(),
            publisher: Some("Editorial Sudamericana".to_owned()),
            published_date: Some("1967-05-30".to_owned()),
            edition_statement: Some("Primera edición".to_owned()),
            language: Some("es".to_owned()),
            ..NewEdition::default()
        },
    )
    .expect("edición original");

    let traduccion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id.clone(),
            title: "One Hundred Years of Solitude".to_owned(),
            publisher: Some("Harper & Row".to_owned()),
            published_date: Some("1970".to_owned()),
            language: Some("en".to_owned()),
            ..NewEdition::default()
        },
    )
    .expect("edición traducida");

    // Identificadores externos: distintos por edición, que es exactamente por lo
    // que no pueden servir como identidad de la obra.
    Edition::add_identifier(&conn, &original.id, IdentifierScheme::Isbn, "9789500720021")
        .expect("isbn del original");
    Edition::add_identifier(
        &conn,
        &traduccion.id,
        IdentifierScheme::Isbn,
        "9780060883287",
    )
    .expect("isbn de la traducción");

    // El autor es el mismo en ambas; la traductora solo existe en una. Es lo que
    // un modelo plano de "autor: texto" no sabe representar.
    let autor = Contributor::create(
        &conn,
        &NewContributor {
            name: "Gabriel García Márquez".to_owned(),
            sort_name: Some("García Márquez, Gabriel".to_owned()),
            orcid: None,
        },
    )
    .expect("autor");
    let traductor = Contributor::create(
        &conn,
        &NewContributor {
            name: "Gregory Rabassa".to_owned(),
            sort_name: Some("Rabassa, Gregory".to_owned()),
            orcid: None,
        },
    )
    .expect("traductor");

    for edicion in [&original.id, &traduccion.id] {
        Contributor::link(&conn, edicion, &autor.id, ContributorRole::Author, 0).expect("autoría");
    }
    Contributor::link(
        &conn,
        &traduccion.id,
        &traductor.id,
        ContributorRole::Translator,
        0,
    )
    .expect("traducción");

    let materia = Subject::ensure(&conn, "local", "Realismo mágico").expect("materia");
    for edicion in [&original.id, &traduccion.id] {
        Subject::link(&conn, edicion, &materia.id).expect("materia enlazada");
    }

    // Un archivo por edición: rutas y contenidos distintos.
    File::create(
        &conn,
        &NewFile {
            edition_id: original.id.clone(),
            root_id: nb_books_lib::db::catalog::MANAGED_ROOT_ID.to_owned(),
            relative_path: "cien-anos-de-soledad.epub".to_owned(),
            content_hash: ContentHash::from_sha256(&[0x01; 32]),
            byte_size: 812_345,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Managed,
        },
    )
    .expect("archivo del original");

    File::create(
        &conn,
        &NewFile {
            edition_id: traduccion.id.clone(),
            root_id: nb_books_lib::db::catalog::MANAGED_ROOT_ID.to_owned(),
            relative_path: "one-hundred-years.epub".to_owned(),
            content_hash: ContentHash::from_sha256(&[0x02; 32]),
            byte_size: 799_001,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Managed,
        },
    )
    .expect("archivo de la traducción");

    // Dos anotaciones en el original y una en la traducción, ancladas por
    // selectores y nunca por una posición numérica.
    for (indice, texto) in ["Sobre el hielo", "Sobre Melquíades"].iter().enumerate() {
        Annotation::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: original.id.clone(),
                kind: AnnotationKind::Note,
                body: Some((*texto).to_owned()),
                target: format!(
                    r#"{{"type":"FragmentSelector","value":"epubcfi(/6/{}!/4/2)"}}"#,
                    indice + 2
                ),
            },
        )
        .expect("anotación del original");
    }

    Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: traduccion.id.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: r#"{"type":"TextQuoteSelector","exact":"the ice"}"#.to_owned(),
        },
    )
    .expect("anotación de la traducción");

    Creado {
        obra: obra.id,
        original: original.id,
        traduccion: traduccion.id,
    }
}

/// El criterio de aceptación, comprobado además tras cerrar y reabrir la base:
/// coexistir un instante en memoria no demuestra nada en una fase cuya regla es
/// que no se pierda nada.
#[test]
fn dos_ediciones_de_la_misma_obra_coexisten_con_sus_archivos_y_anotaciones_separados() {
    let tmp = Temporal::nuevo("coexistencia");

    let creado = {
        let db = Database::open(&tmp.base()).expect("base");
        poblar(&db)
    };

    let db = Database::open(&tmp.base()).expect("base reabierta");
    let conn = db.connection().expect("conexión");

    // --- Ambas ediciones cuelgan de la misma obra ---
    let ediciones = Edition::list_for_work(&conn, &creado.obra).expect("ediciones");
    assert_eq!(ediciones.len(), 2, "la obra debe tener dos ediciones");
    assert!(ediciones.iter().all(|e| e.work_id == creado.obra));

    let titulos: Vec<&str> = ediciones.iter().map(|e| e.title.as_str()).collect();
    assert!(titulos.contains(&"Cien años de soledad"));
    assert!(titulos.contains(&"One Hundred Years of Solitude"));

    // --- Cada edición conserva sus propios datos de publicación ---
    let original = ediciones
        .iter()
        .find(|e| e.id == creado.original)
        .expect("original");
    let traduccion = ediciones
        .iter()
        .find(|e| e.id == creado.traduccion)
        .expect("traducción");

    assert_eq!(original.language.as_deref(), Some("es"));
    assert_eq!(traduccion.language.as_deref(), Some("en"));
    assert_eq!(original.published_date.as_deref(), Some("1967-05-30"));
    assert_eq!(traduccion.published_date.as_deref(), Some("1970"));

    // --- Archivos separados ---
    let archivos_original = File::list_for_edition(&conn, &creado.original).expect("archivos");
    let archivos_traduccion = File::list_for_edition(&conn, &creado.traduccion).expect("archivos");

    assert_eq!(archivos_original.len(), 1);
    assert_eq!(archivos_traduccion.len(), 1);
    assert_ne!(
        archivos_original[0].relative_path,
        archivos_traduccion[0].relative_path
    );
    assert_ne!(
        archivos_original[0].content_hash,
        archivos_traduccion[0].content_hash
    );

    // --- Anotaciones separadas ---
    let notas_original = Annotation::list_for_edition(&conn, &creado.original).expect("notas");
    let notas_traduccion = Annotation::list_for_edition(&conn, &creado.traduccion).expect("notas");

    assert_eq!(notas_original.len(), 2);
    assert_eq!(notas_traduccion.len(), 1);
    assert!(notas_original
        .iter()
        .all(|a| a.edition_id == creado.original));
    assert!(notas_traduccion
        .iter()
        .all(|a| a.edition_id == creado.traduccion));

    // --- Los identificadores externos son de la edición, no de la obra ---
    let ids_original = Edition::identifiers(&conn, &creado.original).expect("identificadores");
    let ids_traduccion = Edition::identifiers(&conn, &creado.traduccion).expect("identificadores");
    assert_ne!(ids_original, ids_traduccion);

    // --- El autor es compartido; la traductora, no ---
    let firmas_original =
        Contributor::list_for_edition(&conn, &creado.original).expect("contribuidores");
    let firmas_traduccion =
        Contributor::list_for_edition(&conn, &creado.traduccion).expect("contribuidores");

    assert_eq!(firmas_original.len(), 1);
    assert_eq!(firmas_traduccion.len(), 2);
    assert_eq!(firmas_original[0].0.id, firmas_traduccion[0].0.id);
    assert!(firmas_traduccion
        .iter()
        .any(|(_, rol, _)| rol == "translator"));
}

/// La otra mitad de "coexisten": lo que le pasa a una edición no le pasa a la
/// otra. Es la prueba de que la separación es real y no una coincidencia de
/// consultas filtradas.
#[test]
fn borrar_una_edicion_no_toca_a_la_otra_ni_a_la_obra() {
    let tmp = Temporal::nuevo("borrado");
    let db = Database::open(&tmp.base()).expect("base");
    let creado = poblar(&db);
    let conn = db.connection().expect("conexión");

    conn.execute("DELETE FROM editions WHERE id = ?1", [&creado.original])
        .expect("borrado de la edición original");

    // La obra sigue existiendo: borrar una edición no borra el concepto.
    assert!(Work::find(&conn, &creado.obra).expect("consulta").is_some());

    // La traducción conserva íntegro todo lo suyo.
    let ediciones = Edition::list_for_work(&conn, &creado.obra).expect("ediciones");
    assert_eq!(ediciones.len(), 1);
    assert_eq!(ediciones[0].id, creado.traduccion);

    assert_eq!(
        File::list_for_edition(&conn, &creado.traduccion)
            .expect("archivos")
            .len(),
        1
    );
    assert_eq!(
        Annotation::list_for_edition(&conn, &creado.traduccion)
            .expect("anotaciones")
            .len(),
        1
    );
    assert_eq!(
        Subject::list_for_edition(&conn, &creado.traduccion)
            .expect("materias")
            .len(),
        1
    );

    // Y lo de la edición borrada se va entero, sin dejar filas colgando.
    assert!(File::list_for_edition(&conn, &creado.original)
        .expect("archivos")
        .is_empty());
    assert!(Annotation::list_for_edition(&conn, &creado.original)
        .expect("anotaciones")
        .is_empty());
    assert!(Edition::identifiers(&conn, &creado.original)
        .expect("identificadores")
        .is_empty());
}

/// La cadena real de migraciones, no la sintética de las pruebas del motor: una
/// base creada por la versión anterior tiene que llegar al esquema nuevo sin
/// perder lo que ya había, y dejando su copia de seguridad.
///
/// Es la propiedad que la regla de esta fase exige y la que ninguna prueba de
/// unidad puede dar por supuesta.
#[test]
fn una_base_de_la_version_anterior_migra_sin_perder_nada() {
    let tmp = Temporal::nuevo("migracion");
    let ruta = tmp.base();

    // Se reconstruye una base tal y como la dejaba la V1: solo `app_metadata`.
    let install_id = {
        let conn = rusqlite::Connection::open(&ruta).expect("base v1");
        conn.execute_batch(include_str!("../migrations/V1__app_metadata.sql"))
            .expect("esquema v1");
        conn.pragma_update(None, "user_version", 1)
            .expect("versión 1");
        conn.query_row(
            "SELECT value FROM app_metadata WHERE key = 'install_id'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("install_id")
    };

    let db = Database::open(&ruta).expect("la base v1 debe poder migrarse");
    let conn = db.connection().expect("conexión");

    // El identificador de instalación sobrevive: si se perdiera, el dispositivo
    // dejaría de reconocerse a sí mismo tras actualizar.
    let tras_migrar: String = conn
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'install_id'",
            [],
            |r| r.get(0),
        )
        .expect("install_id tras migrar");
    assert_eq!(tras_migrar, install_id);

    // Y el esquema nuevo está disponible y es usable. La versión objetivo se
    // consulta, no se escribe a mano: cada migración nueva la mueve, y una
    // constante aquí convertiría esta prueba en un recordatorio de mantenerla.
    assert_eq!(
        db.schema_version().expect("versión"),
        nb_books_lib::db::migrations::target_version()
    );
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra posterior a la migración".to_owned(),
            original_language: None,
        },
    )
    .expect("el catálogo funciona tras migrar");
    assert!(Work::find(&conn, &obra.id).expect("consulta").is_some());

    // La copia previa queda en disco: es la última red si una migración futura
    // destruye información.
    assert!(
        ruta.with_extension("sqlite.v1.backup").exists(),
        "debe existir el respaldo previo a la migración"
    );
}
