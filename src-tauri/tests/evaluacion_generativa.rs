//! La evaluación del modelo que genera en local sobre el corpus de evaluación
//!.
//!
//! **Se salta sola** si no se le dice dónde está el modelo: descargarlo son 1,1 GB y
//! ejecutarlo, varios minutos por texto, así que no puede ser parte obligatoria de
//! `cargo test`. Se ejecuta a mano, y **con AVX2 compilado**, sin el cual un texto
//! tarda cuarto de hora (ver `ai::generativo`):
//!
//! ```text
//! RUSTFLAGS="-C target-cpu=x86-64-v3" NB_MODELOS=<carpeta con qwen25-15b-instruct-q4km/> \
//!   cargo test --release --test evaluacion_generativa -- --nocapture
//! ```
//!
//! Con `NB_EVALUACION=<archivo>` escribe además los resultados en JSON, que es lo
//! que se publica en `docs/architecture/ai/evaluation.md`.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use nb_books_lib::ai::estudio::{self, AlcanceDeEstudio, Funcion, Generadora};
use nb_books_lib::ai::generacion::Documentos;
use nb_books_lib::ai::generativo::{Compartido, GenerativoLocal};
use nb_books_lib::ai::presupuesto::Presupuesto;
use nb_books_lib::ai::rubrica::{self, UMBRALES};
use nb_books_lib::ai::{AiProvider, Availability, Completion, ProviderInfo, Request};
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::reader::secciones::{DocumentoDeSecciones, Seccion};
use nb_books_lib::reader::{fake, DocumentProvider};

const CORPUS: &[(&str, &str, &str)] = &[
    (
        "tecnico-es.txt",
        "es",
        include_str!("fixtures/evaluacion/tecnico-es.txt"),
    ),
    (
        "narrativo-en.txt",
        "en",
        include_str!("fixtures/evaluacion/narrativo-en.txt"),
    ),
    (
        "academico-pt.txt",
        "pt",
        include_str!("fixtures/evaluacion/academico-pt.txt"),
    ),
    (
        "ocr-es.txt",
        "es",
        include_str!("fixtures/evaluacion/ocr-es.txt"),
    ),
    (
        "formulas-es.txt",
        "es",
        include_str!("fixtures/evaluacion/formulas-es.txt"),
    ),
    (
        "narrativo-largo-en.txt",
        "en",
        include_str!("fixtures/evaluacion/narrativo-largo-en.txt"),
    ),
];

/// Cada párrafo, una sección: así cada uno es un fragmento, y las citas y la
/// cobertura significan algo en textos de dos mil caracteres.
fn secciones(texto: &str) -> Vec<Seccion> {
    texto
        .split("\n\n")
        .filter(|p| !p.trim().is_empty())
        .enumerate()
        .map(|(i, p)| Seccion {
            href: format!("p{i}.xhtml"),
            title: format!("{i}"),
            text: p.trim().to_owned(),
        })
        .collect()
}

struct Corpus;

impl Documentos for Corpus {
    fn abrir(&self, edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        let (_, _, texto) = CORPUS
            .iter()
            .find(|(nombre, _, _)| *nombre == edition_id)
            .ok_or(NbError::FileNotFound)?;
        Ok(Box::new(DocumentoDeSecciones::new(
            fake::MEDIA_TYPE,
            secciones(texto),
            fake::con_tres_capitulos().capabilities(),
        )))
    }
}

/// Memoria residente del proceso, en bytes. Solo en Linux, que es donde esto se
/// mide; en otra plataforma la suite no afirma nada de memoria.
fn memoria_residente() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let paginas: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(paginas * 4096)
}

struct Local(Compartido);

impl Generadora for Local {
    fn quien(&self) -> Option<ProviderInfo> {
        match self.0.readiness() {
            Availability::Ready(info) => Some(info),
            Availability::Absent(motivo) => panic!("el modelo no está listo: {motivo:?}"),
        }
    }

    fn caracteres(&self) -> usize {
        self.0.input_chars()
    }

    fn generar(&self, request: &Request) -> NbResult<Completion> {
        self.0.generate(request)
    }
}

