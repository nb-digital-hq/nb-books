//! El catálogo de modelos: qué se puede instalar, con qué bytes exactos y bajo
//! qué licencia.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, secciones 2 y 3, y
//! `docs/legal/model-reviews.md`.
//!
//! # Por qué está compilado en el binario
//!
//! Porque **es la garantía de procedencia**. Los pesos se descargan de Hugging
//! Face y no de un alojamiento del proyecto, que no existe; lo que hace que eso
//! sea aceptable es que la huella de cada archivo viaja aquí, dentro del binario
//! que se firma. El anfitrión solo transporta bytes, y los bytes que no coinciden
//! se tiran. Un catálogo descargado de la red perdería justo eso: la huella
//! vendría del mismo sitio que el archivo, y no daría fe de nada.
//!
//! # Por qué una revisión fija y nunca `main`
//!
//! Porque se revisaron unos bytes, no un nombre. Un repositorio de modelos cambia
//! —se vuelve a subir un archivo, cambia la ficha, cambia la licencia—, y una
//! dirección a `main` descargaría mañana algo distinto de lo revisado hoy. Con la
//! revisión fijada, un cambio en el repositorio no llega a nadie hasta que se
//! vuelve a revisar y se cambia aquí, y cambiarlo aquí sin revisarlo rompe la
//! verificación.

use super::Capability;

/// El anfitrión desde el que se descargan los modelos.
///
/// La descarga **no sale solo de aquí**: este anfitrión redirige a su red de
/// distribución, y `net.rs` no sigue una redirección sin el consentimiento del
/// anfitrión nuevo. Por eso no se declara aquí una lista cerrada de anfitriones:
/// el de la red de distribución cambia por región, y quien decide si se sale a él
/// es la puerta, que lo pregunta cuando aparece.
pub const ANFITRION: &str = "huggingface.co";

/// Cómo está construido el modelo, que es lo que decide qué código lo carga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arquitectura {
    XlmRoberta,
    /// Qwen2 cuantizado en GGUF, para generar.
    Qwen2Gguf,
    /// Qwen3 cuantizado en GGUF. Razona antes de contestar, y por eso se le abre el
    /// turno con el bloque de pensamiento ya cerrado: ver `generativo::Red`.
    Qwen3Gguf,
}

/// Cómo se reduce la salida del modelo a un solo vector.
///
/// No se deduce: lo declara cada modelo en su `1_Pooling/config.json`, y
/// equivocarse no da error, da vectores peores sin que nada lo avise. Por eso se
/// fija aquí, al revisarlo, y no se adivina al cargarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agrupacion {
    /// El vector del primer token.
    Cls,
    /// No produce vectores: es un modelo que genera.
    Ninguna,
}

/// Un archivo del modelo, con los bytes que se revisaron.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Archivo {
    /// Cómo se llama en la carpeta del modelo instalado.
    pub nombre: &'static str,
    pub bytes: u64,
    /// SHA-256 en hexadecimal, en minúsculas.
    pub sha256: &'static str,
    /// De dónde sale, si no es del repositorio del modelo con su mismo nombre.
    ///
    /// Existe por los modelos que generan: los pesos cuantizados los publica su
    /// autor en un repositorio aparte (`…-GGUF`), con un nombre de archivo propio, y
    /// el tokenizador se queda en el del modelo original. Cada origen lleva su
    /// revisión fijada, por lo mismo que la del modelo.
    pub origen: Option<Origen>,
}

/// El repositorio, la revisión y la ruta de un archivo que no sale del del modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Origen {
    pub repositorio: &'static str,
    pub revision: &'static str,
    pub ruta: &'static str,
}

