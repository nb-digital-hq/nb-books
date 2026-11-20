//! Las funciones de estudio: resumir, hacer tarjetas y preguntas, y sacar
//! conceptos, siempre con el sitio del que sale cada cosa.
//!
//! Contratos: `docs/architecture/ai/prompts.md` y `docs/architecture/ai/philosophy.md`.
//!
//!
//! # Un documento, y solo uno
//!
//! Todo lo que llega a un modelo sale de **un** documento, abierto aquí por su
//! edición: no hay forma de pedir una función sobre dos, ni de que la recuperación
//! traiga un fragmento de otro libro. Es lo que hace que un documento escrito para
//! "extraer el contenido de los demás" no tenga nada que extraer: el modelo no lo
//! ha visto.
//!
//! # Recuperar antes de generar
//!
//! Nunca se envía un documento entero (`prompts.md` sección 3). Del alcance que se
//! pide —las secciones de un capítulo, o una selección— se toman sus fragmentos, los
//! mismos del índice de búsqueda y con el mismo locator, y si no caben en lo que
//! admite quien genera se reparten **a lo largo de todo el alcance** en lugar de
//! quedarse con el principio: un resumen del primer tercio de un capítulo diría que
//! es el resumen del capítulo. Que se recortó se dice en la respuesta.
//!
//! # Las citas son locators
//!
//! El modelo cita por número —`F3`—, porque el locator no sale del dispositivo; aquí
//! el número vuelve a ser el locator del fragmento, y con él la interfaz abre el
//! sitio exacto. Una cita que no apunta a un fragmento entregado ya la quitó
//! [`super::salidas::leer`].

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::generacion::Documentos;
use super::plantillas::{self, FormatoDeSalida};
use super::rubrica;
use super::salidas::{self, Lectura};
use super::{contexto, Completion, Passage, ProviderInfo, Request};
use crate::error::{NbError, NbResult};
use crate::reader::Locator;

/// Cuántos fragmentos se entregan como mucho, quepan o no.
///
/// Con más, las citas dejan de servir para verificar: nadie abre cuarenta sitios
/// para comprobar un resumen, y un modelo pequeño confunde los números.
pub const MAXIMO_DE_FRAGMENTOS: usize = 24;

/// Caracteres de una selección que se usan.
pub const MAXIMO_DE_SELECCION: usize = 12_000;

/// Caracteres del extracto de cada fuente que se enseña junto a una cita.
const EXTRACTO: usize = 280;

/// Secciones que puede nombrar un alcance. Un PDF de mil páginas es mil secciones.
const MAXIMO_DE_SECCIONES: usize = 5_000;

/// Qué se le pide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyFunction"
)]
pub enum Funcion {
    Summary,
    Flashcards,
    Questions,
    Concepts,
}

impl Funcion {
    /// La plantilla que la hace.
    #[must_use]
    pub const fn plantilla(self) -> &'static str {
        match self {
            Self::Summary => "estudio.resumen",
            Self::Flashcards => "estudio.tarjetas",
            Self::Questions => "estudio.preguntas",
            Self::Concepts => "estudio.conceptos",
        }
    }

    /// Nombre estable, el que se guarda.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Flashcards => "flashcards",
            Self::Questions => "questions",
            Self::Concepts => "concepts",
        }
    }
}

/// Sobre qué parte del documento.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "StudyScope")]
pub enum AlcanceDeEstudio {
    /// Las secciones de un capítulo: en un EPUB, sus recursos; en un PDF, sus
    /// páginas. Las calcula la interfaz desde el índice, que es quien sabe dónde
    /// empieza y acaba cada capítulo.
    Section {
        edition_id: String,
        hrefs: Vec<String>,
        /// En un documento paginado, la primera página del capítulo. Un PDF es un
        /// solo recurso, así que sin esto un capítulo sería el libro entero.
        from_position: Option<u32>,
        /// La primera página que **ya no** es del capítulo.
        to_position: Option<u32>,
    },
    /// Lo que la persona seleccionó, con dónde empieza.
    Selection {
        edition_id: String,
        locator: Locator,
        text: String,
    },
}

