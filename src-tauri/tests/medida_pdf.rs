//! Lo que cuesta abrir un PDF y pasar sus páginas, sobre PDF de verdad.
//!
//! La sección 3 de `performance-budgets.md` pide pasar página de un PDF en menos de
//! 150 ms y abrir uno de un gigabyte en menos de 2 s, y ninguna de las dos cosas se
//! había. Se mide aquí, y se omite sola si no se le dice dónde:
//!
//! ```text
//! NB_BOOKS_PDF_MEDIDA=/carpeta/con/copias NB_BOOKS_PDF_GRANDE=/un/pdf/grande.pdf \
//!   cargo test --release --test medida_pdf -- --nocapture
//! ```
//!
//! **Sobre copias**, nunca sobre el corpus: medir escribe la caché de páginas y el
//! motor abre el archivo por su cuenta, sin `O_NOATIME`. Lo que se imprime son
//! milisegundos y recuentos, sin nombres de archivo.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use nb_books_lib::reader::pdf::{engine_state, Engine, PdfDocument};

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 10_000.0).round() / 10.0
}

fn mediana(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    xs.get(xs.len() / 2).copied().unwrap_or(0.0)
}

fn maximo(xs: &[f64]) -> f64 {
    xs.iter().copied().fold(0.0, f64::max)
}

/// Abre, pinta la primera y diez páginas más sin caché, y vuelve a pedir una ya pintada.
fn medir(ruta: &Path, cache: &Path) -> Option<(u16, f64, f64, Vec<f64>, f64)> {
    let _ = std::fs::remove_dir_all(cache);
    let inicio = Instant::now();
    let doc = PdfDocument::open(ruta, cache).ok()?;
    let apertura = ms(inicio.elapsed());
    let inicio = Instant::now();
    doc.render_page(1).ok()?;
    let primera = ms(inicio.elapsed());
    let paginas = doc.page_count();
    let mut siguientes = Vec::new();
    for pagina in 2..=paginas.min(11) {
        let inicio = Instant::now();
        doc.render_page(pagina).ok()?;
        siguientes.push(ms(inicio.elapsed()));
    }
    let inicio = Instant::now();
    doc.render_page(1).ok()?;
    let en_cache = ms(inicio.elapsed());
    Some((paginas, apertura, primera, siguientes, en_cache))
}

#[test]
fn abrir_un_pdf_y_pasar_sus_paginas_sobre_pdf_de_verdad() {
    if !matches!(engine_state(), Engine::Available) {
        eprintln!("omitida: sin biblioteca de PDF");
        return;
    }
    let cache = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-medida-pdf-{}", std::process::id()));

    if let Some(carpeta) = std::env::var_os("NB_BOOKS_PDF_MEDIDA") {
        let mut archivos: Vec<PathBuf> = std::fs::read_dir(&carpeta)
            .expect("carpeta")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "pdf"))
            .collect();
        archivos.sort();
        let (mut aperturas, mut primeras, mut pasos, mut caches) = (vec![], vec![], vec![], vec![]);
        let mut peor_apertura = (0.0, 0_u16, 0_u64);
        for ruta in &archivos {
            let Some((paginas, apertura, primera, siguientes, en_cache)) = medir(ruta, &cache)
            else {
                continue;
            };
            if apertura > peor_apertura.0 {
                let bytes = std::fs::metadata(ruta).map_or(0, |m| m.len());
                peor_apertura = (apertura, paginas, bytes);
            }
            aperturas.push(apertura);
            primeras.push(primera);
            pasos.extend(siguientes);
            caches.push(en_cache);
        }
        println!(
            "corpus: {} pdf; abrir mediana {} ms, máx {} ms ({} páginas, {} MB); primera página mediana {} ms, máx {} ms; \
             página siguiente mediana {} ms, máx {} ms ({} pasos); ya pintada mediana {} ms, máx {} ms",
            aperturas.len(),
            mediana(aperturas.clone()),
            maximo(&aperturas),
            peor_apertura.1,
            peor_apertura.2 / 1_000_000,
            mediana(primeras.clone()),
            maximo(&primeras),
            mediana(pasos.clone()),
            maximo(&pasos),
            pasos.len(),
            mediana(caches.clone()),
            maximo(&caches),
        );
    }

    if let Some(grande) = std::env::var_os("NB_BOOKS_PDF_GRANDE") {
        let ruta = PathBuf::from(grande);
        let bytes = std::fs::metadata(&ruta).map_or(0, |m| m.len());
        if let Some((paginas, apertura, primera, siguientes, en_cache)) = medir(&ruta, &cache) {
            println!(
                "grande: {} MB, {} páginas; abrir {} ms; primera página {} ms; siguientes mediana {} ms, máx {} ms; ya pintada {} ms",
                bytes / 1_000_000,
                paginas,
                apertura,
                primera,
                mediana(siguientes.clone()),
                maximo(&siguientes),
                en_cache
            );
        }
    }
    let _ = std::fs::remove_dir_all(&cache);
}
