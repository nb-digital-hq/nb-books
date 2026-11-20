//! Modo de recuperación.
//!
//! # El defecto que este módulo existe para cerrar
//!
//! Hasta ahora, una base de datos corrupta hacía que el arranque devolviera un
//! error, y `run()` lo convertía en un `expect`. El proceso moría **antes de
//! abrir ninguna ventana**: sin mensaje, sin diálogo y sin nada que explicara
//! qué había pasado. Para quien lo sufre eso es la aplicación que "no abre", y es
//! literalmente la pantalla en blanco que el criterio prohíbe.
//!
//! # Qué hace en su lugar
//!
//! 1. **Aísla** el archivo dañado con marca de tiempo, junto a sus archivos de
//!    diario. No lo borra: puede ser la única copia de una biblioteca de años, y
//!    puede que una herramienta de recuperación de SQLite saque algo de ella.
//! 2. **Crea una base nueva** para que la aplicación arranque.
//! 3. **Cuenta lo que ha pasado y dónde ha quedado la copia**, para que la
//!    interfaz lo enseñe y el usuario pueda decidir.
//!
//! # Por qué se arranca en lugar de negarse
//!
//! Porque negarse no protege nada. El archivo dañado se conserva igual, y lo
//! único que se consigue es que quien tiene el problema no pueda ni ver un
//! mensaje. Arrancar vacío y decir la verdad —"tu biblioteca no se pudo abrir,
//! está aquí, y esto que ves está vacío"— deja al usuario en condiciones de
//! hacer algo; negarse a arrancar, no.
//!
//! # Lo que no hace, a propósito
//!
//! **No intenta reparar.** Ejecutar una reparación automática sobre el único
//! archivo que le queda al usuario es cómo se convierte un problema recuperable
//! en uno definitivo. Se aísla y se deja quieto.
//!
//! Contrato: `docs/developer/testing/chaos.md`, escenario 3.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::db::Database;
use crate::error::{NbError, NbResult};

/// Sufijos de los archivos que SQLite crea junto a la base.
///
/// Se mueven **con** ella: un diario de escritura anticipada que se queda al
/// lado de una base nueva es un archivo huérfano que puede confundir a SQLite y
/// que, sobre todo, todavía contiene datos del usuario.
const ACOMPANANTES: &[&str] = &["-wal", "-shm", "-journal"];

/// Qué pasó al abrir la biblioteca.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct RecoveryReport {
    /// Código canónico del fallo original. La interfaz lo traduce.
    pub reason_code: String,
    /// Dónde quedó el archivo aislado, **solo el nombre**.
    ///
    /// No la ruta completa: el usuario sabe dónde está su carpeta de datos, y la
    /// ruta lleva su nombre de cuenta del sistema, que es lo que la política de
    /// registro no deja salir. La interfaz enseña el nombre y ofrece abrir la
    /// carpeta.
    pub isolated_as: String,
}

/// Nombre con el que se aparta un archivo dañado.
///
/// Lleva la fecha porque puede pasar más de una vez, y dos archivos con el mismo
/// nombre significan que el segundo se comió al primero.
fn nombre_aislado(original: &Path, momento: &str) -> PathBuf {
    let base = original
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("nb-books.sqlite");
    original.with_file_name(format!("{base}.corrupted-{momento}"))
}

fn ahora() -> String {
    // Formato ordenable y sin caracteres que ningún sistema de archivos acepte
    // mal. Se construye a mano para no traer una dependencia de fechas por esto.
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    format!("{segundos}")
}

/// Aparta el archivo dañado y sus acompañantes.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede mover. Si esto falla no queda nada
/// que hacer sin arriesgar los datos, así que se propaga.
fn aislar(original: &Path) -> NbResult<PathBuf> {
    let destino = nombre_aislado(original, &ahora());

    std::fs::rename(original, &destino).map_err(|error| {
        tracing::error!(error = %error, "no se pudo apartar la base dañada");
        NbError::StorageFailure
    })?;

    for sufijo in ACOMPANANTES {
        let companero = PathBuf::from(format!("{}{sufijo}", original.display()));
        if companero.exists() {
            let suyo = PathBuf::from(format!("{}{sufijo}", destino.display()));
            // Un acompañante que no se puede mover no impide arrancar: lo
            // importante ya se apartó, y dejarlo ahí es preferible a no abrir.
            if let Err(error) = std::fs::rename(&companero, &suyo) {
                tracing::warn!(error = %error, sufijo, "no se pudo apartar un archivo de diario");
            }
        }
    }

    Ok(destino)
}

/// Comprueba si el archivo está inequívocamente dañado o mal formado.
///
/// Si el archivo existe pero no empieza por la cabecera mágica de SQLite
/// (`SQLite format 3\0`), o si SQLite reporta corrupción estructural
/// (`SQLITE_CORRUPT` o `SQLITE_NOTADB`), está dañado.
///
/// Si la base es legible o si el fallo fue por contención (`database is locked`,
/// `SQLITE_BUSY`, `SQLITE_LOCKED`), permisos o espacio en disco, **no** está
/// dañada y apartarla sería despojar al usuario de su biblioteca real.
fn esta_danada(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    // 1. Cabecera mágica de SQLite (16 bytes).
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut cabecera = [0u8; 16];
    use std::io::Read as _;
    if f.read_exact(&mut cabecera).is_err() || &cabecera != b"SQLite format 3\0" {
        return true;
    }
    drop(f);

    // 2. Si la cabecera es válida, comprobamos si SQLite reporta corrupción al
    // abrir o consultar el esquema en modo de solo lectura.
    let conexion = match rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(err) => {
            return matches!(
                err,
                rusqlite::Error::SqliteFailure(ffi_err, _)
                    if ffi_err.code == rusqlite::ffi::ErrorCode::DatabaseCorrupt
                        || ffi_err.code == rusqlite::ffi::ErrorCode::NotADatabase
            );
        }
    };

    let _ = conexion.busy_timeout(std::time::Duration::from_millis(500));
    match conexion.query_row("PRAGMA schema_version;", [], |row| row.get::<_, i64>(0)) {
        Ok(_) => false,
        Err(err) => matches!(
            err,
            rusqlite::Error::SqliteFailure(ffi_err, _)
                if ffi_err.code == rusqlite::ffi::ErrorCode::DatabaseCorrupt
                    || ffi_err.code == rusqlite::ffi::ErrorCode::NotADatabase
        ),
    }
}