impl AlcanceDeEstudio {
    #[must_use]
    pub fn edicion(&self) -> &str {
        match self {
            Self::Section { edition_id, .. } | Self::Selection { edition_id, .. } => edition_id,
        }
    }
}

/// Un fragmento entregado, para abrirlo desde una cita.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "StudySource")]
pub struct Fuente {
    pub locator: Locator,
    /// El principio del fragmento. Es texto del documento: se pinta como texto.
    pub excerpt: String,
}

/// Lo que devuelve una función de estudio.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyGeneration"
)]
pub struct Generacion {
    pub function: Funcion,
    pub edition_id: String,
    pub template_id: String,
    pub template_version: u32,
    /// Quién lo generó. **Siempre**: es la marca de salida generada (regla 4).
    pub provider_id: String,
    pub provider_label: String,
    /// Si el texto salió del dispositivo para generarlo.
    pub left_device: bool,
    pub reading: Lectura,
    /// Los fragmentos entregados, en orden: la cita `n` es `sources[n]`.
    pub sources: Vec<Fuente>,
    /// Si el alcance no cabía entero y se entregó una parte repartida.
    pub sampled: bool,
}

/// Estimación de lo que saldría al enviar una función de estudio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyScopeEstimate"
)]
pub struct EstimacionDeAlcance {
    #[ts(type = "number")]
    pub characters: usize,
    #[ts(type = "number")]
    pub approximate_tokens: usize,
    pub sampled: bool,
}

/// Quien genera: el proveedor que eligió la persona, visto desde aquí.
///
/// Un contrato y no el servicio directamente para poder probar el camino entero
/// con un modelo que obedece al documento.
pub trait Generadora: Send + Sync {
    /// Quién atendería ahora, o `None` si nadie.
    fn quien(&self) -> Option<ProviderInfo>;

    /// Caracteres de documento que admite de una vez.
    fn caracteres(&self) -> usize;

    /// Genera.
    ///
    /// # Errors
    ///
    /// Los del proveedor.
    fn generar(&self, request: &Request) -> NbResult<Completion>;
}

/// Hace una función de estudio sobre un alcance.
///
/// `idioma` es el de la respuesta, en etiqueta BCP-47.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si nadie puede generar.
/// - [`NbError::FileNotFound`] si el documento no tiene copia disponible, o
///   [`NbError::FormatUnsupported`] si no tiene texto.
/// - [`NbError::DocumentNotFound`] si el alcance no tiene texto: ninguna sección
///   nombrada existe, o la selección está vacía.
/// - Los del proveedor al generar. **Una salida que no se deja leer no es uno**:
///   vuelve como lectura ilegible.
pub fn hacer(
    documentos: &dyn Documentos,
    generadora: &dyn Generadora,
    funcion: Funcion,
    alcance: &AlcanceDeEstudio,
    idioma: &str,
) -> NbResult<Generacion> {
    hacer_y_conservar(documentos, generadora, funcion, alcance, idioma).map(|h| h.generacion)
}

/// Una función hecha, con lo que la evaluación necesita y la interfaz no.
#[derive(Debug, Clone)]
pub struct Hecha {
    pub generacion: Generacion,
    /// El texto entero de cada fragmento entregado, en el orden de las citas.
    pub textos: Vec<String>,
    /// Lo que devolvió el modelo, sin leer.
    pub salida: String,
}

