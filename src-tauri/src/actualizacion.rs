//! Actualizar la aplicación desde sus publicaciones firmadas.
//!
//! # De dónde sale la confianza
//!
//! De la misma firma que verifica un tercero a mano (`docs/developer/deployment/releases.md`,
//! sección 6): la publicación trae `SHA256SUMS` firmado con Ed25519, y la clave
//! pública **viaja compilada dentro del binario**, no al lado de lo que se
//! descarga. Quien pudiera cambiar un instalador en el servidor no puede cambiar
//! la clave con la que este binario lo comprueba. Cada archivo descargado se
//! compara con su línea de la lista antes de tocar nada en el disco.
//!
//! # Por qué no el complemento de actualizaciones de Tauri
//!
//! Porque lleva su propio cliente HTTP y su propio formato de firma (minisign).
//! Aquí toda salida pasa por `net.rs`, con su modo sin red y su registro, y la
//! publicación ya tiene una firma que un tercero sabe comprobar con `openssl`. Un
//! segundo esquema de firma sería una segunda clave que custodiar para firmar lo
//! mismo.
//!
//! # El consentimiento
//!
//! Nunca se busca solo: lo dispara el botón de Ajustes, y ese gesto es el
//! consentimiento para el propósito `update` en los anfitriones de las
//! publicaciones de GitHub, que la interfaz nombra junto al botón. Cualquier otro
//! anfitrión al que redirigiera la descarga se rechaza. El modo sin red lo
//! impide todo, como en el resto de la aplicación.
//!
//! # Cómo se instala
//!
//! Según cómo se instaló la que está corriendo, en Linux:
//!
//! | Instalación | Qué se hace |
//! |---|---|
//! | AppImage | Se sustituye el archivo por el nuevo, comprobado |
//! | `.tar.gz` en una carpeta del usuario | Se extrae al lado y se intercambia la carpeta |
//! | `.deb` | `pkexec dpkg -i`, que pide permiso en una ventana del sistema |
//! | `.rpm` | `pkexec rpm -U` |
//!
//! En cualquier otro caso se responde `COMPONENT_UNAVAILABLE` y la interfaz dice
//! que esta instalación se actualiza con su gestor de paquetes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use ts_rs::TS;

use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::net::{Descarga, Encargo, Network, Purpose};

/// La clave pública con la que se firman las publicaciones.
pub const CLAVE_PUBLICA: &str =
    include_str!("../../docs/architecture/api/clave-de-publicacion.pem");

/// La última publicación del repositorio. GitHub redirige cada archivo a su red
/// de distribución, que es el segundo anfitrión que se admite.
const BASE: &str = "https://github.com/nb-digital-hq/nb-books/releases/latest/download/";

/// El archivo de notas que acompaña a cada publicación, firmado con lo demás.
const NOTAS: &str = "NOTAS.md";

/// Cuánto puede tardar una descarga. Un AppImage pesa unos 130 MB.
const PLAZO: Duration = Duration::from_secs(15 * 60);

/// Una versión publicada más nueva que la instalada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Actualizacion {
    /// La versión publicada, por ejemplo `1.0.1`.
    pub version: String,
    /// Las notas de la publicación, si las trae.
    pub notas: Option<String>,
    /// Si esta instalación puede actualizarse sola. Si no, la interfaz dice que
    /// se actualiza con el gestor de paquetes del sistema.
    pub instalable: bool,
}

/// Lo que se sabe de la última publicación tras comprobar su firma.
struct Publicacion {
    version: String,
    sumas: Vec<u8>,
    notas: Option<String>,
}

/// Si a este anfitrión se le puede pedir una publicación.
fn anfitrion_admitido(host: &str) -> bool {
    host == "github.com" || host.ends_with(".githubusercontent.com")
}

/// Descarga `url` entera en memoria, concediendo el propósito `update` a los
/// anfitriones de las publicaciones cuando lo piden.
fn bajar(net: &Network, db: &Database, url: &str) -> NbResult<Vec<u8>> {
    // Una por anfitrión de la cadena de redirecciones, y una más para la descarga.
    for _ in 0..4 {
        let mut destino = Vec::new();
        let encargo = Encargo {
            url,
            purpose: Purpose::Update,
            plazo: PLAZO,
        };
        match net.descargar(db, &encargo, &mut destino, &mut |_| true)? {
            Descarga::Completa { .. } => return Ok(destino),
            Descarga::FaltaPermiso { host } if anfitrion_admitido(&host) => {
                let conn = db.connection()?;
                crate::net::grant(&conn, &host, Purpose::Update)?;
            }
            Descarga::FaltaPermiso { .. } => {
                tracing::warn!("una publicación redirigió a un anfitrión no admitido");
                return Err(NbError::ConsentRequired);
            }
        }
    }
    Err(NbError::NetworkFailure)
}

