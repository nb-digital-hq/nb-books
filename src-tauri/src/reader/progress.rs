//! Progreso de lectura: dónde se quedó el usuario, y cómo se guarda sin castigar
//! al disco.
//!
//! # Por qué hay antirrebote
//!
//! Leer mueve la posición constantemente: cada línea que baja es una posición
//! nueva. Escribir cada una en SQLite convertiría pasar una página en decenas de
//! transacciones, y en un portátil eso se nota en la batería antes que en el
//! reloj.
//!
//! # Por qué el antirrebote no puede perder la última
//!
//! Un antirrebote ingenuo guarda "cada cinco segundos" y se olvida de lo que
//! quedaba pendiente al cerrar. El resultado es justo lo que este módulo
//! existe para impedir: el usuario cierra el libro donde lo dejó y al reabrirlo
//! aparece cinco segundos más atrás. Aquí **cerrar vacía lo pendiente**, y por
//! eso `close_document` no es una cortesía de la interfaz sino parte del
//! contrato.
//!
//! Queda una ventana: si la aplicación muere de golpe (no se cierra, muere), se
//! pierde lo apuntado desde la última escritura, como mucho [`INTERVALO`]. Se
//! dice aquí en lugar de aparentar que no existe; cerrarla del todo obliga a
//! escribir en cada movimiento, que es lo que el antirrebote viene a evitar.
//!
//! Contratos: `docs/architecture/reader-contracts.md`, sección 6, y
//! `docs/architecture/api/schemas.md`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension};

use super::locator::Locator;
use super::provider::Precision;
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};

/// Tiempo mínimo entre dos escrituras del progreso de un mismo documento.
///
/// Cinco segundos: bastante para que una lectura normal escriba unas pocas veces
/// por capítulo, y poco para que un cierre inesperado no cueste más que volver a
/// buscar el párrafo donde se estaba.
pub const INTERVALO: Duration = Duration::from_secs(5);

/// Perfil por defecto mientras no haya varios.
pub const PERFIL: &str = "default";

/// El progreso guardado de una edición.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub edition_id: String,
    pub profile: String,
    pub locator: Locator,
    /// Con qué precisión se resolvió la última vez que se restauró.
    pub precision: Precision,
    pub updated_at: String,
}

/// Una sesión de lectura registrada.
///
/// Se llama así y no `ReadingSession` porque aquello es el documento abierto en
/// memoria y esto es la fila que queda cuando se cierra. Confundirlos haría creer
/// que cerrar la aplicación borra el registro.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedSession {
    pub id: String,
    pub edition_id: String,
    pub started_at: String,
    /// Nulo mientras sigue abierta, y también si la aplicación murió de golpe.
    pub ended_at: Option<String>,
    pub start_locator: Locator,
    pub end_locator: Option<Locator>,
}

fn precision_como_texto(precision: Precision) -> &'static str {
    match precision {
        Precision::Exact => "exact",
        Precision::Quote => "quote",
        Precision::Approximate => "approximate",
    }
}

fn precision_desde_texto(texto: &str) -> Option<Precision> {
    match texto {
        "exact" => Some(Precision::Exact),
        "quote" => Some(Precision::Quote),
        "approximate" => Some(Precision::Approximate),
        _ => None,
    }
}

/// Lee el progreso guardado de una edición.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla o si lo almacenado no es un
/// Locator legible. Un locator ilegible **no** se trata como "sin progreso": eso
/// devolvería al usuario al principio del libro sin decirle nada, que es
/// exactamente la pérdida silenciosa que el producto no admite.
pub fn load(conn: &Connection, edition_id: &str, profile: &str) -> NbResult<Option<Progress>> {
    let fila: Option<(String, String, String)> = conn
        .query_row(
            "SELECT locator, precision, updated_at FROM progress
             WHERE edition_id = ?1 AND profile = ?2",
            [edition_id, profile],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo leer el progreso");
            NbError::StorageFailure
        })?;

    let Some((locator, precision, updated_at)) = fila else {
        return Ok(None);
    };

    let locator: Locator = serde_json::from_str(&locator).map_err(|error| {
        tracing::error!(error = %error, "el progreso almacenado no es un locator legible");
        NbError::StorageFailure
    })?;

    Ok(Some(Progress {
        edition_id: edition_id.to_owned(),
        profile: profile.to_owned(),
        locator,
        precision: precision_desde_texto(&precision).ok_or_else(|| {
            tracing::error!("precisión fuera de la lista admitida");
            NbError::StorageFailure
        })?,
        updated_at,
    }))
}

