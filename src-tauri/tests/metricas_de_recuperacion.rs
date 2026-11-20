//! Métricas de recuperación sobre el corpus de evaluación.
//!
//! # Qué se mide, y por qué esto y no otra cosa
//!
//! La pregunta que decide si la búsqueda por significado sirve es sencilla de
//! enunciar y fácil de esquivar: **cuando alguien pregunta algo cuya respuesta está
//! en un párrafo concreto, ¿sale ese párrafo, y sale arriba?** Se mide con dos
//! números de toda la vida:
//!
//! * **Acierto en los k primeros** (`recall@k`): en qué proporción de preguntas el
//!   párrafo correcto está entre los k primeros.
//! * **Rango recíproco medio** (`MRR`): uno partido por la posición en la que sale,
//!   promediado. Distingue salir primero de salir quinto, que `recall@5` no.
//!
//! Se mide sobre los vectores, que es donde se decide el orden. Que la consulta
//! llegue desde la interfaz hasta el índice y vuelva con su locator lo comprueba
//! `busqueda_hibrida.rs`; aquí no se vuelve a comprobar eso, se mide la calidad del
//! orden con el modelo real.
//!
//! # Se salta sola sin el modelo
//!
//! Ningún peso entra al repositorio, así que esto pide `NB_BOOKS_MODELO_REAL`
//! apuntando a la carpeta del modelo de embeddings del catálogo. Con
//! `NB_METRICAS=<archivo>` escribe los resultados en JSON, que es lo que se publica
//! en `docs/architecture/ai/evaluation.md`.
//!
//! ```text
//! NB_BOOKS_MODELO_REAL=~/.local/share/nb-books/modelos/granite-embedding-107m-multilingual \
//!   cargo test --release --test metricas_de_recuperacion -- --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use candle_core::Device;

use nb_books_lib::ai::catalogo::GRANITE_107M_MULTILINGUAL;
use nb_books_lib::ai::modelo::Residente;
use nb_books_lib::ai::presupuesto::Presupuesto;

/// Un texto del corpus, sus párrafos y las preguntas con el párrafo que las
/// contesta.
struct Caso {
    texto: &'static str,
    contenido: &'static str,
    /// Pregunta y **índice del párrafo** que la contesta, contando desde cero.
    preguntas: &'static [(&'static str, usize)],
}

/// Las preguntas están escritas mirando el texto y **con palabras distintas de las
/// suyas** siempre que se puede: si la pregunta repite la frase del párrafo, lo que
/// se mide es la coincidencia de palabras y no el significado, que es justo lo que
/// esta búsqueda aporta sobre la léxica.
const CASOS: &[Caso] = &[
    Caso {
        texto: "tecnico-es.txt",
        contenido: include_str!("fixtures/evaluacion/tecnico-es.txt"),
        preguntas: &[
            ("¿por qué se calienta tanto por dentro cuando hace sol?", 1),
            (
                "¿dónde conviene poner las aberturas para que corra el aire?",
                2,
            ),
            ("¿cómo se baja la temperatura cuando abrir no alcanza?", 3),
            ("¿qué se puede hacer para que no hiele de madrugada?", 4),
            ("¿dónde se pone el aparato que mide, para que no engañe?", 5),
        ],
    },
    Caso {
        texto: "formulas-es.txt",
        contenido: include_str!("fixtures/evaluacion/formulas-es.txt"),
        preguntas: &[
            ("¿cuánta agua pierde una parcela en un día?", 1),
            ("¿cómo cambia la necesidad según la fase de la planta?", 2),
            ("¿cuánto se desperdicia según el sistema que se use?", 5),
            ("¿cuánto hay que tener el riego abierto?", 6),
            ("¿qué pasa si se echa más de lo que la tierra aguanta?", 7),
        ],
    },
    Caso {
        texto: "narrativo-largo-en.txt",
        contenido: include_str!("fixtures/evaluacion/narrativo-largo-en.txt"),
        preguntas: &[
            ("how did she get the job at the crossing?", 3),
            (
                "what did she notice when she compared the money with the carts?",
                8,
            ),
            (
                "why did the previous man write down fewer than had crossed?",
                17,
            ),
            ("what did she send to the newspaper?", 19),
            ("what happened to the damaged pier?", 20),
        ],
    },
    Caso {
        texto: "academico-pt.txt",
        contenido: include_str!("fixtures/evaluacion/academico-pt.txt"),
        preguntas: &[
            ("quanto tempo leva ouvir cada aluno contar o que leu?", 2),
            ("as perguntas de manual podem ser respondidas sem ler?", 3),
            ("o que faz uma pergunta boa?", 3),
            ("como se aumenta a concordância entre quem corrige?", 5),
        ],
    },
];

/// Cuántos primeros se miran. Cinco es lo que la interfaz enseña sin desplazar.
const K: usize = 5;

fn coseno(a: &[f32], b: &[f32]) -> f32 {
    let producto: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        producto / (na * nb)
    }
}

fn parrafos(texto: &str) -> Vec<String> {
    texto
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect()
}

