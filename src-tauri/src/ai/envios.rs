//! El registro local de lo que se envía a un proveedor de IA externo.
//!
//! Contrato: `docs/architecture/ai/privacy.md`, sección 2, regla 4 —"se registra
//! localmente qué se envió y cuándo, para que el usuario pueda auditarlo"— y
//! sección 3 —"el historial de IA es local, exportable y borrable"—.//! El porqué del esquema está en la migración V32.

use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// Un envío, tal como quedó registrado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AiSend")]
pub struct EnvioRegistrado {
    #[ts(type = "number")]
    pub id: i64,
    pub sent_at: String,
    /// `openai` o `anthropic`.
    pub protocol: String,
    pub host: String,
    pub model: String,
    pub capability: String,
    /// El mensaje entero, tal como salió y sin la credencial.
    pub body: String,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Nulo si no llegó a haber respuesta.
    pub status: Option<u16>,
}

/// Lo que se anota de un envío.
pub struct Anotacion<'a> {
    pub protocolo: &'a str,
    pub host: &'a str,
    pub modelo: &'a str,
    pub capacidad: super::Capability,
    pub cuerpo: &'a str,
    pub status: Option<u16>,
}

fn fallo(error: rusqlite::Error) -> NbError {
    tracing::error!(%error, "el registro de envíos de IA no respondió");
    NbError::StorageFailure
}

/// Anota un envío que **salió**.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn anotar(conn: &Connection, anotacion: &Anotacion<'_>) -> NbResult<()> {
    conn.execute(
        "INSERT INTO ai_sends (protocol, host, model, capability, body, bytes, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            anotacion.protocolo,
            anotacion.host,
            anotacion.modelo,
            anotacion.capacidad.as_str(),
            anotacion.cuerpo,
            i64::try_from(anotacion.cuerpo.len()).unwrap_or(i64::MAX),
            anotacion.status.map(i64::from),
        ],
    )
    .map(|_| ())
    .map_err(fallo)
}

/// Los envíos, de lo más reciente a lo más antiguo, hasta `limite`.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn listar(conn: &Connection, limite: usize) -> NbResult<Vec<EnvioRegistrado>> {
    let mut consulta = conn
        .prepare(
            "SELECT id, sent_at, protocol, host, model, capability, body, bytes, status
               FROM ai_sends ORDER BY id DESC LIMIT ?1",
        )
        .map_err(fallo)?;
    let limite = i64::try_from(limite).unwrap_or(i64::MAX);
    consulta
        .query_map([limite], |fila| {
            Ok(EnvioRegistrado {
                id: fila.get(0)?,
                sent_at: fila.get(1)?,
                protocol: fila.get(2)?,
                host: fila.get(3)?,
                model: fila.get(4)?,
                capability: fila.get(5)?,
                body: fila.get(6)?,
                bytes: fila.get::<_, i64>(7)?.unsigned_abs(),
                status: fila
                    .get::<_, Option<i64>>(8)?
                    .and_then(|s| u16::try_from(s).ok()),
            })
        })
        .and_then(Iterator::collect)
        .map_err(fallo)
}

/// Borra el registro entero. Devuelve cuántos envíos había.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn borrar(conn: &Connection) -> NbResult<usize> {
    conn.execute("DELETE FROM ai_sends", []).map_err(fallo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Capability;
    use crate::db::Database;

    /// Lo que se anota se lee igual, de lo más reciente a lo más antiguo, y
    /// borrar lo borra todo: es lo que `privacy.md` sección 3 promete.
    #[test]
    fn lo_enviado_se_lee_tal_cual_y_se_puede_borrar() {
        let db = Database::open_in_memory().expect("base");
        let conn = db.connection().expect("conexión");

        for (i, status) in [Some(200), None].into_iter().enumerate() {
            anotar(
                &conn,
                &Anotacion {
                    protocolo: "anthropic",
                    host: "api.anthropic.com",
                    modelo: "un-modelo",
                    capacidad: Capability::Summary,
                    cuerpo: &format!("{{\"n\":{i}}}"),
                    status,
                },
            )
            .expect("anotar");
        }

        let envios = listar(&conn, 10).expect("listar");
        assert_eq!(envios.len(), 2);
        assert_eq!(envios[0].body, "{\"n\":1}");
        assert_eq!(envios[0].status, None);
        assert_eq!(envios[1].status, Some(200));
        assert_eq!(envios[1].capability, "summary");
        assert_eq!(envios[1].bytes, 7);
        assert_eq!(listar(&conn, 1).expect("con límite").len(), 1);

        assert_eq!(borrar(&conn).expect("borrar"), 2);
        assert!(listar(&conn, 10).expect("vacío").is_empty());
    }
}
