//! El modelo de embeddings cargado en memoria, y su liberación por inactividad.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 2.//! //!
//! # Por qué se libera
//!
//! Porque cargado ocupa cientos de megabytes y la mayor parte del tiempo nadie lo
//! usa. La aplicación en reposo ya mide tres veces su presupuesto de memoria
//!, y tener el modelo residente todo el día porque alguien buscó por
//! significado una vez por la mañana sería agravarlo sin que nadie lo decidiera.
//! Así que se carga al pedirlo y se suelta tras un periodo sin uso.
//!
//! # Por qué quien pide no lo nota
//!
//! Porque cargarlo es transparente: [`Residente::vectores`] carga si hace falta y
//! después calcula. Lo único que nota quien pide es que la primera petición tras
//! un periodo largo tarda lo que tarda cargar, y eso está medido y dicho en
//! [`INACTIVIDAD`].

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::xlm_roberta::{Config, XLMRobertaModel};
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use super::catalogo::{Agrupacion, Arquitectura, Modelo};
use super::presupuesto::{self, Presupuesto};
use super::Embedding;
use crate::error::{NbError, NbResult};

/// Cuántos textos se calculan de una vez, como mucho.
///
/// Dieciséis, y **por debajo lo decide el presupuesto de memoria**: ver
/// [`lote_para`]. Por encima no se gana nada que medir: con textos de 512 tokens el
/// tiempo por texto fue de 432 ms con uno solo, 377 con cuatro y 357 con dieciséis,
/// y la memoria de trabajo sí sigue creciendo en línea recta.
pub const LOTE: usize = 16;

/// Tiempo sin uso tras el que se suelta el modelo: **cinco minutos**.
///
/// Se fijó con las dos cifras delante, medidas el 2026-09-16 sobre el modelo real
/// y en release. **Tenerlo cargado cuesta 700 MB**, más del doble de lo que ocupa la
/// aplicación entera en reposo. **Volver a cargarlo cuesta un segundo y medio**: la
/// primera petición tardó 1,74 s y la siguiente, con el modelo ya cargado, 185 ms.
///
/// Lo que se equilibra es esto: demasiado corto, y cada búsqueda por significado
/// después de leer un rato empieza con segundo y medio de espera; demasiado largo,
/// y 700 MB siguen ocupados mucho después de la última vez. Cinco minutos cubren una
/// sesión de consultas seguidas —abrir un documento y buscar en él varias veces— y
/// devuelven la memoria en cuanto se vuelve a leer sin buscar. Es un punto de
/// partida con motivo, no una medida de uso: el presupuesto lo convierte en parte del
/// presupuesto de recursos configurable.
pub const INACTIVIDAD: Duration = Duration::from_secs(300);

/// Un modelo de embeddings cargado.
pub struct ModeloCargado {
    red: XLMRobertaModel,
    tokenizador: Tokenizer,
    dispositivo: Device,
    agrupacion: Agrupacion,
}

impl ModeloCargado {
    /// Carga un modelo del catálogo desde su carpeta verificada.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentCorrupted`] si algún archivo no se deja leer como lo
    ///   que dice ser. Es lo que caza un archivo del mismo tamaño alterado a mano,
    ///   que la comprobación por tamaño del arranque no puede ver.
    /// - [`NbError::FormatUnsupported`] si la arquitectura no es una que se sepa
    ///   cargar.
    pub fn cargar(carpeta: &Path, entrada: &Modelo, dispositivo: &Device) -> NbResult<Self> {
        match entrada.arquitectura {
            Arquitectura::XlmRoberta => {}
            // Un modelo que genera no hace vectores: lo carga `generativo`.
            Arquitectura::Qwen2Gguf | Arquitectura::Qwen3Gguf => {
                return Err(NbError::FormatUnsupported)
            }
        }

        let corrupto = |que: &'static str| {
            move |error: &dyn std::fmt::Display| {
                // Ni la ruta ni el texto del error: puede llevar la ruta dentro.
                let _ = error;
                tracing::warn!(
                    // no es del usuario: es un nombre fijo de los archivos del modelo.
                    archivo = que,
                    "un archivo del modelo no se deja cargar"
                );
                NbError::DocumentCorrupted
            }
        };

