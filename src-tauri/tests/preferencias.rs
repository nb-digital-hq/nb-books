//! Criterio de aceptación de la., en el lado del runtime.
//!
//! "Borrar los datos del webview no pierde ninguna preferencia; una prueba
//! comprueba que ningún secreto llega a la base de datos."
//!
//! La primera mitad se demuestra en el frontend (`src/lib/core/settings.test.ts`),
//! que es donde vive la caché. Aquí se demuestra lo que la sostiene: que las
//! preferencias sobreviven a cerrar la aplicación, y que la migración que las
//! introduce no se lleva nada por delante.
//!
//! ADR-010 lo pide con nombre y apellidos: "una migración descuidada perdería
//! las preferencias de accesibilidad del usuario, que es precisamente el caso
//! más sensible. La migración debe conservar los valores previos y ser
//! idempotente. Una prueba debe demostrarlo antes de dar la función por
//! terminada."

use std::path::PathBuf;

use nb_books_lib::db::catalog::{NewWork, Work};
use nb_books_lib::db::{migrations, Database};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-prefs-{nombre}-{}", std::process::id()));
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

/// Lo que sostiene a la caché del webview: las preferencias están en SQLite, así
/// que cerrar la aplicación, o limpiar los datos del navegador, no se lleva
/// ninguna.
#[test]
fn las_preferencias_sobreviven_a_cerrar_y_reabrir_la_aplicacion() {
    let tmp = Temporal::nuevo("persistencia");

    {
        let db = Database::open(&tmp.base()).expect("base");
        let conn = db.connection().expect("conexión");

        settings::set(
            &conn,
            SettingKey::Theme,
            &SettingValue::Text("dark".to_owned()),
        )
        .expect("tema");
        settings::set(&conn, SettingKey::HighContrast, &SettingValue::Flag(true))
            .expect("alto contraste");
        settings::set(&conn, SettingKey::ReducedMotion, &SettingValue::Flag(true))
            .expect("movimiento reducido");
        settings::set(&conn, SettingKey::FontScale, &SettingValue::Number(1.75))
            .expect("escala tipográfica");
        settings::set(
            &conn,
            SettingKey::Language,
            &SettingValue::Text("pt".to_owned()),
        )
        .expect("idioma");
    }

    let db = Database::open(&tmp.base()).expect("base reabierta");
    let conn = db.connection().expect("conexión");

    assert_eq!(
        settings::get(&conn, SettingKey::Theme).expect("lectura"),
        SettingValue::Text("dark".to_owned())
    );
    assert_eq!(
        settings::get(&conn, SettingKey::HighContrast).expect("lectura"),
        SettingValue::Flag(true)
    );
    assert_eq!(
        settings::get(&conn, SettingKey::ReducedMotion).expect("lectura"),
        SettingValue::Flag(true)
    );
    assert_eq!(
        settings::get(&conn, SettingKey::FontScale).expect("lectura"),
        SettingValue::Number(1.75)
    );
    assert_eq!(
        settings::get(&conn, SettingKey::Language).expect("lectura"),
        SettingValue::Text("pt".to_owned())
    );
}

/// Lo que ADR-010 exige demostrar: reaplicar las migraciones no toca las
/// preferencias ya escritas.
///
/// El riesgo que señala no es teórico. Una migración de preferencias descuidada
/// se lleva por delante el alto contraste y el movimiento reducido de alguien
/// que los necesita, y esa persona se encuentra la aplicación inutilizable sin
/// saber por qué.
#[test]
fn reaplicar_las_migraciones_no_pierde_ninguna_preferencia() {
    let tmp = Temporal::nuevo("idempotente");
    let db = Database::open(&tmp.base()).expect("base");

    {
        let conn = db.connection().expect("conexión");
        settings::set(&conn, SettingKey::HighContrast, &SettingValue::Flag(true))
            .expect("alto contraste");
        settings::set(&conn, SettingKey::FontScale, &SettingValue::Number(2.0)).expect("escala");
    }

    let mut conn = db.connection().expect("conexión");
    let version = migrations::run(&mut conn, Some(&tmp.base())).expect("reaplicar migraciones");

    assert_eq!(version, migrations::target_version());
    assert_eq!(
        settings::get(&conn, SettingKey::HighContrast).expect("lectura"),
        SettingValue::Flag(true),
        "el alto contraste no puede perderse al reaplicar migraciones"
    );
    assert_eq!(
        settings::get(&conn, SettingKey::FontScale).expect("lectura"),
        SettingValue::Number(2.0)
    );
}

