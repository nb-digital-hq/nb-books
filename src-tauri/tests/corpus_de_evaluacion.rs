//! El corpus de evaluación se sostiene solo.
//!
//! # Qué comprueba esto, y qué no puede comprobar
//!
//! Un corpus de evaluación se pudre en silencio: alguien corrige una frase del
//! texto, el hecho de referencia deja de estar en él, y la rúbrica sigue dando un
//! número que ya no significa lo que decía. Esta prueba lo impide en lo que se puede
//! comprobar sin una persona:
//!
//! * Cada texto tiene hechos de referencia y cada lista de hechos tiene su texto.
//! * **Cada hecho está sostenido por su texto**: sus palabras con contenido
//!   aparecen ahí. Un hecho que se despega de su texto se ve al instante.
//! * El corpus cubre las clases que `evaluation.md` exige: los tres idiomas, y
//!   técnico, narrativo, académico, OCR roto, fórmulas con tablas y narrativo largo.
//!
//! Lo que **no** puede comprobar es si el hecho es verdad y si es el hecho que
//! importa. Eso pide una persona distinta de quien escribió el texto, y está dicho
//! como pendiente en `deferred-verification.md`: una comprobación automática que se
//! presentara como esa revisión sería justo la clase de afirmación que este
//! proyecto no hace.

use std::collections::BTreeMap;

const REFERENCIAS: &str = include_str!("fixtures/evaluacion/referencias.json");

/// Los textos del corpus con su idioma y la clase que cubren.
///
/// La tabla vive aquí y no se deduce de la carpeta a propósito: lo que hace falta
/// comprobar es que **las clases exigidas están cubiertas**, y una lista deducida de
/// lo que hay no puede echar en falta lo que no hay.
const EXIGIDOS: &[(&str, &str, &str)] = &[
    ("tecnico-es.txt", "es", "técnico"),
    ("formulas-es.txt", "es", "fórmulas y tablas"),
    ("narrativo-en.txt", "en", "narrativo"),
    ("narrativo-largo-en.txt", "en", "narrativo largo"),
    ("academico-pt.txt", "pt", "académico"),
    ("ocr-es.txt", "es", "OCR roto"),
];

/// Palabras que no dicen nada del contenido y no sirven para comprobar nada.
const VACIAS: &[&str] = &[
    "para", "porque", "como", "cuando", "donde", "entre", "sobre", "desde", "hasta", "cada",
    "está", "están", "sean", "puede", "pueden", "debe", "deben", "with", "that", "than", "from",
    "were", "when", "what", "they", "them", "this", "have", "había", "pelo", "pela", "como",
    "para", "mais", "muito",
];

fn referencias() -> BTreeMap<String, Vec<String>> {
    serde_json::from_str(REFERENCIAS).expect("referencias.json es un objeto de listas")
}

fn texto_de(nombre: &str) -> String {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/evaluacion")
        .join(nombre);
    std::fs::read_to_string(ruta).unwrap_or_else(|_| panic!("falta el texto {nombre}"))
}

/// Minúsculas y sin tildes, para poder comparar palabras de tres idiomas.
fn normalizar(texto: &str) -> String {
    texto
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            otra if otra.is_alphanumeric() => otra,
            _ => ' ',
        })
        .collect()
}

/// Cada clase que la evaluación exige está cubierta, y con su idioma.
#[test]
fn el_corpus_cubre_las_clases_que_la_evaluacion_exige() {
    let referencias = referencias();
    for (nombre, idioma, clase) in EXIGIDOS {
        let texto = texto_de(nombre);
        assert!(
            texto.len() > 800,
            "{nombre} ({clase}, {idioma}) se quedó en {} caracteres",
            texto.len()
        );
        assert!(
            referencias.contains_key(*nombre),
            "{nombre} no tiene hechos de referencia"
        );
    }
    // Y al revés: nada con hechos de referencia que no esté declarado, porque un
    // texto que nadie declaró no cubre ninguna clase y engorda la medida.
    for nombre in referencias.keys() {
        assert!(
            EXIGIDOS.iter().any(|(declarado, _, _)| declarado == nombre),
            "{nombre} tiene hechos de referencia y no está declarado en la tabla"
        );
    }

    let largo = texto_de("narrativo-largo-en.txt");
    assert!(
        largo.len() > 3_500,
        "el narrativo largo no es largo: {} caracteres",
        largo.len()
    );
    let formulas = texto_de("formulas-es.txt");
    assert!(
        formulas.contains('|') && formulas.contains('×'),
        "el texto de fórmulas no trae ni tabla ni fórmula"
    );
}

/// **Cada hecho de referencia está sostenido por su texto.**
///
/// Se exige que **tres de cada cinco** palabras con contenido del hecho aparezcan en
/// el texto, y no todas. El motivo es uno de los textos: el de OCR tiene las
/// palabras partidas por guiones y las tildes perdidas **a propósito**, así que un
/// hecho sobre él no puede repetirlas. Con el listón en el cien por cien la
/// comprobación obligaría a escribir los hechos con las erratas dentro, que es lo
/// contrario de lo que un hecho de referencia tiene que decir.
///
/// Lo que detecta, que es para lo que está: un hecho que **se despegó de su texto**
/// porque alguien corrigió el texto y no el hecho. Eso baja de golpe, no un poco.
#[test]
fn cada_hecho_de_referencia_esta_en_su_texto() {
    let mut sueltos = Vec::new();
    for (nombre, hechos) in referencias() {
        let texto = normalizar(&texto_de(&nombre));
        let palabras_del_texto: std::collections::HashSet<&str> =
            texto.split_whitespace().collect();
        for hecho in hechos {
            let normalizado = normalizar(&hecho);
            let contenido: Vec<&str> = normalizado
                .split_whitespace()
                .filter(|p| p.chars().count() >= 4 && !VACIAS.contains(p))
                .collect();
            let faltan: Vec<&&str> = contenido
                .iter()
                .filter(|p| !palabras_del_texto.contains(**p))
                .collect();
            #[allow(clippy::cast_precision_loss)]
            let sostenido = if contenido.is_empty() {
                0.0
            } else {
                (contenido.len() - faltan.len()) as f64 / contenido.len() as f64
            };
            if sostenido < 0.6 {
                sueltos.push(format!(
                    "{nombre}: «{hecho}» solo se sostiene en un {:.0} % — faltan {}",
                    sostenido * 100.0,
                    faltan
                        .iter()
                        .map(|p| (**p).to_owned())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }
    assert!(
        sueltos.is_empty(),
        "hechos de referencia que su texto no sostiene:\n  {}",
        sueltos.join("\n  ")
    );
}
