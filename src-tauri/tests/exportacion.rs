//! Criterio de aceptación de la.
//!
//! "Exportar, borrar la base, importar y comprobar equivalencia campo a campo."
//!
//! Es la prueba que convierte "el usuario es dueño de sus datos" de frase en
//! propiedad verificable. Si falla, el principio de posesión de los datos es
//! decorativo, por muy bien que esté escrito en la Constitución.

use std::path::PathBuf;

use nb_books_lib::collections::{Collection, CollectionOrigin, Condition, Rule};
use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, ContentHash, Contributor, ContributorRole, Edition, File,
    IdentifierScheme, NewAnnotation, NewContributor, NewEdition, NewFile, NewWork, StorageMode,
    Subject, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::metadata::{self, Field, Source};
use nb_books_lib::portability::{self, EditionContributor, Export};
use nb_books_lib::reader::progress;
use nb_books_lib::reader::{Locations, Locator, Precision, TextContext};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-export-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn base(&self, nombre: &str) -> PathBuf {
        self.0.join(format!("{nombre}.sqlite"))
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Una biblioteca pequeña pero con **algo de cada cosa** que el formato debe
/// llevar: si la prueba solo poblara obras y ediciones, no demostraría nada
/// sobre lo que de verdad duele perder.
fn poblar(db: &Database) {
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
            series_title: Some("Grandes novelas".to_owned()),
            series_index: Some(1.5),
            subtitle: None,
        },
    )
    .expect("edición");

    let traduccion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "One Hundred Years of Solitude".to_owned(),
            language: Some("en".to_owned()),
            published_date: Some("1970".to_owned()),
            ..NewEdition::default()
        },
    )
    .expect("traducción");

    Edition::add_identifier(&conn, &original.id, IdentifierScheme::Isbn, "9789500720021")
        .expect("isbn");
    Edition::add_identifier(
        &conn,
        &original.id,
        IdentifierScheme::Doi,
        "10.1000/ejemplo",
    )
    .expect("doi");

    let autor = Contributor::create(
        &conn,
        &NewContributor {
            name: "Gabriel García Márquez".to_owned(),
            sort_name: Some("García Márquez, Gabriel".to_owned()),
            orcid: Some("0000-0002-1825-0097".to_owned()),
        },
    )
    .expect("autor");
    Contributor::link(&conn, &original.id, &autor.id, ContributorRole::Author, 0).expect("autoría");
    Contributor::link(&conn, &traduccion.id, &autor.id, ContributorRole::Author, 0)
        .expect("autoría");

    let materia = Subject::ensure(&conn, "local", "Realismo mágico").expect("materia");
    Subject::link(&conn, &original.id, &materia.id).expect("materia enlazada");

    File::create(
        &conn,
        &NewFile {
            edition_id: original.id.clone(),
            root_id: nb_books_lib::db::catalog::MANAGED_ROOT_ID.to_owned(),
            relative_path: "cien-anos.epub".to_owned(),
            content_hash: ContentHash::from_sha256(&[0x11; 32]),
            byte_size: 812_345,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Managed,
        },
    )
    .expect("archivo");

    File::create(
        &conn,
        &NewFile {
            edition_id: traduccion.id.clone(),
            root_id: nb_books_lib::library::ensure_root(
                &conn,
                std::path::Path::new("/externo/one-hundred-years.epub"),
                StorageMode::Referenced,
                None,
            )
            .expect("raíz")
            .0,
            relative_path: "one-hundred-years.epub".to_owned(),
            content_hash: ContentHash::from_sha256(&[0x22; 32]),
            byte_size: 799_001,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )
    .expect("archivo referenciado");

    // Anotaciones ancladas por Locator, que es lo que hace inservible cualquier
    // exportación que las guarde como un número.
    Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: original.id.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: r#"{"version":1,"href":"OEBPS/c1.xhtml","mediaType":"application/xhtml+xml",
                        "locations":{"progression":0.02},
                        "text":{"highlight":"frente al pelotón de fusilamiento"}}"#
                .to_owned(),
        },
    )
    .expect("resaltado");
    Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: original.id.clone(),
            kind: AnnotationKind::Note,
            body: Some("Sobre Melquíades".to_owned()),
            target: r#"{"version":1,"href":"OEBPS/c2.xhtml","mediaType":"application/xhtml+xml",
                        "locations":{"position":12}}"#
                .to_owned(),
        },
    )
    .expect("nota");

    // Procedencia y una propuesta viva: la decisión del usuario sobre un campo
    // es trabajo suyo y tiene que sobrevivir a la exportación igual que el resto.
    metadata::propose(
        &conn,
        &original.id,
        Field::Title,
        Source::File,
        Some("Cien anos de soledad"),
    )
    .expect("del archivo");
    metadata::set_manual(
        &conn,
        &original.id,
        Field::Title,
        Some("Cien años de soledad"),
    )
    .expect("corrección manual");
    metadata::propose(
        &conn,
        &original.id,
        Field::Publisher,
        Source::Online,
        Some("Sudamericana"),
    )
    .expect("propuesta");
    metadata::set_manual(
        &conn,
        &original.id,
        Field::Publisher,
        Some("Editorial Sudamericana"),
    )
    .expect("editorial a mano");
    metadata::propose(
        &conn,
        &original.id,
        Field::Publisher,
        Source::Online,
        Some("Sudamericana S.A."),
    )
    .expect("propuesta viva");

    // Preferencias, con las de accesibilidad entre ellas.
    settings::set(
        &conn,
        SettingKey::Theme,
        &SettingValue::Text("dark".to_owned()),
    )
    .expect("tema");
    settings::set(&conn, SettingKey::HighContrast, &SettingValue::Flag(true))
        .expect("alto contraste");
    settings::set(&conn, SettingKey::FontScale, &SettingValue::Number(1.75)).expect("escala");

    // Colecciones: una manual con miembros y una por regla, que no los tiene.
    let manual = Collection::create_manual(&conn, "Para el trabajo", CollectionOrigin::Personal)
        .expect("colección manual");
    Collection::add(&conn, &manual.id, &original.id).expect("miembro");
    Collection::create_rule(
        &conn,
        "Todo lo que hable de soledad",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::TitleContains("soledad".to_owned())]).expect("regla"),
    )
    .expect("colección por regla");

    // Progreso y una sesión de lectura cerrada. Es lo que más duele perder:
    // recuperar la biblioteca y no dónde se estaba leyendo sería una restauración
    // a medias.
    let donde = Locator::new(
        "OEBPS/cap01.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["epubcfi(/6/4!/4/2)".to_owned()],
            progression: Some(0.42),
            total_progression: Some(0.17),
            position: Some(233),
        },
        Some(TextContext::new(Some("antes de "), "el pelotón", Some(" y después")).expect("cita")),
    )
    .expect("locator");
    progress::save(
        &conn,
        &original.id,
        progress::PERFIL,
        &donde,
        Precision::Quote,
    )
    .expect("progreso");
    let sesion = progress::open_session(&conn, &original.id, &donde).expect("sesión");
    progress::close_session(&conn, &sesion, &donde).expect("cierre");
}

