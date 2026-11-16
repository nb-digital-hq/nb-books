//! El índice vectorial: dónde viven los vectores de los fragmentos y cómo se
//! consultan.
//!
//! Contrato: [ADR-007].
//!
//! # Lo que decidió ADR-007, y cómo queda aquí
//!
//! * **Ningún motor vectorial de terceros.** Los vectores van en la base que ya
//!   existe, en `chunk_vectors`, **cuantizados a ocho bits**, junto al fragmento
//!   al que pertenecen: 384 bytes por vector en lugar de 1.536.
//! * **La consulta es por fuerza bruta** sobre el alcance que se pida —el
//!   documento abierto, una colección—: con unos cientos o unos pocos miles de
//!   vectores, un producto escalar es más rápido que construir y mantener un
//!   índice aproximado.
//! * **El índice es caché regenerable, nunca fuente de verdad.** Los fragmentos y
//!   sus localizadores viven en `text_chunks`; un vector solo dice "este fragmento,
//!   con este modelo, apunta aquí". Borrarlos todos no pierde nada que no se pueda
//!   recalcular.
//! * **El `trait` [`VectorIndex`] aísla el motor.** Hoy tiene una sola
//!   implementación, [`EnLaBase`]; el día que una medida enseñe que la fuerza bruta
//!   no llega, entra otra detrás sin tocar a quien genera ni a quien busca.
//!
//! [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md

use std::collections::HashSet;

use rusqlite::OptionalExtension;

use super::Embedding;
use crate::db::Database;
use crate::error::{NbError, NbResult};

/// Un fragmento cercano a una consulta.
#[derive(Debug, Clone, PartialEq)]
pub struct Cercano {
    pub chunk_id: String,
    pub edition_id: String,
    /// Coseno entre la consulta y el fragmento: 1 es el mismo sentido.
    pub similitud: f32,
}

/// Dónde se busca.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alcance {
    /// Solo en estas ediciones.
    Ediciones(Vec<String>),
    /// En todo lo que tenga vectores.
    Todo,
}

/// Lo que sabe hacer un índice vectorial, sea cual sea el motor.
///
/// **No sabe qué es un fragmento ni un documento**: guarda vectores con el
/// identificador de su fragmento y su edición, y los devuelve por cercanía. Qué
/// fragmentos existen lo dice `text_chunks`, que es la fuente de verdad; por eso
/// quien busca descarta lo que devuelva el índice de un fragmento que ya no está.
pub trait VectorIndex: Send + Sync {
    /// Guarda los vectores de unos fragmentos de una edición, calculados con un
    /// modelo. Sustituye los que hubiera de esos fragmentos.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede escribir.
    fn guardar(
        &self,
        modelo: &str,
        edition_id: &str,
        vectores: &[(String, Embedding)],
    ) -> NbResult<()>;

    /// Qué fragmentos de una edición tienen vector de ese modelo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede leer.
    fn con_vector(&self, modelo: &str, edition_id: &str) -> NbResult<HashSet<String>>;

    /// Los `limite` fragmentos más cercanos a la consulta, del más al menos.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede leer.
    fn buscar(
        &self,
        modelo: &str,
        consulta: &[f32],
        alcance: &Alcance,
        limite: usize,
    ) -> NbResult<Vec<Cercano>>;

    /// Si hay algún vector de ese modelo en el alcance.
    ///
    /// Existe para no calcular el vector de una consulta —y no cargar el modelo,
    /// que son setecientos megabytes— cuando no hay nada con qué compararlo.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede leer.
    fn hay(&self, modelo: &str, alcance: &Alcance) -> NbResult<bool>;

    /// Olvida los vectores de una edición. Devuelve cuántos había.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede escribir.
    fn olvidar(&self, edition_id: &str) -> NbResult<usize>;

    /// Olvida **todos** los vectores. Devuelve cuántos había.
    ///
    /// Es lo que hace un cambio de motor o de modelo, y lo que se puede hacer sin
    /// miedo: ver la cabecera.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede escribir.
    fn vaciar(&self) -> NbResult<usize>;
}

/// Cuantiza un vector a ocho bits con signo, con una escala por vector.
///
/// Simétrica y por vector: el mayor valor absoluto pasa a 127, y la escala es lo
/// que hay que multiplicar para volver. Por vector y no global porque cada vector
/// tiene su rango, y una escala común desperdiciaría resolución en todos los que no
/// tengan el valor más grande de la biblioteca.
#[must_use]
pub fn cuantizar(vector: &[f32]) -> (f32, Vec<u8>) {
    let mayor = vector.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
    if mayor == 0.0 || !mayor.is_finite() {
        return (1.0, vec![0; vector.len()]);
    }
    let escala = mayor / 127.0;
    let bytes = vector
        .iter()
        .map(|x| {
            // `as` y no `try_from`: el valor está acotado a [-127, 127] por la
            // escala, así que no puede desbordar.
            #[allow(clippy::cast_possible_truncation)]
            let q = (x / escala).round().clamp(-127.0, 127.0) as i8;
            q.to_ne_bytes()[0]
        })
        .collect();
    (escala, bytes)
}

