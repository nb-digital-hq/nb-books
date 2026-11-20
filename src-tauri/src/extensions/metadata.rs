//! Una fuente de metadatos aportada por una extensión.
//!
//! # Qué puede hacer, y qué no
//!
//! Puede **proponer**. No puede escribir.
//!
//! Todo lo que devuelve pasa por `metadata::propose` con origen `Online`, igual
//! que el proveedor del núcleo, así que una edición manual gana
//! siempre y lo que el usuario ya rechazó no se vuelve a aplicar. Que la fuente
//! sea de un tercero no le da más autoridad que a la nuestra: le da la misma.
//!
//! # Por qué esta sí sale a la red y la de formato no
//!
//! Porque consultar una fuente en línea **es** salir a la red, y un formato no
//! lo necesita. Su mundo importa `host`, y la salida va por la puerta de
//! `net.rs` con el origen de esa extensión: sin permiso concedido recibe
//! `denied` y no llega ni al transporte.
//!
//! # De dónde salió cada candidato
//!
//! Cada candidato lleva su procedencia. Quien elige entre varias fichas tiene
//! que poder saber cuál viene del catálogo de siempre y cuál de una extensión
//! que instaló la semana pasada: sin eso, la decisión se toma a ciegas.

use std::path::Path;
use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::Store;

use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::metadata_online::Candidate as Ficha;
use crate::net::Network;

use super::world::Anfitrion;
use super::{plazo_en_latidos, Limites, Motor, Subsistema};

wasmtime::component::bindgen!({
    path: "wit",
    world: "metadata-provider",
    // El mundo de metadatos importa `host`, así que reutiliza su
    // implementación: la de `world::Anfitrion`, que ya sale por `net.rs` con el
    // origen de la extensión. Una segunda implementación sería una segunda
    // superficie de auditoría para la misma puerta.
    with: { "nb:extension/host": super::world::nb::extension::host },
});

/// La clave con la que un manifiesto declara que aporta metadatos.
///
/// Es la misma que documenta la sección 3 de `plugin-system.md`.
pub const CLAVE: &str = "metadata";

/// Una fuente de metadatos que aporta una extensión.
pub struct FuenteDeExtension {
    /// De quién es. Viaja con cada candidato.
    pub extension: String,
    componente: Vec<u8>,
    motor: Arc<Motor>,
    limites: Limites,
}

impl FuenteDeExtension {
    /// Consulta la fuente por lo que ya se sabe del documento.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si el componente no cumple su mundo;
    /// [`NbError::ExtensionLimitExceeded`] si se pasa de sus límites.
    pub fn buscar(
        &self,
        db: &Database,
        red: &Arc<Network>,
        titulo: &str,
        autor: Option<&str>,
        identificadores: &[String],
    ) -> NbResult<Vec<Ficha>> {
        let Ok(componente) = Component::new(self.motor.interno(), &self.componente) else {
            tracing::warn!(extension = %self.extension, "el componente de metadatos no compila");
            return Err(NbError::ComponentUnavailable);
        };

        let mut enlazador: Linker<Anfitrion> = Linker::new(self.motor.interno());
        wasmtime_wasi::p2::add_to_linker_sync(&mut enlazador).map_err(|error| {
            tracing::error!(?error, "no se pudo preparar WASI para la fuente");
            NbError::ComponentUnavailable
        })?;
        MetadataProvider::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
            &mut enlazador,
            |estado| estado,
        )
        .map_err(|error| {
            tracing::error!(?error, "no se pudo enlazar el mundo de metadatos");
            NbError::ComponentUnavailable
        })?;

        let anfitrion = Anfitrion::para(db.clone(), Arc::clone(red), &self.extension, self.limites);
        let mut almacen = Store::new(self.motor.interno(), anfitrion);
        almacen.limiter(Anfitrion::cupo);
        almacen.set_epoch_deadline(plazo_en_latidos(self.limites.tiempo));

        let Ok(instancia) = MetadataProvider::instantiate(&mut almacen, &componente, &enlazador)
        else {
            tracing::warn!(extension = %self.extension, "el componente no cumple el mundo de metadatos");
            return Err(NbError::ComponentUnavailable);
        };

        // Los identificadores viajan, y hasta entonces iban
        // vacíos. El mundo los declaraba y nadie los rellenaba, así
        // que una fuente que consultara por DOI -que es lo que distingue a una
        // fuente académica de una general- no podía existir. Lo destapó
        // escribir la primera: una API que nadie ha usado de verdad no está
        // probada, solo publicada.
        let consulta = Query {
            title: titulo.to_owned(),
            author: autor.map(ToOwned::to_owned),
            identifiers: identificadores.to_vec(),
        };

        match instancia.call_lookup(&mut almacen, &consulta) {
            Ok(Ok(candidatos)) => Ok(candidatos
                .into_iter()
                .map(|c| Ficha {
                    title: c.title,
                    authors: c.authors,
                    published_date: c.published_date,
                    publisher: c.publisher,
                    language: c.language,
                    cover_url: None,
                    source: Some(self.extension.clone()),
                })
                .collect()),
            // No poder consultar no es una avería de la aplicación: es una
            // fuente que hoy no contesta. Se anota y se sigue con las demás.
            Ok(Err(motivo)) => {
                tracing::info!(extension = %self.extension, ?motivo, "una fuente no pudo consultar");
                Ok(Vec::new())
            }
            Err(_) => {
                tracing::warn!(extension = %self.extension, "una fuente se pasó de sus límites");
                Err(NbError::ExtensionLimitExceeded)
            }
        }
    }
}

/// Las fuentes que aportan las extensiones instaladas y activas.
///
/// Una fuente rota se omite y se anota: que una extensión impida consultar el
/// catálogo de siempre sería el fallo de un componente tumbando a otro.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil del almacén.
pub fn fuentes(subsistema: &Subsistema, limites: Limites) -> NbResult<Vec<FuenteDeExtension>> {
    let inventario = subsistema.registro()?;
    let mut salida = Vec::new();

    for instalada in inventario.instaladas().iter().filter(|e| e.enabled) {
        let Some(manifiesto) = inventario.manifiesto(&instalada.id) else {
            continue;
        };
        let Some(modulo) = manifiesto.contributes.providers.get(CLAVE) else {
            continue;
        };
        let carpeta = subsistema.almacen.ruta_de(instalada);
        // La ruta la escribe quien publica la extensión.
        let Ok(resuelta) = crate::security::resolve_within(&carpeta, Path::new(modulo)) else {
            tracing::warn!(extension = %instalada.id, "una fuente apunta fuera de su carpeta");
            continue;
        };
        let Ok(bytes) = std::fs::read(&resuelta) else {
            tracing::warn!(extension = %instalada.id, "una fuente no tiene su módulo");
            continue;
        };

        salida.push(FuenteDeExtension {
            extension: instalada.id.clone(),
            componente: bytes,
            motor: Arc::clone(&subsistema.motor),
            limites,
        });
    }
    Ok(salida)
}