/// Un modelo del catálogo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modelo {
    /// Identificador estable. Es el nombre de su carpeta y el que se guarda como
    /// procedencia de lo que genere.
    pub id: &'static str,
    pub nombre: &'static str,
    pub autor: &'static str,
    /// Repositorio en el anfitrión.
    pub repositorio: &'static str,
    /// La revisión que se revisó, entera.
    pub revision: &'static str,
    /// Identificador SPDX. Es lo que la interfaz enseña antes de descargar.
    pub licencia: &'static str,
    pub capacidad: Capability,
    pub dimension: usize,
    pub arquitectura: Arquitectura,
    pub agrupacion: Agrupacion,
    /// Tokens que admite de una vez. Lo que pase de ahí se trunca.
    pub longitud_maxima: usize,
    /// Idiomas declarados por su autor, en etiquetas BCP-47.
    pub idiomas: &'static [&'static str],
    /// Memoria que ocupa cargado, en bytes, **sin contar el cálculo**.
    ///
    /// Los pesos vienen en `bfloat16` y el procesador calcula en coma flotante de 32
    /// bits, así que cargados ocupan el doble de lo que pesan en disco, más el
    /// tokenizador. **Medida** y no solo declarada: ver cada entrada.
    pub memoria: u64,
    /// Memoria de trabajo que añade cada texto de un lote, con el texto en su
    /// longitud máxima, en bytes.
    ///
    /// Es lo que crece con el lote —la atención es cuadrática en la longitud— y lo
    /// que el presupuesto usa para decidir de qué tamaño son los lotes.
    /// Medida, igual que [`Self::memoria`].
    pub memoria_por_texto: u64,
    /// Qué clase de salida da, medido. Ver [`Rasgo`].
    pub rasgo: Rasgo,
    /// En qué orden se recomienda entre los que el equipo aguanta: **menor, antes**.
    ///
    /// Sale de la medición, no del tamaño. Ver la nota de cada entrada.
    pub preferencia: u8,
    /// Segundos que tarda una función de estudio típica **en la máquina de
    /// referencia**: 8 núcleos -4 hilos para la inferencia- y AVX2 compilado.
    ///
    /// Existe para poder decirle a quien elige **lo que es probable que pase** con
    /// cada opción en su equipo, y no solo cuál se recomienda: Se pidió el
    /// 2026-09-22, "una cosa es recomendar y otra obligar". De aquí sale
    /// [`prevision`]. Cero en un modelo que no genera.
    /// En segundos enteros y no en minutos con decimales para que el modelo siga
    /// siendo comparable por igualdad, que es lo que usan las pruebas del catálogo.
    pub segundos_de_referencia: u32,
    /// Desde qué equipo se **recomienda**, no desde cuál se permite.
    ///
    /// Se decidió el 2026-09-17: la máquina es lo único que no se puede
    /// igualar entre usuarios, así que el catálogo ofrece varios modelos, la
    /// aplicación lee el equipo y recomienda uno, y quien lo tiene elige. Instalar
    /// otro sigue siendo posible: esto solo dice cuál se señala.
    pub equipo_recomendado: Equipo,
    pub archivos: &'static [Archivo],
}

/// Qué clase de salida da un modelo generativo, **medido** sobre el corpus de
/// evaluación y no leído de su ficha.
///
/// Existe porque la medición del 2026-09-17 dio la vuelta a lo que parecía: el modelo
/// más grande escribe mejor prosa y **cita peor**, porque se aleja del texto; el
/// pequeño se pega al texto y por eso sus citas resisten la comprobación. En una
/// herramienta de estudio, donde toda afirmación tiene que poder abrirse en el libro,
/// eso no es un matiz de estilo: decide cuál se recomienda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rasgo {
    /// Se pega al texto. Sus citas resisten la comprobación.
    FielAlTexto,
    /// Escribe mejor, se aleja más, y pierde citas al comprobarlas.
    MejorProsa,
    /// No genera: hace vectores.
    NoGenera,
}

impl Rasgo {
    /// Nombre estable; lo traduce la interfaz.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FielAlTexto => "fiel",
            Self::MejorProsa => "prosa",
            Self::NoGenera => "vectores",
        }
    }
}

/// El equipo mínimo desde el que se recomienda un modelo.
///
/// Las dos cifras salen de lo medido, no de la ficha del autor: la memoria, de lo
/// que ocupa cargado con su margen de trabajo; los núcleos, de que la velocidad de
/// generación escala con los hilos del grupo de la inferencia, que son la mitad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Equipo {
    pub memoria: u64,
    pub nucleos: usize,
}

impl Modelo {
    /// Lo que ocupa en disco una vez instalado.
    #[must_use]
    pub fn disco(&self) -> u64 {
        self.archivos.iter().map(|a| a.bytes).sum()
    }

    /// De dónde se descarga un archivo suyo.
    ///
    /// Con la revisión dentro de la dirección, nunca con `main`: ver el
    /// encabezado del módulo.
    #[must_use]
    pub fn direccion(&self, archivo: &Archivo) -> String {
        match archivo.origen {
            Some(origen) => format!(
                "https://{ANFITRION}/{}/resolve/{}/{}",
                origen.repositorio, origen.revision, origen.ruta
            ),
            None => format!(
                "https://{ANFITRION}/{}/resolve/{}/{}",
                self.repositorio, self.revision, archivo.nombre
            ),
        }
    }

    /// Si genera texto en lugar de vectores.
    #[must_use]
    pub const fn genera(&self) -> bool {
        !matches!(self.capacidad, Capability::Embedding)
    }
}

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

