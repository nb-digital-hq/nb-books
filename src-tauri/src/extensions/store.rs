//! El almacén inmutable de extensiones y sus generaciones.
//!
//! Las extensiones en un escritorio son propensas a la fricción de
//! dependencias: se actualiza una, algo deja de arrancar, y no hay forma de
//! volver. El modelo es el de NixOS y lo fija la sección 4 de
//! `plugin-system.md`: **el contenido es inmutable y direccionado por su hash**,
//! y lo que cambia es un puntero.
//!
//! ```text
//! <raíz>/
//!   store/<hash>/            el contenido, que nunca se modifica
//!   generations/<n>.json     el histórico, para poder volver
//!   active_profile.json      lo que está puesto ahora
//! ```
//!
//! # Dónde está exactamente el punto de compromiso
//!
//! En **una sola escritura**: la de `active_profile.json`, que se hace por
//! renombrado atómico con [`paths::write_atomic`]. Todo lo anterior —copiar el
//! contenido al almacén— es aditivo y no cambia lo que está puesto; todo lo
//! posterior —archivar la generación en el histórico— es derivado y se rehace
//! solo al abrir.
//!
//! Esa es la razón de que matar el proceso a mitad de una instalación no pueda
//! dejar estado corrupto, y de que se pueda comprobar en lugar de afirmarlo:
//! ver [`PUNTO_DE_MUERTE_ENV`].
//!
//! # Por qué el hash no lleva prefijo
//!
//! `library::hash_file` produce `sha256:<hex>`, que es lo correcto para un
//! valor que se guarda en la base. Aquí el hash **es el nombre de una carpeta**,
//! y los dos puntos no son un carácter válido en un nombre de archivo de
//! Windows. Se guarda el hexadecimal desnudo, y el prefijo se pone al mostrarlo.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::paths;

use super::manifest::{self, Manifest};

/// El perfil activo, el único archivo cuyo cambio compromete una instalación.
const PERFIL: &str = "active_profile.json";
/// Donde vive el contenido inmutable.
const CONTENIDO: &str = "store";
/// Donde vive el histórico.
const HISTORICO: &str = "generations";

/// Variable que hace morir al proceso en un punto concreto de la instalación.
///
/// Existe por el mismo motivo que `NB_BOOKS_TEST_DIR` en `paths`: hay una
/// propiedad que **no se puede comprobar de otra manera**. El criterio de
/// aceptación dice que una instalación interrumpida no deja estado
/// intermedio corrupto, y "interrumpida" significa que el proceso muere, sin
/// desenrollar la pila y sin ejecutar ningún `Drop`. Eso no se simula con un
/// error: hay que matar un proceso de verdad, y para matarlo en el sitio que
/// importa el sitio tiene que ser nombrable.
///
/// Valores admitidos: `tras-copiar` y `tras-activar`. Cualquier otro no hace
/// nada. En producción la variable no está puesta y esto no cuesta ni una rama
/// caliente: se consulta dos veces por instalación.
pub const PUNTO_DE_MUERTE_ENV: &str = "NB_BOOKS_MATAR_INSTALANDO";

/// Una extensión instalada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Installed {
    pub id: String,
    pub version: String,
    /// Hexadecimal del SHA-256 del contenido. Es el nombre de su carpeta.
    pub hash: String,
    /// Si sus contribuciones cuentan. Desactivada sigue instalada y conserva
    /// sus permisos: desactivar no es desinstalar, y confundirlos obligaría a
    /// volver a conceder para probar si una extensión era la que molestaba.
    #[serde(default = "activada_por_defecto")]
    pub enabled: bool,
    /// La clave con la que venía firmada cuando se instaló, si venía firmada.
    ///
    /// **Es lo que fija al autor.** Una firma válida solo dice que quien firmó
    /// tenía esa clave; lo que protege es que la de la actualización sea la
    /// misma. Va aquí, en el perfil, y no en la base de datos, porque tiene que
    /// volver atrás con la generación: si se restaura un perfil anterior, la
    /// clave fijada que vale es la de ese perfil.
    ///
    /// `Option` porque instalar sin firma desde el disco propio es legítimo, y
    /// distinguir "sin firma" de "firmada" es justo lo que permite negarse a
    /// actualizar con algo sin firmar lo que sí lo estaba.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Una extensión recién instalada está activada. Y un perfil escrito por una
