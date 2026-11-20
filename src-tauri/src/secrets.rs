//! El almacén de credenciales del sistema.
//!
//! # Por qué no es la base de datos
//!
//! Porque el esquema lo prohíbe y hay una prueba que lo hace cumplir. Una clave
//! de servicio no es un ajuste: los ajustes se exportan con la biblioteca
//!, se copian a otra máquina y se leen en claro con cualquier visor de
//! SQLite. Un secreto que viaja en la exportación es un secreto filtrado.
//!
//! Lo que hay aquí es la puerta al llavero del anfitrión, que es lo que
//! [authentication.md](../../docs/architecture/security/authentication.md)
//! sección 2 lleva especificado desde antes de que existiera este módulo:
//! Keychain en macOS, Credential Locker en Windows y Secret Service en el
//! resto. La librería elige cuál según la plataforma; aquí no se decide.
//!
//! # Qué pasa cuando no hay llavero
//!
//! Se dice y se desactiva la función. **No hay repliegue a texto plano**, que es
//! lo que `authentication.md` prohíbe expresamente. Un escritorio mínimo, una
//! sesión por SSH o un contenedor no tienen Secret Service, y ahí la respuesta
//! correcta es "esto no se puede usar aquí", no "se guarda peor sin avisar".
//!
//! # Por qué la credencial se ata al anfitrión
//!
//! Porque la cuenta que la nombra incluye a quién se le manda. Si quien lee
//! cambia el servicio de voz, la clave del anterior **no** viaja al nuevo: no
//! aparece, porque se busca bajo otra cuenta. Mandarle a un servicio la
//! credencial que era de otro es la clase de fallo que no se nota hasta que se
//! ha filtrado, y aquí no puede ocurrir por construcción.

use crate::error::{NbError, NbResult};

/// Servicio bajo el que se agrupan las credenciales en el llavero.
///
/// Es lo que la persona verá si abre su gestor de contraseñas, así que se
/// escribe para leerse ahí y no para leerse aquí.
const SERVICIO: &str = "NB Books";

/// Cuánto puede ocupar un secreto.
///
/// Ocho kilobytes. Una clave de API son decenas de caracteres y el token más
/// largo que se ha visto no llega a dos mil; el límite existe para que un
/// valor absurdo se rechace aquí y no en el llavero, cuyo mensaje de error no
/// controlamos.
const MAXIMO: usize = 8 * 1024;

/// La cuenta con la que se nombra la credencial de un servicio de voz.
///
/// El anfitrión va dentro a propósito: ver la nota de cabecera.
#[must_use]
pub fn cuenta_de_voz(host: &str) -> String {
    format!("voz@{}", host.trim().trim_end_matches('/'))
}

/// La cuenta con la que se nombra la credencial de un proveedor de IA externo
///.
///
/// Aparte de la de voz aunque el anfitrión fuera el mismo: son dos permisos
/// distintos que alguien concede por separado, y borrar la clave de uno no puede
/// dejar sin la suya al otro.
#[must_use]
pub fn cuenta_de_ia(host: &str) -> String {
    format!("ia@{}", host.trim().trim_end_matches('/'))
}

/// Si el sistema tiene un almacén de credenciales utilizable.
///
/// No crea ninguna entrada: solo pregunta por el estado de inicialización, que
/// es lo que permite a la interfaz decir "aquí no se puede guardar una clave"
/// antes de que nadie escriba una y la pierda.
#[must_use]
pub fn disponible() -> bool {
    keyring::v1::Entry::store_status().is_ok()
}

