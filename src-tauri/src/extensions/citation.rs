//! Formatos de cita aportados por una extensión.
//!
//! # Por qué la extensión no consulta nada
//!
//! Porque el sentido del dato va **del núcleo hacia ella**. Las referencias
//! salen de las anotaciones y del catálogo —que son del usuario—, las compone el
//! anfitrión, y la extensión recibe lo justo para escribir el archivo. No elige
//! qué documentos entran, no consulta la biblioteca y no escribe en disco:
//! recibe una lista y devuelve texto.
//!
//! Es la misma frontera que en los formatos de documento, y por el mismo
//! motivo: darle a un invitado la capacidad de recorrer la biblioteca sería la
//! segunda clase de extensión con permisos elevados que se descartó.
//!
//! # Por qué devuelve el archivo entero y no una línea por referencia
//!
//! Porque un archivo bibliográfico tiene cabecera, orden y separadores propios
//! de cada formato. Trocearlo obligaría al anfitrión a saber de BibTeX, de RIS
//! y de todo lo que venga después, que es justo lo que no debe saber.

use std::sync::Arc;

use wasmtime::component::{Component, Linker};
use wasmtime::Store;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::error::{NbError, NbResult};

use super::store::Store as Almacen;
use super::{plazo_en_latidos, Cupo, Limites, Motor, Subsistema};

wasmtime::component::bindgen!({
    path: "wit",
    world: "citation-provider",
});

pub use self::exports::Reference as Referencia;

/// Los tipos exportados por el mundo, con nombre corto.
mod exports {
    pub use super::Reference;
}

/// La clave con la que un manifiesto declara que aporta formatos de cita.
pub const CLAVE: &str = "citation";

/// Lo que el almacén del invitado lleva dentro: WASI vacío y el cupo.
///
/// Ni base de datos ni red, **porque su mundo no las ofrece**. Darle un estado
/// más rico del que puede usar sería dejar preparada una capacidad que nadie
/// concedió.
struct EstadoDeCitas {
    wasi: WasiCtx,
    tabla: ResourceTable,
    cupo: Cupo,
}

impl WasiView for EstadoDeCitas {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.tabla,
        }
    }
}

/// Un formato de cita que aporta una extensión.
#[derive(Debug, Clone)]
pub struct FormatoDeCita {
    /// De quién es.
    pub extension: String,
    /// El identificador del formato dentro de la extensión.
    pub id: String,
    /// El nombre que se ve. Lo pone su autor y no se traduce.
    pub name: String,
    /// La extensión de archivo que le corresponde, sin el punto.
    pub file_extension: String,
}

/// Compone las referencias de una edición: su ficha y sus anotaciones.
///
/// **Una referencia por anotación**, más una del documento entero cuando no hay
/// ninguna. Es lo que hace útil exportar: quien cita un libro cita un pasaje, y
/// una entrada por documento obligaría a volver a buscar dónde estaba.
///
/// La clave se compone del primer apellido, el año y un número: es lo que un
/// gestor bibliográfico espera y lo que hace que dos citas del mismo libro no
/// se pisen.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el catálogo o las
/// anotaciones.
pub fn referencias(conn: &rusqlite::Connection, edition_id: &str) -> NbResult<Vec<Referencia>> {
    use crate::db::catalog::{Contributor, Edition};

    let (titulo, editorial, fecha) = conn
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
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => NbError::DocumentNotFound,
            _ => NbError::StorageFailure,
        })?;

    let autores: Vec<String> = Contributor::list_for_edition(conn, edition_id)?
        .into_iter()
        .filter(|(_, papel, _)| papel == "author")
        .map(|(contribuidor, _, _)| contribuidor.name)
        .collect();
    let identificadores: Vec<String> = Edition::identifiers(conn, edition_id)?
        .into_iter()
        .map(|(esquema, valor)| format!("{esquema}:{valor}"))
        .collect();

    // El año, no la fecha entera: una cita lleva año, y recortar aquí evita que
    // cada formato tenga que saber recortar.
    let anio = fecha
        .as_deref()
        .and_then(|fecha| fecha.get(..4).map(str::to_owned))
        .filter(|a| a.chars().all(|c| c.is_ascii_digit()));

    // Un documento con DOI es un artículo; sin él, un libro. Es la distinción
    // que el catálogo permite hacer hoy sin inventarse nada.
    let clase = if identificadores.iter().any(|i| i.starts_with("doi:")) {
        "article"
    } else {
        "book"
    };

    let apellido = autores
        .first()
        .and_then(|autor| autor.split([',', ' ']).next().map(str::to_owned))
        .unwrap_or_else(|| "anon".to_owned());
    let raiz = format!(
        "{}{}",
        apellido.to_lowercase(),
        anio.clone().unwrap_or_default()
    );

    let plantilla = |clave: String, quote: Option<String>, note: Option<String>| Referencia {
        key: clave,
        kind: clase.to_owned(),
        title: titulo.clone(),
        authors: autores.clone(),
        year: anio.clone(),
        publisher: editorial.clone(),
        identifiers: identificadores.clone(),
        quote,
        note,
    };

    let anotaciones = crate::annotations::list(conn, edition_id)?;
    if anotaciones.is_empty() {
        return Ok(vec![plantilla(raiz, None, None)]);
    }

    Ok(anotaciones
        .into_iter()
        .enumerate()
        .map(|(indice, anotacion)| {
            plantilla(
                format!("{raiz}-{}", indice + 1),
                anotacion.target.text.map(|t| t.highlight),
                anotacion.body,
            )
        })
        .collect())
}

