//! Corrección de metadatos por lotes.
//!
//! # Qué corrige, y por qué solo esto
//!
//! Los tres defectos que una biblioteca importada trae siempre, y que se pueden
//! reconocer **sin saber nada del libro**:
//!
//! 1. El título en mayúsculas enteras, que es como sale de muchos catálogos.
//! 2. Los espacios de más: dobles, al principio o al final.
//! 3. El punto final en un título, que no es parte del título.
//!
//! Lo que no hace es adivinar. Nada de "esto parece una serie" ni de "esta
//! editorial se escribe de otra forma": una corrección automática que se
//! equivoca en mil documentos a la vez es peor que mil documentos sin corregir.
//!
//! # Propone, y nada más
//!
//! Devuelve sugerencias. El anfitrión las registra y alguien decide. Por eso
//! cada una lleva su motivo escrito: quien decide tiene que poder ver **por
//! qué** antes de aceptar.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "bulk-metadata",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

struct Normalizador;

impl Guest for Normalizador {
    fn suggest(docs: Vec<Document>) -> Result<Vec<Suggestion>, ReviewError> {
        let mut salida = Vec::new();

        for documento in &docs {
            if let Some((valor, motivo)) = arreglar(&documento.title) {
                salida.push(Suggestion {
                    document: documento.id.clone(),
                    field: Field::Title,
                    value: valor,
                    reason: motivo,
                });
            }
            if let Some((valor, motivo)) = documento
                .publisher
                .as_deref()
                .and_then(|editorial| arreglar_espacios(editorial))
            {
                salida.push(Suggestion {
                    document: documento.id.clone(),
                    field: Field::Publisher,
                    value: valor,
                    reason: motivo,
                });
            }
        }
        Ok(salida)
    }
}

/// Lo que se le puede arreglar a un título, con el motivo.
///
/// Se aplica todo lo que toque y se devuelve **una sola** propuesta: dos
/// propuestas sobre el mismo campo se pisarían, y quien decide vería la segunda
/// sin saber que hubo una primera.
fn arreglar(titulo: &str) -> Option<(String, String)> {
    let mut motivos = Vec::new();
    let mut valor = titulo.to_owned();

    if let Some((limpio, motivo)) = arreglar_espacios(&valor) {
        valor = limpio;
        motivos.push(motivo);
    }
    if es_todo_mayusculas(&valor) {
        valor = a_capitalizacion(&valor);
        motivos.push("estaba en mayúsculas enteras".to_owned());
    }
    // Un punto final no es parte del título. Los puntos suspensivos sí, y por
    // eso se mira que el anterior no sea otro punto.
    if valor.ends_with('.') && !valor.ends_with("..") {
        valor.pop();
        motivos.push("terminaba en punto".to_owned());
    }

    (!motivos.is_empty() && valor != titulo).then(|| (valor, motivos.join("; ")))
}

/// Quita los espacios de más, si los hay.
fn arreglar_espacios(valor: &str) -> Option<(String, String)> {
    let limpio = valor.split_whitespace().collect::<Vec<_>>().join(" ");
    (limpio != valor).then(|| (limpio, "tenía espacios de más".to_owned()))
}

/// Si no tiene ni una minúscula y sí varias mayúsculas.
///
/// "Varias" y no "alguna": un título de una sola palabra en mayúsculas puede ser
/// una sigla, y convertir "NASA" en "Nasa" sería estropearlo. Con dos palabras
/// largas en mayúsculas ya no es una sigla.
fn es_todo_mayusculas(titulo: &str) -> bool {
    let letras: Vec<char> = titulo.chars().filter(|c| c.is_alphabetic()).collect();
    if letras.len() < 8 || letras.iter().any(|c| c.is_lowercase()) {
        return false;
    }
    titulo
        .split_whitespace()
        .filter(|palabra| palabra.chars().filter(|c| c.is_alphabetic()).count() > 2)
        .count()
        >= 2
}

/// Mayúscula solo en la primera palabra, como se escribe un título en español.
///
/// Hasta el 2026-11-12 ponía mayúscula en cada palabra larga, el estilo inglés,
/// y proponía "Funciones de Varias Variables". Lo que lleva cifras se deja
/// como está: "2026-1S" o "4E" son códigos, no palabras, y bajarlos los rompía.
fn a_capitalizacion(titulo: &str) -> String {
    titulo
        .split_whitespace()
        .enumerate()
        .map(|(indice, palabra)| {
            if palabra.chars().any(|c| c.is_ascii_digit()) {
                return palabra.to_owned();
            }
            let minuscula = palabra.to_lowercase();
            if indice > 0 {
                return minuscula;
            }
            let mut letras = minuscula.chars();
            match letras.next() {
                Some(primera) => primera.to_uppercase().collect::<String>() + letras.as_str(),
                None => minuscula,
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod pruebas {
    use super::a_capitalizacion;

    #[test]
    fn un_titulo_en_mayusculas_pasa_a_mayuscula_inicial_y_respeta_los_codigos() {
        assert_eq!(
            a_capitalizacion("ORIGENES DE LAS CIUDADES"),
            "Origenes de las ciudades"
        );
        assert_eq!(
            a_capitalizacion("FUNCIONES DE VARIAS VARIABLES - G12 - 2026-1S"),
            "Funciones de varias variables - G12 - 2026-1S"
        );
    }
}

export!(Normalizador);
