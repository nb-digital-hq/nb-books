//! Leer lo que devuelve un modelo, con la desconfianza que merece.
//!
//! Contrato: `docs/architecture/ai/prompts.md`, sección 2.
//!
//! # Degradar, no romper
//!
//! Un modelo pequeño no siempre cumple el formato: envuelve el JSON en un bloque de
//! código, añade una frase antes, se deja una coma o inventa un campo. Si leer la
//! salida fuera todo o nada, una tarjeta mal escrita haría perder las otras nueve, y
//! una respuesta sin JSON dejaría la pantalla en un error. Aquí se lee **elemento a
//! elemento**: lo que cumple se queda, lo que no se cuenta como descartado, y si no
//! queda nada la lectura lo dice con [`Lectura::ilegible`] en lugar de fallar.
//!
//! # Las citas solo pueden apuntar a lo que se entregó
//!
//! Una cita es el número de un fragmento de los que se le dieron al modelo, `F1` a
//! `Fn`. Cualquier otra —un número fuera de rango, un nombre de archivo, una URL—
//! se descarta al leer, y un elemento que se queda sin ninguna cita válida se marca
//! **no verificable**, que es lo que la interfaz enseña (regla 5 de
//! `philosophy.md`). Así una salida no puede afirmar que algo viene del documento
//! sin que se pueda abrir el sitio del que viene.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::plantillas::FormatoDeSalida;

/// Caracteres máximos de un campo de texto. Lo que pase se corta: una tarjeta de
/// dos mil caracteres no es una tarjeta, y la pantalla no tiene por qué pagarla.
pub const MAXIMO_DE_CAMPO: usize = 600;

/// Elementos máximos que se leen de una salida.
pub const MAXIMO_DE_ELEMENTOS: usize = 30;

/// Cuánto pueden parecerse dos opciones de una pregunta antes de que dejen de ser
/// dos opciones: **tres cuartas partes de sus palabras**.
///
/// El número sale de lo medido: las opciones que hay que descartar son la misma
/// frase con una palabra cambiada, que con veinte palabras se parecen en un 0,90, y
/// las que hay que conservar son respuestas distintas a la misma pregunta, que
/// comparten poco más que los artículos.
pub const PARECIDO_MAXIMO: f64 = 0.75;

/// Un punto de un resumen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "SummaryPoint")]
pub struct Punto {
    pub text: String,
    /// Índices, desde cero, de los fragmentos que lo sostienen.
    pub citations: Vec<u32>,
}

/// Una tarjeta de estudio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "GeneratedCard"
)]
pub struct Tarjeta {
    pub question: String,
    pub answer: String,
    pub citations: Vec<u32>,
}

/// Una pregunta de autoevaluación de opción múltiple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "GeneratedQuestion"
)]
pub struct Pregunta {
    pub prompt: String,
    pub options: Vec<String>,
    pub correct: u32,
    pub explanation: String,
    pub citations: Vec<u32>,
}

/// La clase de un concepto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "ConceptKind")]
pub enum TipoDeConcepto {
    Persona,
    Lugar,
    Obra,
    Organizacion,
    Fecha,
    Concepto,
}

impl TipoDeConcepto {
    /// Nombre estable, el que se guarda.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Persona => "persona",
            Self::Lugar => "lugar",
            Self::Obra => "obra",
            Self::Organizacion => "organizacion",
            Self::Fecha => "fecha",
            Self::Concepto => "concepto",
        }
    }

    #[must_use]
    pub fn de(nombre: &str) -> Option<Self> {
        let limpio = sin_tildes(&nombre.trim().to_lowercase());
        Some(match limpio.as_str() {
            "persona" | "person" => Self::Persona,
            "lugar" | "place" => Self::Lugar,
            "obra" | "work" => Self::Obra,
            "organizacion" | "organization" => Self::Organizacion,
            "fecha" | "date" => Self::Fecha,
            "concepto" | "concept" => Self::Concepto,
            _ => return None,
        })
    }
}

/// Un concepto o una entidad del texto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "GeneratedConcept"
)]
pub struct Concepto {
    pub name: String,
    pub kind: TipoDeConcepto,
    pub description: String,
    pub citations: Vec<u32>,
}

/// Los elementos leídos de una salida, del formato que sea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", content = "items")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "StudyItems")]
pub enum Elementos {
    Resumen(Vec<Punto>),
    Tarjetas(Vec<Tarjeta>),
    Preguntas(Vec<Pregunta>),
    Conceptos(Vec<Concepto>),
}

