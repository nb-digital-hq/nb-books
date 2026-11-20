//! El español, de letras a fonemas.
//!
//! Ver el encabezado de [`super`] para por qué se escribe
//! aquí en lugar de usar `espeak-ng`.
//!
//! # Cómo funciona
//!
//! El español se lee casi como se escribe, y lo que no es evidente cabe en tres
//! reglas que aquí son tres pasos:
//!
//! 1. **Dónde cae el acento.** La tilde manda; sin tilde, la penúltima sílaba si la
//!    palabra acaba en vocal, `n` o `s`, y la última si acaba en otra cosa. Para eso
//!    hay que contar sílabas, y contar sílabas en español es contar **núcleos
//!    vocálicos**: un grupo de vocales es una sola sílaba salvo que haya hiato.
//! 2. **Qué suena cada letra**, que depende de la de al lado: `c` ante `e` es una
//!    cosa y ante `a`, otra; `g` igual; `r` suena fuerte al principio y suave entre
//!    vocales.
//! 3. **Las oclusivas se ablandan entre vocales.** `b`, `d` y `g` son /b/, /d/, /ɡ/
//!    al empezar y tras nasal, y /β/, /ð/, /ɣ/ en el resto. Sin esto, "abogado"
//!    suena a máquina deletreando.
//!
//! Los números se dicen, no se deletrean: un libro está lleno de fechas y de
//! cifras, y el modelo no sabe leerlas.

use super::Variante;

/// Una palabra ya convertida, con dónde empieza cada núcleo vocálico.
struct Palabra {
    fonemas: String,
    /// Posición en `fonemas`, en bytes, donde empieza cada núcleo.
    nucleos: Vec<usize>,
    /// Qué núcleo lleva el acento, si la palabra lo tiene.
    tonica: Option<usize>,
}

/// Convierte un texto en fonemas, con sus marcas de acento y su puntuación.
///
/// Las palabras **no se convierten por separado**: el español encadena. "de abril"
/// no suena como "de" y luego "abril" —la `d` se ablanda porque la palabra anterior
/// acabó en vocal—, y así lo hace también la referencia. Por eso el contexto
/// fonético cruza el espacio entre palabras y solo se reinicia en una pausa.
#[must_use]
pub fn de_texto(texto: &str, variante: Variante) -> String {
    let mut salida = String::with_capacity(texto.len() * 2);
    let mut contexto = Contexto::inicial();
    for pieza in trocear(texto) {
        match pieza {
            Pieza::Palabra(p) => {
                if !salida.is_empty() && !salida.ends_with(' ') {
                    salida.push(' ');
                }
                salida.push_str(&de_palabra(&p, variante, &mut contexto));
            }
            Pieza::Puntuacion(c) => {
                salida.push(c);
                // Tras una pausa se vuelve a empezar: lo que sigue a un punto suena
                // como el principio de una frase, no como la continuación.
                contexto = Contexto::inicial();
            }
        }
    }
    salida.trim().to_owned()
}

/// Lo que la letra anterior deja dicho sobre la siguiente, **también entre
/// palabras**.
#[derive(Clone, Copy)]
struct Contexto {
    tras_vocal: bool,
    tras_nasal: bool,
    tras_pausa: bool,
}

impl Contexto {
    const fn inicial() -> Self {
        Self {
            tras_vocal: false,
            tras_nasal: false,
            tras_pausa: true,
        }
    }
}

/// Palabras que no llevan acento propio.
///
/// Artículos, preposiciones, conjunciones y pronombres átonos: se apoyan en la
/// palabra siguiente y no se marcan. Sin esta lista, "se lo dijo a su madre" sale
/// con seis acentos y suena a dictado, que es exactamente lo que se describió al
/// escuchar la primera prueba. La referencia hace lo mismo.
const ATONAS: &[&str] = &[
    "el", "la", "los", "las", "lo", "le", "les", "me", "te", "se", "nos", "os", "un", "unos",
    "unas", "de", "del", "a", "al", "en", "con", "sin", "por", "y", "e", "o", "u", "que", "si",
    "mi", "mis", "tu", "tus", "su", "sus", "ni",
];

enum Pieza {
    Palabra(String),
    Puntuacion(char),
}

