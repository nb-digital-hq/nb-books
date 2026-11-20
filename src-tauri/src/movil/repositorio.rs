//! Los teléfonos emparejados y lo que cada uno eligió recibir.
//!
//! Contrato: `docs/architecture/mobile-contract.md`, secciones 2 y 4.4.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::emparejamiento::{generar_token, huella_token};
use crate::db::{write_transaction, Database};
use crate::error::{NbError, NbResult};

/// Cuántos teléfonos puede tener emparejados una instalación a la vez.
///
/// No es una cuota: es una cota. Cada teléfono es una puerta abierta a la
/// biblioteca, y una lista sin límite deja que un error en otro programa de la
/// red llene la tabla de emparejamientos que nadie pidió.
pub const MAX_DISPOSITIVOS: usize = 16;

/// Un teléfono autenticado, tal como lo ve el servidor en cada petición.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispositivo {
    pub id: String,
    pub nombre: String,
}

/// Lo que se enseña en Ajustes, Teléfonos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "MobileDevice")]
pub struct Telefono {
    pub id: String,
    pub nombre: String,
    pub plataforma: String,
    pub emparejado_en: String,
    pub visto_en: Option<String>,
    /// Si en este momento controla el lector con Leer en el PC.
    pub controlando: bool,
}

/// Qué parte de la biblioteca recibe un teléfono. Las listas se combinan; `todo`
/// manda sobre ellas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seleccion {
    #[serde(default)]
    pub todo: bool,
    #[serde(default)]
    pub colecciones: Vec<String>,
    #[serde(default)]
    pub autores: Vec<String>,
    #[serde(default)]
    pub series: Vec<String>,
    #[serde(default)]
    pub materias: Vec<String>,
    #[serde(default)]
    pub ediciones: Vec<String>,
}

/// Cuántas claves admite una selección. Es lo que cabe en una petición sensata;
/// quien quiere más quiere `todo`.
const MAX_CLAVES: usize = 5_000;

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo de la base en el servidor móvil");
    NbError::StorageFailure
}

/// Empareja un teléfono nuevo y devuelve su identificador y su `device_token`.
/// Del token solo se guarda la huella.
///
/// # Errors
///
/// - [`NbError::StorageBudgetReached`] si ya hay [`MAX_DISPOSITIVOS`] emparejados.
/// - [`NbError::Internal`] si el nombre o la plataforma no son válidos.
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn emparejar(db: &Database, nombre: &str, plataforma: &str) -> NbResult<(String, String)> {
    let nombre: String = nombre.trim().chars().take(80).collect();
    if nombre.is_empty() || !matches!(plataforma, "android" | "ios") {
        return Err(NbError::Internal);
    }
    let token = generar_token()?;
    let mut conn = db.connection()?;
    let tx = write_transaction(&mut conn)?;
    let activos: i64 = tx
        .query_row(
            "SELECT count(*) FROM dispositivos WHERE revocado_en IS NULL",
            [],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))?;
    if usize::try_from(activos).unwrap_or(usize::MAX) >= MAX_DISPOSITIVOS {
        return Err(NbError::StorageBudgetReached);
    }
    let id: String = tx
        .query_row(
            "INSERT INTO dispositivos (nombre, plataforma, huella_del_token)
             VALUES (?1, ?2, ?3) RETURNING id",
            rusqlite::params![nombre, plataforma, huella_token(&token)],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))?;
    Ok((id, token))
}

/// El teléfono dueño de un token, si sigue emparejado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn por_token(db: &Database, token: &str) -> NbResult<Option<Dispositivo>> {
    let conn = db.connection()?;
    conn.query_row(
        "SELECT id, nombre FROM dispositivos
          WHERE huella_del_token = ?1 AND revocado_en IS NULL",
        [huella_token(token)],
        |f| {
            Ok(Dispositivo {
                id: f.get(0)?,
                nombre: f.get(1)?,
            })
        },
    )
    .optional()
    .map_err(|e| fallo(&e))
}

