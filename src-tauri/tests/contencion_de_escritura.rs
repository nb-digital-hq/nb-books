//! Dos escritores a la vez no pueden perder ninguna escritura.
//!
//! # El defecto que esto reproduce
//!
//! En modo WAL, una transacción **diferida** que empieza leyendo y después
//! escribe tiene que ascender su bloqueo de lectura a bloqueo de escritura. Si
//! entre medias otro escritor confirmó algo, SQLite devuelve `SQLITE_BUSY` **de
//! inmediato y sin respetar `busy_timeout`**, porque reintentar dentro de la
//! misma transacción daría una lectura incoherente.
//!
//! Pasó dos veces. La primera: 12.156 «database is locked» en una
//! sola sesión, con los títulos quedándose con el nombre del archivo porque los
//! metadatos no llegaban a aplicarse. La segunda entró por `boards.rs`, que
//! abría un `BEGIN` a secas -que es `BEGIN DEFERRED`-, y la importación real del
//! 2026-09-09 perdió **1.207 documentos** con `STORAGE_FAILURE`.
//!
//! # Por qué esta prueba usa un archivo y no una base en memoria
//!
//! Porque lo que se está probando **es** el modo WAL, y WAL es una propiedad del
//! archivo. Una base en memoria no lo tiene, así que la contención que produce
//! el defecto no puede ocurrir: la prueba pasaría siempre, incluso con el
//! `BEGIN` diferido puesto otra vez.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use nb_books_lib::db::Database;

/// Cuántas veces se mueve la nota mientras el otro hilo escribe.
///
/// Con el defecto puesto, la primera o la segunda vuelta ya falla; el resto son
/// margen para que no dependa de quién arranque antes.
const VUELTAS: usize = 40;

/// Cuántas veces confirma el otro escritor como mínimo.
///
/// Es un suelo y no un tope: sigue escribiendo hasta que el que mueve termine,
/// pero no para antes de esto aunque se lo pidan. Sin ese suelo, la prueba
/// podría no llegar a solaparse con nada.
const ESCRITURAS: usize = 50;

struct Temporal(std::path::PathBuf);

impl Temporal {
    fn nuevo() -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-contencion-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn mover_una_nota_mientras_otro_escribe_no_pierde_la_escritura() {
    let temporal = Temporal::nuevo();
    let db = Database::open(&temporal.0.join("contencion.sqlite")).expect("base en archivo");

    let (tablero, columnas, tarjeta) = {
        let mut conn = db.connection().expect("conexión");
        let tablero = nb_books_lib::boards::crear(&mut conn, "Tablero", &["A".into(), "B".into()])
            .expect("tablero");
        let nota = nb_books_lib::notes::create(&conn, "Nota", "cuerpo").expect("nota");
        let columnas: Vec<String> = nb_books_lib::boards::contenido(&conn, &tablero.id)
            .expect("columnas")
            .into_iter()
            .map(|c| c.id)
            .collect();
        nb_books_lib::boards::poner_nota(&mut conn, &tablero.id, &columnas[0], &nota.id)
            .expect("colocada");
        let tarjeta = nb_books_lib::boards::contenido(&conn, &tablero.id).expect("contenido")[0]
            .cards[0]
            .id
            .clone();
        (tablero.id, columnas, tarjeta)
    };

    /*
     * El otro escritor: confirma sin parar, que es lo que hacen la importación,
     * la indexación y la extracción de metadatos mientras alguien usa la
     * aplicación.
     *
     * **Los dos lados están acotados, y los dos tienen un mínimo.** La primera
     * versión de esta prueba dejaba al otro escritor corriendo hasta que el que
     * mueve terminara, y el que mueve seguía hasta que el otro terminara: con el
     * bloqueo de escritura yendo y viniendo, ninguno de los dos avanzaba y la
     * prueba se colgaba. La versión anterior a esa tenía el defecto contrario
     * -mover cuarenta veces terminaba antes de que el otro llegara a escribir
     * una- y pasaba en verde **sin haber medido ninguna contención**, que es
     * peor. De ahí la barrera y los dos mínimos.
     */
    let barrera = Arc::new(std::sync::Barrier::new(2));
    let parar = Arc::new(AtomicBool::new(false));

    let db_del_otro = db.clone();
    let barrera_del_otro = Arc::clone(&barrera);
    let señal = Arc::clone(&parar);
    let otro = std::thread::spawn(move || {
        let conn = db_del_otro
            .connection()
            .expect("conexión del otro escritor");
        barrera_del_otro.wait();

        let mut escritas = 0_usize;
        loop {
            conn.execute(
                "INSERT OR REPLACE INTO app_metadata (key, value) VALUES ('contencion', ?1)",
                [escritas.to_string()],
            )
            .expect("el otro escritor no puede fallar: espera su turno");
            escritas += 1;

            if escritas >= ESCRITURAS && señal.load(Ordering::Acquire) {
                break;
            }
        }
        escritas
    });

    let mut conn = db.connection().expect("conexión");
    barrera.wait();

    for vuelta in 0..VUELTAS {
        // Mover de una columna a otra lee antes de escribir, que es justo lo que
        // una transacción diferida no aguanta en WAL.
        let destino = &columnas[(vuelta + 1) % 2];
        nb_books_lib::boards::mover_tarjeta(&mut conn, &tablero, &tarjeta, destino, 0)
            .unwrap_or_else(|error| {
                panic!(
                    "la vuelta {vuelta} falló con {}: la transacción se abrió en diferido y \
                 SQLITE_BUSY no espera a `busy_timeout`",
                    error.code()
                )
            });
    }

    parar.store(true, Ordering::Release);
    let escritas = otro.join().expect("el otro escritor termina");

    assert!(
        escritas >= ESCRITURAS,
        "no hubo contención que medir: solo {escritas} escrituras del otro hilo"
    );

    // Y la nota sigue donde la dejó la última vuelta, que es lo que de verdad
    // importa: no perder de sitio lo que alguien colocó.
    let tarjetas: usize = nb_books_lib::boards::contenido(&conn, &tablero)
        .expect("contenido")
        .iter()
        .map(|c| c.cards.len())
        .sum();
    assert_eq!(tarjetas, 1, "la nota se duplicó o se perdió por el camino");
}