/// Lo mismo que [`hacer`], conservando el texto de los fragmentos y la salida en
/// crudo. Lo usa la evaluación: la rúbrica comprueba lo generado contra el
/// texto del que salió, y la interfaz solo necesita su principio.
///
/// # Errors
///
/// Los de [`hacer`].
pub fn hacer_y_conservar(
    documentos: &dyn Documentos,
    generadora: &dyn Generadora,
    funcion: Funcion,
    alcance: &AlcanceDeEstudio,
    idioma: &str,
) -> NbResult<Hecha> {
    let quien = generadora.quien().ok_or(NbError::ComponentUnavailable)?;
    let plantilla = plantillas::plantilla(funcion.plantilla())?;

    let fragmentos = fragmentos_del_alcance(documentos, alcance)?;
    if fragmentos.is_empty() {
        return Err(NbError::DocumentNotFound);
    }
    let (elegidos, sampled) = repartir(fragmentos, generadora.caracteres());

    let nombre_del_idioma = nombre_de_idioma(idioma);
    let cuantas = cuantas_para(&elegidos).to_string();
    let valores: Vec<(&str, &str)> = plantilla
        .variables
        .iter()
        .map(|v| match *v {
            "cuantas" => ("cuantas", cuantas.as_str()),
            otra => (otra, nombre_del_idioma),
        })
        .collect();
    let rellena = plantilla.rellenar(&valores)?;

    let pasajes: Vec<Passage> = elegidos
        .into_iter()
        .map(|(locator, text)| Passage { locator, text })
        .collect();
    let peticion = contexto::peticion(&rellena, plantilla.capacidad, pasajes, None);
    let completado = generadora.generar(&peticion)?;

    let mut reading = salidas::leer(plantilla.salida, &completado.text, peticion.passages.len());
    let textos: Vec<String> = peticion.passages.iter().map(|p| p.text.clone()).collect();
    comprobar_las_citas(&mut reading, &textos);
    if reading.ilegible() {
        // Ni el texto ni su tamaño dicen nada del documento, pero sí ayudan a saber
        // si un modelo no cumple el formato o no contesta.
        tracing::info!(
            plantilla = plantilla.id,
            proveedor = %completado.provider_id,
            caracteres = completado.text.chars().count(),
            "una salida generada no se pudo leer"
        );
    }

    let generacion = Generacion {
        function: funcion,
        edition_id: alcance.edicion().to_owned(),
        template_id: plantilla.id.to_owned(),
        template_version: plantilla.version,
        provider_id: completado.provider_id,
        provider_label: quien.label,
        left_device: quien.placement.sale_del_dispositivo(),
        reading,
        sources: peticion
            .passages
            .into_iter()
            .map(|p| Fuente {
                excerpt: extracto(&p.text),
                locator: p.locator,
            })
            .collect(),
        sampled,
    };
    Ok(Hecha {
        generacion,
        textos,
        salida: completado.text,
    })
}

/// Comprueba cada cita contra el texto que dice citar, y la corrige o la quita.
///
/// # Por qué no se cree la cita del modelo
///
/// Porque medido no acierta. El 2026-09-17, con Qwen2.5-1.5B sobre el corpus de
/// evaluación, hubo tarjetas correctas citando el fragmento de al lado y preguntas
/// cuyo fragmento citado no decía nada de la respuesta. Una cita que abre un sitio
/// que no sostiene lo que se afirma es **peor que ninguna**: parece verificada.
///
/// Así que la cita del modelo es una pista. Se comprueba con el mismo apoyo léxico
/// que usa la rúbrica: si el fragmento citado sostiene la afirmación, se queda; si
/// no, se busca el que la sostiene; y si ninguno la sostiene, el elemento se queda
/// **sin cita**, que es como se enseña lo no verificable (regla 5 de
/// `philosophy.md`). Es lo que hace cierta la promesa —toda afirmación
/// enlaza a su fragmento de origen— en lugar de delegarla en un modelo pequeño.
///
/// Lo que esto **no** comprueba, y por eso no se dice: que la afirmación sea cierta.
/// Comprueba que sale de ese fragmento y no de la nada.
fn comprobar_las_citas(lectura: &mut Lectura, textos: &[String]) {
    let mejor = |afirmacion: &str, citas: &mut Vec<u32>| {
        let sostiene = |i: u32| {
            textos
                .get(i as usize)
                .is_some_and(|t| rubrica::apoyo(afirmacion, t) >= rubrica::SOSTENIDA)
        };
        citas.retain(|i| sostiene(*i));
        if !citas.is_empty() {
            return;
        }
        let candidato = textos
            .iter()
            .enumerate()
            .map(|(i, texto)| (i, rubrica::apoyo(afirmacion, texto)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, apoyo)| *apoyo >= rubrica::SOSTENIDA);
        if let Some((i, _)) = candidato {
            if let Ok(i) = u32::try_from(i) {
                citas.push(i);
            }
        }
    };

    match &mut lectura.items {
        salidas::Elementos::Resumen(puntos) => {
            for punto in puntos.iter_mut() {
                let texto = punto.text.clone();
                mejor(&texto, &mut punto.citations);
            }
        }
        salidas::Elementos::Tarjetas(tarjetas) => {
            for tarjeta in tarjetas.iter_mut() {
                let texto = format!("{} {}", tarjeta.question, tarjeta.answer);
                mejor(&texto, &mut tarjeta.citations);
            }
        }
        salidas::Elementos::Preguntas(preguntas) => {
            for pregunta in preguntas.iter_mut() {
                let clave = pregunta
                    .options
                    .get(pregunta.correct as usize)
                    .cloned()
                    .unwrap_or_default();
                let texto = format!("{clave} {}", pregunta.explanation);
                mejor(&texto, &mut pregunta.citations);
            }
        }
        salidas::Elementos::Conceptos(conceptos) => {
            for concepto in conceptos.iter_mut() {
                let texto = format!("{} {}", concepto.name, concepto.description);
                mejor(&texto, &mut concepto.citations);
            }
        }
    }
}

