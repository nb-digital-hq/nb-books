//! El inglés, por **diccionario**, no por reglas.
//!
//!
//!
//! # Por qué aquí no hay reglas, y en los otros dos idiomas sí
//!
//! Porque el inglés no se lee como se escribe y eso no es una exageración: "though",
//! "through", "tough", "thought" y "thorough" comparten cinco letras y no comparten
//! ninguna pronunciación. Escribir reglas para eso produce un sistema que se
//! equivoca de forma impredecible, que es peor que uno que diga "esta palabra no la
//! sé": un lector que oye una palabra rara sabe que algo falla, pero uno que oye una
//! palabra **distinta** no se entera.
//!
//! Así que el inglés se resuelve con una tabla de pronunciaciones, y lo que no esté
//! en ella se deletrea por reglas mínimas, marcado como lo que es: una aproximación.
//!
//! # Dónde vive esa tabla, y por qué no está en el repositorio
//!
//! **Viaja con la voz**, como un archivo más del modelo, y se carga de su carpeta.
//! Dos razones. Una tabla de pronunciaciones útil son megabytes, y el innegociable
//! dice que ningún peso de modelo entra al repositorio. Y la tabla es **del modelo**:
//! la escribió quien lo entrenó, con su licencia y su huella, así que entra por la
//! misma puerta verificada que los pesos -catálogo, revisión de licencia y huella
//! comprobada al descargar- en lugar de por una carpeta de datos que nadie revisa.
//!
//! Mientras no haya una entrada inglesa en el catálogo con su huella verificada,
//! esto no tiene voz que lo use. Lo que hay es el mecanismo, probado con una tabla
//! pequeña: ver `fonemas_ingles.rs`.

use std::collections::HashMap;
use std::path::Path;

/// El nombre del archivo dentro de la carpeta del modelo.
///
/// Un nombre fijo y no una preferencia: la tabla es del modelo, y buscarla por
/// configuración permitiría apuntar a otra cosa sin que nadie lo hubiera verificado.
pub const ARCHIVO: &str = "pronunciaciones.txt";

/// Palabra y fonemas, tal y como los espera el modelo.
///
/// El formato es el mismo que el de la referencia de las pruebas: una palabra, un
/// tabulador y su transcripción. Es legible a simple vista, que importa porque es lo
/// que permite comprobar a mano por qué una palabra suena como suena.
#[derive(Debug, Default)]
pub struct Diccionario {
    entradas: HashMap<String, String>,
}

impl Diccionario {
    /// Carga la tabla de la carpeta de un modelo, si está.
    ///
    /// Devuelve `None` cuando no hay archivo, que es distinto de un archivo vacío:
    /// sin tabla el inglés no se ofrece, y con una tabla vacía se ofrecería diciendo
    /// todo por aproximación.
    #[must_use]
    pub fn cargar(carpeta: &Path) -> Option<Self> {
        let texto = std::fs::read_to_string(carpeta.join(ARCHIVO)).ok()?;
        Some(Self::leer(&texto))
    }

    /// Lee la tabla ya en memoria. Las líneas que no tienen la forma se saltan, y
    /// una tabla rota se queda corta en lugar de tirar la voz entera.
    #[must_use]
    pub fn leer(texto: &str) -> Self {
        let entradas = texto
            .lines()
            .filter(|linea| !linea.trim().is_empty() && !linea.starts_with('#'))
            .filter_map(|linea| {
                let (palabra, fonemas) = linea.split_once('\t')?;
                let fonemas = fonemas.trim();
                if fonemas.is_empty() {
                    return None;
                }
                Some((palabra.trim().to_lowercase(), fonemas.to_owned()))
            })
            .collect();
        Self { entradas }
    }

    /// Cuántas palabras sabe decir.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entradas.len()
    }

    /// Si no sabe ninguna.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entradas.is_empty()
    }

    /// La transcripción de una palabra, si está.
    #[must_use]
    pub fn buscar(&self, palabra: &str) -> Option<&str> {
        self.entradas
            .get(&palabra.to_lowercase())
            .map(String::as_str)
    }
}

