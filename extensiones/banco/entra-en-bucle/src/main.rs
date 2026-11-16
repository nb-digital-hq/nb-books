//! Conducta hostil: no devolver nunca el control al anfitrión.
//!
//! Este módulo no importa nada, y no es un descuido: quemar procesador no
//! necesita al anfitrión para nada. Por eso su intención **no se puede probar
//! mirando la lista de importaciones**, y su ficha en `banco.json` lo dice en
//! lugar de fingir una evidencia que no existe. La única prueba posible es
//! ejecutarlo contra el límite de tiempo.

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    let mut contador: u64 = 0;
    loop {
        contador = contador.wrapping_add(1);
        std::hint::black_box(contador);
    }
}
