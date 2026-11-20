//! Preparar el audio de un capítulo o de un libro, antes de escucharlo.
//!
//! # Por qué existe esto, y no solo la narración en vivo
//!
//! Porque **la mayoría de las máquinas no narran en vivo**. El factor de tiempo
//! real medido en el equipo de desarrollo es 1,67: decir un párrafo tarda más de
//! lo que el párrafo dura, así que adelantar la síntesis no lo arregla —un
//! adelanto que se consume más deprisa de lo que se llena se vacía siempre—.
//!
//! Se decidió el 2026-09-18: en lugar de no ofrecer la voz donde no llega,
//! **se ofrecen tres formas de escuchar**, y esta es la que funciona en cualquier
//! equipo. Se paga el tiempo una vez, con el coste delante y pudiendo cancelar, y
//! después se escucha como un audiolibro.
//!
//! # Lo que se guarda, y por qué no es solo un archivo
//!
//! Un archivo de audio dice cómo suena y nada más. Aquí hacen falta tres cosas
//! que no caben dentro de él: con qué voz y qué versión del modelo se hizo,
//! cuánto ocupa —porque cuenta contra el tope de lo derivado— y **por dónde va el
//! texto mientras suena**, que es lo que permite seguir la lectura y saltar a un
//! párrafo. Eso vive en `audio_sections` y `audio_marks` (V35).
//!
//! Contrato: `docs/architecture/voice-services.md`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::neuronal::Sintetizador;
use crate::ai::generacion::Documentos;
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};
use crate::reader::Locator;

/// Caracteres de texto por segundo de audio, **medido**.
///
/// La frase de la prueba —105 caracteres— dura 6,25 segundos con la voz
/// a velocidad normal: unos 17. Sirve para decir el coste **antes** de empezar;
/// en cuanto hay audio hecho, el tiempo restante sale de lo medido en esa pasada
/// y no de esta constante.
const CARACTERES_POR_SEGUNDO: f64 = 17.0;

/// Cuántos fragmentos caben en una preparación. Un libro largo ronda los 3.000.
const MAXIMO_DE_FRAGMENTOS: usize = 20_000;

/// Cuántos alcances caben esperando en la cola.
///
/// Cota del comando que la publica, por la regla 6 del contrato del puente. Son
/// libros enteros: quinientos es más de lo que nadie encola a mano, y encolar una
/// colección de mil no puede dejar la respuesta sin cota.
const MAXIMO_EN_COLA: usize = 500;

/// Cuántos fragmentos entran en la muestra que se escucha antes de decidir.
///
/// Tres: bastante para oír la voz diciendo **este** libro —sus nombres propios,
/// su puntuación— y poco para que llegue en menos de un minuto en cualquier
/// equipo.
const FRAGMENTOS_DE_MUESTRA: usize = 3;

/// Qué parte del libro se prepara.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AudioScope")]
pub enum Alcance {
    /// Las secciones de un capítulo, como las calcula la interfaz desde el índice.
    #[serde(rename_all = "camelCase")]
    Chapter {
        edition_id: String,
        hrefs: Vec<String>,
        /// En un documento paginado, la primera página del capítulo.
        from_position: Option<u32>,
        /// La primera página que **ya no** es del capítulo.
        to_position: Option<u32>,
        /// Cómo se llama, para poder enseñarlo mientras se prepara.
        title: String,
    },
    /// El libro entero.
    #[serde(rename_all = "camelCase")]
    Book { edition_id: String, title: String },
}

impl Alcance {
    #[must_use]
    pub fn edicion(&self) -> &str {
        match self {
            Self::Chapter { edition_id, .. } | Self::Book { edition_id, .. } => edition_id,
        }
    }

    #[must_use]
    pub fn titulo(&self) -> &str {
        match self {
            Self::Chapter { title, .. } | Self::Book { title, .. } => title,
        }
    }

    /// La clave con la que se guarda. El libro entero es la cadena vacía.
    #[must_use]
    pub fn clave(&self) -> String {
        match self {
            Self::Chapter { hrefs, .. } => hrefs.join("|"),
            Self::Book { .. } => String::new(),
        }
    }
}

