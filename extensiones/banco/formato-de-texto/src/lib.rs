//! Un formato nuevo aportado por una extensión.
//!
//! Texto plano con encabezados: una línea que empieza por `# ` abre una sección
//! y da su título; lo que sigue es su texto. Es deliberadamente el formato más
//! simple que sigue teniendo estructura, porque lo que se demuestra aquí **no
//! es el formato**: es que añadir uno no exige tocar el núcleo.
//!
//! Esta extensión no ve el disco, no sabe dónde vive el archivo y no puede
//! salir a la red: su mundo no importa nada del anfitrión. Recibe bytes y
//! devuelve secciones.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "format-provider",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

/// Lo más grande que esta extensión acepta.
///
/// Un límite propio, además del que impone el anfitrión: quien escribe una
/// extensión sabe qué tamaños tienen sentido en su formato, y decirlo produce
/// un motivo mejor que quedarse sin memoria.
const MAXIMO: usize = 8 * 1024 * 1024;

struct Formato;

impl Guest for Formato {
    fn open_document(bytes: Vec<u8>) -> Result<Vec<Section>, OpenError> {
        if bytes.len() > MAXIMO {
            return Err(OpenError::TooLarge);
        }
        // Texto que no es UTF-8 no es este formato. Se dice, en lugar de
        // sustituir los bytes malos por interrogaciones y entregar algo que
        // parece un documento y no lo es.
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
                // Texto antes del primer encabezado: abre una sección sin
                // título en lugar de perderse.
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

export!(Formato);
