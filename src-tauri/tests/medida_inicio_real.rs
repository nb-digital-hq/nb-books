//! Lo que cuesta componer Inicio sobre la biblioteca real, en solo lectura.
//!
//! La medida de las filas de Inicio sin el índice de V29, que la base real no tenía, y
//! dejó escrito que con él se esperaba entrar por índice sin haberlo cronometrado.
//! Ahora la base real está en V38 y lo tiene. Se omite sola sin la ruta:
//!
//! ```text
//! NB_BOOKS_BASE_REAL=~/.local/share/nb-books/nb-books.sqlite \
//!   cargo test --release --test medida_inicio_real -- --nocapture
//! ```
//!
//! **Se abre en solo lectura**, con `SQLITE_OPEN_READ_ONLY`, y no por `Database::open`,
//! que migraría: la base del usuario no se escribe fuera de la aplicación. Lo que se
//! imprime son milisegundos y recuentos, ni un título.

use std::time::Instant;

use rusqlite::{Connection, OpenFlags};

#[test]
fn las_filas_de_inicio_sobre_la_base_real() {
    let Some(ruta) = std::env::var_os("NB_BOOKS_BASE_REAL") else {
        eprintln!("omitida: sin NB_BOOKS_BASE_REAL");
        return;
    };
    let conn = Connection::open_with_flags(
        ruta,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("la base real se abre en solo lectura");

    let mut tiempos = Vec::new();
    let mut filas = 0;
    // La primera en frío y cuatro más: la semilla cambia, que es lo que pasa al volver.
    for semilla in [7_u64, 8, 9, 10, 11] {
        let inicio = Instant::now();
        let resultado = nb_books_lib::home::filas(&conn, semilla).expect("filas");
        tiempos.push(inicio.elapsed().as_secs_f64() * 1000.0);
        filas = resultado.len();
    }
    let inicio = Instant::now();
    nb_books_lib::home::lectura(&conn).expect("lectura");
    let lectura = inicio.elapsed().as_secs_f64() * 1000.0;

    println!(
        "inicio real: {filas} filas; primera composición {:.0} ms; siguientes {:?} ms; resumen de lectura {:.0} ms",
        tiempos[0],
        tiempos[1..].iter().map(|t| t.round()).collect::<Vec<_>>(),
        lectura
    );
}