impl Elementos {
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Resumen(v) => v.len(),
            Self::Tarjetas(v) => v.len(),
            Self::Preguntas(v) => v.len(),
            Self::Conceptos(v) => v.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Lo que se sacó de una salida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "StudyReading")]
pub struct Lectura {
    pub items: Elementos,
    /// Elementos que venían y no cumplían. La interfaz lo dice si no es cero.
    pub discarded: u32,
    /// Citas quitadas por no apuntar a un fragmento entregado.
    pub invalid_citations: u32,
}

impl Lectura {
    /// Si no se pudo sacar nada. No es un error: la interfaz lo enseña como
    /// "la respuesta no se pudo leer" y ofrece repetir.
    #[must_use]
    pub fn ilegible(&self) -> bool {
        self.items.is_empty()
    }
}

/// Lee la salida de un modelo con el formato que pidió la plantilla.
///
/// `fragmentos` es cuántos se entregaron: las citas fuera de `1..=fragmentos` se
/// descartan.
#[must_use]
pub fn leer(formato: FormatoDeSalida, texto: &str, fragmentos: usize) -> Lectura {
    let mut contador = Contador {
        fragmentos,
        invalidas: 0,
    };
    let (clave, vacios) = match formato {
        FormatoDeSalida::Resumen => ("puntos", Elementos::Resumen(Vec::new())),
        FormatoDeSalida::Tarjetas => ("tarjetas", Elementos::Tarjetas(Vec::new())),
        FormatoDeSalida::Preguntas => ("preguntas", Elementos::Preguntas(Vec::new())),
        FormatoDeSalida::Conceptos => ("conceptos", Elementos::Conceptos(Vec::new())),
    };
    // Se leen las dos formas y se queda la que saca más: un JSON válido puede
    // esconder elementos, porque un modelo que repite la clave de la lista
    // —medido con Qwen3-1.7B: `{"tarjetas":[…],"tarjetas":[…]}`— deja al lector
    // solo con la última.
    let entera = extraer(texto).and_then(|valor| lista_de(valor, clave));
    let rescatada = rescatar(texto, formato);
    let lista = match (entera, rescatada) {
        (Some(e), Some(r)) if r.len() > e.len() => Some(r),
        (Some(e), _) => Some(e),
        (None, r) => r,
    };
    let Some(lista) = lista else {
        return Lectura {
            items: vacios,
            discarded: 0,
            invalid_citations: 0,
        };
    };
    let total = lista.len().min(MAXIMO_DE_ELEMENTOS);
    let lista = &lista[..total];

    let items = match formato {
        FormatoDeSalida::Resumen => Elementos::Resumen(
            lista
                .iter()
                .filter_map(|v| punto(v, &mut contador))
                .collect(),
        ),
        FormatoDeSalida::Tarjetas => Elementos::Tarjetas(
            lista
                .iter()
                .filter_map(|v| tarjeta(v, &mut contador))
                .collect(),
        ),
        FormatoDeSalida::Preguntas => Elementos::Preguntas(
            lista
                .iter()
                .filter_map(|v| pregunta(v, &mut contador))
                .collect(),
        ),
        FormatoDeSalida::Conceptos => Elementos::Conceptos(deduplicar(
            lista
                .iter()
                .filter_map(|v| concepto(v, &mut contador))
                .collect(),
        )),
    };
    let leidos = items.len();
    Lectura {
        items,
        discarded: u32::try_from(total.saturating_sub(leidos)).unwrap_or(u32::MAX),
        invalid_citations: contador.invalidas,
    }
}

struct Contador {
    fragmentos: usize,
    invalidas: u32,
}

impl Contador {
    /// Las citas válidas de un elemento, como índices desde cero y sin repetir.
    fn citas(&mut self, valor: Option<&Value>) -> Vec<u32> {
        let crudas: Vec<&Value> = match valor {
            Some(Value::Array(v)) => v.iter().collect(),
            Some(otro) => vec![otro],
            None => Vec::new(),
        };
        let mut citas = Vec::new();
        for cruda in crudas {
            match numero_de_cita(cruda).filter(|n| (1..=self.fragmentos).contains(n)) {
                Some(n) => {
                    let indice = u32::try_from(n - 1).unwrap_or(u32::MAX);
                    if !citas.contains(&indice) {
                        citas.push(indice);
                    }
                }
                None => self.invalidas += 1,
            }
        }
        citas.sort_unstable();
        citas
    }
}

