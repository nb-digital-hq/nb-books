//! Prueba de humo del binario compilado.
//!
//! Lanza el ejecutable real con un directorio de datos aislado y comprueba sus
//! efectos observables: que crea la base, que aplica las migraciones, que el
//! registro no filtra datos personales y que no emite errores en un arranque
//! limpio.
//!
//! No interactúa con el webview: eso requiere WebDriver y **todavía no existe**.
//! La suite que lo cubrirá es la deuda pendiente de la., y llega
//!, cuando haya interfaz real que conducir. Ver
//! `docs/developer/testing/regression.md`.
//!
//! La prueba se omite si no hay servidor gráfico disponible, en lugar de fallar:
//! un entorno sin pantalla no es un defecto del producto.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo() -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hay_pantalla() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// Ruta del binario, inyectada por Cargo.
///
/// `CARGO_BIN_EXE_<nombre>` **garantiza que el ejecutable se compila antes de
/// esta prueba**. Localizarlo a mano junto al ejecutable de la prueba parece
/// equivalente pero no lo es: `cargo test` no reconstruye el binario, y la
/// prueba puede acabar midiendo una compilación antigua sin que nada lo avise.
const BINARIO: &str = env!("CARGO_BIN_EXE_nb-books");

fn leer_registro(dir: &Path) -> String {
    let logs = dir.join("logs");
    let Ok(entradas) = std::fs::read_dir(&logs) else {
        return String::new();
    };

    entradas
        .filter_map(Result::ok)
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect()
}

#[test]
fn humo_arranque_completo() {
    if !hay_pantalla() {
        eprintln!("omitida: no hay servidor gráfico disponible");
        return;
    }

    let tmp = Temporal::nuevo();

    let mut proceso = Command::new(BINARIO)
        .env("NB_BOOKS_TEST_DIR", &tmp.0)
        .env("NB_BOOKS_LOG", "info")
        .spawn()
        .expect("el binario debe poder lanzarse");

    // Se le da tiempo a completar el arranque y luego se cierra: la prueba mide
    // los efectos del arranque, no la vida de la ventana.
    //
    // **Se espera a la señal de terminado, no a un indicio.** Antes bastaba con
    // que el archivo de base existiera y hubiera algo en el registro, y eso
    // dejó de correlacionar en cuanto el esquema creció: el archivo aparece con
    // la primera migración, así que la prueba mataba el proceso a mitad de
    // migrar y luego se quejaba de que la base no estaba lista. Un indicio no
    // es una señal.
    let inicio = std::time::Instant::now();
    let base = tmp.0.join("data").join("nb-books.sqlite");
    while inicio.elapsed() < std::time::Duration::from_secs(20) {
        if base.exists() && leer_registro(&tmp.0).contains("base de datos lista") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let _ = proceso.kill();
    let _ = proceso.wait();

    // --- Efectos observables del arranque ---
    assert!(base.exists(), "el arranque debe crear la base de datos");

    let registro = leer_registro(&tmp.0);
    assert!(
        registro.contains("arrancando el runtime"),
        "el arranque debe quedar registrado: {registro}"
    );
    assert!(
        registro.contains("base de datos lista"),
        "la base debe quedar lista: {registro}"
    );

    // --- Un arranque limpio no produce errores ---
    assert!(
        !registro.contains("ERROR"),
        "un arranque limpio no debe emitir errores:\n{registro}"
    );

    // --- El registro no puede filtrar datos personales ---
    let usuario = std::env::var("USER").or_else(|_| std::env::var("USERNAME"));
    if let Ok(usuario) = usuario {
        if !usuario.is_empty() {
            assert!(
                !registro.contains(&usuario),
                "el registro no puede contener el nombre de usuario del sistema"
            );
        }
    }
    assert!(
        !registro.contains(&tmp.0.to_string_lossy().to_string()),
        "el registro no puede contener rutas absolutas"
    );
}
