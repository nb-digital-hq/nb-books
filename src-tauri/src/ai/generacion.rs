//! Generar los vectores de lo que el usuario pida, en segundo plano.
//!
//! Contratos: `docs/architecture/ai/embeddings.md`, sección 3, y [ADR-007].
//!
//!
//! # Lo que se decidió el 2026-09-16, y cómo queda aquí
//!
//! * **Nada se vectoriza sin que alguien lo pida, con el coste delante.** Ni al
//!   abrir un documento ni al importarlo. Primero [`Generador::estimar`] dice
//!   cuántos documentos, cuántos fragmentos, cuánto disco y cuánto tiempo, y solo
//!   después [`Generador::empezar`]. Es el criterio.
//! * **Dos alcances: el documento abierto y una colección que se elija.** El
//!   primero no espera al tope de lo derivado, igual que el indexador léxico con lo
//!   que se acaba de abrir; la segunda sí, y se detiene al llegar.
//! * **El trabajo de fondo se aparta mientras el lector pinta.** Con un fotograma
//!   que ya va justo, la inferencia en segundo plano
//!   añadiría fotogramas lentos. El lector avisa al desplazar o pasar
//!   página, y el generador espera a que lleve un momento quieto.
//!
//! # Lo que no hace
//!
//! **No calcula el vector de una consulta.** Eso es interactivo, dura décimas de
//! segundo y no espera a nada: es la búsqueda. Lo que hay aquí es lo que puede durar
//! minutos y por eso se pausa, se cancela, se suspende con batería baja o calor y
//! se aparta del lector.
//!
//! [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md

use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::indice::{self, VectorIndex};
use super::presupuesto::Suspension;
use super::Embedding;
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};
use crate::reader::DocumentProvider;

/// Cuántos fragmentos se calculan entre dos miradas a si hay que apartarse.
///
/// **Cuatro.** Con el lote mayor que admite la memoria, nueve, el trabajo tarda en
/// ceder al lector lo que tarda un lote entero, unos cinco segundos: justo lo que
/// se quería evitar. Con cuatro son un par de segundos. Por debajo se gana poco en
/// respuesta y se pierde en ritmo, porque el tiempo por fragmento crece con lotes
/// pequeños: 432 ms con uno solo frente a 377 con cuatro.
pub const LOTE_DE_FONDO: usize = 4;

/// Cuánto tiene que llevar quieto el lector para que el trabajo vuelva.
///
/// Un segundo y medio: cubre el paso de página con su animación y el desplazamiento
/// continuo, que avisa cada medio segundo como mucho, y no deja el trabajo parado
/// mientras alguien lee una página sin tocar nada.
pub const LECTOR_QUIETO: Duration = Duration::from_millis(1_500);

/// Lo que ocupa un vector en la base, con su fila y su índice, **medido** en
/// 620 bytes.
pub const BYTES_POR_VECTOR: u64 = 620;

/// Fragmentos que se suponen para un documento que todavía no se ha indexado:
/// **263**, la media medida sobre 40 libros reales con el troceado.
pub const FRAGMENTOS_SUPUESTOS: u64 = 263;

/// Milisegundos por fragmento que se suponen hasta haber medido en esta máquina:
/// **400**.
///
/// Medido el 2026-09-16 con el modelo real, 48 fragmentos de un libro del corpus,
/// lotes de cuatro y el presupuesto de fábrica —cuatro hilos de ocho, prioridad
/// baja— en un AMD Ryzen 5 3550H: 398 ms. Un libro de 263 fragmentos, algo menos de
/// dos minutos. Es de una máquina y no de todas, y por eso la estimación dice si
/// sale de esto o de lo que ya se midió en la de quien la lee, que sustituye a esto
/// en cuanto se ha preparado un lote.
pub const MS_POR_FRAGMENTO_DE_FABRICA: u64 = 400;

/// Motivo por el que el trabajo espera: el usuario lo pausó.
pub const PAUSA_DEL_USUARIO: &str = "USER_PAUSED";
/// Motivo por el que el trabajo espera: el lector está pintando.
pub const PAUSA_POR_EL_LECTOR: &str = "READER_ACTIVE";
/// Motivo por el que el trabajo se detuvo: no cabe en el tope de lo derivado.
pub const PAUSA_POR_PRESUPUESTO: &str = crate::search::PAUSA_PRESUPUESTO;