/// Abre la biblioteca, y si no se puede, **entra en modo de recuperación**.
///
/// Devuelve la base y, cuando hubo que recuperar, el informe de lo que pasó.
/// Ese informe es lo que la interfaz enseña: sin él, el usuario vería su
/// biblioteca vacía sin ninguna explicación, que es la pérdida silenciosa que el
/// criterio prohíbe tanto como la pantalla en blanco.
///
/// # Errors
///
/// Propaga el error solo cuando **tampoco** se puede crear una base nueva, o
/// cuando el fallo original no es de los que se arreglan empezando de cero:
/// [`NbError::SchemaMismatch`] significa que la base es de una versión más
/// nueva, y apartarla sería castigar a quien simplemente abrió la aplicación
/// antigua por error.
pub fn open_or_recover(path: &Path) -> NbResult<(Database, Option<RecoveryReport>)> {
    let error = match Database::open(path) {
        Ok(db) => return Ok((db, None)),
        Err(error) => error,
    };

    if error == NbError::SchemaMismatch {
        // No se toca. La biblioteca está bien; la aplicación es la vieja.
        tracing::error!("la base es de una versión más nueva que esta aplicación");
        return Err(error);
    }

    if !esta_danada(path) {
        // La biblioteca no se pudo abrir, pero el archivo no está corrupto
        // (por ejemplo: contención por bloqueo, concurrencia de instancias o
        // permisos). Apartarla despojaría silenciosamente al usuario de sus datos.
        tracing::error!(
            code = error.code(),
            "no se pudo abrir la biblioteca, pero el archivo no está corrupto; se rechaza apartarlo"
        );
        return Err(error);
    }

    tracing::error!(
        code = error.code(),
        "no se pudo abrir la biblioteca; se entra en modo de recuperación"
    );

    let aislado = aislar(path)?;
    let db = Database::open(path)?;

    let isolated_as = aislado
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("nb-books.sqlite.corrupted")
        .to_owned();

    tracing::warn!(
        // no es del usuario: el nombre lo pone la aplicación al apartar su base.
        archivo = %isolated_as,
        "biblioteca apartada; se arranca con una base nueva"
    );

    Ok((
        db,
        Some(RecoveryReport {
            reason_code: error.code().to_owned(),
            isolated_as,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_aislado_lleva_marca_de_tiempo() {
        // Sin marca, dos corrupciones seguidas dejarían un solo archivo: la
        // segunda se comería la primera, que es justo la copia más antigua y
        // probablemente la más completa.
        let apartado = nombre_aislado(Path::new("/datos/nb-books.sqlite"), "1700000000");
        assert_eq!(
            apartado.file_name().and_then(|n| n.to_str()),
            Some("nb-books.sqlite.corrupted-1700000000")
        );
    }

    /// Los diarios de SQLite se van **con** la base apartada.
    ///
    /// Un diario que se queda al lado de una base nueva es un archivo huérfano
    /// que puede confundir a SQLite y que, sobre todo, todavía tiene datos del
    /// usuario dentro.
    ///
    /// Se comprueba aquí y no desde fuera porque desde fuera no es determinista:
    /// al fallar la apertura, SQLite puede haber limpiado ya sus propios diarios,
    /// y entonces la prueba pasaría sin haber comprobado nada.
    #[test]
    fn los_diarios_se_van_con_la_base_apartada() {
        let dir =
            crate::paths::temporal_de_pruebas().join(format!("nb-aislar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");

        let base = dir.join("nb-books.sqlite");
        std::fs::write(&base, b"no importa que sea legible").expect("base");
        std::fs::write(dir.join("nb-books.sqlite-wal"), b"datos del usuario").expect("diario");
        std::fs::write(dir.join("nb-books.sqlite-shm"), b"memoria").expect("memoria");

        let apartada = aislar(&base).expect("se aparta");
        let nombre = apartada
            .file_name()
            .and_then(|n| n.to_str())
            .expect("nombre");

        assert!(apartada.exists(), "la base");
        assert!(
            dir.join(format!("{nombre}-wal")).exists(),
            "el diario se va con ella"
        );
        assert!(
            dir.join(format!("{nombre}-shm")).exists(),
            "y la memoria compartida también"
        );
        assert!(
            !dir.join("nb-books.sqlite-wal").exists(),
            "y no se queda ninguno junto a donde irá la base nueva"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_informe_no_lleva_la_ruta_completa() {
        // La ruta lleva el nombre de cuenta del sistema. El usuario ya sabe
        // dónde está su carpeta de datos.
        let informe = RecoveryReport {
            reason_code: "STORAGE_FAILURE".to_owned(),
            isolated_as: "nb-books.sqlite.corrupted-1700000000".to_owned(),
        };
        assert!(!informe.isolated_as.contains('/'));
        assert!(!informe.isolated_as.contains('\\'));
    }
}
