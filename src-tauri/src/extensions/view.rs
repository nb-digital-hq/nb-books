//! Una vista aportada por una extensión.
//!
//! # Aporta contenido, no interfaz
//!
//! Una extensión devuelve **bloques tipados**: encabezados, párrafos, listas y
//! datos con su etiqueta. No devuelve marcado. La interfaz la pinta el núcleo
//! con sus propios componentes, igual que pinta las vistas nativas.
//!
//! Es la misma frontera que en los formatos, y por los mismos
//! motivos: el sistema de diseño, las garantías de accesibilidad y el
//! presupuesto de fotograma dejarían de ser nuestros si dependieran de marcado
//! de terceros. Y hay uno más: marcado ajeno habría que sanearlo, y sanear bien
//! es difícil.
//!
//! # Por qué una extensión en bucle no congela la aplicación
//!
//! Porque **no corre en el hilo de la interfaz**. Corre en el anfitrión, con el
//! plazo, y lo que la interfaz hace es esperar una respuesta que
//! llega o no llega. La alternativa que descarta esto es la de `plugin-system`
//! sección 2.2 —código de la extensión en un hilo del webview—, que exigiría un
//! aislamiento que aquí sale gratis.

use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::Store;

use crate::error::{NbError, NbResult};

use super::world::Anfitrion;
use super::{plazo_en_latidos, Limites, Motor};

wasmtime::component::bindgen!({
    path: "wit",
    world: "view-provider",
    with: { "nb:extension/host": super::world::nb::extension::host },
});

/// La clave con la que un manifiesto declara el módulo que compone sus vistas.
pub const CLAVE: &str = "view";

/// Compone una vista de una extensión.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el componente no cumple su mundo o la
/// extensión no aporta esa vista; [`NbError::ExtensionLimitExceeded`] si se
/// pasa de sus límites.
pub fn renderizar(
    motor: &Motor,
    wasm: &[u8],
    limites: Limites,
    anfitrion: Anfitrion,
    vista: &str,
) -> NbResult<View> {
    let Ok(componente) = Component::new(motor.interno(), wasm) else {
        return Err(NbError::ComponentUnavailable);
    };

    let mut enlazador: Linker<Anfitrion> = Linker::new(motor.interno());
    wasmtime_wasi::p2::add_to_linker_sync(&mut enlazador).map_err(|error| {
        tracing::error!(?error, "no se pudo preparar WASI para la vista");
        NbError::ComponentUnavailable
    })?;
    ViewProvider::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut enlazador, |estado| {
        estado
    })
    .map_err(|error| {
        tracing::error!(?error, "no se pudo enlazar el mundo de vistas");
        NbError::ComponentUnavailable
    })?;

    let mut almacen = Store::new(motor.interno(), anfitrion);
    almacen.limiter(Anfitrion::cupo);
    almacen.set_epoch_deadline(plazo_en_latidos(limites.tiempo));

    let Ok(instancia) = ViewProvider::instantiate(&mut almacen, &componente, &enlazador) else {
        tracing::warn!("el componente no cumple el mundo de vistas");
        return Err(NbError::ComponentUnavailable);
    };

    match instancia.call_render(&mut almacen, vista) {
        Ok(Ok(vista)) => Ok(vista),
        Ok(Err(motivo)) => {
            tracing::info!(?motivo, "una vista de extensión no se pudo componer");
            Err(NbError::ComponentUnavailable)
        }
        // Aquí acaba el bucle infinito: interrumpido por el plazo, y la
        // interfaz recibe un código en lugar de quedarse esperando para siempre.
        Err(_) => {
            tracing::warn!("una vista de extensión se pasó de sus límites");
            Err(NbError::ExtensionLimitExceeded)
        }
    }
}

/// Prepara el anfitrión para componer una vista.
#[must_use]
pub fn anfitrion_de(
    db: &crate::db::Database,
    red: &Arc<crate::net::Network>,
    extension: &str,
    limites: Limites,
) -> Anfitrion {
    Anfitrion::para(db.clone(), Arc::clone(red), extension, limites)
}
