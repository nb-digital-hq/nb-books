//! La primera versión de una extensión que más tarde se vuelve maliciosa.
//!
//! Aquí no hay nada que objetar, y ese es exactamente el punto: la revisión
//! automatizada **tiene que dejarla pasar**. Una revisión que
//! rechaza también lo bueno no demuestra nada.

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    let suma: u32 = (1..=10).sum();
    println!("suma = {suma}");
}