/// `F3`, `f3`, `"3"` o `3` son la cita del tercer fragmento. Nada más lo es.
fn numero_de_cita(valor: &Value) -> Option<usize> {
    match valor {
        Value::Number(n) => n.as_u64().and_then(|n| usize::try_from(n).ok()),
        Value::String(s) => {
            let s = s.trim();
            let s = s
                .strip_prefix('F')
                .or_else(|| s.strip_prefix('f'))
                .unwrap_or(s);
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || s.len() > 4 {
                return None;
            }
            s.parse().ok()
        }
        _ => None,
    }
}

/// Los elementos sueltos de una salida cuyo JSON no se deja leer entero.
///
/// Un modelo pequeño se deja una llave de más al cerrar —medido con
/// Qwen2.5-1.5B el 2026-09-17: cuatro tarjetas correctas y un `}}]` final— y con
/// eso el objeto entero deja de ser JSON. Tirar las cuatro por la llave sería
/// romper justo lo que la lectura promete, así que se rescata cada objeto que
/// tenga la clave que distingue a un elemento de su formato.
fn rescatar(texto: &str, formato: FormatoDeSalida) -> Option<Vec<Value>> {
    let claves: &[&str] = match formato {
        FormatoDeSalida::Resumen => &["texto", "text", "punto"],
        FormatoDeSalida::Tarjetas => &["pregunta", "question", "anverso", "front"],
        FormatoDeSalida::Preguntas => &["enunciado", "opciones", "options"],
        FormatoDeSalida::Conceptos => &["nombre", "name"],
    };
    let mut elementos = Vec::new();
    let mut desde = 0;
    while let Some(relativa) = texto[desde..].find('{') {
        let inicio = desde + relativa;
        let mut lector = serde_json::Deserializer::from_str(&texto[inicio..]).into_iter::<Value>();
        match lector.next() {
            Some(Ok(valor @ Value::Object(_)))
                if claves.iter().any(|c| valor.get(*c).is_some()) =>
            {
                elementos.push(valor);
                desde = inicio + lector.byte_offset();
            }
            _ => desde = inicio + 1,
        }
        if elementos.len() >= MAXIMO_DE_ELEMENTOS {
            break;
        }
    }
    (!elementos.is_empty()).then_some(elementos)
}

/// El primer objeto JSON del texto que se deje leer.
///
/// Se prueba desde cada llave de apertura, porque un modelo pone a veces una frase
/// o un bloque de código delante, y alguno escribe llaves dentro de esa frase.
fn extraer(texto: &str) -> Option<Value> {
    for (posicion, _) in texto.match_indices('{') {
        let mut lector =
            serde_json::Deserializer::from_str(&texto[posicion..]).into_iter::<Value>();
        if let Some(Ok(valor @ Value::Object(_))) = lector.next() {
            return Some(valor);
        }
    }
    // Un modelo que devuelve la lista sin el objeto de fuera también se entiende.
    for (posicion, _) in texto.match_indices('[') {
        let mut lector =
            serde_json::Deserializer::from_str(&texto[posicion..]).into_iter::<Value>();
        if let Some(Ok(valor @ Value::Array(_))) = lector.next() {
            return Some(valor);
        }
    }
    None
}

/// La lista de elementos: la de la clave pedida o, si el objeto trae una sola
/// lista con otro nombre, esa.
fn lista_de(valor: Value, clave: &str) -> Option<Vec<Value>> {
    match valor {
        Value::Array(lista) => Some(lista),
        Value::Object(mut objeto) => {
            if let Some(Value::Array(lista)) = objeto.remove(clave) {
                return Some(lista);
            }
            let mut listas = objeto.into_iter().filter_map(|(_, v)| match v {
                Value::Array(lista) => Some(lista),
                _ => None,
            });
            let unica = listas.next()?;
            listas.next().is_none().then_some(unica)
        }
        _ => None,
    }
}

