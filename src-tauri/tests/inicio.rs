//! Criterio de aceptación de la.
//!
//! "Una biblioteca vacía y una sin historial de lectura producen una pantalla
//! útil y no una pantalla rota; cada recomendación puede decir por qué aparece."
//!
//! Las dos mitades se comprueban aquí porque las dos son consultas: una pantalla
//! rota, en esta vista, casi siempre es una consulta que devuelve un error o una
//! fila que no existe.

use nb_books_lib::db::catalog::{
    Contributor, ContributorRole, Edition, NewEdition, NewWork, Subject, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::home::{
    filas, lectura, HomeRow, HomeRowKind, HomeRowLayout, EN_LISTA, MINIMO_POR_FILA,
};
use nb_books_lib::reader::progress;
use nb_books_lib::reader::provider::Precision;
use nb_books_lib::reader::{Locations, Locator};

fn base() -> Database {
    Database::open_in_memory().expect("base en memoria")
}

fn locator(progresion: f64) -> Locator {
    Locator::new(
        "OEBPS/c1.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["#char=0".to_owned()],
            progression: Some(progresion),
            total_progression: Some(progresion),
            position: None,
        },
        None,
    )
    .expect("locator")
}

/// Crea una edición con su obra, y devuelve su identificador.
fn edicion(conn: &rusqlite::Connection, titulo: &str, serie: Option<&str>) -> String {
    let obra = Work::create(
        conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: None,
        },
    )
    .expect("obra");

    Edition::create(
        conn,
        &NewEdition {
            work_id: obra.id,
            title: titulo.to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: serie.map(ToOwned::to_owned),
            series_index: None,
        },
    )
    .expect("edición")
    .id
}

// --- La pantalla útil sobre una biblioteca vacía ----------------------------

/// **Primera mitad del criterio.**
#[test]
fn una_biblioteca_vacia_no_rompe_la_pantalla_de_inicio() {
    let db = base();
    let conn = db.connection().expect("conexión");

    let cifras = lectura(&conn).expect("las cifras se calculan sobre una base vacía");
    assert_eq!(cifras.documents, 0);
    assert_eq!(cifras.in_progress, 0);
    assert_eq!(cifras.minutes_today, 0);
    assert_eq!(cifras.streak_days, 0);

    // Y sin filas, que es una respuesta y no un fallo.
    assert!(filas(&conn, 1)
        .expect("las filas se consultan sobre una base vacía")
        .is_empty());
}

/// Una biblioteca con documentos pero sin una sola sesión tampoco rompe.
#[test]
fn una_biblioteca_sin_historial_de_lectura_tampoco_rompe() {
    let db = base();
    let conn = db.connection().expect("conexión");
    edicion(&conn, "Sin Abrir", None);

    let cifras = lectura(&conn).expect("cifras");

    assert_eq!(cifras.documents, 1);
    assert_eq!(cifras.in_progress, 0);
    assert_eq!(cifras.streak_days, 0);
}

// --- Las filas de Inicio -------------------------------------------------

fn filas_de(conn: &rusqlite::Connection, semilla: u64) -> Vec<HomeRow> {
    filas(conn, semilla).expect("filas de inicio")
}

/// **El criterio.** Ninguna fila tiene menos obras de las que llenan una,
/// y ninguna obra sale dos veces en la misma pantalla.
///
/// Observación de uso del 2026-09-12: dos filas, "y lo peor es que son duplicados". Y "si
/// vamos a tener cinco libros, es mejor no tener la fila".
#[test]
fn ninguna_fila_se_queda_corta_ni_repite_una_obra() {
    let db = base();
    let conn = db.connection().expect("conexión");
    for n in 0..45 {
        edicion(&conn, &format!("Obra {n}"), None);
    }

    let pantalla = filas_de(&conn, 7);
    assert!(!pantalla.is_empty());
    let mut vistas = std::collections::HashSet::new();
    for fila in &pantalla {
        let minimo = if fila.layout == HomeRowLayout::Lista {
            EN_LISTA
        } else {
            MINIMO_POR_FILA
        };
        assert!(
            fila.items.len() >= minimo,
            "la fila {} trae {}",
            fila.id,
            fila.items.len()
        );
        for obra in &fila.items {
            assert!(
                vistas.insert(obra.edition_id.clone()),
                "{} sale dos veces",
                obra.title
            );
        }
    }
    assert!(matches!(pantalla[0].kind, HomeRowKind::Novedades));
}

/// Con pocas obras no se inventan filas: ninguna, que es una respuesta y no un
/// fallo, y la pantalla de Inicio sigue teniendo su estado vacío.
#[test]
fn con_pocas_obras_no_hay_filas_a_medias() {
    let db = base();
    let conn = db.connection().expect("conexión");
    assert!(filas_de(&conn, 1).is_empty());
    for n in 0..5 {
        edicion(&conn, &format!("Obra {n}"), None);
    }
    assert!(filas_de(&conn, 1).is_empty());
}

