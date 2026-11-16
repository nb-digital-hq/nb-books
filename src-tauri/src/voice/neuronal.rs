//! La voz neuronal como proveedor: carga, calibración, liberación y tope.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 7.
//!
//! # Por qué esta pieza mide la máquina en lugar de preguntarle
//!
//! El criterio de la subfase pide que en un equipo que no da para narrar, el
//! proveedor diga **no soportado sin que el usuario tenga que hacer nada**. La
//! tentación es deducirlo del número de núcleos, como hace el catálogo de los
//! modelos que generan; con la voz no vale, porque lo que decide no es si tarda
//! mucho sino si **tarda más de lo que dura lo que dice**, y eso depende del
//! procesador, de cómo se compiló y de lo que la máquina esté haciendo a la vez.
//!
//! Así que se mide: al cargar, se sintetiza una frase corta y se calcula el factor
//! de tiempo real. Si sale por encima de [`FACTOR_MAXIMO`], la voz se declara no
//! disponible con su motivo y no se ofrece. Cuesta un segundo de cálculo por carga
//! y ahorra ofrecer una narración que se corta cada dos párrafos, que es la clase
//! de promesa que este proyecto no hace.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use candle_core::Device;
use serde::Serialize;
use ts_rs::TS;

use super::catalogo::{ModeloDeVoz, KOKORO_82M};
use super::fonemas::{self, Variante};
use super::kokoro::{Kokoro, Voz};
use crate::ai::presupuesto::Presupuesto;
use crate::error::{NbError, NbResult};

/// Cuánto tarda en soltarse sin uso: dos minutos.
///
/// Menos que el modelo que genera, porque quien deja de escuchar deja de escuchar;
/// y más que el tiempo entre dos párrafos, para que pasar página no recargue 327 MB.
pub const INACTIVIDAD: Duration = Duration::from_secs(120);

/// Factor de tiempo real por encima del cual no se ofrece narrar.
///
/// **0,8 y no 1,0.** Uno es el punto en el que sintetizar tarda exactamente lo que
/// dura el audio, así que ahí no hay margen: cualquier otra cosa que haga la máquina
/// —desplazar la página, indexar, abrir un libro— vacía el adelanto y la voz se
/// corta. Con 0,8 queda una quinta parte de holgura, que es lo que la síntesis
/// adelantada necesita para no llegar tarde.
pub const FACTOR_MAXIMO: f32 = 0.8;

/// La frase con la que se mide la máquina.
///
/// Corta —unos dos segundos de audio— y con la fonética que cuesta: grupos
/// consonánticos, una vibrante múltiple y una jota. Una frase de vocales sueltas
/// mediría un caso que no existe.
const CALIBRACION: &str = "La imprenta llegó a Sevilla y el trabajo se reparte.";

/// Por qué se puede o no se puede narrar con la voz neuronal.
#[derive(Debug, Clone, PartialEq)]
pub enum Disponibilidad {
    /// Lista: instalada, cabe y la máquina llega.
    Lista,
    /// No está instalada. La interfaz puede ofrecer instalarla.
    NoInstalada,
    /// Está instalada y no cabe en el tope de memoria de la IA.
    NoCabe { necesita: u64, tope: u64 },
    /// La máquina no la mueve en tiempo real. **Medido aquí**, no supuesto.
    DemasiadoLenta { factor: f32 },
    /// El modelo está pero no se deja cargar.
    NoSeCarga,
}

impl Disponibilidad {
    /// Código estable del motivo, para que la interfaz lo traduzca.
    #[must_use]
    pub const fn motivo(&self) -> Option<&'static str> {
        match self {
            Self::Lista => None,
            Self::NoInstalada => Some("NO_INSTALADA"),
            Self::NoCabe { .. } => Some("NO_CABE"),
            Self::DemasiadoLenta { .. } => Some("DEMASIADO_LENTA"),
            Self::NoSeCarga => Some("NO_SE_CARGA"),
        }
    }
}

