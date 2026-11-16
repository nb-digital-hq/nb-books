//! De texto a fonemas, **sin `espeak-ng`**.
//!
//! Contrato: `docs/architecture/ai/local-models.md`.
//!
//! # Por qué se escribe aquí y no se usa el de todos
//!
//! Todos los sintetizadores pequeños —Piper, Kokoro— convierten el texto en
//! fonemas con `espeak-ng`, que es **GPL-3.0**: enlazarla obligaría a publicar NB
//! Books bajo esa licencia, y la [política de dependencias] no la admite. Así que
//! la conversión es nuestra.
//!
//! Y al escribirla se gana algo que no se podía comprar: **el español de América**.
//! `espeak` tiene las dos variantes y el crate de referencia que se revisó el
//! 2026-09-17 solo usa la de España, así que "ciudad" salía con la zeta castellana
//! para quien nunca la ha dicho. Aquí la variante es una opción, y la de fábrica es
//! la de América, que es donde nació este producto.
//!
//! **Y arregla un defecto medido.** Ese mismo crate convierte `/x/` en `/k/` para
//! todos los idiomas, así que en español "rojo" sonaba "roko" y "Jorge", "Korge".
//! Se comprobó sintetizando la frase y escuchándola.
//!
//! # Qué es "correcto" aquí, y cómo se comprueba
//!
//! El modelo de voz se entrenó con los fonemas que produce `espeak-ng`, así que lo
//! correcto es **lo que ella habría dicho**, no lo que diga un manual de fonética.
//! Por eso la prueba compara palabra a palabra contra una lista de referencia
//! generada con `espeak-ng` en esta máquina —herramienta de desarrollo, no
//! dependencia: no se enlaza, no se distribuye y su salida no es suya— y lo que no
//! coincide se clasifica en vez de esconderse: ver `fonemas_espanol.rs`.
//!
//! [política de dependencias]: ../../../docs/architecture/dependency-policy.md

pub mod espanol;
pub mod ingles;
pub mod portugues;

/// Qué se habla. Cada variante produce los fonemas de su región.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variante {
    /// Español de América: "ciudad" es /sjuðad/ y "llave", /ʝaβe/.
    #[default]
    EspanolDeAmerica,
    /// Español de España: con /θ/ y con /ʎ/.
    EspanolDeEspana,
    /// Portugués de Brasil: vocales átonas cerradas y /t/ y /d/ palatalizadas.
    ///
    /// **Sin voz en el catálogo todavía**: la conversión existe y está
    /// medida, pero elegir entre /e/ y /ɛ/ en portugués es léxico y no regla, así
    /// que falta el diccionario que lo decida. Una voz que dice mal una de cada
    /// tres palabras no se ofrece.
    PortuguesDeBrasil,
}

impl Variante {
    /// La variante que le toca a una voz del modelo, por su identificador.
    ///
    /// Kokoro nombra sus voces con la inicial del idioma —`e` español, `p`
    /// portugués—, así que la variante **sale de la voz elegida** y no de una
    /// preferencia aparte: elegir una voz portuguesa y que hablara con reglas
    /// españolas sería un ajuste que se puede dejar mal puesto.
    /// **El inglés no está aquí a propósito**: no se resuelve con reglas sino con
    /// la tabla de [`ingles::Diccionario`], que viaja con el modelo, así que elegir
    /// "inglés" sin tener la tabla delante no significaría nada.
    #[must_use]
    pub fn de_voz(voz: &str) -> Self {
        if voz.starts_with('p') {
            Self::PortuguesDeBrasil
        } else {
            Self::default()
        }
    }
}

/// Convierte un texto en la cadena de fonemas que espera el modelo de voz.
///
/// Devuelve IPA con las marcas de acento —`ˈ` principal y `ˌ` secundario— y la
/// puntuación que el modelo entiende como pausa.
#[must_use]
pub fn de_texto(texto: &str, variante: Variante) -> String {
    match variante {
        Variante::EspanolDeAmerica | Variante::EspanolDeEspana => {
            espanol::de_texto(texto, variante)
        }
        Variante::PortuguesDeBrasil => portugues::de_texto(texto),
    }
}

/// Los símbolos que el modelo admite (`config.json` de Kokoro, 114 fichas).
///
/// Se comprueba en una prueba: un fonema fuera de esta lista no lo puede
/// pronunciar el modelo, y lo que no puede pronunciar se lo come en silencio, que
/// es peor que decirlo mal.
pub const ADMITIDOS: &str = ";:,.!?—…\"()“” ̃ʣʥʦʨᵝꭧAIOQSTWYᵊabcdefhijklmnopqrstuvwxyzɑɐɒæβɔɕçɖðʤəɚɛɜɟɡɥɨɪʝɯɰŋɳɲɴøɸθœɹɾɻʁɽʂʃʈʧʊʋʌɣɤχʎʒʔˈˌːʰʲ↓→↗↘ᵻ";

/// Si todos los fonemas de una cadena están en el vocabulario del modelo.
#[must_use]
pub fn todos_admitidos(fonemas: &str) -> bool {
    fonemas.chars().all(|c| c == ' ' || ADMITIDOS.contains(c))
}
