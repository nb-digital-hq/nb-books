//! Qué voz neuronal se puede instalar, con qué bytes exactos y bajo qué licencia.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 7.
//!
//! # Por qué está aquí y no en el catálogo de la IA
//!
//! Porque leer en voz alta no es una capacidad de un proveedor de IA: no la ofrece
//! ninguno de los externos, no entra en el registro de envíos y no tiene dimensión
//! ni agrupación. Meterla en [`crate::ai::catalogo::Modelo`] habría obligado a
//! rellenar media docena de campos con valores que no significan nada, que es como
//! empiezan los campos que nadie se atreve a borrar.
//!
//! **Lo que sí se comparte es la garantía**, que es la parte que importa: la huella
//! revisada viaja compilada en el binario, la descarga pasa por la misma puerta con
//! su consentimiento, y la carpeta definitiva solo existe si se verificó entera. Eso
//! vive en [`crate::ai::instalacion`] y aquí se reutiliza a través de
//! [`Instalable`](crate::ai::instalacion::Instalable).
//!
//! # Por qué los pesos son un `.pth`, contra la regla del catálogo
//!
//! La regla dice que solo entran formatos que no ejecutan nada al abrirse, y un
//! *pickle* de Python ejecuta código al abrirse **con Python**. Este no se abre con
//! Python: lo lee el lector restringido de Candle, que interpreta la estructura y
//! los tensores y no ejecuta ninguna llamada. La excepción está probada, no
//! afirmada: `tests/voz_neuronal.rs` le da un pickle con una llamada dentro y
//! comprueba que no ocurre.
//!
//! La alternativa era convertir los pesos nosotros y servirlos desde un alojamiento
//! del proyecto, que no existe; y al hacerlo se perdería la procedencia, que es el
//! motivo entero por el que el catálogo fija repositorio, revisión y huella.

use crate::ai::catalogo::{Archivo, Origen, ANFITRION};
use crate::ai::instalacion::Instalable;

/// Una voz del modelo: cómo se llama, quién habla y en qué idioma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voz {
    /// Identificador del autor del modelo. Es el nombre de su archivo sin extensión.
    pub id: &'static str,
    /// Cómo se llama en la interfaz. **Nunca el nombre de una persona real**: son
    /// voces sintéticas y la interfaz lo dice al lado.
    pub nombre: &'static str,
    /// Etiqueta BCP-47 de lo que sabe decir.
    pub idioma: &'static str,
}

/// El modelo de voz neuronal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeloDeVoz {
    pub id: &'static str,
    pub nombre: &'static str,
    pub autor: &'static str,
    pub repositorio: &'static str,
    /// La revisión que se revisó, entera. Nunca `main`: ver `ai::catalogo`.
    pub revision: &'static str,
    pub licencia: &'static str,
    /// Muestras por segundo del audio que produce.
    pub muestreo: u32,
    /// Memoria que ocupa cargado, en bytes. **Medida**, no declarada.
    pub memoria: u64,
    pub voces: &'static [Voz],
    pub archivos: &'static [Archivo],
}

impl Instalable for ModeloDeVoz {
    fn id(&self) -> &'static str {
        self.id
    }
    fn archivos(&self) -> &'static [Archivo] {
        self.archivos
    }
    fn direccion(&self, archivo: &Archivo) -> String {
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
}

impl ModeloDeVoz {
    /// Las voces que sirven para un idioma, por su etiqueta BCP-47.
    ///
    /// Compara solo la parte de idioma: quien lee en `es-CO` quiere las voces de
    /// `es`, y ninguna región tiene voz propia.
    #[must_use]
    pub fn voces_de(&self, idioma: &str) -> Vec<&'static Voz> {
        let corto = idioma.split(['-', '_']).next().unwrap_or(idioma);
        self.voces
            .iter()
            .filter(|voz| voz.idioma.split('-').next() == Some(corto))
            .collect()
    }
}

/// La revisión revisada del repositorio del modelo.
const REVISION: &str = "f3ff3571791e39611d31c381e3a41a3af07b4987";

const fn origen_voz(ruta: &'static str) -> Option<Origen> {
    Some(Origen {
        repositorio: "hexgrad/Kokoro-82M",
        revision: REVISION,
        ruta,
    })
}

/// Los pesos se publican con el nombre de su versión y aquí se instalan con el
/// nombre canónico, como ya hace el catálogo de la IA con los `.gguf`.
const fn origen_pesos() -> Option<Origen> {
    origen_voz("kokoro-v1_0.pth")
}

/// Memoria que ocupa cargado, en bytes. **Medida el 2026-09-18**: el
/// proceso pasa de 20 MiB a un pico de 602 con el modelo dentro, y se declara 600.
///
/// No sale de multiplicar parámetros por cuatro: los pesos son de 32 bits y ya
/// ocupan 327 MB en disco, pero el generador de onda trabaja con tramos que no
/// están en el archivo. Se mide porque el tope de memoria decide si se carga.
const MEMORIA: u64 = 600 * 1024 * 1024;

