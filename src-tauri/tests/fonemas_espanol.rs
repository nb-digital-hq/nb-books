//! Criterio de aceptación de la voz neuronal: la conversión de texto a
//! fonemas coincide con la de referencia, y **lo que no coincide está clasificado**.
//!
//! # Qué es la referencia, y por qué no es una dependencia
//!
//! El modelo de voz se entrenó con los fonemas que produce `espeak-ng`, así que lo
//! correcto aquí es lo que ella habría dicho. `tests/fixtures/fonemas/espanol.tsv`
//! son 155 palabras con la salida de `espeak-ng -v es-419 -q --ipa`, generada en
//! esta máquina el 2026-09-17.
//!
//! **`espeak-ng` no entra en el producto**: no se enlaza, no se distribuye y no está
//! en `Cargo.toml`. Se usó como se usa una regla para comprobar una pieza. Su
//! licencia GPL-3.0 gobierna su código, no la lista de palabras que alguien le
//! dictó ni lo que respondió.
//!
//! # Por qué no se exige el cien por cien
//!
//! Porque `espeak` no es solo reglas: tiene un diccionario de excepciones —"México"
//! suena distinto de "dejo" aunque se escriban con la misma letra— y replicarlo
//! entero sería copiarlo. Lo que se exige es que **las reglas coincidan** y que cada
//! palabra que se aparta esté contada y clasificada, para que la siguiente persona
//! sepa qué falta en lugar de encontrarse un número.

use std::collections::BTreeMap;

use nb_books_lib::voice::fonemas::{self, Variante};

const REFERENCIA: &str = include_str!("fixtures/fonemas/espanol.tsv");

/// Proporción mínima de palabras que tienen que salir **idénticas**.
///
/// **Medido el 2026-09-17: 153 de 155, el 99 %.** El listón se pone en 95 para que
/// no salte por una palabra, y lo que falta son las dos que se apartan por el
/// diccionario de la referencia, no por una regla: "México", que ella dice con jota
/// porque lo tiene escrito, y "cigüeña", donde separa la diéresis en otra sílaba.
const EXIGIDA: f64 = 0.95;

fn referencia() -> Vec<(String, String)> {
    REFERENCIA
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let (palabra, ipa) = l.split_once('\t')?;
            Some((palabra.to_owned(), ipa.trim().to_owned()))
        })
        .collect()
}

/// En qué se diferencian dos cadenas de fonemas, para poder clasificar el fallo.
fn clase_del_fallo(nuestro: &str, referencia: &str) -> &'static str {
    let sin_acentos =
        |s: &str| -> String { s.chars().filter(|c| !matches!(c, 'ˈ' | 'ˌ')).collect() };
    if sin_acentos(nuestro) == sin_acentos(referencia) {
        return "acento";
    }
    let vocales = |s: &str| -> String {
        s.chars()
            .map(|c| match c {
                'ɛ' => 'e',
                'ɔ' => 'o',
                'ɪ' => 'i',
                otra => otra,
            })
            .collect()
    };
    if vocales(&sin_acentos(nuestro)) == vocales(&sin_acentos(referencia)) {
        return "timbre de la vocal";
    }
    if sin_acentos(nuestro).replace('ʝ', "j") == sin_acentos(referencia).replace('ʝ', "j") {
        return "yeísmo";
    }
    "sonido"
}

/// **El criterio.** La conversión coincide con la referencia en la proporción
/// exigida, y lo que no coincide se imprime clasificado.
#[test]
fn la_conversion_coincide_con_la_referencia_y_lo_que_no_esta_clasificado() {
    let referencia = referencia();
    assert!(
        referencia.len() >= 100,
        "la lista de referencia se quedó corta: {} palabras",
        referencia.len()
    );

    let mut fallos: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut iguales = 0;
    for (palabra, esperado) in &referencia {
        let nuestro = fonemas::de_texto(palabra, Variante::EspanolDeAmerica);
        if &nuestro == esperado {
            iguales += 1;
        } else {
            fallos
                .entry(clase_del_fallo(&nuestro, esperado))
                .or_default()
                .push(format!("{palabra}: {nuestro} ≠ {esperado}"));
        }
    }

    #[allow(clippy::cast_precision_loss)]
    let proporcion = iguales as f64 / referencia.len() as f64;
    eprintln!(
        "fonemas del español: {iguales} de {} idénticos ({:.0} %)",
        referencia.len(),
        proporcion * 100.0
    );
    for (clase, casos) in &fallos {
        eprintln!("  {clase}: {} — {}", casos.len(), casos.join("; "));
    }

    assert!(
        proporcion >= EXIGIDA,
        "coinciden {proporcion:.2} y se exige {EXIGIDA:.2}"
    );
}

/// Ningún fonema que el modelo no sepa decir: lo que no está en su vocabulario se
/// lo come en silencio, y una palabra comida es peor que una mal dicha.
#[test]
fn todo_lo_que_se_produce_lo_sabe_decir_el_modelo() {
    for (palabra, _) in referencia() {
        for variante in [Variante::EspanolDeAmerica, Variante::EspanolDeEspana] {
            let nuestro = fonemas::de_texto(&palabra, variante);
            assert!(
                fonemas::todos_admitidos(&nuestro),
                "{palabra} produjo fonemas fuera del vocabulario: {nuestro}"
            );
        }
    }
    // Y un texto de verdad, con puntuación, cifras y mayúsculas.
    let parrafo = fonemas::de_texto(
        "El 25 de abril de 1492, la imprenta llegó a Sevilla; nadie lo anotó.",
        Variante::EspanolDeAmerica,
    );
    assert!(fonemas::todos_admitidos(&parrafo), "{parrafo}");
    assert!(parrafo.contains("mˈil"), "los números se dicen: {parrafo}");
}