/// Escribe el progreso, sustituyendo el que hubiera.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn save(
    conn: &Connection,
    edition_id: &str,
    profile: &str,
    locator: &Locator,
    precision: Precision,
) -> NbResult<()> {
    let serializado = serde_json::to_string(locator).map_err(|_| NbError::Internal)?;

    conn.execute(
        "INSERT INTO progress (edition_id, profile, locator, precision)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (edition_id, profile) DO UPDATE
           SET locator = excluded.locator,
               precision = excluded.precision,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![
            edition_id,
            profile,
            serializado,
            precision_como_texto(precision)
        ],
    )
    .map_err(|error| {
        tracing::error!(error = %error, "no se pudo guardar el progreso");
        NbError::StorageFailure
    })?;

    Ok(())
}

/// Abre una sesión registrada y devuelve su identificador.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn open_session(conn: &Connection, edition_id: &str, desde: &Locator) -> NbResult<String> {
    let serializado = serde_json::to_string(desde).map_err(|_| NbError::Internal)?;

    conn.query_row(
        "INSERT INTO reading_sessions (edition_id, profile, start_locator)
         VALUES (?1, ?2, ?3) RETURNING id",
        rusqlite::params![edition_id, PERFIL, serializado],
        |row| row.get(0),
    )
    .map_err(|error| {
        tracing::error!(error = %error, "no se pudo registrar la sesión de lectura");
        NbError::StorageFailure
    })
}

/// Cierra una sesión registrada.
///
/// Cerrar una que no existe o que ya estaba cerrada no es un error: la sesión
/// pudo quedar sin cerrar por un cierre inesperado, y convertir eso en un fallo
/// castigaría al usuario por algo que ya le pasó.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn close_session(conn: &Connection, session_id: &str, hasta: &Locator) -> NbResult<()> {
    let serializado = serde_json::to_string(hasta).map_err(|_| NbError::Internal)?;

    conn.execute(
        "UPDATE reading_sessions
            SET ended_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), end_locator = ?2
          WHERE id = ?1 AND ended_at IS NULL",
        rusqlite::params![session_id, serializado],
    )
    .map_err(|error| {
        tracing::error!(error = %error, "no se pudo cerrar la sesión de lectura");
        NbError::StorageFailure
    })?;

    Ok(())
}

/// Sesiones registradas de una edición, de la más reciente a la más antigua.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn sessions(conn: &Connection, edition_id: &str) -> NbResult<Vec<RecordedSession>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, edition_id, started_at, ended_at, start_locator, end_locator
               FROM reading_sessions WHERE edition_id = ?1
              ORDER BY started_at DESC, id DESC",
        )
        .map_err(|_| NbError::StorageFailure)?;

    let filas = stmt
        .query_map([edition_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|_| NbError::StorageFailure)?;

    filas
        .map(|fila| {
            let (id, edition_id, started_at, ended_at, inicio, fin) =
                fila.map_err(|_| NbError::StorageFailure)?;
            Ok(RecordedSession {
                id,
                edition_id,
                started_at,
                ended_at,
                start_locator: serde_json::from_str(&inicio)
                    .map_err(|_| NbError::StorageFailure)?,
                end_locator: fin
                    .map(|f| serde_json::from_str(&f))
                    .transpose()
                    .map_err(|_| NbError::StorageFailure)?,
            })
        })
        .collect()
}

/// Lo que espera para escribirse.
struct Pendiente {
    locator: Locator,
    precision: Precision,
    ultima_escritura: Instant,
    /// Hay algo apuntado que todavía no está en la base.
    sucio: bool,
}