/// El producto escalar entre una consulta en coma flotante y un vector cuantizado.
///
/// La consulta **no** se cuantiza: es un vector, se calcula una vez por búsqueda, y
/// multiplicarla en coma flotante contra el cuantizado pierde la mitad de precisión
/// que perdería cuantizar los dos.
#[must_use]
pub fn similitud(consulta: &[f32], escala: f32, bytes: &[u8]) -> f32 {
    let suma: f32 = consulta
        .iter()
        .zip(bytes)
        .map(|(c, &b)| c * f32::from(i8::from_ne_bytes([b])))
        .sum();
    suma * escala
}

/// Deja en `cercanos` los `limite` de mayor similitud, ordenados.
fn quedarse_con_los_mejores(cercanos: &mut Vec<Cercano>, limite: usize) {
    let orden = |a: &Cercano, b: &Cercano| b.similitud.total_cmp(&a.similitud);
    if cercanos.len() > limite && limite > 0 {
        cercanos.select_nth_unstable_by(limite - 1, orden);
        cercanos.truncate(limite);
    }
    cercanos.truncate(limite);
    cercanos.sort_by(orden);
}

/// El índice de ADR-007: vectores cuantizados en `chunk_vectors`, consulta por
/// fuerza bruta.
pub struct EnLaBase {
    db: Database,
}

impl EnLaBase {
    #[must_use]
    pub const fn new(db: Database) -> Self {
        Self { db }
    }
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(%error, "el índice vectorial no respondió");
    NbError::StorageFailure
}

impl VectorIndex for EnLaBase {
    fn guardar(
        &self,
        modelo: &str,
        edition_id: &str,
        vectores: &[(String, Embedding)],
    ) -> NbResult<()> {
        let mut conn = self.db.connection()?;
        let tx = crate::db::write_transaction(&mut conn)?;
        {
            // Se inserta **a través del fragmento**: si entre calcular el vector y
            // guardarlo se reindexó la edición y el fragmento ya no existe, no se
            // guarda nada, en lugar de dejar un vector huérfano.
            let mut insertar = tx
                .prepare_cached(
                    "INSERT INTO chunk_vectors (chunk_id, edition_id, model, scale, vector)
                     SELECT id, edition_id, ?2, ?3, ?4 FROM text_chunks
                      WHERE id = ?1 AND edition_id = ?5
                     ON CONFLICT (chunk_id) DO UPDATE
                        SET model = excluded.model, scale = excluded.scale, vector = excluded.vector",
                )
                .map_err(|e| fallo(&e))?;
            for (chunk_id, vector) in vectores {
                let (escala, bytes) = cuantizar(vector);
                insertar
                    .execute(rusqlite::params![
                        chunk_id,
                        modelo,
                        f64::from(escala),
                        bytes,
                        edition_id
                    ])
                    .map_err(|e| fallo(&e))?;
            }
        }
        tx.commit().map_err(|e| fallo(&e))
    }

    fn con_vector(&self, modelo: &str, edition_id: &str) -> NbResult<HashSet<String>> {
        let conn = self.db.connection()?;
        let mut consulta = conn
            .prepare_cached(
                "SELECT chunk_id FROM chunk_vectors WHERE edition_id = ?1 AND model = ?2",
            )
            .map_err(|e| fallo(&e))?;
        let filas = consulta
            .query_map([edition_id, modelo], |fila| fila.get::<_, String>(0))
            .map_err(|e| fallo(&e))?;
        filas.collect::<Result<_, _>>().map_err(|e| fallo(&e))
    }