/// Lo que Ajustes y el lector necesitan saber de la voz neuronal.
///
/// Se declara entero **aunque no esté instalada**: quien decide si la descarga
/// tiene que ver antes de qué tamaño es, de quién es y bajo qué licencia, que es
/// la misma regla que la sección de modelos de IA.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "NeuralVoice")]
pub struct VistaDeLaVoz {
    /// Identificador del modelo en el catálogo. Es el que se pasa para instalarlo.
    pub id: &'static str,
    pub name: &'static str,
    pub author: &'static str,
    pub licence: &'static str,
    /// De dónde se descarga. Se enseña **antes** de ofrecer descargar, como el
    /// resto de los modelos: autorizar una salida sin saber a dónde no es
    /// autorizar nada.
    pub host: &'static str,
    /// Lo que ocupa la descarga, en bytes.
    #[ts(type = "number")]
    pub download_bytes: u64,
    /// Lo que ocupa cargado, medido.
    #[ts(type = "number")]
    pub memory_bytes: u64,
    pub installed: bool,
    /// Si se puede narrar con ella **ahora mismo**.
    pub ready: bool,
    /// Por qué no, cuando no: `NO_INSTALADA`, `NO_CABE`, `DEMASIADO_LENTA` o
    /// `NO_SE_CARGA`. Lo traduce la interfaz.
    pub reason: Option<&'static str>,
    /// El factor de tiempo real **medido en esta máquina**, si ya se midió.
    ///
    /// Es lo que separa "no se ofrece porque sí" de "no se ofrece porque tarda
    /// 1,7 veces lo que dura". La interfaz lo enseña.
    pub factor: Option<f32>,
    /// Cota: las voces del catálogo, compiladas en el binario.
    pub voices: Vec<super::SystemVoice>,
}

/// Lo que se devuelve al sintetizar: muestras y a qué ritmo suenan.
pub struct Sonido {
    pub muestras: Vec<f32>,
    pub muestreo: u32,
}

impl Sonido {
    /// Lo que dura, en segundos.
    #[must_use]
    pub fn duracion(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let muestras = self.muestras.len() as f64;
        muestras / f64::from(self.muestreo.max(1))
    }
}

struct Cargado {
    motor: Kokoro,
    /// Las voces se cargan a demanda y se quedan: pesan 500 KB y volver a leerlas
    /// en cada párrafo sería leer el disco para nada.
    voces: Vec<(String, Voz)>,
    /// Lo que la máquina dio al calibrar. Ver el encabezado del módulo.
    factor: f32,
    uso: Instant,
}

/// Lo que necesita de la voz quien la usa, **sin depender del modelo**.
///
/// Existe para que la preparación de un libro se pueda probar: el modelo
/// pesa 327 MB, no entra en el repositorio y tarda segundos en cargar, así que una
/// prueba que lo necesitara no se ejecutaría en ninguna parte y la lógica de
/// preparar —la cola, las marcas, el tiempo restante, lo que se guarda— se
/// quedaría sin comprobar. Es el mismo motivo por el que el índice vectorial tiene
/// contrato: ver `ai::indice`.
pub trait Sintetizador: Send + Sync {
    /// Si el modelo está en disco.
    fn instalada(&self) -> bool;
    /// La revisión con la que se hace lo que se guarde.
    fn revision(&self) -> &'static str;
    /// El identificador de la voz que se usa cuando nadie elige.
    fn voz_por_defecto(&self) -> String;
    /// El factor de tiempo real medido aquí, si ya se midió.
    fn factor_medido(&self) -> Option<f32>;
    /// Dice un texto.
    ///
    /// # Errors
    ///
    /// Los de la síntesis.
    fn sintetizar(&self, texto: &str, voz: Option<&str>, velocidad: f32) -> NbResult<Sonido>;
    /// Dice un texto y lo deja en un archivo, devolviendo su ruta.
    ///
    /// # Errors
    ///
    /// Los de la síntesis, y los de escribir.
    fn sintetizar_a_archivo(
        &self,
        carpeta: &Path,
        texto: &str,
        voz: Option<&str>,
        velocidad: f32,
    ) -> NbResult<PathBuf>;
}

impl Sintetizador for VozNeuronal {
    fn instalada(&self) -> bool {
        Self::instalada(self)
    }
    fn revision(&self) -> &'static str {
        self.modelo.revision
    }
    fn voz_por_defecto(&self) -> String {
        self.modelo
            .voces
            .first()
            .map_or_else(String::new, |voz| voz.id.to_owned())
    }
    fn factor_medido(&self) -> Option<f32> {
        Self::factor_medido(self)
    }
    fn sintetizar(&self, texto: &str, voz: Option<&str>, velocidad: f32) -> NbResult<Sonido> {
        Self::sintetizar(self, texto, voz, velocidad)
    }
    fn sintetizar_a_archivo(
        &self,
        carpeta: &Path,
        texto: &str,
        voz: Option<&str>,
        velocidad: f32,
    ) -> NbResult<PathBuf> {
        Self::sintetizar_a_archivo(self, carpeta, texto, voz, velocidad)
    }
}

