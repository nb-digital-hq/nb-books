//! Proveedor de formato propio: texto estructurado con secciones.
//!
//! Cumple el contrato del mundo `format-provider` de `nb-extension.wit`:
//! recibe los bytes del documento y devuelve secciones con identificador,
//! título y texto plano. No toca disco ni red.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "format-provider",
});

/// La versión de la API contra la que se compila este componente.
/// Mecanismo del requisito R11 de la política del registro.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

/// Lo más grande que esta extensión admite: ocho megabytes.
const MAXIMO: usize = 8 * 1024 * 1024;

struct FormatoTexto;

impl Guest for FormatoTexto {
    fn open_document(bytes: Vec<u8>) -> Result<Vec<Section>, OpenError> {
        if bytes.len() > MAXIMO {
            return Err(OpenError::TooLarge);
        }
        let texto = String::from_utf8(bytes).map_err(|_| OpenError::Malformed)?;

        let mut secciones: Vec<Section> = Vec::new();
        for linea in texto.lines() {
            if let Some(titulo) = linea.strip_prefix("# ") {
                secciones.push(Section {
                    href: format!("s{}", secciones.len() + 1),
                    title: titulo.trim().to_owned(),
                    text: String::new(),
                });
                continue;
            }
            let Some(actual) = secciones.last_mut() else {
                secciones.push(Section {
                    href: "s1".to_owned(),
                    title: String::new(),
                    text: linea.to_owned(),
                });
                continue;
            };
            if !actual.text.is_empty() {
                actual.text.push(' ');
            }
            actual.text.push_str(linea.trim());
        }

        secciones.retain(|s| !s.text.trim().is_empty());
        if secciones.is_empty() {
            return Err(OpenError::Malformed);
        }
        Ok(secciones)
    }
}

export!(FormatoTexto);
