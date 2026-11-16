//! El portugués de Brasil, de letras a fonemas.
//!
//! Ver el encabezado de [`super`] para por qué la conversión
//! se escribe aquí en lugar de enlazar `espeak-ng`.
//!
//! # En qué se parece al español y en qué no
//!
//! Se parece en la forma: acento por reglas, letras que suenan distinto según la
//! de al lado, y un paso final que ablanda lo que quedó duro. No se parece en tres
//! cosas, y las tres son las que hacen falta para que suene a portugués y no a
//! español mal leído:
//!
//! 1. **Las vocales átonas se cierran.** La `a` final es /æ/, la `o` final es /ʊ/
//!    y la `e` final es /y/: "casa" es /kazæ/ y "chave", /ʃavy/. Sin esto, cada
//!    palabra acaba en la vocal abierta del español y el resultado suena a otro
//!    idioma.
//! 2. **Las nasales son vocales, no consonantes.** "canto" no lleva una /n/: lleva
//!    una /ɐ̃/ seguida de la marca nasal. "não" es un diptongo nasal entero.
//! 3. **La `t` y la `d` se palatalizan ante /i/.** "dia" es /dʒiæ/ y "noite",
//!    /noɪtʃy/. Es lo que más distingue al brasileño del portugués europeo.
//!
//! # Lo que estas reglas no pueden saber, y por eso se declara
//!
//! **Si una `e` o una `o` tónicas son abiertas o cerradas.** "mesa" lleva /e/ y
//! "terra", /ɛ/; se escriben igual. En portugués eso no es una regla: es léxico, y
//! la referencia lo resuelve con su diccionario. Aquí se elige siempre la cerrada
//! y **lo que no coincide se cuenta y se clasifica** en `fonemas_portugues.rs`, en
//! lugar de esconderlo tras un porcentaje. Es la misma decisión que con "México"
//! en español, con la diferencia de que en portugués la clase es grande.
//!
//! Medido el 2026-09-19 sobre 162 palabras: **89 % idénticas a la referencia, y
//! 97 % si no se cuenta el timbre**. Las reglas, por tanto, están bien; lo que
//! falta es léxico. **La voz portuguesa no entra todavía en el catálogo de
//! modelos**, que además no tiene ninguna entrada portuguesa con su huella
//! verificada: lo que hay aquí es la conversión, medida, para que lo que falte sea
//! un diccionario y no un idioma entero.

/// Marca de pausa que el modelo entiende. La misma lista que en español.
const PAUSAS: &str = ",.;:!?…—";

/// Palabras sin acento propio: se apoyan en la siguiente.
///
/// Artículos, preposiciones y pronombres átonos. Sin esta lista, "o livro do meu
/// pai" sale con cinco acentos y suena a dictado.
const ATONAS: &[&str] = &[
    "o", "a", "os", "as", "de", "do", "da", "dos", "das", "em", "no", "na", "nos", "nas", "ao",
    "aos", "à", "às", "por", "pelo", "pela", "com", "sem", "e", "ou", "que", "se", "me", "te",
    "lhe", "nos", "vos",
];

enum Pieza {
    Palabra(String),
    Puntuacion(char),
}

/// Convierte un texto en fonemas, con sus marcas de acento y su puntuación.
#[must_use]
pub fn de_texto(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len() * 2);
    for pieza in trocear(texto) {
        match pieza {
            Pieza::Palabra(p) => {
                if !salida.is_empty() && !salida.ends_with(' ') {
                    salida.push(' ');
                }
                salida.push_str(&de_palabra(&p));
            }
            Pieza::Puntuacion(c) => salida.push(c),
        }
    }
    salida.trim().to_owned()
}