/// La voz neuronal local.
pub struct VozNeuronal {
    modelo: &'static ModeloDeVoz,
    modelos: PathBuf,
    presupuesto: Mutex<Presupuesto>,
    /// El cerrojo es también el turno: quien sintetiza lo tiene.
    cargado: Mutex<Option<Cargado>>,
    /// El último factor medido, **aunque el modelo se haya soltado**. El factor es
    /// de la máquina y no de la carga: olvidarlo a los dos minutos de inactividad
    /// quitaba el aviso de lento y la estimación de lo que tardará preparar.
    ultimo_factor: Mutex<Option<f32>>,
}

impl VozNeuronal {
    /// Prepara el proveedor. **No carga nada**: arrancar no es querer hablar.
    #[must_use]
    pub fn nueva(modelos: &Path, presupuesto: Presupuesto) -> Arc<Self> {
        let proveedor = Arc::new(Self {
            modelo: &KOKORO_82M,
            modelos: modelos.to_path_buf(),
            presupuesto: Mutex::new(presupuesto),
            cargado: Mutex::new(None),
            ultimo_factor: Mutex::new(None),
        });
        proveedor.vigilar();
        proveedor
    }

    /// El modelo que usa, para la interfaz y para instalarlo.
    #[must_use]
    pub const fn modelo(&self) -> &'static ModeloDeVoz {
        self.modelo
    }

    /// Si los archivos están en disco con el tamaño revisado.
    #[must_use]
    pub fn instalada(&self) -> bool {
        crate::ai::instalacion::instalado(&self.modelos, self.modelo)
    }

    /// Qué se puede hacer hoy con ella, **sin cargarla si ya se sabe que no**.
    ///
    /// Cargar para saber si cabe sería justo lo contrario de un tope de memoria.
    #[must_use]
    pub fn disponibilidad(&self) -> Disponibilidad {
        if !self.instalada() {
            return Disponibilidad::NoInstalada;
        }
        let tope = self.presupuesto().memoria;
        if self.modelo.memoria > tope {
            return Disponibilidad::NoCabe {
                necesita: self.modelo.memoria,
                tope,
            };
        }
        let mut cargado = self.cargado.lock().unwrap_or_else(|p| p.into_inner());
        match self.asegurar(&mut cargado) {
            Err(_) => Disponibilidad::NoSeCarga,
            Ok(actual) if actual.factor > FACTOR_MAXIMO => Disponibilidad::DemasiadoLenta {
                factor: actual.factor,
            },
            Ok(_) => Disponibilidad::Lista,
        }
    }

    /// Las voces que ofrece para un idioma, con el identificador con el que se piden.
    #[must_use]
    pub fn voces(&self, idioma: &str) -> Vec<super::SystemVoice> {
        self.modelo
            .voces_de(idioma)
            .into_iter()
            .map(|voz| super::SystemVoice {
                id: voz.id.to_owned(),
                name: voz.nombre.to_owned(),
                language: voz.idioma.to_owned(),
            })
            .collect()
    }

    /// Dice un texto. Devuelve las muestras; **no suena aquí**.
    ///
    /// Reproducir es de quien tenga salida de audio, y en esta aplicación la tiene
    /// la ventana. Separarlo permite además adelantar la síntesis del tramo
    /// siguiente mientras suena el actual, que es lo.
    ///
    /// # Errors
    ///
    /// Un texto sin nada que decir devuelve **silencio y no error**: quien narra un
    /// capítulo se encuentra fragmentos vacíos —una imagen, una línea de separación—
    /// y tratarlos como fallo cortaría la lectura en cada uno.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si el modelo no está instalado o si se
    ///   pide una voz que no existe.
    /// - [`NbError::Internal`] si no se puede cargar o la síntesis falla.
    pub fn sintetizar(&self, texto: &str, voz: Option<&str>, velocidad: f32) -> NbResult<Sonido> {
        if texto.trim().is_empty() {
            return Ok(Sonido {
                muestras: Vec::new(),
                muestreo: super::kokoro::MUESTREO,
            });
        }
        let mut cargado = self.cargado.lock().unwrap_or_else(|p| p.into_inner());
        let actual = self.asegurar(&mut cargado)?;
        let pedida = self.voz_que_se_usa(voz);
        // **La voz pedida se carga al pedirla.** Antes solo se cargaba
        // la primera al cargar el modelo, y elegir cualquier otra hacía fallar cada
        // síntesis con "no disponible": en vivo y al preparar.
        let carpeta = crate::ai::instalacion::carpeta(&self.modelos, self.modelo);
        Self::voz_de(actual, &carpeta, pedida)?;
        let sonido = Self::decir(actual, texto, pedida, velocidad)?;
        actual.uso = Instant::now();
        Ok(sonido)
    }

    /// La voz del catálogo que corresponde a lo pedido.
    ///
    /// Una que no sea de este modelo -el ajuste `voice_name` es el mismo para la voz
    /// del sistema, y puede traer el nombre de una de speech-dispatcher- **cae en la
    /// de fábrica** en lugar de fallar: quien pulsa "preparar" con una voz de otro
    /// proveedor elegida quiere oír el libro, no un error sobre un ajuste que no ve.
    fn voz_que_se_usa<'a>(&'a self, pedida: Option<&'a str>) -> &'a str {
        let de_fabrica = self.modelo.voces.first().map_or("", |v| v.id);
        match pedida {
            Some(id) if self.modelo.voces.iter().any(|v| v.id == id) => id,
            Some(otra) => {
                tracing::debug!(
                    pedida = otra,
                    "voz que no es de este modelo: se usa la de fábrica"
                );
                de_fabrica
            }
            None => de_fabrica,
        }
    }

    /// Sintetiza un tramo y **deja el audio en un archivo**, devolviendo su ruta.
    ///
    /// # Por qué un archivo y no las muestras
    ///
    /// Por lo mismo que la voz de un servicio: son cientos de kilobytes
    /// por párrafo, y cruzarlos por el puente obligaría a codificarlos en base64
    /// sobre el mismo canal por el que va todo lo demás. La ventana ya sabe leer
    /// de la caché por el protocolo de assets, y con un archivo detrás **pausar es
    /// pausar**, en lugar de cortar y volver a empezar el tramo.
    ///
    /// La caché se indexa por el contenido de la petición —voz, velocidad y
    /// texto—, así que volver a un párrafo no vuelve a sintetizarlo. Dos textos
    /// iguales con voces distintas suenan distinto y no se sirven el uno por el
    /// otro.
    ///
    /// **Se guarda comprimido**, como lo que se prepara: un párrafo en crudo son
    /// medio megabyte y comprimido son treinta kilobytes, y la ventana reproduce
    /// Opus igual de bien.
    ///
    /// # Errors
    ///
    /// Los de [`Self::sintetizar`], y [`NbError::StorageFailure`] si no se puede
    /// escribir el audio.
    pub fn sintetizar_a_archivo(
        &self,
        carpeta: &Path,
        texto: &str,
        voz: Option<&str>,
        velocidad: f32,
    ) -> NbResult<PathBuf> {
        let pedida = voz.unwrap_or_else(|| self.modelo.voces.first().map_or("", |v| v.id));
        let huella = crate::library::hash_reader(
            format!(
                "{}|{}|{pedida}|{velocidad}|{}",
                self.modelo.id,
                self.modelo.revision,
                texto.trim()
            )
            .as_bytes(),
        )?;
        let archivo = carpeta.join(format!("{}.opus", huella.0.as_str().replace(':', "-")));
        if archivo.is_file() {
            return Ok(archivo);
        }

        let sonido = self.sintetizar(texto, voz, velocidad)?;
        super::audio::escribir_opus(&archivo, &sonido.muestras, sonido.muestreo)?;
        Ok(archivo)
    }

    /// Lo que se sabe **sin cargar nada**: instalada, si cabe, y el factor si ya se
    /// midió en esta sesión. Ver `ipc::neural_voice`.
    #[must_use]
    pub fn disponibilidad_sin_cargar(&self) -> Disponibilidad {
        if !self.instalada() {
            return Disponibilidad::NoInstalada;
        }
        let tope = self.presupuesto().memoria;
        if self.modelo.memoria > tope {
            return Disponibilidad::NoCabe {
                necesita: self.modelo.memoria,
                tope,
            };
        }
        match self.factor_medido() {
            Some(factor) if factor > FACTOR_MAXIMO => Disponibilidad::DemasiadoLenta { factor },
            _ => Disponibilidad::Lista,
        }
    }

    /// La vista sin cargar el modelo. Ver [`Self::disponibilidad_sin_cargar`].
    #[must_use]
    pub fn vista_sin_cargar(&self) -> VistaDeLaVoz {
        self.vista_con(self.disponibilidad_sin_cargar())
    }

    /// La vista para la interfaz, **midiendo** si hace falta. No carga el modelo si
    /// no está instalado.
    #[must_use]
    pub fn vista(&self) -> VistaDeLaVoz {
        self.vista_con(self.disponibilidad())
    }

    fn vista_con(&self, disponibilidad: Disponibilidad) -> VistaDeLaVoz {
        let factor = self
            .cargado
            .try_lock()
            .ok()
            .and_then(|cargado| cargado.as_ref().map(|c| c.factor));
        VistaDeLaVoz {
            id: self.modelo.id,
            name: self.modelo.nombre,
            author: self.modelo.autor,
            licence: self.modelo.licencia,
            host: crate::ai::catalogo::ANFITRION,
            download_bytes: crate::ai::instalacion::Instalable::disco(self.modelo),
            memory_bytes: self.modelo.memoria,
            installed: self.instalada(),
            ready: disponibilidad == Disponibilidad::Lista,
            reason: disponibilidad.motivo(),
            factor: match disponibilidad {
                Disponibilidad::DemasiadoLenta { factor } => Some(factor),
                _ => factor,
            },
            voices: self
                .modelo
                .voces
                .iter()
                .map(|voz| super::SystemVoice {
                    id: voz.id.to_owned(),
                    name: voz.nombre.to_owned(),
                    language: voz.idioma.to_owned(),
                })
                .collect(),
        }
    }

    /// El factor de tiempo real medido en esta máquina, si ya se midió.
    ///
    /// Lo usa la estimación de lo que cuesta preparar un libro: sin él se dice
    /// cuánto durará el audio y **no** cuánto tardará, que es mejor que inventar
    /// un número sobre una espera de una hora.
    #[must_use]
    pub fn factor_medido(&self) -> Option<f32> {
        *self.ultimo_factor.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Cambia el presupuesto; si ya no cabe, se suelta ahora.
    ///
    /// Un tope que solo se aplica a la próxima carga no es un tope: es una
    /// intención. Es la misma regla que el modelo de embeddings.
    pub fn aplicar(&self, nuevo: Presupuesto) {
        *self.presupuesto.lock().unwrap_or_else(|p| p.into_inner()) = nuevo;
        if self.modelo.memoria > nuevo.memoria {
            if let Ok(mut cargado) = self.cargado.try_lock() {
                *cargado = None;
            }
        }
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
            crate::ai::modelo::devolver_memoria_al_sistema();
            tracing::info!("voz neuronal liberada por inactividad");
        }
        inactivo
    }

    fn presupuesto(&self) -> Presupuesto {
        *self.presupuesto.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Carga el modelo si hace falta y devuelve lo cargado.
    fn asegurar<'a>(&self, cargado: &'a mut Option<Cargado>) -> NbResult<&'a mut Cargado> {
        if cargado.is_none() {
            *cargado = Some(self.cargar()?);
        }
        cargado.as_mut().ok_or(NbError::Internal)
    }

    fn cargar(&self) -> NbResult<Cargado> {
        if !self.instalada() {
            return Err(NbError::ComponentUnavailable);
        }
        let carpeta = crate::ai::instalacion::carpeta(&self.modelos, self.modelo);
        let inicio = Instant::now();
        let motor = Kokoro::cargar(&carpeta, &Device::Cpu)?;
        let mut cargado = Cargado {
            motor,
            voces: Vec::new(),
            factor: 0.0,
            uso: Instant::now(),
        };
        // La primera voz se carga aquí porque la calibración la necesita, y porque
        // una carga que deja el modelo sin poder decir nada no es una carga.
        let primera = self.modelo.voces.first().ok_or(NbError::Internal)?.id;
        Self::voz_de(&mut cargado, &carpeta, primera)?;
        cargado.factor = Self::calibrar(&cargado, primera)?;
        *self.ultimo_factor.lock().unwrap_or_else(|p| p.into_inner()) = Some(cargado.factor);
        tracing::info!(
            ms = inicio.elapsed().as_millis(),
            factor = cargado.factor,
            "voz neuronal cargada y medida"
        );
        Ok(cargado)
    }

    /// Mide qué factor de tiempo real da esta máquina con este modelo.
    fn calibrar(cargado: &Cargado, voz: &str) -> NbResult<f32> {
        let reloj = Instant::now();
        let sonido = Self::decir(cargado, CALIBRACION, voz, 1.0)?;
        let duracion = sonido.duracion();
        if duracion <= 0.0 {
            return Err(NbError::Internal);
        }
        #[allow(clippy::cast_possible_truncation)]
        let factor = (reloj.elapsed().as_secs_f64() / duracion) as f32;
        Ok(factor)
    }

    fn decir(cargado: &Cargado, texto: &str, voz: &str, velocidad: f32) -> NbResult<Sonido> {
        let estilo = cargado
            .voces
            .iter()
            .find(|(id, _)| id == voz)
            .map(|(_, estilo)| estilo)
            .ok_or(NbError::ComponentUnavailable)?;
        // **La variante sale de la voz elegida**, no de un ajuste aparte: una voz
        // portuguesa leyendo con reglas españolas sería una preferencia que se
        // puede dejar mal puesta, y el idioma de la voz ya lo dice su nombre.
        // **Un pánico de aquí dentro no sale de aquí.** La conversión a fonemas y el
        // modelo son código que recibe cualquier texto de cualquier libro, y el
        // 2026-09-23 "III" hacía entrar en pánico a la primera: el hilo que preparaba
        // el capítulo moría sin avisar, el trabajo se quedaba "en curso" para siempre
        // y la ventana decía "algo falló por dentro" sin que el registro dijera nada.
        // Es la regla 1 de la Constitución: el fallo de un componente no tumba a otro.
        let decirlo = || -> NbResult<Vec<f32>> {
            let fonemas = fonemas::de_texto(texto, Variante::de_voz(voz));
            // **En trozos que el modelo admite.** Kokoro lee como mucho 512 fichas de
            // una vez (`max_position_embeddings`), y un fragmento de los que se
            // preparan mide hasta 1.920 caracteres. Antes se le pasaba
            // entero, y el primer párrafo largo de un capítulo real hacía fallar la
            // síntesis y con ella la preparación. Medido con el modelo real:
            // `voz_neuronal.rs`.
            let mut muestras = Vec::new();
            for trozo in trozos(&fonemas, MAXIMO_DE_FICHAS) {
                muestras.extend(cargado.motor.sintetizar(trozo, estilo, velocidad)?);
            }
            Ok(muestras)
        };
        let muestras = std::panic::catch_unwind(std::panic::AssertUnwindSafe(decirlo))
            .unwrap_or_else(|_| {
                tracing::error!("la voz neuronal entró en pánico al decir un texto");
                Err(NbError::Internal)
            })?;
        Ok(Sonido {
            muestras,
            muestreo: super::kokoro::MUESTREO,
        })
    }

    /// Carga una voz y la deja en la lista, si no estaba.
    fn voz_de(cargado: &mut Cargado, carpeta: &Path, id: &str) -> NbResult<()> {
        if cargado.voces.iter().any(|(nombre, _)| nombre == id) {
            return Ok(());
        }
        let voz = cargado.motor.voz(carpeta, id)?;
        cargado.voces.push((id.to_owned(), voz));
        Ok(())
    }

    /// El hilo que suelta el modelo cuando nadie lo usa.
    ///
    /// Con una referencia débil: si el proveedor muere, el hilo termina en lugar de
    /// mantenerlo vivo, que es como un vigilante acaba sosteniendo lo que vigila.
    fn vigilar(self: &Arc<Self>) {
        let debil = Arc::downgrade(self);
        let _ = std::thread::Builder::new()
            .name("nb-voz-neuronal-inactividad".to_owned())
            .spawn(move || loop {
                std::thread::sleep(INACTIVIDAD / 4);
                let Some(proveedor) = debil.upgrade() else {
                    return;
                };
                let vacio = proveedor
                    .cargado
                    .try_lock()
                    .is_ok_and(|cargado| cargado.is_none());
                if proveedor.liberar_si_inactivo() || vacio {
                    return;
                }
            });
    }
}

