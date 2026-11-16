//! El mundo WIT: el contrato tipado entre el anfitrión y una extensión.
//!
//! # Por qué un contrato y no un puñado de funciones
//!
//! Sin esto, lo que el anfitrión ofrece al invitado sería la suma de lo que
//! alguien fue exportando, y esa suma no se puede versionar ni prometer. El
//! mundo vive en `src-tauri/wit/nb-extension.wit`, se versiona con su paquete,
//! y **los enlaces de este lado se generan**: la macro los produce desde el
//! propio archivo, así que el contrato y el código no pueden separarse.
//!
//! # La regla que lo gobierna
//!
//! **Ninguna función se exporta al invitado sin un permiso detrás.** Hoy el
//! mundo tiene una sola función, `fetch`, y detrás está `network:fetch`. Que
//! siga siendo verdad lo comprueba `scripts/check-capacidades.mjs`, que cruza
//! las funciones del archivo WIT con la tabla de capacidades del manifiesto y
//! falla si alguna se queda sin permiso.
//!
//! Sin ese guion, añadir una función al mundo sería el equivalente exacto de
//! una salida de red sin consentimiento, y esa regla tiene mecanismo desde
//! la política de permisos.
//!
//! # Por la misma puerta
//!
//! `fetch` no abre una conexión: se la pide a `net.rs`, con el origen de la
//! extensión. De ahí sale que su salida aparezca en el registro con su nombre y
//! que revocarle el permiso la deje inerte, que es lo que construye este módulo.

use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::Store;

use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::net::{Network, Origin, Purpose};

use super::{clasificar, plazo_en_latidos, Cupo, Desenlace, Limites, Motor};

wasmtime::component::bindgen!({ path: "wit", world: "extension" });

// `FetchError` lo trae el mundo, que lo declara con `use host.{fetch-error}`.
use nb::extension::host::{Host, Response};

/// Lo que el anfitrión le presta a una extensión mientras corre.
///
/// Lleva el origen dentro y no lo recibe por parámetro a propósito: si la
/// extensión pudiera decir de parte de quién sale, el consentimiento por
/// extensión no valdría nada.
pub struct Anfitrion {
    db: Database,
    red: Arc<Network>,
    origen: Origin,
    cupo: Cupo,
    /// Contexto WASI **vacío de capacidades**, igual que el del camino de
    /// módulo suelto.
    ///
    /// Hace falta aunque el mundo no importe WASI, y eso lo enseñó el primer
    /// invitado: una extensión escrita en Rust arrastra la biblioteca estándar,
    /// y la biblioteca estándar enlaza `wasi:io/poll` aunque el código no lo
    /// use. Sin este contexto, **ningún componente real se instancia**.
    ///
    /// Que exista no le da nada: no hay directorio preabierto, ni entorno, ni
    /// argumentos, ni descriptores heredados. Lo que se le niega son las
    /// capacidades, que es donde vive la seguridad.
    wasi: WasiCtx,
    tabla: ResourceTable,
}

impl WasiView for Anfitrion {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.tabla,
        }
    }
}

impl Anfitrion {
    /// Prepara el anfitrión para una extensión concreta.
    ///
    /// Se lleva el pool de base y la puerta de red **en propiedad**, y no
    /// prestados, porque el almacén de Wasmtime exige un estado `'static`: una
    /// conexión prestada no puede vivir dentro de él. Abrir la conexión en cada
    /// llamada es lo que cuesta, y es lo que un pool hace barato.
    #[must_use]
    pub fn para(db: Database, red: Arc<Network>, id: &str, limites: Limites) -> Self {
        Self {
            db,
            red,
            origen: Origin::Extension(id.to_owned()),
            cupo: Cupo {
                memoria: limites.memoria,
                se_le_nego_crecer: false,
            },
            wasi: WasiCtxBuilder::new().build(),
            tabla: ResourceTable::new(),
        }
    }
}

impl Anfitrion {
    /// El cupo, para que quien cree el almacén se lo preste al limitador.
    ///
    /// `Anfitrion` no implementa el limitador directamente porque lleva una
    /// conexión de SQLite dentro y el limitador exige `Sync`.
    pub fn cupo(&mut self) -> &mut dyn wasmtime::ResourceLimiter {
        &mut self.cupo
    }
}

impl Host for Anfitrion {
    fn fetch(&mut self, url: String) -> Result<Response, FetchError> {
        let Ok(conn) = self.db.connection() else {
            return Err(FetchError::Failed);
        };
        match self
            .red
            .get_from(&conn, &self.origen, &url, Purpose::Extension)
        {
            Ok(respuesta) => Ok(Response {
                status: respuesta.status,
                body: respuesta.body,
            }),
            // El motivo se traduce a las tres cosas que una extensión puede
            // hacer distinto: pedir permiso, esperar a que haya red, o
            // reintentar. Un motivo único las obligaría a tratarlas igual.
            Err(NbError::ConsentRequired) => Err(FetchError::Denied),
            Err(NbError::NetworkDisabled) => Err(FetchError::Offline),
            Err(_) => Err(FetchError::Failed),
        }
    }
}

