//! Una vista aportada por una extensión.
//!
//! Devuelve bloques tipados y no marcado: la interfaz la pinta el núcleo. Usa
//! las cuatro clases que existen, para que la prueba compruebe que las cuatro
//! llegan y se pintan, y no solo la primera.
//!
//! Y aporta una segunda vista que **entra en bucle**, que es lo que hace
//! comprobable el criterio: una extensión que no devuelve el
//! control no congela la aplicación, porque no corre en su hilo.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "view-provider",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

/// La vista que se porta bien.
const NORMAL: &str = "vista-de-estudio.resumen";
/// La que no devuelve el control. Existe para poder comprobar que da igual.
const COLGADA: &str = "vista-de-estudio.colgada";

struct Vista;

impl Guest for Vista {
    fn render(id: String) -> Result<View, ViewError> {
        match id.as_str() {
            NORMAL => Ok(View {
                title: "Resumen de prueba".to_owned(),
                blocks: vec![
                    Block::Heading("Lo que hay".to_owned()),
                    Block::Paragraph(
                        "Esta vista la compone una extensión y la pinta el núcleo.".to_owned(),
                    ),
                    Block::Items(vec!["Primero".to_owned(), "Segundo".to_owned()]),
                    Block::Field(("Origen".to_owned(), "Extensión de prueba".to_owned())),
                ],
            }),
            COLGADA => loop {
                std::hint::black_box(0_u64);
            },
            // El manifiesto y el código discreparían.
            _ => Err(ViewError::Unknown),
        }
    }
}

export!(Vista);
