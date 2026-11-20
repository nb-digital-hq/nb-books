//! Instalar, comprobar y quitar un modelo del catálogo.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 2.//! //!
//! # La garantía que da, y cómo
//!
//! **Un modelo que no coincide con lo revisado no aparece nunca como instalado.**
//! Ni a medias, ni por un corte de luz, ni porque el servidor mande otra cosa.
//! Se consigue con una sola regla de disposición en disco:
//!
//! 1. Todo se descarga dentro de una carpeta temporal, `.<id>.parcial`.
//! 2. Cada archivo se compara con su huella y su tamaño **del catálogo compilado**
//!    según se escribe.
//! 3. Solo cuando los tres coinciden, la carpeta temporal se renombra a `<id>`,
//!    que es una sola operación atómica del sistema de archivos.
//!
//! Así que la carpeta `<id>` **solo puede existir si se verificó entera**. Una
//! descarga cortada deja una carpeta `.parcial`, que no tiene el nombre de ningún
//! modelo y se borra al volver a intentarlo.
//!
//! # Por qué al arrancar se mira el tamaño y no la huella
//!
//! Porque la huella de doscientos megabytes cuesta leerlos enteros, en cada
//! arranque, para confirmar algo que la regla de arriba ya garantiza. El tamaño
//! se mira porque sí cuesta nada y caza lo que la regla no puede evitar: que
//! alguien trunque o sustituya el archivo después. Lo que no caza es un archivo
//! del mismo tamaño alterado a mano, y eso lo detecta la carga del modelo
//!, que lee su cabecera antes de usarlo.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::catalogo::{Archivo, Modelo};
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::net::{Descarga, Encargo, Network, Purpose};

/// Margen de espacio libre por encima de lo que ocupa el modelo.
///
/// Una vigésima parte. No es para el modelo, que se renombra y no se copia: es
/// para no dejar el disco a cero, porque la base de datos escribe su diario al
/// lado y un disco lleno es justo lo que no puede pasarle.
const MARGEN_DE_ESPACIO: u64 = 20;

/// Ritmo por debajo del cual una descarga se da por muerta: 25 KiB por segundo.
///
/// `ureq` no ofrece un plazo de inactividad sino uno para la descarga entera, así
/// que el plazo se calcula con el tamaño y un ritmo muy lento: una conexión mala
/// pero viva termina, y una muerta se detecta al agotarse. Ver
/// `net::UreqTransport::descargar`.
const RITMO_MINIMO: u64 = 25 * 1024;

/// Plazo mínimo, para los archivos pequeños: aunque pesen poco, abrir la
/// conexión y seguir la redirección tarda.
const PLAZO_MINIMO: Duration = Duration::from_secs(120);

/// Cómo terminó intentar instalar, cuando no hubo un error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instalacion {
    /// Instalado y verificado, o ya lo estaba.
    Instalado,
    /// Falta el permiso de este anfitrión. Puede ser el de la red de distribución
    /// a la que redirige el del catálogo, y por eso se devuelve su nombre.
    FaltaPermiso { host: String },
}

/// Lo que esta disposición necesita saber de algo que se descarga.
///
/// Existe porque la voz neuronal no es un modelo de IA —no tiene
/// capacidad, ni dimensión, ni agrupación— y aun así necesita **exactamente la
/// misma garantía**: huella revisada, carpeta temporal y renombrado atómico.
/// Duplicar esta disposición para ella habría sido duplicar la única pieza del
/// proyecto que decide si unos bytes descargados se pueden ejecutar.
pub trait Instalable: Sync {
    /// Identificador estable; es el nombre de su carpeta.
    fn id(&self) -> &'static str;
    /// Los archivos que lo componen, con los bytes revisados.
    fn archivos(&self) -> &'static [Archivo];
    /// De dónde se descarga uno de ellos.
    fn direccion(&self, archivo: &Archivo) -> String;
    /// Lo que ocupa instalado.
    fn disco(&self) -> u64 {
        self.archivos().iter().map(|a| a.bytes).sum()
    }
}

impl Instalable for Modelo {
    fn id(&self) -> &'static str {
        self.id
    }
    fn archivos(&self) -> &'static [Archivo] {
        self.archivos
    }
    fn direccion(&self, archivo: &Archivo) -> String {
        Self::direccion(self, archivo)
    }
}

/// Dónde vive un modelo instalado.
#[must_use]
pub fn carpeta(modelos: &Path, modelo: &dyn Instalable) -> PathBuf {
    modelos.join(modelo.id())
}

fn carpeta_parcial(modelos: &Path, modelo: &dyn Instalable) -> PathBuf {
    modelos.join(format!(".{}.parcial", modelo.id()))
}

/// Si el modelo está instalado: su carpeta existe y cada archivo tiene el tamaño
/// revisado. Ver el encabezado del módulo para por qué no se relee la huella.
#[must_use]
pub fn instalado(modelos: &Path, modelo: &dyn Instalable) -> bool {
    let carpeta = carpeta(modelos, modelo);
    modelo.archivos().iter().all(|archivo| {
        std::fs::metadata(carpeta.join(archivo.nombre))
            .is_ok_and(|datos| datos.is_file() && datos.len() == archivo.bytes)
    })
}