/// Una base de la versión anterior, con datos, llega al esquema con preferencias
/// sin perder nada de lo que ya tenía.
#[test]
fn una_base_anterior_a_las_preferencias_migra_sin_perder_su_contenido() {
    let tmp = Temporal::nuevo("desde-v3");
    let ruta = tmp.base();

    // Se construye una base tal y como la dejaba la versión anterior: con el
    // catálogo, pero sin la tabla de preferencias.
    let obra = {
        let conn = rusqlite::Connection::open(&ruta).expect("base");
        conn.execute_batch(include_str!("../migrations/V1__app_metadata.sql"))
            .expect("v1");
        conn.execute_batch(include_str!("../migrations/V2__bibliographic_core.sql"))
            .expect("v2");
        conn.execute_batch(include_str!("../migrations/V3__metadata_provenance.sql"))
            .expect("v3");
        conn.pragma_update(None, "user_version", 3)
            .expect("versión");

        let obra = Work::create(
            &conn,
            &NewWork {
                canonical_title: "Obra anterior a las preferencias".to_owned(),
                original_language: None,
            },
        )
        .expect("obra");

        let existe: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='settings'",
                [],
                |r| r.get(0),
            )
            .expect("consulta");
        assert_eq!(existe, 0, "la base de partida no debe tener preferencias");

        conn.flush_prepared_statement_cache();
        obra.id
    };

    let db = Database::open(&ruta).expect("la base anterior debe poder migrarse");
    let conn = db.connection().expect("conexión");

    // Lo que ya había sigue ahí.
    assert!(
        Work::find(&conn, &obra).expect("consulta").is_some(),
        "la migración a preferencias no puede perder el catálogo"
    );

    // Y lo nuevo funciona, partiendo de los valores por defecto.
    assert_eq!(
        settings::get(&conn, SettingKey::HighContrast).expect("lectura"),
        SettingValue::Flag(false)
    );
    settings::set(&conn, SettingKey::HighContrast, &SettingValue::Flag(true)).expect("escritura");
    assert_eq!(
        settings::get(&conn, SettingKey::HighContrast).expect("lectura"),
        SettingValue::Flag(true)
    );

    // Y queda su respaldo, porque la base tenía datos.
    assert!(
        ruta.with_extension("sqlite.v3.backup").exists(),
        "debe existir el respaldo previo a la migración"
    );
}

/// La otra mitad del criterio, sobre una base real y no en memoria: ni por la
/// superficie pública ni por SQL directo entra un secreto.
#[test]
fn ningun_secreto_llega_a_la_base_por_ninguna_puerta() {
    let tmp = Temporal::nuevo("secretos");
    let db = Database::open(&tmp.base()).expect("base");
    let conn = db.connection().expect("conexión");

    // Puerta 1: la superficie pública. Las claves son una enumeración cerrada,
    // así que un secreto ni siquiera se puede nombrar.
    for intento in [
        "api_key",
        "openai_api_key",
        "access_token",
        "refresh_token",
        "client_secret",
        "user_password",
        "lms_credential",
        "passphrase",
    ] {
        assert!(
            SettingKey::from_name(intento).is_none(),
            "{intento} no puede ser una preferencia conocida"
        );

        // Puerta 2: SQL directo. La base lo rechaza por sí misma.
        let crudo = conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, '\"secreto\"')",
            [intento],
        );
        assert!(
            crudo.is_err(),
            "la base aceptó por SQL directo una clave con pinta de secreto: {intento}"
        );
    }

    let filas: i64 = conn
        .query_row("SELECT count(*) FROM settings", [], |r| r.get(0))
        .expect("consulta");
    assert_eq!(filas, 0, "algún secreto llegó a la base");
}

