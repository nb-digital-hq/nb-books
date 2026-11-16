//! La segunda versión de la misma extensión, ya con carga remota de código.
//!
//! Es el vector clásico y el motivo por el que la revisión se hace
//! en **cada** versión publicada y no solo en el primer envío. Para que la
//! revisión pueda detenerla, el patrón tiene que estar aquí de verdad: una
//! dirección de la que traer código y una función que lo ejecuta.
//!
//! El dominio es `.invalid`, reservado por el RFC 2606 justamente para esto:
//! no resuelve en ninguna parte, así que ni por accidente sale nada.

const ORIGEN_DEL_CODIGO: &str = "https://ejemplo.invalid/carga-util.wasm";

#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn sock_accept(descriptor: i32, banderas: u32, salida: *mut i32) -> i32;
}

fn descargar_modulo() -> Vec<u8> {
    let mut aceptado: i32 = 0;
    unsafe { sock_accept(0, 0, &mut aceptado) };
    ORIGEN_DEL_CODIGO.as_bytes().to_vec()
}

fn ejecutar_modulo_descargado(bytes: &[u8]) {
    println!("ejecutando {} bytes traídos de la red", bytes.len());
}

/// La versión de la API contra la que se compila, en la sección personalizada
/// de la que el anfitrión la lee. Sin ella el módulo no se ejecuta: es lo que
/// la política del registro le pide al autor a cambio de sus garantías.
#[used]
#[link_section = "nb:api-version"]
static VERSION_DE_API: [u8; 5] = *b"1.0.0";

fn main() {
    let bytes = descargar_modulo();
    ejecutar_modulo_descargado(&bytes);
}
