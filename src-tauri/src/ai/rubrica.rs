//! La rúbrica de las tarjetas, las preguntas y los resúmenes.
//!
//! Contrato: `docs/architecture/ai/evaluation.md`, sección 3: el criterio de las
//! funciones de estudio es "supera
//! la rúbrica", y sin rúbrica ese criterio no se podía comprobar.
//!
//! # Lo que mide y lo que no
//!
//! `evaluation.md` pide para las tarjetas **corrección factual, no trivialidad y
//! respuesta única**, y para los resúmenes **fidelidad, cobertura y ausencia de
//! invenciones**, con revisión humana. Lo que se puede medir sin una persona, y es
//! lo que hace esta rúbrica:
//!
//! - **Citada.** Cada elemento apunta a algún fragmento entregado.
//! - **Sostenida.** Las palabras con contenido de la respuesta —o del punto del
//!   resumen— están en el texto de los fragmentos que cita. Es una aproximación a
//!   la corrección factual por léxico, **no una comprobación de que sea cierta**:
//!   una respuesta que reordena palabras del texto hasta decir lo contrario pasaría.
//!   Lo que sí caza, y es lo que más ocurre con un modelo pequeño, es la respuesta
//!   inventada: palabras que no están en ningún sitio del texto.
//! - **No trivial.** La pregunta no contiene su respuesta, y la respuesta no es un
//!   párrafo entero copiado.
//! - **Sin repetir.** Dos tarjetas que preguntan lo mismo cuentan como una.
//! - **Cobertura.** Las citas se reparten por los fragmentos en lugar de quedarse en
//!   el primero.
//!
//! **Lo que no mide, y sigue siendo de una persona:** que una pregunta sea
//! importante, que los distractores sean plausibles y que la clave de una pregunta
//! sea la única defendible. Por eso la rúbrica decide si una plantilla o un modelo
//! **no llega**, y nunca basta para decir que llega del todo (`evaluation.md`
//! sección 5).

use std::collections::HashSet;

use serde::Serialize;

use super::salidas::{normalizar, Elementos, Lectura};

/// Proporción de palabras con contenido que tienen que estar en el texto citado
/// para que un elemento cuente como sostenido.
pub const SOSTENIDA: f64 = 0.6;

/// Umbrales que una salida tiene que superar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Umbrales {
    pub minimo_de_elementos: usize,
    pub citadas: f64,
    pub sostenidas: f64,
    pub no_triviales: f64,
}

/// Los de fábrica, fijados el 2026-09-17 **antes** de medir ningún modelo con
/// ellos, para que no se ajustaran a lo que saliera.
pub const UMBRALES: Umbrales = Umbrales {
    minimo_de_elementos: 3,
    citadas: 0.8,
    sostenidas: 0.7,
    no_triviales: 0.9,
};

/// El resultado de pasar la rúbrica.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Evaluacion {
    pub elementos: usize,
    pub descartados: u32,
    pub citadas: f64,
    pub sostenidas: f64,
    pub no_triviales: f64,
    pub repetidas: usize,
    /// Fragmentos distintos citados sobre los entregados.
    pub cobertura: f64,
    pub supera: bool,
    /// Por qué no supera, en claves estables.
    pub motivos: Vec<&'static str>,
}

/// Palabras que no dicen nada del contenido, en los tres idiomas del corpus.
const VACIAS: &[&str] = &[
    // español
    "para", "como", "pero", "porque", "cuando", "donde", "desde", "hasta", "sobre", "entre", "este",
    "esta", "estos", "estas", "ese", "esa", "esos", "esas", "aquel", "cada", "todo", "toda",
    "todos", "todas", "otro", "otra", "otros", "otras", "muy", "más", "menos", "también", "tiene",
    "tienen", "puede", "pueden", "hace", "hacen", "según", "sino", "cual", "cuales", "quien",
    "quienes", "había", "habia", "eran", "fueron", "está", "están", "son", "ser", "del", "los",
    "las", "una", "unos", "unas", "que", "qué", "por", "con", "sin", "sus", "les",
    // inglés
    "the", "and", "that", "this", "with", "from", "which", "what", "when", "where", "were", "was",
    "have", "has", "had", "their", "they", "them", "there", "into", "about", "after", "before",
    "does", "did", "who", "whom", "why", "how", "for", "not", "but", // portugués
    "uma", "umas", "uns", "com", "sem", "pela", "pelo", "pelos", "pelas", "mas", "quando", "onde",
    "porque", "também", "cada", "seu", "sua", "seus", "suas", "não", "são", "foi", "foram", "pode",
    "podem", "qual", "quais", "quem", "entre", "sobre", "outro", "outra",
];

