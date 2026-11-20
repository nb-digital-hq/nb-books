//! Lo que sube un teléfono: lo que la persona hizo al leer en él.
//!
//! Contrato: `docs/architecture/mobile-contract.md`, sección 4.6, y ADR-016.
//!
//! # Nada pisa nada
//!
//! Cada tipo de dato tiene un dueño y un sentido. Lo que llega del teléfono son
//! hechos que se suman (una anotación, una sesión, un repaso) o la posición del
//! propio teléfono, que vive en su tabla y no toca la de este equipo. La única
//! escritura sobre algo del PC es el cambio de una nota, y solo entra si el
//! teléfono vio la última revisión; si no, entra como nota nueva.
//!
//! Todo es idempotente por identificador: el teléfono reintenta lo que no sabe
//! si llegó, y recibir dos veces lo mismo no cambia nada.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::manifiesto::SELECCIONADAS;
use crate::db::{write_transaction, Database};
use crate::error::{NbError, NbResult};
use crate::reader::Locator;

/// Cota de elementos por envío: el teléfono trocea, y una petición enorme no
/// puede retener la base con una transacción de minutos.
pub const MAX_POR_ENVIO: usize = 2_000;

#[derive(Debug, Default, Deserialize)]
pub struct Envio {
    #[serde(default)]
    pub anotaciones: Vec<Anotacion>,
    #[serde(default)]
    pub posiciones: Vec<Posicion>,
    #[serde(default)]
    pub sesiones: Vec<Sesion>,
    #[serde(default)]
    pub vocabulario: Vec<Palabra>,
    #[serde(default)]
    pub repasos: Vec<Repaso>,
    #[serde(default)]
    pub notas: Vec<Nota>,
}

#[derive(Debug, Deserialize)]
pub struct Anotacion {
    pub id: String,
    pub edicion: String,
    pub tipo: String,
    pub texto: Option<String>,
    pub locator: Value,
    pub trazo: Option<Value>,
    pub creada_en: String,
    pub baja_en: Option<String>,
    pub sustituye_a: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Posicion {
    pub edicion: String,
    pub locator: Value,
    pub actualizado_en: String,
}

#[derive(Debug, Deserialize)]
pub struct Sesion {
    pub id: String,
    pub edicion: String,
    pub inicio: String,
    pub fin: Option<String>,
    pub desde: Value,
    pub hasta: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Palabra {
    pub id: String,
    pub edicion: String,
    pub lema: String,
    pub termino: String,
    pub proveedor: String,
    pub locator: Value,
    pub creada_en: String,
}

#[derive(Debug, Deserialize)]
pub struct Repaso {
    pub id: String,
    pub tarjeta: String,
    pub calificacion: String,
    pub repasado_en: String,
}

#[derive(Debug, Deserialize)]
pub struct Nota {
    pub id: String,
    pub titulo: String,
    pub cuerpo: String,
    /// La revisión que el teléfono tenía al editarla. Nula en una nota nueva.
    pub revision_vista: Option<i64>,
}

/// Qué pasó con cada cosa que no entró tal cual.
#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Resultado {
    /// Identificadores que se rechazaron: no son de algo que el teléfono tenga,
    /// o su forma no es válida.
    pub rechazados: Vec<String>,
    /// Notas que cambiaron en el PC mientras el teléfono las editaba: su cambio
    /// entró como nota nueva, con este identificador.
    pub notas_copiadas: Vec<(String, String)>,
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo al aplicar lo que subió un teléfono");
    NbError::StorageFailure
}

/// Un Locator que el lector puede leer, como texto para la base.
fn locator_valido(valor: &Value) -> Option<String> {
    serde_json::from_value::<Locator>(valor.clone()).ok()?;
    serde_json::to_string(valor).ok()
}

/// Un instante ISO-8601 UTC con milisegundos, la única forma en que la base
/// guarda fechas: ordenarlos como texto es ordenarlos en el tiempo.
fn instante_valido(texto: &str) -> bool {
    let b = texto.as_bytes();
    b.len() == 24
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && b[16] == b':'
        && b[19] == b'.'
        && b[23] == b'Z'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7 | 10 | 13 | 16 | 19 | 23) || c.is_ascii_digit())
}

