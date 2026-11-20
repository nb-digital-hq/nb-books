//! Lo que el usuario marca como incorrecto de una salida generada.
//!
//! Contrato: `docs/architecture/ai/evaluation.md`, sección 6.
//! El porqué del esquema está en la migración V36.
//!
//! # Por qué esto no es una encuesta
//!
//! Porque lo que se necesita saber no es si a alguien le gustó, sino **qué falló y
//! con qué**: la plantilla, su versión, el proveedor y el modelo. Con eso, un fallo
//! que se repite en veinte reportes apunta a una plantilla concreta; con un pulgar
//! hacia abajo no apunta a nada.
//!
//! # Y por qué no lleva el libro dentro
//!
//! Porque el texto del documento **no entra** sin autorización explícita, y la
//! forma de que no entre por descuido es que no haya dónde ponerlo: el esquema
//! guarda la salida del modelo y el identificador de la edición, que fuera de este
//! equipo no dice de qué libro se trata. Exportar produce un archivo que el usuario
//! mira antes de decidir si lo manda a alguna parte; mandarlo es cosa suya, y hoy
//! no hay a dónde -no existe un servicio del proyecto que los reciba-, así que no
//! se ofrece un botón que sugiera que sí.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// Qué está mal. Lista cerrada: un campo libre no se puede contar, y contar es para
/// lo que está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "AiReportReason"
)]
pub enum Motivo {
    /// Dice algo que el libro no dice.
    Falso,
    /// Es cierto, pero no viene a cuento de lo que se estaba leyendo.
    NoVieneACuento,
    /// Se deja fuera lo importante.
    Incompleto,
    /// Está mal escrito, o en otro idioma del que se pidió.
    MalEscrito,
    Otro,
}

impl Motivo {
    /// Cómo se guarda. Es el valor que admite la restricción de V36.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Falso => "falso",
            Self::NoVieneACuento => "no_viene_a_cuento",
            Self::Incompleto => "incompleto",
            Self::MalEscrito => "mal_escrito",
            Self::Otro => "otro",
        }
    }
}

/// Lo que se marca. Lo arma la interfaz con lo que ya tiene de la generación.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "AiReportDraft"
)]
pub struct Reporte {
    pub function: String,
    pub template_id: String,
    pub template_version: u32,
    pub provider_id: String,
    pub provider_label: String,
    pub left_device: bool,
    /// La edición, sin título ni ruta. Puede faltar.
    pub edition_id: Option<String>,
    pub reason: Motivo,
    /// Lo que el usuario quiera añadir, si añade algo.
    pub note: Option<String>,
    /// La salida generada, que es del modelo y no del documento.
    pub output: String,
}

/// Un reporte guardado.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "AiReport")]
pub struct Registrado {
    #[ts(type = "number")]
    pub id: i64,
    pub created_at: String,
    pub function: String,
    pub template_id: String,
    pub template_version: u32,
    pub provider_id: String,
    pub provider_label: String,
    pub left_device: bool,
    pub edition_id: Option<String>,
    pub reason: String,
    pub note: String,
    pub output: String,
}

/// Cuántos caracteres de la salida se guardan.
///
/// Una salida de estudio ronda los dos mil; el tope existe para que un proveedor
/// que devuelva un disparate de un megabyte no llene la base del usuario por un
/// botón que pulsó una vez.
const MAXIMO_DE_SALIDA: usize = 8_000;

/// Y cuántos de la nota, por lo mismo.
const MAXIMO_DE_NOTA: usize = 1_000;

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(%error, "el registro de reportes de IA no respondió");
    NbError::StorageFailure
}

/// Anota un reporte. Devuelve su identificador.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn anotar(conn: &Connection, reporte: &Reporte) -> NbResult<i64> {
    let salida: String = reporte.output.chars().take(MAXIMO_DE_SALIDA).collect();
    let nota: String = reporte
        .note
        .as_deref()
        .unwrap_or_default()
        .chars()
        .take(MAXIMO_DE_NOTA)
        .collect();
    conn.execute(
        "INSERT INTO ai_reports
           (function, template_id, template_ver, provider_id, provider_label,
            left_device, edition_id, reason, note, output)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            reporte.function,
            reporte.template_id,
            reporte.template_version,
            reporte.provider_id,
            reporte.provider_label,
            i64::from(reporte.left_device),
            reporte.edition_id,
            reporte.reason.as_str(),
            nota,
            salida,
        ],
    )
    .map_err(|e| fallo(&e))?;
    Ok(conn.last_insert_rowid())
}

/// Los reportes, de lo más reciente a lo más antiguo, hasta `limite`.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn listar(conn: &Connection, limite: usize) -> NbResult<Vec<Registrado>> {
    let mut consulta = conn
        .prepare(
            "SELECT id, created_at, function, template_id, template_ver, provider_id,
                    provider_label, left_device, edition_id, reason, note, output
               FROM ai_reports ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| fallo(&e))?;
    let limite = i64::try_from(limite).unwrap_or(i64::MAX);
    consulta
        .query_map([limite], |fila| {
            Ok(Registrado {
                id: fila.get(0)?,
                created_at: fila.get(1)?,
                function: fila.get(2)?,
                template_id: fila.get(3)?,
                template_version: fila.get::<_, i64>(4)?.unsigned_abs() as u32,
                provider_id: fila.get(5)?,
                provider_label: fila.get(6)?,
                left_device: fila.get::<_, i64>(7)? != 0,
                edition_id: fila.get(8)?,
                reason: fila.get(9)?,
                note: fila.get(10)?,
                output: fila.get(11)?,
            })
        })
        .and_then(Iterator::collect)
        .map_err(|e| fallo(&e))
}

