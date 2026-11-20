//! Generar en local: el modelo pequeño que resume, hace tarjetas y saca conceptos
//! sin que el texto salga del equipo.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 6.
//!
//! # Lo que cuesta, medido y no supuesto
//!
//! `local.rs` avisaba de que en generación cuantizada Candle es más
//! lento que `llama.cpp` y de que había que medirlo antes de prometerla. Medido el
//! 2026-09-17 con Qwen2.5-1.5B en Q4_K_M, cuatro hilos y una entrada de 813 tokens:
//!
//! - **Con AVX2 compilado**, 14 tokens por segundo al leer la entrada y 4,4 al
//!   escribir: un resumen de un capítulo corto son dos minutos. Se puede, y se
//!   hace mientras se sigue leyendo.
//! - **Sin AVX2 compilado, 1 token por segundo**: catorce minutos solo para leer la
//!   entrada. Los núcleos cuantizados de Candle eligen su versión vectorizada **al
//!   compilar** (`#[cfg(target_feature = "avx2")]`), no al ejecutar, y la
//!   compilación por defecto para x86-64 no la incluye aunque el procesador la
//!   tenga.
//!
//! Así que en x86-64 este proveedor **solo se declara disponible si el binario se
//! compiló con AVX2**. Ofrecer catorce minutos de espera por un resumen sería peor
//! que no ofrecerlo, y está dicho en Ajustes. Cómo se distribuye un binario con
//! AVX2 sin dejar fuera a los procesadores que no la tienen es una decisión de
//! empaquetado que no se toma aquí: ver `local-models.md` sección 6.
//!
//! # Uno a la vez
//!
//! Un modelo que genera ocupa dos gigabytes y cuatro núcleos durante minutos. Dos
//! generaciones a la vez no acabarían antes: se turnan, y la segunda espera a la
//! primera.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::{LogitsProcessor, Sampling};
use candle_transformers::models::{quantized_qwen2, quantized_qwen3};
use tokenizers::Tokenizer;

use super::catalogo::{Arquitectura, Modelo};
use super::hardware::{self, Recursos};
use super::presupuesto::{self, Presupuesto};
use super::{
    contexto, Absence, AiProvider, Availability, Capability, Completion, Placement, ProviderInfo,
    Request,
};
use crate::error::{NbError, NbResult};

/// Identificador estable. Se guarda con cada salida generada.
pub const ID: &str = "local-generativo";

/// Lo que el modelo local atiende: **todo**, también las preguntas de autoevaluación.
///
/// Antes las preguntas no estaban, por lo que se midió el 2026-09-17:
/// los modelos pequeños devuelven las cuatro opciones iguales, o la frase del texto
/// con una palabra cambiada y la clave puesta en la equivocada. Ese día el equipo
/// corrigió el criterio -"una cosa es recomendar y otra obligar"- y lo medido dejó de
/// ser motivo para esconder la función: pasó a ser **lo que se le dice a quien la
/// elige**. Está en [`DESACONSEJADAS`], y la interfaz lo enseña antes de generar.
const CAPACIDADES: &[Capability] = &[
    Capability::Summary,
    Capability::Flashcards,
    Capability::Concepts,
    Capability::Questions,
];

/// Lo que se ofrece **sin recomendarlo**, con el código de por qué.
///
/// `LOCAL_QUESTIONS_UNRELIABLE`: medido sobre el corpus el 2026-09-17, las preguntas
/// de opción múltiple salen con las opciones repetidas o con la clave en la opción
/// equivocada. Una pregunta con la clave corrida hace que se memorice un error, y eso
/// es lo que hay que saber antes de elegirla.
pub const DESACONSEJADAS: &[(Capability, &str)] =
    &[(Capability::Questions, "LOCAL_QUESTIONS_UNRELIABLE")];

/// Caracteres de documento por petición: unos 1.700 tokens, **dos minutos** de
/// lectura de entrada con AVX2. Más no cabe en la espera que se puede pedir.
pub const CARACTERES: usize = 6_000;

/// Tokens que escribe como mucho. Diez tarjetas en JSON son unos 700.
pub const MAXIMO_DE_SALIDA: usize = 900;

/// Cuánto tarda en soltarse sin uso. Menos que el de embeddings: ocupa el triple,
/// y quien estudia un capítulo no pide otro resumen cada minuto.
pub const INACTIVIDAD: Duration = Duration::from_secs(180);