/// El modelo de embeddings, decidido el 2026-09-16.
///
/// Frente a `multilingual-e5-small`, que es mejor —59,3 frente a 55,9 en MIRACL
/// según el propio informe de IBM—, pesó que es dos veces más rápido y descarga
/// menos de la mitad. La revisión y las alternativas están en
/// `docs/legal/model-reviews.md`.
pub const GRANITE_107M_MULTILINGUAL: Modelo = Modelo {
    id: "granite-embedding-107m-multilingual",
    nombre: "Granite Embedding 107M Multilingual",
    autor: "IBM",
    repositorio: "ibm-granite/granite-embedding-107m-multilingual",
    revision: "d6cffd338414d6a1c1f5decfad5fec62eebc90d5",
    licencia: "Apache-2.0",
    capacidad: Capability::Embedding,
    dimension: 384,
    arquitectura: Arquitectura::XlmRoberta,
    agrupacion: Agrupacion::Cls,
    longitud_maxima: 512,
    idiomas: &[
        "en", "de", "es", "fr", "ja", "pt", "ar", "cs", "it", "ko", "nl", "zh",
    ],
    // Medido el 2026-09-16 en release: el proceso pasa de 5 MB a 705 con el modelo
    // cargado. Se declaran 768 MiB, unos 805 MB, de margen.
    //
    // **Y este comentario decía que ese margen cubría los lotes de
    // fragmentos largos, y no los cubre.** Medido con textos de 512 tokens —un
    // fragmento de búsqueda son 1.920 caracteres, que es justo eso—, cada texto del
    // lote añade entre 21 y 26 MB, y el lote de dieciséis llevaba el proceso a
    // 1.074 MB: 270 por encima de lo declarado. Ver `memoria_por_texto`.
    memoria: 768 * MIB,
    // Pico sobre el modelo cargado, con textos de 512 tokens: 24 MB con uno, 43 con
    // dos, 97 con cuatro, 208 con ocho y 370 con dieciséis. Se toma la mayor por
    // texto, redondeada hacia arriba.
    memoria_por_texto: 26 * MIB,
    rasgo: Rasgo::NoGenera,
    preferencia: 0,
    // No genera: no tarda nada que se pueda decir en minutos.
    segundos_de_referencia: 0,
    // Los embeddings caben en cualquier equipo que pase el mínimo de `hardware`.
    equipo_recomendado: Equipo {
        memoria: 4 * GIB,
        nucleos: 2,
    },
    archivos: &[
        Archivo {
            nombre: "model.safetensors",
            bytes: 214_000_232,
            sha256: "ae6f20cc5dc433041118c900b652d87532e1335dade78c37765f0b94d1161283",
            origen: None,
        },
        Archivo {
            nombre: "tokenizer.json",
            bytes: 9_081_382,
            sha256: "7fc9e475f2ac473f070052d92b0c46ec2b91d9cd672935feb6fd837a65d33f78",
            origen: None,
        },
        Archivo {
            nombre: "config.json",
            bytes: 697,
            sha256: "aa3b6a5d1d6efeacdb844fb8ba773556463af2195003ad5e70a6f014d11be9cf",
            origen: None,
        },
    ],
};

/// El modelo que genera en local, **propuesto y pendiente de que se
/// apruebe**: ver su revisión en `docs/legal/model-reviews.md`.
///
/// Medido el 2026-09-17 contra Qwen3-1.7B (Q8_0) y Qwen3-4B (Q4_K_M), los tres en
/// el mismo equipo, con cuatro hilos y el mismo texto de unos 800 tokens: es el que
/// menos memoria ocupa (1,98 GB frente a 3,6 y 4,7) y el que antes lee la entrada
/// (14 tokens por segundo frente a 8-10 y 6). Lo que se descarta a cambio, y está
/// dicho: Qwen3-4B fue el único que devolvió JSON válido en la prueba de tarjetas y
/// el que mejor cita; Qwen2.5 se dejó una llave de más al cerrar, que la lectura de
/// salidas rescata. Las medidas están en `docs/architecture/ai/local-models.md`
/// sección 6.
pub const QWEN25_15B_INSTRUCT: Modelo = Modelo {
    id: "qwen25-15b-instruct-q4km",
    nombre: "Qwen2.5 1.5B Instruct",
    autor: "Qwen (Alibaba Cloud)",
    repositorio: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
    revision: "91cad51170dc346986eccefdc2dd33a9da36ead9",
    licencia: "Apache-2.0",
    capacidad: Capability::Summary,
    dimension: 0,
    arquitectura: Arquitectura::Qwen2Gguf,
    agrupacion: Agrupacion::Ninguna,
    // La ventana que se usa, no la que admite (32.768): cada token de entrada son
    // unos 70 ms de procesador con AVX2, y la entrada se acota antes por caracteres.
    longitud_maxima: 4_096,
    idiomas: &[
        "zh", "en", "fr", "es", "pt", "de", "it", "ru", "ja", "ko", "vi", "th", "ar",
    ],
    // Medido: 1.919 MB de memoria anónima con el modelo cargado. Candle convierte la
    // tabla de embeddings a coma flotante (152.000 × 1.536 × 4 bytes = 933 MB), y eso
    // es la mitad de lo que ocupa. Se declaran 1.920 MiB.
    memoria: 1_920 * MIB,
    // Medido: 58 MB más al generar 283 tokens sobre una entrada de 813.
    memoria_por_texto: 96 * MIB,
    // **El primero que se recomienda**, y no por pequeño: medido sobre el corpus el
    // 2026-09-17, sus tarjetas superan la rúbrica en los cuatro textos y sus citas
    // sobreviven a la comprobación, porque se pega al texto.
    rasgo: Rasgo::FielAlTexto,
    preferencia: 1,
    // Medido el 2026-09-17: unos dos minutos por resumen con AVX2 (14 tokens/s
    // leyendo y 4,4 escribiendo). Es la cifra de la que salen las demás.
    segundos_de_referencia: 120,
    equipo_recomendado: Equipo {
        memoria: 6 * GIB,
        nucleos: 4,
    },
    archivos: &[
        Archivo {
            nombre: "model.gguf",
            bytes: 1_117_320_736,
            sha256: "6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e",
            origen: Some(Origen {
                repositorio: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
                revision: "91cad51170dc346986eccefdc2dd33a9da36ead9",
                ruta: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
            }),
        },
        Archivo {
            nombre: "tokenizer.json",
            bytes: 7_031_645,
            sha256: "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539",
            origen: Some(Origen {
                repositorio: "Qwen/Qwen2.5-1.5B-Instruct",
                revision: "989aa7980e4cf806f80c7fef2b1adb7bc71aa306",
                ruta: "tokenizer.json",
            }),
        },
    ],
};