/// Guarda un secreto bajo una cuenta, sustituyendo el que hubiera.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la cuenta está vacía o el secreto pasa del
///   límite.
/// - [`NbError::ComponentUnavailable`] si el sistema no tiene llavero.
/// - [`NbError::StorageFailure`] si el llavero rechaza la escritura.
pub fn guardar(cuenta: &str, secreto: &str) -> NbResult<()> {
    let cuenta = validar(cuenta)?;
    if secreto.is_empty() || secreto.len() > MAXIMO {
        // El largo sí se registra; el contenido no, ni truncado: encryption.md.
        tracing::info!(
            largo = secreto.len(),
            "se rechazó un secreto fuera de límites"
        );
        return Err(NbError::PathNotAllowed);
    }

    entrada(cuenta)?.set_password(secreto).map_err(|error| {
        tracing::error!(error = %error, "el llavero no aceptó la credencial");
        NbError::StorageFailure
    })
}

/// Lee el secreto de una cuenta. `None` si no hay ninguno guardado.
///
/// Que no haya credencial **no es un error**: es el estado de fábrica de todo
/// servicio, y quien llama tiene que poder distinguirlo de un llavero roto.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la cuenta está vacía.
/// - [`NbError::ComponentUnavailable`] si el sistema no tiene llavero.
/// - [`NbError::StorageFailure`] si el llavero falla al leer.
pub fn leer(cuenta: &str) -> NbResult<Option<String>> {
    let cuenta = validar(cuenta)?;
    match entrada(cuenta)?.get_password() {
        Ok(secreto) => Ok(Some(secreto)),
        Err(keyring::v1::Error::NoEntry) => Ok(None),
        Err(error) => {
            tracing::error!(error = %error, "el llavero no devolvió la credencial");
            Err(NbError::StorageFailure)
        }
    }
}

/// Borra el secreto de una cuenta. No falla si no había ninguno.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la cuenta está vacía.
/// - [`NbError::ComponentUnavailable`] si el sistema no tiene llavero.
/// - [`NbError::StorageFailure`] si el llavero falla al borrar.
pub fn borrar(cuenta: &str) -> NbResult<()> {
    let cuenta = validar(cuenta)?;
    match entrada(cuenta)?.delete_credential() {
        Ok(()) | Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(error) => {
            tracing::error!(error = %error, "el llavero no borró la credencial");
            Err(NbError::StorageFailure)
        }
    }
}

/// La cuenta, comprobada.
fn validar(cuenta: &str) -> NbResult<&str> {
    let limpia = cuenta.trim();
    if limpia.is_empty() {
        tracing::info!("se pidió una credencial sin nombrar la cuenta");
        return Err(NbError::PathNotAllowed);
    }
    Ok(limpia)
}

/// La entrada del llavero, o el motivo por el que no la hay.
fn entrada(cuenta: &str) -> NbResult<keyring::v1::Entry> {
    keyring::v1::Entry::new(SERVICIO, cuenta).map_err(|error| {
        tracing::info!(error = %error, "el sistema no tiene almacén de credenciales");
        NbError::ComponentUnavailable
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cuenta_lleva_el_anfitrion_dentro() {
        assert_eq!(cuenta_de_voz("api.groq.com"), "voz@api.groq.com");
        assert_eq!(cuenta_de_voz(" api.groq.com/ "), "voz@api.groq.com");
    }

    /// Dos servicios distintos no comparten cuenta, y por eso no pueden
    /// compartir credencial. Es la propiedad de la nota de cabecera.
    #[test]
    fn dos_servicios_no_comparten_cuenta() {
        assert_ne!(
            cuenta_de_voz("api.groq.com"),
            cuenta_de_voz("api.cloudflare.com")
        );
    }

    #[test]
    fn una_cuenta_vacia_se_rechaza_antes_de_tocar_el_llavero() {
        assert_eq!(guardar("  ", "x"), Err(NbError::PathNotAllowed));
        assert_eq!(leer("").unwrap_err(), NbError::PathNotAllowed);
        assert_eq!(borrar("\t").unwrap_err(), NbError::PathNotAllowed);
    }

    #[test]
    fn un_secreto_desmesurado_se_rechaza() {
        let enorme = "a".repeat(MAXIMO + 1);
        assert_eq!(guardar("prueba", &enorme), Err(NbError::PathNotAllowed));
        assert_eq!(guardar("prueba", ""), Err(NbError::PathNotAllowed));
    }
}