/// Aplica un envío entero en una transacción.
///
/// # Errors
///
/// - [`NbError::Internal`] si el envío pasa de [`MAX_POR_ENVIO`].
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn aplicar(db: &Database, dispositivo: &str, envio: &Envio) -> NbResult<Resultado> {
    let total = envio.anotaciones.len()
        + envio.posiciones.len()
        + envio.sesiones.len()
        + envio.vocabulario.len()
        + envio.repasos.len()
        + envio.notas.len();
    if total > MAX_POR_ENVIO {
        return Err(NbError::Internal);
    }
    let mut conn = db.connection()?;
    let tx = write_transaction(&mut conn)?;
    let mut r = Resultado::default();
    let seleccionada = |conn: &Connection, edicion: &str| -> NbResult<bool> {
        conn.query_row(
            &format!("SELECT ?2 IN ({SELECCIONADAS})"),
            [dispositivo, edicion],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))
    };

    for a in &envio.anotaciones {
        let locator = locator_valido(&a.locator);
        let trazo = a
            .trazo
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .ok()
            .flatten();
        let forma = locator.is_some()
            && instante_valido(&a.creada_en)
            && a.baja_en.as_deref().is_none_or(instante_valido)
            && matches!(
                a.tipo.as_str(),
                "highlight" | "note" | "bookmark" | "underline" | "drawing"
            )
            && a.trazo.as_ref().is_none_or(Value::is_object);
        if !forma || !seleccionada(&tx, &a.edicion)? {
            r.rechazados.push(a.id.clone());
            continue;
        }
        let existente: Option<Option<String>> = tx
            .query_row(
                "SELECT deleted_at FROM annotations WHERE id = ?1",
                [&a.id],
                |f| f.get(0),
            )
            .optional()
            .map_err(|e| fallo(&e))?;
        match existente {
            None => {
                let insertada = tx.execute(
                    "INSERT INTO annotations
                       (id, edition_id, kind, body, target, payload, created_at, updated_at,
                        deleted_at, supersedes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8,
                             (SELECT id FROM annotations WHERE id = ?9))",
                    rusqlite::params![
                        a.id,
                        a.edicion,
                        a.tipo,
                        a.texto,
                        locator,
                        trazo,
                        a.creada_en,
                        a.baja_en,
                        a.sustituye_a
                    ],
                );
                if insertada.is_err() {
                    // La propia base rechaza lo que no cumple sus reglas (un trazo
                    // sin geometría, una nota sin texto): eso es del teléfono, no
                    // un fallo del PC.
                    r.rechazados.push(a.id.clone());
                }
            }
            // La única transición admitida en una anotación: darla de baja.
            Some(None) if a.baja_en.is_some() => {
                tx.execute(
                    "UPDATE annotations SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                    rusqlite::params![a.id, a.baja_en],
                )
                .map_err(|e| fallo(&e))?;
            }
            Some(_) => {}
        }
    }

    for p in &envio.posiciones {
        let Some(locator) = locator_valido(&p.locator) else {
            r.rechazados.push(p.edicion.clone());
            continue;
        };
        if !instante_valido(&p.actualizado_en) || !seleccionada(&tx, &p.edicion)? {
            r.rechazados.push(p.edicion.clone());
            continue;
        }
        tx.execute(
            "INSERT INTO progreso_de_dispositivos (dispositivo_id, edition_id, locator, actualizado_en)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (dispositivo_id, edition_id) DO UPDATE
                SET locator = excluded.locator, actualizado_en = excluded.actualizado_en
              WHERE excluded.actualizado_en > progreso_de_dispositivos.actualizado_en",
            rusqlite::params![dispositivo, p.edicion, locator, p.actualizado_en],
        )
        .map_err(|e| fallo(&e))?;
    }

    for s in &envio.sesiones {
        let desde = locator_valido(&s.desde);
        let hasta = s.hasta.as_ref().map(locator_valido);
        let forma = desde.is_some()
            && !matches!(hasta, Some(None))
            && instante_valido(&s.inicio)
            && s.fin.as_deref().is_none_or(instante_valido);
        if !forma || !seleccionada(&tx, &s.edicion)? {
            r.rechazados.push(s.id.clone());
            continue;
        }
        let hasta = hasta.flatten();
        tx.execute(
            "INSERT INTO reading_sessions (id, edition_id, started_at, ended_at, start_locator, end_locator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (id) DO UPDATE
                SET ended_at = excluded.ended_at, end_locator = excluded.end_locator
              WHERE reading_sessions.ended_at IS NULL AND excluded.ended_at IS NOT NULL",
            rusqlite::params![s.id, s.edicion, s.inicio, s.fin, desde, hasta],
        )
        .map_err(|e| fallo(&e))?;
    }

    for v in &envio.vocabulario {
        let locator = locator_valido(&v.locator);
        if locator.is_none() || !instante_valido(&v.creada_en) || !seleccionada(&tx, &v.edicion)? {
            r.rechazados.push(v.id.clone());
            continue;
        }
        if tx
            .execute(
                "INSERT OR IGNORE INTO vocabulary (id, headword, term, edition_id, provider_id, locator, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![v.id, v.lema, v.termino, v.edicion, v.proveedor, locator, v.creada_en],
            )
            .is_err()
        {
            r.rechazados.push(v.id.clone());
        }
    }

    let mut tarjetas = BTreeSet::new();
    for g in &envio.repasos {
        let edicion: Option<String> = tx
            .query_row(
                "SELECT edition_id FROM study_cards WHERE id = ?1",
                [&g.tarjeta],
                |f| f.get(0),
            )
            .optional()
            .map_err(|e| fallo(&e))?;
        let valida = crate::ai::repaso_espaciado::CalificacionRepaso::desde_str(&g.calificacion)
            .is_ok()
            && instante_valido(&g.repasado_en);
        let Some(edicion) = edicion.filter(|_| valida) else {
            r.rechazados.push(g.id.clone());
            continue;
        };
        if !seleccionada(&tx, &edicion)? {
            r.rechazados.push(g.id.clone());
            continue;
        }
        let nuevo = tx
            .execute(
                "INSERT OR IGNORE INTO registro_de_repasos (id, card_id, calificacion, repasado_en, dispositivo_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![g.id, g.tarjeta, g.calificacion, g.repasado_en, dispositivo],
            )
            .map_err(|e| fallo(&e))?;
        if nuevo > 0 {
            tarjetas.insert(g.tarjeta.clone());
        }
    }
    for tarjeta in &tarjetas {
        crate::ai::cuaderno::recalcular_tarjeta(&tx, tarjeta)?;
    }

    for n in &envio.notas {
        if n.titulo.trim().is_empty() {
            r.rechazados.push(n.id.clone());
            continue;
        }
        let revision: Option<i64> = tx
            .query_row("SELECT revision FROM notes WHERE id = ?1", [&n.id], |f| {
                f.get(0)
            })
            .optional()
            .map_err(|e| fallo(&e))?;
        match (revision, n.revision_vista) {
            (None, _) => {
                let titulo = titulo_libre(&tx, &n.titulo)?;
                tx.execute(
                    "INSERT INTO notes (id, title, body) VALUES (?1, ?2, ?3)",
                    rusqlite::params![n.id, titulo, n.cuerpo],
                )
                .map_err(|e| fallo(&e))?;
            }
            (Some(actual), Some(vista)) if actual == vista => {
                let titulo = if tx
                    .query_row(
                        "SELECT EXISTS (SELECT 1 FROM notes WHERE title = ?1 AND id <> ?2)",
                        [&n.titulo, &n.id],
                        |f| f.get::<_, bool>(0),
                    )
                    .map_err(|e| fallo(&e))?
                {
                    titulo_libre(&tx, &n.titulo)?
                } else {
                    n.titulo.clone()
                };
                tx.execute(
                    "UPDATE notes SET title = ?2, body = ?3 WHERE id = ?1",
                    rusqlite::params![n.id, titulo, n.cuerpo],
                )
                .map_err(|e| fallo(&e))?;
            }
            (Some(_), _) => {
                let titulo = titulo_libre(&tx, &format!("{} (desde el teléfono)", n.titulo))?;
                let nueva: String = tx
                    .query_row(
                        "INSERT INTO notes (title, body) VALUES (?1, ?2) RETURNING id",
                        rusqlite::params![titulo, n.cuerpo],
                        |f| f.get(0),
                    )
                    .map_err(|e| fallo(&e))?;
                r.notas_copiadas.push((n.id.clone(), nueva));
            }
        }
    }

    tx.commit().map_err(|e| fallo(&e))?;
    Ok(r)
}

