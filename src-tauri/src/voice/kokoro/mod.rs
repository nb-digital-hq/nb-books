//! La voz neuronal **dentro de la aplicación**: Kokoro-82M sobre Candle.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 7.
//!
//! # Qué es esto y de dónde sale
//!
//! El motor está **adaptado de `any-tts`** (MIT o Apache-2.0): lo que se tomó, lo que
//! no, y por qué no se depende del crate está en [`PROCEDENCIA.md`]. Lo decidió
//! el equipo el 2026-09-17 con dos datos medidos delante: su API no admite fonemas
//! propios —así que su fonemizador decidía la calidad del español— y convertía la
//! jota en /k/, de modo que "rojo" sonaba "roko".
//!
//! # Lo que este módulo **no** hace, y no es un olvido
//!
//! **No clona voces.** El original trae clonación a partir de una muestra de audio y
//! aquí no entra: el criterio, escrito en `model-reviews.md`
//! sección 2.1, lo prohíbe. Una herramienta para suplantar a alguien con tres
//! segundos de su voz no es algo que este producto necesite para leer un libro.
//!
//! **No convierte texto en fonemas.** Eso es [`super::fonemas`], que está escrito
//! aquí y con nuestra licencia.
//!
//! [`PROCEDENCIA.md`]: https://github.com/TM9657/any-tts

mod albert;
mod atencion;
mod capas_conv;
mod capas_lstm;
mod config;
mod decoder;
mod motor;
mod prosody;
mod text_encoder;

pub use config::KokoroConfig;
pub use motor::{Kokoro, Voz, MUESTREO};
