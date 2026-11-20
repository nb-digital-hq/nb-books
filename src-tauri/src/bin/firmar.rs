//! Firma un paquete de extensión o una publicación, y crea la clave con la que
//! se firma.
//!
//! ```text
//! cargo run --features herramientas --bin firmar -- --crear-clave <archivo>
//! cargo run --features herramientas --bin firmar -- --clave-publica <archivo>
//! cargo run --features herramientas --bin firmar -- --clave-publica-pem <archivo>
//! cargo run --features herramientas --bin firmar -- <paquete> --clave <archivo>
//! cargo run --features herramientas --bin firmar -- --artefactos <carpeta> --clave <archivo>
//! ```
//!
//! Lo último escribe `SHA256SUMS` y `SHA256SUMS.sig` en la carpeta de una
//! publicación. Vive aquí y no en una herramienta nueva por lo mismo que
//! se dice abajo: es la única que toca una clave privada, y conviene que siga
//! siendo una sola.
//!
//! Va detrás de la opción `herramientas` y no se construye por omisión: sin eso,
//! `tauri build` la empaqueta como si fuera la aplicación. Ver esta función.
//!
//! # Por qué lo trae el proyecto y no se deja para el autor
//!
//! Porque un requisito que solo se puede cumplir con herramientas de fuera no
//! es un requisito, es un obstáculo. El registro exige la firma, así que
//! firmar tiene que costar una orden.
//!
//! # Por qué es un binario aparte de `revisar`
//!
//! Porque hacen cosas distintas con permisos distintos: revisar solo lee, y
//! esto toca una clave privada. Juntarlos habría puesto la clave del autor al
//! alcance de una herramienta que no la necesita para nada.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nb_books_lib::artefactos;
use nb_books_lib::extensions::{manifest, signature};