    fn buscar(
        &self,
        modelo: &str,
        consulta: &[f32],
        alcance: &Alcance,
        limite: usize,
    ) -> NbResult<Vec<Cercano>> {
        let conn = self.db.connection()?;
        let ediciones = match alcance {
            Alcance::Ediciones(ids) => {
                Some(serde_json::to_string(ids).map_err(|_| NbError::Internal)?)
            }
            Alcance::Todo => None,
        };
        // Por el índice de edición y modelo, y con las ediciones como un arreglo
        // JSON: una lista de parámetros de longitud variable obligaría a componer
        // la consulta a mano.
        let mut sentencia = conn
            .prepare_cached(
                "SELECT chunk_id, edition_id, scale, vector FROM chunk_vectors
                  WHERE model = ?1
                    AND (?2 IS NULL OR edition_id IN (SELECT value FROM json_each(?2)))",
            )
            .map_err(|e| fallo(&e))?;
        let mut filas = sentencia
            .query(rusqlite::params![modelo, ediciones])
            .map_err(|e| fallo(&e))?;

        let mut cercanos = Vec::new();
        while let Some(fila) = filas.next().map_err(|e| fallo(&e))? {
            let bytes: Vec<u8> = fila.get(3).map_err(|e| fallo(&e))?;
            if bytes.len() != consulta.len() {
                // Un vector de otra dimensión es de otro modelo con el mismo
                // nombre, o de una fila dañada: no se compara con nada.
                continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            let escala = fila.get::<_, f64>(2).map_err(|e| fallo(&e))? as f32;
            cercanos.push(Cercano {
                chunk_id: fila.get(0).map_err(|e| fallo(&e))?,
                edition_id: fila.get(1).map_err(|e| fallo(&e))?,
                similitud: similitud(consulta, escala, &bytes),
            });
        }
        quedarse_con_los_mejores(&mut cercanos, limite);
        Ok(cercanos)
    }

    fn hay(&self, modelo: &str, alcance: &Alcance) -> NbResult<bool> {
        let conn = self.db.connection()?;
        let ediciones = match alcance {
            Alcance::Ediciones(ids) => {
                Some(serde_json::to_string(ids).map_err(|_| NbError::Internal)?)
            }
            Alcance::Todo => None,
        };
        conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM chunk_vectors
                             WHERE model = ?1
                               AND (?2 IS NULL OR edition_id IN (SELECT value FROM json_each(?2))))",
            rusqlite::params![modelo, ediciones],
            |fila| fila.get::<_, bool>(0),
        )
        .map_err(|e| fallo(&e))
    }

    fn olvidar(&self, edition_id: &str) -> NbResult<usize> {
        let conn = self.db.connection()?;
        conn.execute(
            "DELETE FROM chunk_vectors WHERE edition_id = ?1",
            [edition_id],
        )
        .map_err(|e| fallo(&e))
    }

    fn vaciar(&self) -> NbResult<usize> {
        let conn = self.db.connection()?;
        conn.execute("DELETE FROM chunk_vectors", [])
            .map_err(|e| fallo(&e))
    }
}

/// Los fragmentos de una edición, en orden, con su locator serializado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn fragmentos_de(
    conn: &rusqlite::Connection,
    edition_id: &str,
) -> NbResult<Vec<(String, String)>> {
    let mut consulta = conn
        .prepare_cached("SELECT id, locator FROM text_chunks WHERE edition_id = ?1 ORDER BY rowid")
        .map_err(|e| fallo(&e))?;
    let filas = consulta
        .query_map([edition_id], |fila| Ok((fila.get(0)?, fila.get(1)?)))
        .map_err(|e| fallo(&e))?;
    filas.collect::<Result<_, _>>().map_err(|e| fallo(&e))
}

/// El fragmento de un identificador, con su edición y su locator, si sigue
/// existiendo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
pub fn fragmento(
    conn: &rusqlite::Connection,
    chunk_id: &str,
) -> NbResult<Option<(String, String)>> {
    conn.query_row(
        "SELECT edition_id, locator FROM text_chunks WHERE id = ?1",
        [chunk_id],
        |fila| Ok((fila.get(0)?, fila.get(1)?)),
    )
    .optional()
    .map_err(|e| fallo(&e))
}

