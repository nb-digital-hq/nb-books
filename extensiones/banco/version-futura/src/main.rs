//! Conducta hostil: declarar una versión de API que el anfitrión no soporta.
//!
//! La versión viaja en la sección personalizada `nb:api-version`, que es de
//! donde `plugin-system.md` dice que el anfitrión la lee. Noventa y nueve no
//! va a existir nunca, que es justo lo que se busca: el módulo debe ser
//! rechazado **antes de ejecutar una sola instrucción**, y con un mensaje que
//! le sirva a su autor, por el compromiso.

#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 6] = *b"99.0.0";

fn main() {
    println!("no debería llegar a ejecutarse");
}