/// Borra uno. Devuelve si existía.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn olvidar(conn: &Connection, id: i64) -> NbResult<bool> {
    conn.execute("DELETE FROM ai_reports WHERE id = ?1", [id])
        .map(|filas| filas > 0)
        .map_err(|e| fallo(&e))
}

/// Borra el registro entero. Devuelve cuántos había.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn borrar(conn: &Connection) -> NbResult<usize> {
    conn.execute("DELETE FROM ai_reports", [])
        .map_err(|e| fallo(&e))
}

/// Los reportes en un texto que una persona puede leer antes de decidir si lo manda
/// a alguna parte.
///
/// **Esto es lo que sustituye a un botón de "enviar"**, y no por falta de ganas: no
/// existe un servicio del proyecto que los reciba, y ofrecer un envío que no va a
/// ninguna parte sería peor que no ofrecerlo. Cuando exista, el consentimiento se
/// pide como el de cualquier otra salida, por `net.rs`.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn exportar(conn: &Connection, limite: usize) -> NbResult<String> {
    let reportes = listar(conn, limite)?;
    serde_json::to_string_pretty(&reportes).map_err(|_| NbError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn base() -> Database {
        Database::open_in_memory().expect("base")
    }

    fn reporte() -> Reporte {
        Reporte {
            function: "flashcards".to_owned(),
            template_id: "estudio.tarjetas".to_owned(),
            template_version: 1,
            provider_id: "local".to_owned(),
            provider_label: "Modelo de aquí dentro".to_owned(),
            left_device: false,
            edition_id: Some("ed-1".to_owned()),
            reason: Motivo::Falso,
            note: Some("la fecha no es la del libro".to_owned()),
            output: "{\"tarjetas\":[]}".to_owned(),
        }
    }

    /// Lo marcado se guarda y se puede volver a leer, que es lo que convierte el
    /// botón en un registro.
    #[test]
    fn lo_marcado_se_guarda_y_se_lee() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let id = anotar(&conn, &reporte()).expect("anotar");
        assert!(id > 0);

        let lista = listar(&conn, 10).expect("listar");
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].reason, "falso");
        assert_eq!(lista[0].template_id, "estudio.tarjetas");
        assert_eq!(lista[0].note, "la fecha no es la del libro");
    }

    /// **El texto del documento no cabe en un reporte**, y no porque nadie lo
    /// ponga: porque el esquema no tiene dónde. Si alguien añadiera una columna
    /// para él, esta prueba se lo recordaría.
    #[test]
    fn un_reporte_no_tiene_donde_guardar_el_texto_del_documento() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let columnas: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('ai_reports')")
            .expect("pragma")
            .query_map([], |fila| fila.get(0))
            .expect("columnas")
            .filter_map(Result::ok)
            .collect();
        for prohibida in [
            "excerpt",
            "fragment",
            "fragmento",
            "source_text",
            "title",
            "path",
        ] {
            assert!(
                !columnas.iter().any(|c| c == prohibida),
                "el esquema tiene una columna para el texto del documento: {prohibida}"
            );
        }
    }

    /// Una salida enorme no llena la base por un botón que se pulsó una vez.
    #[test]
    fn una_salida_enorme_se_recorta_al_guardarla() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let mut grande = reporte();
        grande.output = "a".repeat(MAXIMO_DE_SALIDA * 3);
        anotar(&conn, &grande).expect("anotar");

        let lista = listar(&conn, 1).expect("listar");
        assert_eq!(lista[0].output.chars().count(), MAXIMO_DE_SALIDA);
    }

    /// Borrar deja el registro vacío: es historial del usuario, y se borra.
    #[test]
    fn borrar_los_reportes_no_deja_ninguno() {
        let db = base();
        let conn = db.connection().expect("conexión");
        anotar(&conn, &reporte()).expect("anotar");
        let id = anotar(&conn, &reporte()).expect("anotar");

        assert!(olvidar(&conn, id).expect("olvidar"));
        assert!(!olvidar(&conn, id).expect("olvidar dos veces"));
        assert_eq!(borrar(&conn).expect("borrar"), 1);
        assert!(listar(&conn, 10).expect("listar").is_empty());
    }

    /// Lo exportado es legible antes de decidir si se manda: si no se puede leer,
    /// no se puede decidir.
    #[test]
    fn lo_exportado_se_puede_leer_antes_de_mandarlo() {
        let db = base();
        let conn = db.connection().expect("conexión");
        anotar(&conn, &reporte()).expect("anotar");

        let texto = exportar(&conn, 10).expect("exportar");
        assert!(texto.contains("estudio.tarjetas"), "{texto}");
        assert!(texto.contains("falso"), "{texto}");
        let leido: serde_json::Value = serde_json::from_str(&texto).expect("es JSON");
        assert_eq!(leido.as_array().map(Vec::len), Some(1));
    }
}
