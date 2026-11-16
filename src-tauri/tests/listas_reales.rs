//! Lo que cruza el puente no crece con la biblioteca.
//!
//! # Cómo se ejecuta
//!
//! ```text
//! NB_BOOKS_BASE_REAL=/ruta/a/una/copia cargo test --test listas_reales -- --nocapture
//! ```
//!
//! Se omite sola si no se le dice dónde mirar. Y mira **una copia**, nunca la
//! base del usuario: abrirla aplica migraciones, y una medición no toca los
//! datos de nadie.
//!
//! # Qué demuestra, y qué no
//!
//! De las cinco listas acotadas, la base real solo ejerce una con
//! volumen: tiene 1.219 rechazos de importación y ninguna anotación, ninguna
//! nota y ninguna palabra de vocabulario, porque se importó y no se ha leído
//! sobre ella. Las otras cuatro se comprueban con bases de prueba, donde el
//! tramo se puede fabricar. Decirlo importa: una prueba que se ejecuta sobre
//! datos que no ejercen lo que mide da una confianza que no ha ganado.
//!
//! Lo que sí ejerce esta base, y ninguna otra puede, es el recorrido y las
//! estanterías sobre 151.411 ediciones.
//!
//! **Nada sale de aquí**: lo que se imprime son recuentos.

use nb_books_lib::browse::{self, BrowseQuery, ShelfKind, ShelfQuery};
use nb_books_lib::db::Database;

const BASE: &str = "NB_BOOKS_BASE_REAL";

#[test]
fn ninguna_respuesta_trae_mas_filas_de_las_que_se_le_piden() {
    let Some(base) = std::env::var_os(BASE) else {
        eprintln!("sin {BASE}: se omite, la base real solo está en la máquina de desarrollo");
        return;
    };

    let ruta = std::path::PathBuf::from(&base)
        .join("data")
        .join("nb-books.sqlite");
    let db = Database::open(&ruta).expect("la copia de la base real se abre");
    let conn = db.connection().expect("conexión");

    // El recorrido de la biblioteca, que es la lista más grande que existe.
    let pagina = browse::browse(
        &conn,
        &BrowseQuery {
            limit: 50,
            ..BrowseQuery::default()
        },
    )
    .expect("recorrido");
    println!(
        "recorrido: {} filas de un catálogo de {}",
        pagina.items.len(),
        pagina.total
    );
    assert!(pagina.items.len() <= 50);
    assert!(
        pagina.total > 100_000,
        "esta comprobación no significa nada sin la biblioteca real detrás: \
         solo había {} ediciones",
        pagina.total
    );

    // Las estanterías: decenas de miles de autores detrás de una respuesta.
    //
    // Se cronometra porque el criterio es el presupuesto de fotograma,
    // y porque antes de V24 esto costaba 4.141 ms en el panel de autores.
    for clase in [ShelfKind::Author, ShelfKind::Series, ShelfKind::Subject] {
        let pedir = || {
            let reloj = std::time::Instant::now();
            let pagina = browse::shelves(
                &conn,
                &ShelfQuery {
                    kind: clase,
                    search: String::new(),
                    limit: 20,
                },
            )
            .expect("estanterías");
            (pagina, reloj.elapsed())
        };

        // Se miden las dos y se enseñan las dos. La primera lectura trae del
        // disco las páginas del índice y no vuelve a pagarse; esconderla sería
        // el mismo error que medir con la caché caliente y llamarlo frío, que ya
        // pasó una vez. El presupuesto de fotograma se le exige a la
        // segunda, que es la que ocurre mientras alguien usa la aplicación.
        let (_, en_frio) = pedir();
        let (pagina, en_caliente) = pedir();

        println!(
            "estanterías {:?}: {} filas de {} agrupaciones, {} ms en frío y {} ms después",
            clase,
            pagina.shelves.len(),
            pagina.total,
            en_frio.as_millis(),
            en_caliente.as_millis()
        );
        assert!(pagina.shelves.len() <= 20);
        assert!(
            en_caliente < std::time::Duration::from_millis(16),
            "abrir el panel {clase:?} costó {en_caliente:?}, por encima del presupuesto de fotograma"
        );
    }

    // Los rechazos de la última importación: 1.219 en la base real.
    let ultima = nb_books_lib::imports::ultima(&conn).expect("consulta");
    if let Some(ejecucion) = ultima {
        let tramo = nb_books_lib::imports::rechazos(&conn, &ejecucion.id, 50, 0).expect("rechazos");
        println!(
            "rechazos: {} filas de {} que dejó fuera la importación",
            tramo.len(),
            ejecucion.rejected
        );
        assert!(
            tramo.len() <= 50,
            "la respuesta creció con la importación en lugar de con lo que se pidió"
        );
    }
}
