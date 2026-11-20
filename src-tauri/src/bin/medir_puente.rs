//! La latencia del puente, medida en el motor y con el IPC de verdad.
//!
//! ```text
//! pnpm build
//! cargo build --release --features herramientas,tauri/custom-protocol --bin medir_puente
//! target/release/medir_puente
//! ```
//!
//! # Por qué es un binario aparte
//!
//! El presupuesto de la sección 1 de `performance-budgets.md` dice "envío y
//! respuesta de JSON vacío, con `tauri::command` puro", y llevaba sin medirse desde
//! el producto. Medirlo en la suite no sirve: la suite corre en Chromium con el puente
//! simulado, y lo que se pregunta es lo que cuesta cruzar **el IPC de Tauri sobre
//! WebKitGTK**. Tampoco se mete en la aplicación: una orden que solo existe para
//! medir sería superficie del puente que ninguna pantalla usa, que es justo lo que
//! `check-puente` prohíbe.
//!
//! Así que esto monta la misma versión de Tauri, con la misma configuración y la
//! misma interfaz compilada, y registra solo dos órdenes: una que no hace nada y otra
//! que recibe el informe. Cuando la página termina de cargar, espera a que la
//! interfaz se calme -arranca y pide sus órdenes, que aquí no existen y fallan sin
//! más-, calienta con cincuenta viajes y mide mil, uno detrás de otro, con la orden
//! vacía; y quinientos con un kilobyte de ida y vuelta. Imprime el informe y sale.
//!
//! Va detrás de `herramientas`, como `revisar` y `firmar`: sin eso, `tauri build`
//! la empaquetaría como si fuera la aplicación.

use tauri::webview::PageLoadEvent;
use tauri::Manager;

/// Espera antes de medir, para que la interfaz haya dejado de arrancar.
const CALMA_MS: u32 = 4000;

const SONDA: &str = r"
(async () => {
  const invoke = window.__TAURI_INTERNALS__.invoke;
  await new Promise((r) => setTimeout(r, CALMA));
  for (let i = 0; i < 50; i += 1) await invoke('vacio');
  // Cada viaje suelto sale redondeado: WebKit da `performance.now()` al milisegundo.
  // La media sale del total de mil seguidos, que ese redondeo no afecta.
  const inicio = performance.now();
  for (let i = 0; i < 1000; i += 1) await invoke('vacio');
  const media = (performance.now() - inicio) / 1000;
  const viajes = [];
  for (let i = 0; i < 1000; i += 1) {
    const t0 = performance.now();
    await invoke('vacio');
    viajes.push(performance.now() - t0);
  }
  viajes.sort((a, b) => a - b);
  const carga = { texto: 'x'.repeat(1024) };
  const inicioConDatos = performance.now();
  for (let i = 0; i < 500; i += 1) await invoke('eco', { carga });
  const mediaConDatos = (performance.now() - inicioConDatos) / 500;
  const q = (xs, p) => Math.round(xs[Math.floor(xs.length * p)] * 1000) / 1000;
  await invoke('informe', {
    datos: {
      viajes: viajes.length,
      media: Math.round(media * 1000) / 1000,
      mediana: q(viajes, 0.5),
      p95: q(viajes, 0.95),
      p99: q(viajes, 0.99),
      maximo: q(viajes, 0.999),
      mediaConUnKiloDeIdaYVuelta: Math.round(mediaConDatos * 1000) / 1000,
      motor: navigator.userAgent
    }
  });
})();
";

#[tauri::command(async)]
fn vacio() {}

#[tauri::command(async)]
fn eco(carga: serde_json::Value) -> serde_json::Value {
    carga
}

#[tauri::command(async)]
fn informe(app: tauri::AppHandle, datos: serde_json::Value) {
    println!("{datos}");
    app.exit(0);
}

fn main() {
    let sonda = SONDA.replace("CALMA", &CALMA_MS.to_string());
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![vacio, eco, informe])
        .on_page_load(move |webview, carga| {
            if carga.event() == PageLoadEvent::Finished {
                let _ = webview.eval(&sonda);
            }
        })
        .setup(|app| {
            // Si la sonda no contesta, algo falló sin avisar: se sale en lugar de
            // dejar una ventana colgada.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(60));
                eprintln!("la sonda no contestó en un minuto");
                handle.exit(1);
            });
            let _ = app.get_webview_window("main");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("el runtime de Tauri debe poder arrancar");
}