fn trocear(texto: &str) -> Vec<Pieza> {
    let mut piezas = Vec::new();
    let mut actual = String::new();
    let mut cifras = String::new();

    let cerrar_palabra = |actual: &mut String, piezas: &mut Vec<Pieza>| {
        if !actual.is_empty() {
            piezas.push(Pieza::Palabra(std::mem::take(actual)));
        }
    };
    let cerrar_cifras = |cifras: &mut String, piezas: &mut Vec<Pieza>| {
        if !cifras.is_empty() {
            for palabra in numero_en_palabras(cifras).split_whitespace() {
                piezas.push(Pieza::Palabra(palabra.to_owned()));
            }
            cifras.clear();
        }
    };

    for c in texto.chars() {
        if c.is_ascii_digit() {
            cerrar_palabra(&mut actual, &mut piezas);
            cifras.push(c);
        } else if c.is_alphabetic() || c == '\'' {
            cerrar_cifras(&mut cifras, &mut piezas);
            actual.push(c.to_lowercase().next().unwrap_or(c));
        } else {
            cerrar_palabra(&mut actual, &mut piezas);
            cerrar_cifras(&mut cifras, &mut piezas);
            if PAUSAS.contains(c) {
                piezas.push(Pieza::Puntuacion(c));
            }
        }
    }
    cerrar_palabra(&mut actual, &mut piezas);
    cerrar_cifras(&mut cifras, &mut piezas);
    piezas
}

const VOCALES: &str = "aeiouáéíóúâêôàãõ";

fn es_vocal(c: char) -> bool {
    VOCALES.contains(c)
}

/// Si la vocal lleva tilde que **manda sobre el acento**.
///
/// La tilde de `ã` y `õ` no cuenta: marca nasalidad, no acento. "órgão" lleva el
/// acento en la `ó` aunque tenga una `ã` detrás.
fn acentuada(c: char) -> bool {
    matches!(c, 'á' | 'é' | 'í' | 'ó' | 'ú' | 'â' | 'ê' | 'ô')
}

fn nasal_escrita(c: char) -> bool {
    matches!(c, 'ã' | 'õ')
}

/// Una sílaba: dónde empieza su núcleo vocálico y dónde acaba.
struct Nucleo {
    inicio: usize,
    fin: usize,
}

/// Los núcleos vocálicos, que son las sílabas.
///
/// Un grupo de vocales es un solo núcleo salvo que dos fuertes se junten: "saudade"
/// tiene tres sílabas y "ideia", también. La `u` muda de "que", "qui", "gue" y "gui"
/// no cuenta, igual que en español.
fn nucleos(letras: &[char]) -> Vec<Nucleo> {
    let mut mudas = mudas(letras);
    for (i, muda) in mudas.iter_mut().enumerate() {
        if semivocal(letras, i) {
            *muda = true;
        }
    }
    let mut grupos: Vec<Nucleo> = Vec::new();
    let mut i = 0;
    while i < letras.len() {
        if !es_vocal(letras[i]) || mudas[i] {
            i += 1;
            continue;
        }
        let inicio = i;
        let mut fin = i + 1;
        let fuerte = |c: char| {
            matches!(
                c,
                'a' | 'e' | 'o' | 'á' | 'é' | 'ó' | 'â' | 'ê' | 'ô' | 'ã' | 'õ' | 'à'
            )
        };
        let mut ya_hay_fuerte = fuerte(letras[i]);
        while fin < letras.len() && es_vocal(letras[fin]) && !mudas[fin] {
            let actual = letras[fin];
            // **Una sola vocal fuerte por sílaba.** Es lo que separa "areia" en
            // a-rei-a y no en a-reia: lo que va detrás de la fuerte solo puede ser
            // una semivocal. Escrito como "dos fuertes seguidas se separan", que es
            // la regla del español, "eia" salía de una pieza y el acento caía una
            // sílaba antes.
            // "ão", "ãe" y "õe" son un diptongo nasal entero, aunque las dos
            // vocales sean fuertes: "canção" tiene dos sílabas, no tres, y contarle
            // tres le ponía un acento secundario que no tiene.
            let nasal_antes = nasal_escrita(letras[fin - 1]);
            if fuerte(actual) && ya_hay_fuerte && !nasal_antes {
                break;
            }
            // Una débil con tilde rompe el diptongo: "saída" es sa-í-da.
            if matches!(actual, 'í' | 'ú') {
                break;
            }
            ya_hay_fuerte = ya_hay_fuerte || fuerte(actual);
            fin += 1;
        }
        grupos.push(Nucleo { inicio, fin });
        i = fin;
    }
    grupos
}

/// Las `u` que no suenan: las de "que", "qui", "gue" y "gui".
fn mudas(letras: &[char]) -> Vec<bool> {
    let mut mudas = vec![false; letras.len()];
    for i in 0..letras.len() {
        if letras[i] != 'u' {
            continue;
        }
        let tras = i > 0 && matches!(letras[i - 1], 'q' | 'g');
        let ante_palatal = matches!(letras.get(i + 1), Some('e' | 'é' | 'i' | 'í' | 'ê'));
        if tras && ante_palatal {
            mudas[i] = true;
        }
    }
    mudas
}