// --- Los dos planos de personalización -----------------------------

/// **Compromiso 4 de ADR-012:** cambiar el tema de la aplicación no altera cómo
/// se ve un libro, ni al revés.
///
/// La separación no se confía a que nadie mezcle los prefijos: cada preferencia
/// declara su plano, y esta prueba recorre los dos en las dos direcciones. Sin
/// esto, "se guardan por separado" sería una afirmación sin mecanismo.
#[test]
fn tocar_el_plano_de_la_aplicacion_no_altera_el_del_libro_ni_al_reves() {
    let tmp = Temporal::nuevo("planos");
    let db = Database::open(&tmp.base()).expect("base");
    let conn = db.connection().expect("conexión");

    let del_libro = || {
        settings::snapshot(&conn)
            .expect("instantánea")
            .into_iter()
            .filter(|s| s.plane == settings::SettingPlane::Book)
            .map(|s| (s.key, s.value))
            .collect::<Vec<_>>()
    };
    let de_la_aplicacion = || {
        settings::snapshot(&conn)
            .expect("instantánea")
            .into_iter()
            .filter(|s| s.plane == settings::SettingPlane::Application)
            .map(|s| (s.key, s.value))
            .collect::<Vec<_>>()
    };

    let libro_antes = del_libro();
    let aplicacion_antes = de_la_aplicacion();
    assert!(!libro_antes.is_empty() && !aplicacion_antes.is_empty());

    // Cambiar el tema de la aplicación, a fondo.
    settings::set(
        &conn,
        SettingKey::Theme,
        &SettingValue::Text("dark".to_owned()),
    )
    .expect("escritura");
    settings::set(&conn, SettingKey::HighContrast, &SettingValue::Flag(true)).expect("escritura");
    settings::set(&conn, SettingKey::FontScale, &SettingValue::Number(1.5)).expect("escritura");

    assert_eq!(
        del_libro(),
        libro_antes,
        "cambiar la apariencia de la aplicación no puede tocar cómo se ve un libro"
    );

    // Y ahora al revés: componer el libro entero de otra forma.
    settings::set(
        &conn,
        SettingKey::BookFontFamily,
        &SettingValue::Text("serif".to_owned()),
    )
    .expect("escritura");
    settings::set(&conn, SettingKey::BookFontSize, &SettingValue::Number(24.0)).expect("escritura");
    settings::set(&conn, SettingKey::BookJustify, &SettingValue::Flag(true)).expect("escritura");

    assert_eq!(
        de_la_aplicacion(),
        vec![
            ("theme".to_owned(), SettingValue::Text("dark".to_owned())),
            (
                "accent_color".to_owned(),
                SettingValue::Text("theme".to_owned()),
            ),
            ("high_contrast".to_owned(), SettingValue::Flag(true)),
            ("reduced_motion".to_owned(), SettingValue::Flag(false)),
            ("font_scale".to_owned(), SettingValue::Number(1.5)),
            ("language".to_owned(), SettingValue::Text("es".to_owned())),
            ("reading_speed".to_owned(), SettingValue::Number(180.0)),
            (
                "ai_provider".to_owned(),
                SettingValue::Text("none".to_owned())
            ),
            (
                "ai_memory_limit_mb".to_owned(),
                SettingValue::Number(1024.0)
            ),
            ("offline_mode".to_owned(), SettingValue::Flag(false)),
            // Los dos del traductor: vacíos de fábrica, y vacíos
            // significa que no hay servidor y que se traduce al idioma de la
            // interfaz.
            (
                "translation_host".to_owned(),
                SettingValue::Text(String::new())
            ),
            // El registro de extensiones, por el mismo motivo que el
            // traductor: vacío de fábrica, porque el registro oficial todavía
            // no existe y poner una dirección que no responde sería peor que no
            // poner ninguna.
            (
                "registry_host".to_owned(),
                SettingValue::Text(String::new())
            ),
            // El tema de extensión elegido. Se guarda el
            // identificador de quien lo aporta, no sus colores, y al
            // desinstalarla esta preferencia se vacía sola.
            (
                "extension_theme".to_owned(),
                SettingValue::Text(String::new())
            ),
            (
                "translation_target".to_owned(),
                SettingValue::Text(String::new())
            ),
            // Las tres. Son del plano de la aplicación y no del
            // libro: cómo se recorre la biblioteca y con qué voz se lee en alto
            // no son composición tipográfica, que es lo que ADR-012 separa.
            ("library_page_size".to_owned(), SettingValue::Number(50.0)),
            (
                "library_view".to_owned(),
                SettingValue::Text("cuadricula".to_owned())
            ),
            // Las tres de F7: el orden de la biblioteca, las tarjetas plegadas de
            // la barra lateral y el tope de lo derivado. Las tres son de cómo se
            // usa la aplicación, no de cómo se compone un libro.
            (
                "library_sort".to_owned(),
                SettingValue::Text("addedAt:desc".to_owned())
            ),
            (
                "sidebar_folded_cards".to_owned(),
                SettingValue::Text(String::new())
            ),
            ("home_suggestions".to_owned(), SettingValue::Flag(true)),
            ("derived_budget".to_owned(), SettingValue::Number(20.0)),
            // Los hilos de la inferencia local: cero es automático.
            ("ai_threads".to_owned(), SettingValue::Number(0.0)),
            // El proveedor de IA externo: vacío, como la voz.
            ("ai_host".to_owned(), SettingValue::Text(String::new())),
            ("ai_model".to_owned(), SettingValue::Text(String::new())),
            (
                "ai_generator".to_owned(),
                SettingValue::Text("local".to_owned())
            ),
            // Cuál de los modelos generativos del catálogo genera: vacío es "el que
            // se recomienda para este equipo".
            (
                "ai_model_local".to_owned(),
                SettingValue::Text(String::new())
            ),
            ("voice_name".to_owned(), SettingValue::Text(String::new())),
            // Las cuatro. También del plano de la aplicación: de
            // dónde sale la voz no es composición tipográfica del libro.
            (
                "voice_provider".to_owned(),
                SettingValue::Text("local".to_owned())
            ),
            ("speech_host".to_owned(), SettingValue::Text(String::new())),
            ("speech_model".to_owned(), SettingValue::Text(String::new())),
            ("speech_voice".to_owned(), SettingValue::Text(String::new())),
        ],
        "componer el libro de otra forma no puede tocar la apariencia de la aplicación"
    );
}