        let configuracion: Config = std::fs::read(carpeta.join("config.json"))
            .map_err(|e| corrupto("config.json")(&e))
            .and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|e| corrupto("config.json")(&e))
            })?;

        let mut tokenizador = Tokenizer::from_file(carpeta.join("tokenizer.json"))
            .map_err(|e| corrupto("tokenizer.json")(&e))?;
        tokenizador
            .with_truncation(Some(TruncationParams {
                max_length: entrada.longitud_maxima,
                ..TruncationParams::default()
            }))
            .map_err(|e| corrupto("tokenizer.json")(&e))?;
        tokenizador.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            pad_id: configuracion.pad_token_id,
            ..PaddingParams::default()
        }));

        let pesos = carpeta.join("model.safetensors");
        // SEGURIDAD: `from_mmaped_safetensors` proyecta el archivo en memoria, y es
        // inseguro porque si el archivo cambiara mientras está proyectado, lo leído
        // cambiaría debajo. Aquí el archivo vive en la carpeta de datos de la
        // aplicación, solo aparece tras verificar su huella y solo lo borra
        // `desinstalar`, que no puede correr mientras se instala. Se prefiere a
        // leerlo entero porque la conversión a coma flotante de 32 bits copia los
        // pesos de todos modos, y proyectado el original no se duplica en memoria
        // anónima: el sistema lo puede soltar.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[pesos], DType::F32, dispositivo) }
            .map_err(|e| corrupto("model.safetensors")(&e))?;
        let red = XLMRobertaModel::new(&configuracion, vb)
            .map_err(|e| corrupto("model.safetensors")(&e))?;

        Ok(Self {
            red,
            tokenizador,
            dispositivo: dispositivo.clone(),
            agrupacion: entrada.agrupacion,
        })
    }

    /// Los vectores de unos textos, normalizados a longitud uno.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cálculo falla, que con el modelo cargado y la
    /// memoria disponible no debería ocurrir.
    pub fn vectores(&self, textos: &[String], lote: usize) -> NbResult<Vec<Embedding>> {
        let mut todos = Vec::with_capacity(textos.len());
        for lote in textos.chunks(lote.max(1)) {
            todos.extend(self.lote(lote)?);
        }
        Ok(todos)
    }

    fn lote(&self, textos: &[String]) -> NbResult<Vec<Embedding>> {
        let fallo = |error: &dyn std::fmt::Display| {
            let _ = error;
            tracing::warn!("el cálculo de un lote de embeddings falló");
            NbError::Internal
        };

        let codificados = self
            .tokenizador
            .encode_batch(textos.to_vec(), true)
            .map_err(|e| fallo(&e))?;

        let ids: Vec<Vec<u32>> = codificados.iter().map(|c| c.get_ids().to_vec()).collect();
        let mascara: Vec<Vec<u32>> = codificados
            .iter()
            .map(|c| c.get_attention_mask().to_vec())
            .collect();

        let ids = Tensor::new(ids, &self.dispositivo).map_err(|e| fallo(&e))?;
        let mascara = Tensor::new(mascara, &self.dispositivo).map_err(|e| fallo(&e))?;
        let tipos = ids.zeros_like().map_err(|e| fallo(&e))?;

        let ocultos = self
            .red
            .forward(&ids, &mascara, &tipos, None, None, None)
            .map_err(|e| fallo(&e))?;

        let agrupados = match self.agrupacion {
            // El primer token de cada secuencia: [lote, secuencia, dimensión] a
            // [lote, dimensión].
            Agrupacion::Cls => ocultos.narrow(1, 0, 1).and_then(|t| t.squeeze(1)),
            Agrupacion::Ninguna => return Err(NbError::FormatUnsupported),
        }
        .map_err(|e| fallo(&e))?;

        // Normalizados, que es lo que dice la ficha del modelo y lo que permite
        // comparar por producto escalar en lugar de por coseno: con longitud uno
        // son lo mismo, y el producto es lo que calcula la consulta de ADR-007.
        let normas = agrupados
            .sqr()
            .and_then(|t| t.sum_keepdim(1))
            .and_then(|t| t.sqrt())
            .map_err(|e| fallo(&e))?;
        let normalizados = agrupados.broadcast_div(&normas).map_err(|e| fallo(&e))?;

        normalizados.to_vec2::<f32>().map_err(|e| fallo(&e))
    }
}

