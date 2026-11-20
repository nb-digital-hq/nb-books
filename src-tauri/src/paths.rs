//! Resolución de rutas por plataforma.
//!
//! Los datos del usuario y la caché regenerable viven en directorios distintos:
//! borrar la caché nunca puede perder información. Ver
//! `docs/developer/deployment/README.md`.
//!
//! La variable de entorno `NB_BOOKS_TEST_DIR` desvía **todas** las rutas a un
//! directorio temporal, para que las pruebas de extremo a extremo no toquen la
//! biblioteca real de quien las ejecuta.

use std::path::PathBuf;

use crate::error::{NbError, NbResult};

/// Nombre de la variable que aísla el entorno durante las pruebas.
pub const TEST_DIR_ENV: &str = "NB_BOOKS_TEST_DIR";

/// Directorios que el runtime necesita.
#[derive(Debug, Clone)]
pub struct AppPaths {
    /// Datos del usuario: base de datos y biblioteca administrada. Se respalda.
    pub data: PathBuf,
    /// Configuración. Se respalda.
    pub config: PathBuf,
    /// Caché regenerable: portadas, páginas renderizadas, índices. **No** se respalda.
    pub cache: PathBuf,
    /// Registros de diagnóstico.
    pub logs: PathBuf,
}

impl AppPaths {
    /// Resuelve los directorios de la aplicación y los crea si no existen.
    ///
    /// # Errors
    ///
    /// Devuelve [`NbError::StorageFailure`] si el sistema operativo no expone un
    /// directorio de datos o si no se pueden crear los directorios.
    pub fn resolve() -> NbResult<Self> {
        if let Some(base) = std::env::var_os(TEST_DIR_ENV) {
            let base = PathBuf::from(base);
            return Self {
                data: base.join("data"),
                config: base.join("config"),
                cache: base.join("cache"),
                logs: base.join("logs"),
            }
            .ensure_created();
        }

        let data = dirs_data().ok_or(NbError::StorageFailure)?;
        let config = dirs_config().ok_or(NbError::StorageFailure)?;
        let cache = dirs_cache().ok_or(NbError::StorageFailure)?;

        Self {
            logs: data.join("logs"),
            data,
            config,
            cache,
        }
        .ensure_created()
    }

    fn ensure_created(self) -> NbResult<Self> {
        for dir in [&self.data, &self.config, &self.cache, &self.logs] {
            std::fs::create_dir_all(dir).map_err(|_| NbError::StorageFailure)?;
        }
        Ok(self)
    }

    /// Archivo de la base de datos principal.
    pub fn database(&self) -> PathBuf {
        self.data.join("nb-books.sqlite")
    }

    /// Carpeta de la biblioteca administrada, donde viven las copias de los
    /// documentos que el usuario ha pedido que gestionemos nosotros.
    ///
    /// Los documentos en modo referenciado **no** están aquí: se quedan donde el
    /// usuario los dejó. Ver `docs/architecture/storage.md`.
    pub fn library(&self) -> PathBuf {
        self.data.join(LIBRARY_DIR_NAME)
    }

    /// Carpeta del almacén de extensiones.
    ///
    /// Dentro viven el contenido inmutable direccionado por hash, el histórico
    /// de generaciones y el perfil activo. Ver
    /// `src-tauri/src/extensions/store.rs`.
    pub fn extensions(&self) -> PathBuf {
        self.data.join("extensiones")
    }

    /// Carpeta de los diccionarios instalados.
    ///
    /// **La lista de diccionarios instalados es esta carpeta**, no una
    /// preferencia. Es lo que hace que desinstalar uno no pueda dejar rastro:
    /// no hay un segundo sitio del que acordarse de borrarlo.
    pub fn dictionaries(&self) -> PathBuf {
        self.data.join("diccionarios")
    }

    /// Carpeta de los modelos de IA instalados, uno por subcarpeta.
    ///
    /// Igual que con los diccionarios, **la lista de instalados es esta
    /// carpeta**. Va en datos y no en caché por dos razones. Un limpiador de
    /// caché del sistema no debería obligar a volver a descargar doscientos
    /// megabytes sin que nadie lo decida. Y la caché cuenta contra el presupuesto
    /// de lo derivado (`diagnostics::medir_almacenamiento`), que existe para que
    /// el índice no pese más que la biblioteca: un modelo no se deriva de la
    /// biblioteca, y dejarlo ahí le quitaría sitio al índice que el presupuesto acaba de
    /// hacer caber. Su disco se enseña aparte, en Ajustes.
    pub fn models(&self) -> PathBuf {
        self.data.join("modelos")
    }

    /// Carpeta del audio preparado, uno por libro o capítulo.
    ///
    /// Va en datos y no en caché porque costó horas de procesador y una caché es
    /// justo lo que el sistema puede vaciar sin avisar: pasó con las portadas, y
    /// rehacerlas costaba segundos, no una tarde.
    ///
    /// **Al revés que los modelos, esto sí cuenta contra el presupuesto de lo
    /// derivado**, y la diferencia no es un detalle contable: un modelo no sale
    /// de la biblioteca de nadie -pesa lo mismo con un libro que con ciento
    /// cincuenta mil-, mientras que una hora de audio son once megabytes que
    /// existen porque existe ese libro. Un libro de diez horas ocupa ciento diez,
    /// así que sin contarlo el tope que vigila que lo derivado no supere a la
    /// biblioteca dejaría fuera justo lo que más crece.
    pub fn audio(&self) -> PathBuf {
        self.data.join("audio")
    }