/// Empareja los fragmentos que guarda la base con los que produce el documento,
/// para saber el texto de cada uno.
///
/// # Por qué hace falta
///
/// Porque la base ya no guarda el texto de los fragmentos: guarda su
/// locator, y el texto se relee del documento. Para calcular el vector de un
/// fragmento hay que volver a trocear el documento y reconocer cuál es cuál. Se
/// reconocen **por su locator, en orden**, y si la lista entera no coincide
/// —porque el documento se indexó con otro troceado, o cambió—
/// no se adivina: se devuelve `None`, y quien genera reindexa la edición para que
/// coincidan.
///
/// `producidos` son los del documento; `guardados`, los de [`fragmentos_de`].
#[must_use]
pub fn alinear(
    producidos: &[(crate::reader::Locator, String)],
    guardados: &[(String, String)],
) -> Option<Vec<(String, String)>> {
    if producidos.len() != guardados.len() {
        return None;
    }
    producidos
        .iter()
        .zip(guardados)
        .map(|((locator, texto), (id, serializado))| {
            // Se compara **serializado**, como lo guarda el índice léxico, y no
            // leyendo el guardado: `serde_json` lee los decimales con precisión
            // aproximada, así que una progresión como 0,333… no vuelve idéntica y
            // dos locators iguales parecerían distintos. Lo encontró la prueba.
            let producido = serde_json::to_string(locator).ok()?;
            (producido == *serializado).then(|| (id.clone(), texto.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un vector normalizado y determinista, de rango completo.
    fn vector(semilla: u64, dimension: usize) -> Vec<f32> {
        let mut estado = semilla.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let v: Vec<f32> = (0..dimension)
            .map(|_| {
                estado ^= estado << 13;
                estado ^= estado >> 7;
                estado ^= estado << 17;
                #[allow(clippy::cast_precision_loss)]
                let x = (estado % 20_000) as f32 / 10_000.0 - 1.0;
                x
            })
            .collect();
        let norma = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.into_iter().map(|x| x / norma).collect()
    }

    fn coseno(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    /// Cuantizar a ocho bits **apenas mueve la similitud**: con vectores de 384
    /// dimensiones, menos de una centésima, que es mucho menos que la distancia
    /// entre dos fragmentos que no tratan de lo mismo.
    #[test]
    fn cuantizar_apenas_mueve_la_similitud() {
        let mut peor = 0.0_f32;
        for i in 0..200 {
            let a = vector(i, 384);
            let b = vector(i + 1_000, 384);
            let (escala, bytes) = cuantizar(&b);
            assert_eq!(bytes.len(), 384);
            peor = peor.max((similitud(&a, escala, &bytes) - coseno(&a, &b)).abs());
            // Consigo mismo, casi uno.
            let (escala_a, bytes_a) = cuantizar(&a);
            assert!((similitud(&a, escala_a, &bytes_a) - 1.0).abs() < 0.01);
        }
        assert!(peor < 0.01, "la cuantización movió la similitud {peor}");
    }

    /// Lo mismo con **vectores del modelo real**: los de la referencia de
    /// `sentence-transformers` que comprueba la prueba del modelo. Entre las seis frases, cada
    /// similitud se mueve menos de una centésima y el orden de los quince pares no
    /// cambia.
    #[test]
    fn con_vectores_del_modelo_real_la_cuantizacion_no_cambia_el_orden() {
        let referencia: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/granite-embedding-referencia.json"
        ))
        .expect("referencia");
        let vectores: Vec<Vec<f32>> = referencia["vectores"]
            .as_array()
            .expect("vectores")
            .iter()
            .map(|v| {
                v.as_array()
                    .expect("vector")
                    .iter()
                    .map(|x| {
                        #[allow(clippy::cast_possible_truncation)]
                        let x = x.as_f64().expect("número") as f32;
                        x
                    })
                    .collect()
            })
            .collect();

        let mut exactos = Vec::new();
        let mut cuantizados = Vec::new();
        for i in 0..vectores.len() {
            for j in i + 1..vectores.len() {
                let (escala, bytes) = cuantizar(&vectores[j]);
                let exacto = coseno(&vectores[i], &vectores[j]);
                let cuantizado = similitud(&vectores[i], escala, &bytes);
                assert!((exacto - cuantizado).abs() < 0.01, "par {i}-{j}");
                exactos.push(((i, j), exacto));
                cuantizados.push(((i, j), cuantizado));
            }
        }
        exactos.sort_by(|a, b| b.1.total_cmp(&a.1));
        cuantizados.sort_by(|a, b| b.1.total_cmp(&a.1));
        let orden = |v: &[((usize, usize), f32)]| v.iter().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(orden(&exactos), orden(&cuantizados));
    }

    /// Un vector nulo o con valores no finitos no rompe nada: queda en ceros y no
    /// se parece a nada.
    #[test]
    fn un_vector_degenerado_no_rompe_la_cuantizacion() {
        assert_eq!(cuantizar(&[0.0; 4]), (1.0, vec![0; 4]));
        let (escala, bytes) = cuantizar(&[f32::NAN, 1.0]);
        assert!(escala > 0.0 && bytes.len() == 2);
        assert_eq!(similitud(&[1.0, 1.0], 1.0, &[0, 0]), 0.0);
    }

    /// Los mejores, en orden, y nunca más de los pedidos.
    #[test]
    fn se_quedan_los_mejores_en_orden() {
        let mut cercanos: Vec<Cercano> = [0.2_f32, 0.9, 0.5, 0.7, 0.1]
            .iter()
            .enumerate()
            .map(|(i, &s)| Cercano {
                chunk_id: i.to_string(),
                edition_id: "e".to_owned(),
                similitud: s,
            })
            .collect();
        quedarse_con_los_mejores(&mut cercanos, 3);
        let orden: Vec<f32> = cercanos.iter().map(|c| c.similitud).collect();
        assert_eq!(orden, vec![0.9, 0.7, 0.5]);

        let mut vacio: Vec<Cercano> = Vec::new();
        quedarse_con_los_mejores(&mut vacio, 3);
        assert!(vacio.is_empty());
    }
}
