//! **El criterio de aceptación:** una base corrupta produce un
//! arranque en modo recuperación, **nunca** una pantalla en blanco ni una
//! pérdida silenciosa.
//!
//! Son tres cosas distintas y las tres se comprueban aquí:
//!
//! * **Arranca.** Antes, una base dañada hacía que el arranque
//!   devolviera un error y `run()` lo convertía en un `expect`: el proceso moría
//!   antes de abrir ninguna ventana. Para quien lo sufría, la aplicación "no
//!   abría".
//! * **No pierde nada.** El archivo dañado se aparta con marca de tiempo, no se
//!   borra y no se intenta reparar. Puede ser la única copia de una biblioteca de
//!   años, y una reparación automática sobre el único archivo que le queda a
//!   alguien es cómo se convierte un problema recuperable en uno definitivo.
//! * **Lo dice.** Devuelve un informe con el motivo y con dónde quedó la copia.
//!   Sin él, el usuario vería su biblioteca vacía sin explicación, que es la
//!   pérdida silenciosa que el criterio prohíbe igual que la pantalla en blanco.
//!
//! Contrato: `docs/developer/testing/chaos.md`, escenario 3.

use std::io::Write;
use std::path::PathBuf;

use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::recovery::open_or_recover;

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-rec-{nombre}-{}", std::process::id()));
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

/// Deja una base legítima con contenido y luego le destroza la cabecera.
fn base_corrupta(temporal: &Temporal) -> u64 {
    let ruta = temporal.base();
    {
        let db = Database::open(&ruta).expect("base inicial");
        let conn = db.connection().expect("conexión");
        conn.execute(
            "INSERT INTO works (id, canonical_title) VALUES ('w1', 'Cien Años de Soledad')",
            [],
        )
        .expect("contenido del usuario");
    }
    let tamano = std::fs::metadata(&ruta).expect("metadatos").len();

    let mut archivo = std::fs::OpenOptions::new()
        .write(true)
        .open(&ruta)
        .expect("archivo escribible");
    archivo
        .write_all(b"ESTO-NO-ES-SQLITE-EN-ABSOLUTO")
        .expect("escritura");

    tamano
}

/// **El criterio.**
#[test]
fn una_base_corrupta_arranca_en_modo_recuperacion() {
    let temporal = Temporal::nuevo("corrupta");
    base_corrupta(&temporal);

    let (db, informe) = open_or_recover(&temporal.base()).expect("arranca igual");

    // 1. Arranca, y con una base utilizable.
    let conn = db.connection().expect("conexión");
    let obras: i64 = conn
        .query_row("SELECT count(*) FROM works", [], |f| f.get(0))
        .expect("la base nueva funciona");
    assert_eq!(obras, 0, "la base nueva está vacía, que es lo esperable");

    // 2. Lo dice, con el motivo y con dónde quedó la copia.
    let informe = informe.expect("tiene que haber informe: la biblioteca no es la que había");
    assert_eq!(informe.reason_code, "STORAGE_FAILURE");
    assert!(
        informe
            .isolated_as
            .starts_with("nb-books.sqlite.corrupted-"),
        "el informe dice dónde quedó: {}",
        informe.isolated_as
    );
}

/// **Nada se pierde.** El archivo dañado sigue ahí, entero.
#[test]
fn el_archivo_danado_se_aparta_y_no_se_borra() {
    let temporal = Temporal::nuevo("apartado");
    base_corrupta(&temporal);

    let (_db, informe) = open_or_recover(&temporal.base()).expect("arranca");
    let informe = informe.expect("informe");

    let apartado = temporal.0.join(&informe.isolated_as);
    assert!(
        apartado.exists(),
        "el archivo del usuario tiene que seguir existiendo: puede ser su única copia"
    );
    assert!(
        std::fs::metadata(&apartado).expect("metadatos").len() > 0,
        "y con su contenido, no vacío"
    );
}

