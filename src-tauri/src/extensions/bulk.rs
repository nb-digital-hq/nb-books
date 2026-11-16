//! Corrección de metadatos por lotes.
//!
//! # Por qué esto es reversible sin tener que deshacer nada
//!
//! Porque **no escribe**. La extensión propone y el anfitrión registra cada
//! propuesta por el mismo camino que las de cualquier proveedor: si hay una
//! edición manual, gana la manual y lo demás queda como sugerencia; si no la
//! hay, también queda como sugerencia, porque un lote que escribe mil
//! documentos en silencio es exactamente lo que no se puede deshacer.
//!
//! El criterio de la ficha dice "reversible". La forma barata de cumplirlo
//! habría sido guardar los valores anteriores para poder restaurarlos; la forma
//! que no se rompe es no cambiarlos.
//!
//! # Qué recibe la extensión
//!
//! Lo que ya está en el catálogo local de los documentos que **el anfitrión
//! elige**. No recorre la biblioteca, no pregunta por lo que no se le da y no
//! sale a la red: su mundo no importa `host`.

use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::Store;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::error::{NbError, NbResult};
use crate::metadata::{self, Source};

use super::store::Store as Almacen;
use super::{plazo_en_latidos, Cupo, Limites, Motor};

wasmtime::component::bindgen!({
    path: "wit",
    world: "bulk-metadata",
});

pub use self::Document as Documento;
pub use self::Suggestion as Sugerencia;

/// La clave con la que un manifiesto declara que revisa metadatos por lotes.
pub const CLAVE: &str = "bulk-metadata";

/// Cuántos documentos van en cada llamada al invitado.
///
/// El lote existe por dos motivos y ninguno es el rendimiento: el invitado tiene
/// un plazo por llamada, así que mil documentos de una vez lo agotarían y se
/// perdería el trabajo entero; y con lotes, un fallo se lleva cien documentos y
/// no mil.
pub const TAMANO_DEL_LOTE: usize = 100;

struct EstadoDelLote {
    wasi: WasiCtx,
    tabla: ResourceTable,
    cupo: Cupo,
}

impl WasiView for EstadoDelLote {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.tabla,
        }
    }
}

/// Cómo fue la revisión de un lote.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct BulkReviewSummary {
    /// Cuántos documentos se le pasaron a la extensión.
    pub reviewed: u32,
    /// Cuántos cambios propuso.
    pub suggested: u32,
    /// Cuántas propuestas quedaron registradas para que alguien las mire.
    ///
    /// Puede ser menor que `suggested`: una propuesta que repite lo que ya está
    /// guardado, o que ya se rechazó antes, no se vuelve a ofrecer.
    pub recorded: u32,
    /// Cuántos lotes no se pudieron revisar, por plazo o por fallo del módulo.
    ///
    /// Se cuentan y no se ocultan: decir "revisados mil" cuando cien se
    /// perdieron sería mentir sobre lo que se ha mirado.
    pub failed_batches: u32,
}

