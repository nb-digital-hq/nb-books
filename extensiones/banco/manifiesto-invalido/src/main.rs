//! Conducta hostil: un manifiesto que no cumple su forma.
//!
//! El código es inofensivo a propósito. Lo que se prueba es que el anfitrión
//! rechaza el paquete **por el manifiesto y sin llegar a mirar el módulo**, y
//! que al rechazarlo señala el campo concreto, que es el criterio.

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    println!("inofensivo");
}