/// La versión que declara la lista: la del `.deb`, que va en toda publicación.
fn version_listada(sumas: &[u8]) -> Option<String> {
    crate::artefactos::nombres_listados(sumas)
        .into_iter()
        .find_map(|nombre| {
            nombre
                .strip_prefix("nb-books_")?
                .strip_suffix("_amd64.deb")
                .map(str::to_owned)
        })
}

/// Las tres partes numéricas de una versión `mayor.menor.parche`.
fn partes(version: &str) -> Option<(u64, u64, u64)> {
    let mut trozos = version.split(['-', '+']).next()?.split('.');
    let mayor = trozos.next()?.parse().ok()?;
    let menor = trozos.next()?.parse().ok()?;
    let parche = trozos.next()?.parse().ok()?;
    trozos.next().is_none().then_some((mayor, menor, parche))
}

/// Si `publicada` es más nueva que `instalada`.
fn es_mas_nueva(publicada: &str, instalada: &str) -> bool {
    matches!((partes(publicada), partes(instalada)), (Some(p), Some(i)) if p > i)
}

/// Descarga la lista firmada de la última publicación y la comprueba.
fn publicacion(net: &Network, db: &Database) -> NbResult<Publicacion> {
    let sumas = bajar(net, db, &format!("{BASE}{}", crate::artefactos::SUMAS))?;
    let firma = bajar(net, db, &format!("{BASE}{}", crate::artefactos::FIRMA))?;
    if !crate::artefactos::firma_valida(&sumas, &firma, CLAVE_PUBLICA) {
        tracing::warn!("la lista de la publicación no lleva la firma del proyecto");
        return Err(NbError::DocumentCorrupted);
    }
    let version = version_listada(&sumas).ok_or(NbError::FormatUnsupported)?;
    let notas = match crate::artefactos::huella_listada(&sumas, NOTAS) {
        Some(huella) => {
            let bytes = bajar(net, db, &format!("{BASE}{NOTAS}"))?;
            (crate::artefactos::huella_de_bytes(&bytes) == huella)
                .then(|| String::from_utf8_lossy(&bytes).trim().to_owned())
                .filter(|texto| !texto.is_empty())
        }
        None => None,
    };
    Ok(Publicacion {
        version,
        sumas,
        notas,
    })
}

/// Cómo se instaló la aplicación que está corriendo.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Instalacion {
    AppImage(PathBuf),
    Carpeta(PathBuf),
    Deb,
    Rpm,
    Ninguna,
}

fn en_el_path(programa: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|carpeta| carpeta.join(programa).is_file())
    })
}

/// La marca que deja NB Launcher junto al ejecutable de una copia que gestiona él.
///
/// Es su contrato de instalaciones administradas: la marca dice que los binarios
/// los pone y los cambia el Launcher, que verifica su firma, y que esta copia no
/// tiene que actualizarse sola. Hacerlo sería sustituir archivos que otro
/// programa comprueba byte a byte, y el Launcher la daría por dañada.
const MARCA_DEL_LAUNCHER: &str = ".nb-launcher-managed";

/// Si esta copia la instaló y la actualiza NB Launcher.
#[must_use]
pub fn gestionada_por_launcher() -> bool {
    std::env::current_exe()
        .and_then(|e| e.canonicalize())
        .ok()
        .and_then(|e| e.parent().map(|bin| bin.join(MARCA_DEL_LAUNCHER)))
        .is_some_and(|marca| marca.is_file())
}

fn instalacion() -> Instalacion {
    if !cfg!(target_os = "linux") {
        return Instalacion::Ninguna;
    }
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Instalacion::AppImage(PathBuf::from(appimage));
    }
    let Ok(ejecutable) = std::env::current_exe().and_then(|e| e.canonicalize()) else {
        return Instalacion::Ninguna;
    };
    if ejecutable.starts_with("/usr") {
        if en_el_path("pkexec") && en_el_path("dpkg") {
            return Instalacion::Deb;
        }
        if en_el_path("pkexec") && en_el_path("rpm") {
            return Instalacion::Rpm;
        }
        return Instalacion::Ninguna;
    }
    // El `.tar.gz` se instala como `<carpeta>/bin/nb-books`.
    match ejecutable.parent() {
        Some(bin) if bin.file_name().is_some_and(|n| n == "bin") => {
            bin.parent().map_or(Instalacion::Ninguna, |c| {
                Instalacion::Carpeta(c.to_path_buf())
            })
        }
        _ => Instalacion::Ninguna,
    }
}

