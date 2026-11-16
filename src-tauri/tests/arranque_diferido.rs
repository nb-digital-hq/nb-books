//! El arranque abre la ventana antes de trabajar.
//!
//! Lo que se comprueba aquí no es que el arranque sea rápido -eso depende de la
//! máquina y de la biblioteca-, sino que **el trabajo que crece con la
//! biblioteca no está en el camino de la ventana**. Es una propiedad de la
//! estructura, y por eso se puede comprobar con una base de prueba: si
//! `bootstrap` volviera a hacer el trabajo diferido, estas aserciones fallarían
//! aunque la base tuviera dos filas.
//!
//! La cifra que lo motiva está medida sobre el corpus real y vive en
//! `docs/architecture/performance-budgets.md`: nueve minutos y
//! trece segundos sin ninguna ventana en la pantalla.

use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::monitor::{components, ComponentState};

mod tempdir {
    use std::path::{Path, PathBuf};

    pub struct Guard(PathBuf);

    impl Guard {
        pub fn nuevo(nombre: &str) -> Self {
            let dir = nb_books_lib::paths::temporal_de_pruebas()
                .join(format!("nb-books-arranque-{nombre}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("directorio temporal");
            Self(dir)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

/// El estado de un componente en el monitor, o `None` si no lo publica.
fn estado_de(estado: &nb_books_lib::AppState, componente: &str) -> Option<ComponentState> {
    estado
        .monitor
        .snapshot()
        .into_iter()
        .find(|c| c.component == componente)
        .map(|c| c.state)
}

/// Da de alta una edición sin texto indexado, que es lo que la cola busca.
fn catalogar_sin_indexar(db: &nb_books_lib::db::Database) {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición");
}

/// Las tres aserciones van en una sola prueba porque comparten una variable de
/// entorno de proceso: dos pruebas en paralelo se pisarían el directorio de
/// datos, y aislarlas con un mutex escondería que dependen de lo mismo.
#[test]
fn lo_que_crece_con_la_biblioteca_no_esta_en_el_camino_de_la_ventana() {
    let guard = tempdir::Guard::nuevo("diferido");
    // SAFETY: esta prueba es el único binario que toca esta variable, y el
    // directorio es suyo y se borra al terminar.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, guard.path()) };

    let (estado, _telemetria) = nb_books_lib::bootstrap().expect("el runtime arranca");

    // Que la búsqueda esté arrancando, y no lista ni ausente, es lo que permite
    // a la interfaz decir "todavía no" en vez de mentir en cualquiera de las dos
    // direcciones.
    assert_eq!(
        estado_de(&estado, components::SEARCH),
        Some(ComponentState::Starting),
        "tras bootstrap el índice aún no se ha comprobado, así que la búsqueda no puede estar lista"
    );

    // La indexación se detiene antes de cebar la cola: si no, el propio
    // indexador la vaciaría mientras se comprueba, y la aserción de abajo
    // dependería de quién corre más.
    estado.indexer.pause();
    catalogar_sin_indexar(&estado.db);

    assert_eq!(
        estado.indexer.progress().pending,
        0,
        "bootstrap no puede haber recorrido la biblioteca buscando qué falta por indexar"
    );

    nb_books_lib::arranque_diferido(&estado);

    assert_eq!(
        estado_de(&estado, components::SEARCH),
        Some(ComponentState::Ready),
        "comprobado el índice, la búsqueda tiene que anunciarse lista"
    );
    assert_eq!(
        estado.indexer.progress().pending,
        1,
        "el trabajo diferido es quien ceba la cola con lo que quedó sin indexar"
    );
}