/// **El criterio de aceptación.**
#[test]
fn exportar_borrar_e_importar_devuelve_una_biblioteca_equivalente() {
    let tmp = Temporal::nuevo("ida-y-vuelta");

    // 1. Una biblioteca con algo de cada cosa.
    let origen = Database::open(&tmp.base("origen")).expect("base de origen");
    poblar(&origen);
    let antes = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };

    // 2. El archivo se escribe y se vuelve a leer: si la equivalencia solo
    //    valiera en memoria, el formato no serviría para nada.
    let ruta = tmp.0.join("biblioteca.nb-books.json");
    std::fs::write(&ruta, portability::to_json(&antes).expect("json")).expect("escritura");
    let leido = portability::from_json(&std::fs::read_to_string(&ruta).expect("lectura"))
        .expect("archivo legible");
    assert_eq!(leido, antes, "el archivo no representa lo que se exportó");

    // 3. Se borra la base. Literalmente.
    drop(origen);
    std::fs::remove_file(tmp.base("origen")).expect("borrado");
    assert!(!tmp.base("origen").exists());

    // 4. Se importa en una base nueva y vacía.
    let destino = Database::open(&tmp.base("destino")).expect("base de destino");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &leido).expect("importación");
    }

    // 5. Equivalencia campo a campo.
    let despues = {
        let conn = destino.connection().expect("conexión");
        portability::export(&conn).expect("exportación de destino")
    };

    assert_eq!(despues.works, antes.works, "obras");
    assert_eq!(despues.editions, antes.editions, "ediciones");
    assert_eq!(
        despues.edition_identifiers, antes.edition_identifiers,
        "identificadores externos"
    );
    assert_eq!(despues.files, antes.files, "archivos");
    assert_eq!(despues.contributors, antes.contributors, "contribuidores");
    assert_eq!(
        despues.edition_contributors, antes.edition_contributors,
        "autorías"
    );
    assert_eq!(despues.subjects, antes.subjects, "materias");
    assert_eq!(
        despues.edition_subjects, antes.edition_subjects,
        "materias enlazadas"
    );
    assert_eq!(despues.annotations, antes.annotations, "anotaciones");
    assert_eq!(
        despues.metadata_provenance, antes.metadata_provenance,
        "procedencia del metadato"
    );
    assert_eq!(
        despues.metadata_suggestions, antes.metadata_suggestions,
        "propuestas de cambio"
    );
    assert_eq!(despues.settings, antes.settings, "preferencias");

    // Y no está vacío: una comparación entre dos nadas también da igual.
    assert_eq!(antes.works.len(), 1);
    assert_eq!(antes.editions.len(), 2);
    assert_eq!(antes.annotations.len(), 2);
    // Veintiuna del plano de la aplicación —con el acento, los dos
    // del traductor, los tres, los cuatro, que
    // son de dónde sale la voz y a qué servicio, el anfitrión del registro de
    // extensiones y el tema de extensión elegido— y trece
    // del plano del libro:
    // los siete más el modo de lectura y el kit accesible.
    // Que los dos planos entren en la exportación es el compromiso 4 de
    // ADR-012: la personalización sobrevive a un cambio de equipo, y el tema
    // elegido es de lo primero que se echa en falta.
    //
    // Y desde F7, tres más de la aplicación: el orden de la biblioteca, las
    // tarjetas plegadas de la barra lateral y el tope de lo derivado. Y desde
    // las preferencias, los hilos de la inferencia local: lo que alguien decide que la IA
    // puede gastar en su equipo también viaja con la biblioteca. Y,
    // dónde está el proveedor de IA externo y qué modelo, pero no su clave.
    //
    // Lo que **no** entra es ninguna credencial, porque no hay ninguna que
    // guardar: ver la nota de `speech.rs`. Ni si el equipo escucha a los
    // teléfonos, que es de ese equipo y no de la biblioteca.
    assert_eq!(antes.settings.len(), 44);
    assert!(
        antes.settings.iter().all(|s| s.key != "mobile_server"),
        "si el equipo escucha a los teléfonos no viaja con la biblioteca"
    );
    assert!(!antes.metadata_provenance.is_empty());
    assert!(!antes.metadata_suggestions.is_empty());
}