/// El modelo generativo **equilibrado**, para un equipo con memoria de sobra.
///
/// Medido el 2026-09-17: escribe mejor que Qwen2.5-1.5B y cita en prosa, a cambio de
/// casi el doble de memoria y de leer la entrada a la mitad de velocidad. Razona
/// antes de contestar, y eso se le desactiva: ver `generativo::Red::apertura`.
pub const QWEN3_17B: Modelo = Modelo {
    id: "qwen3-17b-q8",
    nombre: "Qwen3 1.7B",
    autor: "Qwen (Alibaba Cloud)",
    repositorio: "Qwen/Qwen3-1.7B-GGUF",
    revision: "90862c4b9d2787eaed51d12237eafdfe7c5f6077",
    licencia: "Apache-2.0",
    capacidad: Capability::Summary,
    dimension: 0,
    arquitectura: Arquitectura::Qwen3Gguf,
    agrupacion: Agrupacion::Ninguna,
    longitud_maxima: 4_096,
    idiomas: &[
        "zh", "en", "fr", "es", "pt", "de", "it", "ru", "ja", "ko", "vi", "th", "ar",
    ],
    // Medido: 3.089 MB con el modelo cargado y 3.591 generando.
    memoria: 3_100 * MIB,
    memoria_por_texto: 512 * MIB,
    // Escribe mejor y **cita peor**: medido, resume con sus palabras, y al comprobar
    // las citas se queda sin ellas. Se ofrece, no se recomienda.
    rasgo: Rasgo::MejorProsa,
    preferencia: 3,
    // Derivado de lo medido, y se dice: lee la entrada **a la mitad de velocidad**
    // que Qwen2.5-1.5B, y la lectura es lo que más pesa en una función de estudio.
    segundos_de_referencia: 240,
    equipo_recomendado: Equipo {
        memoria: 8 * GIB,
        nucleos: 6,
    },
    archivos: &[
        Archivo {
            nombre: "model.gguf",
            bytes: 1_834_426_016,
            sha256: "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a",
            origen: Some(Origen {
                repositorio: "Qwen/Qwen3-1.7B-GGUF",
                revision: "90862c4b9d2787eaed51d12237eafdfe7c5f6077",
                ruta: "Qwen3-1.7B-Q8_0.gguf",
            }),
        },
        Archivo {
            nombre: "tokenizer.json",
            bytes: 11_422_654,
            sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            origen: Some(Origen {
                repositorio: "Qwen/Qwen3-1.7B",
                revision: "70d244cc86ccca08cf5af4e1e306ecf908b1ad5e",
                ruta: "tokenizer.json",
            }),
        },
    ],
};