/// Lo que cuesta preparar un alcance, **antes** de empezar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "AudioEstimate"
)]
pub struct Estimacion {
    pub fragments: u32,
    #[ts(type = "number")]
    pub characters: u64,
    /// Cuánto durará el audio, en segundos.
    #[ts(type = "number")]
    pub audio_seconds: u64,
    /// Lo que ocupará en disco.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Cuánto tardará en prepararse, con el factor medido en esta máquina.
    ///
    /// Nulo si la voz todavía no se ha medido aquí: decir un número inventado
    /// sobre una espera de una hora es peor que no decir ninguno.
    #[ts(type = "number | null")]
    pub work_seconds: Option<u64>,
    /// Si ya está preparado con la voz y el modelo de ahora.
    pub already: bool,
    /// Si lo que ocupará cabe en lo que queda del presupuesto de lo derivado.
    ///
    /// Es el criterio -"lo preparado cabe en el presupuesto, o no se
    /// prepara"- dicho **antes** de que nadie espere. Que se diga aquí no lo hace
    /// cumplir: quien lo hace cumplir es `empezar`, que se niega, y la propia
    /// preparación, que se detiene si el sitio se acaba mientras trabaja.
    pub fits: bool,
    /// Lo que queda hasta el tope, si se puede medir.
    #[ts(type = "number | null")]
    pub free_bytes: Option<u64>,
}

/// Una preparación guardada, con sus marcas.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "PreparedAudio"
)]
pub struct Preparado {
    pub edition_id: String,
    pub title: String,
    pub voice: String,
    pub model_rev: String,
    pub matches_current: bool,
    /// Ruta del archivo, para que la ventana lo reproduzca.
    pub path: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    /// `ready` o `partial`: lo que quedó a medias también se puede escuchar.
    pub status: String,
    /// Cota: un fragmento por marca, y los fragmentos de un libro son miles pero
    /// finitos; `MAXIMO_DE_FRAGMENTOS` los acota al prepararlos.
    pub marks: Vec<Marca>,
}

pub use crate::reader::provider::Marca;

/// Lo que se está preparando ahora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AudioJob")]
pub struct EnCurso {
    pub edition_id: String,
    pub title: String,
    pub done: u32,
    pub total: u32,
    #[ts(type = "number | null")]
    pub remaining_ms: Option<u64>,
    /// Cuántos esperan detrás.
    pub queued: u32,
}

/// Uno de los que esperan turno.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AudioQueued")]
pub struct EnCola {
    pub edition_id: String,
    pub title: String,
}

/// Lo que quedó hecho de una preparación, para guardarlo de una pieza.
struct Hecho<'a> {
    voz: &'a str,
    archivo: &'a str,
    bytes: u64,
    duracion_ms: u64,
    marcas: &'a [(u64, u64, Locator)],
    parcial: bool,
}

struct Estado {
    /// Lo que espera turno, con lo que cada uno ocupará.
    ///
    /// Los bytes viajan con el alcance porque el presupuesto se mira **sumando
    /// lo que ya está prometido**: mirar solo lo que hay en disco dejaría pasar
    /// veinte libros de uno en uno, cada uno cabiendo, y el vigésimo empezaría
    /// con el tope ya pasado por los diecinueve anteriores.
    cola: VecDeque<(Alcance, Option<String>, u64)>,
    actual: Option<EnCurso>,
    /// La suma de lo prometido y aún no escrito.
    bytes_encolados: u64,
}

/// Quien prepara el audio: uno a la vez, con cola y cancelable.
pub struct Preparador {
    db: Database,
    documentos: Arc<dyn Documentos>,
    voz: Arc<dyn Sintetizador>,
    paths: crate::paths::AppPaths,
    carpeta: PathBuf,
    bus: EventBus,
    cancelado: AtomicBool,
    estado: Mutex<Estado>,
}

impl Preparador {
    #[must_use]
    pub fn nuevo(
        db: Database,
        documentos: Arc<dyn Documentos>,
        voz: Arc<dyn Sintetizador>,
        paths: &crate::paths::AppPaths,
        bus: EventBus,
    ) -> Arc<Self> {
        Arc::new(Self {
            db,
            documentos,
            voz,
            carpeta: paths.audio(),
            paths: paths.clone(),
            bus,
            cancelado: AtomicBool::new(false),
            estado: Mutex::new(Estado {
                cola: VecDeque::new(),
                actual: None,
                bytes_encolados: 0,
            }),
        })
    }

    /// Lo que costaría preparar este alcance.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si la edición no existe o no tiene texto.
    /// - [`NbError::StorageFailure`] si la base no responde.
    pub fn estimar(&self, alcance: &Alcance, voz: Option<&str>) -> NbResult<Estimacion> {
        let fragmentos = self.fragmentos(alcance)?;
        let caracteres: u64 = fragmentos
            .iter()
            .map(|(_, texto)| texto.chars().count() as u64)
            .sum();
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let segundos = (caracteres as f64 / CARACTERES_POR_SEGUNDO) as u64;
        let factor = self.voz.factor_medido();

        let bytes = segundos * super::audio::bytes_por_segundo();
        let libre = self.sitio_libre();

        Ok(Estimacion {
            fragments: u32::try_from(fragmentos.len()).unwrap_or(u32::MAX),
            characters: caracteres,
            audio_seconds: segundos,
            bytes,
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            work_seconds: factor.map(|f| (segundos as f64 * f64::from(f)) as u64),
            already: self
                .preparado(alcance, voz)?
                .is_some_and(|p| p.matches_current),
            fits: libre.is_none_or(|queda| bytes <= queda),
            free_bytes: libre,
        })
    }