/// Las palabras con contenido de un texto, normalizadas.
fn contenido(texto: &str) -> Vec<String> {
    let vacias: HashSet<String> = VACIAS.iter().map(|v| normalizar(v)).collect();
    normalizar(texto)
        .split_whitespace()
        .filter(|p| p.chars().count() >= 3 && !vacias.contains(*p))
        .map(str::to_owned)
        .collect()
}

/// Qué parte de las palabras con contenido de `afirmacion` está en `fuente`.
///
/// Lo usa también `estudio` para **comprobar la cita que dio el modelo** antes de
/// enseñarla, en lugar de creérsela.
///
/// Una palabra cuenta si está entera en la fuente o si comparte con alguna de sus
/// palabras las cinco primeras letras: así "impresores" queda sostenida por
/// "impresor", que es la flexión que un resumen cambia. El precio es que también
/// sostiene palabras distintas con el mismo principio, y por eso esto es una
/// aproximación y no una comprobación.
#[must_use]
pub fn apoyo(afirmacion: &str, fuente: &str) -> f64 {
    let palabras = contenido(afirmacion);
    if palabras.is_empty() {
        return 0.0;
    }
    let de_la_fuente: Vec<String> = contenido(fuente);
    let enteras: HashSet<&str> = de_la_fuente.iter().map(String::as_str).collect();
    let raices: HashSet<String> = de_la_fuente
        .iter()
        .filter(|p| p.chars().count() >= 5)
        .map(|p| p.chars().take(5).collect())
        .collect();
    let dentro = palabras
        .iter()
        .filter(|p| {
            enteras.contains(p.as_str())
                || (p.chars().count() >= 5
                    && raices.contains(&p.chars().take(5).collect::<String>()))
        })
        .count();
    #[allow(clippy::cast_precision_loss)]
    let proporcion = dentro as f64 / palabras.len() as f64;
    proporcion
}

