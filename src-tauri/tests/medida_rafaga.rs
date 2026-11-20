//! La ráfaga de arranque de la interfaz contra el pool de conexiones.
//!
//! Al arrancar, la interfaz pide 41 órdenes a la vez -contado con el simulador de la
//! suite el 2026-09-25-, y el runtime las atiende en paralelo, así que
//! el pool de cuatro conexiones pasó de límite de contención a **cola**: quien no
//! encuentra sitio espera, y a los cinco segundos recibe `STORAGE_FAILURE`. Esto
//! reproduce la ráfaga con las funciones que hay detrás de esas órdenes, contra una
//! copia de la base real, y mide cuánto espera cada una por su conexión.
//!
//! ```text
//! NB_BOOKS_COPIA_REAL=/una/copia.sqlite cargo test --release --test medida_rafaga -- --nocapture
//! ```
//!
//! **Sobre una copia**: `Database::open` migra, y la base del usuario no se escribe
//! fuera de la aplicación. Se omite sola sin la ruta.

use std::sync::Arc;
use std::time::Instant;

use nb_books_lib::browse::{self, BrowseQuery, ShelfKind, ShelfQuery};
use nb_books_lib::db::Database;

type Orden = (&'static str, fn(&rusqlite::Connection));

fn ordenes() -> Vec<Orden> {
    let mut lista: Vec<Orden> = vec![
        ("home_rows", |c| {
            nb_books_lib::home::filas(c, 7).expect("filas");
        }),
        ("reading_stats", |c| {
            nb_books_lib::home::lectura(c).expect("lectura");
        }),
        ("browse_library", |c| {
            browse::browse(c, &BrowseQuery::default()).expect("página");
        }),
        ("browse_library", |c| {
            browse::browse(c, &BrowseQuery::default()).expect("página");
        }),
        ("library_facets", |c| {
            browse::facets(c).expect("facetas");
        }),
        ("library_shelves", |c| {
            browse::shelves(
                c,
                &ShelfQuery {
                    kind: ShelfKind::Author,
                    search: String::new(),
                    limit: 20,
                },
            )
            .expect("estanterías");
        }),
        ("settings_snapshot", |c| {
            nb_books_lib::settings::snapshot(c).expect("preferencias");
        }),
        ("list_notes", |c| {
            nb_books_lib::notes::list(c, 50, 0).expect("notas");
        }),
        ("boards_list", |c| {
            nb_books_lib::boards::listar(c, 50, 0).expect("tableros");
        }),
        ("list_vocabulary", |c| {
            nb_books_lib::vocabulary::list(c, 50, 0).expect("vocabulario");
        }),
        ("last_import", |c| {
            nb_books_lib::imports::ultima(c).expect("importación");
        }),
        ("pending_import", |c| {
            nb_books_lib::imports::pendiente(c).expect("importación");
        }),
    ];
    // Las otras veintinueve son lecturas de una tabla pequeña: preferencias,
    // extensiones, catálogos, estado de la IA. Se representan con la más parecida.
    for _ in 0..29 {
        lista.push(("ligera", |c| {
            nb_books_lib::settings::snapshot(c).expect("preferencias");
        }));
    }
    lista
}

#[test]
fn la_rafaga_de_arranque_contra_el_pool() {
    let Some(ruta) = std::env::var_os("NB_BOOKS_COPIA_REAL") else {
        eprintln!("omitida: sin NB_BOOKS_COPIA_REAL");
        return;
    };
    let db = Arc::new(Database::open(std::path::Path::new(&ruta)).expect("la copia se abre"));

    for vuelta in 0..3 {
        let salida = Instant::now();
        let hilos: Vec<_> = ordenes()
            .into_iter()
            .map(|(nombre, orden)| {
                let db = Arc::clone(&db);
                std::thread::spawn(move || {
                    let pedida = Instant::now();
                    let conn = db.connection().expect("conexión");
                    let espera = pedida.elapsed();
                    orden(&conn);
                    (nombre, espera, pedida.elapsed())
                })
            })
            .collect();
        let mut resultados: Vec<_> = hilos.into_iter().map(|h| h.join().expect("hilo")).collect();
        let total = salida.elapsed();
        resultados.sort_by_key(|r| std::cmp::Reverse(r.1));
        let peor_espera = resultados[0].1;
        let mas_lenta = resultados.iter().max_by_key(|r| r.2).expect("hay");
        println!(
            "vuelta {vuelta}: 41 órdenes en {} ms; peor espera por conexión {} ms ({}); la más lenta {} ({} ms)",
            total.as_millis(),
            peor_espera.as_millis(),
            resultados[0].0,
            mas_lenta.0,
            mas_lenta.2.as_millis()
        );
    }
}