    /// Lo que queda hasta el tope de lo derivado, en bytes.
    ///
    /// `None` cuando no se puede medir o cuando el catálogo no tiene documentos:
    /// el tope es un porcentaje de lo que ocupan los libros, y un porcentaje de
    /// cero no acota nada. En ese caso no se frena a nadie, igual que hace el
    /// indexador, porque frenar por una medida que no existe sería negarse sin
    /// motivo.
    fn sitio_libre(&self) -> Option<u64> {
        let conn = self.db.connection().ok()?;
        let medida = crate::diagnostics::medir_almacenamiento(&conn, &self.paths)?;
        if medida.documents_bytes == 0 {
            return None;
        }
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let tope = (medida.documents_bytes as f64 * medida.budget_percent / 100.0) as u64;
        Some(tope.saturating_sub(medida.derived_bytes))
    }

    /// Sintetiza **una muestra corta** del propio libro, para decidir antes de
    /// esperar.
    ///
    /// Se pidió el 2026-09-18, y no es un detalle de cortesía: quien va a
    /// esperar cuarenta minutos tiene derecho a oír antes cómo suena **su** libro
    /// —sus nombres propios, su puntuación— y no una frase de ejemplo.
    ///
    /// # Errors
    ///
    /// Los de [`VozNeuronal::sintetizar_a_archivo`], y
    /// [`NbError::DocumentNotFound`] si el alcance no tiene texto.
    pub fn muestra(&self, alcance: &Alcance, voz: Option<&str>) -> NbResult<String> {
        let fragmentos = self.fragmentos(alcance)?;
        let texto: String = fragmentos
            .iter()
            .take(FRAGMENTOS_DE_MUESTRA)
            .map(|(_, texto)| texto.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if texto.trim().is_empty() {
            return Err(NbError::DocumentNotFound);
        }
        let archivo =
            self.voz
                .sintetizar_a_archivo(&self.carpeta.join("muestras"), &texto, voz, 1.0)?;
        Ok(archivo.to_string_lossy().into_owned())
    }

    /// Encola un alcance y arranca si no había nada en marcha.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si la voz no está instalada.
    /// - [`NbError::DocumentNotFound`] si el alcance no tiene texto.
    /// - [`NbError::StorageBudgetReached`] si lo que ocuparía no cabe en lo que
    ///   queda del presupuesto de lo derivado. **No se empieza y se dice antes**:
    ///   es el criterio, y descubrirlo a los treinta minutos de espera
    ///   sería exactamente lo que el criterio existe para evitar.
    pub fn empezar(self: &Arc<Self>, alcance: Alcance, voz: Option<String>) -> NbResult<()> {
        if !self.voz.instalada() {
            return Err(NbError::ComponentUnavailable);
        }
        let estimacion = self.estimar(&alcance, voz.as_deref())?;
        let cuantos = estimacion.fragments as usize;
        if cuantos == 0 {
            return Err(NbError::DocumentNotFound);
        }

        let arrancar = {
            let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
            let ya =
                estado.actual.as_ref().is_some_and(|a| {
                    a.edition_id == alcance.edicion() && a.title == alcance.titulo()
                }) || estado.cola.iter().any(|(otro, _, _)| *otro == alcance);
            if !ya {
                // El tope se mira contra lo que hay en disco **más lo prometido**,
                // y la cola tiene además su propia cota: las dos dicen lo mismo
                // -no cabe más trabajo-, y por eso comparten código.
                let no_cabe = estimacion
                    .free_bytes
                    .is_some_and(|queda| estimacion.bytes + estado.bytes_encolados > queda);
                if no_cabe || estado.cola.len() >= MAXIMO_EN_COLA {
                    return Err(NbError::StorageBudgetReached);
                }
                estado.bytes_encolados += estimacion.bytes;
                estado
                    .cola
                    .push_back((alcance.clone(), voz, estimacion.bytes));
            }
            let arrancar = estado.actual.is_none();
            // Lo que se prepara se anuncia **aquí**, antes de arrancar el hilo. Sin
            // esto, entre que esta llamada vuelve y el hilo empieza hay un hueco en
            // el que preguntar qué se está preparando contesta "nada", y quien
            // pregunte -la interfaz, o una prueba- da por terminado algo que aún no
            // ha empezado.
            if arrancar {
                let queued = u32::try_from(estado.cola.len().saturating_sub(1)).unwrap_or(u32::MAX);
                estado.actual = Some(EnCurso {
                    edition_id: alcance.edicion().to_owned(),
                    title: alcance.titulo().to_owned(),
                    done: 0,
                    total: u32::try_from(cuantos).unwrap_or(u32::MAX),
                    remaining_ms: None,
                    queued,
                });
            }
            arrancar
        };

        if arrancar {
            self.cancelado.store(false, Ordering::SeqCst);
            let suyo = Arc::clone(self);
            std::thread::Builder::new()
                .name("nb-voz-preparacion".to_owned())
                .spawn(move || suyo.trabajar())
                .map_err(|_| NbError::Internal)?;
        }
        Ok(())
    }

    /// Encola el libro entero de cada obra de una colección.
    ///
    /// Devuelve **cuántas entraron**, que no tiene por qué ser cuántas hay: lo que
    /// no cabe en el presupuesto no se encola, y decir "se encolaron doce de
    /// treinta" es lo único honesto que se puede enseñar. Se para al primer "no
    /// cabe" en lugar de seguir probando, porque la cola es una fila y lo que no
    /// cupo ahora tampoco cabe detrás de lo que ya está prometido.
    ///
    /// Las obras sin texto se saltan sin ruido: una colección con un PDF
    /// escaneado no puede quedarse sin preparar por él.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si la voz no está instalada.
    /// - [`NbError::StorageFailure`] si la base no responde.
    /// - [`NbError::StorageBudgetReached`] si no cupo ninguna.
    pub fn encolar_coleccion(
        self: &Arc<Self>,
        collection_id: &str,
        voz: Option<String>,
    ) -> NbResult<u32> {
        if !self.voz.instalada() {
            return Err(NbError::ComponentUnavailable);
        }
        let conn = self.db.connection()?;
        let miembros = crate::collections::Collection::members(&conn, collection_id)?;
        let mut titulos = Vec::with_capacity(miembros.len());
        for id in miembros {
            let titulo: String = conn
                .query_row("SELECT title FROM editions WHERE id = ?1", [&id], |fila| {
                    fila.get(0)
                })
                .unwrap_or_default();
            titulos.push((id, titulo));
        }
        drop(conn);

        let mut encoladas = 0_u32;
        let mut sin_sitio = false;
        for (edition_id, title) in titulos {
            match self.empezar(Alcance::Book { edition_id, title }, voz.clone()) {
                Ok(()) => encoladas += 1,
                Err(NbError::StorageBudgetReached) => {
                    sin_sitio = true;
                    break;
                }
                Err(_) => continue,
            }
        }

        if encoladas == 0 && sin_sitio {
            return Err(NbError::StorageBudgetReached);
        }
        Ok(encoladas)
    }

    /// Corta lo que se esté preparando. **Lo hecho se queda y se puede escuchar.**
    pub fn cancelar(&self) {
        self.cancelado.store(true, Ordering::SeqCst);
        let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
        estado.cola.clear();
        estado.bytes_encolados = 0;
    }

    /// Los que esperan turno, en orden.
    ///
    /// Cota: `MAXIMO_EN_COLA`, que es lo que `empezar` admite empujar.
    #[must_use]
    pub fn cola(&self) -> Vec<EnCola> {
        self.estado
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .cola
            .iter()
            .map(|(alcance, _, _)| EnCola {
                edition_id: alcance.edicion().to_owned(),
                title: alcance.titulo().to_owned(),
            })
            .collect()
    }

    /// Lo que se está preparando ahora, si hay algo.
    #[must_use]
    pub fn en_curso(&self) -> Option<EnCurso> {
        self.estado
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .actual
            .clone()
    }

    /// Lo preparado de un alcance, si lo hay.
    ///
    /// Busca preferentemente lo preparado con la voz y el modelo de ahora
    /// (`matches_current = true`). Si no coincide pero hay una preparación anterior
    /// en disco, la devuelve con `matches_current = false` para que la interfaz
    /// pueda informar de qué voz y modelo tiene y ofrecer regenerarla sin hacerlo
    /// sola.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn preparado(&self, alcance: &Alcance, voz: Option<&str>) -> NbResult<Option<Preparado>> {
        let conn = self.db.connection()?;
        let elegida = voz
            .map(str::to_owned)
            .unwrap_or_else(|| self.voz_por_defecto());
        let revision_actual = self.voz.revision();

        let mut consulta = conn
            .prepare(
                "SELECT id, title, voice, model_rev, file_name, bytes, duration_ms, status
                 FROM audio_sections
                 WHERE edition_id = ?1 AND scope_key = ?2
                 ORDER BY
                     CASE WHEN voice = ?3 AND model_rev = ?4 THEN 0 ELSE 1 END,
                     created_at DESC",
            )
            .map_err(|_| NbError::StorageFailure)?;

        let filas = consulta
            .query_map(
                rusqlite::params![
                    alcance.edicion(),
                    alcance.clave(),
                    &elegida,
                    revision_actual
                ],
                |fila| {
                    Ok((
                        fila.get::<_, String>(0)?,
                        fila.get::<_, String>(1)?,
                        fila.get::<_, String>(2)?,
                        fila.get::<_, String>(3)?,
                        fila.get::<_, String>(4)?,
                        fila.get::<_, i64>(5)?,
                        fila.get::<_, i64>(6)?,
                        fila.get::<_, String>(7)?,
                    ))
                },
            )
            .map_err(|_| NbError::StorageFailure)?;

        let mut encontrado = None;
        for item in filas.flatten() {
            let (id, titulo, voz_fila, rev_fila, archivo, bytes, duracion, estado) = item;
            let ruta = self.carpeta.join(&archivo);
            if ruta.is_file() {
                let matches_current = voz_fila == elegida && rev_fila == revision_actual;
                encontrado = Some((
                    id,
                    titulo,
                    voz_fila,
                    rev_fila,
                    ruta,
                    bytes,
                    duracion,
                    estado,
                    matches_current,
                ));
                break;
            }
        }

        let Some((id, titulo, voz_fila, rev_fila, ruta, bytes, duracion, estado, matches_current)) =
            encontrado
        else {
            return Ok(None);
        };

        let mut consulta_marcas = conn
            .prepare(
                "SELECT start_ms, end_ms, anchor FROM audio_marks
                 WHERE section_id = ?1 ORDER BY ordinal",
            )
            .map_err(|_| NbError::StorageFailure)?;
        let marcas = consulta_marcas
            .query_map([&id], |fila| {
                Ok((
                    fila.get::<_, i64>(0)?,
                    fila.get::<_, i64>(1)?,
                    fila.get::<_, String>(2)?,
                ))
            })
            .map_err(|_| NbError::StorageFailure)?
            .filter_map(Result::ok)
            .filter_map(|(desde, hasta, ancla)| {
                serde_json::from_str::<Locator>(&ancla)
                    .ok()
                    .map(|locator| Marca {
                        start_ms: desde.unsigned_abs(),
                        end_ms: hasta.unsigned_abs(),
                        locator,
                    })
            })
            .collect();

        Ok(Some(Preparado {
            edition_id: alcance.edicion().to_owned(),
            title: titulo,
            voice: voz_fila,
            model_rev: rev_fila,
            matches_current,
            path: ruta.to_string_lossy().into_owned(),
            duration_ms: duracion.unsigned_abs(),
            bytes: bytes.unsigned_abs(),
            status: estado,
            marks: marcas,
        }))
    }

