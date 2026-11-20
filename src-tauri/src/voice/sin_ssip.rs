//! La voz del sistema **donde no existe `speech-dispatcher`**.
//!
//! # Por qué existe este archivo
//!
//! Porque el otro no compila fuera de Unix. `ssip.rs` habla por un socket de
//! dominio Unix —`std::os::unix::net::UnixStream`—, que en Windows no existe:
//! no es que la voz no funcionara allí, es que **el binario entero no se podía
//! construir**. Se descubrió el 2026-09-02 auditando qué hay atado a una sola
//! plataforma, y llevaba sin verse porque la única máquina donde se compila es
//! Linux y el CI que construye para Windows y macOS **nunca ha llegado a
//! ejecutarse**.
//!
//! # Por qué un módulo hermano y no condicionales repartidos
//!
//! Porque `Ssip` está entretejido en `voice/mod.rs`: lo guarda el narrador, lo
//! devuelven varias funciones y aparece en siete llamadas. Sembrar
//! `#[cfg(unix)]` por todas ellas duplicaría la lógica de narración —que no
//! tiene nada de específico de ninguna plataforma— y cada arreglo futuro habría
//! que hacerlo dos veces.
//!
//! Con la misma interfaz detrás del mismo nombre, `voice/mod.rs` **no cambia ni
//! una línea** y el comportamiento fuera de Unix es el que el proyecto exige:
//! la ausencia se declara. `conectar` falla con `ComponentUnavailable`, el
//! estado de la voz dice `sin-sintetizador` y la interfaz lo enseña como lo que
//! es, un componente que no está.
//!
//! # Lo que esto no es
//!
//! No es la voz de Windows ni la de macOS. Ambas tienen síntesis propia —SAPI y
//! `AVSpeechSynthesizer`— y ponerlas detrás de este mismo contrato es trabajo
//! de una función que no está en este binario, no de un parche. Lo que este
//! archivo arregla es que el producto **se pueda construir** en las tres
//! plataformas y que en dos de ellas diga la verdad sobre lo que le falta.

use std::sync::mpsc::{channel, Receiver};

use crate::error::{NbError, NbResult};

pub use super::tipos::{Evento, Voz};

/// Conexión con un servidor de síntesis que en esta plataforma no hay.
pub struct Ssip;

impl Ssip {
    /// Siempre falla, y es la única función de este archivo que se ejecuta.
    ///
    /// Las demás existen para que el tipo cumpla la interfaz; como nadie
    /// consigue un `Ssip` sin pasar por aquí, no hay ninguna forma de llamarlas.
    ///
    /// # Errors
    ///
    /// Siempre [`NbError::ComponentUnavailable`].
    pub fn conectar(_nombre_del_cliente: &str) -> NbResult<(Self, Receiver<Evento>)> {
        tracing::info!("esta plataforma no tiene speech-dispatcher: no hay voz del sistema");
        Err(NbError::ComponentUnavailable)
    }

    /// # Errors
    ///
    /// Siempre [`NbError::ComponentUnavailable`]. Ver [`Ssip::conectar`].
    pub fn poner_velocidad(&self, _valor: i32) -> NbResult<()> {
        Err(NbError::ComponentUnavailable)
    }

    /// # Errors
    ///
    /// Siempre [`NbError::ComponentUnavailable`]. Ver [`Ssip::conectar`].
    pub fn poner_voz(&self, _nombre: &str) -> NbResult<()> {
        Err(NbError::ComponentUnavailable)
    }

    /// # Errors
    ///
    /// Siempre [`NbError::ComponentUnavailable`]. Ver [`Ssip::conectar`].
    pub fn hablar(&self, _texto: &str) -> NbResult<u64> {
        Err(NbError::ComponentUnavailable)
    }

    /// # Errors
    ///
    /// Siempre [`NbError::ComponentUnavailable`]. Ver [`Ssip::conectar`].
    pub fn callar(&self) -> NbResult<()> {
        Err(NbError::ComponentUnavailable)
    }

    /// # Errors
    ///
    /// Siempre [`NbError::ComponentUnavailable`]. Ver [`Ssip::conectar`].
    pub fn voces(&self) -> NbResult<Vec<Voz>> {
        Err(NbError::ComponentUnavailable)
    }
}

/// Existe para que el canal de eventos tenga el mismo tipo que en Unix.
///
/// No lo llama nadie: sin conexión no hay eventos que recibir.
#[allow(dead_code)]
fn canal_vacio() -> Receiver<Evento> {
    let (_emisor, receptor) = channel();
    receptor
}
