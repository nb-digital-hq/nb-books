//! Proveedor de vistas propio.
//!
//! Cumple el contrato del mundo `view-provider` de `nb-extension.wit`:
//! devuelve bloques tipados para que el núcleo pinte la vista con sus
//! propios componentes del sistema de diseño y garantías de accesibilidad.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "view-provider",
});

/// La versión de la API contra la que se compila este componente.
/// Mecanismo del requisito R11 de la política del registro.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

const VISTA_ESTUDIO: &str = "estudio.resumen";

struct VistaResumen;

impl Guest for VistaResumen {
    fn render(id: String) -> Result<View, ViewError> {
        if id == VISTA_ESTUDIO {
            Ok(View {
                title: "Resumen de Estudio".to_owned(),
                blocks: vec![
                    Block::Heading("Guía de Estudio".to_owned()),
                    Block::Paragraph(
                        "Esta vista la aporta una extensión de primera parte y la pinta el núcleo con componentes accesibles."
                            .to_owned(),
                    ),
                    Block::Items(vec![
                        "Lectura comprensiva".to_owned(),
                        "Anotaciones contextuales".to_owned(),
                        "Repaso espaciado".to_owned(),
                    ]),
                    Block::Field(("Estado".to_owned(), "Activo".to_owned())),
                ],
            })
        } else {
            Err(ViewError::Unknown)
        }
    }
}

export!(VistaResumen);