/// Qué sílaba lleva el acento.
///
/// La tilde manda. Sin tilde: la última si la palabra acaba en `i`, `u`, `l`, `r`,
/// `z`, `im`, `um`, `ns` o en diptongo nasal; la penúltima en el resto. Es la regla
/// al revés que en español, y por eso "falar" no se acentúa como "hablar".
fn tonica(letras: &[char], nucleos: &[Nucleo]) -> Option<usize> {
    if nucleos.is_empty() {
        return None;
    }
    for (indice, nucleo) in nucleos.iter().enumerate() {
        if letras[nucleo.inicio..nucleo.fin]
            .iter()
            .any(|c| acentuada(*c))
        {
            return Some(indice);
        }
    }
    // Una `ã` o `õ` escritas llevan el acento si no hay tilde en otro sitio, salvo
    // cuando van seguidas de otra sílaba: "órfão" lo lleva antes, pero "irmã" en
    // ella.
    if let Some((indice, _)) = nucleos
        .iter()
        .enumerate()
        .find(|(_, n)| letras[n.inicio..n.fin].iter().any(|c| nasal_escrita(*c)))
    {
        if indice + 1 == nucleos.len() {
            return Some(indice);
        }
    }

    let ultima = *letras.last()?;
    let penultima = letras.len().checked_sub(2).map(|i| letras[i]);
    let aguda = match ultima {
        'i' | 'u' | 'l' | 'r' | 'z' | 'í' | 'ú' => true,
        // "assim", "algum": aguda. "homem", "jovem": grave, que es lo que hace que
        // no se pueda decidir solo con la última letra.
        'm' => matches!(penultima, Some('i' | 'u')),
        's' => matches!(penultima, Some('i' | 'u' | 'ã' | 'õ')),
        _ => false,
    };
    Some(if aguda {
        nucleos.len() - 1
    } else {
        nucleos.len().saturating_sub(2)
    })
}

/// Una palabra entera, ya con sus marcas.
fn de_palabra(palabra: &str) -> String {
    let letras: Vec<char> = palabra.chars().collect();
    if letras.is_empty() {
        return String::new();
    }
    let nucleos = nucleos(&letras);
    let atona = ATONAS.contains(&palabra) || nucleos.len() <= 1 && ATONAS.contains(&palabra);
    let tonica = if atona {
        None
    } else {
        tonica(&letras, &nucleos)
    };
    // La secundaria cae en la primera sílaba de las palabras de tres o más, y en
    // la tercera cuando pasan de cinco. Es lo que hace la referencia, y sin ella
    // las palabras largas salen planas.
    let secundarias: Vec<usize> = if nucleos.len() >= 3 {
        let mut marcas = vec![0];
        if nucleos.len() >= 5 {
            marcas.push(2);
        }
        marcas.retain(|i| Some(*i) != tonica);
        marcas
    } else {
        Vec::new()
    };

    let mudas = mudas(&letras);
    let mut salida = String::with_capacity(palabra.len() * 2);
    let mut i = 0;
    while i < letras.len() {
        if mudas[i] {
            i += 1;
            continue;
        }
        if es_vocal(letras[i]) {
            // La `u` de "água" y "quatro" no es núcleo: es la semivocal /w/, y
            // contarla como vocal movía el acento una sílaba.
            if semivocal(&letras, i) {
                salida.push('w');
                i += 1;
                continue;
            }
            let nucleo = nucleos
                .iter()
                .position(|n| i >= n.inicio && i < n.fin)
                .unwrap_or(0);
            let empieza = i == nucleos.get(nucleo).map_or(usize::MAX, |n| n.inicio);
            if empieza {
                if Some(nucleo) == tonica {
                    salida.push('ˈ');
                } else if secundarias.contains(&nucleo) {
                    salida.push('ˌ');
                }
            }
            // **El acento es de la primera letra del núcleo, no de todas.** La `o`
            // final de "cheio" está dentro de la sílaba tónica y aun así se cierra
            // en /ʊ/: lo que decide el cierre es ser la última, no la sílaba.
            let (fonema, avance) = vocal(&letras, i, Some(nucleo) == tonica && empieza);
            salida.push_str(&fonema);
            i += avance;
        } else {
            let (fonema, avance) = consonante(&letras, i);
            salida.push_str(&fonema);
            i += avance;
        }
    }
    salida
}