/// El texto de las fuentes citadas, juntas.
fn citado(citas: &[u32], textos: &[String]) -> String {
    citas
        .iter()
        .filter_map(|i| textos.get(*i as usize))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Pasa la rúbrica a una lectura, con el texto de los fragmentos entregados.
#[must_use]
pub fn evaluar(lectura: &Lectura, textos: &[String], umbrales: Umbrales) -> Evaluacion {
    // (afirmación que se comprueba, pregunta si la hay, citas)
    let elementos: Vec<(String, Option<String>, Vec<u32>)> = match &lectura.items {
        Elementos::Resumen(puntos) => puntos
            .iter()
            .map(|p| (p.text.clone(), None, p.citations.clone()))
            .collect(),
        Elementos::Tarjetas(tarjetas) => tarjetas
            .iter()
            .map(|t| {
                (
                    t.answer.clone(),
                    Some(t.question.clone()),
                    t.citations.clone(),
                )
            })
            .collect(),
        Elementos::Preguntas(preguntas) => preguntas
            .iter()
            .map(|p| {
                let clave = p
                    .options
                    .get(p.correct as usize)
                    .cloned()
                    .unwrap_or_default();
                (clave, Some(p.prompt.clone()), p.citations.clone())
            })
            .collect(),
        Elementos::Conceptos(conceptos) => conceptos
            .iter()
            .map(|c| {
                (
                    format!("{} {}", c.name, c.description),
                    None,
                    c.citations.clone(),
                )
            })
            .collect(),
    };

    let n = elementos.len();
    let proporcion = |cuantos: usize| {
        #[allow(clippy::cast_precision_loss)]
        let p = if n == 0 {
            0.0
        } else {
            cuantos as f64 / n as f64
        };
        p
    };

    let citadas = elementos.iter().filter(|(_, _, c)| !c.is_empty()).count();
    let sostenidas = elementos
        .iter()
        .filter(|(afirmacion, _, citas)| {
            !citas.is_empty() && apoyo(afirmacion, &citado(citas, textos)) >= SOSTENIDA
        })
        .count();
    let no_triviales = elementos
        .iter()
        .filter(|(afirmacion, pregunta, _)| {
            let larga = contenido(afirmacion).len() > 40;
            let regalada = pregunta.as_ref().is_some_and(|q| {
                let q = normalizar(q);
                let a = normalizar(afirmacion);
                !a.is_empty() && q.contains(&a)
            });
            !larga && !regalada
        })
        .count();
    let mut vistas = HashSet::new();
    let repetidas = elementos
        .iter()
        .filter(|(afirmacion, pregunta, _)| {
            !vistas.insert(normalizar(pregunta.as_deref().unwrap_or(afirmacion)))
        })
        .count();
    let distintas: HashSet<u32> = elementos
        .iter()
        .flat_map(|(_, _, c)| c.iter().copied())
        .collect();
    #[allow(clippy::cast_precision_loss)]
    let cobertura = if textos.is_empty() {
        0.0
    } else {
        distintas.len() as f64 / textos.len() as f64
    };

    let mut motivos = Vec::new();
    if n < umbrales.minimo_de_elementos {
        motivos.push("POCOS_ELEMENTOS");
    }
    if proporcion(citadas) < umbrales.citadas {
        motivos.push("SIN_CITAS");
    }
    if proporcion(sostenidas) < umbrales.sostenidas {
        motivos.push("NO_SOSTENIDAS");
    }
    if proporcion(no_triviales) < umbrales.no_triviales {
        motivos.push("TRIVIALES");
    }
    if repetidas > 0 {
        motivos.push("REPETIDAS");
    }

    Evaluacion {
        elementos: n,
        descartados: lectura.discarded,
        citadas: proporcion(citadas),
        sostenidas: proporcion(sostenidas),
        no_triviales: proporcion(no_triviales),
        repetidas,
        cobertura,
        supera: motivos.is_empty(),
        motivos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::plantillas::FormatoDeSalida;
    use crate::ai::salidas::leer;

    fn textos() -> Vec<String> {
        vec![
            "La ventilación natural se basa en dos aberturas situadas a distinta altura. El aire caliente sale por la abertura superior y el aire fresco entra por la inferior.".to_owned(),
            "Una malla de sombreo del cincuenta por ciento reduce a la mitad la radiación y baja la temperatura interior entre cuatro y seis grados.".to_owned(),
            "Un depósito de agua oscuro almacena calor durante el día y lo devuelve lentamente por la noche.".to_owned(),
        ]
    }

    #[test]
    fn unas_tarjetas_sostenidas_por_el_texto_superan_la_rubrica() {
        let salida = r#"{"tarjetas": [
            {"pregunta": "¿Por dónde sale el aire caliente en la ventilación natural?", "respuesta": "Por la abertura superior", "citas": ["F1"]},
            {"pregunta": "¿Cuánto baja la temperatura una malla de sombreo del cincuenta por ciento?", "respuesta": "Entre cuatro y seis grados", "citas": ["F2"]},
            {"pregunta": "¿Qué hace de noche un depósito de agua oscuro?", "respuesta": "Devuelve lentamente el calor almacenado durante el día", "citas": ["F3"]}
        ]}"#;
        let e = evaluar(
            &leer(FormatoDeSalida::Tarjetas, salida, 3),
            &textos(),
            UMBRALES,
        );
        assert!(e.supera, "{e:?}");
        assert!((e.cobertura - 1.0).abs() < f64::EPSILON);
    }

    /// Respuestas que no están en el texto: lo que más hace un modelo pequeño.
    #[test]
    fn unas_respuestas_inventadas_no_superan_la_rubrica() {
        let salida = r#"{"tarjetas": [
            {"pregunta": "¿Quién inventó el invernadero?", "respuesta": "Los romanos en el siglo primero", "citas": ["F1"]},
            {"pregunta": "¿Qué plantas crecen mejor?", "respuesta": "Tomates, pimientos y berenjenas", "citas": ["F2"]},
            {"pregunta": "¿Cuánto cuesta un depósito?", "respuesta": "Unos doscientos euros instalado", "citas": ["F3"]}
        ]}"#;
        let e = evaluar(
            &leer(FormatoDeSalida::Tarjetas, salida, 3),
            &textos(),
            UMBRALES,
        );
        assert!(!e.supera);
        assert!(e.motivos.contains(&"NO_SOSTENIDAS"), "{e:?}");
    }

    #[test]
    fn preguntas_que_regalan_la_respuesta_o_se_repiten_no_superan() {
        let salida = r#"{"tarjetas": [
            {"pregunta": "¿El aire caliente sale por la abertura superior?", "respuesta": "abertura superior", "citas": ["F1"]},
            {"pregunta": "¿Qué baja una malla de sombreo?", "respuesta": "La temperatura interior", "citas": ["F2"]},
            {"pregunta": "¿Qué baja una malla de sombreo?", "respuesta": "La radiación a la mitad", "citas": ["F2"]}
        ]}"#;
        let e = evaluar(
            &leer(FormatoDeSalida::Tarjetas, salida, 3),
            &textos(),
            UMBRALES,
        );
        assert!(e.motivos.contains(&"TRIVIALES"), "{e:?}");
        assert!(e.motivos.contains(&"REPETIDAS"), "{e:?}");
    }

    #[test]
    fn sin_citas_o_con_pocas_no_supera() {
        let salida = r#"{"puntos": [{"texto": "El aire caliente sale por arriba."}]}"#;
        let e = evaluar(
            &leer(FormatoDeSalida::Resumen, salida, 3),
            &textos(),
            UMBRALES,
        );
        assert_eq!(e.motivos[..2], ["POCOS_ELEMENTOS", "SIN_CITAS"]);
        let vacia = evaluar(
            &leer(FormatoDeSalida::Resumen, "nada", 3),
            &textos(),
            UMBRALES,
        );
        assert!(!vacia.supera);
    }
}