/// Un campo de texto no vacío, en una línea de espacios normales y cortado.
fn texto(objeto: &Value, claves: &[&str]) -> Option<String> {
    let crudo = claves.iter().find_map(|c| objeto.get(*c)?.as_str())?;
    let limpio = crudo.split_whitespace().collect::<Vec<_>>().join(" ");
    if limpio.is_empty() {
        return None;
    }
    Some(if limpio.chars().count() > MAXIMO_DE_CAMPO {
        let mut corto: String = limpio.chars().take(MAXIMO_DE_CAMPO - 1).collect();
        corto.push('…');
        corto
    } else {
        limpio
    })
}

fn punto(v: &Value, contador: &mut Contador) -> Option<Punto> {
    let text = match v {
        Value::String(_) => texto(&serde_json::json!({ "texto": v }), &["texto"])?,
        _ => texto(v, &["texto", "text", "punto"])?,
    };
    Some(Punto {
        text,
        citations: contador.citas(
            v.get("citas")
                .or_else(|| v.get("cita"))
                .or_else(|| v.get("citations")),
        ),
    })
}

fn tarjeta(v: &Value, contador: &mut Contador) -> Option<Tarjeta> {
    let question = texto(v, &["pregunta", "question", "anverso", "front"])?;
    let answer = texto(v, &["respuesta", "answer", "reverso", "back"])?;
    if normalizar(&question) == normalizar(&answer) {
        return None;
    }
    Some(Tarjeta {
        question,
        answer,
        citations: contador.citas(
            v.get("citas")
                .or_else(|| v.get("cita"))
                .or_else(|| v.get("citations")),
        ),
    })
}

fn pregunta(v: &Value, contador: &mut Contador) -> Option<Pregunta> {
    let prompt = texto(v, &["enunciado", "pregunta", "question"])?;
    let options: Vec<String> = v
        .get("opciones")
        .or_else(|| v.get("options"))?
        .as_array()?
        .iter()
        .filter_map(|o| texto(&serde_json::json!({ "t": o }), &["t"]))
        .collect();
    if !(3..=5).contains(&options.len()) {
        return None;
    }
    // **Las opciones tienen que poder distinguirse.** No basta con que no sean
    // idénticas: medido el 2026-09-17 con Qwen2.5-1.5B, una pregunta traía las
    // cuatro opciones exactamente iguales y otras dos, la misma frase del texto con
    // una palabra cambiada —"luz visible" frente a "luz infrarroja"— y la clave
    // puesta en la equivocada. Un distractor que se diferencia en una palabra no es
    // un distractor plausible: es una trampa, y una clave mal puesta ahí hace que se
    // memorice un error, que es el daño que `philosophy.md` describe. Lo que no se
    // puede distinguir, no se enseña.
    if se_parecen_demasiado(&options) {
        return None;
    }
    // **La clave se pide como texto, no como número**, y esta es la razón: medido el
    // 2026-09-17, Qwen2.5-1.5B contestaba `"correcta": 1` señalando la primera
    // opción, contando desde uno aunque la plantilla dijera desde cero. Una pregunta
    // con la clave corrida es exactamente el daño que `philosophy.md` describe: el
    // estudiante memoriza un error. Copiando la opción no hay nada que desplazar.
    // El número se sigue admitiendo, y sigue siendo desde cero, para una salida de
    // otro modelo que lo mande así.
    let correct = texto(v, &["respuesta", "answer", "correcta", "correct"])
        .and_then(|respuesta| {
            let buscada = normalizar(&respuesta);
            options
                .iter()
                .position(|o| normalizar(o) == buscada)
                .and_then(|i| u32::try_from(i).ok())
        })
        .or_else(|| {
            v.get("correcta")
                .or_else(|| v.get("correct"))?
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| (*n as usize) < options.len())
        })?;
    let explanation = texto(v, &["explicacion", "explicación", "explanation"]).unwrap_or_default();
    Some(Pregunta {
        prompt,
        options,
        correct,
        explanation,
        citations: contador.citas(
            v.get("citas")
                .or_else(|| v.get("cita"))
                .or_else(|| v.get("citations")),
        ),
    })
}

