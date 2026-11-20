//! Observabilidad local con redacción obligatoria.
//!
//! En un producto local-first no tenemos acceso a la máquina del usuario: los
//! registros son la única herramienta de diagnóstico en producción. Y a la vez,
//! lo que una persona lee es un dato íntimo.
//!
//! De esa tensión sale la regla de este módulo: **se registra el comportamiento
//! del sistema, nunca el contenido del usuario**. Ver
//! `docs/developer/coding-standards/logging.md` y la regla 3 de la
//! Constitución.

use std::path::Path;

// Solo lo usa el escritor de consola de depuración, que en release se compila
// fuera: sin esta condición, el perfil de release avisa de un import sin usar. No
// lo veía `pnpm verify` porque **la verificación nunca compila en release**, y lo
// destapó la puerta de empaquetado la primera vez que se cruzó.
#[cfg(debug_assertions)]
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Guarda que mantiene viva la escritura asíncrona a archivo.
///
/// Debe conservarse durante toda la vida del proceso: al soltarlo se vacía el
/// búfer y se deja de escribir.
pub struct TelemetryGuard(#[allow(dead_code)] tracing_appender::non_blocking::WorkerGuard);

/// Inicializa el registro estructurado.
///
/// Escribe a un archivo rotado por día en `logs_dir` y, en compilaciones de
/// depuración, también a la salida estándar.
///
/// El nivel se controla con la variable `NB_BOOKS_LOG` siguiendo la sintaxis de
/// filtros de `tracing`. Por defecto es `info`.
///
/// # Errors
///
/// No falla: si el archivo de registro no se puede abrir, la aplicación debe
/// seguir arrancando. Perder el diagnóstico es malo; impedir que el usuario lea
/// sus libros por ello es peor.
#[must_use]
pub fn init(logs_dir: &Path) -> TelemetryGuard {
    let appender = tracing_appender::rolling::daily(logs_dir, "nb-books.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);

    // `tauri::protocol::asset` registra como ERROR la ruta absoluta de cada
    // recurso que no encuentra, con el nombre de la cuenta dentro: 1.431 líneas
    // en la instalación real, contra la regla de que ningún registro lleva rutas
    // del usuario. Lo que no encuentra ya lo dice la portada que falta.
    let filter = EnvFilter::try_from_env("NB_BOOKS_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive(
            "tauri::protocol::asset=off"
                .parse()
                .expect("directiva fija"),
        )
        // `rustls` avisa en cada conexión del teléfono de que el nombre del
        // servidor es una IP, y la escribe: la dirección del PC en el registro
        // por cada petición. Es lo esperado en la red local, no un fallo.
        .add_directive(
            "rustls::msgs::handshake=error"
                .parse()
                .expect("directiva fija"),
        );

    // Todo lo que llega al archivo pasa por `Redactor`: es la segunda barrera, la
    // que no depende de que quien registra se acuerde de llamar a `redact_path`.
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(Redactor(writer))
        .with_ansi(false)
        .with_target(true)
        .with_file(false)
        .with_line_number(false);

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);

    #[cfg(debug_assertions)]
    let registry = registry.with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stdout.with_max_level(tracing::Level::DEBUG)),
    );

    // `try_init` y no `init`: un segundo arranque en el mismo proceso -el de una
    // prueba que comprueba qué ve la sesión siguiente- no puede tumbar el
    // runtime por un registro que ya está puesto. Sigue escribiendo el primero.
    let _ = registry.try_init();

    TelemetryGuard(guard)
}

/// Escritor que redacta cada línea antes de que llegue al archivo.
///
/// La regla de no registrar rutas se cumplía en lo que el código escribe a
/// propósito, pero no en el **contenido de un error**: `error = %error` imprime
/// lo que diga el error, y el de una biblioteca de terceros puede citar la ruta
/// del libro que no pudo abrir. Ningún guardián estático ve eso, porque el texto
/// no existe hasta que ocurre. Por eso la última barrera está aquí, en el punto
/// por el que pasa todo: cualquier palabra que parezca una ruta se sustituye por
/// su forma redactada, y la carpeta personal y el nombre de la cuenta también.
#[derive(Clone)]
struct Redactor<W>(W);

impl<'a, W: tracing_subscriber::fmt::MakeWriter<'a>> tracing_subscriber::fmt::MakeWriter<'a>
    for Redactor<W>
{
    type Writer = EscrituraRedactada<W::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        EscrituraRedactada(self.0.make_writer())
    }
}

struct EscrituraRedactada<W>(W);