#[test]
fn el_modelo_local_supera_la_rubrica_sobre_el_corpus_de_evaluacion() {
    let Ok(modelos) = std::env::var("NB_MODELOS") else {
        eprintln!("NB_MODELOS no está definido: la evaluación del modelo local se salta");
        return;
    };
    let nucleos = std::thread::available_parallelism().map_or(4, |n| n.get());
    // Con `NB_MODELO` se mide uno del catálogo; sin él, el que se recomienda para
    // este equipo, que es lo que usaría quien lo instala sin tocar nada.
    let pedido = std::env::var("NB_MODELO").unwrap_or_default();
    // El tope de memoria, el mayor que admiten las preferencias: aquí se mide lo que
    // cuesta el modelo, no lo que cabe en un tope concreto.
    let local = Arc::new(GenerativoLocal::nuevo(
        &PathBuf::from(modelos),
        Presupuesto::desde(4.0, 8192.0, nucleos),
        Arc::new(AtomicBool::new(false)),
    ));
    if !pedido.is_empty() {
        local.usar(&pedido);
    }
    let modelo = local.elegido();
    eprintln!("modelo: {} ({})", modelo.nombre, modelo.id);
    let proveedor = Local(Compartido(local));

    let mut resultados = Vec::new();
    let mut no_superan = Vec::new();
    // Lo que se mide del proveedor y no de la salida: cuánto tarda, cuánto ocupa y
    // **cuántas veces falla**. Una tasa de fallo que nadie cuenta se convierte en
    // "a veces no va", que no es un número y no se puede comparar entre versiones.
    let mut fallos: Vec<String> = Vec::new();
    let mut latencias: Vec<u64> = Vec::new();
    let memoria_antes = memoria_residente();
    let mut memoria_maxima = memoria_antes.unwrap_or(0);
    for (nombre, idioma, texto) in CORPUS {
        let hrefs = (0..secciones(texto).len())
            .map(|i| format!("p{i}.xhtml"))
            .collect();
        let alcance = AlcanceDeEstudio::Section {
            edition_id: (*nombre).to_owned(),
            hrefs,
            from_position: None,
            to_position: None,
        };
        // Lo que el proveedor declara. Ahora el local declara también
        // la autoevaluación -se ofrece desaconsejada en lugar de esconderse-, así
        // que se mide: es justo lo que hay que saber de ella.
        let funciones = [
            Funcion::Flashcards,
            Funcion::Summary,
            Funcion::Concepts,
            Funcion::Questions,
        ]
        .into_iter()
        .filter(|f| {
            nb_books_lib::ai::estudio::plantilla_de(*f)
                .is_some_and(|p| proveedor.0.capabilities().contains(&p.capacidad))
        })
        // `NB_FUNCIONES=flashcards,summary` mide solo esas. Existe para iterar una
        // plantilla, o comparar otro modelo, sin pagar la hora entera de la suite.
        // Lo que se publique así lleva `alcance` diciendo que es parcial: el
        // resultado de referencia de una plantilla sale de una ejecución completa
        // con el modelo recomendado.
        .filter(|f| {
            std::env::var("NB_FUNCIONES").map_or(true, |lista| {
                lista.split(',').any(|n| n.trim() == f.as_str())
            })
        });
        for funcion in funciones {
            let inicio = std::time::Instant::now();
            // Un fallo se anota y se sigue: parar en el primero deja la suite sin
            // decir cuántas de las demás iban bien, que es la mitad de la medida.
            let hecha =
                match estudio::hacer_y_conservar(&Corpus, &proveedor, funcion, &alcance, idioma) {
                    Ok(hecha) => hecha,
                    Err(error) => {
                        eprintln!("{nombre} {} falló: {}", funcion.as_str(), error.code());
                        fallos.push(format!("{nombre} {}: {}", funcion.as_str(), error.code()));
                        continue;
                    }
                };
            if let Some(ahora) = memoria_residente() {
                memoria_maxima = memoria_maxima.max(ahora);
            }
            let evaluacion = rubrica::evaluar(&hecha.generacion.reading, &hecha.textos, UMBRALES);
            let segundos = inicio.elapsed().as_secs();
            latencias.push(inicio.elapsed().as_millis() as u64);
            eprintln!(
                "{nombre} {} en {segundos} s: {evaluacion:?}\n---\n{}\n---",
                funcion.as_str(),
                hecha.salida
            );
            if !evaluacion.supera && funcion == Funcion::Flashcards {
                no_superan.push(format!("{nombre} {}", funcion.as_str()));
            }
            resultados.push(serde_json::json!({
                "texto": nombre,
                "funcion": funcion.as_str(),
                "plantilla": format!("{}@{}", hecha.generacion.template_id, hecha.generacion.template_version),
                "modelo": modelo.id,
                "segundos": segundos,
                "evaluacion": evaluacion,
                "salida": hecha.salida,
            }));
        }
    }

    latencias.sort_unstable();
    let intentos = resultados.len() + fallos.len();
    #[allow(clippy::cast_precision_loss)]
    let tasa_de_fallo = if intentos == 0 {
        0.0
    } else {
        fallos.len() as f64 / intentos as f64
    };
    let mediana = latencias.get(latencias.len() / 2).copied().unwrap_or(0);
    let peor = latencias.last().copied().unwrap_or(0);
    eprintln!(
        "proveedor {}: {intentos} intentos, {} fallos ({tasa_de_fallo:.0}), \
         latencia mediana {mediana} ms, peor {peor} ms",
        modelo.id,
        fallos.len()
    );

    if let Ok(destino) = std::env::var("NB_EVALUACION") {
        let salida = serde_json::json!({
            "proveedor": {
                "id": modelo.id,
                "nombre": modelo.nombre,
                "intentos": intentos,
                "fallos": fallos,
                "tasaDeFallo": tasa_de_fallo,
                "latenciaMs": { "mediana": mediana, "peor": peor },
                "memoriaMaximaBytes": memoria_maxima,
            },
            "generaciones": resultados,
        });
        std::fs::write(
            destino,
            serde_json::to_string_pretty(&salida).expect("json"),
        )
        .expect("escribir los resultados");
    }
    // El criterio es de tarjetas: los resúmenes y los conceptos se miden y
    // se publican, pero un resumen que se queda corto no es una tarjeta equivocada.
    assert!(
        no_superan.is_empty(),
        "no superan la rúbrica: {no_superan:?}"
    );
}