/// Fichas por trozo, con margen sobre las 512 que admite el modelo: cada carácter de
/// la cadena de fonemas es una ficha, y el modelo añade las dos de los extremos. El
/// margen es por la fila de estilo, que el modelo elige por la longitud y que cerca
/// del final de la tabla suena peor.
const MAXIMO_DE_FICHAS: usize = 400;

/// Parte una cadena de fonemas en trozos de como mucho `maximo` caracteres.
///
/// **Por donde se pararía quien lee en voz alta**: primero detrás de un punto, de un
/// signo de interrogación o de exclamación; si un trozo sigue siendo largo, detrás de
/// una coma, un punto y coma o dos puntos; y solo si no queda otra, en un espacio.
/// Cortar a mitad de palabra cambiaría lo que se dice; cortar a mitad de frase, cómo
/// suena.
fn trozos(fonemas: &str, maximo: usize) -> Vec<&str> {
    fn partir<'a>(texto: &'a str, maximo: usize, cortes: &[&[char]], salida: &mut Vec<&'a str>) {
        let texto = texto.trim();
        if texto.is_empty() {
            return;
        }
        if texto.chars().count() <= maximo {
            salida.push(texto);
            return;
        }
        let Some((signos, resto)) = cortes.split_first() else {
            // Sin signos que valgan: en espacios, o a la fuerza si ni eso.
            let mut inicio = 0;
            let mut ultimo_espacio = None;
            for (i, c) in texto.char_indices() {
                if c == ' ' {
                    ultimo_espacio = Some(i);
                }
                if texto[inicio..i].chars().count() >= maximo {
                    let corte = ultimo_espacio.filter(|e| *e > inicio).unwrap_or(i);
                    salida.push(texto[inicio..corte].trim());
                    inicio = corte;
                    ultimo_espacio = None;
                }
            }
            let cola = texto[inicio..].trim();
            if !cola.is_empty() {
                salida.push(cola);
            }
            return;
        };
        // Junta piezas hasta el máximo, cortando solo detrás de uno de estos signos.
        let mut desde = 0;
        let mut hasta = 0;
        for (i, c) in texto.char_indices() {
            if signos.contains(&c) {
                let fin = i + c.len_utf8();
                if texto[desde..fin].chars().count() > maximo && hasta > desde {
                    partir(&texto[desde..hasta], maximo, resto, salida);
                    desde = hasta;
                }
                hasta = fin;
            }
        }
        if texto[desde..].chars().count() > maximo && hasta > desde {
            partir(&texto[desde..hasta], maximo, resto, salida);
            desde = hasta;
        }
        partir(&texto[desde..], maximo, resto, salida);
    }

    let mut salida = Vec::new();
    partir(
        fonemas,
        maximo,
        &[&['.', '!', '?', '…'], &[',', ';', ':', '—']],
        &mut salida,
    );
    salida.retain(|t| !t.is_empty());
    salida
}