/// Devuelve al sistema la memoria que el asignador se quedó al soltar el modelo.
///
/// # Por qué hace falta, medido
///
/// Soltar el modelo **libera** su memoria, pero no la **devuelve**: el asignador
/// de glibc se queda con lo liberado para reutilizarlo. Medido el 2026-09-16 con el
/// modelo real, el proceso pasaba de 5 MB a 705 con el modelo cargado, y al soltarlo
/// se quedaba en **277**: el criterio, "la memoria se devuelve", se
/// cumplía a medias. Tras `malloc_trim` bajaba a **10**. La aplicación en reposo ya
/// triplica su presupuesto de memoria, así que dejar 270 MB retenidos tras
/// una búsqueda por significado es justo lo que no puede pasar.
///
/// Solo en Linux con glibc, que es donde se midió. Con otros asignadores —el de
/// macOS, el de Windows, musl— no se ha medido: está anotado en
/// `deferred-verification.md` en lugar de darse por bueno. Recortar cuesta recorrer
/// el montículo, unos milisegundos, y ocurre solo al soltar el modelo.
pub(crate) fn devolver_memoria_al_sistema() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SEGURIDAD: `malloc_trim` no recibe punteros y es segura entre hilos; solo le
    // pide al asignador que devuelva al sistema la memoria libre que retiene.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// Un modelo que se carga al pedirlo y se suelta tras un periodo sin uso.
///
/// Ahora calcula **dentro de su presupuesto**: en su propio grupo de hilos,
/// con la prioridad bajada, con un tope de memoria que decide si se carga y de qué
/// tamaño son los lotes. Ver `presupuesto`.
pub struct Residente {
    carpeta: PathBuf,
    entrada: &'static Modelo,
    dispositivo: Device,
    inactividad: Duration,
    presupuesto: Mutex<Presupuesto>,
    cargado: Mutex<Option<Cargado>>,
}

/// Lo que existe mientras el modelo está cargado, y desaparece con él.
///
/// **El grupo de hilos va aquí y no en el residente**: vive lo que vive el modelo.
/// Un grupo creado al arrancar tendría sus hilos dormidos todo el día en un equipo
/// donde nadie usa la IA, y soltarlo con el modelo hace que tras la inactividad no
/// quede ni un hilo de inferencia en el proceso.
struct Cargado {
    modelo: Arc<ModeloCargado>,
    grupo: Arc<rayon::ThreadPool>,
    uso: Instant,
}

impl Residente {
    #[must_use]
    pub fn new(
        carpeta: PathBuf,
        entrada: &'static Modelo,
        dispositivo: Device,
        inactividad: Duration,
        presupuesto: Presupuesto,
    ) -> Self {
        Self {
            carpeta,
            entrada,
            dispositivo,
            inactividad,
            presupuesto: Mutex::new(presupuesto),
            cargado: Mutex::new(None),
        }
    }

