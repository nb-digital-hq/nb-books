//! El registro de lo que se importó, y de lo que quedó fuera.
//!
//! Contrato: `docs/architecture/api/schemas.md`. Esquema:
//! `src-tauri/migrations/V18__import_runs.sql`, y las copias exactas en
//! `V37__copias_exactas.sql`.
//!
//! # Qué se perdía
//!
//! Una importación de ciento cincuenta mil archivos dura horas, y la aplicación
//! **no recordaba que la estabas haciendo**. Reanudar era gratis por la identidad
//! de contenido —lo ya importado se reconoce y se salta, y hay prueba con mil
//! archivos— pero solo si te acordabas de volver a lanzarla sobre la misma
//! carpeta. Y el informe de lo que se rechazó vivía en el panel de esa ejecución
//! y en ningún sitio más: cerrarlo era perderlo.
//!
//! Las dos son la pregunta J12, y ninguna es un defecto de lo que se hizo: son
//! alcance que nadie había decidido.
//!
//! # Por qué una fila abierta, y no una bandera
//!
//! Una ejecución con `finished_at` nulo significa "se empezó y no se terminó", y
//! esa señal **funciona si el proceso muere**. Una bandera hay que apagarla, y al
//! caerse no se apaga nada; una fila que nadie cerró queda abierta sola. Terminar,
//! cancelar y fallar la cierran; morir la deja abierta, que es la verdad.
//!
//! # Los dos topes, y por qué existen
//!
//! Sin ellos, esto sería un defecto de crecimiento disfrazado de función. Una
//! importación de la biblioteca de prueba puede rechazar decenas de miles de
//! archivos, y guardar una fila por cada uno convierte el informe en el problema.
//! Se guardan las primeras [`RECHAZOS_POR_EJECUCION`] y **el recuento sigue siendo
//! el de verdad**, que es lo que hace que la cifra no mienta aunque la lista esté
//! acotada. Y se conservan las últimas [`EJECUCIONES_GUARDADAS`], porque un
//! historial de importaciones no es una función que nadie haya pedido.

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::catalog::StorageMode;
use crate::error::{NbError, NbResult};

/// Cuántos rechazos se guardan de una misma ejecución.
///
/// Mil. Por encima, la lista deja de ser algo que alguien lee y pasa a ser un
/// volcado; y quien importa ciento cincuenta mil archivos con veinte mil
/// rechazos necesita saber **cuántos y de qué clase**, no los veinte mil de uno
/// en uno. El recuento total se guarda aparte y no depende de este tope.
pub const RECHAZOS_POR_EJECUCION: usize = 1_000;

/// Cuántas copias exactas se guardan de una misma ejecución, por lo mismo que los
/// rechazos: el recuento es el de verdad, y la lista, lo que alguien lee.
pub const COPIAS_POR_EJECUCION: usize = 1_000;

/// Cuántas ejecuciones se conservan.
///
/// Veinte. Lo que hace falta es la última —para poder continuarla— y unas cuantas
/// hacia atrás para saber qué se hizo; un historial completo sería una función
/// que nadie ha pedido, con su pantalla y su mantenimiento.
pub const EJECUCIONES_GUARDADAS: usize = 20;

/// Una importación, terminada o a medias.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ImportRun {
    pub id: String,
    /// La carpeta que se eligió. Es lo que hay que ofrecer para continuar.
    pub root: String,
    /// `referenced` o `managed`: continuar tiene que hacer lo que se pidió, no lo
    /// que esté por defecto ahora.
    pub storage_mode: String,
    pub started_at: String,
    /// Nulo mientras corre, y también si el proceso murió sin cerrarla.
    pub finished_at: Option<String>,
    pub processed: u32,
    pub imported: u32,
    pub already_in_library: u32,
    /// Cuántos se rechazaron en total, que puede ser más de los que se guardaron.
    pub rejected: u32,
    /// Cuántos eran otra copia, en otra ruta, de algo que ya estaba. No
    /// entran en `already_in_library`: aquellos son el mismo archivo visto otra
    /// vez, y estos, el mismo libro dos veces en el disco.
    pub duplicates: u32,
    pub cancelled: bool,
}

/// Una copia exacta que no entró, y de qué es copia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ImportDuplicate {
    /// La ruta de la copia, relativa a la carpeta elegida.
    pub path: String,
    /// El título de lo que ya estaba: es como lo reconoce quien decide cuál borrar.
    pub original_title: String,
    /// Dónde está lo que ya estaba, relativo a su raíz.
    pub original_path: String,
    pub created_at: String,
}

