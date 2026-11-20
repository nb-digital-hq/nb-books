//! Proveedor simulado: el lector entero, sin ningún formato real.
//!
//! Es el entregable que hace verificable el contrato. Con él se puede
//! ejercitar la navegación, la resolución de locators con su cascada de
//! degradación, la extracción de texto y la negociación de capacidades **antes
//! de que exista un solo proveedor de verdad**, y sin depender de que un EPUB
//! concreto se comporte de una forma concreta.
//!
//! No es solo para pruebas. Permite construir la interfaz del
//! lector contra esto mientras el proveedor de PDF sigue pendiente de su motor
//! nativo, que es exactamente el desacoplamiento que el contrato existe para
//! dar. Por eso es público y no `#[cfg(test)]`.
//!
//! Implementa la cascada de ADR-004 de forma completa y observable: es el patrón
//! contra el que se escribirán los proveedores reales, y la referencia con la
//! que comparar su comportamiento.

use std::path::Path;

use super::provider::{DocumentProvider, ProviderFactory, ReaderCapabilities};
use super::secciones::{DocumentoDeSecciones, Seccion};
use crate::error::NbResult;

/// Tipo de medio con el que se registra.
pub const MEDIA_TYPE: &str = "application/x-nb-books-fake";

/// Un documento simulado con tres capítulos y todas las capacidades.
#[must_use]
pub fn con_tres_capitulos() -> DocumentoDeSecciones {
    DocumentoDeSecciones::new(
        MEDIA_TYPE,
        vec![
            Seccion {
                href: "c1.xhtml".to_owned(),
                title: "Primer capítulo".to_owned(),
                text: "Muchos años después, frente al pelotón de fusilamiento, \
                           el coronel Aureliano Buendía había de recordar aquella tarde remota."
                    .to_owned(),
            },
            Seccion {
                href: "c2.xhtml".to_owned(),
                title: "Segundo capítulo".to_owned(),
                text: "Macondo era entonces una aldea de veinte casas de barro y cañabrava \
                           construidas a la orilla de un río de aguas diáfanas."
                    .to_owned(),
            },
            Seccion {
                href: "c3.xhtml".to_owned(),
                title: "Tercer capítulo".to_owned(),
                text: "El mundo era tan reciente que muchas cosas carecían de nombre, \
                           y para mencionarlas había que señalarlas con el dedo."
                    .to_owned(),
            },
        ],
        ReaderCapabilities {
            text_selection: true,
            internal_search: true,
            reflow: true,
            typography: true,
            synchronized_audio: false,
            text_extraction: true,
        },
    )
}

/// Fábrica del proveedor simulado.
///
/// Ignora la ruta: el documento simulado no vive en el disco. Es lo que permite
/// ejercitar el lector sin ningún archivo.
pub struct FakeFactory;

impl ProviderFactory for FakeFactory {
    fn media_type(&self) -> &'static str {
        MEDIA_TYPE
    }

    fn open(&self, _path: &Path) -> NbResult<Box<dyn DocumentProvider>> {
        Ok(Box::new(con_tres_capitulos()))
    }
}
