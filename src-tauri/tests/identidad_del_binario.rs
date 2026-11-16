//! Que "la aplicación" siga siendo el mismo ejecutable en los dos sitios que lo
//! deciden.
//!
//! El mismo día aparecieron los dos fallos que esto vigila, y ninguno lo veía
//! `cargo test`, `cargo clippy` ni `pnpm verify`:
//!
//! 1. `tauri build` ejecuta `cargo build --bins` y el empaquetador tomó **el
//!    último** binario como si fuera la aplicación: el `.deb` salió con
//!    `/usr/bin/revisar` dentro y sin NB Books. Lo arregló declarar
//!    `mainBinaryName` y dejar las herramientas detrás de una opción.
//! 2. `tauri dev` ejecuta `cargo run`, y cargo **no elige** entre varios
//!    binarios aunque dos exijan una opción apagada: aborta pidiendo `--bin`.
//!    Arrancar en desarrollo dejó de funcionar y el error no menciona la causa.
//!
//! Los dos se arreglan con una línea cada uno, en archivos distintos, y nada
//! obligaba a que esas dos líneas dijeran lo mismo. Esta prueba es lo que las
//! ata.

use std::path::Path;

/// El valor de una clave `clave = "valor"` en la sección `[package]`.
///
/// Se lee a mano en lugar de con un analizador de TOML porque añadir una
/// dependencia para leer cuatro líneas es exactamente lo que la política de
/// dependencias descarta.
fn del_paquete(manifiesto: &str, clave: &str) -> Option<String> {
    let mut en_paquete = false;
    for linea in manifiesto.lines() {
        let linea = linea.trim();
        if linea.starts_with('[') {
            en_paquete = linea == "[package]";
            continue;
        }
        if !en_paquete {
            continue;
        }
        if let Some(resto) = linea.strip_prefix(clave) {
            let resto = resto.trim_start();
            if let Some(valor) = resto.strip_prefix('=') {
                return Some(valor.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Los bloques `[[bin]]` declarados, como pares de nombre y si exigen opción.
fn binarios_declarados(manifiesto: &str) -> Vec<(String, bool)> {
    let mut salida = Vec::new();
    let mut nombre: Option<String> = None;
    let mut exige = false;
    let mut dentro = false;

    for linea in manifiesto.lines() {
        let linea = linea.trim();
        if linea.starts_with('[') {
            if let Some(previo) = nombre.take() {
                salida.push((previo, exige));
            }
            dentro = linea == "[[bin]]";
            exige = false;
            continue;
        }
        if !dentro {
            continue;
        }
        if let Some(valor) = linea.strip_prefix("name") {
            if let Some(valor) = valor.trim_start().strip_prefix('=') {
                nombre = Some(valor.trim().trim_matches('"').to_string());
            }
        }
        if linea.starts_with("required-features") {
            exige = true;
        }
    }
    if let Some(ultimo) = nombre {
        salida.push((ultimo, exige));
    }
    salida
}

fn manifiesto() -> String {
    std::fs::read_to_string(Path::new("Cargo.toml")).expect("Cargo.toml")
}

fn configuracion() -> serde_json::Value {
    let texto = std::fs::read_to_string(Path::new("tauri.conf.json")).expect("tauri.conf.json");
    serde_json::from_str(&texto).expect("json válido")
}

#[test]
fn el_binario_que_se_empaqueta_y_el_que_se_ejecuta_son_el_mismo() {
    let manifiesto = manifiesto();
    let configuracion = configuracion();

    let ejecutar = del_paquete(&manifiesto, "default-run")
        .expect("Cargo.toml declara default-run: sin él, `tauri dev` no arranca");
    let empaquetar = configuracion["mainBinaryName"]
        .as_str()
        .expect("tauri.conf.json declara mainBinaryName: sin él, el instalador toma el que sea");

    assert_eq!(
        ejecutar, empaquetar,
        "el binario que se ejecuta en desarrollo y el que se empaqueta tienen que ser el mismo"
    );
}

#[test]
fn ninguna_herramienta_entra_en_la_construccion_por_omision() {
    let manifiesto = manifiesto();
    let aplicacion = del_paquete(&manifiesto, "default-run").expect("default-run");

    for (nombre, exige_opcion) in binarios_declarados(&manifiesto) {
        if nombre == aplicacion {
            continue;
        }
        assert!(
            exige_opcion,
            "el binario `{nombre}` se construye por omisión, así que `cargo build --bins` lo mete \
             en el empaquetado; declara required-features"
        );
    }
}
