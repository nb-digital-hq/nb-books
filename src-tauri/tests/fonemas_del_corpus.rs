//! La conversión a fonemas, **sobre el texto de libros reales**.
//!
//! # Por qué existe
//!
//! El 2026-09-23, preparar un capítulo seguía fallando en la aplicación con "algo
//! falló por dentro" y en las pruebas no. La causa era un pánico de la conversión
//! con "III": tres débiles seguidas ponían el acento a mitad de un carácter de dos
//! bytes. Un número romano en un título es de lo más corriente en un libro y no
//! estaba en ninguna frase de prueba, porque las frases de prueba se escriben
//! pensando en cómo suena el español, no en cómo viene un EPUB.
//!
//! Esto pasa por la conversión cada fragmento de una muestra del corpus, **por el
//! mismo camino que la preparación**: `text_chunks` con el tamaño de fragmento del
//! índice.
//!
//! # Cómo se ejecuta
//!
//! Se omite sola si no hay corpus. Con él, igual que `corpus_real.rs`:
//!
//! ```text
//! NB_BOOKS_CORPUS=/ruta/al/corpus cargo test --release --test fonemas_del_corpus -- --nocapture
//! ```
//!
//! `NB_FONEMAS_MUESTRA` cambia cuántos libros se miran; de fábrica, cuarenta.
//!
//! **Solo lectura, y nada sale de aquí**: se imprimen cifras, no rutas, títulos ni
//! texto.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use nb_books_lib::reader::DocumentProvider;
use nb_books_lib::voice::fonemas::{self, Variante};

fn corpus() -> Option<PathBuf> {
    let raiz = PathBuf::from(std::env::var_os("NB_BOOKS_CORPUS")?);
    raiz.is_dir().then_some(raiz)
}

/// Un EPUB de cada tantas carpetas de autor, repartidos por todo el corpus: los
/// primeros en orden alfabético serían todos de autores que empiezan por "A", y lo
/// raro de un libro depende de quién lo editó, no de la letra.
fn muestrear(raiz: &Path, cuantos: usize) -> Vec<PathBuf> {
    let mut autores: Vec<PathBuf> = std::fs::read_dir(raiz)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    autores.sort();
    let paso = (autores.len() / cuantos.max(1)).max(1);
    autores
        .iter()
        .step_by(paso)
        .filter_map(|autor| {
            let mut libros: Vec<PathBuf> = std::fs::read_dir(autor)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("epub"))
                })
                .collect();
            libros.sort();
            libros.into_iter().next()
        })
        .take(cuantos)
        .collect()
}

#[test]
fn ningun_fragmento_de_un_libro_real_rompe_la_conversion_a_fonemas() {
    let Some(raiz) = corpus() else {
        eprintln!("NB_BOOKS_CORPUS no está definida o no existe: se omite");
        return;
    };
    let cuantos = std::env::var("NB_FONEMAS_MUESTRA")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(40);
    // El mensaje de un pánico al cortar una cadena lleva la cadena dentro, que aquí
    // es texto de un libro: se registra solo dónde fue.
    let lugares = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::<
        String,
        usize,
    >::new()));
    let suyos = std::sync::Arc::clone(&lugares);
    std::panic::set_hook(Box::new(move |info| {
        let lugar = info
            .location()
            .map_or_else(|| "?".to_owned(), |l| format!("{}:{}", l.file(), l.line()));
        *suyos
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(lugar)
            .or_default() += 1;
    }));
    let libros = muestrear(&raiz, cuantos);
    assert!(!libros.is_empty(), "el corpus no dio ni un EPUB");

    let mut simbolos = std::collections::BTreeMap::<char, usize>::new();
    let mut libros_rotos = 0_usize;
    let (mut abiertos, mut fragmentos, mut caracteres, mut panicos, mut ajenos) =
        (0_usize, 0_usize, 0_usize, 0_usize, 0_usize);
    for libro in &libros {
        let Ok(documento) = nb_books_lib::reader::epub::EpubDocument::open(libro) else {
            continue;
        };
        let Ok(trozos) = documento.text_chunks(nb_books_lib::search::FRAGMENTO) else {
            continue;
        };
        abiertos += 1;
        let antes = panicos;
        for (_, texto) in &trozos {
            fragmentos += 1;
            caracteres += texto.chars().count();
            for variante in [Variante::EspanolDeAmerica, Variante::EspanolDeEspana] {
                match catch_unwind(AssertUnwindSafe(|| fonemas::de_texto(texto, variante))) {
                    Ok(dicho) => {
                        if !fonemas::todos_admitidos(&dicho) {
                            ajenos += 1;
                            // Un símbolo suelto no es texto del libro: se cuenta cuál.
                            for c in dicho
                                .chars()
                                .filter(|c| *c != ' ' && !fonemas::ADMITIDOS.contains(*c))
                            {
                                *simbolos.entry(c).or_insert(0_usize) += 1;
                            }
                        }
                    }
                    Err(_) => panicos += 1,
                }
            }
        }
        if panicos > antes {
            libros_rotos += 1;
        }
    }
    eprintln!(
        "fonemas del corpus: {abiertos} libros, {fragmentos} fragmentos, {caracteres} \
         caracteres; {panicos} pánicos en {libros_rotos} libros, {ajenos} conversiones con símbolos que el modelo no \
         tiene"
    );
    let _ = std::panic::take_hook();
    let mut frecuentes: Vec<_> = simbolos.into_iter().collect();
    frecuentes.sort_by_key(|a| std::cmp::Reverse(a.1));
    for (simbolo, veces) in frecuentes.iter().take(25) {
        eprintln!(
            "  símbolo sin fonema {simbolo:?} (U+{:04X}): {veces}",
            u32::from(*simbolo)
        );
    }
    for (lugar, veces) in lugares.lock().unwrap_or_else(|p| p.into_inner()).iter() {
        eprintln!("  pánico en {lugar}: {veces} veces");
    }
    assert!(abiertos > 0, "no se pudo abrir ni un EPUB de la muestra");
    assert_eq!(
        panicos, 0,
        "la conversión entró en pánico con texto real: preparar ese capítulo fallaría"
    );
}
