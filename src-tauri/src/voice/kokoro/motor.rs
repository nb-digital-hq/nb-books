//! El motor: de fonemas a sonido.
//!
//! **Procedencia.** La secuencia de pasos y la forma de cargar los pesos están
//! adaptadas de `any-tts` 0.1.3 (`src/models/kokoro/model.rs`), de TM9657, bajo MIT o
//! Apache-2.0. Ver `PROCEDENCIA.md` en esta carpeta.
//!
//! # Los pasos, en orden
//!
//! 1. Los fonemas se convierten en fichas con el vocabulario del modelo.
//! 2. ALBERT los codifica.
//! 3. La prosodia predice **cuánto dura cada fonema**, su tono y su ruido. Aquí entra
//!    la velocidad: dividir las duraciones es hablar más deprisa.
//! 4. Con esas duraciones se estira la secuencia hasta la longitud del audio.
//! 5. El generador de onda convierte todo eso en muestras a 24 kHz.
//!
//! # Por qué los pesos son un `.pth` y eso no contradice la regla del catálogo
//!
//! El catálogo solo admite formatos que no ejecutan código, y un *pickle* de Python
//! puede ejecutarlo al abrirse. **Este no se abre con Python**: lo lee el lector
//! restringido de Candle, que interpreta la estructura y los tensores y no ejecuta
//! nada. Es la única forma que publica el autor del modelo, y la alternativa sería
//! convertirlo nosotros y servirlo desde un sitio que no existe, perdiendo la
//! garantía de procedencia que da descargarlo del repositorio del autor con su huella
//! comprobada.

use std::collections::HashMap;
use std::path::Path;

use candle_core::{DType, Device, IndexOp, Module, Tensor};
use candle_nn::VarBuilder;

use super::albert::Albert;
use super::config::KokoroConfig;
use super::decoder::IstftDecoder;
use super::prosody::ProsodyPredictor;
use super::text_encoder::TextEncoder;
use crate::error::{NbError, NbResult};

/// Muestras por segundo del audio que produce.
pub const MUESTREO: u32 = 24_000;

/// Dimensión del estilo de cada mitad —generador y prosodia— del vector de voz.
const ESTILO: usize = 128;

/// Cuánto se redondea al pasar de duración predicha a número de tramos.
const REDONDEO: f32 = 2.5e-4;

/// Una voz del modelo: su nombre y su tensor de estilo.
pub struct Voz {
    pub nombre: String,
    estilo: Tensor,
}

/// El modelo cargado.
pub struct Kokoro {
    config: KokoroConfig,
    bert: Albert,
    bert_encoder: candle_nn::Linear,
    prosodia: ProsodyPredictor,
    texto: TextEncoder,
    generador: IstftDecoder,
    dispositivo: Device,
    tipo: DType,
}

fn fallo(que: &'static str) -> impl Fn(candle_core::Error) -> NbError {
    move |error| {
        // El texto del error puede llevar rutas: se registra el paso, no el mensaje.
        let _ = error;
        tracing::warn!(paso = que, "la voz neuronal falló");
        NbError::Internal
    }
}

impl Kokoro {
    /// Carga el modelo de una carpeta con `model.pth` y `config.json`.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentCorrupted`] si un archivo no se deja leer como lo que
    ///   dice ser.
    /// - [`NbError::Internal`] si el motor no puede construir la red.
    pub fn cargar(carpeta: &Path, dispositivo: &Device) -> NbResult<Self> {
        let corrupto = |archivo: &'static str| {
            tracing::warn!(
                // no es del usuario: es un nombre fijo de los archivos de la voz.
                archivo,
                "un archivo de la voz no se deja cargar"
            );
            NbError::DocumentCorrupted
        };

        let config: KokoroConfig = std::fs::read(carpeta.join("config.json"))
            .map_err(|_| corrupto("config.json"))
            .and_then(|bytes| {
                serde_json::from_slice(&bytes).map_err(|_| corrupto("config.json"))
            })?;

        // Los pesos vienen en un diccionario anidado y envueltos por `DataParallel`,
        // que añade un `.module.` que aquí sobra.
        let pesos = carpeta.join("model.pth");
        let mut tensores: HashMap<String, Tensor> = HashMap::new();
        for llave in [
            "bert",
            "bert_encoder",
            "decoder",
            "predictor",
            "text_encoder",
        ] {
            let leidos = candle_core::pickle::read_all_with_key(&pesos, Some(llave))
                .map_err(|_| corrupto("model.pth"))?;
            for (nombre, tensor) in leidos {
                tensores.insert(format!("{llave}.{nombre}").replace(".module.", "."), tensor);
            }
        }
        if tensores.is_empty() {
            return Err(corrupto("model.pth"));
        }

