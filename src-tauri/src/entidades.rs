//! Los nombres del libro, con dónde aparecen: local y sin IA.
//!
//! Es la versión propia de lo que Kindle llama X-Ray: quién era ese personaje que
//! apareció hace doscientas páginas. La ficha pone tres condiciones, y
//! las tres son de diseño y no de detalle:
//!
//! 1. **Sin llamar a ningún modelo**, y con la IA apagada funciona igual. Los
//!    candidatos salen del texto: secuencias de palabras con mayúscula que no están
//!    solo a principio de frase, con los conectores de un nombre -"de", "del",
//!    "van"- dentro.
//! 2. **Con FTS5**, sobre el índice que ya existe: dónde aparece un nombre se le
//!    pregunta al índice de búsqueda con una consulta de frase acotada a los
//!    fragmentos de esta edición -"Buendía" y "Buendia" son el mismo nombre, como
//!    en la búsqueda, y encuentra también las menciones que el recorrido no ve-. Se
//!    pregunta **al desplegar un nombre** y no al construir la lista: medido sobre
//!    la instalación real, preguntarlo para todos a la vez costaba 273 ms de mediana
//!    y 9,6 s en el peor libro; uno solo, lo que tarda una consulta. Si el libro aún
//!    no está indexado, los sitios son los del propio recorrido, y se dice.
//! 3. **No se guarda nada.** Se calcula al pedirlo y vive lo que vive el documento
//!    abierto: el disco que cuesta es cero, que es la única forma de entrar en el
//!    presupuesto del 20 % de lo derivado (`performance-budgets.md`, 4.4) sin
//!    discutirlo. Medido sobre el corpus real en la sección 4.21 de ese documento.
//!
//! # Lo que no hace, dicho
//!
//! **No clasifica** en personajes, lugares y términos. Distinguir "Macondo" de
//! "Aureliano" sin un modelo sería adivinar, y un índice que adivina mal es peor
//! que una lista que no finge saber. Tampoco une variantes -"Aureliano" y
//! "Aureliano Buendía" salen por separado- porque decidir que son la misma persona
//! también es adivinar.

use std::collections::HashMap;

use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::reader::{DocumentProvider, Locator};

/// Cuántas veces tiene que aparecer un nombre para entrar. Con menos, la lista se
/// llena de lo que el libro nombra una vez de pasada.
pub const MINIMO_DE_MENCIONES: u32 = 3;

/// Cuántos nombres como mucho. Cota: los que más aparecen.
pub const MAXIMO_DE_NOMBRES: usize = 200;

/// Cuántos sitios como mucho por nombre, en el orden del libro.
pub const MAXIMO_DE_SITIOS: usize = 60;

/// El tamaño de los fragmentos en que se recorre el texto: el mismo que usa el
/// índice de búsqueda, para que un sitio sea el mismo fragmento en los dos.
const FRAGMENTO: usize = crate::search::FRAGMENTO;

/// Un nombre del libro.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Entity {
    /// Como aparece escrito la mayoría de las veces.
    pub name: String,
    /// Cuántas veces aparece en todo el libro.
    pub mentions: u32,
    /// Dónde, en el orden del libro: un fragmento por sitio, con su `Locator`.
    ///
    /// Cota: [`MAXIMO_DE_SITIOS`].
    pub places: Vec<Locator>,
}

/// El índice de nombres de un libro, con los sitios del propio recorrido.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct EntityIndex {
    /// Cota: [`MAXIMO_DE_NOMBRES`].
    pub entities: Vec<Entity>,
}

/// Dónde aparece un nombre, y de dónde sale la respuesta.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct EntityPlaces {
    /// Cota: [`MAXIMO_DE_SITIOS`].
    pub places: Vec<Locator>,
    /// Si salieron del índice de búsqueda. Si el libro no está indexado todavía,
    /// son los del recorrido del texto: los mismos fragmentos, sin las menciones
    /// que solo el índice ve.
    pub from_index: bool,
}

