//! Conducta hostil: pedirle al anfitrión una capacidad que no ofrece.
//!
//! Es el otro mecanismo de contención, y hasta que existió el anfitrión no se
//! veía que hacían falta los dos. `abre-una-conexion` importa `sock_accept`,
//! que el anfitrión **sí implementa**: se contiene porque no hay ningún socket
//! que aceptar, y el módulo recibe un error. Este importa algo que no existe en
//! ninguna parte, así que ni siquiera llega a instanciarse.
//!
//! El nombre imita al de un mundo WIT de la API de extensiones, que es de
//! donde llegan las capacidades de verdad.

#[link(wasm_import_module = "nb:extension/host")]
extern "C" {
    fn leer_documento(identificador: i32) -> i32;
}

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    let bytes = unsafe { leer_documento(1) };
    println!("leer_documento devolvió {bytes}");
}
