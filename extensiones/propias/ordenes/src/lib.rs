//! Proveedor de órdenes propio.
//!
//! Cumple el contrato del mundo `extension` de `nb-extension.wit`:
//! exporta `run-command` para ejecutar órdenes declaradas en el manifiesto.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "extension",
});

/// La versión de la API contra la que se compila este componente.
/// Mecanismo del requisito R11 de la política del registro.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

const ORDEN_ESTIMAR: &str = "ordenes.estimar";

struct ExtensionOrdenes;

impl Guest for ExtensionOrdenes {
    fn run_command(id: String) -> Result<Option<String>, CommandError> {
        if id == ORDEN_ESTIMAR {
            Ok(Some(
                "Ritmo medio de lectura estimado: 200 a 250 palabras por minuto.".to_owned(),
            ))
        } else {
            Err(CommandError::Unknown)
        }
    }
}

export!(ExtensionOrdenes);