/// Lo que de verdad importa que sobreviva: la corrección que hizo el usuario a
/// mano sigue ganando después de restaurar, y la propuesta que rechazó sigue
/// siendo una propuesta y no el valor.
#[test]
fn la_precedencia_del_metadato_manual_sobrevive_a_la_restauracion() {
    let tmp = Temporal::nuevo("precedencia");
    let origen = Database::open(&tmp.base("origen")).expect("base");
    poblar(&origen);

    let exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };
    drop(origen);

    let destino = Database::open(&tmp.base("destino")).expect("base");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &exportado).expect("importación");
    }
    let conn = destino.connection().expect("conexión");

    // El valor manual está donde estaba.
    let titulo: String = conn
        .query_row(
            "SELECT title FROM editions WHERE language = 'es'",
            [],
            |r| r.get(0),
        )
        .expect("consulta");
    assert_eq!(titulo, "Cien años de soledad");

    // Y sigue ganando: una actualización automática posterior no lo pisa.
    let edicion: String = conn
        .query_row("SELECT id FROM editions WHERE language = 'es'", [], |r| {
            r.get(0)
        })
        .expect("consulta");
    let resultado = metadata::propose(
        &conn,
        &edicion,
        Field::Title,
        Source::Online,
        Some("Otro titulo cualquiera"),
    )
    .expect("propuesta");
    assert!(
        matches!(resultado, metadata::Outcome::Suggested(_)),
        "la precedencia manual no sobrevivió a la restauración: {resultado:?}"
    );
}