/// Si los núcleos cuantizados de Candle están compilados en su versión rápida.
///
/// En ARM de 64 bits NEON es parte de la arquitectura y siempre lo están. En x86-64
/// depende de cómo se compiló: ver el encabezado del módulo.
#[must_use]
pub const fn motor_rapido() -> bool {
    (cfg!(target_arch = "x86_64") && cfg!(target_feature = "avx2")) || cfg!(target_arch = "aarch64")
}

/// Las arquitecturas que sabe ejecutar, cargadas.
///
/// Dos y no una porque el catálogo ofrece **varios modelos** y la máquina de cada
/// persona no es la misma: Se decidió el 2026-09-17. Añadir una tercera es
/// añadir una rama aquí y una entrada en el catálogo.
enum Red {
    Qwen2(quantized_qwen2::ModelWeights),
    Qwen3(quantized_qwen3::ModelWeights),
}

impl Red {
    fn olvidar_el_contexto(&mut self) {
        match self {
            Self::Qwen2(m) => m.clear_kv_cache(),
            Self::Qwen3(m) => m.clear_kv_cache(),
        }
    }

    fn adelante(&mut self, entrada: &Tensor, posicion: usize) -> candle_core::Result<Tensor> {
        match self {
            Self::Qwen2(m) => m.forward(entrada, posicion),
            Self::Qwen3(m) => m.forward(entrada, posicion),
        }
    }

    /// Lo que se escribe después de abrir el turno del asistente.
    ///
    /// Qwen3 razona antes de contestar y lo escribe; en una función de estudio eso
    /// son cientos de tokens de deliberación que nadie lee y que aquí cuestan un
    /// minuto. El bloque de pensamiento vacío le dice que ya ha pensado.
    const fn apertura(&self) -> &'static str {
        match self {
            Self::Qwen2(_) => "",
            Self::Qwen3(_) => "<think>\n\n</think>\n\n",
        }
    }
}

struct Cargado {
    /// Cuál está cargado: cambiar de modelo en Ajustes suelta este y trae el otro.
    entrada: &'static Modelo,
    /// Dónde calcula. En una edición sin CUDA ni Metal, siempre el procesador.
    dispositivo: Device,
    red: Red,
    tokenizador: Tokenizer,
    fin: Vec<u32>,
    grupo: Arc<rayon::ThreadPool>,
    uso: Instant,
}

/// El proveedor que genera en local, con **el modelo que la persona haya elegido**.
pub struct GenerativoLocal {
    recursos: Recursos,
    /// Cuál se usa. De fábrica, el que se recomienda para este equipo.
    elegido: Mutex<&'static Modelo>,
    modelos: PathBuf,
    presupuesto: Mutex<Presupuesto>,
    /// El modelo cargado. El cerrojo es también el turno: quien genera lo tiene.
    cargado: Mutex<Option<Cargado>>,
    cancelado: Arc<AtomicBool>,
    rapido: bool,
}

impl GenerativoLocal {
    /// Prepara el proveedor. No carga nada.
    #[must_use]
    pub fn nuevo(modelos: &Path, presupuesto: Presupuesto, cancelado: Arc<AtomicBool>) -> Self {
        let recursos = hardware::detectar();
        let elegido = super::catalogo::recomendado(recursos.memoria_total, recursos.nucleos)
            .unwrap_or(&super::catalogo::QWEN25_15B_INSTRUCT);
        Self {
            recursos,
            elegido: Mutex::new(elegido),
            modelos: modelos.to_path_buf(),
            presupuesto: Mutex::new(presupuesto),
            cargado: Mutex::new(None),
            cancelado,
            rapido: motor_rapido(),
        }
    }

    /// El modelo que se usa ahora.
    #[must_use]
    pub fn elegido(&self) -> &'static Modelo {
        *self.elegido.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Lo que es probable que pase con `modelo` en este equipo y con esta
    /// compilación. Ver [`super::catalogo::prevision`].
    #[must_use]
    pub fn prevision(&self, modelo: &Modelo) -> super::catalogo::Prevision {
        super::catalogo::prevision(
            modelo,
            self.recursos.memoria_total,
            self.recursos.nucleos,
            self.rapido,
        )
    }