#[cfg(test)]
mod pruebas {
    /// Ningún trozo pasa del máximo, nada se pierde y se corta por las pausas.
    #[test]
    fn un_texto_largo_se_parte_en_trozos_que_el_modelo_admite_sin_perder_nada() {
        let frase = "la ˈkasa ˈesta ˈθeɾka del ˈrio, ʝ ˈnaðje la ˈβe. ";
        let largo: String = frase.repeat(40);
        let partes = super::trozos(&largo, 120);
        assert!(
            partes.len() > 1,
            "no se partió un texto de {} caracteres",
            largo.len()
        );
        for parte in &partes {
            assert!(
                parte.chars().count() <= 120,
                "un trozo pasa del máximo: {parte}"
            );
            assert!(parte.ends_with('.'), "se cortó a mitad de frase: «{parte}»");
        }
        let junto: String = partes.join(" ");
        let sin_espacios = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert_eq!(
            sin_espacios(&junto),
            sin_espacios(&largo),
            "se perdió texto al partir"
        );
    }

    /// Una frase más larga que el máximo, sin puntos, se parte por las comas; y una
    /// sin signos, por los espacios, nunca a mitad de palabra.
    #[test]
    fn sin_puntos_se_parte_por_las_comas_y_sin_comas_por_los_espacios() {
        let con_comas = "ˈuno, ˈdos, ˈtɾes, ˈkwatɾo, ˈθiŋko, ˈseis, ˈsjete, ˈotʃo";
        for parte in super::trozos(con_comas, 20) {
            assert!(parte.chars().count() <= 20, "{parte}");
        }
        let sin_signos = "ˈpalabɾa ".repeat(30);
        for parte in super::trozos(&sin_signos, 25) {
            assert!(parte.chars().count() <= 25, "{parte}");
            assert!(
                parte.split(' ').all(|p| p == "ˈpalabɾa"),
                "se cortó a mitad de palabra: «{parte}»"
            );
        }
    }