/// Puntuación que el modelo sabe leer como pausa. El resto se descarta.
const PAUSAS: &str = ",.;:!?…—";

/// Parte el texto en palabras, números ya dichos con letras, y pausas.
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
            for palabra in super::espanol::numero_en_palabras(cifras).split_whitespace() {
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
            actual.push(a_letra_espanola(c.to_lowercase().next().unwrap_or(c)));
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

/// Una letra latina que el español no usa, llevada a la que suena más cerca.
///
/// El modelo no tiene fonema para `è`, `ï`, `ã` ni `ª`, y lo que no tiene lo calla:
/// "naïf" sonaba "naf" y "1ª" sonaba "uno". Medido el 2026-09-23 sobre 1.500 libros
/// del corpus: el 5 % de los fragmentos traía alguna, casi todas nombres propios y
/// citas en francés, portugués o catalán. Leerlas con su vocal no es pronunciarlas
/// bien, pero es decir la palabra en lugar de comerse una sílaba. Lo que no es
/// latino -el cirílico, el griego- se sigue dejando fuera: no hay vocal "más cerca"
/// que no fuera inventarse la palabra.
fn a_letra_espanola(c: char) -> char {
    match c {
        'à' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ª' => 'a',
        'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ì' | 'î' | 'ï' | 'ī' | 'ĭ' => 'i',
        'ò' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'º' => 'o',
        'ù' | 'û' | 'ū' | 'ŭ' | 'ů' | 'ű' => 'u',
        'ç' => 'z',
        'ý' | 'ÿ' => 'y',
        otra => otra,
    }
}

/// Vocales, con y sin tilde.
const FUERTES: &str = "aeoáéó";
const DEBILES: &str = "iuü";
const ACENTUADAS: &str = "áéíóú";

fn es_vocal(c: char) -> bool {
    FUERTES.contains(c) || DEBILES.contains(c) || c == 'í' || c == 'ú'
}

fn sin_tilde(c: char) -> char {
    match c {
        'á' => 'a',
        'é' => 'e',
        'í' => 'i',
        'ó' => 'o',
        'ú' | 'ü' => 'u',
        otra => otra,
    }
}

/// Las `u` que no suenan: las de "que", "qui", "gue" y "gui".
///
/// Se marcan **antes** de contar sílabas, porque si contaran, "queso" tendría el
/// acento en la sílaba equivocada: la `u` no es un núcleo, es una regla de
/// ortografía. La `ü` de "cigüeña" sí suena, y por eso lleva diéresis.
fn mudas(letras: &[char]) -> Vec<bool> {
    let mut mudas = vec![false; letras.len()];
    for i in 0..letras.len() {
        if letras[i] != 'u' {
            continue;
        }
        let tras_q = i > 0 && letras[i - 1] == 'q';
        let tras_g = i > 0 && letras[i - 1] == 'g';
        let ante_palatal = matches!(letras.get(i + 1), Some('e' | 'é' | 'i' | 'í'));
        if tras_q || (tras_g && ante_palatal) {
            mudas[i] = true;
        }
    }
    mudas
}

/// Los núcleos vocálicos de una palabra: cada uno es una sílaba.
///
/// Un grupo de vocales seguidas es **un** núcleo —el diptongo— salvo que haya
/// hiato: dos fuertes juntas, o una débil con tilde, se separan. Es la regla que
/// decide si "país" tiene una sílaba o dos, y con ella, dónde cae el acento.
fn nucleos(letras: &[char], mudas: &[bool]) -> Vec<(usize, usize)> {
    let vocal = |i: usize| es_vocal(letras[i]) && !mudas[i];
    let mut grupos: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < letras.len() {
        if !vocal(i) {
            i += 1;
            continue;
        }
        let inicio = i;
        let mut fin = i + 1;
        while fin < letras.len() && vocal(fin) {
            let anterior = letras[fin - 1];
            let actual = letras[fin];
            let hiato = (FUERTES.contains(anterior) && FUERTES.contains(actual))
                || (actual == 'í' || actual == 'ú')
                || (anterior == 'í' || anterior == 'ú')
                // Tras líquida, la débil no se desliza: "luego" se dice lu-e-go y
                // "biblioteca", bi-blio-te-ca. Lo hace igual la referencia.
                || (inicio > 0 && matches!(letras[inicio - 1], 'l' | 'r') && DEBILES.contains(anterior));
            if hiato {
                break;
            }
            fin += 1;
        }
        grupos.push((inicio, fin));
        i = fin;
    }
    grupos
}

/// Qué núcleo lleva el acento.
///
/// La tilde manda. Sin tilde: penúltima si la palabra acaba en vocal, `n` o `s`;
/// última en cualquier otro caso. Una palabra de una sílaba no lleva marca, que es
/// lo que hace `espeak` con los monosílabos átonos.
fn tonica(letras: &[char], grupos: &[(usize, usize)]) -> Option<usize> {
    if grupos.is_empty() {
        return None;
    }
    for (n, (inicio, fin)) in grupos.iter().enumerate() {
        if letras[*inicio..*fin]
            .iter()
            .any(|c| ACENTUADAS.contains(*c))
        {
            return Some(n);
        }
    }
    let ultima = *letras.last()?;
    let llana = es_vocal(ultima) || ultima == 'n' || ultima == 's';
    if llana && grupos.len() >= 2 {
        Some(grupos.len() - 2)
    } else {
        Some(grupos.len() - 1)
    }
}

/// Convierte una palabra, con sus marcas de acento puestas.
fn de_palabra(palabra: &str, variante: Variante, contexto: &mut Contexto) -> String {
    let letras: Vec<char> = palabra.chars().collect();
    let mudas = mudas(&letras);
    let grupos = nucleos(&letras, &mudas);
    let acentuada = if ATONAS.contains(&palabra) {
        None
    } else {
        tonica(&letras, &grupos)
    };
    let convertida = letras_a_fonemas(&letras, &grupos, acentuada, variante, contexto);
    con_acentos(&convertida, grupos.len())
}

/// Pone `ˈ` en la sílaba tónica y `ˌ` en la primera cuando la tónica queda lejos.
///
/// El acento secundario no es adorno: `espeak` lo pone en la primera sílaba cuando
/// la tónica es la tercera o posterior, y el modelo se entrenó con él. Sin él, las
/// palabras largas salen planas.
fn con_acentos(p: &Palabra, silabas: usize) -> String {
    let Some(tonica) = p.tonica else {
        return p.fonemas.clone();
    };
    let mut marcas: Vec<(usize, char)> = vec![(p.nucleos[tonica], 'ˈ')];
    if tonica >= 2 && silabas >= 3 {
        marcas.push((p.nucleos[0], 'ˌ'));
    }
    marcas.sort_unstable();
    let mut salida = String::with_capacity(p.fonemas.len() + 4);
    let mut anterior = 0;
    for (posicion, marca) in marcas {
        // Una posición que no cae en el borde de un carácter es un defecto de aquí,
        // no del texto: se dice la palabra sin esa marca en lugar de entrar en pánico
        // y llevarse la preparación de un libro entero.
        if posicion < anterior || !p.fonemas.is_char_boundary(posicion) {
            continue;
        }
        salida.push_str(&p.fonemas[anterior..posicion]);
        salida.push(marca);
        anterior = posicion;
    }
    salida.push_str(&p.fonemas[anterior..]);
    salida
}

/// El paso de letras a sonidos, mirando siempre la letra de al lado.
#[allow(clippy::too_many_lines)]
fn letras_a_fonemas(
    letras: &[char],
    grupos: &[(usize, usize)],
    acentuada: Option<usize>,
    variante: Variante,
    contexto: &mut Contexto,
) -> Palabra {
    // El portugués no llega hasta aquí -`super::de_texto` lo manda a su módulo-,
    // pero el tipo no lo sabe: se trata como el español de América, que es lo que
    // hacía antes de que existiera la variante.
    let ese = match variante {
        Variante::EspanolDeEspana => "θ",
        _ => "s",
    };
    let elle = match variante {
        Variante::EspanolDeEspana => "ʎ",
        _ => "ʝ",
    };

    let mut fonemas = String::with_capacity(letras.len() * 2);
    let mut nucleos_en_salida = vec![0_usize; grupos.len()];
    // La letra en la que empieza de verdad cada núcleo. Una débil átona delante de
    // otra vocal no es núcleo: es /j/ o /w/, y el acento va detrás, en la vocal de
    // verdad.
    //
    // **Se guarda la letra y no un desplazamiento en bytes.** Antes se
    // sumaban letras a una posición en bytes, suponiendo que cada débil saltada
    // ocupa un byte en la salida; con tres débiles seguidas -"III", un número de
    // capítulo- la segunda sale `ɪ`, de dos bytes, el acento caía a mitad de un
    // carácter y la conversión entraba en pánico. Un capítulo con un número romano
    // en el título no se podía preparar.
    let empieza: Vec<usize> = grupos
        .iter()
        .map(|&(inicio, fin)| {
            let mut k = inicio;
            while k + 1 < fin && DEBILES.contains(letras[k]) && !ACENTUADAS.contains(letras[k]) {
                k += 1;
            }
            k
        })
        .collect();
    let mut i = 0;
    // Viene de la palabra anterior: en español la cadena no se corta en el espacio.
    let Contexto {
        mut tras_vocal,
        mut tras_nasal,
        mut tras_pausa,
    } = *contexto;

    while i < letras.len() {
        let c = letras[i];
        let siguiente = letras.get(i + 1).copied();
        let anterior = if i > 0 {
            letras.get(i - 1).copied()
        } else {
            None
        };

        // Dónde empieza cada núcleo en la cadena de fonemas: es lo que permite
        // poner el acento delante de la vocal y no delante de la sílaba. Se mira
        // antes de emitir la letra, así que es la posición en bytes de lo que ella
        // emita, sea del largo que sea.
        if let Some(n) = empieza.iter().position(|&k| k == i) {
            nucleos_en_salida[n] = fonemas.len();
        }

        let emitido: String = match c {
            'a' | 'á' => "a".to_owned(),
            'e' | 'é' => {
                // Abierta ante nasal trabada: "gente" no suena como "pena".
                // Ante `n` trabada: "gente" y "lengua" suenan abiertas; "siempre",
                // con `m`, no; y "bien", con la `n` al final, tampoco.
                let cierra = siguiente == Some('n')
                    && letras
                        .get(i + 2)
                        .is_some_and(|s| !es_vocal(*s) && *s != 'h');
                if cierra {
                    "ɛ".to_owned()
                } else {
                    "e".to_owned()
                }
            }
            'o' | 'ó' => "o".to_owned(),
            'ü' => "u".to_owned(),
            'i' | 'í' | 'u' | 'ú' => {
                let letra = sin_tilde(c);
                let acentuada_aqui = ACENTUADAS.contains(c);
                let en_grupo = grupos
                    .iter()
                    .find(|(inicio, fin)| i >= *inicio && i < *fin)
                    .copied();
                // En un diptongo de dos débiles —"ciudad", "cuidar"— manda la
                // segunda: la primera es la que se convierte en /j/ o /w/.
                let ambas_debiles = en_grupo.is_some_and(|(inicio, fin)| {
                    fin - inicio == 2 && letras[inicio..fin].iter().all(|v| DEBILES.contains(*v))
                });
                let semiconsonante = !acentuada_aqui
                    && en_grupo.is_some_and(|(inicio, fin)| fin - inicio > 1 && i == inicio);
                let semivocal = !acentuada_aqui
                    && !ambas_debiles
                    && en_grupo.is_some_and(|(inicio, fin)| fin - inicio > 1 && i > inicio);
                match (letra, semiconsonante, semivocal) {
                    ('i', true, _) => "j".to_owned(),
                    ('u', true, _) => "w".to_owned(),
                    ('i', _, true) => "ɪ".to_owned(),
                    ('u', _, true) => "j".to_owned(),
                    ('i', _, _) => "i".to_owned(),
                    _ => "u".to_owned(),
                }
            }
            'b' | 'v' => {
                if tras_pausa || tras_nasal || siguiente.is_none() {
                    "b".to_owned()
                } else {
                    "β".to_owned()
                }
            }
            'c' => {
                if matches!(siguiente, Some('e') | Some('é') | Some('i') | Some('í')) {
                    ese.to_owned()
                } else if siguiente == Some('h') {
                    i += 1;
                    "tʃ".to_owned()
                } else {
                    "k".to_owned()
                }
            }
            'd' => {
                // Al final de palabra vuelve a ser oclusiva: "ciudad" acaba en /d/,
                // no en /ð/, y así la dice también la referencia.
                if tras_pausa || tras_nasal || anterior == Some('l') || siguiente.is_none() {
                    "d".to_owned()
                } else {
                    "ð".to_owned()
                }
            }
            'f' => "f".to_owned(),
            'g' => {
                if matches!(siguiente, Some('e') | Some('é') | Some('i') | Some('í')) {
                    "x".to_owned()
                } else if siguiente == Some('u')
                    && matches!(
                        letras.get(i + 2),
                        Some('e') | Some('é') | Some('i') | Some('í')
                    )
                {
                    // "guerra", "guitarra": la `u` no suena.
                    i += 1;
                    if tras_pausa || tras_nasal { "ɡ" } else { "ɣ" }.to_owned()
                } else if tras_pausa || tras_nasal {
                    "ɡ".to_owned()
                } else {
                    "ɣ".to_owned()
                }
            }
            'h' => String::new(),
            'j' => "x".to_owned(),
            'k' => "k".to_owned(),
            'l' => {
                if siguiente == Some('l') {
                    i += 1;
                    // Entre vocales **de la misma palabra**, la referencia escribe
                    // `jj` y no `ʝ`: "calle" es kˈajje pero "llegó" empieza con ʝ.
                    // Es una rareza suya, y el modelo se entrenó con ella.
                    if anterior.is_some_and(es_vocal) && variante == Variante::EspanolDeAmerica {
                        "jj".to_owned()
                    } else {
                        elle.to_owned()
                    }
                } else {
                    "l".to_owned()
                }
            }
            'm' => "m".to_owned(),
            'n' => {
                // Ante /k/ y /ɡ/ la nasal se va atrás: "lengua" no suena "len-gua".
                // Solo ante `g`: "lengua" suena lˈɛŋɡwa y "cinco", sˈinko. Así lo
                // escribe la referencia, y ella es la que entrenó al modelo.
                if siguiente == Some('g')
                    && !matches!(letras.get(i + 2), Some('e' | 'é' | 'i' | 'í'))
                {
                    "ŋ".to_owned()
                } else {
                    "n".to_owned()
                }
            }
            'ñ' => "ɲ".to_owned(),
            'p' => {
                // "psicología": la `p` inicial no suena.
                if tras_pausa && siguiente == Some('s') {
                    String::new()
                } else {
                    "p".to_owned()
                }
            }
            'q' => {
                if siguiente == Some('u') {
                    i += 1;
                }
                "k".to_owned()
            }
            'r' => {
                if siguiente == Some('r') {
                    i += 1;
                    "r".to_owned()
                } else if tras_pausa || matches!(anterior, Some('n') | Some('l') | Some('s')) {
                    "r".to_owned()
                } else {
                    "ɾ".to_owned()
                }
            }
            's' => "s".to_owned(),
            't' => "t".to_owned(),
            'w' => "w".to_owned(),
            'x' => {
                if tras_pausa {
                    "s".to_owned()
                } else {
                    "ks".to_owned()
                }
            }
            'y' => {
                if siguiente.is_some_and(es_vocal) {
                    "ʝ".to_owned()
                } else if anterior.is_some_and(es_vocal) {
                    // Cierra diptongo dentro de la palabra: "hoy", "muy".
                    if anterior == Some('u') { "j" } else { "ɪ" }.to_owned()
                } else {
                    // La `y` que va sola es la conjunción, y suena /i/.
                    "i".to_owned()
                }
            }
            'z' => ese.to_owned(),
            otra if otra.is_alphabetic() => otra.to_string(),
            _ => String::new(),
        };

        tras_vocal = es_vocal(c);
        tras_nasal = matches!(c, 'n' | 'm' | 'ñ');
        tras_pausa = false;
        fonemas.push_str(&emitido);
        i += 1;
    }

    *contexto = Contexto {
        tras_vocal,
        tras_nasal,
        tras_pausa,
    };
    Palabra {
        fonemas,
        nucleos: nucleos_en_salida,
        tonica: acentuada,
    }
}

const UNIDADES: [&str; 16] = [
    "cero", "uno", "dos", "tres", "cuatro", "cinco", "seis", "siete", "ocho", "nueve", "diez",
    "once", "doce", "trece", "catorce", "quince",
];
const DECENAS: [&str; 10] = [
    "",
    "diez",
    "veinte",
    "treinta",
    "cuarenta",
    "cincuenta",
    "sesenta",
    "setenta",
    "ochenta",
    "noventa",
];
const CENTENAS: [&str; 10] = [
    "",
    "ciento",
    "doscientos",
    "trescientos",
    "cuatrocientos",
    "quinientos",
    "seiscientos",
    "setecientos",
    "ochocientos",
    "novecientos",
];

/// Un número, dicho con palabras.
///
/// Un libro está lleno de fechas, de páginas y de cifras, y el modelo de voz solo
/// sabe leer letras: sin esto, "25" se pierde o se deletrea.
#[must_use]
pub fn numero_en_palabras(cifras: &str) -> String {
    let Ok(n) = cifras.parse::<u64>() else {
        return cifras.chars().map(|c| format!("{c} ")).collect();
    };
    decir(n)
}

fn decir(n: u64) -> String {
    match n {
        0..=15 => UNIDADES[n as usize].to_owned(),
        16 => "dieciséis".to_owned(),
        17..=19 => format!("dieci{}", UNIDADES[(n - 10) as usize]),
        20 => "veinte".to_owned(),
        22 => "veintidós".to_owned(),
        23 => "veintitrés".to_owned(),
        26 => "veintiséis".to_owned(),
        21..=29 => format!("veinti{}", UNIDADES[(n - 20) as usize]),
        30..=99 => {
            let decena = DECENAS[(n / 10) as usize];
            if n.is_multiple_of(10) {
                decena.to_owned()
            } else {
                format!("{decena} y {}", UNIDADES[(n % 10) as usize])
            }
        }
        100 => "cien".to_owned(),
        101..=999 => {
            let centena = CENTENAS[(n / 100) as usize];
            if n.is_multiple_of(100) {
                centena.to_owned()
            } else {
                format!("{centena} {}", decir(n % 100))
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
                format!("{cabeza} {}", decir(n % 1_000))
            }
        }
        _ => {
            let millones = n / 1_000_000;
            let cabeza = if millones == 1 {
                "un millón".to_owned()
            } else {
                format!("{} millones", decir(millones))
            };
            if n.is_multiple_of(1_000_000) {
                cabeza
            } else {
                format!("{cabeza} {}", decir(n % 1_000_000))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonemas(palabra: &str) -> String {
        de_texto(palabra, Variante::EspanolDeAmerica)
    }

    #[test]
    fn las_oclusivas_se_ablandan_entre_vocales() {
        assert_eq!(fonemas("lobo"), "lˈoβo");
        assert_eq!(fonemas("nada"), "nˈaða");
        assert_eq!(fonemas("mago"), "mˈaɣo");
        // Al empezar y tras nasal, no.
        assert_eq!(fonemas("vaca"), "bˈaka");
        assert_eq!(fonemas("también"), "tambjˈen");
    }

    #[test]
    fn la_erre_suena_fuerte_donde_toca() {
        assert_eq!(fonemas("rosa"), "rˈosa");
        assert_eq!(fonemas("perro"), "pˈero");
        assert_eq!(fonemas("caro"), "kˈaɾo");
        assert_eq!(fonemas("honra"), "ˈonra");
    }

    #[test]
    fn la_jota_es_jota_y_no_ka() {
        // El defecto medido en el crate de referencia el 2026-09-17: convertía
        // /x/ en /k/, así que "rojo" sonaba "roko".
        assert_eq!(fonemas("rojo"), "rˈoxo");
        assert_eq!(fonemas("gente"), "xˈɛnte");
        assert!(fonemas("Jorge").starts_with('x'));
    }

    #[test]
    fn el_acento_cae_donde_dice_la_regla() {
        assert_eq!(fonemas("casa"), "kˈasa");
        assert_eq!(fonemas("papel"), "papˈel");
        assert_eq!(fonemas("árbol"), "ˈaɾβol");
        assert_eq!(fonemas("corazón"), "kˌoɾasˈon");
        assert_eq!(fonemas("instituto"), "ˌinstitˈuto");
    }

    #[test]
    fn el_espanol_de_america_no_cecea() {
        assert_eq!(fonemas("ciudad"), "sjuðˈad");
        assert_eq!(fonemas("llave"), "ʝˈaβe");
        assert_eq!(
            de_texto("ciudad", Variante::EspanolDeEspana),
            "θjuðˈad",
            "y el de España sí"
        );
    }

    #[test]
    fn los_numeros_se_dicen_con_palabras() {
        assert_eq!(numero_en_palabras("25"), "veinticinco");
        assert_eq!(numero_en_palabras("100"), "cien");
        assert_eq!(
            numero_en_palabras("1492"),
            "mil cuatrocientos noventa y dos"
        );
        assert_eq!(numero_en_palabras("2026"), "dos mil veintiséis");
        assert!(
            fonemas("25 libros").starts_with("bˌeɪnti"),
            "{}",
            fonemas("25 libros")
        );
    }

    /// Antes "III" hacía entrar en pánico a la conversión, y con ella
    /// a la preparación entera del capítulo: tres débiles seguidas ponían el acento
    /// a mitad de un carácter de dos bytes. Un número romano en un título no es raro.
    #[test]
    fn un_numero_romano_se_dice_y_no_rompe_la_conversion() {
        for texto in ["III", "Capítulo III", "XIII", "iii", "CAPÍTULO VIII."] {
            let dicho = fonemas(texto);
            // Con su acento: sin él, el defecto seguiría ahí y solo se habría
            // dejado de ver.
            assert!(dicho.contains('ˈ'), "{texto}: {dicho}");
        }
    }

    /// **Ningún núcleo de ninguna palabra cae a mitad de un carácter.** Se recorren
    /// todas las de hasta cuatro letras de un alfabeto que mezcla débiles, fuertes,
    /// tildes, diéresis y las consonantes que se comen la letra siguiente: es donde
    /// vivía el defecto de arriba, y es barato mirarlo entero. Se mira la posición y
    /// no solo que no haya pánico, porque `con_acentos` ya no entra en pánico: se
    /// salta la marca, y eso callaría el defecto en lugar de impedirlo.
    #[test]
    fn ningun_acento_cae_a_mitad_de_un_caracter() {
        let alfabeto: Vec<char> = "aeiouíúüyhqgcl".chars().collect();
        let mut palabra = String::new();
        fn recorrer(alfabeto: &[char], palabra: &mut String, quedan: usize) {
            if !palabra.is_empty() {
                let letras: Vec<char> = palabra.chars().collect();
                let grupos = nucleos(&letras, &mudas(&letras));
                let tonica = tonica(&letras, &grupos);
                for variante in [Variante::EspanolDeAmerica, Variante::EspanolDeEspana] {
                    let mut contexto = Contexto::inicial();
                    let hecha = letras_a_fonemas(&letras, &grupos, tonica, variante, &mut contexto);
                    for posicion in &hecha.nucleos {
                        assert!(
                            hecha.fonemas.is_char_boundary(*posicion),
                            "{palabra}: núcleo en {posicion} de {}",
                            hecha.fonemas
                        );
                    }
                }
            }
            if quedan == 0 {
                return;
            }
            for c in alfabeto {
                palabra.push(*c);
                recorrer(alfabeto, palabra, quedan - 1);
                palabra.pop();
            }
        }
        recorrer(&alfabeto, &mut palabra, 4);
    }

    /// Lo que el modelo no sabe decir, lo calla: "naïf" sonaba "naf".
    #[test]
    fn una_letra_latina_ajena_se_dice_con_su_vocal_y_no_se_calla() {
        for texto in [
            "naïf",
            "à la carte",
            "São Paulo",
            "1ª",
            "Citroën",
            "garçon",
            "Gödel",
        ] {
            let dicho = fonemas(texto);
            assert!(super::super::todos_admitidos(&dicho), "{texto}: {dicho}");
        }
        assert_eq!(fonemas("è"), fonemas("e"));
    }

    #[test]
    fn la_puntuacion_se_conserva_como_pausa() {
        let dicho = fonemas("Hola, mundo.");
        assert!(dicho.contains(','), "{dicho}");
        assert!(dicho.ends_with('.'), "{dicho}");
        assert!(super::super::todos_admitidos(&dicho), "{dicho}");
    }
}