/// El modelo generativo **grande**, para quien tenga con qué.
///
/// Medido el 2026-09-17: el único de los tres que devolvió JSON válido en la prueba
/// de tarjetas y el que mejor las escribió, a cambio de 4,7 GB cargados y de casi
/// cinco minutos por resumen.
pub const QWEN3_4B: Modelo = Modelo {
    id: "qwen3-4b-q4km",
    nombre: "Qwen3 4B",
    autor: "Qwen (Alibaba Cloud)",
    repositorio: "Qwen/Qwen3-4B-GGUF",
    revision: "bc640142c66e1fdd12af0bd68f40445458f3869b",
    licencia: "Apache-2.0",
    capacidad: Capability::Summary,
    dimension: 0,
    arquitectura: Arquitectura::Qwen3Gguf,
    agrupacion: Agrupacion::Ninguna,
    longitud_maxima: 4_096,
    idiomas: &[
        "zh", "en", "fr", "es", "pt", "de", "it", "ru", "ja", "ko", "vi", "th", "ar",
    ],
    // Medido: 4.020 MB con el modelo cargado y 4.678 generando.
    memoria: 4_032 * MIB,
    memoria_por_texto: 672 * MIB,
    rasgo: Rasgo::MejorProsa,
    preferencia: 4,
    // Medido el 2026-09-17: "casi cinco minutos por resumen".
    segundos_de_referencia: 300,
    equipo_recomendado: Equipo {
        memoria: 12 * GIB,
        nucleos: 8,
    },
    archivos: &[
        Archivo {
            nombre: "model.gguf",
            bytes: 2_497_280_256,
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5",
            origen: Some(Origen {
                repositorio: "Qwen/Qwen3-4B-GGUF",
                revision: "bc640142c66e1fdd12af0bd68f40445458f3869b",
                ruta: "Qwen3-4B-Q4_K_M.gguf",
            }),
        },
        Archivo {
            nombre: "tokenizer.json",
            bytes: 11_422_654,
            sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
            origen: Some(Origen {
                repositorio: "Qwen/Qwen3-4B",
                revision: "1cfa9a7208912126459214e8b04321603b3df60c",
                ruta: "tokenizer.json",
            }),
        },
    ],
};

/// Todo lo que se puede instalar, **de menos a más exigente**: el orden es el que
/// recorre [`recomendado`].
pub const CATALOGO: &[Modelo] = &[
    GRANITE_107M_MULTILINGUAL,
    QWEN25_15B_INSTRUCT,
    QWEN3_17B,
    QWEN3_4B,
];

/// Los modelos que generan texto, de menos a más exigente.
pub fn generativos() -> impl Iterator<Item = &'static Modelo> {
    CATALOGO.iter().filter(|m| m.genera())
}

/// El modelo generativo que se recomienda en un equipo, si alguno.
///
/// **El mejor medido de los que el equipo aguanta**, no el mayor. Esa distinción la
/// impuso la medición del 2026-09-17, que dio la vuelta a lo que parecía: el modelo
/// más grande escribe mejor prosa y **cita peor**, porque se aleja del texto, y en una
/// herramienta de estudio lo que no se puede abrir en el libro no vale. Recomendar
/// por tamaño habría puesto delante justo a ese.
///
/// `memoria` es la del equipo entero; `None` cuando no se pudo leer, y entonces se
/// recomienda el de menos exigencia, que es lo prudente.
#[must_use]
pub fn recomendado(memoria: Option<u64>, nucleos: usize) -> Option<&'static Modelo> {
    let Some(memoria) = memoria else {
        return generativos().min_by_key(|m| m.equipo_recomendado.memoria);
    };
    generativos()
        .filter(|m| {
            memoria >= m.equipo_recomendado.memoria && nucleos >= m.equipo_recomendado.nucleos
        })
        .min_by_key(|m| m.preferencia)
}

/// Hilos de la inferencia en la máquina de referencia de [`Modelo::segundos_de_referencia`].
const HILOS_DE_REFERENCIA: f32 = 4.0;

/// Cuántas veces más tarda una función sin AVX2 compilado. **Medido**: veinte
/// minutos frente a dos con Qwen2.5-1.5B, el 2026-09-17.
const SIN_AVX2: f32 = 10.0;

/// Lo que es probable que pase con un modelo **en este equipo**.
///
/// # Por qué existe
///
/// Porque Se decidió el 2026-09-22: la aplicación recomienda, no obliga.
/// Hasta entonces, un modelo que el equipo no movía bien simplemente no se
/// ofrecía -sin AVX2 compilado, la generación local se declaraba `NOT_SUPPORTED`- y
/// la persona no tenía forma de elegirlo aunque quisiera. Ahora se ofrecen todos, y
/// cada uno dice qué es probable que ocurra, con los datos que hay.
#[derive(Debug, Clone, PartialEq)]
pub struct Prevision {
    /// Minutos por función de estudio en este equipo. Estimados a partir de lo
    /// medido en la máquina de referencia, **no medidos aquí**, y la interfaz lo
    /// dice así. `None` en un modelo que no genera.
    pub minutos: Option<f32>,
    /// Lo que hay que saber antes de elegirlo, de una lista cerrada: la interfaz
    /// traduce cada código.
    pub avisos: Vec<&'static str>,
}