/// Qué se vectoriza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "scope",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "VectorScope")]
pub enum AlcanceDeVectores {
    /// Un documento, el que está abierto.
    Document { edition_id: String },
    /// Todos los de una colección.
    Collection { collection_id: String },
}

/// Lo que costará vectorizar un alcance, **antes** de hacerlo.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "VectorEstimate"
)]
pub struct Estimacion {
    /// Documentos que se vectorizarán.
    pub documents: u32,
    /// Documentos del alcance que el usuario excluyó.
    pub excluded: u32,
    /// Documentos sin indexar todavía, cuyos fragmentos se han supuesto.
    pub not_indexed: u32,
    #[ts(type = "number")]
    pub fragments: u64,
    /// Los que aún no tienen vector de este modelo.
    #[ts(type = "number")]
    pub pending: u64,
    /// Disco que ocuparán los que faltan.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Tiempo que tardarán, en segundos.
    #[ts(type = "number")]
    pub seconds: u64,
    /// Si el tiempo sale de lo medido en esta máquina o todavía es una suposición.
    pub measured_rate: bool,
    /// Si cabe en el tope de lo derivado. El documento abierto cabe siempre.
    pub fits: bool,
    /// Si hay un modelo de embeddings listo para calcular.
    pub model_ready: bool,
}

/// Cómo va el trabajo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "VectorJob")]
pub struct EstadoDelTrabajo {
    pub running: bool,
    pub paused: bool,
    /// Por qué espera ahora, si espera: `USER_PAUSED`, `READER_ACTIVE`,
    /// `AI_SUSPENDED_BATTERY`, `AI_SUSPENDED_HEAT` o `STORAGE_BUDGET_REACHED`.
    pub pause_reason: Option<&'static str>,
    pub edition_id: Option<String>,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    /// Documentos que esperan después del actual.
    pub queued: u32,
}

/// Lo que calcula los vectores. En la aplicación, el subsistema de IA.
pub trait Vectorizador: Send + Sync {
    /// El identificador del modelo con el que se calcula, si hay uno listo.
    fn modelo(&self) -> Option<&'static str>;
    /// Los vectores de unos textos.
    ///
    /// # Errors
    ///
    /// Los del proveedor que calcule.
    fn vectores(&self, textos: &[String]) -> NbResult<Vec<Embedding>>;
    /// Si el trabajo de fondo tiene que esperar a la batería o a la temperatura.
    fn pausa(&self) -> Option<Suspension>;
}

/// De dónde se abre el documento de una edición.
pub trait Documentos: Send + Sync {
    /// # Errors
    ///
    /// [`NbError::FileNotFound`] si no hay copia disponible, o los del proveedor.
    fn abrir(&self, edition_id: &str) -> NbResult<Box<dyn DocumentProvider>>;
}

/// Los documentos de la biblioteca, por la misma consulta que usa el indexador.
pub struct DeLaBiblioteca {
    pub db: Database,
    pub registry: Arc<crate::reader::ProviderRegistry>,
}

impl Documentos for DeLaBiblioteca {
    fn abrir(&self, edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        let conn = self.db.connection()?;
        let archivo: Option<(Option<String>, String)> = conn
            .query_row(
                &format!(
                    "SELECT {}, f.media_type FROM files f
                      WHERE f.edition_id = ?1 AND f.availability = 'available'
                      ORDER BY f.created_at LIMIT 1",
                    crate::db::catalog::SQL_RUTA_ABSOLUTA
                ),
                [edition_id],
                |fila| Ok((fila.get(0)?, fila.get(1)?)),
            )
            .ok();
        drop(conn);
        let Some((Some(ruta), tipo)) = archivo else {
            return Err(NbError::FileNotFound);
        };
        self.registry.open(&tipo, std::path::Path::new(&ruta))
    }
}

