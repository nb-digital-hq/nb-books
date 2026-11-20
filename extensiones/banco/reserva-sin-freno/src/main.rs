//! Conducta hostil: pedir memoria hasta agotar la que haya.
//!
//! Como el del bucle, no importa nada del anfitrión: `memory.grow` es una
//! instrucción de WebAssembly, no una llamada. Se prueba ejecutándolo contra
//! el límite de memoria por instancia.

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    let mut trozos: Vec<Vec<u8>> = Vec::new();
    loop {
        trozos.push(vec![0_u8; 1 << 20]);
        std::hint::black_box(trozos.len());
    }
}