/// Qué suena una vocal, con lo que lleve detrás.
///
/// Devuelve el fonema y cuántas letras consume: un diptongo y una nasal se comen
/// más de una.
fn vocal(letras: &[char], i: usize, tonica: bool) -> (String, usize) {
    let c = letras[i];
    let siguiente = letras.get(i + 1).copied();
    let final_de_palabra = |j: usize| j + 1 >= letras.len();

    // Diptongos nasales escritos: "não", "mãe", "põe".
    if c == 'ã' {
        return match siguiente {
            Some('o') => ("ɐ̃ʊ̃".to_owned(), 2),
            Some('e') => ("ɐ̃y".to_owned(), 2),
            _ => ("ɐ̃".to_owned(), 1),
        };
    }
    if c == 'õ' {
        return match siguiente {
            Some('e') => ("o\u{303}j".to_owned(), 2),
            _ => ("o\u{303}".to_owned(), 1),
        };
    }

    // Nasal por consonante: la `m` o la `n` que cierra la sílaba no suena, nasaliza.
    if let Some(nasal @ ('m' | 'n')) = siguiente {
        let tras_nasal = letras.get(i + 2).copied();
        // La `n` de "vinho" no cierra la sílaba: es la primera letra del dígrafo
        // `nh`, que suena /ɲ/. Sin esta excepción "vinho" salía nasalizado y sin
        // la /ɲ/, que es justo la consonante que lo distingue de "vino".
        let digrafo = nasal == 'n' && tras_nasal == Some('h');
        let cierra = !digrafo && tras_nasal.is_none_or(|t| !es_vocal(t));
        if cierra {
            // Ante `p` o `b` la nasal se queda: "tempo" y "também" la conservan.
            let labial = matches!(tras_nasal, Some('p' | 'b')) && nasal == 'm';
            let cola = if labial { "m" } else { "ŋ" };
            let nucleo = match sin_tilde(c) {
                'a' => "ɐ̃",
                'e' => "eɪ",
                'i' => "i",
                'o' => "o",
                'u' => "u\u{303}",
                _ => "ɐ̃",
            };
            return (format!("{nucleo}{cola}"), 2);
        }
    }

    // Ante nasal que **no** cierra sílaba, la vocal se colorea igualmente: la
    // referencia nasaliza la tónica -"exame" es /ezɐ̃my/ y "número", /nũmeɾʊ/- y
    // cierra la `a` átona en /æ/ -"amor", "família"-. Ante el dígrafo `nh` nasaliza
    // siempre: "manhã".
    if let Some('m' | 'n') = siguiente {
        let ante_h = letras.get(i + 2) == Some(&'h');
        let intervocalica = letras.get(i + 2).copied().is_some_and(es_vocal) || ante_h;
        if intervocalica {
            let base = sin_tilde(c);
            if ante_h && base == 'a' {
                return ("ɐ\u{303}".to_owned(), 1);
            }
            if tonica {
                match base {
                    'a' => return ("ɐ\u{303}".to_owned(), 1),
                    'u' => return ("u\u{303}".to_owned(), 1),
                    _ => {}
                }
            } else if base == 'a' && !acentuada(c) {
                return ("æ".to_owned(), 1);
            }
        }
    }

    // Diptongos orales.
    if let Some(segunda) = siguiente {
        let glide = match (sin_tilde(c), segunda) {
            ('a', 'i') => Some("aɪ"),
            ('a', 'u') => Some("aʊ"),
            ('e', 'i') => Some("eɪ"),
            ('e', 'u') => Some("eʊ"),
            ('o', 'i') => Some("oɪ"),
            ('o', 'u') => Some("ow"),
            ('i', 'u') => Some("iʊ"),
            _ => None,
        };
        if let Some(glide) = glide {
            let abierta = matches!(c, 'é' | 'ó');
            let fonema = match (abierta, glide) {
                (true, "eʊ") => "ɛʊ",
                (true, "eɪ") => "ɛɪ",
                (true, "oɪ") => "ɔɪ",
                _ => glide,
            };
            return (fonema.to_owned(), 2);
        }
    }

    // La `i` átona delante de otra vocal es una semivocal, no una sílaba:
    // "história" acaba en /ɾjæ/ y no en /ɾiæ/.
    if sin_tilde(c) == 'i' && !tonica && siguiente.is_some_and(es_vocal) && i > 0 {
        return ("j".to_owned(), 1);
    }

    // Vocal suelta. Las átonas finales se cierran, que es lo que hace que suene a
    // portugués: "casa" acaba en /æ/ y no en /a/.
    let ultima = final_de_palabra(i) || (siguiente == Some('s') && letras.len() == i + 2);
    let fonema = match c {
        'á' | 'â' => "a",
        'é' => "ɛ",
        'ê' => "e",
        'í' => "i",
        'ó' => "ɔ",
        'ô' => "o",
        'ú' => "u",
        'a' if ultima && !tonica => "æ",
        'a' => "a",
        'e' if ultima && !tonica => "y",
        'e' => "e",
        'o' if ultima && !tonica => "ʊ",
        'o' => "o",
        'i' => "i",
        'u' => "u",
        _ => "",
    };
    (fonema.to_owned(), 1)
}