    /// El que se recomienda para este equipo, que es el de fábrica.
    #[must_use]
    pub fn recomendado(&self) -> Option<&'static Modelo> {
        super::catalogo::recomendado(self.recursos.memoria_total, self.recursos.nucleos)
    }

    /// Cambia de modelo. Uno que no exista o que no genere no cambia nada.
    ///
    /// **Suelta el que estuviera cargado**: dos modelos generativos a la vez son
    /// cuatro gigabytes, y quien acaba de elegir otro espera que conteste el otro.
    pub fn usar(&self, id: &str) {
        let Some(nuevo) = super::catalogo::generativos().find(|m| m.id == id) else {
            return;
        };
        let mut elegido = self.elegido.lock().unwrap_or_else(|p| p.into_inner());
        if elegido.id == nuevo.id {
            return;
        }
        *elegido = nuevo;
        drop(elegido);
        if let Ok(mut cargado) = self.cargado.try_lock() {
            *cargado = None;
        }
        tracing::info!(modelo = nuevo.id, "modelo generativo elegido");
    }

    /// Cambia el presupuesto; si el modelo ya no cabe, se suelta ahora.
    pub fn aplicar(&self, nuevo: Presupuesto) {
        *self.presupuesto.lock().unwrap_or_else(|p| p.into_inner()) = nuevo;
        if !super::modelo::cabe(self.elegido(), nuevo) {
            // Si está generando, el cerrojo lo tiene la generación: se soltará al
            // terminar, en la siguiente vuelta de la inactividad.
            if let Ok(mut cargado) = self.cargado.try_lock() {
                *cargado = None;
            }
        }
    }

    fn presupuesto(&self) -> Presupuesto {
        *self.presupuesto.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Suelta el modelo si lleva su periodo sin usarse. Devuelve si lo soltó.
    pub fn liberar_si_inactivo(&self) -> bool {
        let Ok(mut cargado) = self.cargado.try_lock() else {
            return false;
        };
        let inactivo = cargado
            .as_ref()
            .is_some_and(|c| c.uso.elapsed() >= INACTIVIDAD);
        if inactivo {
            *cargado = None;
            tracing::info!("modelo generativo liberado por inactividad");
        }
        inactivo
    }

    fn vigilar(self: &Arc<Self>) {
        let debil = Arc::downgrade(self);
        let _ = std::thread::Builder::new()
            .name("nb-ia-generativo-inactividad".to_owned())
            .spawn(move || loop {
                std::thread::sleep(INACTIVIDAD / 4);
                let Some(proveedor) = debil.upgrade() else {
                    return;
                };
                let vacio = proveedor
                    .cargado
                    .try_lock()
                    .map(|c| c.is_none())
                    .unwrap_or(false);
                if proveedor.liberar_si_inactivo() || vacio {
                    return;
                }
            });
    }

    fn cargar(&self, entrada: &'static Modelo, grupo: Arc<rayon::ThreadPool>) -> NbResult<Cargado> {
        let carpeta = super::instalacion::carpeta(&self.modelos, entrada);
        let corrupto = |archivo: &'static str| {
            tracing::warn!(
                // no es del usuario: es un nombre fijo de los archivos del modelo.
                archivo,
                "un archivo del modelo generativo no se deja cargar"
            );
            NbError::DocumentCorrupted
        };
        let inicio = Instant::now();
        // **El dispositivo que el motor contesta**, no uno fijo. Antes
        // era `Device::Cpu` escrito aquí; ese día se prepararon las ediciones con
        // CUDA y Metal (decisión de producto, sin probar: aquí no hay con qué), y sin
        // esto habrían compilado la GPU para no usarla. En una edición sin esas
        // opciones `detectar` contesta procesador y nada cambia.
        let dispositivo = super::local::dispositivo_para(self.recursos.acelerador);
        let (red, tokenizador) = grupo.install(|| {
            let mut archivo = std::fs::File::open(carpeta.join("model.gguf"))
                .map_err(|_| corrupto("model.gguf"))?;
            let contenido =
                gguf_file::Content::read(&mut archivo).map_err(|_| corrupto("model.gguf"))?;
            let red = match entrada.arquitectura {
                Arquitectura::Qwen2Gguf => {
                    quantized_qwen2::ModelWeights::from_gguf(contenido, &mut archivo, &dispositivo)
                        .map(Red::Qwen2)
                }
                Arquitectura::Qwen3Gguf => {
                    quantized_qwen3::ModelWeights::from_gguf(contenido, &mut archivo, &dispositivo)
                        .map(Red::Qwen3)
                }
                Arquitectura::XlmRoberta => return Err(NbError::FormatUnsupported),
            }
            .map_err(|_| corrupto("model.gguf"))?;
            let tokenizador = Tokenizer::from_file(carpeta.join("tokenizer.json"))
                .map_err(|_| corrupto("tokenizer.json"))?;
            Ok::<_, NbError>((red, tokenizador))
        })?;
        let fin = ["<|im_end|>", "<|endoftext|>"]
            .iter()
            .filter_map(|t| tokenizador.token_to_id(t))
            .collect();
        tracing::info!(
            modelo = entrada.id,
            milisegundos = inicio.elapsed().as_millis() as u64,
            "modelo generativo cargado"
        );
        tracing::info!(dispositivo = ?self.recursos.acelerador, "la generación calcula aquí");
        Ok(Cargado {
            entrada,
            dispositivo,
            red,
            tokenizador,
            fin,
            grupo,
            uso: Instant::now(),
        })
    }

    /// Genera sobre el modelo cargado, cargándolo si hace falta.
    fn generar_con(self: &Arc<Self>, request: &Request) -> NbResult<String> {
        let presupuesto = self.presupuesto();
        let entrada = self.elegido();
        if !super::modelo::cabe(entrada, presupuesto) {
            return Err(NbError::ComponentUnavailable);
        }
        let mut cargado = self.cargado.lock().unwrap_or_else(|p| p.into_inner());
        // Si el cargado no es el elegido —se cambió mientras generaba otro—, se
        // suelta antes de traer el nuevo, no se quedan los dos.
        if cargado.as_ref().is_some_and(|c| c.entrada.id != entrada.id) {
            *cargado = None;
        }
        if cargado.is_none() {
            let grupo = presupuesto::grupo_de_hilos(presupuesto.hilos).map_err(|_| {
                tracing::warn!("no se pudo crear el grupo de hilos de la generación");
                NbError::ComponentUnavailable
            })?;
            *cargado = Some(self.cargar(entrada, grupo)?);
            self.vigilar();
        }
        let Some(modelo) = cargado.as_mut() else {
            return Err(NbError::Internal);
        };
        let resultado = escribir(modelo, request, &self.cancelado);
        modelo.uso = Instant::now();
        resultado
    }
}