/// Los formatos de cita que aportan las extensiones instaladas y activas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil del almacén.
pub fn formatos(subsistema: &Subsistema) -> NbResult<Vec<FormatoDeCita>> {
    let inventario = subsistema.registro()?;
    let mut salida = Vec::new();

    for instalada in inventario.instaladas().iter().filter(|e| e.enabled) {
        let Some(manifiesto) = inventario.manifiesto(&instalada.id) else {
            continue;
        };
        for formato in &manifiesto.contributes.citation_formats {
            salida.push(FormatoDeCita {
                extension: instalada.id.clone(),
                id: formato.id.clone(),
                name: formato.name.clone(),
                file_extension: formato.file_extension.clone(),
            });
        }
    }
    Ok(salida)
}

/// Pide a una extensión que componga el archivo con estas referencias.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la extensión no está, está apagada,
///   no declara ese formato o su componente no cumple el mundo.
/// - [`NbError::ExtensionLimitExceeded`] si se pasa de su plazo o su memoria.
/// - [`NbError::FormatUnsupported`] si dice que no conoce ese formato.
pub fn componer(
    motor: &Arc<Motor>,
    almacen: &Almacen,
    formato: &FormatoDeCita,
    referencias: &[Referencia],
    limites: Limites,
) -> NbResult<String> {
    let inventario = super::registry::Registro::cargar(almacen)?;
    if !inventario.esta_activa(&formato.extension) {
        return Err(NbError::ComponentUnavailable);
    }
    let bytes = inventario.modulo_de(almacen, &formato.extension, CLAVE)?;

    let Ok(componente) = Component::new(motor.interno(), &bytes) else {
        tracing::warn!(extension = %formato.extension, "el componente de citas no compila");
        return Err(NbError::ComponentUnavailable);
    };

    let mut enlazador: Linker<EstadoDeCitas> = Linker::new(motor.interno());
    wasmtime_wasi::p2::add_to_linker_sync(&mut enlazador).map_err(|error| {
        tracing::error!(?error, "no se pudo preparar WASI para las citas");
        NbError::ComponentUnavailable
    })?;

    let mut estado = Store::new(
        motor.interno(),
        EstadoDeCitas {
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

    let Ok(instancia) = CitationProvider::instantiate(&mut estado, &componente, &enlazador) else {
        tracing::warn!(extension = %formato.extension, "el componente no cumple el mundo de citas");
        return Err(NbError::ComponentUnavailable);
    };

    match instancia.call_format_citations(&mut estado, &formato.id, referencias) {
        Ok(Ok(texto)) => Ok(texto),
        Ok(Err(_)) => Err(NbError::FormatUnsupported),
        Err(_) => {
            tracing::warn!(extension = %formato.extension, "el formato de citas se pasó de sus límites");
            Err(NbError::ExtensionLimitExceeded)
        }
    }
}
