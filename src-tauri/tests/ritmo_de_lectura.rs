//! Ritmo de lectura y cuánto falta.
//!
//! El criterio: se calcula con los minutos que el runtime ya guarda y el progreso
//! real, sin cronómetro que arrancar y parar, y si no hay datos suficientes no se
//! inventa una estimación. Aquí se escriben sesiones como las deja el lector -con
//! su hora y su posición de principio y de fin- y se comprueba qué ritmo sale, y
//! cuándo no sale ninguno.

use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::reader::progress::ritmo;

struct Base {
    dir: std::path::PathBuf,
    db: Database,
    edicion: String,
}

impl Drop for Base {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn base(nombre: &str) -> Base {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-ritmo-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("carpeta");
    let db = Database::open(&dir.join("nb.sqlite")).expect("base");
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
    .expect("edición")
    .id;
    drop(conn);
    Base { dir, db, edicion }
}

fn locator(progresion: f64) -> String {
    serde_json::json!({
        "version": 1,
        "href": "c1.xhtml",
        "mediaType": "application/xhtml+xml",
        "locations": { "fragments": ["#char=0"], "totalProgression": progresion },
        "text": { "highlight": "algo" }
    })
    .to_string()
}

/// Una sesión de `minutos` que fue de `desde` a `hasta`, empezada hace `hace` horas.
fn sesion(base: &Base, hace: f64, minutos: f64, desde: f64, hasta: f64) {
    let conn = base.db.connection().expect("conexión");
    conn.execute(
        "INSERT INTO reading_sessions (edition_id, started_at, ended_at, start_locator, end_locator)
         VALUES (?1,
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?2),
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?3),
                 ?4, ?5)",
        rusqlite::params![
            base.edicion,
            format!("-{} minutes", hace * 60.0),
            format!("-{} minutes", hace * 60.0 - minutos),
            locator(desde),
            locator(hasta)
        ],
    )
    .expect("sesión");
}

#[test]
fn sin_bastante_lectura_no_se_inventa_un_ritmo() {
    let base = base("poca");
    let conn = base.db.connection().expect("conexión");
    assert_eq!(
        ritmo(&conn, &base.edicion).expect("consulta"),
        None,
        "sin sesiones"
    );
    drop(conn);

    // Cinco minutos no dicen a qué ritmo lee nadie.
    sesion(&base, 3.0, 5.0, 0.0, 0.01);
    let conn = base.db.connection().expect("conexión");
    assert_eq!(ritmo(&conn, &base.edicion).expect("consulta"), None);
}

#[test]
fn el_ritmo_sale_de_las_sesiones_y_no_de_los_saltos_ni_de_los_ratos_quietos() {
    let base = base("ritmo");
    // Lectura de verdad: 20 y 30 minutos, al mismo ritmo de 0,25 % por minuto.
    sesion(&base, 30.0, 20.0, 0.10, 0.15);
    sesion(&base, 20.0, 30.0, 0.15, 0.225);
    // Un salto por el índice: un minuto y medio libro. No es lectura.
    sesion(&base, 10.0, 1.0, 0.225, 0.70);
    // El libro abierto toda una tarde sin tocarlo.
    sesion(&base, 8.0, 300.0, 0.70, 0.71);
    // Volver atrás para releer algo no resta ritmo.
    sesion(&base, 2.0, 15.0, 0.71, 0.40);

    let conn = base.db.connection().expect("conexión");
    let calculado = ritmo(&conn, &base.edicion)
        .expect("consulta")
        .expect("hay bastante");
    assert_eq!(calculado.sessions, 2, "{calculado:?}");
    assert!((calculado.minutes - 50.0).abs() < 0.1, "{calculado:?}");
    assert!(
        (calculado.progression_per_minute - 0.0025).abs() < 1e-6,
        "{calculado:?}"
    );
    // Con la mitad por leer, al ritmo al que se lee: unos doscientos minutos.
    let faltan = (1.0 - 0.5) / calculado.progression_per_minute;
    assert!((faltan - 200.0).abs() < 0.5, "{faltan}");
}