/// Compone lo que se le pasa a la extensión sobre un documento.
///
/// Devuelve `None` si la edición no existe: un identificador que ya no está no
/// es un error del lote, es un documento que se borró mientras se revisaba.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn documento_de(conn: &rusqlite::Connection, edition_id: &str) -> NbResult<Option<Documento>> {
    use crate::db::catalog::Contributor;

    let fila = conn
        .query_row(
            "SELECT title, publisher, published_date FROM editions WHERE id = ?1",
            [edition_id],
            |fila| {
                Ok((
                    fila.get::<_, String>(0)?,
                    fila.get::<_, Option<String>>(1)?,
                    fila.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map(Some)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            _ => Err(NbError::StorageFailure),
        })?;

    let Some((titulo, editorial, fecha)) = fila else {
        return Ok(None);
    };

    let autores = Contributor::list_for_edition(conn, edition_id)?
        .into_iter()
        .filter(|(_, papel, _)| papel == "author")
        .map(|(contribuidor, _, _)| contribuidor.name)
        .collect();

    Ok(Some(Documento {
        id: edition_id.to_owned(),
        title: titulo,
        authors: autores,
        publisher: editorial,
        year: fecha
            .as_deref()
            .and_then(|f| f.get(..4).map(str::to_owned))
            .filter(|a| a.chars().all(|c| c.is_ascii_digit())),
    }))
}

/// Revisa una lista de documentos con la extensión que la aporte.
///
/// Los recorre en lotes de [`TAMANO_DEL_LOTE`]. Un lote que falle no detiene los
/// demás: se cuenta y se sigue, porque perder novecientos documentos porque cien
/// dieron problema no ayuda a nadie.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la extensión no está activa o no
///   declara este proveedor.
/// - [`NbError::StorageFailure`] si no se pueden registrar las propuestas.
pub fn revisar(
    conn: &rusqlite::Connection,
    motor: &Arc<Motor>,
    almacen: &Almacen,
    extension: &str,
    documentos: &[Documento],
    limites: Limites,
) -> NbResult<BulkReviewSummary> {
    let inventario = super::registry::Registro::cargar(almacen)?;
    if !inventario.esta_activa(extension) {
        return Err(NbError::ComponentUnavailable);
    }
    let bytes = inventario.modulo_de(almacen, extension, CLAVE)?;

    let Ok(componente) = Component::new(motor.interno(), &bytes) else {
        tracing::warn!(%extension, "el componente del lote no compila");
        return Err(NbError::ComponentUnavailable);
    };

    let mut resumen = BulkReviewSummary {
        reviewed: u32::try_from(documentos.len()).unwrap_or(u32::MAX),
        ..BulkReviewSummary::default()
    };

    for lote in documentos.chunks(TAMANO_DEL_LOTE) {
        match sugerencias_de(motor, &componente, lote, limites) {
            Ok(sugerencias) => {
                resumen.suggested += u32::try_from(sugerencias.len()).unwrap_or(u32::MAX);
                resumen.recorded += registrar(conn, lote, &sugerencias)?;
            }
            Err(_) => resumen.failed_batches += 1,
        }
    }
    Ok(resumen)
}

/// Lo que la extensión propone para un lote.
fn sugerencias_de(
    motor: &Arc<Motor>,
    componente: &Component,
    lote: &[Documento],
    limites: Limites,
) -> NbResult<Vec<Sugerencia>> {
    let mut enlazador: Linker<EstadoDelLote> = Linker::new(motor.interno());
    wasmtime_wasi::p2::add_to_linker_sync(&mut enlazador).map_err(|error| {
        tracing::error!(?error, "no se pudo preparar WASI para el lote");
        NbError::ComponentUnavailable
    })?;

    let mut estado = Store::new(
        motor.interno(),
        EstadoDelLote {
            wasi: WasiCtxBuilder::new().build(),
            tabla: ResourceTable::new(),
            cupo: Cupo {
                memoria: limites.memoria,
                se_le_nego_crecer: false,
            },
        },
    );
    estado.limiter(|e| &mut e.cupo);
    estado.set_epoch_deadline(plazo_en_latidos(limites.tiempo));

    let Ok(instancia) = BulkMetadata::instantiate(&mut estado, componente, &enlazador) else {
        return Err(NbError::ComponentUnavailable);
    };

    match instancia.call_suggest(&mut estado, lote) {
        Ok(Ok(sugerencias)) => Ok(sugerencias),
        Ok(Err(_)) => Err(NbError::ComponentUnavailable),
        Err(_) => Err(NbError::ExtensionLimitExceeded),
    }
}

/// Registra las propuestas y devuelve cuántas quedaron anotadas.
///
/// Una sugerencia sobre un documento que no iba en el lote **se descarta**: la
/// extensión solo puede opinar sobre lo que se le dio, y aceptar un
/// identificador que no mandamos sería dejarla escribir donde quisiera.
fn registrar(
    conn: &rusqlite::Connection,
    lote: &[Documento],
    sugerencias: &[Sugerencia],
) -> NbResult<u32> {
    let mut anotadas = 0;

    for sugerencia in sugerencias {
        if !lote.iter().any(|d| d.id == sugerencia.document) {
            tracing::info!("una extensión propuso sobre un documento que no se le dio");
            continue;
        }
        // Proponer vacío borraría lo que sí se sabía.
        if sugerencia.value.trim().is_empty() {
            continue;
        }

        let campo = match sugerencia.field {
            Field::Title => metadata::Field::Title,
            Field::Subtitle => metadata::Field::Subtitle,
            Field::Publisher => metadata::Field::Publisher,
        };
        // `Source::Online` y no un origen propio: para el modelo de procedencia
        // esto es exactamente lo mismo que un proveedor en línea —algo
        // automático que propone—, y una edición manual le gana igual. Un origen
        // nuevo habría pedido migración y no habría cambiado ni una decisión.
        let resultado = metadata::sugerir(
            conn,
            &sugerencia.document,
            campo,
            Source::Online,
            Some(sugerencia.value.trim()),
        )?;
        if resultado {
            anotadas += 1;
        }
    }
    Ok(anotadas)
}