/// Un documento en la cola, y si espera al tope de lo derivado.
struct Encargo {
    edition_id: String,
    /// El documento abierto no espera al tope; los de una colección, sí.
    prioritario: bool,
}

#[derive(Default)]
struct Estado {
    cola: VecDeque<Encargo>,
    trabajando: bool,
    edition_id: Option<String>,
    hecho: u64,
    total: u64,
    motivo: Option<&'static str>,
}

/// El generador de vectores de la aplicación.
pub struct Generador {
    db: Database,
    indice: Arc<dyn VectorIndex>,
    vectorizador: Arc<dyn Vectorizador>,
    documentos: Arc<dyn Documentos>,
    bus: EventBus,
    /// Para medir lo derivado. Sin rutas —en una prueba— no hay tope que mirar.
    paths: Option<crate::paths::AppPaths>,
    estado: Mutex<Estado>,
    cancelado: AtomicBool,
    pausado: AtomicBool,
    /// Cuándo avisó el lector por última vez de que estaba pintando.
    lector: Mutex<Option<Instant>>,
    /// Lo que tardó de verdad cada fragmento en esta máquina, en microsegundos. Cero
    /// mientras no se haya medido.
    micros_por_fragmento: AtomicU64,
}

impl Generador {
    #[must_use]
    pub fn new(
        db: Database,
        indice: Arc<dyn VectorIndex>,
        vectorizador: Arc<dyn Vectorizador>,
        documentos: Arc<dyn Documentos>,
        bus: EventBus,
        paths: Option<crate::paths::AppPaths>,
    ) -> Self {
        Self {
            db,
            indice,
            vectorizador,
            documentos,
            bus,
            paths,
            estado: Mutex::new(Estado::default()),
            cancelado: AtomicBool::new(false),
            pausado: AtomicBool::new(false),
            lector: Mutex::new(None),
            micros_por_fragmento: AtomicU64::new(0),
        }
    }