/// Guarda el progreso agrupando escrituras.
///
/// No lleva ningún hilo de fondo. La decisión de escribir se toma en el momento
/// de apuntar: si desde la última escritura de **ese** documento ha pasado el
/// intervalo, se escribe; si no, queda apuntado y se escribirá con la siguiente
/// que sí toque, o al cerrar. Un temporizador de fondo haría lo mismo con un
/// hilo más, y con un hilo más que apagar bien al salir.
pub struct ProgressTracker {
    db: Database,
    pendientes: Mutex<HashMap<(String, String), Pendiente>>,
    intervalo: Duration,
    /// A quién se anuncia cada escritura. Opcional porque las pruebas
    /// del antirrebote no lo necesitan, y un registrador sin bus sigue guardando.
    bus: Option<EventBus>,
}

impl ProgressTracker {
    /// Crea el registrador con el intervalo por defecto.
    #[must_use]
    pub fn new(db: Database) -> Self {
        Self::with_interval(db, INTERVALO)
    }

    /// Crea el registrador con un intervalo dado.
    ///
    /// Existe para las pruebas: el criterio de aceptación se comprueba con un
    /// intervalo de horas, para que **nada** se escriba salvo por el cierre, y
    /// con uno de cero, para comprobar lo contrario. Con el de producción, la
    /// prueba dependería de dormir cinco segundos y de que la máquina no vaya
    /// lenta ese día.
    #[must_use]
    pub fn with_interval(db: Database, intervalo: Duration) -> Self {
        Self {
            db,
            pendientes: Mutex::new(HashMap::new()),
            intervalo,
            bus: None,
        }
    }

    /// Anuncia cada escritura en el bus, como [`DomainEvent::ProgressSaved`].
    ///
    /// Es lo que hace que la biblioteca y Inicio muevan su barra mientras se lee,
    /// sin cerrar el lector: principio 2 de la sección 7 de la filosofía de
    /// producto. Se anuncia lo **escrito** y no lo apuntado, para que quien
    /// vuelva a preguntar lea lo mismo que dice el evento.
    #[must_use]
    pub fn con_bus(mut self, bus: EventBus) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Escribe y, si hay bus, lo anuncia. El único camino por el que este
    /// registrador toca la base, para que ninguna escritura se quede sin evento.
    fn escribir(
        &self,
        edition_id: &str,
        profile: &str,
        locator: &Locator,
        precision: Precision,
    ) -> NbResult<()> {
        let conn = self.db.connection()?;
        save(&conn, edition_id, profile, locator, precision)?;
        drop(conn);
        if let Some(bus) = &self.bus {
            bus.emit(DomainEvent::ProgressSaved {
                edition_id: edition_id.to_owned(),
                progression: locator.locations.total_progression,
            });
        }
        Ok(())
    }

    /// Apunta una posición. Escribe solo si toca.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la escritura falla, o [`NbError::Internal`]
    /// si el cerrojo quedó envenenado.
    pub fn note(&self, edition_id: &str, locator: &Locator, precision: Precision) -> NbResult<()> {
        let clave = (edition_id.to_owned(), PERFIL.to_owned());
        let mut pendientes = self.pendientes.lock().map_err(|_| NbError::Internal)?;

        let toca = match pendientes.get(&clave) {
            Some(previo) => previo.ultima_escritura.elapsed() >= self.intervalo,
            // La primera posición de un documento se escribe en el acto: es la
            // que dice "esto se ha empezado a leer", y perderla en un cierre
            // inesperado dejaría el documento como no empezado.
            None => true,
        };

        if toca {
            self.escribir(edition_id, PERFIL, locator, precision)?;
            pendientes.insert(
                clave,
                Pendiente {
                    locator: locator.clone(),
                    precision,
                    ultima_escritura: Instant::now(),
                    sucio: false,
                },
            );
            return Ok(());
        }

        if let Some(previo) = pendientes.get_mut(&clave) {
            previo.locator = locator.clone();
            previo.precision = precision;
            previo.sucio = true;
        }

        Ok(())
    }

