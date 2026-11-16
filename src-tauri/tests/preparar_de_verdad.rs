//! Preparar un capítulo **de verdad**: el `Preparador` real, la voz Kokoro real y un
//! EPUB real, por el mismo camino que la aplicación.
//!
//! # Por qué existe
//!
//! Porque las pruebas de `audio_preparado.rs` usan una voz simulada, y la revisión
//! de la fase del 2026-09-22 encontró que en la aplicación preparar un capítulo real
//! **fallaba en el primer párrafo largo**: el modelo admite 512 fichas por llamada y
//! nadie troceaba. Con la voz simulada eso no podía verse nunca. Esto es lo que lo
//! habría visto.
//!
//! # Cómo se ejecuta
//!
//! Se salta sola si no se le dan las dos cosas, que solo están en la máquina de
//! desarrollo:
//!
//! ```text
//! NB_VOZ_MODELOS=<carpeta con kokoro-82m/> NB_EPUB_REAL=<un EPUB> \
//!   cargo test --release --test preparar_de_verdad -- --nocapture
//! ```
//!
//! **Nada del libro sale de aquí**: se imprimen cifras, no títulos, rutas ni texto.
//! El audio se escribe en un directorio temporal y se borra al terminar.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use nb_books_lib::ai::generacion::Documentos;
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbResult;
use nb_books_lib::events::EventBus;
use nb_books_lib::reader::DocumentProvider;
use nb_books_lib::voice::neuronal::{Sintetizador, VozNeuronal};
use nb_books_lib::voice::preparacion::{Alcance, Preparador};

struct UnEpub(PathBuf);

impl Documentos for UnEpub {
    fn abrir(&self, _edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        Ok(Box::new(nb_books_lib::reader::epub::EpubDocument::open(
            &self.0,
        )?))
    }
}

/// El capítulo más corto que tenga al menos un fragmento de más de mil caracteres:
/// lo bastante largo para pasar del límite del modelo, y lo bastante corto para que
/// la prueba no tarde una hora.
fn capitulo_con_fragmento_largo(epub: &Path) -> Option<(String, usize)> {
    let documento = nb_books_lib::reader::epub::EpubDocument::open(epub).ok()?;
    let fragmentos = documento
        .text_chunks(nb_books_lib::search::FRAGMENTO)
        .ok()?;
    let mut por_recurso: Vec<(String, usize, usize)> = Vec::new();
    for (locator, texto) in &fragmentos {
        let largo = texto.chars().count();
        match por_recurso.iter_mut().find(|(h, _, _)| *h == locator.href) {
            Some((_, total, mayor)) => {
                *total += largo;
                *mayor = (*mayor).max(largo);
            }
            None => por_recurso.push((locator.href.clone(), largo, largo)),
        }
    }
    por_recurso
        .into_iter()
        .filter(|(_, total, mayor)| *mayor > 1_000 && *total < 4_000)
        .min_by_key(|(_, total, _)| *total)
        .map(|(href, total, _)| (href, total))
}

#[test]
fn un_capitulo_real_se_prepara_entero_con_la_voz_real() {
    let (Some(modelos), Some(epub)) = (
        std::env::var_os("NB_VOZ_MODELOS").map(PathBuf::from),
        std::env::var_os("NB_EPUB_REAL").map(PathBuf::from),
    ) else {
        eprintln!("sin NB_VOZ_MODELOS y NB_EPUB_REAL: se salta");
        return;
    };
    let Some((href, caracteres)) = capitulo_con_fragmento_largo(&epub) else {
        eprintln!("ese EPUB no tiene un capítulo corto con un fragmento largo: se salta");
        return;
    };

    let destino = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-preparar-real-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&destino);
    std::fs::create_dir_all(&destino).expect("carpeta");
    let db = Database::open(&destino.join("nb-books.sqlite")).expect("base");
    let edicion = {
        let conn = db.connection().expect("conexión");
        let obra = Work::create(
            &conn,
            &NewWork {
                canonical_title: "Un libro".to_owned(),
                original_language: None,
            },
        )
        .expect("obra");
        Edition::create(
            &conn,
            &NewEdition {
                work_id: obra.id,
                title: "Un libro".to_owned(),
                subtitle: None,
                publisher: None,
                published_date: None,
                edition_statement: None,
                language: None,
                series_title: None,
                series_index: None,
            },
        )
        .expect("edición")
        .id
    };
    let rutas = nb_books_lib::paths::AppPaths {
        data: destino.clone(),
        config: destino.join("config"),
        cache: destino.join("cache"),
        logs: destino.join("logs"),
    };
    let voz = VozNeuronal::nueva(
        &modelos,
        nb_books_lib::ai::presupuesto::Presupuesto::de_fabrica(
            nb_books_lib::ai::hardware::nucleos(),
        ),
    );
    let preparador = Preparador::nuevo(
        db,
        Arc::new(UnEpub(epub)),
        Arc::clone(&voz) as Arc<dyn Sintetizador>,
        &rutas,
        EventBus::new(),
    );
    let alcance = Alcance::Chapter {
        edition_id: edicion,
        hrefs: vec![href],
        from_position: None,
        to_position: None,
        title: "Un capítulo".to_owned(),
    };

    let reloj = Instant::now();
    preparador
        .empezar(alcance.clone(), Some("ef_dora".to_owned()))
        .expect("empieza");
    while preparador.en_curso().is_some() {
        assert!(
            reloj.elapsed() < Duration::from_secs(1_800),
            "la preparación no terminó en media hora"
        );
        std::thread::sleep(Duration::from_millis(500));
    }

    let preparado = preparador
        .preparado(&alcance, Some("ef_dora"))
        .expect("consulta")
        .expect("el capítulo tiene que quedar preparado: si no, falló a medias");
    #[allow(clippy::cast_precision_loss)]
    let segundos = preparado.duration_ms as f64 / 1000.0;
    #[allow(clippy::cast_precision_loss)]
    let esperado = caracteres as f64 / 17.0;
    eprintln!(
        "capítulo real: {caracteres} caracteres, {} fragmentos, {segundos:.0} s de audio \
         (se esperaban unos {esperado:.0}), {} KB, en {:.0} s",
        preparado.marks.len(),
        preparado.bytes / 1024,
        reloj.elapsed().as_secs_f64()
    );
    assert_eq!(preparado.status, "ready", "quedó a medias");
    assert!(
        segundos > esperado * 0.6,
        "se dijo a medias: {segundos:.0} s para unos {esperado:.0}"
    );
    assert!(Path::new(&preparado.path).is_file());

    let _ = std::fs::remove_dir_all(&destino);
}