    fn bloquear(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.estado.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Lo que costaría vectorizar un alcance.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si la colección no existe.
    /// - [`NbError::StorageFailure`] si la base no responde.
    pub fn estimar(&self, alcance: &AlcanceDeVectores) -> NbResult<Estimacion> {
        let conn = self.db.connection()?;
        let (ediciones, excluidas) = ediciones_del_alcance(&conn, alcance)?;
        let modelo = self.vectorizador.modelo();

        let mut fragmentos = 0_u64;
        let mut pendientes = 0_u64;
        let mut sin_indexar = 0_u32;
        for id in &ediciones {
            let (total, con_vector) = cobertura(&conn, id, modelo)?;
            if total == 0 {
                sin_indexar += 1;
                fragmentos += FRAGMENTOS_SUPUESTOS;
                pendientes += FRAGMENTOS_SUPUESTOS;
            } else {
                fragmentos += total;
                pendientes += total.saturating_sub(con_vector);
            }
        }

        let medido = self.micros_por_fragmento.load(Ordering::Relaxed);
        let micros = if medido == 0 {
            MS_POR_FRAGMENTO_DE_FABRICA * 1_000
        } else {
            medido
        };
        let bytes = pendientes * BYTES_POR_VECTOR;
        let cabe = match alcance {
            AlcanceDeVectores::Document { .. } => true,
            AlcanceDeVectores::Collection { .. } => self.cabe(&conn, bytes),
        };

        Ok(Estimacion {
            documents: u32::try_from(ediciones.len()).unwrap_or(u32::MAX),
            excluded: excluidas,
            not_indexed: sin_indexar,
            fragments: fragmentos,
            pending: pendientes,
            bytes,
            seconds: (pendientes * micros).div_ceil(1_000_000),
            measured_rate: medido != 0,
            fits: cabe,
            model_ready: modelo.is_some(),
        })
    }

    /// Si `bytes` más caben en el tope de lo derivado. Sin rutas, o sin poder
    /// medir, se dice que sí: el freno de verdad está al trabajar, que vuelve a
    /// medir.
    fn cabe(&self, conn: &rusqlite::Connection, bytes: u64) -> bool {
        let Some(paths) = &self.paths else {
            return true;
        };
        let Some(medida) = crate::diagnostics::medir_almacenamiento(conn, paths) else {
            return true;
        };
        if medida.documents_bytes == 0 {
            return true;
        }
        #[allow(clippy::cast_precision_loss)]
        let despues = (medida.derived_bytes + bytes) as f64 / medida.documents_bytes as f64 * 100.0;
        despues < medida.budget_percent
    }

    /// Empieza a vectorizar un alcance en segundo plano, o lo añade a lo que ya
    /// se está haciendo.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si no hay modelo de embeddings listo.
    /// - [`NbError::DocumentNotFound`] si la colección no existe.
    /// - [`NbError::StorageFailure`] si la base no responde.
    pub fn empezar(self: &Arc<Self>, alcance: &AlcanceDeVectores) -> NbResult<()> {
        if self.vectorizador.modelo().is_none() {
            return Err(NbError::ComponentUnavailable);
        }
        let conn = self.db.connection()?;
        let (ediciones, _) = ediciones_del_alcance(&conn, alcance)?;
        drop(conn);
        let prioritario = matches!(alcance, AlcanceDeVectores::Document { .. });

        let arrancar = {
            let mut estado = self.bloquear();
            for edition_id in ediciones {
                let ya = estado.edition_id.as_deref() == Some(edition_id.as_str())
                    || estado.cola.iter().any(|e| e.edition_id == edition_id);
                if ya {
                    continue;
                }
                let encargo = Encargo {
                    edition_id,
                    prioritario,
                };
                // Lo abierto pasa delante: es lo que alguien está esperando mirar.
                if prioritario {
                    estado.cola.push_front(encargo);
                } else {
                    estado.cola.push_back(encargo);
                }
            }
            let arrancar = !estado.trabajando && !estado.cola.is_empty();
            if arrancar {
                estado.trabajando = true;
            }
            arrancar
        };

        if arrancar {
            self.cancelado.store(false, Ordering::SeqCst);
            let generador = Arc::clone(self);
            let hilo = std::thread::Builder::new()
                .name("nb-vectores".to_owned())
                .spawn(move || generador.trabajar());
            if hilo.is_err() {
                self.bloquear().trabajando = false;
                return Err(NbError::Internal);
            }
        }
        Ok(())
    }

    /// Busca por significado con el mismo índice, el mismo vectorizador y los mismos
    /// documentos con que se prepara.
    ///
    /// # Errors
    ///
    /// Los de [`super::busqueda::por_significado`].
    pub fn buscar(
        &self,
        consulta: &str,
        alcance: &crate::search::Scope,
        limite: usize,
    ) -> NbResult<Vec<super::busqueda::Cercania>> {
        super::busqueda::por_significado(
            &super::busqueda::Fuentes {
                db: &self.db,
                indice: self.indice.as_ref(),
                vectorizador: self.vectorizador.as_ref(),
                documentos: self.documentos.as_ref(),
            },
            consulta,
            alcance,
            limite,
        )
    }

    /// El índice donde guarda. Lo necesita quien excluye, para borrar lo guardado.
    #[must_use]
    pub fn indice(&self) -> &dyn VectorIndex {
        self.indice.as_ref()
    }

    /// Pausa o reanuda.
    pub fn pausar(&self, pausado: bool) {
        self.pausado.store(pausado, Ordering::SeqCst);
    }

    /// Corta lo que se esté haciendo y vacía la cola. Lo ya guardado se queda.
    pub fn cancelar(&self) {
        self.cancelado.store(true, Ordering::SeqCst);
        self.bloquear().cola.clear();
    }

    /// El lector está pintando: el trabajo de fondo se aparta un momento.
    pub fn apuntar_actividad_del_lector(&self) {
        *self.lector.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    }

    /// Cómo va.
    #[must_use]
    pub fn estado(&self) -> EstadoDelTrabajo {
        let estado = self.bloquear();
        EstadoDelTrabajo {
            running: estado.trabajando,
            paused: self.pausado.load(Ordering::SeqCst),
            pause_reason: estado.motivo,
            edition_id: estado.edition_id.clone(),
            done: estado.hecho,
            total: estado.total,
            queued: u32::try_from(estado.cola.len()).unwrap_or(u32::MAX),
        }
    }

    fn trabajar(&self) {
        super::presupuesto::bajar_prioridad();
        let mut resultado = "done";
        let mut motivo: Option<&'static str> = None;

        loop {
            let encargo = {
                let mut estado = self.bloquear();
                match estado.cola.pop_front() {
                    Some(encargo) => {
                        estado.edition_id = Some(encargo.edition_id.clone());
                        estado.hecho = 0;
                        estado.total = 0;
                        encargo
                    }
                    None => {
                        estado.trabajando = false;
                        estado.edition_id = None;
                        estado.motivo = None;
                        break;
                    }
                }
            };

            if self.cancelado.load(Ordering::SeqCst) {
                resultado = "cancelled";
                continue;
            }

            if !encargo.prioritario {
                if let Ok(conn) = self.db.connection() {
                    if !self.cabe(&conn, 0) {
                        tracing::info!("vectores detenidos: lo derivado llegó a su tope");
                        resultado = "budget";
                        motivo = Some(PAUSA_POR_PRESUPUESTO);
                        self.bloquear().cola.clear();
                        continue;
                    }
                }
            }

            match self.vectorizar(&encargo.edition_id) {
                Ok(()) => {}
                Err(NbError::Cancelled) => resultado = "cancelled",
                Err(error) => {
                    // Un documento que falla no para los demás de una colección:
                    // se dice y se sigue.
                    tracing::info!(
                        codigo = error.code(),
                        "no se pudieron generar los vectores de un documento"
                    );
                    if resultado == "done" {
                        resultado = "failed";
                        motivo = Some(error.code());
                    }
                }
            }
        }

        self.bus.emit(DomainEvent::VectorJobFinished {
            outcome: resultado,
            reason_code: motivo,
        });
    }

    /// Los vectores de una edición: los que falten, por lotes, apartándose cuando
    /// toca.
    fn vectorizar(&self, edition_id: &str) -> NbResult<()> {
        let conn = self.db.connection()?;
        if excluidas(&conn)?.contains(edition_id) {
            return Ok(());
        }
        drop(conn);
        let modelo = self
            .vectorizador
            .modelo()
            .ok_or(NbError::ComponentUnavailable)?;

        let documento = self.documentos.abrir(edition_id)?;
        let producidos = documento.text_chunks(crate::search::FRAGMENTO)?;

        let mut conn = self.db.connection()?;
        let emparejados =
            match indice::alinear(&producidos, &indice::fragmentos_de(&conn, edition_id)?) {
                Some(emparejados) => emparejados,
                None => {
                    // Indexado con otro troceado, o sin indexar: se reindexa ahora, y
                    // entonces coinciden por construcción.
                    crate::search::index_edition(&mut conn, edition_id, documento.as_ref())?;
                    indice::alinear(&producidos, &indice::fragmentos_de(&conn, edition_id)?)
                        .ok_or(NbError::Internal)?
                }
            };
        drop(conn);

        let con_vector = self.indice.con_vector(modelo, edition_id)?;
        let total = emparejados.len() as u64;
        let pendientes: Vec<(String, String)> = emparejados
            .into_iter()
            .filter(|(id, _)| !con_vector.contains(id))
            .collect();
        let mut hecho = total - pendientes.len() as u64;
        self.avanzar(edition_id, hecho, total, None);

        for lote in pendientes.chunks(LOTE_DE_FONDO) {
            self.esperar_turno(edition_id, hecho, total)?;

            let inicio = Instant::now();
            let textos: Vec<String> = lote.iter().map(|(_, texto)| texto.clone()).collect();
            let vectores = self.vectorizador.vectores(&textos)?;
            if vectores.len() != lote.len() {
                return Err(NbError::Internal);
            }
            let pares: Vec<(String, Embedding)> = lote
                .iter()
                .map(|(id, _)| id.clone())
                .zip(vectores)
                .collect();
            self.indice.guardar(modelo, edition_id, &pares)?;
            self.medir_ritmo(inicio.elapsed(), lote.len());

            hecho += lote.len() as u64;
            self.avanzar(edition_id, hecho, total, None);
        }
        Ok(())
    }

    /// Espera mientras haya motivo para no calcular. Sale con
    /// [`NbError::Cancelled`] si se cancela mientras espera.
    fn esperar_turno(&self, edition_id: &str, hecho: u64, total: u64) -> NbResult<()> {
        let mut anunciado: Option<&'static str> = None;
        loop {
            if self.cancelado.load(Ordering::SeqCst) {
                return Err(NbError::Cancelled);
            }
            let motivo = self.motivo_para_esperar();
            if motivo != anunciado {
                self.avanzar(edition_id, hecho, total, motivo);
                anunciado = motivo;
            }
            if motivo.is_none() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// Por qué no se puede calcular ahora, si no se puede.
    fn motivo_para_esperar(&self) -> Option<&'static str> {
        if self.pausado.load(Ordering::SeqCst) {
            return Some(PAUSA_DEL_USUARIO);
        }
        if let Some(suspension) = self.vectorizador.pausa() {
            return Some(suspension.reason_code());
        }
        let ultima = *self.lector.lock().unwrap_or_else(|p| p.into_inner());
        if lector_activo(ultima, Instant::now()) {
            return Some(PAUSA_POR_EL_LECTOR);
        }
        None
    }

    fn avanzar(&self, edition_id: &str, hecho: u64, total: u64, motivo: Option<&'static str>) {
        let queued = {
            let mut estado = self.bloquear();
            estado.hecho = hecho;
            estado.total = total;
            estado.motivo = motivo;
            u32::try_from(estado.cola.len()).unwrap_or(u32::MAX)
        };
        self.bus.emit(DomainEvent::VectorJobProgress {
            edition_id: edition_id.to_owned(),
            done: hecho,
            total,
            queued,
            pause_reason: motivo,
        });
    }

    /// Apunta lo que tardó un lote, con una media que pesa más lo reciente: el
    /// primero incluye cargar el modelo, y la máquina cambia de carga.
    fn medir_ritmo(&self, tardo: Duration, fragmentos: usize) {
        if fragmentos == 0 {
            return;
        }
        let micros = u64::try_from(tardo.as_micros()).unwrap_or(u64::MAX) / fragmentos as u64;
        let anterior = self.micros_por_fragmento.load(Ordering::Relaxed);
        let nuevo = if anterior == 0 {
            micros
        } else {
            (anterior * 3 + micros) / 4
        };
        self.micros_por_fragmento.store(nuevo, Ordering::Relaxed);
    }
}

/// Si el lector avisó hace menos de [`LECTOR_QUIETO`].
#[must_use]
pub fn lector_activo(ultima: Option<Instant>, ahora: Instant) -> bool {
    ultima.is_some_and(|u| ahora.saturating_duration_since(u) < LECTOR_QUIETO)
}

/// Las ediciones de un alcance que se vectorizarían, y cuántas se excluyeron.
fn ediciones_del_alcance(
    conn: &rusqlite::Connection,
    alcance: &AlcanceDeVectores,
) -> NbResult<(Vec<String>, u32)> {
    let excluidas = excluidas(conn)?;
    let todas = match alcance {
        AlcanceDeVectores::Document { edition_id } => vec![edition_id.clone()],
        AlcanceDeVectores::Collection { collection_id } => {
            crate::collections::Collection::members(conn, collection_id)?
        }
    };
    let antes = todas.len();
    let quedan: Vec<String> = todas
        .into_iter()
        .filter(|id| !excluidas.contains(id))
        .collect();
    let fuera = u32::try_from(antes - quedan.len()).unwrap_or(u32::MAX);
    Ok((quedan, fuera))
}

/// Cuántos fragmentos tiene una edición y cuántos tienen vector del modelo.
fn cobertura(
    conn: &rusqlite::Connection,
    edition_id: &str,
    modelo: Option<&str>,
) -> NbResult<(u64, u64)> {
    conn.query_row(
        "SELECT (SELECT count(*) FROM text_chunks WHERE edition_id = ?1),
                (SELECT count(*) FROM chunk_vectors WHERE edition_id = ?1 AND model = ?2)",
        rusqlite::params![edition_id, modelo],
        |fila| {
            Ok((
                fila.get::<_, i64>(0)?.unsigned_abs(),
                fila.get::<_, i64>(1)?.unsigned_abs(),
            ))
        },
    )
    .map_err(|error| {
        tracing::error!(%error, "no se pudo medir la cobertura de vectores");
        NbError::StorageFailure
    })
}

/// Qué se excluye: una edición o una colección.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "VectorExclusionKind"
)]
pub enum Exclusion {
    Edition,
    Collection,
}