fn main() -> ExitCode {
    let mut paquete: Option<PathBuf> = None;
    let mut clave: Option<PathBuf> = None;
    let mut crear: Option<PathBuf> = None;
    let mut publica: Option<PathBuf> = None;
    let mut publica_pem: Option<PathBuf> = None;
    let mut publicacion: Option<PathBuf> = None;

    let mut argumentos = std::env::args().skip(1);
    while let Some(argumento) = argumentos.next() {
        match argumento.as_str() {
            "--crear-clave" => crear = argumentos.next().map(PathBuf::from),
            "--clave-publica" => publica = argumentos.next().map(PathBuf::from),
            "--clave-publica-pem" => publica_pem = argumentos.next().map(PathBuf::from),
            "--artefactos" => publicacion = argumentos.next().map(PathBuf::from),
            "--clave" => clave = argumentos.next().map(PathBuf::from),
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

    if let Some(destino) = crear {
        return crear_clave(&destino);
    }
    if let Some(origen) = publica {
        return decir_publica(&origen);
    }
    if let Some(origen) = publica_pem {
        return decir_publica_pem(&origen);
    }
    if let (Some(carpeta), Some(clave)) = (&publicacion, &clave) {
        return firmar_artefactos(carpeta, clave);
    }
    match (paquete, clave) {
        (Some(paquete), Some(clave)) => firmar(&paquete, &clave),
        _ => {
            uso();
            ExitCode::from(2)
        }
    }
}

fn uso() {
    eprintln!(
        "Uso:\n\
         \x20 firmar --crear-clave <archivo>        un par de claves nuevo\n\
         \x20 firmar --clave-publica <archivo>      la clave pública de ese par\n\
         \x20 firmar --clave-publica-pem <archivo>  la misma, en PEM, para openssl\n\
         \x20 firmar <paquete> --clave <archivo>    firma el paquete\n\
         \x20 firmar --artefactos <carpeta> --clave <archivo>\n\
         \x20                                       suma y firma una publicación\n\
         \n\
         La firma cubre la huella del paquete entero, más su identificador y su\n\
         versión, y se escribe en firma.json dentro del paquete. Vuelve a firmar\n\
         cada vez que cambies algo: la firma deja de valer al primer byte."
    );
}

fn crear_clave(destino: &Path) -> ExitCode {
    if destino.exists() {
        eprintln!(
            "{} ya existe. No se sobrescribe una clave: si la pierdes, pierdes\n\
             la identidad de todas tus extensiones publicadas.",
            destino.display()
        );
        return ExitCode::FAILURE;
    }
    let clave = match signature::crear_clave() {
        Ok(clave) => clave,
        Err(error) => {
            eprintln!("No se pudo generar la clave: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = std::fs::write(destino, &clave) {
        eprintln!("No se pudo escribir {}: {error}", destino.display());
        return ExitCode::FAILURE;
    }
    proteger(destino);

    match signature::clave_publica(&clave) {
        Ok(publica) => {
            println!("Clave creada en {}", destino.display());
            println!("Clave pública: {publica}");
            println!(
                "\nGuarda el archivo como guardarías una llave. Quien lo tenga puede\n\
                 publicar actualizaciones de tus extensiones, y quien lo pierda no\n\
                 puede volver a publicarlas."
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("La clave se escribió y no se pudo leer: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Deja la clave privada legible solo para su dueño.
///
/// Solo en Unix, y sin condicionar el resto: en Windows los permisos son otra
/// cosa y el archivo lo protege el perfil del usuario. Escribir una clave
/// privada legible para todo el mundo sería un defecto aunque el programa
/// funcionara.
#[cfg(unix)]
fn proteger(ruta: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn proteger(_ruta: &Path) {}

fn decir_publica(origen: &Path) -> ExitCode {
    let Ok(bytes) = std::fs::read(origen) else {
        eprintln!("No se pudo leer {}.", origen.display());
        return ExitCode::FAILURE;
    };
    match signature::clave_publica(&bytes) {
        Ok(publica) => {
            println!("{publica}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "{} no es un par de claves de Ed25519: {error}",
                origen.display()
            );
            ExitCode::FAILURE
        }
    }
}

fn firmar(paquete: &Path, clave: &Path) -> ExitCode {
    let Ok(texto) = std::fs::read_to_string(paquete.join(manifest::ARCHIVO)) else {
        eprintln!(
            "{} no trae {}: sin manifiesto no hay qué firmar.",
            paquete.display(),
            manifest::ARCHIVO
        );
        return ExitCode::FAILURE;
    };
    let manifiesto = match manifest::parse(&texto) {
        Ok(manifiesto) => manifiesto,
        Err(defectos) => {
            eprintln!("El manifiesto no vale, así que no se firma:");
            for defecto in defectos {
                eprintln!("  {} {:?}", defecto.field, defecto.code);
            }
            return ExitCode::FAILURE;
        }
    };
    let Ok(bytes) = std::fs::read(clave) else {
        eprintln!("No se pudo leer {}.", clave.display());
        return ExitCode::FAILURE;
    };

    let firma = match signature::firmar(paquete, &manifiesto.id, &manifiesto.version, &bytes) {
        Ok(firma) => firma,
        Err(error) => {
            eprintln!("No se pudo firmar: {error}");
            return ExitCode::FAILURE;
        }
    };
    let destino = paquete.join(signature::ARCHIVO);
    let Ok(json) = serde_json::to_string_pretty(&firma) else {
        eprintln!("No se pudo escribir la firma.");
        return ExitCode::FAILURE;
    };
    if let Err(error) = std::fs::write(&destino, json) {
        eprintln!("No se pudo escribir {}: {error}", destino.display());
        return ExitCode::FAILURE;
    }

    println!(
        "Firmado {} {} con la clave {}.",
        manifiesto.id, manifiesto.version, firma.clave
    );
    println!("La firma está en {}.", destino.display());
    ExitCode::SUCCESS
}

fn decir_publica_pem(origen: &Path) -> ExitCode {
    let Ok(bytes) = std::fs::read(origen) else {
        eprintln!("No se pudo leer {}.", origen.display());
        return ExitCode::FAILURE;
    };
    match artefactos::pem_publica(&bytes) {
        Ok(pem) => {
            print!("{pem}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "{} no es un par de claves de Ed25519: {error}",
                origen.display()
            );
            ExitCode::FAILURE
        }
    }
}

/// Suma todo lo que hay en la carpeta y firma la lista.
///
/// La lista se rehace entera cada vez: una lista vieja con un archivo nuevo al
/// lado es justo el caso que la comprobación da por malo, y firmarla tal cual
/// sería publicar algo que no se sostiene.
fn firmar_artefactos(carpeta: &Path, clave: &Path) -> ExitCode {
    let Ok(bytes) = std::fs::read(clave) else {
        eprintln!("No se pudo leer {}.", clave.display());
        return ExitCode::FAILURE;
    };
    let sumas = match artefactos::sumar(carpeta) {
        Ok(sumas) if !sumas.is_empty() => sumas,
        Ok(_) => {
            eprintln!("{} está vacía: no hay qué firmar.", carpeta.display());
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!(
                "No se pudo sumar {}: {error}. Una publicación es una carpeta\n\
                 plana de archivos, sin subcarpetas.",
                carpeta.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let firma = match artefactos::firmar(sumas.as_bytes(), &bytes) {
        Ok(firma) => firma,
        Err(error) => {
            eprintln!("No se pudo firmar: {error}");
            return ExitCode::FAILURE;
        }
    };
    for (nombre, contenido) in [
        (artefactos::SUMAS, sumas.as_bytes()),
        (artefactos::FIRMA, firma.as_slice()),
    ] {
        if let Err(error) = std::fs::write(carpeta.join(nombre), contenido) {
            eprintln!("No se pudo escribir {nombre}: {error}");
            return ExitCode::FAILURE;
        }
    }
    println!(
        "Firmados {} archivos de {}.",
        sumas.lines().count(),
        carpeta.display()
    );
    ExitCode::SUCCESS
}
