//! Conducta hostil: abrir una conexión de red.
//!
//! No usa `std::net`, y el motivo importa. Bajo `wasm32-wasip1` la biblioteca
//! estándar devuelve "no soportado" **sin llamar al anfitrión**, así que un
//! módulo escrito con `TcpStream` no importaría ni una función del anfitrión:
//! sería exactamente el simulador que no puede fallar. Medido el 2026-09-07
//! sobre un módulo real, su única importación era `fd_write`, la de imprimir.
//!
//! Un módulo hostil de verdad declara la importación cruda, y eso es lo que
//! hace este.

#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn sock_accept(descriptor: i32, banderas: u32, salida: *mut i32) -> i32;
}

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    let mut aceptado: i32 = 0;
    let codigo = unsafe { sock_accept(0, 0, &mut aceptado) };
    println!("sock_accept devolvió {codigo}");
}