/// El archivo de la publicación que corresponde a una instalación.
///
/// Cada paquete lleva las dos ediciones, la universal y la rápida con AVX2, y el
/// arranque elige la que el procesador admite (`main.rs`): no hay un archivo por
/// edición que elegir aquí.
fn archivo_para(instalacion: &Instalacion, version: &str) -> Option<String> {
    match instalacion {
        Instalacion::AppImage(_) => Some(format!("nb-books_{version}_amd64.AppImage")),
        Instalacion::Carpeta(_) => Some(format!("nb-books_{version}_linux-x86_64.tar.gz")),
        Instalacion::Deb => Some(format!("nb-books_{version}_amd64.deb")),
        Instalacion::Rpm => Some(format!("nb-books-{version}-1.x86_64.rpm")),
        Instalacion::Ninguna => None,
    }
}

/// Mira si hay una versión publicada más nueva que la instalada.
///
/// # Errors
///
/// - [`NbError::NetworkDisabled`] con el modo sin red puesto.
/// - [`NbError::ConsentRequired`] si la publicación redirige a otro anfitrión.
/// - [`NbError::NetworkFailure`] si no se pudo consultar.
/// - [`NbError::DocumentCorrupted`] si la lista no lleva la firma del proyecto.
pub fn buscar(net: &Network, db: &Database) -> NbResult<Option<Actualizacion>> {
    // Una copia del Launcher no pregunta a la red: sus actualizaciones las busca él.
    if gestionada_por_launcher() {
        return Err(NbError::ComponentUnavailable);
    }
    let publicada = publicacion(net, db)?;
    if !es_mas_nueva(&publicada.version, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let instalable = archivo_para(&instalacion(), &publicada.version).is_some_and(|archivo| {
        crate::artefactos::huella_listada(&publicada.sumas, &archivo).is_some()
    });
    Ok(Some(Actualizacion {
        version: publicada.version,
        notas: publicada.notas,
        instalable,
    }))
}

/// Descarga, comprueba e instala la versión más nueva. Devuelve si instaló algo:
/// quien llama reinicia la aplicación.
///
/// # Errors
///
/// Los de [`buscar`]; [`NbError::ComponentUnavailable`] si esta instalación no se
/// actualiza sola o el sistema no dio permiso; [`NbError::DocumentCorrupted`] si
/// lo descargado no es lo que la lista firmada dice.
pub fn instalar(net: &Network, db: &Database, temporal: &Path) -> NbResult<bool> {
    if gestionada_por_launcher() {
        return Err(NbError::ComponentUnavailable);
    }
    let publicada = publicacion(net, db)?;
    if !es_mas_nueva(&publicada.version, env!("CARGO_PKG_VERSION")) {
        return Ok(false);
    }
    let como = instalacion();
    let archivo = archivo_para(&como, &publicada.version).ok_or(NbError::ComponentUnavailable)?;
    let huella = crate::artefactos::huella_listada(&publicada.sumas, &archivo)
        .ok_or(NbError::ComponentUnavailable)?;
    let bytes = bajar(net, db, &format!("{BASE}{archivo}"))?;
    if crate::artefactos::huella_de_bytes(&bytes) != huella {
        tracing::warn!("el instalador descargado no coincide con la lista firmada");
        return Err(NbError::DocumentCorrupted);
    }
    aplicar(&como, &bytes, &archivo, temporal)?;
    tracing::info!("actualización instalada");
    Ok(true)
}

#[cfg(target_os = "linux")]
fn aplicar(como: &Instalacion, bytes: &[u8], archivo: &str, temporal: &Path) -> NbResult<()> {
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    let escribir = |ruta: &Path| -> NbResult<()> {
        std::fs::write(ruta, bytes).map_err(|_| NbError::StorageFailure)
    };
    let ejecutar = |programa: &str, argumentos: &[&std::ffi::OsStr]| -> NbResult<()> {
        let bien = Command::new(programa)
            .args(argumentos)
            .status()
            .is_ok_and(|s| s.success());
        if bien {
            Ok(())
        } else {
            tracing::warn!(programa, "la instalación del sistema no terminó");
            Err(NbError::ComponentUnavailable)
        }
    };

    match como {
        Instalacion::AppImage(destino) => {
            // Al lado del destino, para que el cambio de nombre sea atómico.
            let nuevo = destino.with_extension("nueva");
            escribir(&nuevo)?;
            std::fs::set_permissions(&nuevo, std::fs::Permissions::from_mode(0o755))
                .map_err(|_| NbError::StorageFailure)?;
            std::fs::rename(&nuevo, destino).map_err(|_| NbError::StorageFailure)
        }
        Instalacion::Carpeta(carpeta) => {
            let paquete = temporal.join(archivo);
            escribir(&paquete)?;
            let nueva = carpeta.with_extension("nueva");
            let anterior = carpeta.with_extension("anterior");
            let _ = std::fs::remove_dir_all(&nueva);
            std::fs::create_dir_all(&nueva).map_err(|_| NbError::StorageFailure)?;
            ejecutar(
                "tar",
                &[
                    "-xzf".as_ref(),
                    paquete.as_os_str(),
                    "--strip-components=1".as_ref(),
                    "-C".as_ref(),
                    nueva.as_os_str(),
                ],
            )?;
            let _ = std::fs::remove_file(&paquete);
            if !nueva.join("bin").join("nb-books").is_file() {
                let _ = std::fs::remove_dir_all(&nueva);
                return Err(NbError::DocumentCorrupted);
            }
            // El mismo enlace que deja la instalación local.
            #[cfg(unix)]
            let _ = std::os::unix::fs::symlink("bin/nb-books", nueva.join("AppRun"));
            let _ = std::fs::remove_dir_all(&anterior);
            std::fs::rename(carpeta, &anterior).map_err(|_| NbError::StorageFailure)?;
            if std::fs::rename(&nueva, carpeta).is_err() {
                let _ = std::fs::rename(&anterior, carpeta);
                return Err(NbError::StorageFailure);
            }
            let _ = std::fs::remove_dir_all(&anterior);
            Ok(())
        }
        Instalacion::Deb | Instalacion::Rpm => {
            let paquete = temporal.join(archivo);
            escribir(&paquete)?;
            let resultado = if *como == Instalacion::Deb {
                ejecutar(
                    "pkexec",
                    &["dpkg".as_ref(), "-i".as_ref(), paquete.as_os_str()],
                )
            } else {
                ejecutar(
                    "pkexec",
                    &["rpm".as_ref(), "-U".as_ref(), paquete.as_os_str()],
                )
            };
            let _ = std::fs::remove_file(&paquete);
            resultado
        }
        Instalacion::Ninguna => Err(NbError::ComponentUnavailable),
    }
}

#[cfg(not(target_os = "linux"))]
fn aplicar(_como: &Instalacion, _bytes: &[u8], _archivo: &str, _temporal: &Path) -> NbResult<()> {
    Err(NbError::ComponentUnavailable)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn una_version_solo_es_mas_nueva_si_lo_es_numericamente() {
        assert!(es_mas_nueva("1.0.1", "1.0.0"));
        assert!(es_mas_nueva("1.10.0", "1.9.9"));
        assert!(!es_mas_nueva("1.0.0", "1.0.0"));
        assert!(!es_mas_nueva("0.9.9", "1.0.0"));
        assert!(!es_mas_nueva("version", "1.0.0"));
    }

    #[test]
    fn la_version_sale_del_deb_de_la_lista() {
        let sumas = format!(
            "{} *nb-books_1.2.3_amd64.deb\n{} *SHA256SUMS.txt\n",
            "a".repeat(64),
            "b".repeat(64)
        );
        assert_eq!(version_listada(sumas.as_bytes()).as_deref(), Some("1.2.3"));
        assert_eq!(version_listada(b"basura"), None);
    }

    #[test]
    fn solo_se_admiten_los_anfitriones_de_las_publicaciones() {
        assert!(anfitrion_admitido("github.com"));
        assert!(anfitrion_admitido("release-assets.githubusercontent.com"));
        assert!(!anfitrion_admitido("githubusercontent.com.ejemplo.org"));
        assert!(!anfitrion_admitido("ejemplo.org"));
    }

    #[test]
    fn cada_instalacion_pide_su_archivo() {
        assert_eq!(
            archivo_para(&Instalacion::Deb, "1.0.1").as_deref(),
            Some("nb-books_1.0.1_amd64.deb")
        );
        assert_eq!(
            archivo_para(&Instalacion::Rpm, "1.0.1").as_deref(),
            Some("nb-books-1.0.1-1.x86_64.rpm")
        );
        assert_eq!(archivo_para(&Instalacion::Ninguna, "1.0.1"), None);
    }

    #[test]
    fn la_clave_compilada_es_una_clave_publica_de_ed25519() {
        assert!(CLAVE_PUBLICA.contains("BEGIN PUBLIC KEY"));
        assert!(!crate::artefactos::firma_valida(
            b"x",
            &[0; 64],
            CLAVE_PUBLICA
        ));
    }
}