/// Lo que es probable que pase con `modelo` en un equipo con `memoria` bytes,
/// `nucleos` núcleos y los núcleos cuantizados rápidos o no (`rapido`).
///
/// Los avisos, de más grave a menos:
///
/// * `EXCEEDS_MACHINE_MEMORY`: cargado y generando pide más memoria de la que el
///   equipo tiene **en total**. Lo más probable es que el sistema cierre la
///   aplicación. Se ofrece igual, con confirmación: Se decidió el 2026-09-22.
/// * `SLOW_BUILD`: esta compilación no usa AVX2 y el procesador es x86-64. Diez
///   veces más lento, medido.
/// * `BELOW_RECOMMENDED`: el equipo no llega a lo que se recomienda para él.
/// * `CITES_WORSE`: medido sobre el corpus, escribe mejor y sus citas resisten
///   peor la comprobación.
#[must_use]
pub fn prevision(modelo: &Modelo, memoria: Option<u64>, nucleos: usize, rapido: bool) -> Prevision {
    let mut avisos = Vec::new();
    if !modelo.genera() {
        return Prevision {
            minutos: None,
            avisos,
        };
    }

    let pide = modelo.memoria + modelo.memoria_por_texto;
    if memoria.is_some_and(|total| pide > total) {
        avisos.push("EXCEEDS_MACHINE_MEMORY");
    }
    if !rapido {
        avisos.push("SLOW_BUILD");
    }
    let por_debajo = memoria.is_some_and(|total| total < modelo.equipo_recomendado.memoria)
        || nucleos < modelo.equipo_recomendado.nucleos;
    if por_debajo {
        avisos.push("BELOW_RECOMMENDED");
    }
    if modelo.rasgo == Rasgo::MejorProsa {
        avisos.push("CITES_WORSE");
    }

    // La velocidad escala con los hilos del grupo de la inferencia, que son la
    // mitad de los núcleos: es lo que se midió al fijar `equipo_recomendado`.
    #[allow(clippy::cast_precision_loss)]
    let hilos = (super::hardware::hilos_para_inferencia(nucleos).max(1)) as f32;
    #[allow(clippy::cast_precision_loss)]
    let mut minutos = modelo.segundos_de_referencia as f32 / 60.0 * HILOS_DE_REFERENCIA / hilos;
    if !rapido {
        minutos *= SIN_AVX2;
    }
    Prevision {
        minutos: Some(minutos),
        avisos,
    }
}