        let tipo = DType::F32;
        let vb = VarBuilder::from_tensors(tensores, tipo, dispositivo);
        let bert =
            Albert::load(&config.plbert, vb.pp("bert"), dispositivo).map_err(fallo("albert"))?;
        let bert_encoder = candle_nn::linear(
            config.plbert.hidden_size,
            config.hidden_dim,
            vb.pp("bert_encoder"),
        )
        .map_err(fallo("proyección"))?;
        let prosodia = ProsodyPredictor::load(
            config.style_dim,
            config.hidden_dim,
            config.n_layer,
            config.max_dur,
            vb.pp("predictor"),
            dispositivo,
        )
        .map_err(fallo("prosodia"))?;
        let texto = TextEncoder::load(
            config.hidden_dim,
            config.text_encoder_kernel_size,
            config.n_layer,
            config.n_token,
            vb.pp("text_encoder"),
            dispositivo,
        )
        .map_err(fallo("texto"))?;
        let istft = &config.istftnet;
        let generador = IstftDecoder::load(
            config.hidden_dim,
            config.style_dim,
            config.n_mels,
            &istft.resblock_kernel_sizes,
            &istft.upsample_rates,
            istft.upsample_initial_channel,
            &istft.resblock_dilation_sizes,
            &istft.upsample_kernel_sizes,
            istft.gen_istft_n_fft,
            istft.gen_istft_hop_size,
            vb.pp("decoder"),
            dispositivo,
            tipo,
        )
        .map_err(fallo("generador"))?;