    /// El identificador del modelo del catálogo que carga.
    #[must_use]
    pub const fn modelo(&self) -> &'static str {
        self.entrada.id
    }

    /// El presupuesto vigente.
    #[must_use]
    pub fn presupuesto(&self) -> Presupuesto {
        *self.presupuesto.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Si el modelo cabe en el tope de memoria vigente.
    #[must_use]
    pub fn cabe_en_memoria(&self) -> bool {
        cabe(self.entrada, self.presupuesto())
    }

    /// Cambia el presupuesto **sin esperar al siguiente arranque**.
    ///
    /// Si el modelo está cargado y el tope nuevo ya no lo admite, se suelta ahora:
    /// un tope que solo se aplica a la próxima carga no es un tope, es una
    /// intención. Si cambian los hilos, el grupo se rehace; un cálculo en curso
    /// termina en el grupo viejo, que sostiene su propia referencia.
    pub fn aplicar(&self, nuevo: Presupuesto) {
        let anterior = std::mem::replace(
            &mut *self.presupuesto.lock().unwrap_or_else(|p| p.into_inner()),
            nuevo,
        );
        if anterior == nuevo {
            return;
        }

        let mut cargado = self.cargado.lock().unwrap_or_else(|p| p.into_inner());
        let Some(actual) = cargado.as_mut() else {
            return;
        };
        if !cabe(self.entrada, nuevo) {
            *cargado = None;
            drop(cargado);
            devolver_memoria_al_sistema();
            tracing::info!("modelo de IA soltado: el tope de memoria nuevo no lo admite");
            return;
        }
        if anterior.hilos != nuevo.hilos {
            match presupuesto::grupo_de_hilos(nuevo.hilos) {
                Ok(grupo) => actual.grupo = grupo,
                Err(_) => {
                    // Sin grupo nuevo no se sigue con el viejo, que tendría el tope
                    // anterior: se suelta, y la próxima petición lo vuelve a
                    // intentar.
                    *cargado = None;
                    tracing::warn!("no se pudo rehacer el grupo de hilos de la inferencia");
                }
            }
        }
    }

    /// Los vectores de unos textos, cargando el modelo si no lo está.
    ///
    /// Todo ocurre **dentro del grupo de la inferencia**, también la carga y el
    /// tokenizado: `install` ejecuta el cálculo en uno de sus hilos, que son los de
    /// prioridad baja, y lo que Candle reparta desde ahí se reparte entre ellos.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si el modelo no cabe en el tope de
    ///   memoria vigente. No se intenta cargar.
    /// - Los de [`ModeloCargado::cargar`] y [`ModeloCargado::vectores`].
    pub fn vectores(self: &Arc<Self>, textos: &[String]) -> NbResult<Vec<Embedding>> {
        let presupuesto = self.presupuesto();
        if !cabe(self.entrada, presupuesto) {
            return Err(NbError::ComponentUnavailable);
        }
        let (modelo, grupo) = self.tomar()?;
        let lote = lote_para(self.entrada, presupuesto);
        let resultado = grupo.install(|| modelo.vectores(textos, lote));
        // Se apunta el uso también al terminar: un cálculo largo no puede contar
        // como inactividad, o el modelo se soltaría justo al acabar de usarse.
        self.apuntar_uso();
        resultado
    }

    /// Si está cargado ahora.
    #[must_use]
    pub fn esta_cargado(&self) -> bool {
        self.cargado
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    }

    /// Cuántos hilos tiene el grupo de la inferencia ahora. `None` sin modelo
    /// cargado, que es cuando no hay grupo.
    #[must_use]
    pub fn hilos_vivos(&self) -> Option<usize> {
        self.cargado
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|c| c.grupo.current_num_threads())
    }

    /// Suelta el modelo si lleva más de su periodo sin usarse. Devuelve si lo
    /// soltó.
    ///
    /// Lo que se suelta es la referencia de aquí. Si un cálculo está en curso, el
    /// modelo sigue vivo hasta que ese cálculo termine, porque lo sostiene su
    /// propia referencia: soltar no puede romper a quien está usándolo.
    pub fn liberar_si_inactivo(&self) -> bool {
        let mut cargado = self.cargado.lock().unwrap_or_else(|p| p.into_inner());
        let inactivo = cargado
            .as_ref()
            .is_some_and(|c| c.uso.elapsed() >= self.inactividad);
        if inactivo {
            *cargado = None;
            drop(cargado);
            devolver_memoria_al_sistema();
            tracing::info!("modelo de IA liberado por inactividad");
        }
        inactivo
    }

    /// Vigila la inactividad en un hilo propio **mientras haya modelo cargado**.
    ///
    /// Solo mientras está cargado: sin modelo no hay nada que soltar, y un hilo
    /// despertándose cada minuto para mirar una memoria vacía sería trabajo en
    /// reposo, que es justo lo que el proyecto no admite. Termina en cuanto suelta
    /// el modelo, y también si el residente desaparece, porque lo sostiene con una
    /// referencia débil.
    fn vigilar(self: &Arc<Self>) {
        let debil: Weak<Self> = Arc::downgrade(self);
        let vuelta = (self.inactividad / 4).max(Duration::from_millis(10));
        let _ = std::thread::Builder::new()
            .name("nb-ia-inactividad".to_owned())
            .spawn(move || loop {
                std::thread::sleep(vuelta);
                let Some(residente) = debil.upgrade() else {
                    return;
                };
                if residente.liberar_si_inactivo() || !residente.esta_cargado() {
                    return;
                }
            });
    }

    fn tomar(self: &Arc<Self>) -> NbResult<(Arc<ModeloCargado>, Arc<rayon::ThreadPool>)> {
        let mut cargado = self.cargado.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(actual) = cargado.as_mut() {
            actual.uso = Instant::now();
            return Ok((Arc::clone(&actual.modelo), Arc::clone(&actual.grupo)));
        }

        let grupo = presupuesto::grupo_de_hilos(self.presupuesto().hilos).map_err(|_| {
            tracing::warn!("no se pudo crear el grupo de hilos de la inferencia");
            NbError::ComponentUnavailable
        })?;
        let inicio = Instant::now();
        // Cargar también en el grupo: convertir 214 MB de pesos es segundo y medio
        // de procesador, y es trabajo de la inferencia como cualquier otro.
        let modelo =
            Arc::new(grupo.install(|| {
                ModeloCargado::cargar(&self.carpeta, self.entrada, &self.dispositivo)
            })?);
        tracing::info!(
            milisegundos = inicio.elapsed().as_millis() as u64,
            "modelo de IA cargado"
        );
        *cargado = Some(Cargado {
            modelo: Arc::clone(&modelo),
            grupo: Arc::clone(&grupo),
            uso: Instant::now(),
        });
        drop(cargado);
        self.vigilar();
        Ok((modelo, grupo))
    }

    fn apuntar_uso(&self) {
        if let Some(actual) = self
            .cargado
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            actual.uso = Instant::now();
        }
    }
}