/// Lo que se está leyendo trae una fila de su autor, que lo dice, y ni se
/// recomienda a sí mismo ni sale en ninguna otra fila.
///
/// Antes **cada obra traía su propia fila de autor**: en la base real un
/// autor con 23 obras eran 24 filas de `contributors`, y la primera versión de esta
/// fila buscaba por identificador, encontraba una obra y no salía nunca. Desde V38
/// el autor es una fila y la importación la reutiliza, que es como se da de alta
/// aquí; la búsqueda sigue siendo por nombre, que vale en los dos mundos.
#[test]
fn lo_que_se_lee_trae_la_fila_de_su_autor_sin_lo_empezado() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let obras: Vec<String> = (0..14)
        .map(|n| {
            let id = edicion(&conn, &format!("Del Autor {n}"), None);
            let autor = Contributor::ensure(&conn, "Gabriel García Márquez").expect("autor");
            Contributor::link(&conn, &id, &autor.id, ContributorRole::Author, 0).expect("vínculo");
            id
        })
        .collect();
    progress::save(
        &conn,
        &obras[0],
        progress::PERFIL,
        &locator(0.5),
        Precision::Exact,
    )
    .expect("progreso");

    let pantalla = filas_de(&conn, 3);
    assert!(
        pantalla.iter().any(|f| matches!(
            &f.kind,
            HomeRowKind::PorqueLees { author, .. } if author == "Gabriel García Márquez"
        )),
        "no hay fila del autor de lo que se lee"
    );
    assert!(pantalla
        .iter()
        .flat_map(|f| &f.items)
        .all(|o| o.edition_id != obras[0]));
}

/// Cada visita cambia qué materias se enseñan, y la misma semilla da la misma
/// pantalla. Es la enmienda de ADR-013: reproducibles, no fijas.
#[test]
fn las_materias_cambian_con_la_visita_y_la_misma_semilla_repite_la_pantalla() {
    let db = base();
    let conn = db.connection().expect("conexión");
    for m in 0..16 {
        let materia = Subject::ensure(&conn, "local", &format!("Materia {m}")).expect("materia");
        for n in 0..12 {
            let id = edicion(&conn, &format!("M{m} Obra {n}"), None);
            Subject::link(&conn, &id, &materia.id).expect("vínculo");
        }
    }

    let materias_de = |semilla: u64| -> Vec<String> {
        filas_de(&conn, semilla)
            .into_iter()
            .filter_map(|f| match f.kind {
                HomeRowKind::Materia { label, .. } => Some(label),
                _ => None,
            })
            .collect()
    };

    assert_eq!(
        filas_de(&conn, 42),
        filas_de(&conn, 42),
        "la misma semilla, la misma pantalla"
    );
    let primera = materias_de(1);
    assert!(!primera.is_empty());
    assert!(
        (2..20).any(|semilla| materias_de(semilla) != primera),
        "ninguna visita cambió las materias"
    );
}

/// Un documento empezado y sin terminar cuenta como en curso; uno terminado, no.
#[test]
fn en_curso_es_empezado_y_sin_terminar() {
    let db = base();
    let conn = db.connection().expect("conexión");

    let empezada = edicion(&conn, "A Medias", None);
    let terminada = edicion(&conn, "Terminada", None);
    let intacta = edicion(&conn, "Intacta", None);

    progress::save(
        &conn,
        &empezada,
        progress::PERFIL,
        &locator(0.5),
        Precision::Exact,
    )
    .expect("progreso");
    progress::save(
        &conn,
        &terminada,
        progress::PERFIL,
        &locator(1.0),
        Precision::Exact,
    )
    .expect("progreso");
    // La intacta no tiene progreso guardado.
    let _ = intacta;

    assert_eq!(lectura(&conn).expect("cifras").in_progress, 1);
}

/// La racha sigue viva si se leyó hoy o **ayer**.
///
/// Cortarla a medianoche haría que quien lee de noche la perdiera por acostarse
/// tarde, y la racha existe para acompañar, no para castigar.
#[test]
fn la_racha_no_se_corta_por_acostarse_tarde() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let edicion_id = edicion(&conn, "Leído", None);

    // Tres días seguidos que terminan **ayer**: nadie ha leído todavía hoy.
    for dias in [3, 2, 1] {
        conn.execute(
            "INSERT INTO reading_sessions (id, edition_id, started_at, ended_at, start_locator)
             VALUES (?1, ?2, datetime('now', ?3), datetime('now', ?3, '+20 minutes'), ?4)",
            (
                format!("s{dias}"),
                &edicion_id,
                format!("-{dias} days"),
                serde_json::to_string(&locator(0.1)).expect("json"),
            ),
        )
        .expect("sesión");
    }

    let cifras = lectura(&conn).expect("cifras");

    assert_eq!(cifras.streak_days, 3, "la racha cuenta los tres días");
    assert_eq!(cifras.minutes_today, 0, "hoy todavía no se ha leído");
    assert!(
        cifras.minutes_week >= 55,
        "la semana suma las tres sesiones"
    );
}

/// Un hueco corta la racha, que es lo que la hace significar algo.
#[test]
fn un_dia_sin_leer_corta_la_racha() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let edicion_id = edicion(&conn, "Leído", None);

    // Hoy y hace tres días: el hueco de en medio corta.
    for dias in [0, 3] {
        conn.execute(
            "INSERT INTO reading_sessions (id, edition_id, started_at, ended_at, start_locator)
             VALUES (?1, ?2, datetime('now', ?3), datetime('now', ?3, '+10 minutes'), ?4)",
            (
                format!("s{dias}"),
                &edicion_id,
                format!("-{dias} days"),
                serde_json::to_string(&locator(0.1)).expect("json"),
            ),
        )
        .expect("sesión");
    }

    assert_eq!(lectura(&conn).expect("cifras").streak_days, 1);
}