/// Palabras que se escriben con mayúscula sin ser un nombre: al principio de una
/// frase o porque el idioma las pone así. En español, inglés y portugués, que son
/// los de la interfaz.
const COMUNES: &str = concat!(
    // español
    "el la los las un una unos unas y o pero si no sí que qué cuando cuándo como cómo donde ",
    "dónde entonces después antes ya aunque mientras porque pues así yo tú él ella nosotros ",
    "ellos ellas usted ustedes eso esto este esta ese esa aquel aquella en de del al a con por ",
    "para sin sobre tras hasta desde hay era fue es muy más menos todo todos nada nadie también ",
    "tampoco ahora luego aquí allí ni mi su sus le lo se me te nos capítulo señor señora don ",
    "doña ",
    // inglés
    "the an and or but if when then i he she it we they you his her its their this that these ",
    "those in on at of to for with from by as is was not there what who how why where my our ",
    "your chapter mr mrs miss ",
    // portugués
    "os as um uma e mas se quando então eu ele ela nós eles elas você isso isto esse essa em do ",
    "da dos das no na nos nas com para não sim ",
);

/// Lo que puede ir dentro de un nombre sin mayúscula: "Gabriel García de la Vega".
const CONECTORES: &[&str] = &[
    "de", "del", "la", "las", "los", "y", "e", "da", "das", "do", "dos", "van", "von", "der",
    "den", "di", "du", "of", "the",
];

/// Una palabra del texto, con si empieza por mayúscula y si va a principio de frase.
struct Palabra<'a> {
    texto: &'a str,
    mayuscula: bool,
    inicial: bool,
}

fn palabras(texto: &str) -> Vec<Palabra<'_>> {
    let mut salida = Vec::new();
    let mut inicial = true;
    let mut inicio: Option<usize> = None;
    for (i, c) in texto.char_indices() {
        if c.is_alphanumeric() || (inicio.is_some() && matches!(c, '\'' | '’' | '-')) {
            inicio.get_or_insert(i);
            continue;
        }
        if let Some(desde) = inicio.take() {
            let texto_palabra = &texto[desde..i];
            salida.push(Palabra {
                texto: texto_palabra,
                mayuscula: texto_palabra.chars().next().is_some_and(char::is_uppercase),
                inicial,
            });
            inicial = false;
        }
        // Lo que acaba una frase -o abre un diálogo- hace que la palabra que sigue
        // lleve mayúscula por norma y no por ser un nombre.
        if matches!(
            c,
            '.' | '!' | '?' | '¿' | '¡' | '«' | '"' | '“' | '—' | ':' | '\n'
        ) {
            inicial = true;
        }
    }
    if let Some(desde) = inicio {
        let texto_palabra = &texto[desde..];
        salida.push(Palabra {
            texto: texto_palabra,
            mayuscula: texto_palabra.chars().next().is_some_and(char::is_uppercase),
            inicial,
        });
    }
    salida
}

fn es_comun(palabra: &str) -> bool {
    static CONJUNTO: std::sync::OnceLock<std::collections::HashSet<&'static str>> =
        std::sync::OnceLock::new();
    CONJUNTO
        .get_or_init(|| COMUNES.split_whitespace().collect())
        .contains(palabra.to_lowercase().as_str())
}

