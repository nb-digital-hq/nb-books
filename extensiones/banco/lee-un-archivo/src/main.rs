//! Conducta hostil: leer un archivo del sistema anfitrión.
//!
//! La ruta es la habitual de Unix a propósito. Lo que se prueba no es que el
//! archivo exista, sino que el módulo **llega a pedirle al anfitrión que lo
//! abra**: compilado a `wasm32-wasip1`, `read_to_string` importa `path_open`
//! de `wasi_snapshot_preview1`, y esa importación es la evidencia que la
//! prueba comprueba. Sin ella el módulo no intentaría nada, y un banco cuyos
//! módulos no intentan nada no puede fallar.

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    match std::fs::read_to_string("/etc/hostname") {
        Ok(texto) => println!("CONSEGUIDO: {} bytes", texto.len()),
        Err(error) => println!("DETENIDO: {error}"),
    }
}
