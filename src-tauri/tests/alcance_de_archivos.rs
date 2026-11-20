//! Lo que la ventana lee directamente del disco está dentro de su alcance, y la
//! base no.
//!
//! # El defecto que esto fija
//!
//! La ventana lee algunos archivos por el protocolo de archivos de Tauri
//! (`convertFileSrc`), que solo sirve lo que cae dentro de
//! `app.security.assetProtocol.scope`. Antes ese alcance era **la
//! caché y nada más**, y el audio preparado vive en la carpeta
//! de **datos** -costó horas de procesador, y una caché es lo que el sistema vacía
//! sin avisar-. En una instalación real, escuchar un capítulo o un libro
//! preparado, o su muestra, habría fallado sin remedio.
//!
//! Ninguna prueba podía verlo: el simulador de los recorridos no pasa por el
//! protocolo de archivos, y `cargo test` no arranca la ventana. Lo encontró la
//! puerta de empaquetado, revisando qué viaja y qué se puede leer.
//!
//! # Lo que exige, y lo que no permite
//!
//! * Cada carpeta que el runtime entrega a la ventana para leer directamente
//!   está cubierta por el alcance.
//! * **La base de datos no lo está.** El alcance se abre a la carpeta del audio y
//!   no a la de datos entera: dar a la ventana lectura de la base sería regalarle
//!   la biblioteca entera a cualquier cosa que consiga ejecutar en ella.
//!
//! Solo en Linux, que es donde las carpetas se resuelven con XDG y se pueden
//! desviar a un directorio temporal sin tocar las del usuario.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};

fn alcance() -> Vec<String> {
    let texto =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json"))
            .expect("tauri.conf.json");
    let conf: serde_json::Value = serde_json::from_str(&texto).expect("json");
    conf["app"]["security"]["assetProtocol"]["scope"]
        .as_array()
        .expect("el alcance es una lista")
        .iter()
        .map(|v| v.as_str().expect("texto").to_owned())
        .collect()
}

/// Si `ruta` cae dentro de algún patrón del alcance, con `$DATA` y `$CACHE`
/// resueltos como los resuelve Tauri en Linux: las bases XDG, sin la carpeta de la
/// aplicación, que el patrón ya nombra.
fn cubierta(ruta: &Path, datos: &Path, cache: &Path, alcance: &[String]) -> bool {
    alcance.iter().any(|patron| {
        let resuelto = patron
            .replace("$DATA", &datos.to_string_lossy())
            .replace("$CACHE", &cache.to_string_lossy());
        let Some(raiz) = resuelto.strip_suffix("/**") else {
            return false;
        };
        ruta.starts_with(raiz)
    })
}

#[test]
fn la_ventana_puede_leer_el_audio_y_la_cache_y_no_la_base() {
    let base = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-alcance-{}", std::process::id()));
    let datos = base.join("share");
    let cache = base.join("cache");

    // SAFETY: este binario de prueba tiene una sola prueba, así que nadie más lee
    // estas variables mientras se escriben.
    unsafe {
        std::env::remove_var(nb_books_lib::paths::TEST_DIR_ENV);
        std::env::set_var("XDG_DATA_HOME", &datos);
        std::env::set_var("XDG_CACHE_HOME", &cache);
        std::env::set_var("XDG_CONFIG_HOME", base.join("config"));
    }
    let rutas = nb_books_lib::paths::AppPaths::resolve().expect("rutas");
    let alcance = alcance();

    let leidas: [(&str, PathBuf); 2] = [
        ("audio preparado", rutas.audio().join("libro.opus")),
        ("caché", rutas.cache.join("voz-neuronal").join("tramo.opus")),
    ];
    for (que, ruta) in &leidas {
        assert!(
            cubierta(ruta, &datos, &cache, &alcance),
            "la ventana no puede leer el {que}: {} queda fuera de {alcance:?}",
            ruta.display()
        );
    }

    assert!(
        !cubierta(&rutas.database(), &datos, &cache, &alcance),
        "el alcance deja a la ventana leer la base de datos entera"
    );

    let _ = std::fs::remove_dir_all(&base);
}
