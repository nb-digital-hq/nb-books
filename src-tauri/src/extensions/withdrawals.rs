//! Retirada de una extensión, y aviso a quien ya la tenía.
//!
//! # El problema que resuelve
//!
//! Quitar una extensión maliciosa del catálogo no le hace nada a quien ya la
//! tiene instalada: la sigue ejecutando. La política del registro lo dice desde
//! su primera versión —"retirarla del catálogo sin avisar a quien ya la tiene
//! no resuelve el problema"— y esto es su mecanismo.
//!
//! # Qué sale de este equipo al comprobar: nada
//!
//! Se descarga la **lista entera** y se compara aquí. Preguntar por las
//! extensiones que hay instaladas sería más barato en bytes y le contaría al
//! registro qué tiene cada usuario, que es exactamente lo que no hace falta
//! enviar para que esto funcione. La lista de retiradas es corta por
//! naturaleza: si algún día no lo fuera, el problema sería otro.
//!
//! # Apagar, y solo cuando toca
//!
//! Una retirada por maliciosa **apaga la extensión**. Las otras dos causas de
//! la política —incumplir sin corregir, y quedarse abandonada e incompatible—
//! avisan y no apagan: apagar por todo enseñaría a reactivar sin leer, y
//! entonces el apagado dejaría de significar algo el día que importe.
//!
//! Apagar no es desinstalar. La extensión se queda, con sus permisos y sus
//! datos, y el interruptor sigue donde estaba: quien la quiera de vuelta puede
//! encenderla, y quien no, desinstalarla. Decidir por el usuario que se borra
//! sería perder algo suyo sin preguntar.
//!
//! # Por qué el aviso se guarda
//!
//! Porque tiene que sobrevivir a cerrar la aplicación y a quedarse sin red. Sin
//! guardarlo, una extensión apagada por maliciosa aparecería, al volver a
//! abrir, apagada y sin explicación.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::net::{Network, Origin, Purpose};

use super::store::{Installed, Store};

/// La ruta que sirve el registro, colgando de su anfitrión.
pub const RUTA: &str = "retiradas.json";

/// El comodín que significa "todas las versiones".
const TODAS: &str = "*";

/// Por qué se retiró.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum WithdrawalReason {
    /// Resultó maliciosa. Es la única que apaga.
    Malicious,
    /// Incumple la política y su autor no corrigió.
    Breach,
    /// Abandonada e incompatible.
    Abandoned,
}

impl WithdrawalReason {
    /// Si enterarse obliga a apagarla.
    #[must_use]
    pub const fn apaga(self) -> bool {
        matches!(self, Self::Malicious)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Malicious => "malicious",
            Self::Breach => "breach",
            Self::Abandoned => "abandoned",
        }
    }

    fn de_str(texto: &str) -> Self {
        match texto {
            "malicious" => Self::Malicious,
            "abandoned" => Self::Abandoned,
            // Lo que no se reconoce se trata como incumplimiento: avisa y no
            // apaga. Un motivo desconocido no puede apagar nada, porque una
            // lista de un registro futuro con motivos nuevos apagaría media
            // biblioteca de extensiones sin que nadie lo hubiera decidido.
            _ => Self::Breach,
        }
    }
}

/// Una entrada de la lista que publica el registro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retirada {
    pub id: String,
    /// Qué versiones afecta. `["*"]` son todas.
    ///
    /// Versiones exactas y no rangos, a propósito: un rango obliga a comparar
    /// SemVer y a decidir qué hace un rango mal escrito, y el registro sabe
    /// exactamente qué versiones publicó. Lo que no se puede escribir mal no
    /// hay que interpretarlo.
    #[serde(default)]
    pub versions: Vec<String>,
    pub reason: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub date: String,
}

impl Retirada {
    fn alcanza(&self, version: &str) -> bool {
        self.versions.is_empty()
            || self
                .versions
                .iter()
                .any(|v| v == TODAS || v.as_str() == version)
    }
}

/// La lista completa, tal y como la sirve el registro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lista {
    /// Cuándo la generó el registro. Se guarda para poder decir de cuándo es lo
    /// que se está enseñando.
    #[serde(default)]
    pub generated: String,
    #[serde(default)]
    pub withdrawn: Vec<Retirada>,
}