    /// Escribe lo que quede pendiente de un documento y deja de seguirlo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la escritura falla, o [`NbError::Internal`]
    /// si el cerrojo quedó envenenado.
    pub fn flush(&self, edition_id: &str) -> NbResult<()> {
        let clave = (edition_id.to_owned(), PERFIL.to_owned());
        let mut pendientes = self.pendientes.lock().map_err(|_| NbError::Internal)?;

        if let Some(pendiente) = pendientes.remove(&clave) {
            if pendiente.sucio {
                self.escribir(edition_id, PERFIL, &pendiente.locator, pendiente.precision)?;
            }
        }

        Ok(())
    }

    /// Escribe lo pendiente de un documento **sin dejar de seguirlo**.
    ///
    /// Es lo que se hace al salir del lector sin cerrar el libro. El
    /// antirrebote deja apuntada la última posición y la escribe con el siguiente
    /// movimiento; mientras se lee da igual, pero quien va a la biblioteca a mitad
    /// de capítulo vería allí la barra de hace unos segundos, y en el lector la de
    /// ahora: dos cifras para el mismo dato. Cerrar no sirve, porque el libro
    /// sigue abierto y la sesión de lectura también.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la escritura falla, o [`NbError::Internal`]
    /// si el cerrojo quedó envenenado.
    pub fn write_pending(&self, edition_id: &str) -> NbResult<()> {
        let clave = (edition_id.to_owned(), PERFIL.to_owned());
        let mut pendientes = self.pendientes.lock().map_err(|_| NbError::Internal)?;

        if let Some(pendiente) = pendientes.get_mut(&clave) {
            if pendiente.sucio {
                self.escribir(edition_id, PERFIL, &pendiente.locator, pendiente.precision)?;
                pendiente.sucio = false;
                pendiente.ultima_escritura = Instant::now();
            }
        }

        Ok(())
    }

    /// Escribe todo lo pendiente. Se llama al cerrar la aplicación.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si alguna escritura falla, o
    /// [`NbError::Internal`] si el cerrojo quedó envenenado. Un fallo en uno no
    /// impide intentar los demás: perder un progreso es malo, perder todos los
    /// demás por culpa de ese es peor.
    pub fn flush_all(&self) -> NbResult<()> {
        let pendientes = {
            let mut guardadas = self.pendientes.lock().map_err(|_| NbError::Internal)?;
            std::mem::take(&mut *guardadas)
        };

        let mut primer_error = None;
        for ((edition_id, profile), pendiente) in pendientes {
            if !pendiente.sucio {
                continue;
            }
            let resultado = self.escribir(
                &edition_id,
                &profile,
                &pendiente.locator,
                pendiente.precision,
            );
            if let Err(error) = resultado {
                tracing::error!("no se pudo vaciar el progreso de un documento");
                primer_error.get_or_insert(error);
            }
        }

        primer_error.map_or(Ok(()), Err)
    }

    /// Cuántos documentos tienen algo apuntado sin escribir.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo quedó envenenado.
    pub fn pending(&self) -> NbResult<usize> {
        Ok(self
            .pendientes
            .lock()
            .map_err(|_| NbError::Internal)?
            .values()
            .filter(|p| p.sucio)
            .count())
    }

    /// Descarta todo lo pendiente sin escribirlo.
    ///
    /// Se llama al borrar el historial de lectura o la biblioteca entera: sin esto,
    /// el próximo vaciado o el `Drop` volvería a escribir en `progress` el punto de
    /// lectura que el usuario acaba de pedir borrar.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo quedó envenenado.
    pub fn clear(&self) -> NbResult<()> {
        self.pendientes
            .lock()
            .map_err(|_| NbError::Internal)?
            .clear();
        Ok(())
    }
}