/// Ninguna preferencia del libro entra en la caché del primer pintado.
///
/// La caché de ADR-010 existe para no pintar la carcasa dos veces, y el libro no
/// está en el primer pintado: se abre después. Meterla ahí ampliaría la
/// superficie de divergencia entre la caché y la fuente de verdad sin comprar
/// nada.
#[test]
fn el_plano_del_libro_no_entra_en_la_cache_de_arranque() {
    let tmp = Temporal::nuevo("cache-arranque");
    let db = Database::open(&tmp.base()).expect("base");
    let conn = db.connection().expect("conexión");

    for preferencia in settings::snapshot(&conn).expect("instantánea") {
        if preferencia.plane == settings::SettingPlane::Book {
            assert!(
                !preferencia.affects_first_paint,
                "{} no puede estar en la caché de arranque",
                preferencia.key
            );
        }
    }

    // El plano del libro tiene los siete valores de ADR-012 —tipografía, cuerpo,
    // interlineado, márgenes, ancho, justificación y paginación— más el modo de
    // lectura y los cinco del kit accesible.
    let del_libro = settings::snapshot(&conn)
        .expect("instantánea")
        .into_iter()
        .filter(|s| s.plane == settings::SettingPlane::Book)
        .count();
    assert_eq!(del_libro, 14);
}