/// Apunta que el teléfono se ha visto ahora.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn tocar(db: &Database, id: &str) -> NbResult<()> {
    let mut conn = db.connection()?;
    let tx = write_transaction(&mut conn)?;
    tx.execute(
        "UPDATE dispositivos SET visto_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        [id],
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Los teléfonos emparejados, del más reciente al más antiguo.
///
/// Cota: [`MAX_DISPOSITIVOS`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn listar(db: &Database, controlando: Option<&str>) -> NbResult<Vec<Telefono>> {
    let conn = db.connection()?;
    let mut consulta = conn
        .prepare(
            "SELECT id, nombre, plataforma, emparejado_en, visto_en FROM dispositivos
              WHERE revocado_en IS NULL ORDER BY emparejado_en DESC LIMIT ?1",
        )
        .map_err(|e| fallo(&e))?;
    let filas = consulta
        .query_map([MAX_DISPOSITIVOS as i64], |f| {
            let id: String = f.get(0)?;
            Ok(Telefono {
                controlando: controlando == Some(id.as_str()),
                id,
                nombre: f.get(1)?,
                plataforma: f.get(2)?,
                emparejado_en: f.get(3)?,
                visto_en: f.get(4)?,
            })
        })
        .map_err(|e| fallo(&e))?;
    filas.collect::<Result<Vec<_>, _>>().map_err(|e| fallo(&e))
}

/// Revoca un teléfono. La fila se conserva; su token deja de valer.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn revocar(db: &Database, id: &str) -> NbResult<()> {
    let mut conn = db.connection()?;
    let tx = write_transaction(&mut conn)?;
    tx.execute(
        "UPDATE dispositivos SET revocado_en = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
          WHERE id = ?1 AND revocado_en IS NULL",
        [id],
    )
    .map_err(|e| fallo(&e))?;
    tx.execute(
        "DELETE FROM seleccion_de_dispositivos WHERE dispositivo_id = ?1",
        [id],
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))
}

/// La selección guardada de un teléfono.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn seleccion(conn: &Connection, id: &str) -> NbResult<Seleccion> {
    let mut consulta = conn
        .prepare("SELECT tipo, clave FROM seleccion_de_dispositivos WHERE dispositivo_id = ?1")
        .map_err(|e| fallo(&e))?;
    let filas = consulta
        .query_map([id], |f| {
            Ok((f.get::<_, String>(0)?, f.get::<_, String>(1)?))
        })
        .map_err(|e| fallo(&e))?;
    let mut s = Seleccion::default();
    for fila in filas {
        let (tipo, clave) = fila.map_err(|e| fallo(&e))?;
        match tipo.as_str() {
            "todo" => s.todo = true,
            "coleccion" => s.colecciones.push(clave),
            "autor" => s.autores.push(clave),
            "serie" => s.series.push(clave),
            "materia" => s.materias.push(clave),
            "edicion" => s.ediciones.push(clave),
            _ => return Err(NbError::StorageFailure),
        }
    }
    Ok(s)
}

/// Sustituye la selección de un teléfono y dice si **se amplió**: si entra algo
/// que antes no estaba, el teléfono tiene que volver a pedir el manifiesto desde
/// el principio para recibirlo.
///
/// # Errors
///
/// - [`NbError::Internal`] si la selección pasa de [`MAX_CLAVES`].
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn fijar_seleccion(db: &Database, id: &str, nueva: &Seleccion) -> NbResult<bool> {
    let claves = nueva.colecciones.len()
        + nueva.autores.len()
        + nueva.series.len()
        + nueva.materias.len()
        + nueva.ediciones.len();
    if claves > MAX_CLAVES {
        return Err(NbError::Internal);
    }
    let mut conn = db.connection()?;
    let tx = write_transaction(&mut conn)?;
    let anterior = seleccion(&tx, id)?;
    tx.execute(
        "DELETE FROM seleccion_de_dispositivos WHERE dispositivo_id = ?1",
        [id],
    )
    .map_err(|e| fallo(&e))?;
    let insertar = |tipo: &str, clave: &str| {
        tx.execute(
            "INSERT OR IGNORE INTO seleccion_de_dispositivos (dispositivo_id, tipo, clave)
             VALUES (?1, ?2, ?3)",
            [id, tipo, clave],
        )
        .map(|_| ())
        .map_err(|e| fallo(&e))
    };
    if nueva.todo {
        insertar("todo", "")?;
    } else {
        for (tipo, lista) in [
            ("coleccion", &nueva.colecciones),
            ("autor", &nueva.autores),
            ("serie", &nueva.series),
            ("materia", &nueva.materias),
            ("edicion", &nueva.ediciones),
        ] {
            for clave in lista.iter().filter(|c| !c.trim().is_empty()) {
                insertar(tipo, clave)?;
            }
        }
    }
    tx.commit().map_err(|e| fallo(&e))?;
    Ok(se_amplia(&anterior, nueva))
}