/// Si un modelo cabe en un presupuesto: su memoria declarada, con un lote de un
/// texto, no pasa del tope.
#[must_use]
pub fn cabe(entrada: &Modelo, presupuesto: Presupuesto) -> bool {
    entrada.memoria.saturating_add(entrada.memoria_por_texto) <= presupuesto.memoria
}

/// De qué tamaño son los lotes con este presupuesto.
///
/// Lo que el modelo ocupa cargado es fijo; lo que crece con el lote es la memoria de
/// trabajo del cálculo. Así que el lote es lo que cabe en el margen entre el modelo y
/// el tope, sin pasar de [`LOTE`] ni bajar de uno.
#[must_use]
pub fn lote_para(entrada: &Modelo, presupuesto: Presupuesto) -> usize {
    let margen = presupuesto.memoria.saturating_sub(entrada.memoria);
    let caben = usize::try_from(margen / entrada.memoria_por_texto.max(1)).unwrap_or(usize::MAX);
    caben.clamp(1, LOTE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalogo::GRANITE_107M_MULTILINGUAL;

    /// **El lote nunca lleva la memoria por encima del tope**, con cualquier tope
    /// en el que el modelo quepa; nunca baja de uno ni pasa de [`LOTE`]. Y con el
    /// de fábrica sale el lote que midió la prueba del fotograma.
    #[test]
    fn el_lote_cabe_siempre_en_el_tope_de_memoria() {
        let modelo = &GRANITE_107M_MULTILINGUAL;
        const MIB: u64 = 1024 * 1024;

        for mib in (512..=8192).step_by(8) {
            let presupuesto = Presupuesto {
                hilos: 2,
                memoria: mib * MIB,
            };
            if !cabe(modelo, presupuesto) {
                continue;
            }
            let lote = lote_para(modelo, presupuesto);
            assert!((1..=LOTE).contains(&lote));
            assert!(
                modelo.memoria + lote as u64 * modelo.memoria_por_texto <= presupuesto.memoria,
                "con {mib} MiB el lote de {lote} pasa del tope"
            );
        }

        assert_eq!(lote_para(modelo, Presupuesto::de_fabrica(8)), 9);
        assert_eq!(lote_para(modelo, Presupuesto::desde(0.0, 4096.0, 8)), LOTE);
        assert!(!cabe(modelo, Presupuesto::desde(0.0, 512.0, 8)));
    }
}
