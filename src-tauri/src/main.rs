// Impide que se abra una consola además de la ventana en Windows en modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    usar_la_edicion_rapida_si_cabe();
    preferir_wayland();
    nb_books_lib::run();
}

/// Nombre del binario de la edición rápida, al lado de este en el paquete.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const EDICION_RAPIDA: &str = "nb-books-avx2";

/// Si el procesador tiene AVX2 y el paquete trae la edición rápida, se pasa a ella
///.
///
/// La IA local va diez veces más rápida compilada con AVX2, pero esa compilación
/// no arranca en un procesador sin ella, y por eso había dos descargas y la
/// aplicación pedía "la edición rápida" a quien tenía la otra: un resumen tardaba
/// veinte minutos en un equipo que podía hacerlo en dos. Ahora el paquete lleva
/// las dos y elige al arrancar. `NB_EDICION=universal` se queda en esta.
///
/// `exec` sustituye el proceso, así que no hay dos aplicaciones vivas, y la marca
/// del entorno impide volver a saltar si la rápida es este mismo binario.
fn usar_la_edicion_rapida_si_cabe() {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        #[cfg(unix)]
        use std::os::unix::process::CommandExt;
        const MARCA: &str = "NB_BOOKS_EDICION_ELEGIDA";
        if std::env::var_os(MARCA).is_some()
            || std::env::var("NB_EDICION").is_ok_and(|v| v == "universal")
            || cfg!(target_feature = "avx2")
        {
            return;
        }
        let cabe = [
            "avx", "avx2", "fma", "bmi1", "bmi2", "f16c", "movbe", "xsave",
        ]
        .iter()
        .all(|f| match *f {
            "avx" => std::arch::is_x86_feature_detected!("avx"),
            "avx2" => std::arch::is_x86_feature_detected!("avx2"),
            "fma" => std::arch::is_x86_feature_detected!("fma"),
            "bmi1" => std::arch::is_x86_feature_detected!("bmi1"),
            "bmi2" => std::arch::is_x86_feature_detected!("bmi2"),
            "f16c" => std::arch::is_x86_feature_detected!("f16c"),
            "movbe" => std::arch::is_x86_feature_detected!("movbe"),
            _ => std::arch::is_x86_feature_detected!("xsave"),
        });
        if !cabe {
            return;
        }
        let Some(rapida) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.join(EDICION_RAPIDA)))
            .filter(|r| r.is_file())
        else {
            return;
        };
        // Si `exec` vuelve es que falló: se sigue con esta edición, que funciona.
        let _ = std::process::Command::new(rapida)
            .args(std::env::args_os().skip(1))
            .env(MARCA, "1")
            .exec();
    }
}

/// En un escritorio Wayland, se pinta con Wayland aunque el AppImage diga X11.
///
/// El gancho GTK de linuxdeploy exporta `GDK_BACKEND=x11` sin condición, por un
/// fallo antiguo de Tauri con Wayland. Por XWayland la ventana ignora la escala
/// del escritorio: en una pantalla al 125 % todo salía más pequeño que el resto,
/// y maximizada medía 1920 píxeles en una pantalla de 1536 lógicos. Comprobado el
/// 2026-11-12 en KDE Plasma: con Wayland se pinta nítida y a su escala.
///
/// Solo se cambia lo que puso el AppImage (`APPDIR` presente), nunca lo que el
/// usuario eligió; y `NB_BOOKS_X11=1` vuelve a X11 si algún equipo lo necesita.
/// Va aquí, antes de cualquier hilo, porque cambiar el entorno con otros hilos
/// vivos no es seguro.
fn preferir_wayland() {
    #[cfg(target_os = "linux")]
    {
        let del_appimage = std::env::var_os("APPDIR").is_some();
        let en_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let forzado_x11 = std::env::var("GDK_BACKEND").is_ok_and(|v| v == "x11");
        let pide_x11 = std::env::var("NB_BOOKS_X11").is_ok_and(|v| v == "1");
        if del_appimage && en_wayland && forzado_x11 && !pide_x11 {
            std::env::set_var("GDK_BACKEND", "wayland");
        }
    }
}