/// **Kokoro-82M**, revisado el 2026-09-17 contra el criterio
/// (`docs/legal/model-reviews.md` sección 2.1).
///
/// Entra porque cumple los cinco requisitos: licencia permisiva, datos declarados,
/// pesos publicados por su autor con revisión fija, tamaño que cabe en una máquina
/// normal y **ninguna capacidad de clonar la voz de una persona** —el motor la
/// tiene y aquí no se vendorizó: ver `kokoro/PROCEDENCIA.md`.
///
/// **Solo trae voces de español.** El modelo publica más, y no entran hasta que
/// exista el conversor de fonemas de su idioma: una voz inglesa leyendo fonemas
/// hechos con reglas del español no suena mal, suena a otra cosa.
pub const KOKORO_82M: ModeloDeVoz = ModeloDeVoz {
    id: "kokoro-82m",
    nombre: "Kokoro 82M",
    autor: "hexgrad",
    repositorio: "hexgrad/Kokoro-82M",
    revision: REVISION,
    licencia: "Apache-2.0",
    muestreo: 24_000,
    // Medida sobre esta máquina: ver `tests/voz_neuronal.rs`.
    memoria: MEMORIA,
    voces: &[
        Voz {
            id: "ef_dora",
            nombre: "Dora",
            idioma: "es",
        },
        Voz {
            id: "em_alex",
            nombre: "Alex",
            idioma: "es",
        },
        Voz {
            id: "em_santa",
            nombre: "Santa",
            idioma: "es",
        },
    ],
    archivos: &[
        Archivo {
            nombre: "model.pth",
            bytes: 327_212_226,
            sha256: "496dba118d1a58f5f3db2efc88dbdc216e0483fc89fe6e47ee1f2c53f18ad1e4",
            origen: origen_pesos(),
        },
        Archivo {
            nombre: "config.json",
            bytes: 2_351,
            sha256: "5abb01e2403b072bf03d04fde160443e209d7a0dad49a423be15196b9b43c17f",
            origen: None,
        },
        Archivo {
            nombre: "ef_dora.pt",
            bytes: 523_420,
            sha256: "d9d69b0f8a2b87a345f269d89639f89dfbd1a6c9da0c498ae36dd34afcf35530",
            origen: origen_voz("voices/ef_dora.pt"),
        },
        Archivo {
            nombre: "em_alex.pt",
            bytes: 523_420,
            sha256: "5eac53f767c3f31a081918ba531969aea850bed18fe56419b804d642c6973431",
            origen: origen_voz("voices/em_alex.pt"),
        },
        Archivo {
            nombre: "em_santa.pt",
            bytes: 523_430,
            sha256: "aa8620cb96cec705823efca0d956a63e158e09ad41aca934d354b7f0778f63cb",
            origen: origen_voz("voices/em_santa.pt"),
        },
    ],
};

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::ai::instalacion::Instalable;

    /// La revisión es una revisión, no un nombre de rama: ver `ai::catalogo`.
    #[test]
    fn la_revision_esta_fijada_por_su_huella() {
        assert_eq!(KOKORO_82M.revision.len(), 40);
        assert!(KOKORO_82M.revision.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// Toda huella es un SHA-256 en minúsculas, y ningún archivo pesa cero.
    #[test]
    fn cada_archivo_lleva_su_huella_y_su_tamano() {
        for archivo in KOKORO_82M.archivos {
            assert_eq!(archivo.sha256.len(), 64, "{}", archivo.nombre);
            assert!(
                archivo
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                "{}",
                archivo.nombre
            );
            assert!(archivo.bytes > 0, "{}", archivo.nombre);
        }
    }

    /// Cada voz del catálogo trae su archivo, y cada archivo de voz, su voz.
    ///
    /// Una voz sin archivo se ofrecería y fallaría al cargarse; un archivo sin voz
    /// serían 500 KB que nadie puede elegir.
    #[test]
    fn toda_voz_tiene_archivo_y_todo_archivo_de_voz_tiene_voz() {
        for voz in KOKORO_82M.voces {
            assert!(
                KOKORO_82M
                    .archivos
                    .iter()
                    .any(|a| a.nombre == format!("{}.pt", voz.id)),
                "la voz {} no trae archivo",
                voz.id
            );
        }
        for archivo in KOKORO_82M.archivos.iter().filter(|a| {
            std::path::Path::new(a.nombre)
                .extension()
                .is_some_and(|e| e == "pt")
        }) {
            let id = archivo.nombre.trim_end_matches(".pt");
            assert!(
                KOKORO_82M.voces.iter().any(|v| v.id == id),
                "el archivo {} no lo puede elegir nadie",
                archivo.nombre
            );
        }
    }

    /// Solo hay voces de los idiomas que el conversor de fonemas sabe decir.
    ///
    /// Es la regla que evita la capacidad construida y sin conectar: una voz sin su
    /// conversor está en la interfaz y no puede sonar bien.
    #[test]
    fn ninguna_voz_de_un_idioma_sin_conversor_de_fonemas() {
        for voz in KOKORO_82M.voces {
            assert_eq!(
                voz.idioma, "es",
                "{} habla un idioma sin conversor de fonemas",
                voz.id
            );
        }
    }

    /// Las direcciones salen del repositorio revisado y con la revisión dentro.
    #[test]
    fn toda_direccion_lleva_la_revision_revisada() {
        for archivo in KOKORO_82M.archivos {
            let url = KOKORO_82M.direccion(archivo);
            assert!(url.starts_with("https://huggingface.co/hexgrad/Kokoro-82M/resolve/"));
            assert!(url.contains(REVISION), "{url}");
        }
    }

    /// Las voces se filtran por idioma sin mirar la región.
    #[test]
    fn las_voces_de_un_idioma_no_dependen_de_la_region() {
        assert_eq!(KOKORO_82M.voces_de("es-CO").len(), 3);
        assert_eq!(KOKORO_82M.voces_de("es").len(), 3);
        assert!(KOKORO_82M.voces_de("en-US").is_empty());
    }
}
