//! Importación contra una biblioteca de verdad.
//!
//! **No corre en la suite normal**: está marcada `#[ignore]` y necesita que
//! `NB_BOOKS_BIBLIOTECA` apunte a una carpeta con documentos reales. Se ejecuta
//! a mano cuando hay que medir algo que el corpus generado no puede enseñar:
//!
//! ```bash
//! NB_BOOKS_BIBLIOTECA=/ruta/a/una/carpeta \
//!   cargo test --test importacion_real -- --ignored --nocapture
//! ```
//!
//! Existe porque tres defectos solo se vieron con datos reales: los títulos se
//! quedaban con el nombre del archivo, cada subcarpeta declaraba su propia raíz
//! de biblioteca, y la contención de escritura llenaba el registro de
//! «database is locked». Ninguno de los tres aparece con ocho archivos.
//!
//! **Nunca escribe en la carpeta de origen.** Importa en modo referenciado, que
//! solo registra rutas.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{self, CancelToken};

#[test]
#[ignore = "necesita una biblioteca real en NB_BOOKS_BIBLIOTECA"]
fn una_carpeta_real_entra_con_una_raiz_y_sus_titulos() {
    let Ok(origen) = std::env::var("NB_BOOKS_BIBLIOTECA") else {
        panic!("define NB_BOOKS_BIBLIOTECA con la carpeta a importar");
    };
    let origen = PathBuf::from(origen);

    // Sin dependencia nueva para un directorio temporal: la suite ya lo resuelve
    // así en `importacion.rs` y añadir un crate para esto sería lo que la
    // política de dependencias desaconseja.
    let temporal =
        nb_books_lib::paths::temporal_de_pruebas().join(format!("nb-real-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temporal);
    std::fs::create_dir_all(&temporal).expect("directorio temporal");

    let db = Database::open(&temporal.join("prueba.sqlite")).expect("base");
    let bus = EventBus::new();

    let portadas = temporal.join("portadas");
    std::fs::create_dir_all(&portadas).expect("portadas");
    nb_books_lib::metadata::subscribe(&bus, db.clone(), portadas);

    /*
     * El indexador también, porque es el que hace daño: indexar un documento
     * escribe cerca de novecientas filas en el índice de texto completo. Sin él
     * montado, esta prueba mediría un escenario que no existe.
     *
     * `NB_BOOKS_SIN_PAUSA=1` deja que indexe durante la importación, que es como
     * estaba antes: sirve para comprobar que la pausa hace algo.
     */
    let mut lectores = nb_books_lib::reader::ProviderRegistry::new();
    lectores.register(Box::new(
        nb_books_lib::reader::epub::EpubFactory::solo_texto(),
    ));
    // SAFETY: esta prueba usa su propio directorio temporal y lo borra al salir.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &temporal) };
    let rutas = nb_books_lib::paths::AppPaths::resolve().expect("rutas");
    let indexador = Arc::new(nb_books_lib::search::Indexer::start(
        db.clone(),
        Arc::new(lectores),
        bus.clone(),
        rutas,
    ));
    nb_books_lib::search::subscribe(&bus, db.clone(), Arc::clone(&indexador));

    let con_pausa = std::env::var("NB_BOOKS_SIN_PAUSA").is_err();
    if con_pausa {
        indexador.pause_for_import();
    }

    // Con `NB_BOOKS_LIMITE` se corta tras N archivos. Sirve para medir sobre la
    // biblioteca entera —incluido el coste de recorrer dieciocho mil carpetas—
    // sin esperar a los ciento cincuenta mil documentos.
    let limite: usize = std::env::var("NB_BOOKS_LIMITE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(usize::MAX);

    let cancel = CancelToken::new();
    let freno = cancel.clone();

    let avances = Arc::new(AtomicUsize::new(0));
    let contador = Arc::clone(&avances);
    bus.subscribe("cuenta-avances", move |evento| {
        if let nb_books_lib::events::DomainEvent::BulkImportProgress {
            processed, total, ..
        } = evento
        {
            assert!(*total > 0, "el avance tiene que decir el total");
            assert!(processed <= total, "no se puede procesar más de lo que hay");
            contador.fetch_add(1, Ordering::SeqCst);
            if *processed as usize >= limite {
                freno.cancel();
            }
        }
    });

    let mut conn = db.connection().expect("conexión");
    let ctx = library::ImportContext {
        scope: &origen,
        library: &temporal.join("biblioteca"),
        bus: &bus,
    };

    /*
     * Mientras la importación corre, un hilo pregunta a la base lo mismo que
     * pregunta la interfaz al pintar la biblioteca, y se queda con la respuesta
     * más lenta. Es la medida de "se congela": si la interfaz no consigue leer
     * mientras se escribe, se queda quieta.
     */
    let peor = Arc::new(std::sync::Mutex::new(std::time::Duration::ZERO));
    let parar = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let vigilante = {
        let db = db.clone();
        let peor = Arc::clone(&peor);
        let parar = Arc::clone(&parar);
        std::thread::spawn(move || {
            while !parar.load(Ordering::SeqCst) {
                let t = std::time::Instant::now();
                if let Ok(conn) = db.connection() {
                    let _: i64 = conn
                        .query_row("SELECT count(*) FROM editions", [], |r| r.get(0))
                        .unwrap_or(0);
                }
                let tardanza = t.elapsed();
                let mut guardia = peor.lock().unwrap_or_else(|p| p.into_inner());
                if tardanza > *guardia {
                    *guardia = tardanza;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        })
    };

    let comienzo = std::time::Instant::now();
    let informe = library::import_directory(
        &mut conn,
        &ctx,
        &origen,
        // Referenciado: **no se copia nada**, así que la carpeta de origen no se
        // toca de ninguna manera.
        StorageMode::Referenced,
        &cancel,
    )
    .expect("importación");
    let duracion = comienzo.elapsed();

    parar.store(true, Ordering::SeqCst);
    vigilante.join().expect("vigilante");
    if con_pausa {
        indexador.resume_after_import();
    }
    let peor_lectura = *peor.lock().unwrap_or_else(|p| p.into_inner());

    // Los metadatos se extraen en el bus, en su propio hilo: hay que dejarle
    // terminar antes de mirar los títulos. Se espera a que la cuenta deje de
    // subir en lugar de dormir un rato fijo, que con carpetas grandes se queda
    // corto y hace la prueba mentirosa.
    let mut anterior = -1_i64;
    for _ in 0..120 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let ahora: i64 = conn
            .query_row(
                r"SELECT count(*) FROM editions WHERE title NOT LIKE '%\_%' ESCAPE '\'",
                [],
                |r| r.get(0),
            )
            .expect("títulos");
        if ahora == anterior {
            break;
        }
        anterior = ahora;
    }

    let raices: i64 = conn
        .query_row("SELECT count(*) FROM library_roots", [], |r| r.get(0))
        .expect("raíces");
    let ediciones: i64 = conn
        .query_row("SELECT count(*) FROM editions", [], |r| r.get(0))
        .expect("ediciones");
    let con_guion: i64 = conn
        .query_row(
            r"SELECT count(*) FROM editions WHERE title LIKE '%\_%' ESCAPE '\'",
            [],
            |r| r.get(0),
        )
        .expect("títulos con guion bajo");

    println!(
        "procesados={} importados={} raices={raices} ediciones={ediciones} \
         titulos_con_guion_bajo={con_guion} avances={} en {duracion:?} \
         peor_lectura={peor_lectura:?}",
        informe.processed,
        informe.imported,
        avances.load(Ordering::SeqCst)
    );

    /*
     * **Lo que quedó fuera, y por qué**: códigos y recuentos, nunca rutas.
     *
     * Es la mitad del criterio que exige una importación real: ningún
     * `STORAGE_FAILURE` después de los reintentos. La del 2026-09-09 dejó 1.207.
     *
     * No se comprueba que los procesados sean la suma de lo demás: lo son por
     * construcción, y una comprobación que no puede fallar no demuestra nada. Lo
     * que sí puede esconder archivos es el recorrido de la carpeta, y eso se compara
     * fuera de aquí, contra lo que hay en el disco.
     */
    let mut por_codigo = std::collections::BTreeMap::<&str, usize>::new();
    for rechazo in &informe.rejected {
        *por_codigo.entry(rechazo.code).or_default() += 1;
    }
    println!(
        "ya_estaban={} rechazados={} cancelada={} rechazos_por_codigo={por_codigo:?}",
        informe.already_in_library,
        informe.rejected.len(),
        informe.cancelled
    );
    assert!(
        !por_codigo.contains_key("STORAGE_FAILURE"),
        "quedaron {} documentos fuera por almacenamiento después de reintentar",
        por_codigo.get("STORAGE_FAILURE").copied().unwrap_or(0)
    );

    /*
     * Presupuesto de fotograma: 16 ms. Se admite hasta un cuarto de segundo,
     * que es lo que separa "va lento" de "se ha colgado". Antes de arreglarlo,
     * el registro de una sesión real acumuló diecisiete «timed out waiting for
     * connection»: cinco segundos enteros sin poder leer.
     */
    assert!(
        peor_lectura < std::time::Duration::from_millis(250),
        "la interfaz tardó {peor_lectura:?} en leer mientras se importaba"
    );

    /*
     * Y un documento real se abre y **trae texto**. Es la comprobación que
     * ninguna prueba con el corpus generado puede hacer: los EPUB de
     * `tests/fixtures` los escribimos nosotros, así que solo demuestran que
     * sabemos leer lo que sabemos escribir.
     */
    let (edition_id, file_id): (String, String) = conn
        .query_row(
            "SELECT e.id, f.id FROM editions e
               JOIN files f ON f.edition_id = e.id
              WHERE f.media_type = 'application/epub+zip'
              LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("un EPUB importado");

    let mut lectores_lectura = nb_books_lib::reader::ProviderRegistry::new();
    lectores_lectura.register(Box::new(
        nb_books_lib::reader::epub::EpubFactory::solo_texto(),
    ));
    let ruta: String = conn
        .query_row(
            &format!(
                "SELECT {} FROM files f WHERE f.id = ?1",
                nb_books_lib::db::catalog::SQL_RUTA_ABSOLUTA
            ),
            [&file_id],
            |r| r.get(0),
        )
        .expect("ruta del archivo");

    let proveedor = lectores_lectura
        .open("application/epub+zip", std::path::Path::new(&ruta))
        .expect("proveedor");
    let navegacion = proveedor.navigation().expect("índice");
    assert!(
        !navegacion.entries.is_empty(),
        "un EPUB real tiene que traer índice"
    );

    // Se pasa por `resolve` a propósito: es el camino real, el que traduce un
    // locator a una posición nativa y avisa con qué precisión lo consiguió.
    let resuelta = proveedor
        .resolve(&navegacion.entries[0].locator)
        .expect("resolución");
    let pintado = proveedor.render(&resuelta.position).expect("marcado");
    let largo = match &pintado {
        nb_books_lib::reader::Rendered::Markup { html, .. } => html.len(),
        nb_books_lib::reader::Rendered::Image { .. } => 0,
    };
    println!(
        "edicion={edition_id} capitulos={} html={largo}",
        navegacion.entries.len()
    );
    // Nada del texto del libro sale de aquí: es material con derechos vigentes, y
    // `AGENTS.md`, sección 5, no deja que llegue ni a un registro. Hasta el
    // 2026-09-11 se imprimían sus primeros cuatrocientos caracteres.
    /*
     * Y lo que de verdad importa: **abrir la sesión deja texto delante**. La
     * primera entrada del índice suele ser la cubierta —186 caracteres de
     * marcado, ninguna palabra—, y abrir ahí dejaba al lector con una página en
     * blanco. Se comprueba el camino completo, no el proveedor suelto.
     */
    let sesion = nb_books_lib::reader::ReadingSession::open(
        &db,
        &lectores_lectura,
        &file_id,
        Arc::new(nb_books_lib::reader::progress::ProgressTracker::new(
            db.clone(),
        )),
        None,
    )
    .expect("sesión");

    let al_abrir = sesion.render_at(&sesion.info().locator).expect("marcado");
    let visible = match &al_abrir {
        nb_books_lib::reader::Rendered::Markup { html, .. } => {
            let mut dentro = false;
            html.chars()
                .filter(|c| {
                    match c {
                        '<' => dentro = true,
                        '>' => dentro = false,
                        _ => {}
                    }
                    !dentro && *c != '>' && !c.is_whitespace()
                })
                .count()
        }
        nb_books_lib::reader::Rendered::Image { .. } => usize::MAX,
    };
    println!("al abrir hay {visible} caracteres de texto visible");
    assert!(
        visible > 200,
        "abrir un documento real dejó {visible} caracteres a la vista"
    );

    // Una carpeta elegida es **una** raíz. La administrada existe siempre.
    assert!(
        raices <= 2,
        "una carpeta importada declaró {raices} raíces; debía declarar una"
    );

    // El título sale de los metadatos del documento, no del nombre del archivo.
    // Se admite que algunos no traigan metadatos legibles; lo que no se admite
    // es que sea la norma.
    if ediciones > 0 {
        let proporcion = (con_guion as f64) / (ediciones as f64);
        assert!(
            proporcion < 0.5,
            "el {:.0}% de los títulos sigue siendo el nombre del archivo",
            proporcion * 100.0
        );
    }
}