/// Los nombres que aparecen en un trozo de texto: cada uno con si fue solo a
/// principio de frase, que es cuando la mayúscula no prueba nada.
fn nombres_en(texto: &str) -> Vec<(String, bool)> {
    let lista = palabras(texto);
    let mut salida = Vec::new();
    let mut i = 0;
    while i < lista.len() {
        let primera = &lista[i];
        if !primera.mayuscula || es_comun(primera.texto) || primera.texto.chars().count() < 2 {
            i += 1;
            continue;
        }
        // Se alarga mientras sigan mayúsculas, dejando pasar los conectores de un
        // nombre solo si detrás vuelve a haber una mayúscula.
        let mut fin = i + 1;
        let mut j = i + 1;
        while j < lista.len() {
            let palabra = &lista[j];
            if palabra.inicial {
                break;
            }
            if palabra.mayuscula && !es_comun(palabra.texto) {
                fin = j + 1;
                j += 1;
            } else if CONECTORES.contains(&palabra.texto.to_lowercase().as_str()) {
                j += 1;
            } else {
                break;
            }
        }
        let nombre = lista[i..fin]
            .iter()
            .map(|p| p.texto)
            .collect::<Vec<_>>()
            .join(" ");
        salida.push((nombre, primera.inicial && fin == i + 1));
        i = fin;
    }
    salida
}

/// Lo que se sabe de un nombre mientras se recorre el libro.
#[derive(Default)]
struct Cuenta {
    menciones: u32,
    en_medio: u32,
    formas: HashMap<String, u32>,
    sitios: Vec<usize>,
}

/// La forma normalizada con la que se agrupa un nombre: sin tildes y en
/// minúsculas, como lo hace el índice de búsqueda.
fn clave(nombre: &str) -> String {
    nombre
        .chars()
        .flat_map(char::to_lowercase)
        .map(sin_tilde)
        .collect()
}

fn sin_tilde(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        otro => otro,
    }
}

/// Construye el índice de nombres de un documento abierto, con los sitios del
/// propio recorrido del texto.
///
/// # Errors
///
/// [`NbError::FormatUnsupported`] si el documento no tiene texto: un cómic o un
/// escaneado sin capa de texto no tienen nombres que listar.
pub fn construir(documento: &dyn DocumentProvider) -> NbResult<EntityIndex> {
    let fragmentos = documento.text_chunks(FRAGMENTO)?;
    let mut cuentas: HashMap<String, Cuenta> = HashMap::new();
    for (indice, (_, texto)) in fragmentos.iter().enumerate() {
        for (nombre, solo_inicial) in nombres_en(texto) {
            let cuenta = cuentas.entry(clave(&nombre)).or_default();
            cuenta.menciones += 1;
            if !solo_inicial {
                cuenta.en_medio += 1;
            }
            *cuenta.formas.entry(nombre).or_default() += 1;
            if cuenta.sitios.last() != Some(&indice) {
                cuenta.sitios.push(indice);
            }
        }
    }

    let mut elegidos: Vec<(String, Cuenta)> = cuentas
        .into_iter()
        // Un nombre que solo aparece a principio de frase no prueba nada: es la
        // mayúscula de la norma, no la del nombre.
        .filter(|(_, c)| c.menciones >= MINIMO_DE_MENCIONES && c.en_medio > 0)
        .collect();
    elegidos.sort_by(|a, b| {
        b.1.menciones
            .cmp(&a.1.menciones)
            .then_with(|| a.0.cmp(&b.0))
    });
    elegidos.truncate(MAXIMO_DE_NOMBRES);

    let mut entidades = Vec::with_capacity(elegidos.len());
    for (_, cuenta) in elegidos {
        let nombre = cuenta
            .formas
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
            .map(|(forma, _)| forma.clone())
            .unwrap_or_default();
        let sitios = cuenta
            .sitios
            .iter()
            .take(MAXIMO_DE_SITIOS)
            .filter_map(|i| fragmentos.get(*i).map(|(l, _)| l.clone()))
            .collect();
        entidades.push(Entity {
            name: nombre,
            mentions: cuenta.menciones,
            places: sitios,
        });
    }

    Ok(EntityIndex {
        entities: entidades,
    })
}

/// Dónde aparece un nombre según el índice de búsqueda, o nada si esta edición no
/// está indexada todavía.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
pub fn sitios_del_indice(
    conn: &Connection,
    edition_id: &str,
    nombre: &str,
) -> NbResult<Option<Vec<Locator>>> {
    let Some(rango) = rango_indexado(conn, edition_id)? else {
        return Ok(None);
    };
    sitios_en_el_indice(conn, nombre, rango).map(Some)
}