/// La conversación en el formato de chat de Qwen.
fn indicacion(request: &Request, apertura: &str) -> String {
    let mensajes = contexto::mensajes(request);
    format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n{apertura}",
        mensajes.sistema, mensajes.usuario
    )
}

/// Cuántas fichas de la entrada se leen entre dos miradas a la señal de cancelar.
const TRAMO_DE_LECTURA: usize = 16;

fn escribir(modelo: &mut Cargado, request: &Request, cancelado: &AtomicBool) -> NbResult<String> {
    let fallo = |que: &'static str| {
        move |_: candle_core::Error| {
            tracing::warn!(paso = que, "la generación local falló");
            NbError::Internal
        }
    };
    let ids = modelo
        .tokenizador
        .encode(indicacion(request, modelo.red.apertura()), false)
        .map_err(|_| NbError::Internal)?
        .get_ids()
        .to_vec();
    let Cargado {
        red,
        tokenizador,
        fin,
        grupo,
        dispositivo,
        ..
    } = modelo;
    red.olvidar_el_contexto();

    grupo.install(|| {
        // Determinista: la misma entrada da la misma salida, que es lo que permite
        // evaluar una plantilla y repetir un fallo.
        let mut muestreo = LogitsProcessor::from_sampling(0, Sampling::ArgMax);
        // La entrada se lee por tramos para poder cancelar en medio: leída de una
        // vez son dos minutos sin mirar la señal. Y el tramo es corto a propósito:
        // con 128 fichas, en un procesador sin AVX2 cada tramo tardaba más de dos
        // minutos, y cancelar no hacía nada en todo ese tiempo. Con 16 se
        // mira la señal en segundos y la lectura apenas se encarece.
        let mut posicion = 0;
        let mut ultimos = None;
        for tramo in ids.chunks(TRAMO_DE_LECTURA) {
            if cancelado.load(Ordering::SeqCst) {
                return Err(NbError::Cancelled);
            }
            let entrada = Tensor::new(tramo, &*dispositivo)
                .and_then(|t| t.unsqueeze(0))
                .map_err(fallo("entrada"))?;
            ultimos = Some(red.adelante(&entrada, posicion).map_err(fallo("lectura"))?);
            posicion += tramo.len();
        }
        let ultimos = ultimos.ok_or(NbError::Internal)?;
        let mut siguiente = muestreo
            .sample(&ultimos.squeeze(0).map_err(fallo("muestreo"))?)
            .map_err(fallo("muestreo"))?;
        let mut salida = Vec::with_capacity(MAXIMO_DE_SALIDA);
        for _ in 0..MAXIMO_DE_SALIDA {
            if fin.contains(&siguiente) {
                break;
            }
            if cancelado.load(Ordering::SeqCst) {
                return Err(NbError::Cancelled);
            }
            salida.push(siguiente);
            let entrada = Tensor::new(&[siguiente], &*dispositivo)
                .and_then(|t| t.unsqueeze(0))
                .map_err(fallo("entrada"))?;
            let logits = red
                .adelante(&entrada, posicion)
                .and_then(|t| t.squeeze(0))
                .map_err(fallo("escritura"))?;
            posicion += 1;
            siguiente = muestreo.sample(&logits).map_err(fallo("muestreo"))?;
        }
        tokenizador
            .decode(&salida, true)
            .map_err(|_| NbError::Internal)
    })
}

