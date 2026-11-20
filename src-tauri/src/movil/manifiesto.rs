//! El manifiesto: lo que cambió desde la última versión que aplicó un teléfono,
//! limitado a lo que ese teléfono eligió recibir.
//!
//! Contrato: `docs/architecture/mobile-contract.md`, sección 4.3.
//!
//! # Por qué las fichas se componen en SQL
//!
//! Una página son quinientas cosas, cada una con sus autores, sus materias y su
//! archivo. Leerlas fila a fila y montarlas en Rust serían miles de consultas por
//! página; `json_object` las compone en la misma consulta que las lee, y lo que
//! sale de la base ya es el JSON que viaja.

use rusqlite::Connection;
use serde::Serialize;
use serde_json::value::RawValue;

use crate::error::{NbError, NbResult};

/// Versión del protocolo. Un cambio que no sea aditivo la sube (ADR-016).
pub const PROTOCOLO: u32 = 1;

/// Páginas de este tamaño: lo bastante grandes para que el primer manifiesto de
/// una biblioteca seleccionada entera no sean miles de idas y vueltas, y lo
/// bastante pequeñas para que el teléfono aplique cada una en una transacción
/// corta.
pub const PAGINA_POR_DEFECTO: u32 = 500;
pub const PAGINA_MAXIMA: u32 = 2_000;

/// Las ediciones que recibe un teléfono, como subconsulta. `?1` es el teléfono.
///
/// Una sola expresión para el manifiesto, los archivos y las portadas: si cada
/// sitio decidiera por su cuenta qué está seleccionado, un teléfono podría
/// recibir una ficha cuyo archivo después se le niega.
pub const SELECCIONADAS: &str = "
    SELECT e.id FROM editions e
     WHERE EXISTS (SELECT 1 FROM seleccion_de_dispositivos
                    WHERE dispositivo_id = ?1 AND tipo = 'todo')
    UNION
    SELECT ci.edition_id FROM collection_items ci
      JOIN seleccion_de_dispositivos s
        ON s.dispositivo_id = ?1 AND s.tipo = 'coleccion' AND s.clave = ci.collection_id
    UNION
    SELECT ec.edition_id FROM edition_contributors ec
      JOIN contributors c ON c.id = ec.contributor_id
      JOIN seleccion_de_dispositivos s
        ON s.dispositivo_id = ?1 AND s.tipo = 'autor' AND s.clave = c.name
     WHERE ec.role = 'author'
    UNION
    SELECT e.id FROM editions e
      JOIN seleccion_de_dispositivos s
        ON s.dispositivo_id = ?1 AND s.tipo = 'serie' AND s.clave = e.series_title
    UNION
    SELECT es.edition_id FROM edition_subjects es
      JOIN seleccion_de_dispositivos s
        ON s.dispositivo_id = ?1 AND s.tipo = 'materia' AND s.clave = es.subject_id
    UNION
    SELECT s.clave FROM seleccion_de_dispositivos s
     WHERE s.dispositivo_id = ?1 AND s.tipo = 'edicion'";

/// La ficha de una edición, como la recibe el teléfono. `x` es la edición.
const FICHA_EDICION: &str = "
    json_object(
      'id', x.id,
      'titulo', x.title,
      'subtitulo', x.subtitle,
      'autores', json((SELECT json_group_array(c.name) FROM (
                    SELECT c.name FROM edition_contributors ec
                      JOIN contributors c ON c.id = ec.contributor_id
                     WHERE ec.edition_id = x.id AND ec.role = 'author'
                     ORDER BY ec.position) c)),
      'orden_autor', x.author_sort,
      'serie', x.series_title,
      'indice_serie', x.series_index,
      'idioma', x.language,
      'editorial', x.publisher,
      'fecha', x.published_date,
      'sinopsis', x.description,
      'materias', json((SELECT json_group_array(json_object('id', su.id, 'nombre', su.label))
                     FROM edition_subjects es JOIN subjects su ON su.id = es.subject_id
                    WHERE es.edition_id = x.id)),
      'portada', json(CASE WHEN x.cover_path IS NULL THEN 'false' ELSE 'true' END),
      'archivo', json((SELECT json_object('huella', f.content_hash, 'bytes', f.byte_size,
                                     'tipo', f.media_type,
                                     'disponible', json(CASE f.availability WHEN 'available' THEN 'true' ELSE 'false' END))
                    FROM files f WHERE f.edition_id = x.id
                   ORDER BY f.availability = 'available' DESC, f.created_at LIMIT 1))
    )";