/// versión anterior, que no llevaba el campo, también: lo que había instalado
/// estaba activo.
const fn activada_por_defecto() -> bool {
    true
}

/// Qué generación está puesta y qué lleva.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Profile {
    /// Número de generación. Es `u32` y no `u64` porque cruza el puente, y
    /// `ts-rs` traduce `u64` a `bigint`: un tipo que `JSON.parse` no produce, y
    /// por tanto una mentira que solo se ve el día que alguien lo usa.
    pub generation: u32,
    pub extensions: Vec<Installed>,
}

/// El almacén.
pub struct Store {
    raiz: PathBuf,
}

impl Store {
    /// Abre el almacén de esta instalación, en el directorio de datos.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el sistema no expone un directorio de
    /// datos o no se puede crear el almacén.
    pub fn del_usuario() -> NbResult<Self> {
        Self::abrir(paths::AppPaths::resolve()?.extensions())
    }

    /// Abre el almacén en una raíz, creándolo si hace falta.
    ///
    /// Al abrir se repara el histórico: si el archivo de la generación activa
    /// falta o no coincide, se reescribe desde el perfil. Esa es la mitad
    /// derivada del punto de compromiso, y repararla aquí es lo que permite que
    /// archivarla no tenga que ser atómico.
    pub fn abrir(raiz: impl Into<PathBuf>) -> NbResult<Self> {
        let raiz = raiz.into();
        for directorio in [raiz.join(CONTENIDO), raiz.join(HISTORICO)] {
            std::fs::create_dir_all(&directorio).map_err(|_| NbError::StorageFailure)?;
        }
        let almacen = Self { raiz };
        almacen.reparar()?;
        let perfil = almacen.perfil()?;
        if perfil.generation > 0 {
            almacen.archivar(&perfil)?;
        }
        Ok(almacen)
    }

    /// Repara un perfil ilegible volviendo a la última generación archivada.
    ///
    /// # Por qué recuperarse en vez de fallar
    ///
    /// Porque el histórico existe exactamente para esto. Un perfil que no se
    /// puede leer —un disco con errores, un apagado en el momento justo— no es
    /// motivo para perder lo que había instalado: el contenido sigue en el
    /// almacén y la última generación dice cuál era. Fallar aquí dejaría al
    /// usuario sin sus extensiones **y** sin explicación, que es la pérdida
    /// silenciosa que el producto existe para impedir.
    ///
    /// Si tampoco hay histórico, se arranca vacío: es lo único que queda, y es
    /// mejor que no arrancar.
    fn reparar(&self) -> NbResult<()> {
        let ruta = self.raiz.join(PERFIL);
        if !ruta.exists() || self.perfil().is_ok() {
            return Ok(());
        }
        tracing::warn!("el perfil de extensiones no se puede leer; se vuelve al histórico");

        let Some(ultima) = self.generaciones()?.last().copied() else {
            let _ = std::fs::remove_file(&ruta);
            return Ok(());
        };
        let perfil = self.leer_generacion(ultima)?;
        self.activar_sin_archivar(&perfil)
    }

    /// Lo que está puesto ahora.
    pub fn perfil(&self) -> NbResult<Profile> {
        let ruta = self.raiz.join(PERFIL);
        let Ok(texto) = std::fs::read_to_string(&ruta) else {
            return Ok(Profile::default());
        };
        serde_json::from_str(&texto).map_err(|_| NbError::StorageFailure)
    }