/// El proveedor, compartido: se registra en el servicio y el subsistema se queda
/// con él para cambiarle el presupuesto.
pub struct Compartido(pub Arc<GenerativoLocal>);

impl AiProvider for Compartido {
    fn id(&self) -> &str {
        ID
    }

    fn label(&self) -> &str {
        self.0.elegido().nombre
    }

    fn placement(&self) -> Placement {
        Placement::Local
    }

    fn capabilities(&self) -> &[Capability] {
        CAPACIDADES
    }

    fn readiness(&self) -> Availability {
        let proveedor = &self.0;
        // **Ni la compilación sin AVX2 ni un equipo justo de memoria esconden el
        // proveedor**, que es lo que hacían antes. Si el modelo está
        // instalado, alguien lo eligió sabiendo lo que es probable que pase -Ajustes
        // se lo dijo con `catalogo::prevision`-, y quitárselo después sería decidir
        // por él. Lo que sí se respeta es el tope de memoria: ese lo puso la propia
        // persona, y subirlo está a un clic.
        let elegido = proveedor.elegido();
        if super::instalacion::instalado(&proveedor.modelos, elegido) {
            if !super::modelo::cabe(elegido, proveedor.presupuesto()) {
                return Availability::Absent(Absence::OverBudget);
            }
            return Availability::Ready(ProviderInfo {
                id: ID.to_owned(),
                label: elegido.nombre.to_owned(),
                placement: Placement::Local,
                host: None,
            });
        }
        // Lo mismo que el de embeddings: sin modelo, nadie lo ha pedido; con la
        // carpeta de uno roto, sí. Ver `LocalProvider::readiness`.
        if super::instalacion::hay_rastro(&proveedor.modelos, elegido) {
            Availability::Absent(Absence::NotInstalled)
        } else {
            Availability::Absent(Absence::Disabled)
        }
    }

    fn input_chars(&self) -> usize {
        CARACTERES
    }

