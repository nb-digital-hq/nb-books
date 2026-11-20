//! Un tercero puede comprobar una publicación con lo que ya tiene.
//!
//! Es el criterio de aceptación, y no se cumple con que nuestra
//! herramienta se dé la razón a sí misma: lo que se publica son instrucciones
//! para `sha256sum` y `openssl`, así que la prueba ejecuta **esas dos** sobre
//! lo que firma el proyecto. Si el formato de la lista, de la firma o de la
//! clave se apartara de lo que esas herramientas leen, las instrucciones
//! publicadas dejarían de funcionar y nada de aquí dentro lo notaría.
//!
//! Se salta sola si la máquina no trae alguna de las dos: no son parte de la
//! cadena de herramientas del proyecto.

use std::path::Path;
use std::process::{Command, Stdio};

use nb_books_lib::artefactos;
use nb_books_lib::extensions::signature::crear_clave;

fn hay(programa: &str) -> bool {
    Command::new(programa)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|estado| estado.success())
}

/// Las dos órdenes de las instrucciones publicadas, tal cual.
fn como_un_tercero(carpeta: &Path, clave: &Path) -> (bool, bool) {
    let firma = Command::new("openssl")
        .args(["pkeyutl", "-verify", "-pubin", "-inkey"])
        .arg(clave)
        .args([
            "-rawin",
            "-in",
            artefactos::SUMAS,
            "-sigfile",
            artefactos::FIRMA,
        ])
        .current_dir(carpeta)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("openssl se ejecuta");
    let sumas = Command::new("sha256sum")
        .args(["--check", "--strict", artefactos::SUMAS])
        .current_dir(carpeta)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("sha256sum se ejecuta");
    (firma.success(), sumas.success())
}

#[test]
fn lo_que_firma_el_proyecto_lo_comprueban_sha256sum_y_openssl() {
    if !hay("openssl") || !hay("sha256sum") {
        eprintln!("sin openssl o sin sha256sum: la prueba se salta");
        return;
    }
    let raiz = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-procedencia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&raiz);
    let carpeta = raiz.join("publicacion");
    std::fs::create_dir_all(&carpeta).expect("carpeta");
    std::fs::write(carpeta.join("nb-books_0.1.1_amd64.AppImage"), b"instalador").expect("uno");
    std::fs::write(carpeta.join("nb-books_1.0.0.cdx.json"), b"{}").expect("dos");

    let par = crear_clave().expect("clave");
    let sumas = artefactos::sumar(&carpeta).expect("sumas");
    std::fs::write(carpeta.join(artefactos::SUMAS), &sumas).expect("lista");
    std::fs::write(
        carpeta.join(artefactos::FIRMA),
        artefactos::firmar(sumas.as_bytes(), &par).expect("firma"),
    )
    .expect("archivo de firma");
    // Fuera de la carpeta, como en la realidad: la clave viene del repositorio.
    let clave = raiz.join("clave.pem");
    std::fs::write(&clave, artefactos::pem_publica(&par).expect("pem")).expect("pem escrito");

    assert_eq!(
        como_un_tercero(&carpeta, &clave),
        (true, true),
        "una publicación intacta pasa las dos órdenes publicadas"
    );

    // Un instalador cambiado: la firma de la lista sigue valiendo y es la suma
    // la que lo delata.
    std::fs::write(carpeta.join("nb-books_0.1.1_amd64.AppImage"), b"otro").expect("cambio");
    assert_eq!(como_un_tercero(&carpeta, &clave), (true, false));

    // Y con la lista rehecha para que cuadre, la que lo delata es la firma.
    std::fs::write(
        carpeta.join(artefactos::SUMAS),
        artefactos::sumar(&carpeta).expect("sumas"),
    )
    .expect("lista rehecha");
    assert_eq!(como_un_tercero(&carpeta, &clave), (false, true));

    let _ = std::fs::remove_dir_all(&raiz);
}
