//! Los tipos que comparten las dos implementaciones de la voz del sistema.
//!
//! Viven aquí y no en [`super::ssip`] porque ese módulo **no compila fuera de
//! Unix**: habla por un socket de dominio Unix. Donde no lo hay, la voz del
//! sistema la sirve [`super::sin_ssip`], que declara que no está disponible; los
//! dos exponen la misma interfaz, y estos tipos son la parte de esa interfaz que
//! no tiene nada de específico de ninguna plataforma.
//!
//! Separarlos evita lo único peor que un condicional: dos definiciones del mismo
//! tipo que hay que acordarse de cambiar a la vez.

/// Lo que el servidor cuenta por su cuenta sobre un mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// Empezó a decirse.
    Empezado(u64),
    /// Terminó de decirse entero.
    Terminado(u64),
    /// Se cortó antes de terminar, casi siempre porque alguien paró.
    Cortado(u64),
}

/// Una voz que el sintetizador ofrece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voz {
    pub nombre: String,
    pub idioma: String,
    pub variante: String,
}
