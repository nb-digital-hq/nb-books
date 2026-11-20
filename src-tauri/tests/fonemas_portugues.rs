//! Criterio de aceptación de la voz neuronal para el portugués: la conversión coincide
//! con la de referencia en una proporción **declarada y medida**, y lo que no
//! coincide está clasificado.
//!
//! # Qué es la referencia
//!
//! `tests/fixtures/fonemas/portugues.tsv` son 162 palabras con la salida de
//! `espeak-ng -v pt-br -q --ipa`, generada en esta máquina el 2026-09-19. Como en
//! español, `espeak-ng` **no entra en el producto**: no se enlaza, no se distribuye
//! y no está en `Cargo.toml`; se usa como se usa una regla para comprobar una
//! pieza.
//!
//! # Por qué el listón es más bajo que el del español, y eso es el hallazgo
//!
//! Porque en portugués **la mitad de lo que hay que saber no está escrito**. "mesa"
//! lleva /e/ y "terra", /ɛ/; "jogo" lleva /o/ y "rosa", /ɔ/. Las cuatro se escriben
//! con la misma letra y sin tilde, así que ninguna regla puede acertarlas: la
//! referencia lo resuelve con un diccionario. En español eso eran dos palabras de
//! 155; aquí es una clase entera, y por eso esta prueba la cuenta aparte.
//!
//! La consecuencia está dicha donde toca y no escondida en un porcentaje. Medido el
//! 2026-09-19: **89 % de palabras idénticas, y 97 % si no se cuenta el timbre**. Las
//! reglas, por tanto, están bien; lo que falta es un diccionario de trece palabras
//! de cada ciento sesenta, y hasta que exista **la voz portuguesa no entra en el
//! catálogo de modelos**: ahí tampoco hay todavía ninguna entrada con su huella
//! verificada.

use std::collections::BTreeMap;

use nb_books_lib::voice::fonemas::{self, Variante};

const REFERENCIA: &str = include_str!("fixtures/fonemas/portugues.tsv");

/// Proporción mínima de palabras **idénticas**.
///
/// **Medido el 2026-09-19: 144 de 162, el 89 %.** El listón va cuatro puntos por
/// debajo para que no salte por una palabra, y si alguien mejora las reglas, sube.
/// De las 18 que se apartan, 13 son el timbre de la vocal tónica y 5 son la `x`,
/// que suena de cuatro maneras distintas y ninguna es una regla.
const EXIGIDA: f64 = 0.85;

/// Proporción mínima que coincide **si se ignora el timbre de la vocal tónica**.
///
/// Es la cifra que dice si las reglas están bien: separa lo que ninguna regla puede
/// saber -abierta o cerrada- de lo que sí, que es todo lo demás. **Medido: 97 %.**
const EXIGIDA_SIN_TIMBRE: f64 = 0.93;

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

fn sin_acentos(s: &str) -> String {
    s.chars().filter(|c| !matches!(c, 'ˈ' | 'ˌ')).collect()
}

/// Las vocales que solo se distinguen por abiertas o cerradas, unificadas.
fn sin_timbre(s: &str) -> String {
    sin_acentos(s)
        .chars()
        .map(|c| match c {
            'ɛ' => 'e',
            'ɔ' => 'o',
            'æ' => 'a',
            'ʊ' => 'u',
            'ɪ' => 'i',
            'y' => 'i',
            _ => c,
        })
        .collect()
}

/// En qué se diferencian dos cadenas, para poder clasificar el fallo.
fn clase_del_fallo(nuestro: &str, referencia: &str) -> &'static str {
    if sin_acentos(nuestro) == sin_acentos(referencia) {
        return "acento";
    }
    if sin_timbre(nuestro) == sin_timbre(referencia) {
        return "timbre de la vocal";
    }
    if sin_timbre(nuestro).replace('ɐ', "a").replace('̃', "")
        == sin_timbre(referencia).replace('ɐ', "a").replace('̃', "")
    {
        return "nasalidad";
    }
    "sonido"
}

/// **El criterio.** Lo que coincide se mide, y lo que no, se clasifica.
#[test]
fn la_conversion_al_portugues_coincide_con_la_referencia_y_lo_que_no_esta_clasificado() {
    let referencia = referencia();
    assert!(
        referencia.len() >= 100,
        "la lista de referencia se quedó corta: {} palabras",
        referencia.len()
    );

    let mut fallos: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut iguales = 0;
    let mut iguales_sin_timbre = 0;
    for (palabra, esperado) in &referencia {
        let nuestro = fonemas::de_texto(palabra, Variante::PortuguesDeBrasil);
        if &nuestro == esperado {
            iguales += 1;
        } else {
            fallos
                .entry(clase_del_fallo(&nuestro, esperado))
                .or_default()
                .push(format!("{palabra}: {nuestro} ≠ {esperado}"));
        }
        if sin_timbre(&nuestro) == sin_timbre(esperado) {
            iguales_sin_timbre += 1;
        }
    }

    #[allow(clippy::cast_precision_loss)]
    let proporcion = iguales as f64 / referencia.len() as f64;
    #[allow(clippy::cast_precision_loss)]
    let proporcion_sin_timbre = iguales_sin_timbre as f64 / referencia.len() as f64;
    eprintln!(
        "fonemas del portugués: {iguales} de {} idénticos ({:.0} %), y {:.0} % sin contar el timbre",
        referencia.len(),
        proporcion * 100.0,
        proporcion_sin_timbre * 100.0
    );
    for (clase, casos) in &fallos {
        eprintln!("  {clase}: {} — {}", casos.len(), casos.join("; "));
    }

    assert!(
        proporcion >= EXIGIDA,
        "coinciden {proporcion:.2} y se exige {EXIGIDA:.2}"
    );
    assert!(
        proporcion_sin_timbre >= EXIGIDA_SIN_TIMBRE,
        "sin timbre coinciden {proporcion_sin_timbre:.2} y se exige {EXIGIDA_SIN_TIMBRE:.2}"
    );
}

/// Ningún fonema que el modelo no sepa decir.
#[test]
fn el_portugues_tampoco_produce_nada_que_el_modelo_no_sepa_decir() {
    for (palabra, _) in referencia() {
        let nuestro = fonemas::de_texto(&palabra, Variante::PortuguesDeBrasil);
        assert!(
            fonemas::todos_admitidos(&nuestro),
            "{palabra} produjo fonemas fuera del vocabulario: {nuestro}"
        );
    }
    let parrafo = fonemas::de_texto(
        "Em 25 de abril de 1492, a imprensa chegou a Sevilha; ninguém anotou.",
        Variante::PortuguesDeBrasil,
    );
    assert!(fonemas::todos_admitidos(&parrafo), "{parrafo}");
    assert!(parrafo.contains("mˈiʊ"), "los números se dicen: {parrafo}");
}

/// La variante sale de la voz elegida, no de un ajuste aparte.
#[test]
fn la_voz_portuguesa_habla_portugues() {
    assert_eq!(Variante::de_voz("pf_dora"), Variante::PortuguesDeBrasil);
    assert_eq!(Variante::de_voz("ef_dora"), Variante::EspanolDeAmerica);
}