    /// Las generaciones archivadas, de menor a mayor.
    pub fn generaciones(&self) -> NbResult<Vec<u32>> {
        let mut numeros: Vec<u32> = std::fs::read_dir(self.raiz.join(HISTORICO))
            .map_err(|_| NbError::StorageFailure)?
            .filter_map(Result::ok)
            .filter_map(|entrada| {
                entrada
                    .file_name()
                    .to_str()?
                    .strip_suffix(".json")?
                    .parse()
                    .ok()
            })
            .collect();
        numeros.sort_unstable();
        Ok(numeros)
    }

    /// Instala un paquete —un directorio con su manifiesto y sus módulos— y
    /// deja una generación nueva puesta.
    ///
    /// # Errors
    ///
    /// [`NbError::FormatUnsupported`] si el manifiesto no es válido: el paquete
    /// no entra al almacén ni a medias. [`NbError::StorageFailure`] si el disco
    /// falla.
    pub fn instalar(&self, paquete: &Path) -> NbResult<Installed> {
        let manifiesto = self.leer_manifiesto(paquete)?;

        // Una extensión de una API que no hablamos no se instala. Podría
        // instalarse y fallar al invocarla, pero eso sería dejar entrar algo
        // que no puede funcionar y descubrirlo más tarde. La comprobación va
        // **aquí y no al leer**, porque leer el manifiesto de una extensión ya
        // instalada tiene que seguir funcionando: es lo que permite decir por
        // qué no se ejecuta, en lugar de que desaparezca de la lista.
        let (minima, maxima) = super::admitidas();
        match manifiesto.mayor_de_api() {
            Some(mayor) if (minima..=maxima).contains(&mayor) => {}
            _ => {
                tracing::info!("se intentó instalar una extensión de una API no soportada");
                return Err(NbError::ExtensionApiUnsupported);
            }
        }
        /*
         * **Cada módulo que declara tiene que venir dentro**.
         *
         * Antes esto no se miraba, y se notaba tarde: el equipo
         * instaló las extensiones propias desde su carpeta de fuentes, que trae
         * el manifiesto y el código pero no el `.wasm` compilado. Entraron sin
         * queja y cada uso respondía "no se encontró el archivo", sin decir cuál
         * ni por qué. Un paquete incompleto se rechaza aquí, antes de copiar
         * nada, igual que uno de una API que no se habla.
         */
        let faltan = modulos_que_faltan(paquete, &manifiesto);
        if !faltan.is_empty() {
            tracing::info!(
                faltan = faltan.len(),
                "se intentó instalar una extensión sin los módulos que declara"
            );
            return Err(NbError::ExtensionManifestInvalid);
        }

        // La firma, antes de copiar nada: rechazar después de copiar deja
        // basura en el almacén y obliga a recogerla.
        let firma = super::signature::verificar(paquete, &manifiesto.id, &manifiesto.version)?;
        let perfil_actual = self.perfil()?;
        let clave = clave_admitida(&perfil_actual, &manifiesto.id, &firma)?;

        let hash = huella_del_directorio(paquete)?;
        let destino = self.raiz.join(CONTENIDO).join(&hash);

        // Direccionado por contenido: si ya está, es bit a bit el mismo y no hay
        // nada que copiar. Reinstalar lo mismo no cuesta ni escribe.
        if !destino.exists() {
            copiar_y_publicar(paquete, &destino)?;
        }
        morir_si_toca("tras-copiar");

        let instalada = Installed {
            id: manifiesto.id.clone(),
            version: manifiesto.version.clone(),
            hash,
            enabled: true,
            key: clave,
        };

        let mut perfil = perfil_actual;
        perfil.extensions.retain(|e| e.id != instalada.id);
        perfil.extensions.push(instalada.clone());
        perfil.extensions.sort_by(|a, b| a.id.cmp(&b.id));
        perfil.generation = self.siguiente_generacion()?;

        self.activar(&perfil)?;
        Ok(instalada)
    }

