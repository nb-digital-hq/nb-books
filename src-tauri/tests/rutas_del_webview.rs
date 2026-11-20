//! Las rutas que llegan del webview se canonicalizan antes de usarse.
//!
//! `scripts/check-rutas.mjs` comprueba que cada comando pase su ruta por una puerta;
//! esto comprueba que la puerta que faltaba hace lo que dice. Antes //! `relocate_library_root` guardaba la ruta tal como llegaba: una raíz escrita con
//! `..` o con una barra de más no casaba con las rutas canónicas de sus archivos.
//!
//! Contrato: `AGENTS.md` sección 3 y `docs/audit/threat-model.md` 3.2.

mod comun;

use comun::estado::{estado_aislado, invocar};
use nb_books_lib::db::catalog::LibraryRoot;
use tauri::Manager;

#[test]
fn reapuntar_una_raiz_guarda_su_ruta_canonica() {
    let (estado, guard) = estado_aislado("rutas-reapuntar");
    let carpeta = guard.path().join("libros");
    std::fs::create_dir_all(carpeta.join("dentro")).expect("carpetas");
    let raiz = {
        let conn = estado.db.connection().expect("conexión");
        LibraryRoot::create_referenced(&conn, "Mis libros", "/donde/estaba").expect("raíz")
    };

    let app = tauri::test::mock_builder()
        .manage(estado)
        .invoke_handler(tauri::generate_handler![
            nb_books_lib::ipc::relocate_library_root
        ])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("la aplicación de prueba se construye");
    let ventana = tauri::WebviewWindowBuilder::new(&app, "principal", Default::default())
        .build()
        .expect("la ventana de prueba se crea");

    // La misma carpeta, dicha de una forma que no es la canónica.
    let rodeo = format!("{}/dentro/../", carpeta.display());
    invocar(
        &ventana,
        "relocate_library_root",
        serde_json::json!({ "rootId": raiz.id, "path": rodeo }),
    )
    .expect("se reapunta");

    let estado = app.state::<nb_books_lib::AppState>();
    let conn = estado.db.connection().expect("conexión");
    let guardada: String = conn
        .query_row(
            "SELECT path FROM library_roots WHERE id = ?1",
            [&raiz.id],
            |f| f.get(0),
        )
        .expect("raíz");
    assert_eq!(
        guardada,
        carpeta.canonicalize().expect("canónica").to_string_lossy(),
        "la raíz se guarda como la guarda la importación, no como la escribió quien llamó"
    );

    // Y una ruta que no existe no se guarda.
    let rechazo = invocar(
        &ventana,
        "relocate_library_root",
        serde_json::json!({ "rootId": raiz.id, "path": "/no/existe/nada" }),
    );
    assert!(rechazo.is_err());
}