fn sin_tilde(c: char) -> char {
    match c {
        'á' | 'â' | 'à' | 'ã' => 'a',
        'é' | 'ê' => 'e',
        'í' => 'i',
        'ó' | 'ô' | 'õ' => 'o',
        'ú' => 'u',
        otra => otra,
    }
}

/// Qué suena una consonante, con lo que lleve al lado.
fn consonante(letras: &[char], i: usize) -> (String, usize) {
    let c = letras[i];
    let siguiente = letras.get(i + 1).copied();
    let anterior = i.checked_sub(1).map(|j| letras[j]);
    let palatal = |c: Option<char>| matches!(c, Some('e' | 'é' | 'ê' | 'i' | 'í'));
    let entre_vocales = anterior.is_some_and(es_vocal) && siguiente.is_some_and(es_vocal);
    let ante_i = || {
        // La `i` que palataliza incluye la `e` átona final, que suena /i/: "noite"
        // acaba en /tʃy/ y "cidade", en /dʒy/.
        matches!(siguiente, Some('i' | 'í'))
            || (siguiente == Some('e') && letras.len() == i + 2)
            || (siguiente == Some('e') && letras.get(i + 2) == Some(&'s') && letras.len() == i + 3)
    };

    match c {
        'b' => ("b".to_owned(), 1),
        'c' => match siguiente {
            Some('h') => ("ʃ".to_owned(), 2),
            s if palatal(s) => ("s".to_owned(), 1),
            _ => ("k".to_owned(), 1),
        },
        'ç' => ("s".to_owned(), 1),
        'd' if ante_i() => ("dʒ".to_owned(), 1),
        'd' => ("d".to_owned(), 1),
        'f' => ("f".to_owned(), 1),
        'g' => match siguiente {
            s if palatal(s) => ("ʒ".to_owned(), 1),
            _ => ("ɡ".to_owned(), 1),
        },
        'h' => (String::new(), 1),
        'j' => ("ʒ".to_owned(), 1),
        'k' => ("k".to_owned(), 1),
        'l' => match siguiente {
            Some('h') => ("lj".to_owned(), 2),
            // La `l` que cierra sílaba se vocaliza: "Brasil" acaba en /iʊ/. Tras
            // `o` la referencia la conserva -"volta"-, así que aquí también.
            s if s.is_none() || !es_vocal(s.unwrap_or(' ')) => {
                // Al final de palabra siempre: "Brasil" acaba en /iʊ/. Dentro,
                // solo tras `a` -"alto", "falso"-, porque la referencia conserva la
                // /l/ en "volta" y en "último".
                if siguiente.is_none() || anterior == Some('a') || anterior == Some('á') {
                    ("ʊ".to_owned(), 1)
                } else {
                    ("l".to_owned(), 1)
                }
            }
            _ => ("l".to_owned(), 1),
        },
        'm' => ("m".to_owned(), 1),
        'n' => match siguiente {
            Some('h') => ("ɲ".to_owned(), 2),
            _ => ("n".to_owned(), 1),
        },
        'p' => ("p".to_owned(), 1),
        'q' => ("k".to_owned(), 1),
        'r' => {
            if siguiente == Some('r') {
                ("x".to_owned(), 2)
            } else if anterior.is_none() {
                ("x".to_owned(), 1)
            } else if !anterior.is_some_and(es_vocal) && anterior != Some('r') {
                // Tras consonante sigue siendo suave: "prato", "livro".
                ("r".to_owned(), 1)
            } else if siguiente.is_none() {
                ("r".to_owned(), 1)
            } else if siguiente.is_some_and(es_vocal) {
                ("ɾ".to_owned(), 1)
            } else {
                // La `r` que cierra sílaba arrastra una vocal de apoyo, como en la
                // referencia: "partir" es /paɾətʃir/.
                ("ɾə".to_owned(), 1)
            }
        }
        's' => match siguiente {
            Some('s') => ("s".to_owned(), 2),
            _ if entre_vocales => ("z".to_owned(), 1),
            _ => ("s".to_owned(), 1),
        },
        't' if ante_i() => ("tʃ".to_owned(), 1),
        't' => ("t".to_owned(), 1),
        'v' => ("v".to_owned(), 1),
        'w' => ("v".to_owned(), 1),
        'x' => {
            // "exame" suena /z/; el resto, /ʃ/. Cuál de las cuatro cosas que puede
            // sonar una `x` es léxico, no regla, y está contado en la prueba.
            if anterior == Some('e') && i == 1 && siguiente.is_some_and(es_vocal) {
                ("z".to_owned(), 1)
            } else {
                ("ʃ".to_owned(), 1)
            }
        }
        'y' => ("j".to_owned(), 1),
        'z' if siguiente.is_none() => ("s".to_owned(), 1),
        'z' => ("z".to_owned(), 1),
        _ => (String::new(), 1),
    }
}