/// Un aviso sobre algo que está instalado aquí.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
#[serde(rename_all = "camelCase")]
pub struct Withdrawal {
    pub extension: String,
    /// La versión instalada, no la retirada: es la que tiene delante quien lee.
    pub version: String,
    pub reason: WithdrawalReason,
    /// Lo que cuenta el registro. Puede venir vacío.
    pub detail: String,
    /// La fecha que declara el registro.
    pub date: String,
    /// Si la aplicación la apagó al enterarse.
    pub disabled: bool,
}

/// Cuáles de las instaladas afecta esta lista.
///
/// Función aparte y sin base de datos ni red porque es donde vive la decisión,
/// y es lo que hay que poder probar con una tabla de casos en lugar de con un
/// servidor.
#[must_use]
pub fn afectadas(lista: &Lista, instaladas: &[Installed]) -> Vec<Withdrawal> {
    let mut avisos = Vec::new();
    for instalada in instaladas {
        let Some(retirada) = lista
            .withdrawn
            .iter()
            .find(|r| r.id == instalada.id && r.alcanza(&instalada.version))
        else {
            continue;
        };
        let reason = WithdrawalReason::de_str(&retirada.reason);
        avisos.push(Withdrawal {
            extension: instalada.id.clone(),
            version: instalada.version.clone(),
            reason,
            detail: retirada.detail.clone(),
            date: retirada.date.clone(),
            disabled: reason.apaga(),
        });
    }
    avisos
}

/// Guarda los avisos y apaga lo que haya que apagar.
///
/// Reemplaza lo anterior entero: una extensión que el registro deja de tener
/// retirada desaparece de aquí sola.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn aplicar(conn: &Connection, almacen: &Store, avisos: &[Withdrawal]) -> NbResult<()> {
    conn.execute("DELETE FROM extension_withdrawals", [])
        .map_err(|_| NbError::StorageFailure)?;

    for aviso in avisos {
        conn.execute(
            "INSERT INTO extension_withdrawals \
             (extension, version, reason, detail, withdrawn_at, disabled) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                aviso.extension,
                aviso.version,
                aviso.reason.as_str(),
                aviso.detail,
                aviso.date,
                i32::from(aviso.disabled),
            ],
        )
        .map_err(|_| NbError::StorageFailure)?;

        if aviso.disabled {
            almacen.habilitar(&aviso.extension, false)?;
        }
    }
    Ok(())
}

/// Los avisos guardados, para poder enseñarlos sin salir a la red.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn guardados(conn: &Connection) -> NbResult<Vec<Withdrawal>> {
    let mut consulta = conn
        .prepare(
            "SELECT extension, version, reason, detail, withdrawn_at, disabled \
             FROM extension_withdrawals ORDER BY extension",
        )
        .map_err(|_| NbError::StorageFailure)?;

    let filas = consulta
        .query_map([], |fila| {
            Ok(Withdrawal {
                extension: fila.get(0)?,
                version: fila.get(1)?,
                reason: WithdrawalReason::de_str(&fila.get::<_, String>(2)?),
                detail: fila.get(3)?,
                date: fila.get(4)?,
                disabled: fila.get::<_, i32>(5)? != 0,
            })
        })
        .map_err(|_| NbError::StorageFailure)?;

    filas
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| NbError::StorageFailure)
}

/// Pregunta al registro y aplica lo que diga.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si no hay registro configurado. No es un
///   fallo: es el estado de hoy, y decirlo es mejor que fingir una comprobación.
/// - Lo que devuelva la puerta de red: sin consentimiento, sin red o con el
///   interruptor general puesto.
/// - [`NbError::DocumentCorrupted`] si lo que responde no es la lista.
pub fn comprobar(
    conn: &Connection,
    red: &Network,
    almacen: &Store,
    anfitrion: &str,
) -> NbResult<Vec<Withdrawal>> {
    let anfitrion = anfitrion.trim();
    if anfitrion.is_empty() {
        return Err(NbError::ComponentUnavailable);
    }
    let url = if anfitrion.contains("://") {
        format!("{}/{RUTA}", anfitrion.trim_end_matches('/'))
    } else {
        format!("https://{anfitrion}/{RUTA}")
    };

    let respuesta = red.get_from(conn, &Origin::App, &url, Purpose::Registry)?;
    if respuesta.status != 200 {
        tracing::info!(status = respuesta.status, "el registro no respondió bien");
        return Err(NbError::NetworkFailure);
    }
    let lista: Lista =
        serde_json::from_slice(&respuesta.body).map_err(|_| NbError::DocumentCorrupted)?;

    let avisos = afectadas(&lista, &almacen.perfil()?.extensions);
    aplicar(conn, almacen, &avisos)?;
    Ok(avisos)
}