/// Añade el mundo al enlazador de componentes.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el mundo no se puede registrar, que
/// solo puede pasar si el archivo WIT y esta compilación se desincronizan.
pub fn enlazar(enlazador: &mut wasmtime::component::Linker<Anfitrion>) -> Result<(), NbError> {
    Extension::add_to_linker::<_, wasmtime::component::HasSelf<_>>(enlazador, |estado| estado)
        .map_err(|error| {
            tracing::error!(?error, "el mundo de extensiones no se pudo enlazar");
            NbError::ComponentUnavailable
        })
}

/// Cómo terminó ejecutar una orden de una extensión.
///
/// No deriva `PartialEq` porque los tipos que genera el mundo no lo derivan, y
/// forzarlo obligaría a escribir a mano lo que la macro produce: exactamente la
/// clase de copia que este contrato existe para no tener.
#[derive(Debug)]
pub enum Activacion {
    /// La orden se ejecutó. Lleva el texto que la extensión quiera enseñar, si
    /// quiere enseñar alguno.
    Hecha(Option<String>),
    /// La extensión no pudo, y dice por qué. Los motivos se conservan porque
    /// piden acciones distintas de quien la escribió.
    NoPudo(CommandError),
    /// La detuvo el anfitrión, o falló por su cuenta.
    Detenido(Desenlace),
}

/// Ejecuta una orden de un componente compilado contra el mundo.
///
/// Es el otro camino del anfitrión, junto al de módulo suelto, y
/// **conviene que convivan**: los módulos hostiles del banco prueban el camino
/// que usaría un atacante, y los componentes el camino por el que entran las
/// extensiones de verdad.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el mundo no se puede enlazar.
pub fn activar(
    motor: &Motor,
    wasm: &[u8],
    limites: Limites,
    anfitrion: Anfitrion,
    orden: &str,
) -> NbResult<Activacion> {
    let Ok(componente) = Component::new(motor.interno(), wasm) else {
        return Ok(Activacion::Detenido(Desenlace::NoEsUnModulo));
    };

    let mut enlazador: Linker<Anfitrion> = Linker::new(motor.interno());
    // WASI primero, y vacío de capacidades: sin él ningún componente compilado
    // desde Rust llega a instanciarse. Ver el comentario de `Anfitrion::wasi`.
    wasmtime_wasi::p2::add_to_linker_sync(&mut enlazador).map_err(|error| {
        tracing::error!(?error, "no se pudo preparar WASI para componentes");
        NbError::ComponentUnavailable
    })?;
    enlazar(&mut enlazador)?;

    let mut almacen = Store::new(motor.interno(), anfitrion);
    // El cupo se presta tal cual y `Anfitrion` no implementa el limitador: una
    // conexión de SQLite no es `Sync`, y el limitador lo exige. Prestar el
    // campo es además lo que hace el camino de módulo suelto.
    almacen.limiter(Anfitrion::cupo);
    almacen.set_epoch_deadline(plazo_en_latidos(limites.tiempo));

    let instanciada = Extension::instantiate(&mut almacen, &componente, &enlazador);
    if let Err(error) = &instanciada {
        tracing::warn!(?error, "el componente no cumple el mundo");
    }
    let Ok(instancia) = instanciada else {
        // No cumple el mundo: le falta una exportación, o importa algo que el
        // anfitrión no ofrece. Es el mismo rechazo que el de una capacidad
        // denegada en el camino de módulo suelto.
        return Ok(Activacion::Detenido(Desenlace::FalloDelModulo));
    };

    match instancia.call_run_command(&mut almacen, orden) {
        Ok(Ok(texto)) => Ok(Activacion::Hecha(texto)),
        Ok(Err(motivo)) => Ok(Activacion::NoPudo(motivo)),
        Err(error) => {
            let sin_memoria = almacen.data().cupo.se_le_nego_crecer;
            Ok(Activacion::Detenido(clasificar(Err(error), sin_memoria)))
        }
    }
}

/// Los nombres de las funciones que el anfitrión exporta al invitado.
///
/// Se declara aquí, junto a su implementación, y no en el guion: el guion lee
/// el archivo WIT, y esta lista es con lo que **la prueba** comprueba que el
/// archivo y el código dicen lo mismo. Si alguien añade una función al mundo y
/// no la implementa, no compila; si la implementa y no la declara aquí, la
/// prueba falla.
pub const FUNCIONES_DEL_ANFITRION: &[&str] = &["fetch"];

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_funcion_del_mundo_tiene_una_capacidad_que_la_respalda() {
        // La misma regla que comprueba el guion, comprobada también aquí sobre
        // la lista que acompaña a la implementación. Son dos sitios a propósito:
        // el guion vigila el archivo WIT, y esto vigila que la lista no se quede
        // vieja respecto a lo que el anfitrión implementa de verdad.
        assert_eq!(FUNCIONES_DEL_ANFITRION.len(), 1);
        assert!(FUNCIONES_DEL_ANFITRION.contains(&"fetch"));
        assert_eq!(super::super::manifest::CAPACIDADES.len(), 1);
    }
}