        Ok(Self {
            config,
            bert,
            bert_encoder,
            prosodia,
            texto,
            generador,
            dispositivo: dispositivo.clone(),
            tipo,
        })
    }

    /// Carga una voz de su archivo `.pt`.
    ///
    /// # Por qué se lee el contenedor a mano y no con el lector de Candle
    ///
    /// Porque el archivo de una voz **no es un diccionario de tensores**: es un
    /// tensor suelto, y el lector de Candle recorre diccionarios, así que devuelve
    /// cero entradas sin dar error. Se comprobó el 2026-09-17 leyendo el archivo
    /// real: `read_all` devolvía una lista vacía y la voz salía "corrupta".
    ///
    /// Lo que hay dentro es un ZIP con la estructura de `torch.save`: el *pickle*
    /// con la forma, y el almacén `data/0` con los números en crudo. Aquí se lee el
    /// almacén con [`crate::zip`], que es el lector del proyecto y trae sus límites
    /// dentro, y **no se ejecuta el pickle**: la forma se deduce del tamaño, que es
    /// posible porque el catálogo fija la huella exacta del archivo. Si mañana el
    /// autor publicara otro, no llegaría hasta aquí: lo pararía la huella.
    ///
    /// # Errors
    ///
    /// [`NbError::DocumentCorrupted`] si el archivo no trae el almacén esperado, si
    /// no viene en orden de bytes pequeño o si su tamaño no cuadra con el estilo.
    pub fn voz(&self, carpeta: &Path, nombre: &str) -> NbResult<Voz> {
        let corrupta = || {
            tracing::warn!(voz = nombre, "una voz no se deja cargar");
            NbError::DocumentCorrupted
        };
        let archivo = carpeta.join(format!("{nombre}.pt"));
        let mut contenedor = crate::zip::Archive::open(&archivo)?;

        // El orden de bytes lo declara el propio contenedor. Leer al revés no da
        // error: da ruido, que es peor.
        if let Some(entrada) = nombre_que_termina(&contenedor, "/byteorder") {
            let orden = contenedor.read_text(&entrada)?;
            if orden.trim() != "little" {
                tracing::warn!(voz = nombre, "la voz viene en otro orden de bytes");
                return Err(NbError::DocumentCorrupted);
            }
        }

        let entrada = nombre_que_termina(&contenedor, "/data/0").ok_or_else(corrupta)?;
        let bytes = contenedor.read(&entrada)?;
        if bytes.len() % (4 * 2 * ESTILO) != 0 || bytes.is_empty() {
            return Err(corrupta());
        }
        let valores: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        let filas = valores.len() / (2 * ESTILO);

        Ok(Voz {
            nombre: nombre.to_owned(),
            estilo: Tensor::from_vec(valores, (filas, 2 * ESTILO), &self.dispositivo)
                .and_then(|t| t.to_dtype(self.tipo))
                .map_err(fallo("voz"))?,
        })
    }

    /// Las fichas de una cadena de fonemas. Lo que el modelo no sabe decir se cae
    /// aquí, y por eso `fonemas::todos_admitidos` existe: para que no se caiga nada.
    fn fichas(&self, fonemas: &str) -> Vec<u32> {
        let mut fichas = Vec::with_capacity(fonemas.len() + 2);
        // El modelo espera la secuencia entre ceros.
        fichas.push(0);
        for c in fonemas.chars() {
            if let Some(ficha) = self.config.vocab.get(&c.to_string()) {
                fichas.push(*ficha);
            }
        }
        fichas.push(0);
        fichas
    }

    /// Sintetiza una cadena de **fonemas** —no de texto— con una voz y una velocidad.
    ///
    /// `velocidad` es un factor: 1,0 es la del modelo y 1,25 va una cuarta parte más
    /// deprisa.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cálculo falla.
    ///
    /// Una cadena sin nada que el modelo sepa decir -"* * *", un símbolo suelto-
    /// devuelve **silencio y no error**. Antes era `Internal`, sin
    /// registrar nada, y una línea de separación entre escenas bastaba para parar la
    /// preparación de un capítulo entero con "algo falló por dentro".
    pub fn sintetizar(&self, fonemas: &str, voz: &Voz, velocidad: f32) -> NbResult<Vec<f32>> {
        let fichas = self.fichas(fonemas);
        if fichas.len() <= 2 {
            return Ok(Vec::new());
        }
        let estilo = self.fila_de_estilo(voz, fichas.len())?;
        let audio = self.adelante(&fichas, &estilo, velocidad.clamp(0.5, 3.0))?;
        audio
            .flatten_all()
            .and_then(|t| t.to_vec1::<f32>())
            .map_err(fallo("muestras"))
    }

    /// La fila del tensor de voz que corresponde a un texto de esta longitud.
    ///
    /// El tensor trae una fila por longitud de entrada —510 en el modelo publicado,
    /// y el número sale del tensor y no de una constante—: la prosodia de una frase
    /// corta no es la de un párrafo. Lo que pase de la última fila usa esa última.
    fn fila_de_estilo(&self, voz: &Voz, fichas: usize) -> NbResult<Tensor> {
        let estilo = voz.estilo.squeeze(1).unwrap_or_else(|_| voz.estilo.clone());
        let filas = estilo.dim(0).map_err(fallo("voz"))?;
        let fila = fichas.saturating_sub(3).min(filas.saturating_sub(1));
        estilo
            .i(fila)
            .and_then(|t| t.unsqueeze(0))
            .map_err(fallo("voz"))
    }

    fn adelante(&self, fichas: &[u32], estilo: &Tensor, velocidad: f32) -> NbResult<Tensor> {
        let largo = fichas.len();
        let ids = Tensor::new(fichas, &self.dispositivo)
            .and_then(|t| t.unsqueeze(0))
            .map_err(fallo("entrada"))?;
        let longitudes = Tensor::new(
            &[u32::try_from(largo).unwrap_or(u32::MAX)],
            &self.dispositivo,
        )
        .map_err(fallo("entrada"))?;
        // Ninguna posición sobra: se sintetiza una secuencia cada vez.
        let mascara =
            Tensor::zeros((1, largo), DType::U8, &self.dispositivo).map_err(fallo("entrada"))?;
        let atencion = Tensor::ones((1, largo), DType::F32, &self.dispositivo)
            .and_then(|t| t.to_dtype(self.tipo))
            .map_err(fallo("entrada"))?;

        // Los tiempos por etapa se registran porque el factor de tiempo real es el
        // dato que decide si esta voz se puede usar, y un total sin desglose no
        // dice dónde mirar. Van a `debug`: no cuestan nada apagados.
        let reloj = std::time::Instant::now();
        let codificado = self
            .bert
            .forward(&ids, &atencion)
            .map_err(fallo("albert"))?;
        let t_bert = reloj.elapsed();
        let proyectado = self
            .bert_encoder
            .forward(&codificado)
            .and_then(|t| t.transpose(1, 2))
            .map_err(fallo("proyección"))?;

        // La primera mitad del vector de voz es del generador; la segunda, de la
        // prosodia.
        let estilo_prosodia = estilo.narrow(1, ESTILO, ESTILO).map_err(fallo("voz"))?;
        let estilo_generador = estilo.narrow(1, 0, ESTILO).map_err(fallo("voz"))?;

        let reloj = std::time::Instant::now();
        let rasgos = self
            .prosodia
            .text_encoder
            .forward(&proyectado, &estilo_prosodia, &longitudes, &mascara)
            .map_err(fallo("prosodia"))?;
        let duracion = self
            .prosodia
            .predict_duration(&rasgos, &estilo_prosodia)
            .map_err(fallo("duración"))?;
        // Hablar más deprisa es durar menos: es el mismo control de velocidad que ya
        // tiene el panel de voz para la del sistema.
        let duracion = Tensor::new(1.0_f32 / velocidad, duracion.device())
            .and_then(|f| f.to_dtype(duracion.dtype()))
            .and_then(|f| duracion.broadcast_mul(&f))
            .map_err(fallo("velocidad"))?;

        let tramos = self.tramos(&duracion)?;
        let total: usize = tramos.iter().sum();
        if total == 0 {
            tracing::warn!(paso = "duración", "la voz neuronal no dio ni un tramo");
            return Err(NbError::Internal);
        }

        let alineacion = self.alineacion(&tramos, largo, total)?;
        let estirado = rasgos
            .transpose(1, 2)
            .and_then(|t| t.contiguous())
            .and_then(|t| t.matmul(&alineacion))
            .map_err(fallo("alineación"))?;

        let (tono, ruido) = self
            .prosodia
            .f0_n_predict(&estirado, &estilo_prosodia)
            .map_err(fallo("tono"))?;
        let t_prosodia = reloj.elapsed();

        let reloj = std::time::Instant::now();
        let del_texto = self
            .texto
            .forward(&ids, &longitudes, &mascara)
            .and_then(|t| t.contiguous())
            .and_then(|t| t.matmul(&alineacion))
            .map_err(fallo("texto"))?;

        let t_texto = reloj.elapsed();

        let reloj = std::time::Instant::now();
        let audio = self
            .generador
            .forward(&del_texto, &tono, &ruido, &estilo_generador)
            .map_err(fallo("generador"))?;
        tracing::debug!(
            fonemas = largo,
            tramos = total,
            bert_ms = t_bert.as_millis(),
            prosodia_ms = t_prosodia.as_millis(),
            texto_ms = t_texto.as_millis(),
            generador_ms = reloj.elapsed().as_millis(),
            "voz neuronal: tiempo por etapa"
        );
        Ok(audio)
    }

    /// Cuántos tramos de audio dura cada fonema. Ninguno menos de uno: un fonema que
    /// dura cero no se oye.
    fn tramos(&self, duracion: &Tensor) -> NbResult<Vec<usize>> {
        let valores: Vec<f32> = duracion
            .squeeze(0)
            .and_then(|t| t.to_dtype(DType::F32))
            .and_then(|t| t.to_vec1())
            .map_err(fallo("duración"))?;
        Ok(valores
            .iter()
            .map(|d| ((d + REDONDEO).round().max(1.0)) as usize)
            .collect())
    }

    /// La matriz que estira cada fonema a sus tramos.
    fn alineacion(&self, tramos: &[usize], largo: usize, total: usize) -> NbResult<Tensor> {
        let mut matriz = vec![0.0_f32; largo * total];
        let mut columna = 0;
        for (fila, cuantos) in tramos.iter().enumerate() {
            for _ in 0..*cuantos {
                matriz[fila * total + columna] = 1.0;
                columna += 1;
            }
        }
        Tensor::from_vec(matriz, (1, largo, total), &self.dispositivo)
            .and_then(|t| t.to_dtype(self.tipo))
            .and_then(|t| t.contiguous())
            .map_err(fallo("alineación"))
    }
}

/// La primera entrada del contenedor cuyo nombre termina así.
///
/// El prefijo es el nombre con el que se guardó —`ef_dora/data/0`—, y depender de
/// él sería depender de cómo llamó el autor a su archivo al crearlo.
fn nombre_que_termina(contenedor: &crate::zip::Archive, sufijo: &str) -> Option<String> {
    contenedor
        .entries()
        .iter()
        .find(|e| e.name.ends_with(sufijo))
        .map(|e| e.name.clone())
}
