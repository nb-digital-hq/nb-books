//! La otra mitad del criterio de aceptación: **ninguna estructura del
//! sistema almacena la posición como entero.**
//!
//! Esa afirmación no se puede demostrar leyendo el código una vez, porque lo que
//! importa es que siga siendo cierta dentro de dos años y de veinte migraciones.
//! Se demuestra inspeccionando el esquema vivo: cualquier tabla que ancle una
//! posición de lectura tiene que hacerlo con un Locator serializado, y la propia
//! base tiene que rechazar lo contrario.
//!
//! Contratos: `docs/adr/004-reader-locator-model.md` y
//! `docs/architecture/reader-contracts.md`.

use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, Edition, NewAnnotation, NewEdition, NewWork, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::reader::{Locations, Locator, TextContext};

/// Tablas que anclan una posición de lectura.
///
/// La lista es cerrada y se amplía a mano a propósito: añadir una tabla que
/// guarde dónde estaba leyendo alguien obliga a añadirla aquí, y entonces las
/// comprobaciones de abajo se aplican sola sobre ella. Las que todavía no
/// existen se ignoran; aparecerán con su módulo.
const TABLAS_QUE_ANCLAN: &[&str] = &[
    "annotations",
    "progress",
    "reading_sessions",
    "progreso_de_dispositivos",
    "bookmarks",
    "text_chunks",
    "vocabulary",
    "study_cards",
    "concepts",
    // El audio preparado ancla por fragmento para poder seguir el texto mientras
    // suena.
    "audio_marks",
];

/// Columnas numéricas admitidas en esas tablas.
///
/// Nada de esto es una posición de lectura: son contadores, tamaños y marcas.
/// Cualquier otra columna numérica en una tabla que ancla es sospechosa de ser
/// la posición-entero que ADR-004 descarta.
const NUMERICAS_ADMITIDAS: &[(&str, &str)] = &[
    // Milisegundos **dentro del archivo de audio**, no del documento: son una
    // coordenada del sonido, y el documento se ancla al lado con `anchor`. Y el
    // orden en que se dicen los fragmentos, que tampoco es dónde están.
    ("audio_marks", "start_ms"),
    ("audio_marks", "end_ms"),
    ("audio_marks", "ordinal"),
    // Parámetros de repaso espaciado: intervalo en días, factor de facilidad
    // y recuento de repasos. No tienen nada que ver con la posición de lectura.
    ("study_cards", "interval_days"),
    ("study_cards", "ease_factor"),
    ("study_cards", "repetitions"),
    // Su punto de partida, del que se derivan con el registro de repasos.
    ("study_cards", "base_interval_days"),
    ("study_cards", "base_ease_factor"),
    ("study_cards", "base_repetitions"),
];

fn columnas(conn: &rusqlite::Connection, tabla: &str) -> Vec<(String, String)> {
    let mut stmt = conn
        .prepare("SELECT name, type FROM pragma_table_info(?1)")
        .expect("consulta de columnas");
    let filas = stmt
        .query_map([tabla], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("columnas");
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("columnas")
}

fn existe_tabla(conn: &rusqlite::Connection, tabla: &str) -> bool {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [tabla],
        |r| r.get::<_, i64>(0),
    )
    .expect("consulta")
        > 0
}

fn sql_de(conn: &rusqlite::Connection, tabla: &str) -> String {
    conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [tabla],
        |r| r.get(0),
    )
    .expect("definición de la tabla")
}