/// Las filas del índice que son de esta edición, de la primera a la última.
///
/// Los fragmentos de una edición se escriben juntos, así que ocupan un tramo
/// seguido de filas: acotar la consulta de frase a ese tramo es lo que evita que
/// "José" recorra los fragmentos de toda la biblioteca para quedarse con los de un
/// libro.
fn rango_indexado(conn: &Connection, edition_id: &str) -> NbResult<Option<(i64, i64)>> {
    conn.query_row(
        "SELECT min(rowid), max(rowid) FROM text_chunks WHERE edition_id = ?1",
        [edition_id],
        |fila| {
            Ok((
                fila.get::<_, Option<i64>>(0)?,
                fila.get::<_, Option<i64>>(1)?,
            ))
        },
    )
    .map(|(desde, hasta)| desde.zip(hasta))
    .map_err(|error| {
        tracing::error!(error = %error, "no se pudo acotar el índice de una edición");
        NbError::StorageFailure
    })
}

/// Dónde aparece un nombre, según el índice de búsqueda, en el orden del libro.
fn sitios_en_el_indice(
    conn: &Connection,
    nombre: &str,
    (desde, hasta): (i64, i64),
) -> NbResult<Vec<Locator>> {
    // Una frase de FTS5: los términos entre comillas y juntos, y las comillas de
    // dentro duplicadas, que es lo único que hay que neutralizar ahí.
    let frase = nombre
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .replace('"', "\"\"");
    let mut consulta = conn
        .prepare_cached(
            "SELECT c.locator FROM documents_fts
               JOIN text_chunks c ON c.rowid = documents_fts.rowid
              WHERE documents_fts MATCH ?1 AND documents_fts.rowid BETWEEN ?2 AND ?3
              ORDER BY documents_fts.rowid
              LIMIT ?4",
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo preparar la consulta de nombres");
            NbError::StorageFailure
        })?;
    let limite = i64::try_from(MAXIMO_DE_SITIOS).unwrap_or(i64::MAX);
    let filas = consulta
        .query_map(
            rusqlite::params![format!("\"{frase}\""), desde, hasta, limite],
            |f| f.get::<_, String>(0),
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo consultar dónde aparece un nombre");
            NbError::StorageFailure
        })?;
    Ok(filas
        .filter_map(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect())
}

#[cfg(test)]
mod pruebas {
    use super::{clave, nombres_en};

    #[test]
    fn un_nombre_compuesto_con_sus_conectores_es_uno_solo() {
        let nombres: Vec<String> =
            nombres_en("Y entonces llegó José Arcadio Buendía con Úrsula de la Vega a Macondo.")
                .into_iter()
                .map(|(n, _)| n)
                .collect();
        assert_eq!(
            nombres,
            vec!["José Arcadio Buendía", "Úrsula de la Vega", "Macondo"]
        );
    }

    #[test]
    fn la_mayuscula_de_principio_de_frase_no_hace_un_nombre() {
        let nombres = nombres_en("Pero nadie vino. Entonces Melquíades habló. Mañana llovió.");
        // "Pero" y "Entonces" son comunes; "Mañana" va a principio de frase y queda
        // marcada como tal, "Melquíades" no.
        assert_eq!(
            nombres,
            vec![
                ("Melquíades".to_owned(), false),
                ("Mañana".to_owned(), true)
            ]
        );
    }

    #[test]
    fn un_conector_al_final_no_se_queda_dentro_del_nombre() {
        let nombres: Vec<String> = nombres_en("El coronel Aureliano de nuevo.")
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(nombres, vec!["Aureliano"]);
    }

    #[test]
    fn la_clave_agrupa_sin_tildes_ni_mayusculas() {
        assert_eq!(clave("Buendía"), clave("BUENDIA"));
    }
}
