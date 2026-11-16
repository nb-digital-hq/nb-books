//! El lector: posición, contrato de formato y proveedores.
//!
//! El principio que gobierna esta subfase es "diseñar para el objetivo, no para
//! el formato": el lector no sabe qué es un PDF ni qué es un EPUB. Sabe pedirle
//! a un proveedor que resuelva un [`Locator`], que le dé texto de un rango y que
//! declare qué es capaz de hacer. Todo lo demás lo pone el proveedor.
//!
//! Contrato: `docs/architecture/reader-contracts.md`.

pub mod comic;
pub mod epub;
pub mod fake;
pub mod locator;
pub mod pdf;
pub mod progress;
pub mod provider;
pub mod sanitize;
pub mod secciones;
pub mod segmentos;
pub mod session;

pub use locator::{Locations, Locator, TextContext, LOCATOR_VERSION};
pub use session::{ReadingSession, SessionInfo, SessionStore};

pub use provider::{
    DocumentProvider, FitMode, NativePosition, Navigation, NavigationEntry, PageSize, Precision,
    ProviderFactory, ProviderRegistry, ReaderCapabilities, Rendered, Resolution, SynchronizedAudio,
    TextLayer, TextLayerStatus, TextSpan,
};