impl<W: std::io::Write> std::io::Write for EscrituraRedactada<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let texto = String::from_utf8_lossy(buf);
        self.0.write_all(redactar_linea(&texto).as_bytes())?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

/// Sustituye en una línea de registro todo lo que pueda identificar al usuario.
///
/// Una palabra que contiene `/` o `\` se trata como ruta, salvo las direcciones
/// web, que son de servicios y no del usuario. Una ruta con espacios se parte
/// en varias palabras: cada trozo que lleve separador se redacta igual, y la
/// carpeta personal y el nombre de la cuenta se quitan antes, estén donde estén.
#[must_use]
pub fn redactar_linea(linea: &str) -> String {
    static PROPIO: std::sync::OnceLock<(Option<String>, Option<String>)> =
        std::sync::OnceLock::new();
    let (casa, cuenta) = PROPIO.get_or_init(|| {
        let casa = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .filter(|c| c.len() > 1);
        let cuenta = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok()
            .filter(|c| c.len() >= 3);
        (casa, cuenta)
    });
    let mut texto = linea.to_owned();
    if let Some(casa) = casa {
        texto = texto.replace(casa.as_str(), "~");
    }
    if let Some(cuenta) = cuenta {
        texto = texto.replace(cuenta.as_str(), "<cuenta>");
    }
    let mut salida = String::with_capacity(texto.len());
    let mut palabra = String::new();
    let volcar = |palabra: &mut String, salida: &mut String| {
        if palabra.is_empty() {
            return;
        }
        let es_web = palabra.contains("://");
        let es_ruta = (palabra.contains('/') || palabra.contains('\\')) && !es_web;
        if es_ruta {
            salida.push_str(&redact_path(Path::new(
                palabra.trim_matches(|c| c == '"' || c == '\''),
            )));
        } else {
            salida.push_str(palabra);
        }
        palabra.clear();
    };
    for c in texto.chars() {
        if c.is_whitespace() || c == '=' || c == ',' || c == '(' || c == ')' {
            volcar(&mut palabra, &mut salida);
            salida.push(c);
        } else {
            palabra.push(c);
        }
    }
    volcar(&mut palabra, &mut salida);
    salida
}

pub use nb_books_lector::telemetry::{redact_path, redact_text};

#[cfg(test)]
mod tests {
    use super::*;

    /// Si esta prueba falla, los registros pueden estar filtrando la biblioteca
    /// del usuario y su nombre de cuenta del sistema operativo.
    #[test]
    fn la_redaccion_de_rutas_no_revela_nada_del_usuario() {
        let redacted = redact_path(Path::new("/home/ana/Documentos/Secreto.pdf"));

        assert!(!redacted.contains("ana"));
        assert!(!redacted.contains("Secreto"));
        assert!(!redacted.contains("Documentos"));
        assert!(!redacted.contains('/'));
        assert!(
            redacted.contains("pdf"),
            "la extensión sí es útil: {redacted}"
        );
    }

    /// La segunda barrera: lo que el código no redactó, porque venía dentro de
    /// un error, no llega al archivo.
    #[test]
    fn una_ruta_dentro_de_un_error_no_llega_al_registro() {
        let linea = redactar_linea(
            "WARN nb_books_lib::reader: fallo error=No such file: /media/disco/Mis Libros/Ulises.epub (os 2)",
        );
        assert!(!linea.contains("/media"));
        assert!(!linea.contains("Ulises"));
        assert!(linea.contains("nb_books_lib::reader"), "{linea}");
        assert!(linea.contains("os 2"), "{linea}");
        let web = redactar_linea("consulta a https://openlibrary.org/search.json fallida");
        assert!(web.contains("https://openlibrary.org/search.json"), "{web}");
    }

    #[test]
    fn la_redaccion_de_rutas_funciona_con_rutas_de_windows() {
        let redacted = redact_path(Path::new(r"C:\Users\Ana\Libros\Ulises.epub"));

        assert!(!redacted.contains("Ana"));
        assert!(!redacted.contains("Ulises"));
        assert!(redacted.contains("epub"));
    }

    #[test]
    fn la_redaccion_de_rutas_sin_extension_no_entra_en_panico() {
        assert_eq!(
            redact_path(Path::new("/tmp/sin_extension")),
            "<ruta ext=desconocida len=18>"
        );
    }

    #[test]
    fn la_redaccion_de_texto_conserva_solo_la_longitud() {
        let redacted = redact_text("La montaña mágica");

        assert!(!redacted.contains("montaña"));
        assert_eq!(redacted, "<texto len=17>");
    }
}