/// Restaurar sobre una biblioteca con contenido exigiría decidir qué gana ante
/// un conflicto, y esa decisión es ADR-006, todavía propuesto. Se rechaza en
/// lugar de inventarla.
#[test]
fn importar_sobre_una_biblioteca_con_contenido_se_rechaza() {
    let tmp = Temporal::nuevo("ocupada");
    let origen = Database::open(&tmp.base("origen")).expect("base");
    poblar(&origen);
    let exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };

    let mut conn = origen.connection().expect("conexión");
    let resultado = portability::import(&mut conn, &exportado);

    assert!(matches!(resultado, Err(NbError::SchemaMismatch)));
}

/// Una importación que falla a mitad dejaría una biblioteca que parece la del
/// usuario y no lo es, que es peor que un fallo limpio.
#[test]
fn una_importacion_que_falla_no_deja_nada_a_medias() {
    let tmp = Temporal::nuevo("atomica");
    let origen = Database::open(&tmp.base("origen")).expect("base");
    poblar(&origen);
    let mut exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };
    drop(origen);

    // Se corrompe el archivo: una anotación apunta a una edición inexistente.
    exportado.annotations[0].edition_id = "0".repeat(32);

    let destino = Database::open(&tmp.base("destino")).expect("base");
    let mut conn = destino.connection().expect("conexión");
    let resultado = portability::import(&mut conn, &exportado);

    assert!(matches!(resultado, Err(NbError::StorageFailure)));

    for tabla in ["works", "editions", "files", "annotations", "contributors"] {
        let filas: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
            .expect("consulta");
        assert_eq!(
            filas, 0,
            "quedaron filas de una importación fallida en {tabla}"
        );
    }
}