/// Convierte un texto inglés en fonemas con la tabla que le pasen.
///
/// Lo que no esté en la tabla se aproxima por reglas mínimas. **La aproximación no
/// se esconde**: [`sin_diccionario`] dice qué palabras salieron así, y quien llame
/// puede decidir -por ejemplo, no ofrecer la voz si son muchas- en lugar de
/// enterarse por el oído.
#[must_use]
pub fn de_texto(texto: &str, diccionario: &Diccionario) -> String {
    let mut salida = String::with_capacity(texto.len() * 2);
    for pieza in trocear(texto) {
        match pieza {
            Pieza::Palabra(palabra) => {
                if !salida.is_empty() && !salida.ends_with(' ') {
                    salida.push(' ');
                }
                match diccionario.buscar(&palabra) {
                    Some(fonemas) => salida.push_str(fonemas),
                    None => salida.push_str(&aproximar(&palabra)),
                }
            }
            Pieza::Puntuacion(c) => salida.push(c),
        }
    }
    salida.trim().to_owned()
}

/// Las palabras de un texto que la tabla no sabe decir.
///
/// Existe para poder mirar antes de hablar: una tabla a la que le falta la mitad del
/// texto no se usa, se dice.
#[must_use]
pub fn sin_diccionario(texto: &str, diccionario: &Diccionario) -> Vec<String> {
    trocear(texto)
        .into_iter()
        .filter_map(|pieza| match pieza {
            Pieza::Palabra(p) if diccionario.buscar(&p).is_none() => Some(p),
            _ => None,
        })
        .collect()
}

enum Pieza {
    Palabra(String),
    Puntuacion(char),
}

const PAUSAS: &str = ",.;:!?…—";

fn trocear(texto: &str) -> Vec<Pieza> {
    let mut piezas = Vec::new();
    let mut actual = String::new();
    for c in texto.chars() {
        if c.is_alphanumeric() || c == '\'' {
            actual.push(c.to_lowercase().next().unwrap_or(c));
        } else {
            if !actual.is_empty() {
                piezas.push(Pieza::Palabra(std::mem::take(&mut actual)));
            }
            if PAUSAS.contains(c) {
                piezas.push(Pieza::Puntuacion(c));
            }
        }
    }
    if !actual.is_empty() {
        piezas.push(Pieza::Palabra(actual));
    }
    piezas
}

/// Una aproximación para lo que no está en la tabla.
///
/// Deliberadamente corta: lo que hace falta de ella es que un nombre propio
/// desconocido suene a algo y no desaparezca, no que acierte. Acertar es el trabajo
/// de la tabla.
fn aproximar(palabra: &str) -> String {
    let letras: Vec<char> = palabra.chars().collect();
    let mut salida = String::with_capacity(palabra.len());
    let mut i = 0;
    while i < letras.len() {
        let c = letras[i];
        let siguiente = letras.get(i + 1).copied();
        let (fonema, avance): (&str, usize) = match (c, siguiente) {
            ('c', Some('h')) => ("ʧ", 2),
            ('s', Some('h')) => ("ʃ", 2),
            ('t', Some('h')) => ("θ", 2),
            ('p', Some('h')) => ("f", 2),
            ('c', Some('e' | 'i' | 'y')) => ("s", 1),
            ('c', _) => ("k", 1),
            ('g', Some('e' | 'i' | 'y')) => ("ʤ", 1),
            ('g', _) => ("ɡ", 1),
            ('j', _) => ("ʤ", 1),
            ('q', _) => ("k", 1),
            ('x', _) => ("ks", 1),
            ('y', _) => ("i", 1),
            ('a', _) => ("æ", 1),
            ('e', _) if i + 1 == letras.len() => ("", 1),
            ('e', _) => ("ɛ", 1),
            ('i', _) => ("ɪ", 1),
            ('o', _) => ("ɔ", 1),
            ('u', _) => ("ʌ", 1),
            (otra, _) if otra.is_ascii_alphabetic() => {
                salida.push(otra);
                i += 1;
                continue;
            }
            _ => ("", 1),
        };
        salida.push_str(fonema);
        i += avance;
    }
    // Sin saber dónde cae el acento, la primera sílaba: en inglés es lo más
    // frecuente, y una palabra sin ninguna marca sale plana.
    match salida.char_indices().find(|(_, c)| "æɛɪɔʌiɑ".contains(*c)) {
        Some((donde, _)) => {
            let mut con_acento = salida[..donde].to_owned();
            con_acento.push('ˈ');
            con_acento.push_str(&salida[donde..]);
            con_acento
        }
        None => salida,
    }
}
