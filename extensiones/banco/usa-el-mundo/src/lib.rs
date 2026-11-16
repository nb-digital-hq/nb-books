//! La primera extensión compilada contra el mundo WIT.
//!
//! No es hostil: las nueve del banco intentan salirse, y esta existe para lo
//! contrario, para comprobar que el camino honesto **funciona**. Un cajón de
//! arena que solo se prueba con módulos que se portan mal no dice si uno que se
//! porta bien puede hacer su trabajo.
//!
//! El mundo se lee del mismo archivo que usa el anfitrión, sin copia: si
//! hubiera dos, llegaría el día en que dicen cosas distintas.

wit_bindgen::generate!({
    path: "../../../src-tauri/wit",
    world: "extension",
});

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

// `FetchError` y `CommandError` los trae el mundo.
use crate::nb::extension::host::fetch;

struct Extension;

/// La única orden que esta extensión aporta. Su identificador tiene que ser el
/// mismo que el de su manifiesto: si discrepan, el anfitrión pinta un botón que
/// no hace nada.
const ORDEN: &str = "usa-el-mundo.consultar";

impl Guest for Extension {
    /// Ejecuta la orden que la extensión declara en su manifiesto.
    ///
    /// **No escribe nada por la salida estándar**, y eso importa: la primera
    /// versión del mundo no devolvía nada, así que para contar el resultado
    /// había que imprimirlo, y para imprimir hace falta una capacidad. Pedir
    /// permisos para hablar del resultado es una API mal hecha.
    fn run_command(id: String) -> Result<Option<String>, CommandError> {
        if id != ORDEN {
            // El manifiesto y el código discreparían, y eso es un defecto de
            // esta extensión. Decirlo es mejor que hacer algo por si acaso.
            return Err(CommandError::Unknown);
        }
        let respuesta = fetch("https://datos.ejemplo.org/api/obra")
            .map_err(CommandError::Network)?;
        Ok(Some(format!("Respondió {}", respuesta.status)))
    }
}

export!(Extension);
