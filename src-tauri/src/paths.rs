//! Resolución de rutas por plataforma.
//!
//! Los datos del usuario y la caché regenerable viven en directorios distintos:
//! borrar la caché nunca puede perder información. Ver
//! `docs/developer/deployment/README.md`.
//!
//! La variable de entorno `NB_BOOKS_TEST_DIR` desvía **todas** las rutas a un
//! directorio temporal, para que las pruebas de extremo a extremo no toquen la
//! biblioteca real de quien las ejecuta.

use std::path::{Path, PathBuf};

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

/// Escribe un archivo de forma que nadie pueda leerlo a medias.
///
/// # Por qué no basta `fs::write`
///
/// Los archivos que produce la aplicación en la caché (portadas, páginas
/// rendidas) los escribe un hilo mientras otro comprueba si ya existen. Con una
/// escritura directa, el segundo puede encontrar el archivo creado y todavía a
/// medio llenar, y entonces la portada se ve cortada o la página no se decodifica
/// y el fallo aparece una vez de cada cien, en una máquina, sin forma de
/// reproducirlo.
///
/// Se escribe a un temporal **en el mismo directorio** y se renombra encima. El
/// renombrado dentro de un mismo sistema de archivos es atómico: quien mire verá
/// o el archivo entero o ningún archivo, nunca uno a medias. En el mismo
/// directorio y no en el temporal del sistema porque entre sistemas de archivos
/// distintos el renombrado deja de ser un renombrado y vuelve a ser una copia.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir o renombrar. El temporal
/// se retira en ese caso: dejarlo llenaría la caché de restos que nada limpia.
pub fn write_atomic(destino: &Path, escribir: impl FnOnce(&Path) -> NbResult<()>) -> NbResult<()> {
    let directorio = destino.parent().ok_or(NbError::PathNotAllowed)?;
    std::fs::create_dir_all(directorio).map_err(|_| NbError::StorageFailure)?;

    let nombre = destino
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(NbError::PathNotAllowed)?;
    // Identificado por proceso e hilo: dos hilos escribiendo la misma página no
    // pueden pisarse el temporal, que es justo el caso que esto viene a arreglar.
    let temporal = directorio.join(format!(
        ".{nombre}.{}.{:?}.parcial",
        std::process::id(),
        std::thread::current().id()
    ));

    if let Err(error) = escribir(&temporal) {
        let _ = std::fs::remove_file(&temporal);
        return Err(error);
    }

    std::fs::rename(&temporal, destino).map_err(|_| {
        let _ = std::fs::remove_file(&temporal);
        NbError::StorageFailure
    })
}

/// Abre un archivo para lectura intentando no actualizar su hora de acceso (`atime`).
///
/// En Linux utiliza la bandera `O_NOATIME` (`libc::O_NOATIME`) para evitar que
/// operaciones de importación, indexación y apertura de libros modifiquen la marca
/// temporal `atime` de los archivos referenciados del usuario.
///
/// Si `O_NOATIME` falla (por ejemplo, con `EPERM` cuando el archivo pertenece a otro
/// usuario o el sistema de archivos no lo admite), reintenta abriéndolo de forma
/// estándar sin la bandera, garantizando degradación elegante.
///
/// En plataformas no Linux (Windows, macOS), abre el archivo para lectura de forma
/// habitual, ya que `O_NOATIME` es una característica específica del núcleo Linux.
///
/// # Errors
///
/// Devuelve los errores estándar de [`std::fs::File::open`].
pub fn open_noatime(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;

        let mut opciones = std::fs::OpenOptions::new();
        opciones.read(true);
        opciones.custom_flags(libc::O_NOATIME);
        match opciones.open(path) {
            Ok(archivo) => Ok(archivo),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(error),
            Err(_) => std::fs::File::open(path),
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        std::fs::File::open(path)
    }
}

/// Lee el contenido completo de un archivo en memoria usando [`open_noatime`].
///
/// # Errors
///
/// Devuelve los errores estándar de [`open_noatime`] o de lectura de bytes.
pub fn read_bytes_noatime(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;

    let mut archivo = open_noatime(path)?;
    let mut buffer = Vec::new();
    archivo.read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Carpeta temporal de las pruebas de este proceso.
///
/// Las pruebas crean carpetas en el temporal del sistema y casi ninguna las
/// borraba: se acumularon miles, unos 6 GB, en un `/tmp` que vive en memoria.
/// Ahora todas cuelgan de `nb-pruebas/<pid>`, y la primera llamada de cada
/// proceso barre las de procesos que ya no existen. Así, aunque una prueba
/// falle a medias o no limpie, lo que queda es como mucho lo de una ejecución.
///
/// No es parte de la aplicación: solo la usan las pruebas.
#[doc(hidden)]
#[must_use]
pub fn temporal_de_pruebas() -> PathBuf {
    static RAIZ: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    RAIZ.get_or_init(|| {
        let raiz = std::env::temp_dir().join("nb-pruebas");
        if let Ok(entradas) = std::fs::read_dir(&raiz) {
            for entrada in entradas.flatten() {
                let vivo = entrada
                    .file_name()
                    .to_str()
                    .and_then(|n| n.parse::<u32>().ok())
                    .is_some_and(proceso_vivo);
                if !vivo {
                    borrar_aunque_sea_de_solo_lectura(&entrada.path());
                }
            }
        }
        let propia = raiz.join(std::process::id().to_string());
        let _ = std::fs::create_dir_all(&propia);
        propia
    })
    .clone()
}

#[cfg(target_os = "linux")]
fn proceso_vivo(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

// Fuera de Linux no hay `/proc`: se da por vivo y no se barre nada ajeno.
#[cfg(not(target_os = "linux"))]
const fn proceso_vivo(_pid: u32) -> bool {
    true
}

/// Borra una carpeta aunque alguna prueba la dejara sin permiso de escritura,
/// como hace la de Calibre en solo lectura.
fn borrar_aunque_sea_de_solo_lectura(ruta: &Path) {
    if std::fs::remove_dir_all(ruta).is_ok() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fn abrir(ruta: &Path) {
            let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o755));
            if let Ok(entradas) = std::fs::read_dir(ruta) {
                for e in entradas.flatten() {
                    if e.file_type().is_ok_and(|t| t.is_dir()) {
                        abrir(&e.path());
                    }
                }
            }
        }
        abrir(ruta);
    }
    let _ = std::fs::remove_dir_all(ruta);
}

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