/// Dos corrupciones seguidas no se pisan.
///
/// Sin marca de tiempo, la segunda se comería a la primera, que probablemente es
/// la copia más completa.
#[test]
fn dos_recuperaciones_seguidas_conservan_las_dos_copias() {
    let temporal = Temporal::nuevo("dos");

    base_corrupta(&temporal);
    let (db, primero) = open_or_recover(&temporal.base()).expect("primera");
    let primero = primero.expect("informe");
    // Se suelta antes de volver a corromper: con la base abierta, SQLite sirve
    // páginas de su propia caché y no llega a ver el archivo destrozado.
    drop(db);

    // La marca es en segundos: se espera para que la segunda no coincida.
    std::thread::sleep(std::time::Duration::from_millis(1_100));

    base_corrupta(&temporal);
    let (db, segundo) = open_or_recover(&temporal.base()).expect("segunda");
    let segundo = segundo.expect("informe");
    drop(db);

    assert_ne!(
        primero.isolated_as, segundo.isolated_as,
        "la segunda copia no puede comerse a la primera"
    );
    assert!(temporal.0.join(&primero.isolated_as).exists());
    assert!(temporal.0.join(&segundo.isolated_as).exists());
}

/// Un arranque normal **no** deja informe.
///
/// Un informe donde no pasó nada enseñaría a ignorar los informes, que es cómo
/// se pierde el que importa.
#[test]
fn un_arranque_normal_no_dice_nada() {
    let temporal = Temporal::nuevo("normal");

    let (_db, informe) = open_or_recover(&temporal.base()).expect("arranque limpio");
    assert!(informe.is_none());

    // Y al volver a abrirla, tampoco.
    let (_db, informe) = open_or_recover(&temporal.base()).expect("segunda apertura");
    assert!(informe.is_none());
}

/// Una base **del futuro** no se aparta.
///
/// Es quien abre su biblioteca con una versión antigua de la aplicación. La
/// biblioteca está perfectamente: la vieja es la aplicación. Apartarla sería
/// castigar a alguien por haber abierto el programa equivocado, y dejarle la
/// biblioteca en un archivo con nombre raro.
#[test]
fn una_base_mas_nueva_que_la_aplicacion_no_se_aparta() {
    let temporal = Temporal::nuevo("futuro");
    let ruta = temporal.base();

    {
        let db = Database::open(&ruta).expect("base");
        let conn = db.connection().expect("conexión");
        let futura = db.schema_version().expect("versión") + 5;
        conn.pragma_update(None, "user_version", futura)
            .expect("versión del futuro");
    }

    let resultado = open_or_recover(&ruta);
    assert!(matches!(resultado, Err(NbError::SchemaMismatch)));

    assert!(ruta.exists(), "la base sigue en su sitio, con su nombre");
    let apartados: Vec<_> = std::fs::read_dir(&temporal.0)
        .expect("directorio")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("corrupted"))
        .collect();
    assert!(
        apartados.is_empty(),
        "no se aparta nada: la biblioteca está bien, la vieja es la aplicación"
    );
}

/// Una base **bloqueada por otro proceso** no se aparta.
///
/// Si dos procesos corren a la vez (o si el usuario abrió dos instancias), la
/// contención produce un fallo de almacenamiento (`SQLITE_BUSY` / `database is locked`).
/// La biblioteca está intacta: apartarla sería arrebatarle sus datos al usuario.
#[test]
fn una_base_bloqueada_no_se_aparta() {
    let temporal = Temporal::nuevo("bloqueada");
    let ruta = temporal.base();

    let primera = Database::open(&ruta).expect("base inicial");
    let conn = primera.connection().expect("conexión");
    conn.execute_batch("PRAGMA user_version = 0; BEGIN EXCLUSIVE;")
        .expect("bloqueo exclusivo");

    // Durante el bloqueo exclusivo, intentar abrir con un timeout corto falla
    // por contención, pero la base NO está dañada.
    // Para no esperar 5 segundos del pool en la prueba, Database::open fallará
    // al vencer el busy_timeout de r2d2/sqlite.
    let resultado = open_or_recover(&ruta);
    conn.execute_batch("COMMIT;").expect("libera bloqueo");

    assert!(
        matches!(resultado, Err(NbError::StorageFailure)),
        "debe devolver error de almacenamiento sin entrar en recuperación"
    );

    assert!(ruta.exists(), "la base sigue en su sitio");
    let apartados: Vec<_> = std::fs::read_dir(&temporal.0)
        .expect("directorio")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("corrupted"))
        .collect();
    assert!(
        apartados.is_empty(),
        "no se aparta nada: la base no está corrupta, solo estaba ocupada"
    );
}
