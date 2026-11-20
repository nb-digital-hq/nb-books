//! **Regresión**: el arranque en frío sobre una base recién creada producía
//! `database is locked` porque varias conexiones del pool intentaban fijar el
//! modo del diario a la vez. El modo WAL se fija ahora una sola vez, antes de
//! construir el pool.
//!
//! # Por qué esto vive en su propio binario
//!
//! Porque es una aserción de reloj: dentro de la suite de la biblioteca, con
//! cientos de pruebas en paralelo sobre el mismo disco, el resultado dependía de
//! lo ocupada que estuviera la máquina. Cargo ejecuta los binarios de prueba uno
//! detrás de otro, así que aquí se mide sin nadie más compitiendo.
//!
//! # Qué se mide
//!
//! La apertura del pool sobre una base ya migrada, no la creación del esquema:
//! aplicar todas las migraciones a una base vacía cuesta más con cada una que se
//! añade, y mezclarlo con la medición convertiría la prueba en un reloj del
//! número de migraciones.

use nb_books_lib::db::{migrations, Database};

#[test]
fn el_arranque_en_frio_no_sufre_contencion() {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-books-frio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("directorio");

    // Las migraciones se aplican antes y fuera de la medición: aplicar el esquema
    // entero sobre una base vacía cuesta lo que cuestan sus cuarenta y una
    // migraciones, crece con cada una y no dice nada de la contención. Lo que se
    // vigila es abrir el pool, que es donde las conexiones competían por fijar el
    // diario, y eso ocurre en cada arranque.
    let path = dir.join("prueba.sqlite");
    let _ = std::fs::remove_file(&path);
    drop(Database::open(&path).expect("base en archivo"));

    // El mejor de tres: la contención que esto vigila es sistemática -si
    // volviera, la pagarían los tres-, así que un pico suelto del sistema de
    // archivos no tiene por qué contar como regresión.
    let mut mejor = std::time::Duration::MAX;
    let mut db = None;
    for _ in 0..3 {
        drop(db.take());
        let inicio = std::time::Instant::now();
        let abierta = Database::open(&path).expect("base en archivo");
        mejor = mejor.min(inicio.elapsed());
        db = Some(abierta);
    }
    let db = db.expect("abierta");

    // El reintento de r2d2 ante contención cuesta cientos de milisegundos. Un
    // margen amplio evita falsos positivos en máquinas de CI lentas y sigue
    // detectando la regresión, que costaba más de 400 ms.
    assert!(
        mejor < std::time::Duration::from_millis(300),
        "el arranque en frío tardó {mejor:?} en el mejor de tres: posible contención al fijar el diario"
    );
    assert_eq!(
        db.schema_version().expect("legible"),
        migrations::target_version()
    );

    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}

/// El respaldo previo a una migración se retira solo tras tres sesiones limpias,
/// y una sesión que no cerró bien no cuenta.
#[test]
fn el_respaldo_de_migracion_se_retira_tras_tres_sesiones_limpias() {
    use nb_books_lib::db::migrations::retirar_respaldo_si_ya_no_hace_falta;
    let dir = nb_books_lib::paths::temporal_de_pruebas().join("respaldo-retirado");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio");
    let ruta = dir.join("nb-books.sqlite");
    let db = Database::open(&ruta).expect("base");
    let respaldo = dir.join("nb-books.sqlite.v40.backup");
    std::fs::write(&respaldo, b"respaldo").expect("respaldo");
    let conn = db.connection().expect("conexión");
    conn.execute(
        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES ('sesiones_limpias_tras_migrar', '0')",
        [],
    )
    .expect("cuenta");

    assert!(!retirar_respaldo_si_ya_no_hace_falta(&conn, &ruta, true));
    assert!(!retirar_respaldo_si_ya_no_hace_falta(&conn, &ruta, false));
    assert!(!retirar_respaldo_si_ya_no_hace_falta(&conn, &ruta, true));
    assert!(respaldo.exists(), "dos sesiones limpias no bastan");
    assert!(retirar_respaldo_si_ya_no_hace_falta(&conn, &ruta, true));
    assert!(!respaldo.exists());
    drop(conn);
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}