/// La plantilla de una función.
#[must_use]
pub fn plantilla_de(funcion: Funcion) -> Option<&'static plantillas::Plantilla> {
    plantillas::plantilla(funcion.plantilla()).ok()
}

/// Comprueba el formato que declara la plantilla de una función. Lo usa quien
/// guarda, para no aceptar tarjetas en un resumen.
#[must_use]
pub fn formato_de(funcion: Funcion) -> Option<FormatoDeSalida> {
    plantillas::plantilla(funcion.plantilla())
        .ok()
        .map(|p| p.salida)
}

/// Estima los caracteres y tokens aproximados que saldrían para un alcance.
///
/// # Errors
///
/// [`NbError::FileNotFound`] o [`NbError::FormatUnsupported`] si el documento no se puede abrir.
pub fn estimar(
    documentos: &dyn Documentos,
    alcance: &AlcanceDeEstudio,
    max_caracteres: usize,
) -> NbResult<EstimacionDeAlcance> {
    let fragmentos = match fragmentos_del_alcance(documentos, alcance) {
        Ok(f) => f,
        Err(NbError::DocumentNotFound) => Vec::new(),
        Err(e) => return Err(e),
    };
    let (elegidos, sampled) = repartir(fragmentos, max_caracteres);
    let characters: usize = elegidos.iter().map(|(_, t)| t.chars().count()).sum();
    let approximate_tokens = characters.div_ceil(4);
    Ok(EstimacionDeAlcance {
        characters,
        approximate_tokens,
        sampled,
    })
}

/// Los fragmentos del alcance, en el orden del documento.
fn fragmentos_del_alcance(
    documentos: &dyn Documentos,
    alcance: &AlcanceDeEstudio,
) -> NbResult<Vec<(Locator, String)>> {
    match alcance {
        AlcanceDeEstudio::Section {
            edition_id,
            hrefs,
            from_position,
            to_position,
        } => {
            if hrefs.is_empty() || hrefs.len() > MAXIMO_DE_SECCIONES {
                return Err(NbError::DocumentNotFound);
            }
            // Sin la parte de ancla: el índice de un capítulo apunta a veces a un
            // título dentro del recurso, y los fragmentos son del recurso entero.
            let hrefs: std::collections::HashSet<&str> = hrefs
                .iter()
                .map(|h| h.split('#').next().unwrap_or(h))
                .collect();
            let documento = documentos.abrir(edition_id)?;
            Ok(documento
                .text_chunks(crate::search::FRAGMENTO)?
                .into_iter()
                .filter(|(locator, texto)| {
                    let pagina = locator.locations.position;
                    let dentro = match pagina {
                        Some(p) => {
                            from_position.is_none_or(|desde| p >= desde)
                                && to_position.is_none_or(|hasta| p < hasta)
                        }
                        None => true,
                    };
                    dentro && hrefs.contains(locator.href.as_str()) && !texto.trim().is_empty()
                })
                .collect())
        }
        AlcanceDeEstudio::Selection { locator, text, .. } => {
            let texto: String = text.trim().chars().take(MAXIMO_DE_SELECCION).collect();
            if texto.is_empty() {
                return Ok(Vec::new());
            }
            // Troceada como el índice, para que el modelo pueda citar partes de una
            // selección larga; todas apuntan al principio de la selección, que es lo
            // único que se sabe abrir de ella.
            let caracteres: Vec<char> = texto.chars().collect();
            // Un párrafo empieza detrás de cada salto de línea.
            let fronteras: Vec<usize> = caracteres
                .iter()
                .enumerate()
                .filter(|(i, c)| **c == '\n' && i + 1 < caracteres.len())
                .map(|(i, _)| i + 1)
                .collect();
            Ok(crate::reader::segmentos::segmentar(
                &caracteres,
                &fronteras,
                crate::search::FRAGMENTO,
            )
            .into_iter()
            .map(|tramo| (locator.clone(), caracteres[tramo].iter().collect()))
            .collect())
        }
    }
}