/// Ninguna tabla que ancle una posición puede tener una columna numérica que no
/// esté declarada como algo distinto de una posición.
///
/// Si alguien añade `progress INTEGER` o `page REAL` en el futuro, esta prueba
/// falla antes de que llegue a la máquina de un usuario y se lleve por delante
/// sus anotaciones al primer cambio de tipografía.
#[test]
fn ninguna_tabla_que_ancla_guarda_la_posicion_como_numero() {
    let db = Database::open_in_memory().expect("base en memoria");
    let conn = db.connection().expect("conexión");

    let mut revisadas = 0;
    for tabla in TABLAS_QUE_ANCLAN {
        if !existe_tabla(&conn, tabla) {
            continue;
        }
        revisadas += 1;

        for (columna, tipo) in columnas(&conn, tabla) {
            let numerica = matches!(tipo.to_uppercase().as_str(), "INTEGER" | "REAL" | "NUMERIC");
            let admitida = NUMERICAS_ADMITIDAS
                .iter()
                .any(|(t, c)| t == tabla && *c == columna);

            assert!(
                !numerica || admitida,
                "{tabla}.{columna} es {tipo}: una posición de lectura no puede ser un número. \
                 Si no es una posición, decláralo en NUMERICAS_ADMITIDAS con su motivo. \
                 Ver ADR-004"
            );
        }
    }

    assert!(
        revisadas > 0,
        "ninguna de las tablas que anclan existe todavía: la prueba no está comprobando nada"
    );
}

/// La columna que ancla no solo es de texto: la base exige que sea un objeto
/// JSON. Un `42` en texto seguiría siendo un entero de progreso disfrazado.
#[test]
fn la_columna_que_ancla_exige_un_objeto_de_selectores() {
    let db = Database::open_in_memory().expect("base en memoria");
    let conn = db.connection().expect("conexión");

    for tabla in TABLAS_QUE_ANCLAN {
        if !existe_tabla(&conn, tabla) {
            continue;
        }
        let sql = sql_de(&conn, tabla);
        assert!(
            sql.contains("json_valid") && sql.contains("json_type"),
            "{tabla} no obliga a que su ancla sea un objeto JSON:\n{sql}"
        );
    }
}

/// El cierre del círculo: lo que produce el modelo es
/// exactamente lo que acepta el esquema. Que cada mitad funcione por su cuenta
/// no demuestra que encajen.
#[test]
fn el_locator_serializado_es_lo_que_la_base_acepta_como_ancla() {
    let db = Database::open_in_memory().expect("base en memoria");
    let conn = db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición");

    let locator = Locator::new(
        "OEBPS/chapter05.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["epubcfi(/6/14!/4/10/3:10)".to_owned()],
            progression: Some(0.42),
            total_progression: Some(0.17),
            position: Some(233),
        },
        Some(
            TextContext::new(
                Some("Muchos años después, "),
                "frente al pelotón de fusilamiento",
                None,
            )
            .expect("contexto"),
        ),
    )
    .expect("locator");

    let anotacion = Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.id.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: locator.to_json().expect("json"),
        },
    )
    .expect("la base debe aceptar un locator como ancla");

    // Y vuelve entero: guardar una anotación y releerla no puede mover la
    // posición ni un carácter.
    let recuperadas = Annotation::list_for_edition(&conn, &edicion.id).expect("anotaciones");
    assert_eq!(recuperadas.len(), 1);
    assert_eq!(recuperadas[0].id, anotacion.id);

    let releido = Locator::from_json(&recuperadas[0].target).expect("locator legible");
    assert_eq!(releido, locator);
}

/// Y al revés: lo que el modelo rechaza, la base también. Dos defensas que
/// dicen lo mismo, para que quitar una no abra la puerta.
#[test]
fn un_entero_de_progreso_lo_rechazan_el_modelo_y_la_base() {
    let db = Database::open_in_memory().expect("base en memoria");
    let conn = db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición");

    for entero in ["42", "0.75"] {
        assert!(
            Locator::from_json(entero).is_err(),
            "el modelo no puede leer {entero} como locator"
        );

        let en_la_base = Annotation::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.id.clone(),
                kind: AnnotationKind::Highlight,
                body: None,
                target: entero.to_owned(),
            },
        );
        assert!(
            matches!(en_la_base, Err(NbError::StorageFailure)),
            "la base no puede aceptar {entero} como ancla"
        );
    }
}