/// Si queda rastro de que alguien lo instaló: su carpeta existe, esté entera o
/// no.
///
/// Es lo que separa "nadie lo ha pedido" de "se instaló y ahora está roto", que
/// piden cosas opuestas a la interfaz. Ver `LocalProvider::readiness`.
#[must_use]
pub fn hay_rastro(modelos: &Path, modelo: &dyn Instalable) -> bool {
    carpeta(modelos, modelo).is_dir()
}

/// Si hay sitio para instalarlo, con su margen.
#[must_use]
pub fn cabe(libre: u64, modelo: &dyn Instalable) -> bool {
    let necesario = modelo.disco();
    libre >= necesario.saturating_add(necesario / MARGEN_DE_ESPACIO)
}

/// Descarga, verifica e instala un modelo.
///
/// `progreso` recibe los bytes descargados del modelo entero y el total, para
/// que la interfaz enseñe una sola barra y no una por archivo.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si no hay sitio, o si no se puede escribir.
/// - [`NbError::DocumentCorrupted`] si lo descargado no coincide con la huella o
///   el tamaño revisados. Se reutiliza el código canónico en lugar de añadir uno:
///   la acción que pide es la misma —no hay nada que el usuario pueda arreglar,
///   solo volver a intentarlo—, y la vista pone el texto en su contexto, igual que
///   ya hace Descubrir con un feed que no se puede leer.
/// - [`NbError::Cancelled`] si `cancelado` se puso a cierto.
/// - Los de [`Network::descargar`].
///
/// En **todos** los casos de error la carpeta temporal se borra, y la del modelo
/// no se ha tocado.
pub fn instalar(
    db: &Database,
    red: &Network,
    modelos: &Path,
    modelo: &dyn Instalable,
    cancelado: &AtomicBool,
    progreso: &mut dyn FnMut(u64, u64),
) -> NbResult<Instalacion> {
    if instalado(modelos, modelo) {
        return Ok(Instalacion::Instalado);
    }

    std::fs::create_dir_all(modelos).map_err(|_| NbError::StorageFailure)?;
    if let Some(libre) = crate::ai::hardware::disco_libre(modelos) {
        if !cabe(libre, modelo) {
            tracing::info!(
                necesario = modelo.disco(),
                libre,
                "no hay sitio para instalar el modelo"
            );
            return Err(NbError::StorageFailure);
        }
    }

    let parcial = carpeta_parcial(modelos, modelo);
    // Lo que haya quedado de un intento anterior no se aprovecha: no se sabe en
    // qué punto se cortó, y reanudar sobre bytes sin verificar es justo lo que
    // esta disposición existe para no hacer.
    borrar_si_existe(&parcial)?;
    std::fs::create_dir_all(&parcial).map_err(|_| NbError::StorageFailure)?;

    let resultado = descargar_todo(db, red, &parcial, modelo, cancelado, progreso);
    match resultado {
        Ok(Instalacion::Instalado) => {
            let definitiva = carpeta(modelos, modelo);
            // Si hay una carpeta con el nombre del modelo y no pasó `instalado`,
            // está rota —un archivo truncado después—, y se sustituye.
            if let Err(error) = borrar_si_existe(&definitiva).and_then(|()| {
                std::fs::rename(&parcial, &definitiva).map_err(|_| NbError::StorageFailure)
            }) {
                let _ = borrar_si_existe(&parcial);
                return Err(error);
            }
            tracing::info!(bytes = modelo.disco(), "modelo instalado y verificado");
            Ok(Instalacion::Instalado)
        }
        otro => {
            let _ = borrar_si_existe(&parcial);
            otro
        }
    }
}

/// Quita un modelo instalado, y cualquier intento a medias.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede borrar.
pub fn desinstalar(modelos: &Path, modelo: &dyn Instalable) -> NbResult<()> {
    borrar_si_existe(&carpeta_parcial(modelos, modelo))?;
    borrar_si_existe(&carpeta(modelos, modelo))
}

fn descargar_todo(
    db: &Database,
    red: &Network,
    parcial: &Path,
    modelo: &dyn Instalable,
    cancelado: &AtomicBool,
    progreso: &mut dyn FnMut(u64, u64),
) -> NbResult<Instalacion> {
    let total = modelo.disco();
    let mut previos: u64 = 0;

    for archivo in modelo.archivos() {
        let ruta = parcial.join(archivo.nombre);
        let destino = std::fs::File::create(&ruta).map_err(|_| NbError::StorageFailure)?;
        let mut escritor = Verificador::new(destino, archivo);

        let descarga = red
            .descargar(
                db,
                &Encargo {
                    url: &modelo.direccion(archivo),
                    purpose: Purpose::Model,
                    plazo: plazo_para(archivo.bytes),
                },
                &mut escritor,
                &mut |bytes| {
                    progreso(previos + bytes, total);
                    !cancelado.load(Ordering::SeqCst)
                },
            )
            .map_err(|error| {
                if escritor.desbordado {
                    tracing::warn!(
                        // no es del usuario: el nombre sale del catálogo de modelos.
                        archivo = archivo.nombre,
                        "el servidor mandó más bytes de los que se revisaron"
                    );
                    NbError::DocumentCorrupted
                } else {
                    error
                }
            })?;

        match descarga {
            Descarga::FaltaPermiso { host } => return Ok(Instalacion::FaltaPermiso { host }),
            Descarga::Completa { .. } => escritor.terminar()?,
        }
        previos += archivo.bytes;
    }

    Ok(Instalacion::Instalado)
}