    /// Quita una extensión dejando una generación nueva sin ella.
    ///
    /// No borra su contenido del almacén: si se vuelve atrás, tiene que seguir
    /// estando. Lo borra [`Store::recolectar`], y solo cuando ya no lo
    /// referencia ninguna generación.
    pub fn desinstalar(&self, id: &str) -> NbResult<bool> {
        let mut perfil = self.perfil()?;
        let antes = perfil.extensions.len();
        perfil.extensions.retain(|e| e.id != id);
        if perfil.extensions.len() == antes {
            return Ok(false);
        }
        perfil.generation = self.siguiente_generacion()?;
        self.activar(&perfil)?;
        Ok(true)
    }

    /// Activa o desactiva una extensión instalada.
    ///
    /// Deja una generación nueva, como cualquier otro cambio del perfil: así se
    /// puede volver atrás de haberla desactivado igual que de haberla
    /// instalado.
    ///
    /// Devuelve `false` si no estaba instalada, o si ya estaba como se pide.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede escribir.
    pub fn habilitar(&self, id: &str, activa: bool) -> NbResult<bool> {
        let mut perfil = self.perfil()?;
        let Some(extension) = perfil.extensions.iter_mut().find(|e| e.id == id) else {
            return Ok(false);
        };
        if extension.enabled == activa {
            return Ok(false);
        }
        extension.enabled = activa;
        perfil.generation = self.siguiente_generacion()?;
        self.activar(&perfil)?;
        Ok(true)
    }

    /// Vuelve a la generación anterior a la puesta.
    ///
    /// Es una sola escritura, así que no hay estado intermedio: o se está en la
    /// generación de antes o en la de ahora.
    ///
    /// # Errors
    ///
    /// [`NbError::ExtensionRollbackUnavailable`] si no hay ninguna generación
    /// anterior a la que volver.
    pub fn volver_atras(&self) -> NbResult<Profile> {
        let actual = self.perfil()?.generation;
        let anterior = self
            .generaciones()?
            .into_iter()
            .rfind(|n| *n < actual)
            .ok_or(NbError::ExtensionRollbackUnavailable)?;
        let perfil = self.leer_generacion(anterior)?;
        self.activar_sin_archivar(&perfil)?;
        Ok(perfil)
    }

    /// Borra del almacén lo que no referencia ninguna generación, y los restos
    /// de instalaciones interrumpidas.
    ///
    /// Devuelve cuántas carpetas se retiraron.
    pub fn recolectar(&self) -> NbResult<usize> {
        let mut vivos: BTreeSet<String> = self
            .perfil()?
            .extensions
            .into_iter()
            .map(|e| e.hash)
            .collect();
        for numero in self.generaciones()? {
            for extension in self.leer_generacion(numero)?.extensions {
                vivos.insert(extension.hash);
            }
        }

        let mut retiradas = 0;
        for entrada in std::fs::read_dir(self.raiz.join(CONTENIDO))
            .map_err(|_| NbError::StorageFailure)?
            .filter_map(Result::ok)
        {
            let nombre = entrada.file_name().to_string_lossy().into_owned();
            if vivos.contains(&nombre) {
                continue;
            }
            std::fs::remove_dir_all(entrada.path()).map_err(|_| NbError::StorageFailure)?;
            retiradas += 1;
        }
        Ok(retiradas)
    }

    /// El manifiesto de una extensión instalada, leído de su carpeta.
    ///
    /// Hace falta para comparar lo que pedía con lo que pide una versión nueva,
    /// que es de lo que vive el consentimiento renovado.
    ///
    /// # Errors
    ///
    /// [`NbError::DocumentNotFound`] si su contenido ya no está;
    /// [`NbError::FormatUnsupported`] si el manifiesto dejó de ser válido.
    pub fn manifiesto_de(&self, instalada: &Installed) -> NbResult<Manifest> {
        self.leer_manifiesto(&self.ruta_de(instalada))
    }

