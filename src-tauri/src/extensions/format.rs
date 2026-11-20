//! Un formato aportado por una extensión.
//!
//! # Qué aporta una extensión, y qué no
//!
//! Aporta **de qué está hecho un documento**: sus secciones, con su
//! identificador, su título y su texto. No aporta un lector.
//!
//! Esa frontera no es comodidad, es lo que sostiene tres garantías del núcleo.
//! El modelo de Locator de [ADR-004] —ninguna posición se guarda como entero—
//! lo aplica [`DocumentoDeSecciones`], no la extensión. El saneado del marcado
//! lo hace el núcleo. Y la negociación de capacidades del lector la decide el
//! núcleo. Si una extensión trajera su propio lector, las tres dependerían de
//! código de terceros.
//!
//! # Qué recibe, y qué no
//!
//! Recibe los **bytes** del documento. No recibe una ruta, no toca el disco y
//! no sabe dónde vive el archivo: eso lo hace el anfitrión, que ya sabe
//! canonicalizar y acotar. Y su mundo **no importa `host`**, así que un
//! proveedor de formato no puede salir a la red.
//!
//! [ADR-004]: ../../../docs/adr/decisions/ADR-004-reader-locator-model.md

use std::path::Path;
use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::Store;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::error::{NbError, NbResult};
use crate::reader::provider::{DocumentProvider, ProviderFactory, ReaderCapabilities};
use crate::reader::secciones::{DocumentoDeSecciones, Seccion};

use super::{plazo_en_latidos, Cupo, Limites, Motor};

wasmtime::component::bindgen!({
    path: "wit",
    world: "format-provider",
    // El mundo del formato no importa nada del anfitrión: lo único que se
    // genera aquí es su exportación.
});

/// Lo que el almacén de un proveedor de formato lleva dentro.
///
/// Solo WASI vacío y el cupo. No hay base de datos ni puerta de red **porque su
/// mundo no las ofrece**: darle un estado más rico del que su mundo puede usar
/// sería dejar preparada una capacidad que nadie concedió.
struct EstadoDeFormato {
    wasi: WasiCtx,
    tabla: ResourceTable,
    cupo: Cupo,
}

impl WasiView for EstadoDeFormato {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.tabla,
        }
    }
}

/// Fábrica de un formato que aporta una extensión.
pub struct FabricaDeExtension {
    tipo_de_medio: String,
    componente: Vec<u8>,
    motor: Arc<Motor>,
    limites: Limites,
    /// De quién es, para poder decirlo cuando algo falle.
    extension: String,
}

impl FabricaDeExtension {
    /// Prepara la fábrica con los bytes del componente ya leídos.
    #[must_use]
    pub fn nueva(
        extension: impl Into<String>,
        tipo_de_medio: impl Into<String>,
        componente: Vec<u8>,
        motor: Arc<Motor>,
        limites: Limites,
    ) -> Self {
        Self {
            tipo_de_medio: tipo_de_medio.into(),
            componente,
            motor,
            limites,
            extension: extension.into(),
        }
    }

    /// Pide a la extensión las secciones de un documento.
    fn secciones(&self, bytes: &[u8]) -> NbResult<Vec<Seccion>> {
        let Ok(componente) = Component::new(self.motor.interno(), &self.componente) else {
            tracing::warn!(extension = %self.extension, "el componente del formato no compila");
            return Err(NbError::ComponentUnavailable);
        };

        let mut enlazador: Linker<EstadoDeFormato> = Linker::new(self.motor.interno());
        // WASI vacío de capacidades, por lo mismo que en el otro camino: una
        // extensión escrita en Rust lo arrastra aunque su mundo no lo importe.
        wasmtime_wasi::p2::add_to_linker_sync(&mut enlazador).map_err(|error| {
            tracing::error!(?error, "no se pudo preparar WASI para el formato");
            NbError::ComponentUnavailable
        })?;

        let mut almacen = Store::new(
            self.motor.interno(),
            EstadoDeFormato {
                wasi: WasiCtxBuilder::new().build(),
                tabla: ResourceTable::new(),
                cupo: Cupo {
                    memoria: self.limites.memoria,
                    se_le_nego_crecer: false,
                },
            },
        );
        almacen.limiter(|estado| &mut estado.cupo);
        almacen.set_epoch_deadline(plazo_en_latidos(self.limites.tiempo));

        let Ok(instancia) = FormatProvider::instantiate(&mut almacen, &componente, &enlazador)
        else {
            tracing::warn!(extension = %self.extension, "el componente no cumple el mundo del formato");
            return Err(NbError::ComponentUnavailable);
        };

        match instancia.call_open_document(&mut almacen, bytes) {
            Ok(Ok(secciones)) => Ok(secciones
                .into_iter()
                .map(|s| Seccion {
                    href: s.href,
                    title: s.title,
                    text: s.text,
                })
                .collect()),
            // Que la extensión diga que el archivo está mal es un documento
            // corrupto, no un fallo de la extensión: son cosas distintas y la
            // interfaz reacciona distinto a cada una.
            Ok(Err(_)) => Err(NbError::DocumentCorrupted),
            Err(_) => {
                tracing::warn!(extension = %self.extension, "el formato se pasó de sus límites");
                Err(NbError::ExtensionLimitExceeded)
            }
        }
    }
}

