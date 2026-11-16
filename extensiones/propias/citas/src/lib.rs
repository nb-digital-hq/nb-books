//! Exportación de citas a BibTeX y RIS.
//!
//! # Qué recibe y qué no
//!
//! Recibe una lista de referencias ya compuesta por el anfitrión y devuelve el
//! archivo entero. No consulta la biblioteca, no elige qué documentos entran y
//! no escribe en disco: el dato es del usuario y esto solo le da forma.
//!
//! # Los dos formatos, y por qué esos dos
//!
//! BibTeX lo importa cualquier gestor bibliográfico y es el de LaTeX; RIS lo
//! importan Zotero, Mendeley, EndNote y las bases de datos académicas. Con los
//! dos se cubre lo que alguien va a necesitar de verdad. CSL-JSON sería el
//! tercero y no está: es el formato de intercambio de los procesadores de
//! citas, no el que se importa a mano.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "citation-provider",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

struct Citas;

impl Guest for Citas {
    fn format_citations(id: String, refs: Vec<Reference>) -> Result<String, FormatError> {
        match id.as_str() {
            "bibtex" => Ok(refs.iter().map(bibtex).collect::<Vec<_>>().join("\n")),
            "ris" => Ok(refs.iter().map(ris).collect::<Vec<_>>().join("\n")),
            _ => Err(FormatError::Unknown),
        }
    }
}

/// Una entrada de BibTeX.
///
/// Los autores van unidos por ` and `, que es como BibTeX separa personas: con
/// comas se leería como "Apellido, Nombre" de una sola.
fn bibtex(referencia: &Reference) -> String {
    let clase = if referencia.kind == "article" {
        "article"
    } else {
        "book"
    };
    let mut salida = format!("@{clase}{{{},\n", llave(&referencia.key));
    let mut campo = |nombre: &str, valor: &str| {
        if !valor.is_empty() {
            salida.push_str(&format!("  {nombre} = {{{}}},\n", escapar_bibtex(valor)));
        }
    };

    campo("title", &referencia.title);
    campo("author", &referencia.authors.join(" and "));
    campo("year", referencia.year.as_deref().unwrap_or_default());
    campo(
        "publisher",
        referencia.publisher.as_deref().unwrap_or_default(),
    );
    for identificador in &referencia.identifiers {
        if let Some(doi) = identificador.strip_prefix("doi:") {
            campo("doi", doi);
        }
        if let Some(isbn) = identificador.strip_prefix("isbn:") {
            campo("isbn", isbn);
        }
    }
    // La cita literal va en `annotation` y la nota propia en `note`: son cosas
    // distintas -una es del libro y otra de quien lo leyó- y mezclarlas haría
    // imposible saber quién dijo qué.
    campo(
        "annotation",
        referencia.quote.as_deref().unwrap_or_default(),
    );
    campo("note", referencia.note.as_deref().unwrap_or_default());

    salida.push_str("}\n");
    salida
}

/// Una entrada de RIS.
///
/// Cada campo en su línea, y `ER  -` cierra: el formato es de línea, así que
/// todo salto dentro de un valor se convierte en espacio. Sin eso, una nota de
/// dos párrafos rompería el archivo para quien lo importe.
fn ris(referencia: &Reference) -> String {
    let clase = if referencia.kind == "article" {
        "JOUR"
    } else {
        "BOOK"
    };
    let mut salida = format!("TY  - {clase}\n");
    let mut campo = |etiqueta: &str, valor: &str| {
        if !valor.is_empty() {
            salida.push_str(&format!("{etiqueta}  - {}\n", en_una_linea(valor)));
        }
    };

    campo("TI", &referencia.title);
    for autor in &referencia.authors {
        campo("AU", autor);
    }
    campo("PY", referencia.year.as_deref().unwrap_or_default());
    campo("PB", referencia.publisher.as_deref().unwrap_or_default());
    for identificador in &referencia.identifiers {
        if let Some(doi) = identificador.strip_prefix("doi:") {
            campo("DO", doi);
        }
        if let Some(isbn) = identificador.strip_prefix("isbn:") {
            campo("SN", isbn);
        }
    }
    campo("N1", referencia.note.as_deref().unwrap_or_default());
    campo("AB", referencia.quote.as_deref().unwrap_or_default());

    salida.push_str("ER  - \n");
    salida
}

/// Una clave de cita admisible: letras, dígitos y guiones.
///
/// BibTeX no admite espacios ni llaves en la clave, y una clave rota hace que
/// el archivo entero no compile. Lo que no vale se sustituye por un guion en
/// lugar de quitarse, para que dos claves distintas no se conviertan en la
/// misma.
fn llave(bruta: &str) -> String {
    bruta
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == ':' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Escapa lo que BibTeX interpreta: la barra y las llaves.
///
/// En **una sola pasada**, y eso importa: encadenando `replace` la sustitución
/// de la barra mete llaves que la siguiente vuelve a escapar, y sale
/// `\textbackslash\{\}` en lugar de `\textbackslash{}`. Lo destapó una prueba
/// con los dos caracteres en el mismo título.
fn escapar_bibtex(valor: &str) -> String {
    let mut salida = String::with_capacity(valor.len());
    for caracter in valor.chars() {
        match caracter {
            '\\' => salida.push_str(r"\textbackslash{}"),
            '{' => salida.push_str(r"\{"),
            '}' => salida.push_str(r"\}"),
            otro => salida.push(otro),
        }
    }
    salida
}

fn en_una_linea(valor: &str) -> String {
    valor.split_whitespace().collect::<Vec<_>>().join(" ")
}

export!(Citas);