/// Las ediciones excluidas: las que lo están directamente y las de las colecciones
/// excluidas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
pub fn excluidas(conn: &rusqlite::Connection) -> NbResult<HashSet<String>> {
    let fallo = |error: rusqlite::Error| {
        tracing::error!(%error, "no se pudieron leer las exclusiones de vectores");
        NbError::StorageFailure
    };
    let mut conjunto: HashSet<String> = conn
        .prepare("SELECT edition_id FROM vector_excluded_editions")
        .and_then(|mut s| s.query_map([], |f| f.get(0))?.collect())
        .map_err(fallo)?;
    let colecciones: Vec<String> = conn
        .prepare("SELECT collection_id FROM vector_excluded_collections")
        .and_then(|mut s| s.query_map([], |f| f.get(0))?.collect())
        .map_err(fallo)?;
    for coleccion in colecciones {
        // Una colección que ya no se puede leer no excluye nada, pero tampoco
        // impide leer las demás.
        if let Ok(miembros) = crate::collections::Collection::members(conn, &coleccion) {
            conjunto.extend(miembros);
        }
    }
    Ok(conjunto)
}

/// Si algo está excluido directamente. Para enseñar el interruptor.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
pub fn esta_excluido(conn: &rusqlite::Connection, tipo: Exclusion, id: &str) -> NbResult<bool> {
    let tabla = match tipo {
        Exclusion::Edition => "SELECT 1 FROM vector_excluded_editions WHERE edition_id = ?1",
        Exclusion::Collection => {
            "SELECT 1 FROM vector_excluded_collections WHERE collection_id = ?1"
        }
    };
    conn.query_row(tabla, [id], |_| Ok(()))
        .map(|()| true)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            error => {
                tracing::error!(%error, "no se pudo leer una exclusión de vectores");
                Err(NbError::StorageFailure)
            }
        })
}