impl ProviderFactory for FabricaDeExtension {
    fn media_type(&self) -> &str {
        &self.tipo_de_medio
    }

    fn open(&self, path: &Path) -> NbResult<Box<dyn DocumentProvider>> {
        // El anfitrión lee el archivo. La extensión recibe bytes y nunca una
        // ruta: no tiene disco, y no debe saber dónde vive nada.
        let bytes = crate::paths::read_bytes_noatime(path).map_err(|_| NbError::FileNotFound)?;
        let secciones = self.secciones(&bytes)?;
        if secciones.is_empty() {
            return Err(NbError::DocumentCorrupted);
        }

        // Las capacidades las decide el núcleo, no la extensión. Un formato de
        // secciones de texto se relee, se selecciona, se busca y se extrae; lo
        // que no hace es audio sincronizado, porque nada de esto lo trae.
        Ok(Box::new(DocumentoDeSecciones::new(
            &self.tipo_de_medio,
            secciones,
            ReaderCapabilities {
                text_selection: true,
                internal_search: true,
                reflow: true,
                typography: true,
                synchronized_audio: false,
                text_extraction: true,
            },
        )))
    }
}

/// El prefijo con el que un manifiesto declara que aporta un formato.
///
/// La clave de `contributes.providers` dice **qué** aporta, y para un formato
/// el qué incluye su tipo de medio: `format:text/x-lo-que-sea`. Va en la clave
/// y no en un campo nuevo porque el manifiesto ya tiene un sitio para "de esta
/// clase de proveedor, este módulo", y añadir una forma paralela para lo mismo
/// deja dos maneras de decir una cosa.
pub const PREFIJO: &str = "format:";

/// Registra los formatos que aporten las extensiones instaladas y activas.
///
/// Devuelve cuántos se registraron. **Esto es lo que hace que añadir un formato
/// sea instalar una extensión y no tocar el núcleo.**
///
/// Una extensión con un formato roto no impide arrancar: se omite y se anota.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil del almacén.
pub fn registrar(
    registro: &mut crate::reader::provider::ProviderRegistry,
    subsistema: &super::Subsistema,
    limites: Limites,
) -> NbResult<usize> {
    let inventario = subsistema.registro()?;
    let mut registrados = 0;

    for instalada in inventario.instaladas().iter().filter(|e| e.enabled) {
        let Some(manifiesto) = inventario.manifiesto(&instalada.id) else {
            continue;
        };
        let carpeta = subsistema.almacen.ruta_de(instalada);

        for (clave, modulo) in &manifiesto.contributes.providers {
            let Some(tipo_de_medio) = clave.strip_prefix(PREFIJO) else {
                continue;
            };
            // La ruta la escribe quien publica la extensión: se acota contra su
            // propia carpeta antes de leer nada.
            let resuelta = match crate::security::resolve_within(&carpeta, Path::new(modulo)) {
                Ok(ruta) => ruta,
                Err(_) => {
                    tracing::warn!(
                        extension = %instalada.id,
                        "un formato apunta fuera de su carpeta y se omite"
                    );
                    continue;
                }
            };
            let Ok(bytes) = std::fs::read(&resuelta) else {
                tracing::warn!(extension = %instalada.id, "un formato no tiene su módulo");
                continue;
            };

            registro.register(Box::new(FabricaDeExtension::nueva(
                &instalada.id,
                tipo_de_medio,
                bytes,
                Arc::clone(&subsistema.motor),
                limites,
            )));
            registrados += 1;
        }
    }
    Ok(registrados)
}
