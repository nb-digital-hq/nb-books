//! Comprueba una publicación de NB Books contra la clave del proyecto.
//!
//! ```text
//! cargo run --features herramientas --bin comprobar_artefactos -- <carpeta> --clave <clave.pem>
//! ```
//!
//! Devuelve 0 si todo lo que hay en la carpeta está firmado y es lo que se
//! firmó, y 1 si no, diciendo qué. Solo lee.
//!
//! # Por qué existe, si basta con `sha256sum` y `openssl`
//!
//! Porque esas dos comprueban que lo listado es lo firmado, y ninguna mira lo
//! que **no** está en la lista: un instalador de más en la carpeta pasa las dos
//! sin que nadie lo haya firmado. Las instrucciones publicadas siguen siendo las
//! de las herramientas del sistema -nadie tiene que compilar esto para fiarse-;
//! esta es la que usa el propio proyecto antes de publicar.
//!
//! # Por qué es un binario aparte de `firmar`
//!
//! Por lo mismo que `revisar`: esto solo lee, y `firmar` toca una clave privada.

use std::path::PathBuf;
use std::process::ExitCode;

use nb_books_lib::artefactos;

fn main() -> ExitCode {
    let mut carpeta: Option<PathBuf> = None;
    let mut clave: Option<PathBuf> = None;

    let mut argumentos = std::env::args().skip(1);
    while let Some(argumento) = argumentos.next() {
        match argumento.as_str() {
            "--clave" => clave = argumentos.next().map(PathBuf::from),
            otro if otro.starts_with('-') => {
                eprintln!("No conozco la opción {otro}.");
                return ExitCode::from(2);
            }
            otro => carpeta = Some(PathBuf::from(otro)),
        }
    }
    let (Some(carpeta), Some(clave)) = (carpeta, clave) else {
        eprintln!("Uso: comprobar_artefactos <carpeta> --clave <clave.pem>");
        return ExitCode::from(2);
    };

    let Ok(pem) = std::fs::read_to_string(&clave) else {
        eprintln!("No se pudo leer {}.", clave.display());
        return ExitCode::FAILURE;
    };
    match artefactos::comprobar(&carpeta, &pem) {
        Ok(defectos) if defectos.is_empty() => {
            println!("{}: todo firmado y sin cambios.", carpeta.display());
            ExitCode::SUCCESS
        }
        Ok(defectos) => {
            eprintln!("{} no se puede dar por buena:", carpeta.display());
            for defecto in defectos {
                eprintln!("  {defecto}");
            }
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("No se pudo leer {}: {error}", carpeta.display());
            ExitCode::FAILURE
        }
    }
}