#[cfg(test)]
mod escritura_atomica {
    use super::*;

    fn temporal(nombre: &str) -> PathBuf {
        let dir = crate::paths::temporal_de_pruebas().join(format!(
            "nb-atomica-{nombre}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        dir
    }

    /// Regresión: la portada de un EPUB de dos megabytes se leyó a medias.
    ///
    /// Ocurrió de verdad, en `tests/metadatos.rs`: el servicio de metadatos
    /// escribía la portada mientras otro hilo comprobaba si existía, y esa
    /// comprobación la encontró creada con 765 952 de sus 2 097 152 bytes. Con
    /// `fs::write` el archivo aparece vacío y se llena a la vista de todos; aquí
    /// no aparece hasta que está entero.
    #[test]
    fn el_destino_no_existe_mientras_se_escribe() {
        let dir = temporal("durante");
        let destino = dir.join("portada.png");

        write_atomic(&destino, |parcial| {
            std::fs::write(parcial, vec![7_u8; 4096]).expect("escritura");
            assert!(
                !destino.exists(),
                "quien mire ahora mismo no debe ver nada, y menos algo a medias"
            );
            Ok(())
        })
        .expect("escritura atómica");

        assert_eq!(
            std::fs::read(&destino).expect("lectura").len(),
            4096,
            "y al terminar está entero"
        );
    }

    #[test]
    fn una_escritura_fallida_no_deja_ni_destino_ni_restos() {
        let dir = temporal("fallo");
        let destino = dir.join("pagina.png");

        let resultado = write_atomic(&destino, |parcial| {
            std::fs::write(parcial, b"a medias").expect("escritura");
            Err(NbError::StorageFailure)
        });

        assert_eq!(resultado.err(), Some(NbError::StorageFailure));
        assert!(!destino.exists(), "un fallo no deja un archivo a medias");
        assert_eq!(
            std::fs::read_dir(&dir).expect("caché").count(),
            0,
            "ni restos que nada limpie después"
        );
    }

    #[test]
    fn reescribir_encima_sustituye_sin_dejar_hueco() {
        let dir = temporal("encima");
        let destino = dir.join("pagina.png");
        std::fs::write(&destino, b"anterior").expect("primera");

        write_atomic(&destino, |parcial| {
            std::fs::write(parcial, b"posterior mas larga").expect("segunda");
            // El archivo viejo sigue completo hasta el instante del renombrado:
            // nunca hay un momento en que la caché esté vacía.
            assert_eq!(std::fs::read(&destino).expect("lectura"), b"anterior");
            Ok(())
        })
        .expect("escritura atómica");

        assert_eq!(
            std::fs::read(&destino).expect("lectura"),
            b"posterior mas larga"
        );
        assert_eq!(std::fs::read_dir(&dir).expect("caché").count(), 1);
    }
}

#[cfg(test)]
mod lectura_sin_hora_de_acceso {
    use super::*;

    #[test]
    fn open_noatime_lee_archivo_existente_y_falla_si_no_existe() {
        let dir =
            crate::paths::temporal_de_pruebas().join(format!("nb-noatime-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("prueba.txt");
        std::fs::write(&ruta, b"contenido de prueba").expect("escritura");

        let mut archivo = open_noatime(&ruta).expect("abrir con open_noatime");
        let mut contenido = Vec::new();
        std::io::Read::read_to_end(&mut archivo, &mut contenido).expect("lectura");
        assert_eq!(contenido, b"contenido de prueba");

        let bytes = read_bytes_noatime(&ruta).expect("read_bytes_noatime");
        assert_eq!(bytes, b"contenido de prueba");

        let inexistente = dir.join("no_existe.txt");
        assert_eq!(
            open_noatime(&inexistente).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn degrada_con_elegancia_si_el_archivo_no_admite_o_noatime() {
        // En Linux, abrir un archivo del sistema que no pertenece al usuario (p. ej. /etc/hosts)
        // con O_NOATIME devuelve EPERM a menos que el proceso tenga CAP_FOWNER.
        // open_noatime debe capturar el error y degradar a File::open estándar sin fallar.
        let hosts = Path::new("/etc/hosts");
        if hosts.exists() {
            let resultado = open_noatime(hosts);
            assert!(
                resultado.is_ok(),
                "debe degradar y abrir /etc/hosts exitosamente"
            );
        }
    }
}