/// Plazo para descargar un archivo de este tamaño.
fn plazo_para(bytes: u64) -> Duration {
    Duration::from_secs(bytes / RITMO_MINIMO).max(PLAZO_MINIMO)
}

/// Escribe en disco y comprueba **según escribe**.
///
/// Comprueba el tamaño en cada bloque y no al final: un servidor que mande diez
/// gigas en lugar de doscientos megas llenaría el disco antes de que la huella
/// tuviera ocasión de decir que no coincide. Pasado el tamaño revisado, la
/// escritura falla y la descarga se corta.
struct Verificador<'a> {
    archivo: std::fs::File,
    revisado: &'a Archivo,
    huella: Sha256,
    escritos: u64,
    /// Si se intentó escribir más de lo revisado.
    ///
    /// Hace falta saberlo después: el transporte convierte cualquier fallo de
    /// escritura en `StorageFailure`, y "no se pudo escribir" sería mentir sobre
    /// un servidor que mandó otra cosa.
    desbordado: bool,
}

impl<'a> Verificador<'a> {
    fn new(archivo: std::fs::File, revisado: &'a Archivo) -> Self {
        Self {
            archivo,
            revisado,
            huella: Sha256::new(),
            escritos: 0,
            desbordado: false,
        }
    }

    /// Cierra el archivo y dice si es lo que se revisó.
    fn terminar(self) -> NbResult<()> {
        let Self {
            archivo,
            revisado,
            huella,
            escritos,
            ..
        } = self;

        // Al disco antes de dar nada por instalado: el renombrado que viene
        // después es atómico, pero no sirve de nada si lo que renombra son bytes
        // que se perderían con un corte de luz.
        archivo.sync_all().map_err(|_| NbError::StorageFailure)?;

        let calculada = hex(&huella.finalize());
        if escritos != revisado.bytes || calculada != revisado.sha256 {
            // Ni la ruta ni las huellas: basta con saber que no coincidió.
            tracing::warn!(
                // no es del usuario: el nombre del archivo sale del catálogo de modelos.
                archivo = revisado.nombre,
                "lo descargado no coincide con lo que se revisó"
            );
            return Err(NbError::DocumentCorrupted);
        }
        Ok(())
    }
}

impl Write for Verificador<'_> {
    fn write(&mut self, bloque: &[u8]) -> std::io::Result<usize> {
        let tras = self.escritos.saturating_add(bloque.len() as u64);
        if tras > self.revisado.bytes {
            self.desbordado = true;
            return Err(std::io::Error::other("más bytes de los revisados"));
        }
        let escritos = self.archivo.write(bloque)?;
        self.huella.update(&bloque[..escritos]);
        self.escritos += escritos as u64;
        Ok(escritos)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.archivo.flush()
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn borrar_si_existe(ruta: &Path) -> NbResult<()> {
    match std::fs::remove_dir_all(ruta) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(NbError::StorageFailure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalogo::GRANITE_107M_MULTILINGUAL;

    /// Un modelo que cabe justo no se descarga: dejar el disco a cero es poner en
    /// riesgo el diario de la base, que escribe al lado.
    #[test]
    fn un_modelo_que_cabe_justo_no_cabe_sin_su_margen() {
        let modelo = &GRANITE_107M_MULTILINGUAL;
        let necesario = modelo.disco();

        assert!(!cabe(necesario, modelo), "sin margen no puede caber");
        assert!(!cabe(necesario - 1, modelo));
        assert!(cabe(necesario + necesario / MARGEN_DE_ESPACIO, modelo));
        assert!(cabe(u64::MAX, modelo));
    }

    /// El plazo crece con el tamaño: un plazo fijo cortaría una descarga lenta
    /// pero viva de doscientos megabytes, que es la que más cuesta repetir.
    #[test]
    fn el_plazo_de_un_archivo_grande_admite_una_conexion_lenta() {
        let grande = plazo_para(214_000_232);
        // A 25 KiB por segundo son unas dos horas y media.
        assert!(grande >= Duration::from_secs(8_000));
        assert_eq!(plazo_para(697), PLAZO_MINIMO);
    }

    #[test]
    fn la_huella_se_escribe_como_la_publica_el_catalogo() {
        let calculada = hex(&Sha256::digest(b""));
        assert_eq!(
            calculada,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