/// Si `nueva` deja entrar algo que `anterior` no dejaba.
fn se_amplia(anterior: &Seleccion, nueva: &Seleccion) -> bool {
    if anterior.todo {
        return false;
    }
    if nueva.todo {
        return true;
    }
    let falta = |viejas: &[String], nuevas: &[String]| nuevas.iter().any(|n| !viejas.contains(n));
    falta(&anterior.colecciones, &nueva.colecciones)
        || falta(&anterior.autores, &nueva.autores)
        || falta(&anterior.series, &nueva.series)
        || falta(&anterior.materias, &nueva.materias)
        || falta(&anterior.ediciones, &nueva.ediciones)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_token_identifica_al_telefono_hasta_que_se_revoca() {
        let db = Database::open_in_memory().expect("base");
        let (id, token) = emparejar(&db, "Teléfono", "android").expect("emparejar");
        assert_eq!(
            por_token(&db, &token).expect("consulta").map(|d| d.id),
            Some(id.clone())
        );
        assert_eq!(por_token(&db, "otro").expect("consulta"), None);
        revocar(&db, &id).expect("revocar");
        assert_eq!(por_token(&db, &token).expect("consulta"), None);
    }

    #[test]
    fn la_base_no_guarda_el_token_sino_su_huella() {
        let db = Database::open_in_memory().expect("base");
        let (_, token) = emparejar(&db, "Teléfono", "ios").expect("emparejar");
        let guardado: String = db
            .connection()
            .expect("conexión")
            .query_row("SELECT huella_del_token FROM dispositivos", [], |f| {
                f.get(0)
            })
            .expect("fila");
        assert_ne!(guardado, token);
        assert_eq!(guardado, huella_token(&token));
    }

    #[test]
    fn no_se_empareja_por_encima_de_la_cota() {
        let db = Database::open_in_memory().expect("base");
        for i in 0..MAX_DISPOSITIVOS {
            emparejar(&db, &format!("t{i}"), "android").expect("cabe");
        }
        assert_eq!(
            emparejar(&db, "uno más", "android"),
            Err(NbError::StorageBudgetReached)
        );
    }

    #[test]
    fn ampliar_la_seleccion_se_distingue_de_reducirla() {
        let db = Database::open_in_memory().expect("base");
        let (id, _) = emparejar(&db, "Teléfono", "android").expect("emparejar");
        let una = Seleccion {
            autores: vec!["Autora".to_owned()],
            ..Seleccion::default()
        };
        assert!(
            fijar_seleccion(&db, &id, &una).expect("fijar"),
            "de nada a algo amplía"
        );
        assert!(
            !fijar_seleccion(&db, &id, &una).expect("fijar"),
            "lo mismo no amplía"
        );
        assert!(
            !fijar_seleccion(&db, &id, &Seleccion::default()).expect("fijar"),
            "quitar no amplía"
        );
        let todo = Seleccion {
            todo: true,
            ..Seleccion::default()
        };
        assert!(fijar_seleccion(&db, &id, &todo).expect("fijar"));
        let leida = seleccion(&db.connection().expect("conexión"), &id).expect("leer");
        assert!(leida.todo);
    }
}
