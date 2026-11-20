//! Lo que un paquete de personalización deja puesto, y cómo se va.
//!
//! # El criterio, y por qué necesita código
//!
//! *"Instalarlo y quitarlo no deja rastro en las preferencias del usuario."*
//! Un tema o un modo de lectura de una extensión **se elige**, y elegir algo se
//! guarda: si no, se olvidaría al cerrar la aplicación y habría que volver a
//! ponerlo cada vez. Lo que se guarda es el identificador de quien lo aporta,
//! así que al desinstalarla esa preferencia apuntaría a algo que ya no existe.
//!
//! Aquí se limpia. No es cortesía: una preferencia que señala a una extensión
//! que se fue es un rastro, y además deja al lector con un modo que no se puede
//! pintar. Se vacía en la misma operación que desinstala, para que no haya un
//! instante en el que el perfil y las preferencias digan cosas distintas.
//!
//! # Por qué no se restaura al reinstalar
//!
//! Porque volver a instalar algo no es volver a elegirlo. Reponer la
//! preferencia sola sería decidir por el usuario que quiere otra vez lo que
//! tenía, y eso no se sabe.

use rusqlite::Connection;

use crate::error::NbResult;
use crate::settings::{self, SettingKey, SettingValue};

/// Quita de las preferencias todo lo que apunte a esta extensión.
///
/// Devuelve cuántas preferencias se han tenido que vaciar, que es lo que
/// permite comprobar en una prueba que **no queda ninguna** en lugar de
/// suponerlo.
///
/// # Errors
///
/// [`NbError::StorageFailure`](crate::error::NbError::StorageFailure) si no se
/// puede leer o escribir la preferencia.
pub fn olvidar(conn: &Connection, extension: &str) -> NbResult<usize> {
    let mut limpiadas = 0;

    if let SettingValue::Text(puesto) = settings::get(conn, SettingKey::ExtensionTheme)? {
        if puesto == extension {
            settings::set(
                conn,
                SettingKey::ExtensionTheme,
                &SettingValue::Text(String::new()),
            )?;
            limpiadas += 1;
        }
    }

    // El modo de lectura guarda `<extensión>/<modo>`, así que se reconoce por
    // el prefijo. Se compara con la barra incluida: sin ella, desinstalar
    // `org.ejemplo.tema` se llevaría por delante el modo de
    // `org.ejemplo.temas-oscuros`.
    if let SettingValue::Text(puesto) = settings::get(conn, SettingKey::BookReadingMode)? {
        if puesto.starts_with(&format!("{extension}/")) {
            settings::set(
                conn,
                SettingKey::BookReadingMode,
                &SettingKey::BookReadingMode.default_value(),
            )?;
            limpiadas += 1;
        }
    }

    Ok(limpiadas)
}