/// Los fragmentos que caben, repartidos por todo el alcance, y si hubo que
/// recortar.
fn repartir(
    fragmentos: Vec<(Locator, String)>,
    caracteres: usize,
) -> (Vec<(Locator, String)>, bool) {
    let total: usize = fragmentos.iter().map(|(_, t)| t.chars().count()).sum();
    if total <= caracteres && fragmentos.len() <= MAXIMO_DE_FRAGMENTOS {
        return (fragmentos, false);
    }
    let medio = (total / fragmentos.len().max(1)).max(1);
    let cuantos = (caracteres / medio)
        .clamp(1, MAXIMO_DE_FRAGMENTOS)
        .min(fragmentos.len());
    let n = fragmentos.len();
    // Uno de cada tramo, empezando por el centro del primero: así entran el
    // principio, el final y lo de en medio.
    let indices: Vec<usize> = (0..cuantos)
        .map(|i| i * n / cuantos + n / (2 * cuantos))
        .collect();
    let mut usados = 0;
    let mut elegidos = Vec::with_capacity(cuantos);
    for (i, fragmento) in fragmentos.into_iter().enumerate() {
        if !indices.contains(&i) {
            continue;
        }
        let largo = fragmento.1.chars().count();
        if usados + largo > caracteres && !elegidos.is_empty() {
            break;
        }
        usados += largo;
        elegidos.push(fragmento);
    }
    (elegidos, true)
}

/// Cuántas tarjetas o preguntas se piden: una por cada dos fragmentos, entre tres
/// y diez. Más, sobre poco texto, obliga a preguntar por lo trivial.
fn cuantas_para(fragmentos: &[(Locator, String)]) -> usize {
    fragmentos.len().div_ceil(2).clamp(3, 10)
}

/// El nombre del idioma de la respuesta, como lo lee la instrucción.
fn nombre_de_idioma(etiqueta: &str) -> &'static str {
    match etiqueta.split(['-', '_']).next().unwrap_or("") {
        "en" => "inglés",
        "pt" => "portugués",
        "fr" => "francés",
        "de" => "alemán",
        "it" => "italiano",
        _ => "español",
    }
}