/// Excluye o vuelve a incluir una edición o una colección.
///
/// **Excluir borra los vectores que ya hubiera**: quien excluye un documento no
/// quiere que siga apareciendo en una búsqueda por significado, y un vector
/// guardado es justo lo que haría que apareciera.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la colección no existe.
/// - [`NbError::StorageFailure`] si la base no responde.
pub fn excluir(
    db: &Database,
    indice: &dyn VectorIndex,
    tipo: Exclusion,
    id: &str,
    excluido: bool,
) -> NbResult<()> {
    let conn = db.connection()?;
    let fallo = |error: rusqlite::Error| {
        tracing::error!(%error, "no se pudo guardar una exclusión de vectores");
        NbError::StorageFailure
    };
    let (poner, quitar) = match tipo {
        Exclusion::Edition => (
            "INSERT OR IGNORE INTO vector_excluded_editions (edition_id) VALUES (?1)",
            "DELETE FROM vector_excluded_editions WHERE edition_id = ?1",
        ),
        Exclusion::Collection => (
            "INSERT OR IGNORE INTO vector_excluded_collections (collection_id) VALUES (?1)",
            "DELETE FROM vector_excluded_collections WHERE collection_id = ?1",
        ),
    };
    conn.execute(if excluido { poner } else { quitar }, [id])
        .map_err(fallo)?;

    if excluido {
        let ediciones = match tipo {
            Exclusion::Edition => vec![id.to_owned()],
            Exclusion::Collection => crate::collections::Collection::members(&conn, id)?,
        };
        drop(conn);
        for edicion in ediciones {
            indice.olvidar(&edicion)?;
        }
    }
    Ok(())
}