impl Drop for ProgressTracker {
    fn drop(&mut self) {
        // Última red: si alguien olvidó cerrar, lo pendiente se escribe igual. No
        // sustituye a `flush_all`, porque un `Drop` no corre si el proceso muere
        // de golpe, pero cubre el cierre ordenado que se saltó un paso.
        if let Err(error) = self.flush_all() {
            tracing::error!(code = error.code(), "quedó progreso sin escribir al cerrar");
        }
    }
}

/// A qué ritmo se lee un libro, sacado de lo que ya se guarda.
///
/// Sin cronómetro que arrancar y parar: son las sesiones de lectura que el lector
/// abre y cierra solo, con su hora y su posición de principio y de fin. Lo que se
/// devuelve es cuánto avanza el libro por minuto de lectura, y el tiempo que queda
/// lo calcula quien sabe por dónde va ahora.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ReadingPace {
    /// Cuánto de la obra, de 0 a 1, se lee en un minuto.
    pub progression_per_minute: f64,
    /// Los minutos de lectura con los que se calculó.
    pub minutes: f64,
    /// Las sesiones que cuentan.
    pub sessions: u32,
}

/// Cuántos minutos de lectura hacen falta para decir un ritmo. Con menos, la
/// estimación saldría de un rato que no dice nada, y se inventaría.
pub const MINUTOS_PARA_UN_RITMO: f64 = 10.0;

/// Una sesión de más de esto no es lectura seguida: es el libro abierto mientras se
/// hacía otra cosa, y contarla haría el ritmo más lento de lo que es.
const SESION_MAS_LARGA: f64 = 180.0;

/// Una sesión que avanza más de cinco veces el ritmo de las demás es un salto por
/// el índice o una búsqueda, no lectura.
const VECES_EL_RITMO: f64 = 5.0;

/// El ritmo de lectura de una edición, o nada si no hay bastante para decirlo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn ritmo(conn: &Connection, edition_id: &str) -> NbResult<Option<ReadingPace>> {
    let mut consulta = conn
        .prepare(
            "SELECT (julianday(ended_at) - julianday(started_at)) * 1440.0, start_locator, end_locator
               FROM reading_sessions
              WHERE edition_id = ?1 AND ended_at IS NOT NULL AND end_locator IS NOT NULL
              ORDER BY started_at DESC
              LIMIT 200",
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudieron leer las sesiones de lectura");
            NbError::StorageFailure
        })?;
    let filas = consulta
        .query_map([edition_id], |f| {
            Ok((
                f.get::<_, f64>(0)?,
                f.get::<_, String>(1)?,
                f.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudieron leer las sesiones de lectura");
            NbError::StorageFailure
        })?;

    let progresion = |bruto: &str| {
        serde_json::from_str::<Locator>(bruto)
            .ok()
            .and_then(|l| l.locations.total_progression)
    };
    // (minutos, avance) de cada sesión que fue lectura hacia delante.
    let tramos: Vec<(f64, f64)> = filas
        .filter_map(Result::ok)
        .filter_map(|(minutos, desde, hasta)| {
            let avance = progresion(&hasta)? - progresion(&desde)?;
            (minutos > 0.5 && minutos <= SESION_MAS_LARGA && avance > 0.0)
                .then_some((minutos, avance))
        })
        .collect();
    if tramos.is_empty() {
        return Ok(None);
    }

    let mut ritmos: Vec<f64> = tramos.iter().map(|(m, a)| a / m).collect();
    ritmos.sort_by(f64::total_cmp);
    let mediana = ritmos[ritmos.len() / 2];
    let lectura: Vec<&(f64, f64)> = tramos
        .iter()
        .filter(|(m, a)| a / m <= mediana * VECES_EL_RITMO)
        .collect();
    let minutos: f64 = lectura.iter().map(|(m, _)| m).sum();
    let avance: f64 = lectura.iter().map(|(_, a)| a).sum();
    if minutos < MINUTOS_PARA_UN_RITMO || avance <= 0.0 {
        return Ok(None);
    }
    Ok(Some(ReadingPace {
        progression_per_minute: avance / minutos,
        minutes: minutos,
        sessions: u32::try_from(lectura.len()).unwrap_or(u32::MAX),
    }))
}
