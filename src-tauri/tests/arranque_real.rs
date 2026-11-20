//! Cuánto cuesta arrancar con la biblioteca real dentro.
//!
//! # Cómo se ejecuta
//!
//! Se omite sola si no se le dice dónde mirar, porque la base real solo existe
//! en la máquina de desarrollo y pesa 24 GB:
//!
//! ```text
//! NB_BOOKS_BASE_REAL=/ruta/a/una/copia cargo test --test arranque_real -- --nocapture
//! ```
//!
//! # Reglas, que no son negociables
//!
//! * **Sobre una copia, nunca sobre la base del usuario.** El arranque escribe
//!   -migraciones, reconciliación de portadas, reparación del índice-, y una
//!   medición no puede tocar los datos de nadie.
//! * **Nada sale de aquí**: lo que se imprime son milisegundos y recuentos. Ni
//!   rutas, ni títulos, ni autores. Ver `docs/audit/performance/testing.md`.

use std::time::Instant;

/// La variable dice dónde está el directorio base de la copia; la ruta no se
/// codifica aquí porque es personal y este repositorio se divide en uno público.
const BASE: &str = "NB_BOOKS_BASE_REAL";

#[test]
fn la_ventana_no_espera_a_lo_que_crece_con_la_biblioteca() {
    let Some(base) = std::env::var_os(BASE) else {
        eprintln!("sin {BASE}: se omite, la base real solo está en la máquina de desarrollo");
        return;
    };

    // SAFETY: este binario de prueba tiene una sola prueba, así que nadie más
    // lee esta variable mientras se escribe.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &base) };

    let reloj = Instant::now();
    let (estado, _telemetria) = nb_books_lib::bootstrap().expect("el runtime arranca");
    let arranque = reloj.elapsed();

    // Sin esto, cebar la cola con ciento treinta mil documentos pondría al
    // indexador a leer la biblioteca entera en mitad de la medición.
    estado.indexer.pause();

    let reloj = Instant::now();
    nb_books_lib::arranque_diferido(&estado);
    let diferido = reloj.elapsed();

    println!(
        "arranque hasta poder abrir la ventana: {} ms",
        arranque.as_millis()
    );
    println!(
        "trabajo diferido, con la ventana ya abierta: {} ms",
        diferido.as_millis()
    );
    println!(
        "documentos encolados: {}",
        estado.indexer.progress().pending
    );

    // El presupuesto de la ficha: la ventana en menos de dos segundos con
    // cualquier tamaño de biblioteca. El de `performance-budgets.md` es 200 ms,
    // y se comprueba aparte porque esa cifra es sobre la aplicación vacía.
    assert!(
        arranque < std::time::Duration::from_secs(2),
        "el arranque tardó {arranque:?} en dejar la ventana lista"
    );
}