/// La `u` de "qua" y "gua", que no es núcleo: suena /w/.
fn semivocal(letras: &[char], i: usize) -> bool {
    letras[i] == 'u'
        && i > 0
        && matches!(letras[i - 1], 'q' | 'g')
        && letras.get(i + 1).copied().is_some_and(es_vocal)
}

const UNIDADES: &[&str] = &[
    "zero",
    "um",
    "dois",
    "três",
    "quatro",
    "cinco",
    "seis",
    "sete",
    "oito",
    "nove",
    "dez",
    "onze",
    "doze",
    "treze",
    "catorze",
    "quinze",
    "dezesseis",
    "dezessete",
    "dezoito",
    "dezenove",
];

const DECENAS: &[&str] = &[
    "",
    "",
    "vinte",
    "trinta",
    "quarenta",
    "cinquenta",
    "sessenta",
    "setenta",
    "oitenta",
    "noventa",
];

const CENTENAS: &[&str] = &[
    "",
    "cento",
    "duzentos",
    "trezentos",
    "quatrocentos",
    "quinhentos",
    "seiscentos",
    "setecentos",
    "oitocentos",
    "novecentos",
];

/// Un número dicho con letras. Un libro está lleno de fechas, y el modelo no sabe
/// leer cifras: se las come.
#[must_use]
pub fn numero_en_palabras(cifras: &str) -> String {
    let Ok(n) = cifras.parse::<u64>() else {
        return cifras.chars().map(|c| format!("{c} ")).collect();
    };
    decir(n)
}

fn decir(n: u64) -> String {
    match n {
        0..=19 => UNIDADES[n as usize].to_owned(),
        20..=99 => {
            let decena = DECENAS[(n / 10) as usize];
            if n.is_multiple_of(10) {
                decena.to_owned()
            } else {
                format!("{decena} e {}", UNIDADES[(n % 10) as usize])
            }
        }
        100 => "cem".to_owned(),
        101..=999 => {
            let centena = CENTENAS[(n / 100) as usize];
            if n.is_multiple_of(100) {
                centena.to_owned()
            } else {
                format!("{centena} e {}", decir(n % 100))
            }
        }
        1_000 => "mil".to_owned(),
        1_001..=999_999 => {
            let miles = n / 1_000;
            let cabeza = if miles == 1 {
                "mil".to_owned()
            } else {
                format!("{} mil", decir(miles))
            };
            if n.is_multiple_of(1_000) {
                cabeza
            } else {
                format!("{cabeza} e {}", decir(n % 1_000))
            }
        }
        _ => n.to_string().chars().map(|c| format!("{c} ")).collect(),
    }
}