/// Un modelo del catálogo, por su identificador.
#[must_use]
pub fn modelo(id: &str) -> Option<&'static Modelo> {
    CATALOGO.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Recomendar no es obligar** (2026-09-22): la previsión dice lo que
    /// es probable que pase con cada modelo, y lo dice con los datos que hay.
    #[test]
    fn la_prevision_dice_lo_que_es_probable_que_pase_en_cada_equipo() {
        const GIB: u64 = 1024 * 1024 * 1024;

        // En la máquina de referencia, el recomendado tarda lo medido y no avisa.
        let referencia = prevision(&QWEN25_15B_INSTRUCT, Some(16 * GIB), 8, true);
        assert!(referencia.avisos.is_empty(), "{:?}", referencia.avisos);
        assert!((referencia.minutos.expect("genera") - 2.0).abs() < 0.01);

        // Con la mitad de hilos, el doble; sin AVX2, diez veces.
        let pequeno = prevision(&QWEN25_15B_INSTRUCT, Some(16 * GIB), 4, false);
        assert!((pequeno.minutos.expect("genera") - 40.0).abs() < 0.01);
        assert!(pequeno.avisos.contains(&"SLOW_BUILD"));

        // El grande en un equipo de 4 GB: pide más de lo que hay, está por debajo de
        // lo recomendado, y cita peor. Los tres se dicen; ninguno lo esconde.
        let grande = prevision(&QWEN3_4B, Some(4 * GIB), 4, true);
        for aviso in ["EXCEEDS_MACHINE_MEMORY", "BELOW_RECOMMENDED", "CITES_WORSE"] {
            assert!(
                grande.avisos.contains(&aviso),
                "falta {aviso}: {:?}",
                grande.avisos
            );
        }

        // Y un modelo que no genera no promete minutos.
        assert_eq!(
            prevision(&GRANITE_107M_MULTILINGUAL, Some(16 * GIB), 8, true).minutos,
            None
        );
    }

    fn registro() -> String {
        let ruta =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/legal/model-reviews.md");
        std::fs::read_to_string(&ruta).expect("el registro de revisiones existe")
    }

    /// La sección de un modelo en el registro: desde su encabezado hasta el
    /// siguiente de su nivel.
    fn seccion<'a>(registro: &'a str, id: &str) -> Option<&'a str> {
        let encabezado = format!("\n## `{id}`\n");
        let inicio = registro.find(&encabezado)? + encabezado.len();
        let resto = &registro[inicio..];
        let fin = resto.find("\n## ").unwrap_or(resto.len());
        Some(&resto[..fin])
    }

    /// **El criterio de aceptación, y el mecanismo de la regla de
    /// `local-models.md` sección 3.**
    ///
    /// Un modelo entra al catálogo solo con su revisión registrada, y la revisión
    /// tiene que hablar de **los mismos bytes**: la misma licencia, la misma
    /// revisión del repositorio y la misma huella de cada archivo. Cambiar
    /// cualquiera de las tres aquí sin volver a revisar hace fallar esta prueba.
    ///
    /// Lo que no comprueba, y está dicho en el registro: que el juicio de la
    /// revisión sea correcto. Eso lo firma una persona.
    #[test]
    fn todo_modelo_del_catalogo_tiene_su_revision_registrada() {
        let registro = registro();

        for modelo in CATALOGO {
            let seccion = seccion(&registro, modelo.id).unwrap_or_else(|| {
                panic!(
                    "{} está en el catálogo sin revisión registrada en docs/legal/model-reviews.md",
                    modelo.id
                )
            });

            assert!(
                seccion.contains(&format!("| Licencia | `{}` |", modelo.licencia)),
                "la licencia de {} no es la que se revisó",
                modelo.id
            );
            assert!(
                seccion.contains(&format!("| Revisión fijada | `{}` |", modelo.revision)),
                "la revisión de {} no es la que se revisó",
                modelo.id
            );
            for archivo in modelo.archivos {
                if let Some(origen) = archivo.origen {
                    assert!(
                        seccion.contains(&format!(
                            "| `{}` | `{}` | `{}` |",
                            origen.repositorio, origen.revision, origen.ruta
                        )),
                        "el origen de {} de {} no es el que se revisó",
                        archivo.nombre,
                        modelo.id
                    );
                }
                assert!(
                    seccion.contains(&format!(
                        "| `{}` | {} | `{}` |",
                        archivo.nombre, archivo.bytes, archivo.sha256
                    )),
                    "{} de {} no tiene los bytes ni la huella que se revisaron",
                    archivo.nombre,
                    modelo.id
                );
            }
        }
    }

    /// **La recomendación sale de la máquina y de lo medido**, no del tamaño. Lo
    /// primero Se decidió el 2026-09-17 —la máquina es lo único que no se
    /// puede igualar entre usuarios—; lo segundo lo impuso la medición de ese mismo
    /// día, que puso último al modelo más grande porque cita peor.
    #[test]
    fn se_recomienda_el_mejor_medido_que_el_equipo_aguanta() {
        const GB: u64 = 1024 * 1024 * 1024;
        let id = |m: Option<&Modelo>| m.map(|m| m.id);

        // Un portátil corriente: el único que cabe.
        assert_eq!(
            id(recomendado(Some(8 * GB), 4)),
            Some(QWEN25_15B_INSTRUCT.id)
        );
        // Con memoria y núcleos de sobra caben los tres, y **sigue ganando el que
        // mejor cita**, que es el pequeño. Recomendar por tamaño daría otro.
        assert_eq!(
            id(recomendado(Some(32 * GB), 16)),
            Some(QWEN25_15B_INSTRUCT.id)
        );
        // Y que el grande siga siendo el más exigente, o la comprobación de arriba
        // dejaría de decir nada. Se lee del catálogo para que un cambio se note.
        let exigencias: Vec<u64> = generativos()
            .map(|m| m.equipo_recomendado.memoria)
            .collect();
        assert_eq!(
            exigencias.iter().max(),
            exigencias.last(),
            "el último del catálogo dejó de ser el más exigente"
        );
        // Un equipo por debajo de todos no recibe recomendación: descargar un
        // gigabyte para no poder ejecutarlo es peor que no ofrecerlo.
        assert_eq!(id(recomendado(Some(2 * GB), 2)), None);
        // Y si no se pudo leer la memoria, el de menos exigencia.
        assert_eq!(id(recomendado(None, 8)), Some(QWEN25_15B_INSTRUCT.id));
    }

    /// El catálogo sigue yendo de menos a más exigente, que es el orden en que se
    /// enseña en Ajustes; lo que decide la recomendación es `preferencia`.
    #[test]
    fn los_generativos_estan_ordenados_de_menos_a_mas_exigente() {
        let exigencias: Vec<u64> = generativos()
            .map(|m| m.equipo_recomendado.memoria)
            .collect();
        assert!(
            exigencias.windows(2).all(|p| p[0] <= p[1]),
            "{exigencias:?}"
        );
        assert!(
            generativos().count() >= 2,
            "varios modelos, por decisión de producto"
        );
        // Y ninguno declara embeddings: son dos cosas distintas.
        assert!(generativos().all(|m| m.capacidad != Capability::Embedding));
    }

    /// Una huella mal copiada no da error al compilar: da una descarga que se
    /// rechaza siempre, y el usuario vería un modelo imposible de instalar.
    #[test]
    fn cada_huella_es_un_sha256_bien_formado() {
        for modelo in CATALOGO {
            for archivo in modelo.archivos {
                assert_eq!(
                    archivo.sha256.len(),
                    64,
                    "{} de {}",
                    archivo.nombre,
                    modelo.id
                );
                assert!(
                    archivo
                        .sha256
                        .chars()
                        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                    "la huella de {} de {} no es hexadecimal en minúsculas",
                    archivo.nombre,
                    modelo.id
                );
                assert!(archivo.bytes > 0);
                if let Some(origen) = archivo.origen {
                    assert_eq!(origen.revision.len(), 40, "{}", archivo.nombre);
                }
            }
            assert_eq!(
                modelo.revision.len(),
                40,
                "la revisión de {} no es un commit entero",
                modelo.id
            );
        }
    }

    /// Nunca se descarga desde `main`: se revisaron unos bytes, y `main` puede
    /// servir otros mañana.
    #[test]
    fn toda_descarga_apunta_a_la_revision_revisada_y_nunca_a_main() {
        for modelo in CATALOGO {
            for archivo in modelo.archivos {
                let direccion = modelo.direccion(archivo);
                assert!(direccion.starts_with(&format!("https://{ANFITRION}/")));
                let revision = archivo.origen.map_or(modelo.revision, |o| o.revision);
                assert!(direccion.contains(&format!("/resolve/{revision}/")));
                assert!(!direccion.contains("/resolve/main/"));
            }
        }
    }

    /// Solo formatos que no ejecutan nada al abrirse.
    ///
    /// GGUF entra con los modelos que generan: es, como `safetensors`, una
    /// cabecera de metadatos tipados y los tensores en crudo, sin nada que se
    /// ejecute al leerlo.
    ///
    /// Los repositorios de modelos traen a menudo `pytorch_model.bin`, que es un
    /// `pickle` de Python: abrirlo puede ejecutar código. `safetensors` existe
    /// precisamente para que un archivo de pesos sea solo datos, y un catálogo que
    /// admitiera lo otro convertiría "descargar un modelo" en "descargar un
    /// programa".
    #[test]
    fn ningun_archivo_del_catalogo_es_un_formato_que_ejecute_codigo() {
        for modelo in CATALOGO {
            for archivo in modelo.archivos {
                let permitido = std::path::Path::new(archivo.nombre)
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| matches!(e, "safetensors" | "gguf" | "json"));
                assert!(
                    permitido,
                    "{} de {} no es safetensors, gguf ni json",
                    archivo.nombre, modelo.id
                );
            }
            assert!(
                modelo
                    .archivos
                    .iter()
                    .any(|a| a.nombre.ends_with(".safetensors") || a.nombre.ends_with(".gguf")),
                "{} no trae pesos",
                modelo.id
            );
        }
    }

    /// Dos entradas con el mismo identificador compartirían carpeta, y borrar una
    /// se llevaría la otra.
    #[test]
    fn no_hay_dos_modelos_con_el_mismo_identificador() {
        let ids: std::collections::BTreeSet<_> = CATALOGO.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), CATALOGO.len());
        for modelo in CATALOGO {
            assert!(
                modelo
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{} no sirve como nombre de carpeta en las tres plataformas",
                modelo.id
            );
        }
    }

    /// El vector que produce el modelo tiene que ser el que espera la
    /// recuperación: [ADR-007] hizo sus cuentas de disco con 384 dimensiones, y un
    /// modelo de 768 las doblaría sin que nadie lo notara.
    ///
    /// [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md
    #[test]
    fn el_modelo_de_embeddings_produce_la_dimension_con_la_que_cuenta_adr_007() {
        for modelo in CATALOGO
            .iter()
            .filter(|m| m.capacidad == Capability::Embedding)
        {
            assert_eq!(modelo.dimension, 384, "{}", modelo.id);
        }
    }

    /// La biblioteca de referencia está en español: un modelo de embeddings que
    /// no lo declara no sirve para ella. Es la suposición equivocada que se
    /// corrigió el 2026-09-16 —el "modelo de unos 100 MB" solo sabía inglés—.
    #[test]
    fn el_modelo_de_embeddings_declara_espanol_ingles_y_portugues() {
        for modelo in CATALOGO
            .iter()
            .filter(|m| m.capacidad == Capability::Embedding)
        {
            for idioma in ["es", "en", "pt"] {
                assert!(
                    modelo.idiomas.contains(&idioma),
                    "{} no declara {idioma}",
                    modelo.id
                );
            }
        }
    }

    #[test]
    fn lo_que_ocupa_en_disco_es_la_suma_de_sus_archivos() {
        assert_eq!(
            GRANITE_107M_MULTILINGUAL.disco(),
            214_000_232 + 9_081_382 + 697
        );
        assert_eq!(
            modelo(GRANITE_107M_MULTILINGUAL.id),
            Some(&GRANITE_107M_MULTILINGUAL)
        );
        assert_eq!(modelo("no-existe"), None);
    }
}