fn extracto(texto: &str) -> String {
    let limpio = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.chars().count() <= EXTRACTO {
        return limpio;
    }
    let corto: String = limpio.chars().take(EXTRACTO).collect();
    let corte = corto.rfind(' ').unwrap_or(corto.len());
    format!("{}…", &corto[..corte])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragmento(i: usize, largo: usize) -> (Locator, String) {
        (
            Locator::new(
                &format!("c{i}.xhtml"),
                "application/xhtml+xml",
                crate::reader::locator::Locations {
                    progression: Some(0.0),
                    ..crate::reader::locator::Locations::default()
                },
                None,
            )
            .expect("localizador"),
            "x".repeat(largo),
        )
    }

    /// Lo que no cabe se reparte por todo el alcance: entran el principio y el
    /// final, y nunca más de lo que se admite.
    #[test]
    fn lo_que_no_cabe_se_reparte_por_todo_el_alcance() {
        let fragmentos: Vec<_> = (0..100).map(|i| fragmento(i, 1000)).collect();
        let (elegidos, recortado) = repartir(fragmentos, 10_000);
        assert!(recortado);
        assert_eq!(elegidos.len(), 10);
        let primero: usize = elegidos[0].0.href[1..]
            .trim_end_matches(".xhtml")
            .parse()
            .expect("n");
        let ultimo: usize = elegidos[9].0.href[1..]
            .trim_end_matches(".xhtml")
            .parse()
            .expect("n");
        assert!(primero < 10, "no entró el principio: {primero}");
        assert!(ultimo >= 90, "no entró el final: {ultimo}");

        let pocos: Vec<_> = (0..3).map(|i| fragmento(i, 100)).collect();
        assert_eq!(
            repartir(pocos, 10_000),
            ((0..3).map(|i| fragmento(i, 100)).collect(), false)
        );

        let muchos_cortos: Vec<_> = (0..200).map(|i| fragmento(i, 10)).collect();
        let (e, r) = repartir(muchos_cortos, 1_000_000);
        assert!(r && e.len() == MAXIMO_DE_FRAGMENTOS);
    }

    fn textos_de_prueba() -> Vec<String> {
        vec![
            "La ventilación natural se basa en dos aberturas situadas a distinta altura.".to_owned(),
            "Una malla de sombreo del cincuenta por ciento baja la temperatura interior entre cuatro y seis grados."
                .to_owned(),
        ]
    }

    /// **La cita del modelo se comprueba, no se cree.** La que apunta al fragmento
    /// equivocado se corrige al que sí sostiene lo dicho, y la de una afirmación que
    /// no está en ningún fragmento se quita: eso es lo no verificable.
    #[test]
    fn una_cita_que_el_texto_no_sostiene_se_corrige_o_se_quita() {
        let salida = r#"{"tarjetas": [
            {"pregunta": "¿Cuánto baja la temperatura una malla de sombreo?", "respuesta": "Entre cuatro y seis grados", "citas": ["F1"]},
            {"pregunta": "¿En qué se basa la ventilación natural?", "respuesta": "En dos aberturas situadas a distinta altura", "citas": ["F1"]},
            {"pregunta": "¿Quién inventó el invernadero?", "respuesta": "Los romanos en el siglo primero", "citas": ["F2"]}
        ]}"#;
        let textos = textos_de_prueba();
        let mut lectura = salidas::leer(FormatoDeSalida::Tarjetas, salida, textos.len());
        comprobar_las_citas(&mut lectura, &textos);
        let salidas::Elementos::Tarjetas(tarjetas) = &lectura.items else {
            panic!("se esperaban tarjetas");
        };
        assert_eq!(
            tarjetas[0].citations,
            vec![1],
            "citaba el fragmento de al lado"
        );
        assert_eq!(tarjetas[1].citations, vec![0], "la cita buena se queda");
        assert!(
            tarjetas[2].citations.is_empty(),
            "lo que no está en ningún fragmento no puede citar ninguno"
        );
    }

    #[test]
    fn el_idioma_se_nombra_y_lo_desconocido_es_espanol() {
        assert_eq!(nombre_de_idioma("pt-BR"), "portugués");
        assert_eq!(nombre_de_idioma("en"), "inglés");
        assert_eq!(nombre_de_idioma("{{x}}"), "español");
    }

    #[test]
    fn estimar_calcula_caracteres_y_tokens_aproximados() {
        struct SinDocs;
        impl Documentos for SinDocs {
            fn abrir(&self, _ed: &str) -> NbResult<Box<dyn crate::reader::DocumentProvider>> {
                Err(NbError::FileNotFound)
            }
        }

        let texto = "Macondo era entonces una aldea de veinte casas de barro y cañabrava.";
        let alcance = AlcanceDeEstudio::Selection {
            edition_id: "ed-1".to_owned(),
            locator: Locator {
                version: 1,
                href: "c1.xhtml".to_owned(),
                media_type: "application/xhtml+xml".to_owned(),
                title: None,
                locations: crate::reader::Locations::default(),
                text: None,
            },
            text: texto.to_owned(),
        };

        let est = estimar(&SinDocs, &alcance, 10_000).expect("estimar");
        assert_eq!(est.characters, texto.chars().count());
        assert_eq!(est.approximate_tokens, texto.chars().count().div_ceil(4));
        assert!(!est.sampled);
    }
}