/// El archivo se autodescribe y se niega a ser interpretado por reglas que no
/// son las suyas. Una restauración incompleta es peor que ninguna, porque parece
/// haber funcionado.
#[test]
fn el_archivo_declara_lo_que_es_y_rechaza_lo_que_no_entiende() {
    let tmp = Temporal::nuevo("formato");
    let db = Database::open(&tmp.base("origen")).expect("base");
    poblar(&db);
    let conn = db.connection().expect("conexión");
    let exportado = portability::export(&conn).expect("exportación");
    let json = portability::to_json(&exportado).expect("json");

    assert!(
        json.contains(r#""format": "nb-books-export""#),
        "no se autodescribe"
    );
    assert!(json.contains(r#""formatVersion": 1"#));
    assert!(json.contains(r#""name": "NB Books""#));

    // Otro formato cualquiera.
    let ajeno = json.replace(r#""format": "nb-books-export""#, r#""format": "calibre""#);
    assert!(matches!(
        portability::from_json(&ajeno),
        Err(NbError::DocumentCorrupted)
    ));

    // Una versión posterior del nuestro.
    let futuro = json.replace(r#""formatVersion": 1"#, r#""formatVersion": 99"#);
    assert!(matches!(
        portability::from_json(&futuro),
        Err(NbError::SchemaMismatch)
    ));

    // Basura.
    for basura in ["", "null", "{}", "[]", "{\"format\":"] {
        assert!(portability::from_json(basura).is_err(), "aceptó {basura:?}");
    }
}

/// Dos exportaciones del mismo estado producen el mismo archivo, salvo la marca
/// de tiempo. Un archivo que cambia sin que cambien los datos no se puede
/// comparar ni versionar, y convierte cualquier copia de seguridad incremental
/// en una copia completa.
#[test]
fn dos_exportaciones_del_mismo_estado_son_identicas() {
    let tmp = Temporal::nuevo("estable");
    let db = Database::open(&tmp.base("origen")).expect("base");
    poblar(&db);
    let conn = db.connection().expect("conexión");

    let primera = portability::export(&conn).expect("primera");
    let segunda = portability::export(&conn).expect("segunda");

    let sin_marca = |mut e: Export| {
        e.exported_at = String::new();
        portability::to_json(&e).expect("json")
    };

    assert_eq!(sin_marca(primera), sin_marca(segunda));
}

/// La exportación lleva **todo** lo del usuario, no solo su catálogo.
///
/// Regresión de un hueco real: `collections` y `progress` se declararon en el
/// formato desde la versión 1 y llegaban vacías incluso después de que las
/// funciones que las crean estuvieran terminadas. El documento decía que se
/// poblarían y el código no lo hacía, que es exactamente la contradicción que
/// `docs/` no admite.
#[test]
fn la_exportacion_incluye_colecciones_progreso_y_sesiones() {
    let tmp = Temporal::nuevo("todo");
    let db = Database::open(&tmp.base("origen")).expect("base");
    poblar(&db);

    let conn = db.connection().expect("conexión");
    let export = portability::export(&conn).expect("exportación");

    assert_eq!(export.collections.len(), 2, "la manual y la de regla");
    assert_eq!(
        export.collection_items.len(),
        1,
        "solo la manual tiene miembros; la de regla guarda el criterio, no la lista"
    );
    assert!(
        export
            .collections
            .iter()
            .any(|c| c.kind == "rule" && c.rule.is_some()),
        "la regla tiene que viajar, o la colección llega vacía y sin criterio"
    );
    assert_eq!(export.progress.len(), 1);
    assert_eq!(export.reading_sessions.len(), 1);
    assert_eq!(export.progress[0].precision, "quote");
}

/// Y lo devuelve entero al importarlo.
#[test]
fn el_progreso_y_las_colecciones_sobreviven_a_la_ida_y_vuelta() {
    let tmp = Temporal::nuevo("ida-y-vuelta-completa");

    let origen = Database::open(&tmp.base("origen")).expect("base de origen");
    poblar(&origen);
    let antes = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };
    drop(origen);

    let destino = Database::open(&tmp.base("destino")).expect("base de destino");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &antes).expect("importación");
    }

    let conn = destino.connection().expect("conexión");
    let despues = portability::export(&conn).expect("exportación del destino");

    assert_eq!(despues.collections, antes.collections);
    assert_eq!(despues.collection_items, antes.collection_items);
    assert_eq!(despues.progress, antes.progress);
    assert_eq!(despues.reading_sessions, antes.reading_sessions);

    // Y el locator restaurado se deja interpretar: si viajara roto, la posición
    // estaría "presente" y sería inservible.
    let recuperado = progress::load(
        &conn,
        &antes.progress[0].edition_id,
        &antes.progress[0].profile,
    )
    .expect("consulta")
    .expect("progreso");
    assert_eq!(recuperado.precision, Precision::Quote);
    assert_eq!(recuperado.locator.locations.position, Some(233));
    assert_eq!(
        recuperado.locator.text.expect("cita").highlight,
        "el pelotón"
    );
}

/// Lo que escribe el botón de exportar se relee igual, y no deja
/// nada a medias al lado.
#[test]
fn la_exportacion_guardada_en_disco_se_relee_igual_y_no_deja_restos() {
    let tmp = Temporal::nuevo("a-disco");
    let db = Database::open(&tmp.base("origen")).expect("base");
    poblar(&db);
    let exportacion = {
        let conn = db.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };

    let destino = tmp.0.join("biblioteca.json");
    std::fs::write(&destino, "la exportación de la semana pasada").expect("anterior");
    portability::write_json(&exportacion, &destino).expect("escritura");

    let releida = portability::from_json(&std::fs::read_to_string(&destino).expect("lectura"))
        .expect("archivo legible");
    assert_eq!(
        releida, exportacion,
        "el archivo no representa lo que se exportó"
    );

    let restos: Vec<String> = std::fs::read_dir(&tmp.0)
        .expect("directorio")
        .filter_map(Result::ok)
        .map(|entrada| entrada.file_name().to_string_lossy().into_owned())
        .filter(|nombre| nombre.ends_with(".parcial"))
        .collect();
    assert!(restos.is_empty(), "quedó un archivo a medias: {restos:?}");
}

/// Si la escritura falla, la exportación anterior sigue entera: la copia buena no
/// se pierde por intentar hacer otra.
#[test]
fn una_exportacion_que_no_se_puede_escribir_deja_la_anterior_intacta() {
    let tmp = Temporal::nuevo("sin-pisar");
    let db = Database::open(&tmp.base("origen")).expect("base");
    poblar(&db);
    let exportacion = {
        let conn = db.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };

    let destino = tmp.0.join("biblioteca.json");
    std::fs::write(&destino, "la exportación buena").expect("anterior");
    // Un directorio justo donde iría el archivo a medias: crearlo falla igual que
    // con el disco lleno, sin tener que llenar un disco.
    std::fs::create_dir(tmp.0.join(".biblioteca.json.parcial")).expect("obstáculo");

    let resultado = portability::write_json(&exportacion, &destino);
    assert!(
        matches!(resultado, Err(NbError::StorageFailure)),
        "se esperaba un fallo de almacenamiento: {resultado:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&destino).expect("lectura"),
        "la exportación buena",
        "un fallo al exportar pisó la exportación anterior"
    );
}

/// Lo que se escribe mientras se exporta no entra a medias: la exportación es
/// la foto de un instante, aunque la indexación siga escribiendo.
#[test]
fn una_lectura_coherente_no_ve_lo_que_se_escribe_mientras_dura() {
    let tmp = Temporal::nuevo("foto");
    let db = Database::open(&tmp.base("origen")).expect("base");
    poblar(&db);

    let mut lector = db.connection().expect("conexión de lectura");
    let foto = nb_books_lib::db::read_snapshot(&mut lector).expect("foto");
    let obras = portability::export(&foto).expect("exportación").works.len();

    // Otra conexión escribe en medio, como lo haría la indexación.
    let escritor = db.connection().expect("conexión de escritura");
    Work::create(
        &escritor,
        &NewWork {
            canonical_title: "Llegó tarde".to_owned(),
            original_language: None,
        },
    )
    .expect("obra nueva");

    let durante = portability::export(&foto).expect("exportación").works.len();
    assert_eq!(
        durante, obras,
        "la foto vio una escritura posterior a su comienzo"
    );
    drop(foto);
    drop(lector);

    let conn = db.connection().expect("conexión");
    let despues = portability::export(&conn).expect("exportación").works.len();
    assert_eq!(despues, obras + 1, "la escritura no llegó a la base");
}

/// Un archivo exportado antes de V38 puede traer al mismo autor en
/// varias filas, y la base ya solo admite una. Restaurarlo tiene que fundirlas como
/// las fundió la migración, sin que ninguna obra pierda a su autor: fallar por el
/// índice único sería perder la biblioteca justo el día que hace falta.
///
/// Dos casos, porque chocan con índices distintos: sin ORCID lo que decide es el
/// nombre; con él, el ORCID, aunque el nombre se escriba de otra manera.
#[test]
fn restaurar_una_exportacion_con_el_mismo_autor_en_dos_filas_las_funde() {
    for con_orcid in [false, true] {
        let tmp = Temporal::nuevo(&format!("autores-repetidos-{con_orcid}"));
        let origen = Database::open(&tmp.base("origen")).expect("origen");
        poblar(&origen);
        let mut export =
            portability::export(&origen.connection().expect("conexión")).expect("exporta");
        if !con_orcid {
            export.contributors[0].orcid = None;
        }

        // Como la dejaba la importación de antes: la traducción con su propia fila
        // de la misma autora, escrita en mayúsculas.
        let autora = export.contributors[0].clone();
        let mut repetida = autora.clone();
        repetida.id = "autora-repetida".to_owned();
        repetida.name = autora.name.to_uppercase();
        export.contributors.push(repetida);
        let enlaces: Vec<EditionContributor> = export
            .edition_contributors
            .iter()
            .filter(|ec| ec.contributor_id == autora.id)
            .cloned()
            .collect();
        assert_eq!(enlaces.len(), 2, "la autora firma las dos ediciones");
        export
            .edition_contributors
            .retain(|ec| ec.contributor_id != autora.id || ec.edition_id != enlaces[1].edition_id);
        export.edition_contributors.push(EditionContributor {
            contributor_id: "autora-repetida".to_owned(),
            ..enlaces[1].clone()
        });

        let destino = Database::open(&tmp.base("destino")).expect("destino");
        portability::import(&mut destino.connection().expect("conexión"), &export)
            .unwrap_or_else(|e| panic!("con ORCID {con_orcid}: no se restaura: {e:?}"));

        let conn = destino.connection().expect("conexión");
        let filas: i64 = conn
            .query_row("SELECT count(*) FROM contributors", [], |f| f.get(0))
            .expect("cuenta");
        assert_eq!(filas, 1, "con ORCID {con_orcid}: la misma autora, una fila");
        for enlace in &enlaces {
            let quien: String = conn
                .query_row(
                    "SELECT contributor_id FROM edition_contributors WHERE edition_id = ?1",
                    [&enlace.edition_id],
                    |f| f.get(0),
                )
                .expect("la edición conserva a su autora");
            assert_eq!(quien, autora.id);
        }
    }
}