    /// Lo corto se queda como está: trocear no puede cambiar lo que ya cabía.
    #[test]
    fn lo_que_cabe_no_se_parte() {
        assert_eq!(super::trozos("la ˈkasa.", 400), vec!["la ˈkasa."]);
        assert!(super::trozos("   ", 400).is_empty());
    }

    use super::*;

    fn presupuesto(memoria: u64) -> Presupuesto {
        Presupuesto { hilos: 2, memoria }
    }

    /// Sin el modelo instalado se dice **no instalada**, que es lo que la interfaz
    /// puede resolver ofreciendo instalarlo; no "falla", que no se puede resolver.
    #[test]
    fn sin_el_modelo_en_disco_se_distingue_de_un_fallo() {
        let vacia = crate::paths::temporal_de_pruebas()
            .join(format!("nb-voz-vacia-{}", std::process::id()));
        let proveedor = VozNeuronal::nueva(&vacia, presupuesto(8 * 1024 * 1024 * 1024));
        assert_eq!(proveedor.disponibilidad(), Disponibilidad::NoInstalada);
        assert_eq!(
            proveedor.disponibilidad().motivo(),
            Some("NO_INSTALADA"),
            "el motivo viaja como código, no como frase"
        );
    }

    /// Un fragmento sin nada que decir devuelve silencio **sin cargar el modelo**:
    /// ni falla ni trae 327 MB para no decir nada.
    #[test]
    fn un_fragmento_vacio_devuelve_silencio_sin_cargar_nada() {
        let vacia = crate::paths::temporal_de_pruebas()
            .join(format!("nb-voz-vacio-{}", std::process::id()));
        let proveedor = VozNeuronal::nueva(&vacia, presupuesto(8 * 1024 * 1024 * 1024));
        let sonido = proveedor.sintetizar("   ", None, 1.0).expect("silencio");
        assert!(sonido.muestras.is_empty());
        assert_eq!(sonido.duracion(), 0.0);
    }