    fn generate(&self, request: &Request) -> NbResult<Completion> {
        if !CAPACIDADES.contains(&request.capability) || !self.readiness().is_ready() {
            return Err(NbError::ComponentUnavailable);
        }
        let text = self.0.generar_con(request)?;
        Ok(Completion {
            text,
            grounding: Vec::new(),
            provider_id: ID.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::catalogo::QWEN25_15B_INSTRUCT;

    fn proveedor(nombre: &str, presupuesto: Presupuesto) -> Compartido {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-generativo-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        let mut p = GenerativoLocal::nuevo(&dir, presupuesto, Arc::new(AtomicBool::new(false)));
        p.elegido = Mutex::new(&QWEN25_15B_INSTRUCT);
        p.recursos = Recursos {
            memoria_total: Some(16 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: hardware::Acelerador::Cpu,
        };
        p.rapido = true;
        Compartido(Arc::new(p))
    }

    fn disponer(p: &Compartido) {
        let carpeta = super::super::instalacion::carpeta(&p.0.modelos, p.0.elegido());
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        for archivo in p.0.elegido().archivos {
            std::fs::File::create(carpeta.join(archivo.nombre))
                .and_then(|f| f.set_len(archivo.bytes))
                .expect("archivo disperso");
        }
    }

    /// **Sin los núcleos rápidos se ofrece igual, y se dice cuánto va a tardar.**
    ///
    /// Antes esta prueba exigía lo contrario -que no se ofreciera-, y
    /// es la regresión del cambio de criterio de ese día: "una cosa es recomendar y
    /// otra obligar". Si alguien vuelve a esconder el proveedor por la compilación,
    /// esto falla.
    #[test]
    fn sin_los_nucleos_rapidos_se_ofrece_y_la_prevision_lo_dice() {
        let mut p = proveedor("lento", Presupuesto::desde(0.0, 2048.0, 8));
        disponer(&p);
        let rapida = p.0.prevision(&QWEN25_15B_INSTRUCT);
        Arc::get_mut(&mut p.0).expect("único").rapido = false;

        assert!(
            p.readiness().is_ready(),
            "se escondió un modelo instalado por la compilación"
        );
        let lenta = p.0.prevision(&QWEN25_15B_INSTRUCT);
        assert!(lenta.avisos.contains(&"SLOW_BUILD"), "{:?}", lenta.avisos);
        let (Some(con), Some(sin)) = (rapida.minutos, lenta.minutos) else {
            panic!("un modelo que genera tiene que prever minutos");
        };
        assert!(
            (sin / con - 10.0).abs() < 0.01,
            "sin AVX2 se midió diez veces más lento, y se prevé {:.1} veces",
            sin / con
        );
    }

    /// Las preguntas de autoevaluación **se ofrecen con el modelo local**, y van
    /// marcadas: lo medido dice cómo salen, y eso es lo que se enseña al elegirlas.
    #[test]
    fn las_preguntas_se_ofrecen_en_local_y_van_desaconsejadas() {
        let p = proveedor("preguntas", Presupuesto::desde(0.0, 2048.0, 8));
        assert!(p.capabilities().contains(&Capability::Questions));
        assert!(DESACONSEJADAS
            .iter()
            .any(|(c, motivo)| *c == Capability::Questions && !motivo.is_empty()));
    }

    /// El tope de memoria de fábrica (1 GB) no lo admite: lo dice con su motivo, y
    /// no invita a descargar lo que ya está.
    #[test]
    fn con_el_tope_de_fabrica_no_cabe_y_lo_dice() {
        let p = proveedor("tope", Presupuesto::desde(0.0, 1024.0, 8));
        assert_eq!(p.readiness(), Availability::Absent(Absence::Disabled));
        disponer(&p);
        assert_eq!(p.readiness(), Availability::Absent(Absence::OverBudget));
        assert_eq!(
            p.generate(&Request {
                capability: Capability::Summary,
                instruction: String::new(),
                passages: Vec::new(),
                task: String::new(),
                language: None,
            }),
            Err(NbError::ComponentUnavailable)
        );
    }

    #[test]
    fn es_local_y_no_hace_vectores() {
        let p = proveedor("local", Presupuesto::desde(0.0, 2048.0, 8));
        assert_eq!(p.placement(), Placement::Local);
        assert!(p.host().is_none());
        assert!(!p.capabilities().contains(&Capability::Embedding));
        assert_eq!(p.input_chars(), CARACTERES);
    }

    /// La indicación separa la instrucción del documento con los delimitadores de
    /// `contexto`, también en local.
    #[test]
    fn la_indicacion_local_usa_la_misma_separacion_que_la_externa() {
        let r = Request {
            capability: Capability::Summary,
            instruction: "SISTEMA".to_owned(),
            passages: Vec::new(),
            task: "TAREA".to_owned(),
            language: None,
        };
        let i = indicacion(&r, "");
        assert!(i.starts_with("<|im_start|>system\nSISTEMA<|im_end|>"));
        assert!(i.ends_with("<|im_start|>assistant\n"));
        assert!(i.contains("TAREA"));
    }
}