/// Un cambio del manifiesto: qué cosa, en qué versión, y su contenido actual o su
/// lápida.
#[derive(Debug, Serialize)]
pub struct Cambio {
    pub version: i64,
    pub entidad: String,
    pub clave: String,
    pub borrado: bool,
    /// La ficha, ya en JSON. Ausente en las lápidas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datos: Option<Box<RawValue>>,
}

/// Una página del manifiesto.
#[derive(Debug, Serialize)]
pub struct Pagina {
    pub cambios: Vec<Cambio>,
    /// La última versión que existe ahora mismo en el PC.
    pub version_actual: i64,
    /// Desde dónde pedir la siguiente página.
    pub siguiente: i64,
    pub hay_mas: bool,
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo al componer el manifiesto");
    NbError::StorageFailure
}

/// Una página de cambios para un teléfono.
///
/// Las lápidas viajan siempre, también las de cosas que el teléfono nunca
/// recibió: decidir si las tuvo exigiría recordar qué se le envió, y una lápida
/// de algo que no tiene no le cuesta nada ignorarla.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn pagina(conn: &Connection, dispositivo: &str, desde: i64, limite: u32) -> NbResult<Pagina> {
    let limite = limite.clamp(1, PAGINA_MAXIMA);
    let version_actual: i64 = conn
        .query_row(
            "SELECT COALESCE(max(version), 0) FROM registro_de_cambios",
            [],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))?;

    let sql = format!(
        "WITH sel(id) AS ({SELECCIONADAS})
         SELECT r.version, r.entidad, r.clave, r.borrado,
                CASE
                  WHEN r.borrado = 1 THEN NULL
                  WHEN r.entidad = 'edicion' THEN
                    (SELECT {FICHA_EDICION} FROM editions x WHERE x.id = r.clave)
                  WHEN r.entidad = 'coleccion' THEN
                    (SELECT json_object('id', k.id, 'nombre', k.name, 'tipo', k.kind,
                              'ediciones', json((SELECT json_group_array(edition_id) FROM (
                                              SELECT edition_id FROM collection_items
                                               WHERE collection_id = k.id
                                               ORDER BY position, added_at))))
                       FROM collections k WHERE k.id = r.clave)
                  WHEN r.entidad = 'anotacion' THEN
                    (SELECT json_object('id', a.id, 'edicion', a.edition_id, 'tipo', a.kind,
                              'texto', a.body, 'locator', json(a.target), 'trazo', json(a.payload),
                              'creada_en', a.created_at, 'baja_en', a.deleted_at,
                              'sustituye_a', a.supersedes)
                       FROM annotations a WHERE a.id = r.clave)
                  WHEN r.entidad = 'progreso' THEN
                    (SELECT json_object('edicion', p.edition_id, 'locator', json(p.locator),
                              'actualizado_en', p.updated_at)
                       FROM progress p WHERE p.edition_id = r.clave AND p.profile = 'default')
                  WHEN r.entidad = 'sesion' THEN
                    (SELECT json_object('id', s.id, 'edicion', s.edition_id,
                              'inicio', s.started_at, 'fin', s.ended_at,
                              'desde', json(s.start_locator), 'hasta', json(s.end_locator))
                       FROM reading_sessions s WHERE s.id = r.clave)
                  WHEN r.entidad = 'vocabulario' THEN
                    (SELECT json_object('id', v.id, 'edicion', v.edition_id, 'lema', v.headword,
                              'termino', v.term, 'proveedor', v.provider_id,
                              'locator', json(v.locator), 'creada_en', v.created_at)
                       FROM vocabulary v WHERE v.id = r.clave)
                  WHEN r.entidad = 'tarjeta' THEN
                    (SELECT json_object('id', t.id, 'edicion', t.edition_id, 'tipo', t.kind,
                              'anverso', t.front, 'reverso', t.back, 'opciones', json(t.choices),
                              'base', json_object('repeticiones', t.base_repetitions,
                                                  'intervalo_dias', t.base_interval_days,
                                                  'facilidad', t.base_ease_factor,
                                                  'vence_en', t.base_due_at,
                                                  'repasada_en', t.base_last_reviewed_at),
                              'creada_en', t.created_at)
                       FROM study_cards t WHERE t.id = r.clave)
                  WHEN r.entidad = 'repaso' THEN
                    (SELECT json_object('id', g.id, 'tarjeta', g.card_id,
                              'calificacion', g.calificacion, 'repasado_en', g.repasado_en)
                       FROM registro_de_repasos g WHERE g.id = r.clave)
                  WHEN r.entidad = 'nota' THEN
                    (SELECT json_object('id', n.id, 'titulo', n.title, 'cuerpo', n.body,
                              'revision', n.revision, 'actualizada_en', n.updated_at)
                       FROM notes n WHERE n.id = r.clave)
                END
           FROM registro_de_cambios r
          WHERE r.version > ?2
            AND (r.borrado = 1
                 OR (r.entidad = 'edicion' AND r.clave IN sel)
                 OR (r.entidad = 'coleccion' AND EXISTS (
                       SELECT 1 FROM seleccion_de_dispositivos s
                        WHERE s.dispositivo_id = ?1
                          AND (s.tipo = 'todo' OR (s.tipo = 'coleccion' AND s.clave = r.clave))))
                 OR (r.entidad = 'anotacion'
                     AND (SELECT edition_id FROM annotations WHERE id = r.clave) IN sel)
                 OR (r.entidad = 'progreso' AND r.clave IN sel)
                 OR (r.entidad = 'sesion'
                     AND (SELECT edition_id FROM reading_sessions WHERE id = r.clave) IN sel)
                 OR (r.entidad = 'vocabulario'
                     AND (SELECT edition_id FROM vocabulary WHERE id = r.clave) IN sel)
                 OR (r.entidad = 'tarjeta'
                     AND (SELECT edition_id FROM study_cards WHERE id = r.clave) IN sel)
                 OR (r.entidad = 'repaso'
                     AND (SELECT t.edition_id FROM registro_de_repasos g
                            JOIN study_cards t ON t.id = g.card_id
                           WHERE g.id = r.clave) IN sel)
                 OR r.entidad = 'nota')
          ORDER BY r.version
          LIMIT ?3"
    );

    let mut consulta = conn.prepare_cached(&sql).map_err(|e| fallo(&e))?;
    let filas = consulta
        .query_map(
            rusqlite::params![dispositivo, desde, i64::from(limite) + 1],
            |f| {
                Ok((
                    f.get::<_, i64>(0)?,
                    f.get::<_, String>(1)?,
                    f.get::<_, String>(2)?,
                    f.get::<_, i64>(3)? == 1,
                    f.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .map_err(|e| fallo(&e))?;

    let mut cambios = Vec::with_capacity(limite as usize);
    let mut hay_mas = false;
    for fila in filas {
        let (version, entidad, clave, borrado, datos) = fila.map_err(|e| fallo(&e))?;
        if cambios.len() == limite as usize {
            hay_mas = true;
            break;
        }
        let datos = match datos {
            Some(texto) => Some(RawValue::from_string(texto).map_err(|_| NbError::Internal)?),
            None => None,
        };
        cambios.push(Cambio {
            version,
            entidad,
            clave,
            borrado,
            datos,
        });
    }
    let siguiente = cambios.last().map_or(desde.max(0), |c| c.version);
    Ok(Pagina {
        cambios,
        version_actual,
        // Sin más páginas, todo lo posterior a `desde` ya se recorrió: el teléfono
        // puede apuntar la versión actual aunque lo último no le incumbiera.
        siguiente: if hay_mas {
            siguiente
        } else {
            version_actual.max(desde)
        },
        hay_mas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn biblioteca() -> Database {
        let db = Database::open_in_memory().expect("base");
        db.connection()
            .expect("conexión")
            .execute_batch(
                "INSERT INTO works (id, canonical_title) VALUES ('w1', 'Obra'), ('w2', 'Otra');
                 INSERT INTO editions (id, work_id, title) VALUES ('e1', 'w1', 'Uno'), ('e2', 'w2', 'Dos');
                 INSERT INTO contributors (id, name) VALUES ('c1', 'Autora');
                 INSERT INTO edition_contributors (edition_id, contributor_id, role) VALUES ('e1', 'c1', 'author');
                 INSERT INTO dispositivos (id, nombre, plataforma, huella_del_token)
                 VALUES ('d1', 'Teléfono', 'android', printf('%064d', 1));",
            )
            .expect("datos");
        db
    }

    fn claves(p: &Pagina, entidad: &str) -> Vec<String> {
        p.cambios
            .iter()
            .filter(|c| c.entidad == entidad && !c.borrado)
            .map(|c| c.clave.clone())
            .collect()
    }

    #[test]
    fn sin_seleccion_el_telefono_no_recibe_ninguna_edicion() {
        let db = biblioteca();
        let p = pagina(&db.connection().expect("c"), "d1", 0, 100).expect("página");
        assert!(claves(&p, "edicion").is_empty());
    }

    #[test]
    fn solo_llega_lo_del_autor_elegido_y_con_su_ficha() {
        let db = biblioteca();
        let conn = db.connection().expect("c");
        conn.execute(
            "INSERT INTO seleccion_de_dispositivos VALUES ('d1', 'autor', 'Autora')",
            [],
        )
        .expect("selección");
        let p = pagina(&conn, "d1", 0, 100).expect("página");
        assert_eq!(claves(&p, "edicion"), vec!["e1".to_owned()]);
        let ficha: serde_json::Value = serde_json::from_str(
            p.cambios
                .iter()
                .find(|c| c.clave == "e1")
                .and_then(|c| c.datos.as_ref())
                .expect("ficha")
                .get(),
        )
        .expect("json");
        assert_eq!(ficha["titulo"], "Uno");
        assert_eq!(ficha["autores"][0], "Autora");
    }

    #[test]
    fn las_paginas_recorren_todo_sin_repetir_ni_saltar() {
        let db = biblioteca();
        let conn = db.connection().expect("c");
        conn.execute(
            "INSERT INTO seleccion_de_dispositivos VALUES ('d1', 'todo', '')",
            [],
        )
        .expect("selección");
        let mut desde = 0;
        let mut vistas = Vec::new();
        loop {
            let p = pagina(&conn, "d1", desde, 1).expect("página");
            vistas.extend(p.cambios.iter().map(|c| c.version));
            desde = p.siguiente;
            if !p.hay_mas {
                break;
            }
        }
        let mut ordenadas = vistas.clone();
        ordenadas.sort_unstable();
        ordenadas.dedup();
        assert_eq!(vistas, ordenadas, "sin repetidos y en orden");
        assert!(vistas.len() >= 2);
    }

    #[test]
    fn una_lapida_llega_aunque_la_cosa_ya_no_exista() {
        let db = biblioteca();
        let conn = db.connection().expect("c");
        let antes: i64 = conn
            .query_row("SELECT max(version) FROM registro_de_cambios", [], |f| {
                f.get(0)
            })
            .expect("versión");
        conn.execute("DELETE FROM editions WHERE id = 'e2'", [])
            .expect("borrar");
        let p = pagina(&conn, "d1", antes, 100).expect("página");
        assert!(p
            .cambios
            .iter()
            .any(|c| c.clave == "e2" && c.borrado && c.datos.is_none()));
    }
}