/// Un título que no choca con otro: el título es el nombre con el que las notas
/// se enlazan, y la base no admite dos iguales.
fn titulo_libre(conn: &Connection, deseado: &str) -> NbResult<String> {
    let existe = |t: &str| -> NbResult<bool> {
        conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM notes WHERE title = ?1)",
            [t],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))
    };
    if !existe(deseado)? {
        return Ok(deseado.to_owned());
    }
    for n in 2..1_000 {
        let candidato = format!("{deseado} ({n})");
        if !existe(&candidato)? {
            return Ok(candidato);
        }
    }
    Err(NbError::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn banco() -> Database {
        let db = Database::open_in_memory().expect("base");
        db.connection()
            .expect("conexión")
            .execute_batch(
                "INSERT INTO works (id, canonical_title) VALUES ('w1', 'Obra');
                 INSERT INTO editions (id, work_id, title) VALUES ('e1', 'w1', 'Uno');
                 INSERT INTO works (id, canonical_title) VALUES ('w2', 'Ajena');
                 INSERT INTO editions (id, work_id, title) VALUES ('e2', 'w2', 'Dos');
                 INSERT INTO dispositivos (id, nombre, plataforma, huella_del_token)
                 VALUES ('d1', 'Teléfono', 'android', printf('%064d', 1));
                 INSERT INTO seleccion_de_dispositivos VALUES ('d1', 'edicion', 'e1');",
            )
            .expect("datos");
        db
    }

    fn locator() -> Value {
        json!({"href": "c1.xhtml", "mediaType": "application/xhtml+xml",
               "locations": {"fragments": ["#char=10"], "progression": 0.1}})
    }

    fn anotacion(id: &str, edicion: &str) -> Anotacion {
        Anotacion {
            id: id.to_owned(),
            edicion: edicion.to_owned(),
            tipo: "bookmark".to_owned(),
            texto: None,
            locator: locator(),
            trazo: None,
            creada_en: "2026-01-01T10:00:00.000Z".to_owned(),
            baja_en: None,
            sustituye_a: None,
        }
    }

    #[test]
    fn recibir_dos_veces_lo_mismo_no_cambia_nada() {
        let db = banco();
        let envio = Envio {
            anotaciones: vec![anotacion("a1", "e1")],
            ..Envio::default()
        };
        assert_eq!(
            aplicar(&db, "d1", &envio).expect("aplicar"),
            Resultado::default()
        );
        assert_eq!(
            aplicar(&db, "d1", &envio).expect("otra vez"),
            Resultado::default()
        );
        let n: i64 = db
            .connection()
            .expect("c")
            .query_row("SELECT count(*) FROM annotations", [], |f| f.get(0))
            .expect("cuenta");
        assert_eq!(n, 1);
    }

    #[test]
    fn no_se_anota_lo_que_el_telefono_no_tiene() {
        let db = banco();
        let envio = Envio {
            anotaciones: vec![anotacion("a2", "e2")],
            ..Envio::default()
        };
        let r = aplicar(&db, "d1", &envio).expect("aplicar");
        assert_eq!(r.rechazados, vec!["a2".to_owned()]);
    }

    #[test]
    fn la_posicion_del_telefono_no_toca_la_del_pc_ni_retrocede() {
        let db = banco();
        let pos = |en: &str| Envio {
            posiciones: vec![Posicion {
                edicion: "e1".to_owned(),
                locator: locator(),
                actualizado_en: en.to_owned(),
            }],
            ..Envio::default()
        };
        aplicar(&db, "d1", &pos("2026-01-02T00:00:00.000Z")).expect("nueva");
        aplicar(&db, "d1", &pos("2026-01-01T00:00:00.000Z")).expect("vieja");
        let conn = db.connection().expect("c");
        let en: String = conn
            .query_row(
                "SELECT actualizado_en FROM progreso_de_dispositivos",
                [],
                |f| f.get(0),
            )
            .expect("fila");
        assert_eq!(en, "2026-01-02T00:00:00.000Z");
        let del_pc: i64 = conn
            .query_row("SELECT count(*) FROM progress", [], |f| f.get(0))
            .expect("cuenta");
        assert_eq!(del_pc, 0);
    }

    #[test]
    fn una_nota_cambiada_en_el_pc_no_se_pisa_y_el_cambio_entra_como_nota_nueva() {
        let db = banco();
        let conn = db.connection().expect("c");
        conn.execute(
            "INSERT INTO notes (id, title, body) VALUES ('n1', 'Idea', 'v0')",
            [],
        )
        .expect("nota");
        conn.execute("UPDATE notes SET body = 'v1 del PC' WHERE id = 'n1'", [])
            .expect("editar en el PC");
        drop(conn);
        let envio = Envio {
            notas: vec![Nota {
                id: "n1".to_owned(),
                titulo: "Idea".to_owned(),
                cuerpo: "v1 del teléfono".to_owned(),
                revision_vista: Some(0),
            }],
            ..Envio::default()
        };
        let r = aplicar(&db, "d1", &envio).expect("aplicar");
        assert_eq!(r.notas_copiadas.len(), 1);
        let conn = db.connection().expect("c");
        let del_pc: String = conn
            .query_row("SELECT body FROM notes WHERE id = 'n1'", [], |f| f.get(0))
            .expect("nota");
        assert_eq!(del_pc, "v1 del PC");
        let copia: String = conn
            .query_row(
                "SELECT body FROM notes WHERE id = ?1",
                [&r.notas_copiadas[0].1],
                |f| f.get(0),
            )
            .expect("copia");
        assert_eq!(copia, "v1 del teléfono");
    }

    #[test]
    fn solo_se_aceptan_instantes_con_la_forma_de_la_base() {
        assert!(instante_valido("2026-01-01T10:00:00.000Z"));
        assert!(!instante_valido("2026-01-01 10:00:00"));
        assert!(!instante_valido("2026-01-01T10:00:00Z"));
    }
}