    /// El tope de memoria se comprueba **antes** de cargar. Cargar para descubrir
    /// que no cabe es no tener tope.
    #[test]
    fn un_tope_por_debajo_de_lo_que_ocupa_no_llega_a_cargarlo() {
        let carpeta =
            crate::paths::temporal_de_pruebas().join(format!("nb-voz-tope-{}", std::process::id()));
        std::fs::create_dir_all(carpeta.join(KOKORO_82M.id)).expect("carpeta");
        for archivo in KOKORO_82M.archivos {
            std::fs::write(
                carpeta.join(KOKORO_82M.id).join(archivo.nombre),
                vec![0_u8; usize::try_from(archivo.bytes).unwrap_or(0)],
            )
            .expect("archivo");
        }
        let proveedor = VozNeuronal::nueva(&carpeta, presupuesto(16 * 1024 * 1024));
        assert!(matches!(
            proveedor.disponibilidad(),
            Disponibilidad::NoCabe { .. }
        ));
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    /// **Preguntar al arrancar no carga el modelo.** Con archivos del tamaño correcto
    /// y contenido que no es un modelo, cargarlo falla; así que si la consulta sin
    /// cargar dijera "no se carga", es que intentó cargar. Y la que mide, sí lo
    /// intenta.
    ///
    /// Antes solo existía la que mide, y el narrador la pedía en cada
    /// arranque: 600 MB y una síntesis de calibración sin que nadie fuera a escuchar.
    #[test]
    fn preguntar_sin_medir_no_carga_el_modelo() {
        let carpeta = crate::paths::temporal_de_pruebas()
            .join(format!("nb-voz-sin-medir-{}", std::process::id()));
        std::fs::create_dir_all(carpeta.join(KOKORO_82M.id)).expect("carpeta");
        for archivo in KOKORO_82M.archivos {
            let destino = std::fs::File::create(carpeta.join(KOKORO_82M.id).join(archivo.nombre))
                .expect("archivo");
            destino.set_len(archivo.bytes).expect("tamaño");
        }
        let proveedor = VozNeuronal::nueva(&carpeta, presupuesto(8 * 1024 * 1024 * 1024));

        assert_eq!(proveedor.disponibilidad_sin_cargar(), Disponibilidad::Lista);
        assert!(
            proveedor.factor_medido().is_none(),
            "midió sin que se lo pidieran"
        );
        assert_eq!(
            proveedor.disponibilidad(),
            Disponibilidad::NoSeCarga,
            "la que mide tiene que intentar cargarlo"
        );
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    /// Las voces se ofrecen por idioma, y con el identificador con el que se piden.
    #[test]
    fn las_voces_se_ofrecen_por_idioma() {
        let vacia = crate::paths::temporal_de_pruebas()
            .join(format!("nb-voz-idioma-{}", std::process::id()));
        let proveedor = VozNeuronal::nueva(&vacia, presupuesto(8 * 1024 * 1024 * 1024));
        let espanolas = proveedor.voces("es-CO");
        assert_eq!(espanolas.len(), 3);
        assert!(espanolas.iter().all(|v| v.language == "es"));
        assert!(proveedor.voces("en").is_empty());
    }
}