    /// Dónde vive el contenido de una extensión instalada.
    pub fn ruta_de(&self, instalada: &Installed) -> PathBuf {
        self.raiz.join(CONTENIDO).join(&instalada.hash)
    }

    // --- interior ---------------------------------------------------------

    /// Los módulos que declara una extensión ya instalada y no están en su
    /// carpeta. Vacío en una instalación sana.
    ///
    /// Existe por las que entraron antes de que la instalación lo comprobara:
    /// Ajustes tiene que poder decir que una está incompleta en lugar de que
    /// cada uso falle con "no se encontró el archivo".
    #[must_use]
    pub fn modulos_que_faltan(&self, instalada: &Installed, manifiesto: &Manifest) -> Vec<String> {
        modulos_que_faltan(&self.ruta_de(instalada), manifiesto)
    }

    /// Qué módulos le faltan a un paquete **que todavía no está instalado**.
    ///
    /// Existe para poder decirlo con nombres en lugar de rechazar el paquete con
    /// un "manifiesto inválido" que no es cierto: el manifiesto está bien, lo que
    /// falta es el `.wasm` que declara. Se reportó el 2026-09-18
    /// intentando instalar las extensiones propias desde su carpeta de fuentes.
    #[must_use]
    pub fn le_faltan_modulos(paquete: &Path, manifiesto: &Manifest) -> Vec<String> {
        modulos_que_faltan(paquete, manifiesto)
    }

    fn leer_manifiesto(&self, paquete: &Path) -> NbResult<Manifest> {
        let texto = std::fs::read_to_string(paquete.join(manifest::ARCHIVO))
            .map_err(|_| NbError::DocumentNotFound)?;
        manifest::parse(&texto).map_err(|defectos| {
            // El detalle de cada defecto es de quien escribió la extensión, y
            // llega a la interfaz por `manifest::parse`, no por aquí. En el
            // registro solo se dice cuántos, porque un campo de manifiesto puede
            // llevar el título de un documento dentro.
            tracing::warn!(
                defectos = defectos.len(),
                "manifiesto de extensión inválido"
            );
            NbError::FormatUnsupported
        })
    }

    fn siguiente_generacion(&self) -> NbResult<u32> {
        let maximo = self
            .generaciones()?
            .last()
            .copied()
            .unwrap_or(0)
            .max(self.perfil()?.generation);
        Ok(maximo + 1)
    }

    /// El punto de compromiso, y después el histórico.
    fn activar(&self, perfil: &Profile) -> NbResult<()> {
        self.activar_sin_archivar(perfil)?;
        morir_si_toca("tras-activar");
        self.archivar(perfil)
    }

    fn activar_sin_archivar(&self, perfil: &Profile) -> NbResult<()> {
        let texto = serde_json::to_string_pretty(perfil).map_err(|_| NbError::StorageFailure)?;
        paths::write_atomic(&self.raiz.join(PERFIL), |temporal| {
            std::fs::write(temporal, &texto).map_err(|_| NbError::StorageFailure)
        })
    }

    fn archivar(&self, perfil: &Profile) -> NbResult<()> {
        let ruta = self
            .raiz
            .join(HISTORICO)
            .join(format!("{}.json", perfil.generation));
        if self.leer_generacion(perfil.generation).ok().as_ref() == Some(perfil) {
            return Ok(());
        }
        let texto = serde_json::to_string_pretty(perfil).map_err(|_| NbError::StorageFailure)?;
        paths::write_atomic(&ruta, |temporal| {
            std::fs::write(temporal, &texto).map_err(|_| NbError::StorageFailure)
        })
    }

    fn leer_generacion(&self, numero: u32) -> NbResult<Profile> {
        let ruta = self.raiz.join(HISTORICO).join(format!("{numero}.json"));
        let texto = std::fs::read_to_string(ruta).map_err(|_| NbError::DocumentNotFound)?;
        serde_json::from_str(&texto).map_err(|_| NbError::StorageFailure)
    }
}