/// Un archivo que no entró, y por qué.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ImportRejection {
    /// La ruta del archivo, para poder ir a mirarlo.
    pub path: String,
    /// El código canónico de `error.rs`. **Nunca el mensaje**: se traduce al
    /// pintarlo, que es la regla del contrato de errores.
    pub code: String,
    pub created_at: String,
}

fn fila_a_ejecucion(fila: &Row<'_>) -> rusqlite::Result<ImportRun> {
    Ok(ImportRun {
        id: fila.get("id")?,
        root: fila.get("root")?,
        storage_mode: fila.get("storage_mode")?,
        started_at: fila.get("started_at")?,
        finished_at: fila.get("finished_at")?,
        processed: fila.get::<_, i64>("processed")?.try_into().unwrap_or(0),
        imported: fila.get::<_, i64>("imported")?.try_into().unwrap_or(0),
        already_in_library: fila
            .get::<_, i64>("already_in_library")?
            .try_into()
            .unwrap_or(0),
        rejected: fila.get::<_, i64>("rejected")?.try_into().unwrap_or(0),
        duplicates: fila.get::<_, i64>("duplicates")?.try_into().unwrap_or(0),
        cancelled: fila.get::<_, i64>("cancelled")? != 0,
    })
}

const COLUMNAS: &str = "id, root, storage_mode, started_at, finished_at, \
                        processed, imported, already_in_library, rejected, duplicates, \
                        cancelled";

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo en el registro de importaciones");
    NbError::StorageFailure
}

/// Apunta que empieza una importación y devuelve su identificador.
///
/// Se llama **antes** de recorrer nada: si el recorrido tarda diez minutos y el
/// proceso muere dentro, la fila ya está abierta y se puede ofrecer continuar.
/// Apuntarla al terminar no serviría para nada, porque terminar es justo lo que
/// no ocurre en el caso que esto existe para cubrir.
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir la fila.
pub fn abrir(conn: &Connection, root: &str, modo: StorageMode) -> NbResult<String> {
    let id: String = conn
        .query_row(
            "INSERT INTO import_runs (root, storage_mode) VALUES (?1, ?2) RETURNING id",
            rusqlite::params![root, modo.as_str()],
            |fila| fila.get(0),
        )
        .map_err(|e| fallo(&e))?;
    podar(conn)?;
    Ok(id)
}