    /// Quita lo preparado de **un alcance**: sus filas, sus marcas y sus archivos,
    /// con cualquier voz y revisión del modelo.
    ///
    /// Antes recibía la edición y borraba todo lo de ese libro, así que
    /// "quitar lo preparado" de un capítulo se llevaba también el libro entero, que
    /// son horas de procesador. Ahora quita lo que se pide y nada más.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn olvidar(&self, alcance: &Alcance) -> NbResult<()> {
        let conn = self.db.connection()?;
        let mut consulta = conn
            .prepare(
                "SELECT file_name FROM audio_sections WHERE edition_id = ?1 AND scope_key = ?2",
            )
            .map_err(|_| NbError::StorageFailure)?;
        let archivos: Vec<String> = consulta
            .query_map(
                rusqlite::params![alcance.edicion(), alcance.clave()],
                |fila| fila.get::<_, String>(0),
            )
            .map_err(|_| NbError::StorageFailure)?
            .filter_map(Result::ok)
            .collect();
        drop(consulta);

        conn.execute(
            "DELETE FROM audio_sections WHERE edition_id = ?1 AND scope_key = ?2",
            rusqlite::params![alcance.edicion(), alcance.clave()],
        )
        .map_err(|_| NbError::StorageFailure)?;
        for archivo in archivos {
            let _ = std::fs::remove_file(self.carpeta.join(archivo));
        }
        Ok(())
    }

    /// Retira lo preparado con una voz que ya no existe en el catálogo.
    ///
    /// Lo preparado con **otra** voz o con otra revisión del modelo se conserva:
    /// el audio sigue sonando y sus marcas siguen apuntando al texto, así que es
    /// compatible, y la interfaz ofrece regenerarlo sin hacerlo sola. Lo que ya no
    /// es compatible es lo de una voz que una actualización retiró: no se puede
    /// regenerar igual ni continuar, y ocuparía disco contra el tope de lo
    /// derivado para siempre. Eso se borra solo, y la cuenta queda apuntada para
    /// que la interfaz lo diga una vez (`aviso_de_audio_retirado`).
    ///
    /// Devuelve cuántas preparaciones se retiraron.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn retirar_de_voces_retiradas(&self, voces: &[&str]) -> NbResult<u32> {
        let conn = self.db.connection()?;
        let filas: Vec<(String, String, String)> = {
            let mut consulta = conn
                .prepare("SELECT id, voice, file_name FROM audio_sections")
                .map_err(|_| NbError::StorageFailure)?;
            let filas: Vec<(String, String, String)> = consulta
                .query_map([], |f| Ok((f.get(0)?, f.get(1)?, f.get(2)?)))
                .map_err(|_| NbError::StorageFailure)?
                .filter_map(Result::ok)
                .collect();
            filas
        };
        let mut retiradas = 0_u32;
        for (id, voz, archivo) in filas {
            if voces.contains(&voz.as_str()) {
                continue;
            }
            conn.execute("DELETE FROM audio_sections WHERE id = ?1", [&id])
                .map_err(|_| NbError::StorageFailure)?;
            let _ = std::fs::remove_file(self.carpeta.join(archivo));
            retiradas += 1;
        }
        if retiradas > 0 {
            let _ = conn.execute(
                "INSERT INTO app_metadata (key, value) VALUES ('audio_retirado', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + ?1",
                [retiradas],
            );
            tracing::info!(retiradas, "audio preparado con voces retiradas, borrado");
        }
        Ok(retiradas)
    }

    /// Cuántas preparaciones se retiraron desde el último aviso, y lo da por dicho.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn aviso_de_audio_retirado(&self) -> NbResult<Option<u32>> {
        let conn = self.db.connection()?;
        let cuenta: Option<String> = conn
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'audio_retirado'",
                [],
                |f| f.get(0),
            )
            .ok();
        let _ = conn.execute("DELETE FROM app_metadata WHERE key = 'audio_retirado'", []);
        Ok(cuenta.and_then(|c| c.parse().ok()).filter(|&n: &u32| n > 0))
    }

    fn voz_por_defecto(&self) -> String {
        self.voz.voz_por_defecto()
    }

    /// Los fragmentos del alcance, con su locator y en el orden del documento.
    fn fragmentos(&self, alcance: &Alcance) -> NbResult<Vec<(Locator, String)>> {
        let documento = self.documentos.abrir(alcance.edicion())?;
        let todos = documento.text_chunks(crate::search::FRAGMENTO)?;

        let dentro: Vec<(Locator, String)> = match alcance {
            Alcance::Book { .. } => todos
                .into_iter()
                .filter(|(_, texto)| !texto.trim().is_empty())
                .collect(),
            Alcance::Chapter {
                hrefs,
                from_position,
                to_position,
                ..
            } => {
                // Sin la parte de ancla: el índice de un capítulo apunta a veces a un
                // título dentro del recurso, y los fragmentos son del recurso entero.
                let hrefs: std::collections::HashSet<&str> = hrefs
                    .iter()
                    .map(|h| h.split('#').next().unwrap_or(h))
                    .collect();
                todos
                    .into_iter()
                    .filter(|(locator, texto)| {
                        let dentro = match locator.locations.position {
                            Some(p) => {
                                from_position.is_none_or(|desde| p >= desde)
                                    && to_position.is_none_or(|hasta| p < hasta)
                            }
                            None => true,
                        };
                        dentro && hrefs.contains(locator.href.as_str()) && !texto.trim().is_empty()
                    })
                    .collect()
            }
        };

        Ok(dentro.into_iter().take(MAXIMO_DE_FRAGMENTOS).collect())
    }

    fn trabajar(self: &Arc<Self>) {
        loop {
            let siguiente = {
                let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
                let siguiente = estado.cola.pop_front();
                if siguiente.is_none() {
                    estado.actual = None;
                }
                siguiente
            };
            let Some((alcance, voz, bytes)) = siguiente else {
                return;
            };

            // Un pánico se convierte en un fallo **con su final**: sin esto, el hilo
            // moría, `actual` se quedaba puesto para siempre, no llegaba ningún
            // `AudioJobFinished` y todo lo que se encolara después esperaba a un
            // trabajo que ya no existía.
            let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.preparar(&alcance, voz.as_deref())
            }))
            .unwrap_or_else(|_| {
                tracing::error!("la preparación de audio entró en pánico");
                Err(NbError::Internal)
            });
            // Lo que se está preparando **sigue contando como prometido hasta que
            // termina**, y no deja de contar al salir de la cola. Descontarlo al
            // empezar dejaba pasar un segundo libro mientras el primero llevaba
            // escritos cuatro megabytes de los novecientos que va a ocupar: el
            // disco todavía no sabe lo que viene, así que quien tiene que saberlo
            // es esto.
            {
                let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
                estado.bytes_encolados = estado.bytes_encolados.saturating_sub(bytes);
            }
            let (outcome, reason_code) = match resultado {
                Ok(()) => ("done", None),
                Err(NbError::Cancelled) => ("cancelled", None),
                // Quedarse sin sitio no es un fallo: lo hecho sirve, y lo que hay
                // que ofrecer -liberar sitio o subir el tope- no es "reintentar".
                // Por eso tiene desenlace propio y no cae en "failed".
                Err(NbError::StorageBudgetReached) => {
                    ("paused", Some(NbError::StorageBudgetReached.code()))
                }
                Err(error) => {
                    tracing::warn!(codigo = error.code(), "la preparación de audio se detuvo");
                    ("failed", Some(error.code()))
                }
            };
            self.bus.emit(DomainEvent::AudioJobFinished {
                edition_id: alcance.edicion().to_owned(),
                outcome,
                reason_code,
            });

            if outcome == "cancelled" || outcome == "paused" {
                let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
                estado.cola.clear();
                estado.bytes_encolados = 0;
            }
        }
    }

    /// Prepara un alcance entero. Es lo que corre en el hilo.
    fn preparar(&self, alcance: &Alcance, voz: Option<&str>) -> NbResult<()> {
        let fragmentos = self.fragmentos(alcance)?;
        let total = u32::try_from(fragmentos.len()).unwrap_or(u32::MAX);
        let elegida = voz
            .map(str::to_owned)
            .unwrap_or_else(|| self.voz_por_defecto());

        let nombre = format!(
            "{}-{}.opus",
            alcance.edicion(),
            crate::library::hash_reader(
                format!("{}|{elegida}|{}", alcance.clave(), self.voz.revision()).as_bytes()
            )?
            .0
            .as_str()
            .replace(':', "-")
        );
        let destino = self.carpeta.join(&nombre);

        {
            let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
            estado.actual = Some(EnCurso {
                edition_id: alcance.edicion().to_owned(),
                title: alcance.titulo().to_owned(),
                done: 0,
                total,
                remaining_ms: None,
                queued: u32::try_from(estado.cola.len()).unwrap_or(u32::MAX),
            });
        }

        let mut grabadora = super::audio::Grabadora::nueva(&destino, super::kokoro::MUESTREO)?;
        let mut marcas: Vec<(u64, u64, Locator)> = Vec::with_capacity(fragmentos.len());
        let reloj = Instant::now();
        let mut desde_ms = 0_u64;
        let mut cancelado = false;
        let mut sin_sitio = false;
        // Lo que queda hasta el tope se mide **una vez**, al empezar, y a partir
        // de ahí lo que se gasta es lo que pesa este archivo. Volver a medir por
        // fragmento recorrería la caché entera -ciento cincuenta mil portadas-
        // cada dos segundos, que es más trabajo que el que vigila. Lo que otro
        // subsistema gaste mientras tanto lo ve la vuelta siguiente de la cola,
        // que sí vuelve a medir.
        let libre = self.sitio_libre();

        for (indice, (locator, texto)) in fragmentos.iter().enumerate() {
            if self.cancelado.load(Ordering::SeqCst) {
                cancelado = true;
                break;
            }
            // Cada veinte fragmentos, porque `metadata` es una llamada al sistema
            // y veinte fragmentos son medio minuto de audio: el exceso posible es
            // de kilobytes, no de megabytes.
            if indice % 20 == 0 && indice > 0 {
                if let Some(queda) = libre {
                    let escrito = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);
                    if escrito >= queda {
                        sin_sitio = true;
                        break;
                    }
                }
            }

            let sonido = self.voz.sintetizar(texto, Some(&elegida), 1.0)?;
            let hasta_ms = grabadora.anadir(&sonido.muestras)?;
            marcas.push((desde_ms, hasta_ms, locator.clone()));
            desde_ms = hasta_ms;

            let hechos = u32::try_from(indice + 1).unwrap_or(u32::MAX);
            self.avisar(alcance, hechos, total, reloj, desde_ms);
        }

        let duracion = grabadora.cerrar()?;
        let bytes = std::fs::metadata(&destino).map(|m| m.len()).unwrap_or(0);

        if marcas.is_empty() {
            let _ = std::fs::remove_file(&destino);
            return if sin_sitio {
                Err(NbError::StorageBudgetReached)
            } else if cancelado {
                Err(NbError::Cancelled)
            } else {
                Err(NbError::DocumentNotFound)
            };
        }

        self.guardar(
            alcance,
            &Hecho {
                voz: &elegida,
                archivo: &nombre,
                bytes,
                duracion_ms: duracion,
                marcas: &marcas,
                parcial: cancelado || sin_sitio,
            },
        )?;

        if sin_sitio {
            return Err(NbError::StorageBudgetReached);
        }
        if cancelado {
            return Err(NbError::Cancelled);
        }
        Ok(())
    }

    /// Publica el avance y **el tiempo que falta según lo medido en esta pasada**.
    fn avisar(&self, alcance: &Alcance, hechos: u32, total: u32, reloj: Instant, audio_ms: u64) {
        // Con menos de tres fragmentos hechos no se dice nada: el primero incluye
        // la carga del modelo y daría una cuenta atrás de horas que después cae en
        // picado, que es la clase de número que nadie vuelve a creerse.
        let restante = (hechos >= 3 && total > hechos).then(|| {
            let por_fragmento = reloj.elapsed().as_millis() as u64 / u64::from(hechos);
            por_fragmento * u64::from(total - hechos)
        });

        let queued = {
            let mut estado = self.estado.lock().unwrap_or_else(|p| p.into_inner());
            let queued = u32::try_from(estado.cola.len()).unwrap_or(u32::MAX);
            if let Some(actual) = estado.actual.as_mut() {
                actual.done = hechos;
                actual.remaining_ms = restante;
                actual.queued = queued;
            }
            queued
        };

        self.bus.emit(DomainEvent::AudioJobProgress {
            edition_id: alcance.edicion().to_owned(),
            done: hechos,
            total,
            remaining_ms: restante,
            seconds: audio_ms / 1_000,
            queued,
        });
    }

    fn guardar(&self, alcance: &Alcance, hecho: &Hecho<'_>) -> NbResult<()> {
        let Hecho {
            voz,
            archivo,
            bytes,
            duracion_ms,
            marcas,
            parcial,
        } = *hecho;
        let mut conn = self.db.connection()?;
        // Inmediata: en WAL, una transacción diferida que lee y después escribe
        // recibe `SQLITE_BUSY` sin respetar el plazo. Es la regla de `storage.md`.
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| NbError::StorageFailure)?;

        tx.execute(
            "DELETE FROM audio_sections
             WHERE edition_id = ?1 AND scope_key = ?2 AND voice = ?3 AND model_rev = ?4",
            rusqlite::params![alcance.edicion(), alcance.clave(), voz, self.voz.revision()],
        )
        .map_err(|_| NbError::StorageFailure)?;

        let id: String = tx
            .query_row(
                "INSERT INTO audio_sections
                   (edition_id, scope_key, title, voice, model_rev, file_name, bytes,
                    duration_ms, fragments, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 RETURNING id",
                rusqlite::params![
                    alcance.edicion(),
                    alcance.clave(),
                    alcance.titulo(),
                    voz,
                    self.voz.revision(),
                    archivo,
                    i64::try_from(bytes).unwrap_or(i64::MAX),
                    i64::try_from(duracion_ms).unwrap_or(i64::MAX),
                    i64::try_from(marcas.len()).unwrap_or(i64::MAX),
                    if parcial { "partial" } else { "ready" },
                ],
                |fila| fila.get(0),
            )
            .map_err(|_| NbError::StorageFailure)?;

        {
            let mut insertar = tx
                .prepare(
                    "INSERT INTO audio_marks (section_id, ordinal, start_ms, end_ms, anchor)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|_| NbError::StorageFailure)?;
            for (orden, (desde, hasta, locator)) in marcas.iter().enumerate() {
                let ancla = serde_json::to_string(locator).map_err(|_| NbError::Internal)?;
                insertar
                    .execute(rusqlite::params![
                        &id,
                        i64::try_from(orden).unwrap_or(i64::MAX),
                        i64::try_from(*desde).unwrap_or(i64::MAX),
                        i64::try_from(*hasta).unwrap_or(i64::MAX),
                        ancla,
                    ])
                    .map_err(|_| NbError::StorageFailure)?;
            }
        }

        tx.commit().map_err(|_| NbError::StorageFailure)
    }
}
