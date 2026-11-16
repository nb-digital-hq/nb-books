//! El indexador frente al presupuesto de disco: qué se indexa primero y qué no
//! espera. Respuesta a la.
//!
//! En su propio binario porque llena la caché para pasarse del presupuesto, y la
//! carpeta de datos de las pruebas es de todo el proceso: dentro de
//! `indexacion.rs` pararía a las demás pruebas.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nb_books_lib::db::catalog::{
    ContentHash, Edition, File, NewEdition, NewFile, NewWork, StorageMode, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::ProviderRegistry;
use nb_books_lib::search::{self, Indexer, SearchStatus};

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-presupuesto-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio");
    dir
}

fn libro(dir: &Path, n: usize) -> PathBuf {
    let origen = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimo-valido.epub");
    let destino = dir.join(format!("libro-{n}.epub"));
    std::fs::copy(origen, &destino).expect("copia");
    destino
}

/// Una edición con su archivo. Cada archivo **declara un byte**: lo derivado
/// queda muy por encima de cualquier presupuesto sin fabricar gigas de caché.
fn alta(db: &Database, ruta: &Path, n: usize) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: format!("obra {n}"),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: format!("edicion {n}"),
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
    let (root_id, relative_path) =
        nb_books_lib::library::ensure_root(&conn, ruta, StorageMode::Referenced, None)
            .expect("raíz");
    File::create(
        &conn,
        &NewFile {
            edition_id: edicion.id.clone(),
            root_id,
            relative_path,
            content_hash: ContentHash::parse(&format!("sha256:{:064x}", n + 1)).expect("hash"),
            byte_size: 1,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )
    .expect("archivo");
    edicion.id
}

fn estado(db: &Database, id: &str) -> SearchStatus {
    let conn = db.connection().expect("conexión");
    search::status_of(&conn, id).expect("estado")
}

/// Todo en una prueba y no en varias: las dos mitades comparten la carpeta de
/// datos del proceso, y en paralelo una pisaría la caché de la otra.
#[test]
fn lo_abierto_se_indexa_aunque_no_quepa_y_lo_leido_va_primero() {
    let dir = temporal("prioridad");
    // SAFETY: este binario tiene una sola prueba, y la carpeta es suya.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, dir.join("datos")) };
    let rutas = nb_books_lib::paths::AppPaths::resolve().expect("rutas");
    let db = Database::open(&rutas.database()).expect("base");

    let libros: Vec<String> = (0..3).map(|n| alta(&db, &libro(&dir, n), n)).collect();
    {
        let conn = db.connection().expect("conexión");
        conn.execute_batch(&format!(
            "INSERT INTO collections (id, name, kind) VALUES ('c', 'Tesis', 'manual');
             INSERT INTO collection_items (collection_id, edition_id) VALUES ('c', '{}');",
            libros[2]
        ))
        .expect("colección");
    }

    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(EpubFactory::solo_texto()));
    let indexador = Indexer::start(db.clone(), Arc::new(registro), EventBus::new(), rutas);

    // --- El orden: lo de una colección antes que lo demás ---
    indexador.pause();
    let conn = db.connection().expect("conexión");
    assert_eq!(indexador.enqueue_missing(&conn).expect("encolar"), 3);
    drop(conn);
    assert_eq!(
        indexador.cola_pendiente().first(),
        Some(&libros[2]),
        "lo que está en una colección va antes que lo que no"
    );

    // --- Sin sitio, lo de la cola espera ---
    indexador.resume();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(estado(&db, &libros[0]), SearchStatus::Pending);
    assert_eq!(
        indexador.progress().pause_reason,
        Some(search::PAUSA_PRESUPUESTO)
    );
    assert!(indexador.medida().is_some(), "la medida queda a la vista");

    // --- Y lo que se abre, no ---
    indexador.enqueue_prioritario(&libros[1]);
    let comienzo = Instant::now();
    while estado(&db, &libros[1]) != SearchStatus::Searchable
        && comienzo.elapsed() < Duration::from_secs(15)
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        estado(&db, &libros[1]),
        SearchStatus::Searchable,
        "lo que se abrió no se indexó por falta de sitio"
    );
    assert_eq!(
        estado(&db, &libros[0]),
        SearchStatus::Pending,
        "lo que no se abrió siguió esperando"
    );
}
