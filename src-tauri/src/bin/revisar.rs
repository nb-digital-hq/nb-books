//! La puerta de revisión del registro, ejecutable sobre cualquier paquete.
//!
//! ```text
//! cargo run --features herramientas --bin revisar -- <paquete> [--de-primera-parte] [--sin-firma] [--json]
//! ```
//!
//! Va detrás de la opción `herramientas` y no se construye por omisión: sin eso,
//! `tauri build` la empaqueta como si fuera la aplicación, y así salió el
//! instalador la primera vez que se cruzó la puerta.
//!
//! # Por qué es un binario y no un comando de la aplicación
//!
//! Porque quien la usa no es quien lee: es quien publica. Meterla en la
//! interfaz habría añadido una pantalla que ningún lector va a abrir, y habría
//! atado la revisión del registro al ciclo de vida de la aplicación. Aquí
//! corre sobre un directorio cualquiera, en una máquina cualquiera, sin
//! instalar NB Books.
//!
//! # Por qué en local
//!
//! Porque las Actions de la cuenta no arrancan y ninguna ha llegado nunca a
//! empezar. Una puerta que solo existe en un CI que no se ejecuta no es un
//! mecanismo. Cuando el CI funcione, su trabajo será invocar esto.

use std::path::PathBuf;
use std::process::ExitCode;

use nb_books_lib::extensions::review::{self, Gravedad, Informe, Opciones};

fn main() -> ExitCode {
    let mut paquete: Option<PathBuf> = None;
    let mut opciones = Opciones::default();
    let mut como_json = false;

    for argumento in std::env::args().skip(1) {
        match argumento.as_str() {
            "--de-primera-parte" => opciones.de_primera_parte = true,
            "--sin-firma" => opciones.exige_firma = false,
            "--json" => como_json = true,
            "--ayuda" | "-h" | "--help" => {
                uso();
                return ExitCode::SUCCESS;
            }
            otro if otro.starts_with('-') => {
                eprintln!("No conozco la opción {otro}.");
                uso();
                return ExitCode::from(2);
            }
            otro => paquete = Some(PathBuf::from(otro)),
        }
    }

    let Some(paquete) = paquete else {
        uso();
        return ExitCode::from(2);
    };

    let informe = match review::revisar(&paquete, opciones) {
        Ok(informe) => informe,
        Err(error) => {
            eprintln!("No se pudo revisar {}: {error}", paquete.display());
            return ExitCode::from(2);
        }
    };

    if como_json {
        match serde_json::to_string_pretty(&informe) {
            Ok(texto) => println!("{texto}"),
            Err(error) => {
                eprintln!("No se pudo escribir el informe: {error}");
                return ExitCode::from(2);
            }
        }
    } else {
        contar(&informe);
    }

    if informe.pasa() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn uso() {
    eprintln!(
        "Uso: revisar <directorio del paquete> [--de-primera-parte] [--sin-firma] [--json]\n\
         \n\
         Aplica la política del registro (docs/legal/governance/extension-policy.md) a un\n\
         paquete de extensión. Devuelve 0 si se publica y 1 si no.\n\
         \n\
         --de-primera-parte  el envío viene del propio proyecto, así que puede usar el\n\
         \x20                   espacio de nombres reservado lat.nbdigitallabs.books.*\n\
         --sin-firma         revisa solo el contenido, sin exigir que vaya firmada.\n\
         \x20                   Es lo que hace su autor mientras la escribe: firmar un\n\
         \x20                   borrador en cada cambio no protege de nada\n\
         --json              el informe entero, para quien lo procese"
    );
}

/// El informe para quien lo lee, no para quien lo procesa.
fn contar(informe: &Informe) {
    let quien = match (&informe.extension, &informe.version) {
        (Some(id), Some(version)) => format!("{id} {version}"),
        _ => informe.paquete.clone(),
    };
    println!("Revisión de {quien}: {} artefacto(s).", informe.modulos);

    if informe.hallazgos.is_empty() {
        println!("Sin hallazgos: el paquete cumple lo que hoy se comprueba solo.");
        return;
    }

    println!();
    for hallazgo in &informe.hallazgos {
        let marca = match hallazgo.gravedad {
            Gravedad::Detiene => "detiene",
            Gravedad::Advierte => "advierte",
        };
        println!(
            "  [{}] {marca}  {}\n      {}",
            hallazgo.requisito, hallazgo.donde, hallazgo.que_pasa
        );
    }
    println!();

    if informe.pasa() {
        println!("Se publica: ningún hallazgo lo detiene.");
    } else {
        let requisitos: Vec<&str> = informe.requisitos().into_iter().collect();
        println!(
            "No se publica. Requisitos incumplidos: {}.\n\
             Cada uno está explicado, con su motivo, en docs/legal/governance/extension-policy.md.",
            requisitos.join(", ")
        );
    }
}