/// Mata el proceso si la variable de prueba señala este punto.
///
/// `abort` y no `exit`: lo que hay que reproducir es una muerte sin desenrollar
/// la pila y sin ejecutar ningún `Drop`, que es lo que hace un corte de luz o un
/// `kill -9`. Un `exit` limpio no probaría lo mismo.
fn morir_si_toca(punto: &str) {
    if std::env::var(PUNTO_DE_MUERTE_ENV).as_deref() == Ok(punto) {
        std::process::abort();
    }
}

/// Copia el paquete a un temporal y lo publica de un renombrado.
///
/// El renombrado es lo que impide que nadie vea una carpeta del almacén a
/// medio copiar: o está entera o no está. En el mismo directorio, porque entre
/// sistemas de archivos distintos un renombrado vuelve a ser una copia.
fn copiar_y_publicar(origen: &Path, destino: &Path) -> NbResult<()> {
    let padre = destino.parent().ok_or(NbError::PathNotAllowed)?;
    let temporal = padre.join(format!(".parcial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temporal);
    if let Err(error) = copiar_recursivo(origen, &temporal) {
        let _ = std::fs::remove_dir_all(&temporal);
        return Err(error);
    }
    std::fs::rename(&temporal, destino).map_err(|_| {
        let _ = std::fs::remove_dir_all(&temporal);
        NbError::StorageFailure
    })
}

fn copiar_recursivo(origen: &Path, destino: &Path) -> NbResult<()> {
    std::fs::create_dir_all(destino).map_err(|_| NbError::StorageFailure)?;
    for entrada in std::fs::read_dir(origen)
        .map_err(|_| NbError::StorageFailure)?
        .filter_map(Result::ok)
    {
        let hijo = destino.join(entrada.file_name());
        if entrada.path().is_dir() {
            copiar_recursivo(&entrada.path(), &hijo)?;
        } else {
            std::fs::copy(entrada.path(), &hijo).map_err(|_| NbError::StorageFailure)?;
        }
    }
    Ok(())
}

/// La huella de un directorio: SHA-256 de sus rutas y su contenido, **sin la
/// firma**.
///
/// Entra la ruta relativa además de los bytes, y en orden estable, porque dos
/// paquetes con los mismos archivos en sitios distintos **no son el mismo
/// paquete**. Sin la ruta, mover un módulo de carpeta no cambiaría la huella y
/// el almacén serviría el contenido viejo creyendo que es el nuevo.
pub fn huella_del_directorio(raiz: &Path) -> NbResult<String> {
    let mut archivos = Vec::new();
    recorrer(raiz, raiz, &mut archivos)?;
    archivos.sort();

    let mut hasher = Sha256::new();
    for relativa in archivos {
        hasher.update(relativa.as_bytes());
        hasher.update([0]);
        let bytes = std::fs::read(raiz.join(&relativa)).map_err(|_| NbError::StorageFailure)?;
        hasher.update(bytes.len().to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut salida, byte| {
            use std::fmt::Write as _;
            let _ = write!(salida, "{byte:02x}");
            salida
        }))
}

fn recorrer(raiz: &Path, actual: &Path, salida: &mut Vec<String>) -> NbResult<()> {
    for entrada in std::fs::read_dir(actual)
        .map_err(|_| NbError::StorageFailure)?
        .filter_map(Result::ok)
    {
        let ruta = entrada.path();
        if ruta.is_dir() {
            recorrer(raiz, &ruta, salida)?;
        } else {
            let relativa = ruta
                .strip_prefix(raiz)
                .map_err(|_| NbError::PathNotAllowed)?
                .to_string_lossy()
                // Separador estable: si no, el mismo paquete tendría huellas
                // distintas en Windows y en Linux, y el almacén dejaría de ser
                // direccionado por contenido en cuanto se cruzara de sistema.
                .replace('\\', "/");
            // La firma del autor no entra en la huella, y tiene que ser así:
            // firma **sobre** la huella, así que si entrara se estaría
            // firmando algo que cambia al firmarlo. Dejarla fuera hace además
            // que un paquete tenga la misma dirección de contenido antes y
            // después de firmarse, y que volver a firmarlo no lo mueva de
            // sitio.
            if relativa != super::signature::ARCHIVO {
                salida.push(relativa);
            }
        }
    }
    Ok(())
}

/// Qué clave queda fijada al instalar, o por qué no se instala.
///
/// Es la mitad de **R13** que no se puede comprobar mirando un paquete suelto:
/// que la actualización la firme quien firmó lo anterior.
///
/// # La regla, en una frase
///
/// Lo que ya está firmado solo se actualiza con algo firmado por la misma
/// clave.
///
/// # Los cuatro casos, y por qué cada uno es así
///
/// * **Ya había una clave fijada y la nueva firma es de esa clave.** Se
///   instala. Es el camino normal de una actualización de un autor.
/// * **Ya había una clave fijada y lo que llega no la trae** —porque viene sin
///   firmar, porque la firma no vale, o porque está firmada por otro—. **No se
///   instala.** Este es el caso que existe para el vector de la política: la
///   extensión benigna que se vuelve maliciosa en una actualización. Con la
///   clave fijada, conseguirlo pide robársela al autor.
/// * **No había clave y lo que llega trae una firma que no vale.** No se
///   instala. Traer una firma rota no es lo mismo que no traer ninguna: alguien
///   quiso firmar esto y lo que hay no corresponde al contenido.
/// * **No había clave y lo que llega no viene firmado.** Se instala, sin clave.
///   Instalar desde el disco propio algo sin firmar es legítimo —así entran las
///   extensiones que uno escribe— y prohibirlo no protegería de nada: quien
///   pone el paquete es quien lo instala.
fn clave_admitida(
    perfil: &Profile,
    id: &str,
    firma: &super::signature::SignatureState,
) -> NbResult<Option<String>> {
    use super::signature::SignatureState;

    let fijada = perfil
        .extensions
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.key.as_deref());

    match (fijada, firma) {
        (Some(fijada), SignatureState::Valida { clave }) if clave == fijada => {
            Ok(Some(clave.clone()))
        }
        (Some(_), _) => {
            tracing::warn!("se intentó actualizar una extensión con otra firma");
            Err(NbError::ExtensionSignatureMismatch)
        }
        (None, SignatureState::Invalida { .. }) => {
            tracing::warn!("se intentó instalar una extensión con la firma rota");
            Err(NbError::ExtensionSignatureMismatch)
        }
        (None, SignatureState::Valida { clave }) => Ok(Some(clave.clone())),
        (None, SignatureState::SinFirma) => Ok(None),
    }
}

/// Los módulos que el manifiesto declara y la carpeta no trae.
///
/// Un módulo cuenta como presente si está **dentro** de la carpeta -la ruta la
/// escribe quien publica, y se acota igual que al invocarlo- y empieza por la
/// firma de WebAssembly. Un archivo con otro contenido y el nombre bueno es tan
/// inútil como uno que no está, y descubrirlo al invocarlo es descubrirlo tarde.
fn modulos_que_faltan(carpeta: &Path, manifiesto: &Manifest) -> Vec<String> {
    const FIRMA_WASM: &[u8; 4] = b"\0asm";
    manifiesto
        .contributes
        .providers
        .values()
        .filter(|declarado| {
            let Ok(ruta) = crate::security::resolve_within(carpeta, Path::new(declarado)) else {
                return true;
            };
            let mut cabecera = [0_u8; 4];
            let leido = std::fs::File::open(&ruta)
                .and_then(|mut archivo| std::io::Read::read_exact(&mut archivo, &mut cabecera));
            leido.is_err() || &cabecera != FIRMA_WASM
        })
        .cloned()
        .collect()
}