/// Si dos opciones comparten demasiadas palabras como para ser opciones distintas.
///
/// Se compara por palabras y no por letras porque lo que las hace indistinguibles es
/// repetir la frase entera cambiando una: por letras, dos frases de veinte palabras
/// que difieren en una salen casi idénticas y dos redacciones distintas de la misma
/// idea también, así que no separaría lo uno de lo otro.
fn se_parecen_demasiado(opciones: &[String]) -> bool {
    /// Palabras compartidas sobre palabras distintas de las dos, en tanto por uno.
    fn parecido(a: &str, b: &str) -> f64 {
        let de = |t: &str| -> std::collections::HashSet<String> {
            normalizar(t)
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        };
        let (a, b) = (de(a), de(b));
        let union = a.union(&b).count();
        if union == 0 {
            return 1.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let p = a.intersection(&b).count() as f64 / union as f64;
        p
    }

    opciones.iter().enumerate().any(|(i, a)| {
        opciones
            .iter()
            .skip(i + 1)
            .any(|b| parecido(a, b) >= PARECIDO_MAXIMO)
    })
}

fn concepto(v: &Value, contador: &mut Contador) -> Option<Concepto> {
    let name = texto(v, &["nombre", "name"])?;
    if name.chars().count() > 120 {
        return None;
    }
    let kind = v
        .get("tipo")
        .or_else(|| v.get("kind"))
        .and_then(Value::as_str)
        .and_then(TipoDeConcepto::de)
        .unwrap_or(TipoDeConcepto::Concepto);
    Some(Concepto {
        name,
        kind,
        description: texto(v, &["descripcion", "descripción", "description"]).unwrap_or_default(),
        citations: contador.citas(
            v.get("citas")
                .or_else(|| v.get("cita"))
                .or_else(|| v.get("citations")),
        ),
    })
}

/// Un concepto que aparece dos veces se queda una, con las citas de las dos.
fn deduplicar(conceptos: Vec<Concepto>) -> Vec<Concepto> {
    let mut unicos: Vec<Concepto> = Vec::with_capacity(conceptos.len());
    for c in conceptos {
        if let Some(previo) = unicos
            .iter_mut()
            .find(|u| normalizar(&u.name) == normalizar(&c.name))
        {
            for cita in c.citations {
                if !previo.citations.contains(&cita) {
                    previo.citations.push(cita);
                }
            }
            previo.citations.sort_unstable();
        } else {
            unicos.push(c);
        }
    }
    unicos
}

/// Para comparar dos textos sin que cuenten mayúsculas, tildes ni puntuación.
pub fn normalizar(texto: &str) -> String {
    sin_tildes(&texto.to_lowercase())
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sin_tildes(texto: &str) -> String {
    texto
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            otro => otro,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tarjetas(lectura: &Lectura) -> &[Tarjeta] {
        match &lectura.items {
            Elementos::Tarjetas(t) => t,
            otro => panic!("se esperaban tarjetas: {otro:?}"),
        }
    }

    #[test]
    fn una_salida_correcta_se_lee_entera() {
        let salida = r#"{"tarjetas": [
            {"pregunta": "¿Quién?", "respuesta": "Gutenberg", "citas": ["F1"]},
            {"pregunta": "¿Cuándo?", "respuesta": "1492", "citas": ["F2", "F1"]}
        ]}"#;
        let l = leer(FormatoDeSalida::Tarjetas, salida, 2);
        assert_eq!(l.discarded, 0);
        assert_eq!(l.invalid_citations, 0);
        let t = tarjetas(&l);
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].citations, vec![0, 1]);
    }

    /// **El criterio.** Nada de lo que devuelva un modelo hace fallar la
    /// lectura: lo que no se entiende se queda fuera, y si no se entiende nada la
    /// lectura lo dice.
    #[test]
    fn una_salida_malformada_degrada_en_vez_de_fallar() {
        for basura in [
            "",
            "Lo siento, no puedo ayudar con eso.",
            "{",
            "{\"tarjetas\": \"no es una lista\"}",
            "{\"otra\": [1], \"cosa\": [2]}",
            "[[[[",
            "{\"tarjetas\": [null, 3, \"x\", {}]}",
            "\u{0}\u{fffd}{{{}}}",
        ] {
            let l = leer(FormatoDeSalida::Tarjetas, basura, 3);
            assert!(l.ilegible(), "{basura:?} dio {l:?}");
        }
        for formato in [
            FormatoDeSalida::Resumen,
            FormatoDeSalida::Preguntas,
            FormatoDeSalida::Conceptos,
        ] {
            assert!(leer(formato, "no es JSON", 3).ilegible());
        }
    }

    /// Un modelo pequeño envuelve, antecede y se equivoca en un elemento; lo que
    /// cumple se queda y lo demás se cuenta.
    #[test]
    fn lo_que_cumple_se_queda_y_lo_demas_se_cuenta() {
        let salida = "Claro, aquí tienes {las tarjetas}:\n```json\n{\"tarjetas\": [
            {\"pregunta\": \"¿Qué llegó?\", \"respuesta\": \"La imprenta\", \"citas\": [\"F1\"]},
            {\"pregunta\": \"sin respuesta\"},
            {\"pregunta\": \"Eco\", \"respuesta\": \"eco.\", \"citas\": [\"F1\"]},
            {\"pregunta\": \"¿Dónde?\", \"respuesta\": \"Sevilla\", \"citas\": \"F2\"}
        ]}\n```\nEspero que te sirva.";
        let l = leer(FormatoDeSalida::Tarjetas, salida, 2);
        let t = tarjetas(&l);
        assert_eq!(t.len(), 2);
        assert_eq!(l.discarded, 2, "la vacía y la que repite la pregunta");
        assert_eq!(t[1].citations, vec![1]);
    }

    /// Una cita que no apunta a un fragmento entregado no existe.
    #[test]
    fn solo_se_admiten_citas_a_los_fragmentos_entregados() {
        let salida = r#"{"puntos": [
            {"texto": "Uno", "citas": ["F0", "F4", "F2", "https://x.invalid", "capitulo-7.xhtml", "F2", -1, 1]},
            {"texto": "Dos", "citas": []},
            "Tres, sin objeto"
        ]}"#;
        let l = leer(FormatoDeSalida::Resumen, salida, 3);
        let Elementos::Resumen(puntos) = &l.items else {
            panic!()
        };
        assert_eq!(puntos.len(), 3);
        assert_eq!(puntos[0].citations, vec![0, 1]);
        assert_eq!(l.invalid_citations, 5);
        assert!(puntos[1].citations.is_empty() && puntos[2].citations.is_empty());
    }

    /// La clave copiada como texto es la que manda; el número sigue valiendo, y una
    /// respuesta que no es ninguna de las opciones no cuela.
    #[test]
    fn la_clave_de_una_pregunta_se_reconoce_por_el_texto_de_la_opcion() {
        let salida = r#"{"preguntas": [
            {"enunciado": "¿Por dónde sale el aire caliente?", "opciones": ["Por la abertura superior", "Por la inferior", "Por el vidrio"], "respuesta": "por la abertura SUPERIOR", "explicacion": "Lo dice el texto", "citas": ["F1"]},
            {"enunciado": "¿Y el fresco?", "opciones": ["Por arriba", "Por abajo", "Por el suelo"], "correcta": 1, "explicacion": "Lo dice el texto", "citas": ["F1"]},
            {"enunciado": "¿Y el tercero?", "opciones": ["Uno", "Dos", "Tres"], "respuesta": "Cuatro", "citas": ["F1"]}
        ]}"#;
        let l = leer(FormatoDeSalida::Preguntas, salida, 1);
        let Elementos::Preguntas(p) = &l.items else {
            panic!()
        };
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].correct, 0, "la opción copiada, sin contar posiciones");
        assert_eq!(p[1].correct, 1);
        assert_eq!(
            l.discarded, 1,
            "una respuesta que no es ninguna opción no vale"
        );
    }

    /// La salida real de Qwen2.5-1.5B el 2026-09-17: cuatro opciones que son la
    /// misma frase, y otra pregunta con la frase del texto repetida con una palabra
    /// cambiada. Ninguna de las dos es una pregunta.
    #[test]
    fn unas_opciones_que_no_se_distinguen_no_son_una_pregunta() {
        let salida = r#"{"preguntas": [
            {"enunciado": "¿Qué hace la sombra?", "opciones": [
                "La sombra reduce la radiación que llega a las plantas y baja la temperatura",
                "La sombra reduce la radiación que llega a las plantas y baja la temperatura",
                "La sombra reduce la radiación que llega a las plantas y baja la temperatura",
                "La sombra reduce la radiación que llega a las plantas y baja la temperatura"],
             "respuesta": "La sombra reduce la radiación que llega a las plantas y baja la temperatura", "citas": ["F1"]},
            {"enunciado": "¿Qué deja pasar el vidrio?", "opciones": [
                "El vidrio deja pasar la luz visible y retiene la radiación infrarroja",
                "El vidrio deja pasar la luz infrarroja y retiene la radiación infrarroja",
                "El vidrio deja pasar la luz visible y retiene la radiación visible",
                "El vidrio deja pasar la luz infrarroja y retiene la radiación visible"],
             "respuesta": "El vidrio deja pasar la luz visible y retiene la radiación infrarroja", "citas": ["F1"]},
            {"enunciado": "¿Por dónde sale el aire caliente?", "opciones": [
                "Por la abertura superior", "Por la inferior", "Por el vidrio", "Por las paredes"],
             "respuesta": "Por la abertura superior", "citas": ["F1"]}
        ]}"#;
        let l = leer(FormatoDeSalida::Preguntas, salida, 1);
        let Elementos::Preguntas(p) = &l.items else {
            panic!()
        };
        assert_eq!(p.len(), 1, "solo la de opciones cortas y distintas");
        assert_eq!(p[0].correct, 0);
        assert_eq!(l.discarded, 2);
    }

    #[test]
    fn una_pregunta_necesita_opciones_distintas_y_una_clave_que_exista() {
        let salida = r#"{"preguntas": [
            {"enunciado": "¿A?", "opciones": ["a", "b", "c", "d"], "correcta": 2, "explicacion": "Porque", "citas": ["F1"]},
            {"enunciado": "¿B?", "opciones": ["a", "b", "c", "d"], "correcta": 4},
            {"enunciado": "¿C?", "opciones": ["a", "A", "c", "d"], "correcta": 0},
            {"enunciado": "¿D?", "opciones": ["a", "b"], "correcta": 0}
        ]}"#;
        let l = leer(FormatoDeSalida::Preguntas, salida, 1);
        let Elementos::Preguntas(p) = &l.items else {
            panic!()
        };
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].correct, 2);
        assert_eq!(l.discarded, 3);
    }

    #[test]
    fn los_conceptos_repetidos_se_juntan_con_sus_citas() {
        let salida = r#"{"conceptos": [
            {"nombre": "Nebrija", "tipo": "persona", "descripcion": "Gramático", "citas": ["F2"]},
            {"nombre": "nebrija", "tipo": "Persona", "citas": ["F1"]},
            {"nombre": "Imprenta", "tipo": "invento", "citas": ["F1"]}
        ]}"#;
        let l = leer(FormatoDeSalida::Conceptos, salida, 2);
        let Elementos::Conceptos(c) = &l.items else {
            panic!()
        };
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].citations, vec![0, 1]);
        assert_eq!(
            c[1].kind,
            TipoDeConcepto::Concepto,
            "un tipo que no existe es concepto"
        );
    }

    /// La salida real de Qwen2.5-1.5B el 2026-09-17: cuatro tarjetas buenas, la
    /// cita en singular y una llave de más al cerrar.
    #[test]
    fn un_json_roto_al_cerrar_no_se_lleva_los_elementos_buenos() {
        let salida = r#"{"tarjetas":[{"pregunta":"¿Qué llegó a la península?","respuesta":"La imprenta de tipos móviles","cita":"F1"},{"pregunta":"¿Dónde se instalaron los talleres?","respuesta":"Valencia, Zaragoza, Sevilla y Salamanca","cita":"F1"}}]"#;
        let l = leer(FormatoDeSalida::Tarjetas, salida, 1);
        let t = tarjetas(&l);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].citations, vec![0]);
        // La clave de la lista repetida, que en JSON deja solo la última.
        let repetida = r#"{"tarjetas":[{"pregunta":"¿El primero?","respuesta":"Uno","cita":"F1"}],"tarjetas":[{"pregunta":"¿El segundo?","respuesta":"Dos","cita":"F1"}]}"#;
        assert_eq!(
            tarjetas(&leer(FormatoDeSalida::Tarjetas, repetida, 1)).len(),
            2
        );
        // Un objeto sin la clave de su formato no se rescata como elemento.
        assert!(leer(FormatoDeSalida::Tarjetas, r#"{"x": 1} {"y": {"z": 2}"#, 1).ilegible());
    }

    #[test]
    fn un_campo_desmesurado_se_corta() {
        let largo = "palabra ".repeat(500);
        let salida =
            serde_json::json!({"tarjetas": [{"pregunta": largo, "respuesta": "r"}]}).to_string();
        let l = leer(FormatoDeSalida::Tarjetas, &salida, 1);
        assert_eq!(tarjetas(&l)[0].question.chars().count(), MAXIMO_DE_CAMPO);
    }
}