    /// La identidad TLS del servidor para los teléfonos emparejados.
    ///
    /// Con los datos y no con la caché: el teléfono fija la huella de este
    /// certificado, y perderlo al limpiar la caché obligaría a emparejar de
    /// nuevo cada teléfono.
    pub fn movil(&self) -> PathBuf {
        self.data.join("movil")
    }
}

/// Nombre de la carpeta de la biblioteca administrada.
///
/// Vive aquí y no incrustado en dos sitios porque la capa de base de datos
/// también lo necesita: al abrir, reescribe la ruta de la raíz administrada con
/// la de esta instalación, y la deduce del lugar donde está el propio archivo de
/// base de datos.
pub const LIBRARY_DIR_NAME: &str = "biblioteca";

/// Carpeta de biblioteca que acompaña a un archivo de base de datos.
///
/// Devuelve `None` para una base en memoria, que no tiene carpeta ninguna.
#[must_use]
pub fn library_beside_database(database: &std::path::Path) -> Option<PathBuf> {
    database.parent().map(|dir| dir.join(LIBRARY_DIR_NAME))
}

// Resolución por plataforma sin dependencias externas: son cuatro llamadas y la
// política de dependencias exige no añadir un paquete por menos de cien líneas.
// Ver docs/architecture/dependency-policy.md

#[cfg(target_os = "linux")]
const APP_DIR: &str = "nb-books";

#[cfg(target_os = "linux")]
fn dirs_data() -> Option<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share")
}

#[cfg(target_os = "linux")]
fn dirs_config() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

#[cfg(target_os = "linux")]
fn dirs_cache() -> Option<PathBuf> {
    xdg("XDG_CACHE_HOME", ".cache")
}

#[cfg(target_os = "linux")]
fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    let base = match std::env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(fallback),
    };
    Some(base.join(APP_DIR))
}

#[cfg(target_os = "windows")]
fn dirs_data() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("NB Books"))
}

#[cfg(target_os = "windows")]
fn dirs_config() -> Option<PathBuf> {
    dirs_data()
}

#[cfg(target_os = "windows")]
fn dirs_cache() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("NB Books\\Cache"))
}

#[cfg(target_os = "macos")]
fn dirs_data() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library/Application Support")
            .join("NB Books"),
    )
}

#[cfg(target_os = "macos")]
fn dirs_config() -> Option<PathBuf> {
    dirs_data()
}

#[cfg(target_os = "macos")]
fn dirs_cache() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library/Caches")
            .join("NB Books"),
    )
}

pub use nb_books_lector::paths::{
    open_noatime, read_bytes_noatime, temporal_de_pruebas, write_atomic,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Las pruebas que tocan el entorno van de una en una.
    ///
    /// # Por qué hace falta un candado y no basta con ser cuidadoso
    ///
    /// Porque una variable de entorno es **del proceso, no del hilo**, y `cargo
    /// test` corre las pruebas de un binario en hilos del mismo proceso. Dos
    /// pruebas que ponen y quitan la misma variable se pisan por muy bien escrita
    /// que esté cada una: basta con que el `remove_var` de una caiga entre el
    /// `set_var` y el `resolve()` de la otra para que la segunda mire las rutas
    /// **de verdad** en lugar de las desviadas.
    ///
    /// No es hipotético: el 2026-09-06 falló así, y lo destapó algo sin ninguna
    /// relación —dos tipos nuevos del puente que añadieron dos pruebas al binario
    /// y cambiaron el reparto entre hilos—. Antes pasaba por dónde caía cada una,
    /// que es la peor forma de pasar.
    ///
    /// Y aquí importa más que en otro sitio: lo que esta prueba vigila es que el
    /// aislamiento funcione. Una guarda intermitente sobre "las pruebas no tocan
    /// la biblioteca real" no es una guarda.
    static ENTORNO: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// El aislamiento de pruebas debe desviar **todas** las rutas, no solo la
    /// base de datos. Si esta prueba falla, las pruebas de extremo a extremo
    /// pueden destruir la biblioteca real de quien las ejecute.
    #[test]
    fn la_variable_de_prueba_desvia_todas_las_rutas() {
        // Un candado envenenado por el pánico de otra prueba no puede impedir que
        // esta corra: lo que protege es el entorno, y el entorno sigue ahí.
        let _turno = ENTORNO.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = crate::paths::temporal_de_pruebas()
            .join(format!("nb-books-test-{}", std::process::id()));
        // SAFETY: la prueba controla el entorno y limpia al terminar.
        unsafe { std::env::set_var(TEST_DIR_ENV, &tmp) };

        let paths = AppPaths::resolve().expect("rutas resolubles");
        assert!(paths.data.starts_with(&tmp));
        assert!(paths.config.starts_with(&tmp));
        assert!(paths.cache.starts_with(&tmp));
        assert!(paths.logs.starts_with(&tmp));
        assert!(paths.database().starts_with(&tmp));

        unsafe { std::env::remove_var(TEST_DIR_ENV) };
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Datos y caché deben estar separados: borrar la caché no puede perder nada.
    #[test]
    fn los_datos_y_la_cache_estan_separados() {
        let _turno = ENTORNO.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = crate::paths::temporal_de_pruebas()
            .join(format!("nb-books-sep-{}", std::process::id()));
        // SAFETY: la prueba controla el entorno y limpia al terminar.
        unsafe { std::env::set_var(TEST_DIR_ENV, &tmp) };

        let paths = AppPaths::resolve().expect("rutas resolubles");
        assert_ne!(paths.data, paths.cache);
        assert!(!paths.database().starts_with(&paths.cache));

        unsafe { std::env::remove_var(TEST_DIR_ENV) };
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