/// **El criterio para la recuperación**: el párrafo que contesta sale, y
/// sale arriba. Los umbrales se fijan **antes** de mirar ningún número.
#[test]
fn la_busqueda_por_significado_encuentra_el_parrafo_que_contesta() {
    let Ok(carpeta) = std::env::var("NB_BOOKS_MODELO_REAL") else {
        eprintln!("se salta: pide NB_BOOKS_MODELO_REAL=<carpeta del modelo>");
        return;
    };

    // Escritos aquí antes de ejecutar nada, que es lo que hace que un umbral sea un
    // umbral y no una descripción de lo que salió.
    const ACIERTO_EXIGIDO: f64 = 0.80;
    const RANGO_EXIGIDO: f64 = 0.60;

    let residente = Arc::new(Residente::new(
        PathBuf::from(carpeta),
        &GRANITE_107M_MULTILINGUAL,
        Device::Cpu,
        Duration::from_secs(60),
        Presupuesto::de_fabrica(nb_books_lib::ai::hardware::nucleos()),
    ));

    let mut por_texto = Vec::new();
    let mut aciertos_totales = 0;
    let mut rango_total = 0.0;
    let mut preguntas_totales = 0;
    let mut latencias = Vec::new();

    for caso in CASOS {
        let parrafos = parrafos(caso.contenido);
        let vectores = residente.vectores(&parrafos).expect("vectores del texto");

        let mut aciertos = 0;
        let mut rango = 0.0;
        let mut primeras = 0;
        for (pregunta, esperado) in caso.preguntas {
            let inicio = Instant::now();
            let consulta = residente
                .vectores(&[(*pregunta).to_owned()])
                .expect("vector de la consulta");
            latencias.push(inicio.elapsed().as_millis());

            let mut puntuados: Vec<(usize, f32)> = vectores
                .iter()
                .enumerate()
                .map(|(i, v)| (i, coseno(&consulta[0], v)))
                .collect();
            puntuados.sort_by(|a, b| b.1.total_cmp(&a.1));

            let posicion = puntuados.iter().position(|(i, _)| i == esperado);
            match posicion {
                Some(p) if p < K => {
                    aciertos += 1;
                    rango += 1.0 / (p as f64 + 1.0);
                    if p == 0 {
                        primeras += 1;
                    }
                }
                Some(p) => {
                    rango += 1.0 / (p as f64 + 1.0);
                    eprintln!(
                        "  fuera de los {K}: «{pregunta}» salió en la posición {}",
                        p + 1
                    );
                }
                None => eprintln!("  no salió: «{pregunta}»"),
            }
        }

        let cuantas = caso.preguntas.len();
        preguntas_totales += cuantas;
        aciertos_totales += aciertos;
        rango_total += rango;
        #[allow(clippy::cast_precision_loss)]
        let acierto = f64::from(aciertos) / cuantas as f64;
        #[allow(clippy::cast_precision_loss)]
        let medio = rango / cuantas as f64;
        eprintln!(
            "{}: acierto@{K} {acierto:.2}, MRR {medio:.2}, primeras {primeras} de {cuantas}",
            caso.texto
        );
        por_texto.push(serde_json::json!({
            "texto": caso.texto,
            "preguntas": cuantas,
            "aciertoEnK": acierto,
            "rangoReciprocoMedio": medio,
            "primeras": primeras,
        }));
    }

    #[allow(clippy::cast_precision_loss)]
    let acierto = f64::from(aciertos_totales) / preguntas_totales as f64;
    #[allow(clippy::cast_precision_loss)]
    let rango = rango_total / preguntas_totales as f64;
    latencias.sort_unstable();
    let mediana = latencias[latencias.len() / 2];
    let peor = latencias.last().copied().unwrap_or(0);
    eprintln!(
        "total: acierto@{K} {acierto:.2}, MRR {rango:.2} sobre {preguntas_totales} preguntas; \
         vectorizar la consulta: mediana {mediana} ms, peor {peor} ms"
    );

    if let Ok(destino) = std::env::var("NB_METRICAS") {
        let salida = serde_json::json!({
            "modelo": GRANITE_107M_MULTILINGUAL.id,
            "k": K,
            "preguntas": preguntas_totales,
            "aciertoEnK": acierto,
            "rangoReciprocoMedio": rango,
            "latenciaConsultaMs": { "mediana": mediana, "peor": peor },
            "umbrales": { "aciertoEnK": ACIERTO_EXIGIDO, "rangoReciprocoMedio": RANGO_EXIGIDO },
            "porTexto": por_texto,
        });
        std::fs::write(
            destino,
            serde_json::to_string_pretty(&salida).expect("json"),
        )
        .expect("escribir las métricas");
    }

    assert!(
        acierto >= ACIERTO_EXIGIDO,
        "acierto@{K} {acierto:.2}, se exige {ACIERTO_EXIGIDO:.2}"
    );
    assert!(
        rango >= RANGO_EXIGIDO,
        "MRR {rango:.2}, se exige {RANGO_EXIGIDO:.2}"
    );
}
