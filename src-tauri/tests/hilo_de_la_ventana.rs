//! `(async)` no es una etiqueta: saca al comando del hilo que atiende la ventana.
//!
//! Toda la. descansa en una afirmación sobre Tauri —que un
//! comando sin `async` corre en el hilo principal y uno con `async` no—, y una
//! afirmación sobre una dependencia no se da por buena: se comprueba contra la
//! versión que está instalada. Si una actualización de Tauri cambiara esa
//! semántica, el guardián `scripts/check-comandos.mjs` seguiría en verde
//! vigilando una palabra que ya no significaría nada, y la ventana volvería a
//! congelarse sin que nadie lo viera. Esta prueba es lo que lo impediría.
//!
//! Se declaran comandos propios, y no se usan los de `ipc.rs`, porque lo que se
//! mide es el mecanismo de Tauri, no el trabajo de ningún comando concreto.

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;

/// El hilo donde se está ejecutando, como texto comparable.
fn hilo_actual() -> String {
    format!("{:?}", std::thread::current().id())
}

#[tauri::command]
fn hilo_sincrono() -> String {
    hilo_actual()
}

#[tauri::command(async)]
fn hilo_asincrono() -> String {
    hilo_actual()
}

/// Pide un comando por el mismo camino que lo pediría el webview.
fn invocar(webview: &tauri::WebviewWindow<tauri::test::MockRuntime>, comando: &str) -> String {
    let respuesta = tauri::test::get_ipc_response(
        webview,
        InvokeRequest {
            cmd: comando.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .expect("la url del webview es válida"),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .expect("el comando responde");

    respuesta
        .deserialize::<String>()
        .expect("responde el identificador de su hilo")
}

#[test]
fn un_comando_asincrono_no_corre_en_el_hilo_que_lo_pide_y_uno_sincrono_si() {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![hilo_sincrono, hilo_asincrono])
        .build(mock_context(noop_assets()))
        .expect("la aplicación de prueba se construye");
    let webview = tauri::WebviewWindowBuilder::new(&app, "principal", Default::default())
        .build()
        .expect("la ventana de prueba se crea");

    let aqui = hilo_actual();

    // El hilo que pide es, en la aplicación real, el que atiende la ventana.
    assert_eq!(
        invocar(&webview, "hilo_sincrono"),
        aqui,
        "un comando síncrono corre en el hilo que lo pide: mientras dura, la ventana está bloqueada"
    );

    assert_ne!(
        invocar(&webview, "hilo_asincrono"),
        aqui,
        "un comando con `(async)` tiene que correr fuera del hilo de la ventana; si corre dentro, \
         la palabra dejó de significar lo que se da por supuesto"
    );
}