/// Apunta un archivo que no entró.
///
/// Por encima del tope se deja de guardar y **no se avisa**: el recuento que
/// viaja en la ejecución sigue siendo el de verdad, y devolver un error aquí
/// convertiría un tope de almacenamiento en un fallo de importación.
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta o la escritura fallan.
pub fn anotar_rechazo(conn: &Connection, run_id: &str, path: &str, code: &str) -> NbResult<()> {
    let guardados: i64 = conn
        .query_row(
            "SELECT count(*) FROM import_rejections WHERE run_id = ?1",
            [run_id],
            |fila| fila.get(0),
        )
        .map_err(|e| fallo(&e))?;
    if guardados >= i64::try_from(RECHAZOS_POR_EJECUCION).unwrap_or(i64::MAX) {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO import_rejections (run_id, path, code) VALUES (?1, ?2, ?3)",
        rusqlite::params![run_id, path, code],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Apunta una copia exacta que no entró, con el mismo tope silencioso que
/// los rechazos y por el mismo motivo.
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta o la escritura fallan.
pub fn anotar_copia(conn: &Connection, run_id: &str, path: &str, file_id: &str) -> NbResult<()> {
    let guardadas: i64 = conn
        .query_row(
            "SELECT count(*) FROM import_duplicates WHERE run_id = ?1",
            [run_id],
            |fila| fila.get(0),
        )
        .map_err(|e| fallo(&e))?;
    if guardadas >= i64::try_from(COPIAS_POR_EJECUCION).unwrap_or(i64::MAX) {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO import_duplicates (run_id, path, file_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![run_id, path, file_id],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Lo que salió de una importación.
///
/// Va en una estructura y no en cuatro parámetros seguidos porque son cuatro
/// recuentos del mismo tipo: intercambiar dos al llamar compila igual y produce
/// un informe que miente, y nadie lo notaría hasta mirarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resumen {
    pub procesados: u32,
    pub importados: u32,
    pub ya_estaban: u32,
    /// El total, no el número de filas que se llegaron a guardar.
    pub rechazados: u32,
    /// El total de copias exactas, igual.
    pub copias: u32,
    pub cancelada: bool,
}

/// Cierra la ejecución con lo que salió.
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn cerrar(conn: &Connection, run_id: &str, resumen: Resumen) -> NbResult<()> {
    conn.execute(
        "UPDATE import_runs
            SET finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                processed = ?2, imported = ?3, already_in_library = ?4,
                rejected = ?5, cancelled = ?6, duplicates = ?7
          WHERE id = ?1",
        rusqlite::params![
            run_id,
            i64::from(resumen.procesados),
            i64::from(resumen.importados),
            i64::from(resumen.ya_estaban),
            i64::from(resumen.rechazados),
            i64::from(resumen.cancelada),
            i64::from(resumen.copias)
        ],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// La importación que quedó a medias, si la hay.
///
/// Solo puede haber una: el testigo de importación no deja arrancar dos a la vez.
/// Si hubiera más de una abierta —porque el proceso murió dos veces— se devuelve
/// la más reciente, que es la que alguien recuerda haber lanzado.
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn pendiente(conn: &Connection) -> NbResult<Option<ImportRun>> {
    let sql = format!(
        "SELECT {COLUMNAS} FROM import_runs
          WHERE finished_at IS NULL
          ORDER BY started_at DESC LIMIT 1"
    );
    conn.query_row(&sql, [], fila_a_ejecucion)
        .optional()
        .map_err(|e| fallo(&e))
}

/// Lo que una ejecución dejó fuera, en el orden en que ocurrió.
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn rechazos(
    conn: &Connection,
    run_id: &str,
    limite: u32,
    desde: u32,
) -> NbResult<Vec<ImportRejection>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT path, code, created_at FROM import_rejections
          WHERE run_id = ?1 ORDER BY id LIMIT {limite} OFFSET {desde}"
        ))
        .map_err(|e| fallo(&e))?;
    let filas = stmt
        .query_map([run_id], |fila| {
            Ok(ImportRejection {
                path: fila.get("path")?,
                code: fila.get("code")?,
                created_at: fila.get("created_at")?,
            })
        })
        .map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// Las copias exactas de una ejecución, en el orden en que aparecieron, con el
/// título y la ruta de lo que ya estaba.
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn copias(
    conn: &Connection,
    run_id: &str,
    limite: u32,
    desde: u32,
) -> NbResult<Vec<ImportDuplicate>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT d.path, e.title, f.relative_path, d.created_at
               FROM import_duplicates d
               JOIN files f ON f.id = d.file_id
               JOIN editions e ON e.id = f.edition_id
              WHERE d.run_id = ?1 ORDER BY d.id LIMIT {limite} OFFSET {desde}"
        ))
        .map_err(|e| fallo(&e))?;
    let filas = stmt
        .query_map([run_id], |fila| {
            Ok(ImportDuplicate {
                path: fila.get(0)?,
                original_title: fila.get(1)?,
                original_path: fila.get(2)?,
                created_at: fila.get(3)?,
            })
        })
        .map_err(|e| fallo(&e))?;
    filas
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))
}

/// La última ejecución terminada, para poder enseñar su informe.
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn ultima(conn: &Connection) -> NbResult<Option<ImportRun>> {
    let sql = format!("SELECT {COLUMNAS} FROM import_runs ORDER BY started_at DESC LIMIT 1");
    conn.query_row(&sql, [], fila_a_ejecucion)
        .optional()
        .map_err(|e| fallo(&e))
}

/// Da por zanjada una importación a medias sin continuarla.
///
/// Es lo que hace falta cuando alguien contesta que no quiere seguir: sin esto,
/// la aplicación volvería a ofrecérselo en cada arranque, que es exactamente el
/// aviso que la gente aprende a ignorar.
///
/// **Descartar lo que ya está cerrado no es un error**, por el mismo motivo que
/// cancelar una importación que acaba de terminar tampoco lo es: la interfaz
/// puede pulsarlo justo cuando otra cosa la cerró, y convertir eso en un fallo
/// solo produce manejo de errores que no arregla nada. La condición del `WHERE`
/// hace que la operación sea idempotente sin comprobar nada antes.
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn descartar(conn: &Connection, run_id: &str) -> NbResult<()> {
    conn.execute(
        "UPDATE import_runs
            SET finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), cancelled = 1
          WHERE id = ?1 AND finished_at IS NULL",
        [run_id],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Se queda con las últimas y borra el resto.
///
/// Los rechazos se van con su ejecución por la clave ajena en cascada, así que
/// aquí no hay que acordarse de borrarlos: acordarse es justo lo que falla.
fn podar(conn: &Connection) -> NbResult<()> {
    conn.execute(
        "DELETE FROM import_runs
          WHERE id NOT IN (
              SELECT id FROM import_runs ORDER BY started_at DESC LIMIT ?1
          )",
        [i64::try_from(EJECUCIONES_GUARDADAS).unwrap_or(i64::MAX)],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}
