//! Búsqueda léxica de texto completo.
//!
//! # Cómo está montado
//!
//! El proveedor trocea el documento y da el locator de cada fragmento; este
//! módulo lo guarda en `text_chunks` y consulta el índice FTS5 que los
//! disparadores de la V8 mantienen al día. Así, añadir un formato no obliga a
//! tocar la búsqueda, y olvidarse de reindexar es imposible: no hay un paso que
//! olvidar.
//!
//! # Esto es caché
//!
//! Todo lo que hay aquí se regenera leyendo los documentos otra vez. Perderlo
//! cuesta CPU, no información. Por eso reindexar es una operación normal y no
//! una recuperación de desastre.
//!
//! # La consulta del usuario no es una consulta de FTS5
//!
//! FTS5 tiene su propio lenguaje: comillas, `AND`, `OR`, `NEAR`, prefijos con
//! asterisco. Pasarle lo que alguien teclee produce dos problemas, y el segundo
//! es el grave: una comilla suelta hace fallar la consulta con un error de
//! sintaxis, y escribir `documento OR` cambia lo que se busca sin que nadie lo
//! haya pedido. Aquí la entrada se convierte en términos entrecomillados, así
//! que el usuario busca **palabras**, no expresiones.
//!
//! Contrato: `docs/architecture/search.md`.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::reader::{DocumentProvider, Locator};

/// Caracteres por fragmento indexado.
///
/// Suficiente para que un fragmento tenga contexto de sobra al mostrarlo y corto
/// para que el resultado abra una posición útil: con fragmentos de cinco mil
/// caracteres, "abrir la posición exacta" sería abrir el principio del capítulo.
///
/// # Por qué 1.920 y no 480, que era el de antes
///
/// Porque **cada fragmento se paga aunque no lo lea nadie**: lleva un
/// identificador de 32 caracteres y su locator en JSON, unos 300 bytes, y eso se
/// multiplica por cuantos haya. Medido sobre cuarenta EPUB reales, con 480
/// caracteres salen 37.358 fragmentos y ese coste por fila son unos 13 MB sobre
/// 44 de biblioteca -el 29 %-; con 1.920 son 9.783 y baja a 3,4 MB, el 7,7 %.
///
/// No se sube más porque el que abre un resultado tiene que caer en el pasaje y
/// no en el principio del capítulo, y porque a partir del tamaño de página de
/// SQLite la fila se desborda y se desperdicia sitio: 5.760 mide peor que 3.840.
///
/// Y lo que **no** hace este número es bajar el índice invertido, que depende del
/// vocabulario y no de en cuántas filas esté repartido: entre 8,5 y 9,6 MB con
/// diez veces menos fragmentos. Ver `performance-budgets.md`, sección 4.5.
pub const FRAGMENTO: usize = 1_920;

/// Mínimo de caracteres para buscar por prefijo.
///
/// Con menos, la consulta incremental recorre casi el índice entero en cada
/// pulsación y el resultado no dice nada: "a*" encuentra media biblioteca.
pub const MINIMO_PREFIJO: usize = 3;

/// Un resultado de búsqueda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Hit {
    pub edition_id: String,
    /// El archivo con el que se abre, si hay alguno disponible.
    ///
    /// **Viaja con el resultado**, por el mismo motivo por el que ya
    /// viajaba con una anotación, con una palabra del vocabulario y con una
    /// recomendación: sin él, quien pulsa un resultado tiene una edición y
    /// ninguna forma de abrirla, y la interfaz no puede hacer más que llevar a
    /// otra pantalla. Antes de esa fecha eso es exactamente lo que pasaba, y
    /// pulsar un resultado no abría nada.
    ///
    /// `None` cuando ninguna copia está disponible -el volumen desconectado, por
    /// ejemplo-, que es lo que permite a la interfaz no ofrecer un salto que
    /// fallaría.
    pub file_id: Option<String>,
    /// Dónde está, con todos sus selectores. Es lo que permite que un resultado
    /// abra la posición exacta.
    pub locator: Locator,
    /// El fragmento con la coincidencia marcada, **generado en SQL**.
    ///
    /// En SQL y no en el frontend porque marcar la coincidencia exige saber
    /// cómo tokenizó el índice: sin tildes, sin mayúsculas y por raíces. Un
    /// resaltado hecho a mano en JavaScript buscaría la cadena literal y no
    /// marcaría nada al buscar "analisis" sobre un texto que dice "análisis".
    pub snippet: String,
    /// Puntuación de FTS5. Más negativo es mejor coincidencia.
    pub rank: f64,
    /// El título de la obra, para decir **de qué libro** es el pasaje. Hasta el
    /// 2026-11-12 no viajaba y la lista de resultados era una columna de frases
    /// sin dueño.
    pub title: String,
}

/// Una coincidencia tal como sale de la base: edición, locator, puntuación,
/// archivo y título.
pub type Fila = (String, String, f64, Option<String>, Option<String>);

/// Marcas que envuelven la coincidencia dentro del fragmento.
///
/// Corchetes y no etiquetas HTML: lo que sale de aquí es texto, y devolver
/// `<mark>` obligaría a la interfaz a interpretarlo como marcado, que es
/// exactamente por donde entra una inyección cuando el texto viene de un
/// documento que escribió un desconocido.
pub const MARCA_INICIO: &str = "\u{2039}";
pub const MARCA_FIN: &str = "\u{203a}";

/// Indexa una edición a partir de su documento abierto.
///
/// Borra lo que hubiera indexado antes: reindexar sustituye, no acumula.
///
/// Devuelve cuántos fragmentos quedaron indexados.
///
/// # Errors
///
/// - [`NbError::FormatUnsupported`] si el documento no tiene texto que indexar.
///   No es un fallo del sistema: es un cómic, o un PDF escaneado sin OCR, y hay
///   que decirlo para poder marcarlo como no buscable.
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn index_edition(
    conn: &mut Connection,
    edition_id: &str,
    provider: &dyn DocumentProvider,
) -> NbResult<usize> {
    index_edition_con(conn, edition_id, provider, FRAGMENTO)
}

/// Cuántas palabras se prueban antes de renunciar a poner sonda a una edición.
///
/// Cinco. La primera casi siempre sirve; las otras cuatro son para el documento
/// cuyo principio es una portada, un número de página o una cita en otro
/// alfabeto. Renunciar tras cinco es más barato que recorrer un libro entero
/// buscando la palabra perfecta: una edición sin sonda no rompe nada, solo no
/// participa en la comprobación.
const CANDIDATAS_A_SONDA: usize = 5;

/// Lo mismo, con el tamaño de fragmento como parámetro.
///
/// Existe porque **el tamaño es una política y no un mecanismo**: decide cuánto
/// ocupa el índice y con qué precisión abre un resultado, y las dos cosas se
/// eligen midiendo. Que se pueda pasar es lo que permite barrer varios tamaños
/// sobre los mismos libros en una sola ejecución; quien indexa de verdad usa
/// [`index_edition`], que pone el que decidió el proyecto.
///
/// # Errors
///
/// Los mismos que [`index_edition`].
pub fn index_edition_con(
    conn: &mut Connection,
    edition_id: &str,
    provider: &dyn DocumentProvider,
    fragmento: usize,
) -> NbResult<usize> {
    let fragmentos = provider.text_chunks(fragmento)?;

    let tx = crate::db::write_transaction(conn)?;
    tx.execute(
        "DELETE FROM text_chunks WHERE edition_id = ?1",
        [edition_id],
    )
    .map_err(|e| fallo(&e))?;
    tx.execute(
        "DELETE FROM index_probes WHERE edition_id = ?1",
        [edition_id],
    )
    .map_err(|e| fallo(&e))?;

    // Las candidatas a sonda de esta edición: palabras que deberían poder
    // encontrarse, con el fragmento donde tienen que estar. Ver `verify_index`.
    let mut candidatas: Vec<(String, i64)> = Vec::new();

    for (locator, texto) in &fragmentos {
        let serializado = serde_json::to_string(locator).map_err(|_| NbError::Internal)?;
        tx.execute(
            "INSERT INTO text_chunks (edition_id, locator) VALUES (?1, ?2)",
            rusqlite::params![edition_id, serializado],
        )
        .map_err(|e| fallo(&e))?;
        let fila = tx.last_insert_rowid();

        /*
         * El alta del índice se hace aquí, y no por disparador.
         *
         * V8 la hacía por disparador y explicaba por qué: un índice que el
         * código olvida actualizar produce búsquedas que no encuentran lo que
         * está ahí. Con el texto fuera de la tabla, el disparador ya no tiene de
         * dónde sacarlo, así que la garantía baja de la base al código y este es
         * el único sitio que escribe fragmentos. La baja sí sigue siendo un
         * disparador: para borrar basta el identificador de fila.
         */
        tx.execute(
            "INSERT INTO documents_fts(rowid, text) VALUES (?1, ?2)",
            rusqlite::params![fila, texto],
        )
        .map_err(|e| fallo(&e))?;

        if candidatas.len() < CANDIDATAS_A_SONDA {
            candidatas.extend(
                terminos_de_sonda(texto, CANDIDATAS_A_SONDA - candidatas.len())
                    .into_iter()
                    .map(|termino| (termino, fila)),
            );
        }
    }

    /*
     * **La sonda se comprueba antes de guardarla, y eso es lo que la hace
     * utilizable.**
     *
     * Si se guardara sin comprobar, una sonda que no responde podría significar
     * dos cosas: que el índice está corrupto, o que se eligió mal la palabra. Lo
     * segundo ya pasó -partir "cañabrava" por la eñe- y el precio de confundirlo
     * con lo primero es borrar un índice sano y releer ciento cincuenta mil
     * libros. Comprobándola aquí solo queda un significado posible.
     *
     * Cuesta una búsqueda por edición, dentro de una operación que acaba de leer
     * y trocear un libro entero.
     */
    let sonda = candidatas
        .into_iter()
        .find(|(termino, fila)| sonda_responde(&tx, termino, *fila));

    if let Some((termino, fila)) = sonda {
        tx.execute(
            "INSERT INTO index_probes (edition_id, term, chunk_rowid) VALUES (?1, ?2, ?3)",
            rusqlite::params![edition_id, termino, fila],
        )
        .map_err(|e| fallo(&e))?;
    }

    /*
     * El estado se apunta **en la misma transacción** que los fragmentos.
     * Desde entonces es lo que dice qué falta por indexar al arrancar, en lugar
     * de preguntar a cada edición si tiene fragmentos, y un estado que se apunta
     * aparte podría decir "pendiente" de algo que ya está indexado y volverlo a
     * indexar entero.
     */
    if !fragmentos.is_empty() {
        set_status(&tx, edition_id, &SearchStatus::Searchable)?;
    }

    tx.commit().map_err(|e| fallo(&e))?;
    tracing::info!(
        fragmentos = fragmentos.len(),
        "documento indexado para búsqueda"
    );

    Ok(fragmentos.len())
}

/// Elige palabras con las que comprobar después que el índice sigue vivo.
///
/// # Qué palabra sirve
///
/// Una que el tokenizador vaya a devolver **igual** cuando la consulta pase por
/// él. Eso descarta dos cosas y las dos por el mismo motivo:
///
/// * Las que llevan algo que no sea ASCII, porque el índice guarda "analisis"
///   donde el texto dice "análisis" -`remove_diacritics 2`-, y guardar la forma
///   acentuada obligaría a repetir aquí la normalización del tokenizador, que es
///   la clase de copia que se desincroniza en silencio.
/// * Las que **lindan** con algo que no sea ASCII, porque ahí es donde se cortan
///   mal: "cañabrava" es un solo término para el tokenizador y en cambio se
///   parte en dos si se recorre por caracteres alfanuméricos. Buscar el trozo no
///   encuentra nada, y eso pasó: la primera versión de esto declaró corrupto un
///   índice sano.
///
/// Cuatro caracteres como mínimo: por debajo, la palabra aparece en casi
/// cualquier fragmento, y una sonda que puede encontrarse en otro sitio no dice
/// si el sitio que le tocaba sigue estando.
///
/// # Por qué devuelve varias
///
/// Porque quien la guarda **la comprueba antes de guardarla**, y necesita
/// alternativas si la primera no responde. Ver [`index_edition_con`].
fn terminos_de_sonda(texto: &str, cuantos: usize) -> Vec<String> {
    let mut candidatas = Vec::new();
    let caracteres: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < caracteres.len() && candidatas.len() < cuantos {
        if !caracteres[i].is_alphanumeric() {
            i += 1;
            continue;
        }
        let inicio = i;
        while i < caracteres.len() && caracteres[i].is_alphanumeric() {
            i += 1;
        }
        let palabra = &caracteres[inicio..i];
        let vecino_limpio = |c: Option<&char>| c.is_none_or(char::is_ascii);
        if palabra.len() >= 4
            && palabra.iter().all(char::is_ascii_alphanumeric)
            && palabra.iter().any(char::is_ascii_alphabetic)
            && vecino_limpio(inicio.checked_sub(1).and_then(|p| caracteres.get(p)))
            && vecino_limpio(caracteres.get(i))
        {
            candidatas.push(palabra.iter().collect::<String>().to_lowercase());
        }
    }
    candidatas
}

/// Retira una edición del índice.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn forget_edition(conn: &Connection, edition_id: &str) -> NbResult<()> {
    conn.execute(
        "DELETE FROM text_chunks WHERE edition_id = ?1",
        [edition_id],
    )
    .map_err(|e| fallo(&e))?;
    // Su sonda se va con ella: una sonda sin fragmento detrás señalaría una
    // corrupción que no ha ocurrido.
    conn.execute(
        "DELETE FROM index_probes WHERE edition_id = ?1",
        [edition_id],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Indica si una edición tiene algo indexado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn is_indexed(conn: &Connection, edition_id: &str) -> NbResult<bool> {
    let cuantos: i64 = conn
        .query_row(
            "SELECT count(*) FROM text_chunks WHERE edition_id = ?1",
            [edition_id],
            |r| r.get(0),
        )
        .map_err(|e| fallo(&e))?;
    Ok(cuantos > 0)
}

/// Convierte lo que el usuario teclea en una consulta de FTS5.
///
/// Cada término va entre comillas, así que nada de lo que escriba se interpreta
/// como operador. Al último se le añade `*` para que la búsqueda incremental
/// encuentre mientras se escribe, pero solo si es lo bastante largo: con dos
/// letras, el prefijo recorre casi el índice entero y no dice nada.
///
/// Devuelve `None` si no queda ningún término, que es distinto de una consulta
/// que no encuentra nada: no hay nada que buscar.
#[must_use]
pub fn to_fts_query(entrada: &str) -> Option<String> {
    let terminos: Vec<String> = entrada
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        // Las comillas dobles son el único carácter que hay que neutralizar
        // dentro de un término entrecomillado de FTS5, y se hace duplicándolas.
        .map(|t| t.replace('"', "\"\""))
        .collect();

    if terminos.is_empty() {
        return None;
    }

    let ultimo = terminos.len() - 1;
    let consulta = terminos
        .iter()
        .enumerate()
        .map(|(i, termino)| {
            if i == ultimo && termino.chars().count() >= MINIMO_PREFIJO {
                format!("\"{termino}\"*")
            } else {
                format!("\"{termino}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    Some(consulta)
}

/// Busca en el texto de una edición concreta.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn search_in_edition(
    conn: &Connection,
    edition_id: &str,
    consulta: &str,
    limite: usize,
) -> NbResult<Vec<Hit>> {
    buscar(conn, consulta, Some(edition_id), limite, 0)
}

/// Busca en todo el corpus indexado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn search_corpus(
    conn: &Connection,
    consulta: &str,
    limite: usize,
    desde: usize,
) -> NbResult<Vec<Hit>> {
    buscar(conn, consulta, None, limite, desde)
}

/// Rellena el pasaje de cada resultado releyéndolo del documento.
///
/// # Por qué no sale del índice
///
/// Porque el texto ya no se guarda: pesaba el 52,1 % de lo que ocupan los
/// documentos, más que el índice invertido y más que las portadas, y el
/// presupuesto de disco son 20. Lo que sí se guarda es el **locator** de cada
/// fragmento, que es lo que abre la posición exacta, y con él se vuelve al
/// documento y se lee el mismo trozo que se indexó. Es lo que el lector hace
/// para pintar una página.
///
/// # Qué pasa cuando el documento no está
///
/// El pasaje se queda vacío, y eso **es** la respuesta: el volumen está
/// desconectado, y quien mira tiene que saberlo en lugar de ver un hueco. La
/// interfaz distingue las dos cosas porque el resultado sigue trayendo su
/// edición y su locator; lo que no puede es enseñar palabras que no tiene.
///
/// Se abre un documento por resultado, y los resultados vienen ya acotados por
/// la página que se está enseñando. Abrir un EPUB es leer su índice central:
/// medido, del orden de un milisegundo.
pub fn con_pasajes(
    conn: &Connection,
    registry: &ProviderRegistry,
    consulta: &str,
    hits: Vec<Hit>,
) -> Vec<Hit> {
    con_pasajes_mientras(conn, registry, consulta, hits, || true)
}

/// Como [`con_pasajes`], dejando de releer en cuanto `vigente` diga que no.
///
/// Releer un pasaje abre un documento, y una búsqueda que ya sustituyó otra no
/// tiene por qué seguir abriendo veinte libros para una lista que no se va a
/// pintar. Lo que quede sin releer vuelve sin pasaje, que es lo mismo que vuelve
/// con el volumen desconectado.
pub fn con_pasajes_mientras(
    conn: &Connection,
    registry: &ProviderRegistry,
    consulta: &str,
    hits: Vec<Hit>,
    vigente: impl Fn() -> bool,
) -> Vec<Hit> {
    hits.into_iter()
        .map(|mut hit| {
            if vigente() {
                hit.snippet = pasaje_de(conn, registry, &hit, consulta).unwrap_or_default();
            }
            hit
        })
        .collect()
}

/// Lee el trozo del documento que corresponde a un resultado y lo marca.
fn pasaje_de(
    conn: &Connection,
    registry: &ProviderRegistry,
    hit: &Hit,
    consulta: &str,
) -> Option<String> {
    let (ruta, media_type): (Option<String>, String) = conn
        .query_row(
            &format!(
                "SELECT {}, f.media_type FROM files f
                  WHERE f.edition_id = ?1 AND f.availability = 'available'
                  ORDER BY f.created_at LIMIT 1",
                crate::db::catalog::SQL_RUTA_ABSOLUTA
            ),
            [&hit.edition_id],
            |fila| Ok((fila.get(0)?, fila.get(1)?)),
        )
        .ok()?;

    let ruta = ruta?;
    let documento = registry
        .open(&media_type, std::path::Path::new(&ruta))
        .ok()?;
    let texto = documento.text_range(&hit.locator, FRAGMENTO).ok()?;

    marcar(conn, &texto, consulta)
}

/// Marca dentro del pasaje lo que se estaba buscando, **con el mismo tokenizador
/// que el índice**.
///
/// # Por qué se pasa por una tabla y no se busca la cadena
///
/// Porque buscar la cadena literal no marcaría nada en el caso que importa. El
/// índice tokeniza quitando diacríticos, así que quien escribe "senal" encuentra
/// "señal" -y esa es la mitad del valor de la búsqueda para quien teclea deprisa
/// o sin acentos-. Un resaltado que compare texto no sabe eso, y dejaría el
/// resultado encontrado y sin resaltar: exactamente lo que este proyecto ya
/// rechazó cuando decidió que el fragmento resaltado lo genera la base y no el
/// frontend.
///
/// Así que se le da el pasaje a una tabla temporal con el mismo tokenizador y se
/// le pide el fragmento. Es una fila que vive lo que dura la consulta.
fn marcar(conn: &Connection, texto: &str, consulta: &str) -> Option<String> {
    let fts = to_fts_query(consulta)?;

    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS temp.pasaje
             USING fts5(text, tokenize = \"unicode61 remove_diacritics 2\");
         DELETE FROM temp.pasaje;",
    )
    .ok()?;

    conn.execute(
        "INSERT INTO temp.pasaje(rowid, text) VALUES (1, ?1)",
        [texto],
    )
    .ok()?;

    conn.query_row(
        "SELECT snippet(pasaje, 0, ?1, ?2, '…', 12) FROM temp.pasaje WHERE pasaje MATCH ?3",
        rusqlite::params![MARCA_INICIO, MARCA_FIN, fts],
        |fila| fila.get::<_, String>(0),
    )
    .ok()
}

fn buscar(
    conn: &Connection,
    consulta: &str,
    edition_id: Option<&str>,
    limite: usize,
    desde: usize,
) -> NbResult<Vec<Hit>> {
    let Some(fts) = to_fts_query(consulta) else {
        return Ok(Vec::new());
    };

    // La consulta **no se registra**: revela intereses e intenciones, y
    // `docs/audit/privacy.md` lo prohíbe expresamente. Se registra
    // cuántos resultados hubo, que no dice nada de nadie.
    /*
     * **Sin `snippet`.** Esa función lee el texto original de la
     * tabla que alimenta el índice, y desde que el texto no se guarda no hay de
     * dónde leerlo: el índice es "sin contenido". El pasaje se compone después,
     * releyéndolo del documento por el locator del fragmento. Ver `con_pasajes`.
     */
    /*
     * Dentro de un libro la consulta es otra, y no la misma con un filtro más.
     *
     * Con el filtro puesto detrás, FTS5 emparejaba la palabra en los 39 millones
     * de fragmentos de la biblioteca y descartaba después los de otros libros; y
     * aun acotando por filas, `bm25()` recorre la lista entera de cada término
     * para saber en cuántos fragmentos aparece. Medido sobre la base real,
     * buscar "que" en Hamlet tardaba 4,4 s y "ser o no ser" 8,7 s, y la búsqueda
     * del lector se quedaba en "Buscando". Aquí el índice se acota al rango de
     * filas del libro -sus fragmentos se escriben seguidos- y los resultados
     * van **en el orden del libro**, sin puntuar: 11 a 14 ms las mismas tres
     * consultas. `CROSS JOIN` fija el orden de la unión, porque el planificador
     * prefería empezar por los fragmentos y sondear el índice una vez por cada
     * uno. El filtro por edición se queda por si dos ediciones se intercalan.
     */
    let sql = if edition_id.is_some() {
        "SELECT c.edition_id, c.locator, 0.0,
                (SELECT min(a.id) FROM files a
                  WHERE a.edition_id = c.edition_id
                    AND a.availability = 'available') AS archivo,
                (SELECT e.title FROM editions e WHERE e.id = c.edition_id) AS titulo
           FROM documents_fts
          CROSS JOIN text_chunks c ON c.rowid = documents_fts.rowid
          WHERE documents_fts MATCH ?1
            AND documents_fts.rowid BETWEEN
                (SELECT min(r.rowid) FROM text_chunks r WHERE r.edition_id = ?4)
            AND (SELECT max(r.rowid) FROM text_chunks r WHERE r.edition_id = ?4)
            AND c.edition_id = ?4
          ORDER BY documents_fts.rowid
          LIMIT ?2 OFFSET ?3"
    } else {
        "SELECT c.edition_id, c.locator, bm25(documents_fts),
                (SELECT min(a.id) FROM files a
                  WHERE a.edition_id = c.edition_id
                    AND a.availability = 'available') AS archivo,
                (SELECT e.title FROM editions e WHERE e.id = c.edition_id) AS titulo
           FROM documents_fts
           JOIN text_chunks c ON c.rowid = documents_fts.rowid
          WHERE documents_fts MATCH ?1
          ORDER BY bm25(documents_fts)
          LIMIT ?2 OFFSET ?3"
    };

    let mut stmt = conn.prepare(sql).map_err(|e| fallo(&e))?;
    let leer = |row: &rusqlite::Row<'_>| -> rusqlite::Result<Fila> {
        Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
        ))
    };

    // Con la biblioteca grande, por obra y sin puntuar: ver `buscar_por_obra`.
    let grande = fragmentos_indexados(conn) > UMBRAL_SIN_PUNTUAR;
    // También dentro de un libro: el prefijo se funde sobre el índice entero
    // antes de que el rango de filas pueda recortar nada.
    let fts = if grande { sin_prefijo_corto(&fts) } else { fts };
    if edition_id.is_none() && grande {
        let filas = buscar_por_obra(conn, &fts, limite, desde)?;
        return hits_de(filas);
    }

    let limite = i64::try_from(limite).unwrap_or(i64::MAX);
    let desde = i64::try_from(desde).unwrap_or(0);

    let filas: Vec<Fila> = if let Some(id) = edition_id {
        stmt.query_map(rusqlite::params![fts, limite, desde, id], leer)
            .map_err(|e| fallo(&e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo(&e))?
    } else {
        stmt.query_map(rusqlite::params![fts, limite, desde], leer)
            .map_err(|e| fallo(&e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo(&e))?
    };

    hits_de(filas)
}

/// A partir de cuántos fragmentos indexados la biblioteca entera deja de
/// puntuarse con `bm25`.
///
/// Trescientos mil fragmentos son unas mil obras. Por debajo, ordenar por
/// relevancia cuesta décimas y es mejor respuesta; por encima, ver
/// [`buscar_por_obra`].
pub const UMBRAL_SIN_PUNTUAR: i64 = 300_000;

/// Cuántos fragmentos hay indexados, por la fila más alta. No es un recuento
/// exacto -hay huecos de lo reindexado-, y no hace falta: solo decide el camino.
fn fragmentos_indexados(conn: &Connection) -> i64 {
    conn.query_row("SELECT coalesce(max(rowid), 0) FROM text_chunks", [], |f| {
        f.get(0)
    })
    .unwrap_or(0)
}

/// La biblioteca entera, **una coincidencia por obra y en el orden de la
/// biblioteca**, sin puntuar.
///
/// # Por qué no se ordena por relevancia
///
/// Porque con la biblioteca real no termina. Medido el 2026-11-13 con el índice
/// completo -39 millones de fragmentos de 152.074 obras-: `ORDER BY bm25` sobre
/// "amor", "la casa" o "que" no había contestado a los **dos minutos**, y
/// "molinos de viento" tardó 15 s. Para ordenar hay que puntuar cada fragmento
/// que empareja, y son millones. Las cifras de 0,45 s eran con el 13,7 %
/// indexado.
///
/// Sin ordenar, FTS5 entrega las coincidencias por fila y se detiene en cuanto
/// tiene las que se le piden: las mismas consultas, de 30 a 280 ms.
///
/// # Por qué una por obra
///
/// Porque los fragmentos de una obra van seguidos, y sin esto las veinte
/// primeras coincidencias de "amor" serían veinte párrafos del mismo libro. Tras
/// cada coincidencia se salta al final de su obra, así que cada resultado es una
/// obra distinta: la respuesta pasa de "los mejores párrafos" a "en qué obras
/// aparece", que es además lo que se puede contestar con honradez a esta escala.
/// Dentro de cada obra, la búsqueda del lector las enseña todas.
///
/// Cota: las coincidencias que hagan falta para reunir `desde + limite` obras,
/// hasta [`MAXIMO_DE_FILAS_RECORRIDAS`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla, o [`NbError::Cancelled`] si
/// otra búsqueda la interrumpe.
pub fn buscar_por_obra(
    conn: &Connection,
    fts: &str,
    limite: usize,
    desde: usize,
) -> NbResult<Vec<Fila>> {
    /*
     * Una sola consulta que se recorre, y no una por obra. Preparar la consulta
     * es lo caro -hay que abrir la lista de cada término-, y repetirlo veinte
     * veces convertía 15 ms en varios segundos. Las filas de una obra ya vista se
     * saltan aquí, que es barato: son enteros que salen ordenados.
     */
    let mut coincidencias = conn
        .prepare_cached(
            "SELECT rowid FROM documents_fts WHERE documents_fts MATCH ?1 ORDER BY rowid",
        )
        .map_err(|e| fallo(&e))?;
    let mut fragmento = conn
        .prepare_cached(
            "SELECT c.edition_id, c.locator,
                    (SELECT min(a.id) FROM files a
                      WHERE a.edition_id = c.edition_id
                        AND a.availability = 'available'),
                    (SELECT e.title FROM editions e WHERE e.id = c.edition_id)
               FROM text_chunks c WHERE c.rowid = ?1",
        )
        .map_err(|e| fallo(&e))?;
    let mut fin_de_obra = conn
        .prepare_cached("SELECT max(rowid) FROM text_chunks WHERE edition_id = ?1")
        .map_err(|e| fallo(&e))?;

    let mut siguiente_obra = conn.prepare_cached(
        "SELECT min(rowid) FROM text_chunks WHERE rowid > ?1 AND rowid <= ?2 AND edition_id != ?3",
    ).map_err(|e| fallo(&e))?;
    let mut vistas = std::collections::HashSet::new();
    let mut filas = Vec::with_capacity(limite);
    let mut cursor = 0_i64;
    let mut recorridas = 0_u32;
    let mut filas_fts = coincidencias.query([fts]).map_err(|e| fallo(&e))?;
    while let Some(fila) = filas_fts.next().map_err(|e| fallo(&e))? {
        // Cota del trabajo: una consulta que empareja en casi todo no puede
        // recorrer la biblioteca entera buscando la obra siguiente.
        recorridas += 1;
        if filas.len() >= limite || recorridas > MAXIMO_DE_FILAS_RECORRIDAS {
            break;
        }
        let fila_fts: i64 = fila.get(0).map_err(|e| fallo(&e))?;
        if fila_fts <= cursor {
            continue;
        }
        let datos = fragmento
            .query_row([fila_fts], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })
            .optional()
            .map_err(|e| fallo(&e))?;
        let Some((edicion, locator, archivo, titulo)) = datos else {
            continue;
        };
        let fin: i64 = fin_de_obra
            .query_row([&edicion], |f| f.get(0))
            .map_err(|e| fallo(&e))?;
        // No se presume que todas las filas del intervalo sean de la misma obra.
        // Un índice importado o reparado puede intercalar otras ediciones.
        let otra: Option<i64> = siguiente_obra
            .query_row(rusqlite::params![fila_fts, fin, &edicion], |row| row.get(0))
            .map_err(|e| fallo(&e))?;
        cursor = otra.map_or(fin, |fila| fila - 1).max(fila_fts);
        if vistas.insert(edicion.clone()) && vistas.len() > desde {
            filas.push((edicion, locator, 0.0, archivo, titulo));
        }
    }
    Ok(filas)
}

/// Cuántas coincidencias recorre como mucho [`buscar_por_obra`].
const MAXIMO_DE_FILAS_RECORRIDAS: u32 = 400_000;

/// Quita el prefijo del último término si es corto.
///
/// Mientras se escribe, el último término busca por prefijo. Con la biblioteca
/// grande, un prefijo corto es carísimo: para "que*" hay que fundir las listas
/// de todas las palabras que empiezan así. Medido sobre el índice real, 11 s
/// frente a 15 ms del término exacto. Con cinco letras o más ya son pocas
/// palabras y se conserva.
fn sin_prefijo_corto(fts: &str) -> String {
    let Some(sin_asterisco) = fts.strip_suffix("\"*") else {
        return fts.to_owned();
    };
    let ultimo = sin_asterisco.rsplit('"').next().unwrap_or_default();
    if ultimo.chars().count() >= 5 {
        fts.to_owned()
    } else {
        format!("{sin_asterisco}\"")
    }
}

fn hits_de(filas: Vec<Fila>) -> NbResult<Vec<Hit>> {
    let resultados: Vec<Hit> = filas
        .into_iter()
        .map(|(edition_id, locator, rank, file_id, titulo)| {
            Ok(Hit {
                edition_id,
                file_id,
                locator: serde_json::from_str(&locator).map_err(|_| {
                    tracing::error!("un fragmento indexado tiene el locator ilegible");
                    NbError::StorageFailure
                })?,
                // Vacío hasta que alguien lo rellene con el documento delante.
                snippet: String::new(),
                rank,
                title: titulo.unwrap_or_default(),
            })
        })
        .collect::<NbResult<Vec<_>>>()?;

    tracing::info!(resultados = resultados.len(), "búsqueda resuelta");
    Ok(resultados)
}

fn fallo(error: &rusqlite::Error) -> NbError {
    // Una consulta interrumpida no es un fallo del índice: la cortó una búsqueda
    // más nueva, y registrarla como error llenaría el registro de avisos falsos
    // cada vez que alguien escribe deprisa. Ver `BusquedaEnCurso`.
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::OperationInterrupted) {
        tracing::debug!("una búsqueda se interrumpió porque llegó otra");
        return NbError::Cancelled;
    }
    tracing::error!(error = %error, "fallo en el índice de búsqueda");
    NbError::StorageFailure
}

// ---------------------------------------------------------------------------
// Una búsqueda interactiva a la vez
// ---------------------------------------------------------------------------

/// La búsqueda de la barra superior que está corriendo, para poder cortarla.
///
/// # Por qué existe
///
/// Porque la guarda de respuesta obsoleta de la interfaz descarta lo que llega
/// tarde, **pero no lo para**. Medido el 2026-09-12 sobre la base real: buscar
/// dentro de los libros "amor" cuesta 0,45 s, "amo*" 0,83 s y "el amor*" 1,67 s,
/// sin contar releer los pasajes. Quien escribe "el amor" despacio lanza tres o
/// cuatro de esas, y el pool tiene **cuatro** conexiones: la biblioteca, las
/// portadas y la propia búsqueda siguiente esperaban detrás de consultas cuya
/// respuesta nadie iba a mirar.
///
/// Esto no se arregla con más pausa al teclear: la pausa hace menos peticiones,
/// no corta las que ya salieron. Lo que hace falta es `sqlite3_interrupt` sobre
/// la conexión de la anterior.
///
/// # Por qué la interrupción no puede alcanzar a quien no toca
///
/// Una interrupción va a una **conexión**, y las conexiones vuelven al pool. Si
/// se interrumpiera una conexión que ya atiende otro comando, se le cortaría a
/// ese. Por eso el turno se retira bajo el mismo cerrojo con el que se
/// interrumpe, y se retira **antes** de soltar la conexión: quien declara el
/// turno después de la conexión consigue ese orden sin pensarlo, porque las
/// variables locales se sueltan al revés de como se declararon.
#[derive(Default)]
pub struct BusquedaEnCurso {
    actual: Mutex<Option<(u64, rusqlite::InterruptHandle)>>,
    siguiente: std::sync::atomic::AtomicU64,
}

/// El turno de una búsqueda. Mientras vive, una búsqueda nueva lo interrumpe.
pub struct Turno<'a> {
    dueno: &'a BusquedaEnCurso,
    id: u64,
}

impl BusquedaEnCurso {
    /// Toma el turno sobre esta conexión, interrumpiendo a quien lo tuviera.
    pub fn empezar<'a>(&'a self, conn: &Connection) -> Turno<'a> {
        let id = self.siguiente.fetch_add(1, Ordering::SeqCst) + 1;
        let mut actual = self.actual.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((_, anterior)) = actual.take() {
            anterior.interrupt();
        }
        *actual = Some((id, conn.get_interrupt_handle()));
        Turno { dueno: self, id }
    }
}

impl Turno<'_> {
    /// Si nadie ha empezado otra búsqueda después de esta.
    #[must_use]
    pub fn vigente(&self) -> bool {
        self.dueno.siguiente.load(Ordering::SeqCst) == self.id
    }
}

impl Drop for Turno<'_> {
    fn drop(&mut self) {
        let mut actual = self.dueno.actual.lock().unwrap_or_else(|p| p.into_inner());
        if actual.as_ref().is_some_and(|(id, _)| *id == self.id) {
            *actual = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Indexación en segundo plano
// ---------------------------------------------------------------------------

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::db::Database;
use crate::events::{DomainEvent, EventBus};
use crate::power;
use crate::reader::ProviderRegistry;

/// Motivo canónico por el que la indexación se ha detenido sola.
pub const PAUSA_BATERIA: &str = "LOW_BATTERY";
/// Motivo canónico por el que la ha detenido el usuario.
pub const PAUSA_USUARIO: &str = "USER_PAUSED";
/// La indexación cede el paso mientras entra una importación masiva.
///
/// Se distingue de la pausa del usuario porque al terminar la importación solo
/// se reanuda lo que paró la importación: si alguien la había pausado a mano,
/// sigue pausada.
pub const PAUSA_IMPORTACION: &str = "IMPORT_IN_PROGRESS";

/// El indexador se detuvo porque lo derivado llegó al presupuesto de disco.
///
/// Es el mecanismo del innegociable "lo derivado nunca pesa más que la
/// biblioteca que describe", que era un número escrito y nada más.
/// Ver `docs/architecture/performance-budgets.md`, sección 4.4.
///
/// Se distingue de las otras tres pausas a propósito: no es algo que pase y se
/// arregle solo -como la batería- ni algo que alguien pidió -como la del
/// usuario-. Es un límite alcanzado, y lo que hay que hacer con él es distinto:
/// o se sube el presupuesto, o se libera sitio, o se acepta que la búsqueda
/// cubre lo que cabe. La interfaz tiene que poder decir cuál de las tres.
/// Ahora es además el código de [`NbError::StorageBudgetReached`], y la
/// prueba `la_pausa_y_el_error_del_presupuesto_son_el_mismo_nombre` lo ata: la
/// misma condición la alcanzan dos subsistemas -el indexador pausando y la
/// preparación de audio negándose-, y si cada uno la llamara distinto la interfaz
/// tendría que traducir dos veces lo mismo.
pub const PAUSA_PRESUPUESTO: &str = "STORAGE_BUDGET_REACHED";

/// Cada cuántos documentos se vuelve a mirar lo que ocupa lo derivado.
///
/// Medirlo recorre la caché entera, así que no se hace por documento: con
/// fragmentos de 1.920 caracteres, veinte documentos mueven el disco lo bastante
/// poco como para que mirar cada veinte no deje pasar de largo el límite, y lo
/// bastante como para que la medida no cueste más que lo que vigila.
const CADA_CUANTOS_SE_MIDE: u32 = 20;

/// Cada cuánto se vuelve a mirar la alimentación estando en pausa por batería.
const ESPERA_BATERIA: std::time::Duration = std::time::Duration::from_secs(30);

/// Avance de la indexación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndexProgress {
    pub pending: u32,
    pub indexed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub paused: bool,
    pub pause_reason: Option<&'static str>,
}

#[derive(Default)]
struct Cola {
    pendientes: VecDeque<String>,
    /// Lo que se abrió y todavía no se puede buscar. Va delante de todo y **no
    /// espera al presupuesto**: la búsqueda cubre siempre lo que se ha leído, que
    /// es lo que prometió la decisión y no se cumplía.
    prioritarios: VecDeque<String>,
    /// Alguien pidió volver a mirar lo que ocupa lo derivado: cambió el tope, o
    /// se liberó sitio. Despierta al indexador aunque estuviera esperando.
    revisar: bool,
    /// Cierto cuando hay que terminar. No se vacía la cola: lo que quede se
    /// recupera al arrancar, porque la cola no es la fuente de verdad.
    detener: bool,
}

/// Indexa documentos en un hilo aparte, sin bloquear a nadie.
///
/// # Por qué no hay cursor guardado
///
/// La cola vive en memoria y se pierde al cerrar. No importa: al arrancar se
/// vuelven a encolar las ediciones que **no tienen fragmentos**, y eso se sabe
/// consultando la base. Un cursor persistido sería un dato más que mantener
/// coherente, y el primer fallo de coherencia dejaría documentos sin indexar sin
/// que nadie lo notara. Es la misma decisión que en la importación masiva.
///
/// # Por qué mira la batería
///
/// Indexar es la tarea más cara de la aplicación. Hacerlo con el 8 % de batería
/// le gasta la tarde a alguien por un trabajo que puede esperar. Ver
/// `crate::power`.
pub struct Indexer {
    estado: Arc<Compartido>,
    hilo: Option<std::thread::JoinHandle<()>>,
}

/// Lo que comparten el indexador y su hilo.
///
/// Junto en una estructura y no en seis `Arc` sueltos: seis parámetros por
/// función acaban pasándose en el orden equivocado el día que se añade el
/// séptimo, y el compilador no tiene forma de avisar cuando tres de ellos son
/// del mismo tipo.
#[derive(Default)]
struct Compartido {
    cola: (Mutex<Cola>, Condvar),
    pausado: AtomicBool,
    indexados: AtomicU32,
    omitidos: AtomicU32,
    fallidos: AtomicU32,
    motivo: Mutex<Option<&'static str>>,
    /// La última medida de lo derivado, para enseñarla sin volver a recorrer la
    /// caché cada vez que la interfaz pregunta.
    medida: Mutex<Option<crate::diagnostics::DerivedStorage>>,
    /// Si la última vez se paró por el presupuesto: el aviso se registra al
    /// pararse y al reanudar, **no en cada vuelta**. Antes salía cada
    /// treinta segundos, idéntico, toda la tarde.
    sin_sitio: AtomicBool,
}

impl Indexer {
    /// Arranca el indexador con su hilo.
    #[must_use]
    pub fn start(
        db: Database,
        registry: Arc<ProviderRegistry>,
        bus: EventBus,
        paths: crate::paths::AppPaths,
    ) -> Self {
        let estado = Arc::new(Compartido::default());

        let hilo = std::thread::Builder::new()
            .name("nb-indexador".to_owned())
            .spawn({
                let estado = Arc::clone(&estado);
                move || trabajar(&db, &registry, &bus, &estado, &paths)
            })
            .ok();

        Self { estado, hilo }
    }

    /// Encola una edición. Encolar la misma dos veces no la indexa dos veces.
    pub fn enqueue(&self, edition_id: &str) {
        let (cola, aviso) = &self.estado.cola;
        let mut guardia = cola.lock().unwrap_or_else(|p| p.into_inner());
        if !guardia.pendientes.iter().any(|id| id == edition_id) {
            guardia.pendientes.push_back(edition_id.to_owned());
        }
        aviso.notify_all();
    }

    /// Encola con prioridad algo que se acaba de abrir.
    ///
    /// Va delante de todo lo demás y no espera al presupuesto de disco: quien
    /// abre un libro espera poder buscar dentro de él, y un libro son unos
    /// cientos de kilobytes de índice.
    pub fn enqueue_prioritario(&self, edition_id: &str) {
        let (cola, aviso) = &self.estado.cola;
        let mut guardia = cola.lock().unwrap_or_else(|p| p.into_inner());
        guardia.pendientes.retain(|id| id != edition_id);
        if !guardia.prioritarios.iter().any(|id| id == edition_id) {
            guardia.prioritarios.push_front(edition_id.to_owned());
        }
        aviso.notify_all();
    }

    /// Vuelve a mirar el presupuesto ahora, sin esperar a la siguiente vuelta.
    ///
    /// Lo piden quien cambia el tope y quien libera sitio: sin esto, un
    /// indexador parado esperaría diez minutos a enterarse.
    pub fn revisar_presupuesto(&self) {
        let (cola, aviso) = &self.estado.cola;
        cola.lock().unwrap_or_else(|p| p.into_inner()).revisar = true;
        aviso.notify_all();
    }

    /// Lo que hay en la cola, en el orden en que se va a indexar. Solo para las
    /// pruebas que vigilan ese orden.
    #[doc(hidden)]
    #[must_use]
    pub fn cola_pendiente(&self) -> Vec<String> {
        let cola = self.estado.cola.0.lock().unwrap_or_else(|p| p.into_inner());
        cola.prioritarios
            .iter()
            .chain(cola.pendientes.iter())
            .cloned()
            .collect()
    }

    /// La última medida de lo derivado, si ya se tomó alguna.
    #[must_use]
    pub fn medida(&self) -> Option<crate::diagnostics::DerivedStorage> {
        *self.estado.medida.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Encola todo lo que falte por indexar.
    ///
    /// Es lo que hace la indexación reanudable sin llevar cursor: se pregunta a
    /// la base qué ediciones no tienen fragmentos, y esas son las que faltan.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn enqueue_missing(&self, conn: &Connection) -> NbResult<usize> {
        /*
         * **En qué orden **. Si solo va a caber una
         * parte de la biblioteca, que sea la que se lee: primero lo empezado,
         * después lo que está en una colección, después lo de los autores que se
         * leen, y el resto de lo más reciente a lo más antiguo. Hasta el
         * 2026-09-12 era por fecha de importación, de lo más viejo a lo más
         * nuevo, y lo indexado era el primer quinto que se importó.
         *
         * **Y desde el estado de búsqueda, no desde los fragmentos.** Preguntar a
         * cada edición si tiene fragmentos son 152.080 búsquedas en un índice de
         * 6,5 millones de filas repartidas por un archivo de 10 GB: en la
         * instalación real, el arranque diferido tardó 5 min 49 s en contestar.
         * `search_status` ya dice qué está pendiente, con su propio índice.
         */
        let mut stmt = conn
            .prepare(
                "SELECT e.id,
                        CASE
                          WHEN EXISTS (SELECT 1 FROM progress p WHERE p.edition_id = e.id) THEN 0
                          WHEN EXISTS (SELECT 1 FROM collection_items ci WHERE ci.edition_id = e.id) THEN 1
                          WHEN e.id IN (SELECT ec.edition_id FROM edition_contributors ec
                                         WHERE ec.role = 'author'
                                           AND ec.contributor_id IN (
                                               SELECT ec2.contributor_id
                                                 FROM progress p2
                                                 JOIN edition_contributors ec2
                                                   ON ec2.edition_id = p2.edition_id
                                                WHERE ec2.role = 'author')) THEN 2
                          ELSE 3
                        END AS prioridad
                   FROM editions e
                   LEFT JOIN search_status s ON s.edition_id = e.id
                  WHERE s.edition_id IS NULL OR s.state = 'pending'
                  ORDER BY prioridad, e.created_at DESC, e.id",
            )
            .map_err(|e| fallo(&e))?;

        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| fallo(&e))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| fallo(&e))?;

        let (cola, aviso) = &self.estado.cola;
        let mut guardia = cola.lock().unwrap_or_else(|p| p.into_inner());
        for id in &ids {
            if !guardia.pendientes.iter().any(|ya| ya == id) {
                guardia.pendientes.push_back(id.clone());
            }
        }
        aviso.notify_all();
        Ok(ids.len())
    }

    /// Detiene la indexación hasta que se reanude.
    pub fn pause(&self) {
        self.estado.pausado.store(true, Ordering::SeqCst);
        *self.estado.motivo.lock().unwrap_or_else(|p| p.into_inner()) = Some(PAUSA_USUARIO);
        self.estado.cola.1.notify_all();
    }

    /// Detiene la indexación mientras dura una importación masiva.
    ///
    /// Indexar un documento escribe cerca de novecientas filas en el índice de
    /// texto completo; importar escribe unas pocas. Si las dos cosas ocurren a
    /// la vez, la que el usuario está esperando —la importación— pasa el tiempo
    /// esperando su turno de escritura detrás de la que puede esperar.
    ///
    /// El motivo se distingue del de la pausa del usuario a propósito: al
    /// terminar la importación se reanuda **solo** si nadie la había pausado a
    /// mano, y para saberlo hay que poder distinguir quién la paró.
    pub fn pause_for_import(&self) {
        self.estado.pausado.store(true, Ordering::SeqCst);
        let mut motivo = self.estado.motivo.lock().unwrap_or_else(|p| p.into_inner());
        if motivo.is_none() {
            *motivo = Some(PAUSA_IMPORTACION);
        }
        self.estado.cola.1.notify_all();
    }

    /// Reanuda tras una importación, si fue la importación quien la paró.
    pub fn resume_after_import(&self) {
        let mut motivo = self.estado.motivo.lock().unwrap_or_else(|p| p.into_inner());
        if motivo.as_deref() != Some(PAUSA_IMPORTACION) {
            // La pausó el usuario: no es nuestra para reanudarla.
            return;
        }
        *motivo = None;
        drop(motivo);
        self.estado.pausado.store(false, Ordering::SeqCst);
        self.estado.cola.1.notify_all();
    }

    /// Reanuda la indexación.
    pub fn resume(&self) {
        self.estado.pausado.store(false, Ordering::SeqCst);
        *self.estado.motivo.lock().unwrap_or_else(|p| p.into_inner()) = None;
        self.estado.cola.1.notify_all();
    }

    /// Avance actual.
    #[must_use]
    pub fn progress(&self) -> IndexProgress {
        self.estado.avance()
    }

    /// Indica si no queda nada por hacer.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.estado
            .cola
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pendientes
            .is_empty()
            && self
                .estado
                .cola
                .0
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .prioritarios
                .is_empty()
    }
}

impl Compartido {
    fn avance(&self) -> IndexProgress {
        let pendientes = {
            let cola = self.cola.0.lock().unwrap_or_else(|p| p.into_inner());
            cola.pendientes.len() + cola.prioritarios.len()
        };
        let motivo = *self.motivo.lock().unwrap_or_else(|p| p.into_inner());

        IndexProgress {
            pending: u32::try_from(pendientes).unwrap_or(u32::MAX),
            indexed: self.indexados.load(Ordering::SeqCst),
            skipped: self.omitidos.load(Ordering::SeqCst),
            failed: self.fallidos.load(Ordering::SeqCst),
            paused: self.pausado.load(Ordering::SeqCst) || motivo.is_some(),
            pause_reason: motivo,
        }
    }

    fn anunciar(&self, bus: &EventBus, paused: bool, pause_reason: Option<&'static str>) {
        let avance = self.avance();
        bus.emit(DomainEvent::IndexingProgress {
            pending: avance.pending,
            indexed: avance.indexed,
            skipped: avance.skipped,
            failed: avance.failed,
            paused,
            pause_reason,
        });
    }
}

impl Drop for Indexer {
    fn drop(&mut self) {
        {
            let (cola, aviso) = &self.estado.cola;
            cola.lock().unwrap_or_else(|p| p.into_inner()).detener = true;
            aviso.notify_all();
        }
        // Se espera al hilo: dejarlo suelto mientras la base se cierra produce
        // un error de escritura sobre una conexión muerta en cada apagado.
        if let Some(hilo) = self.hilo.take() {
            let _ = hilo.join();
        }
    }
}

/// Si lo derivado ya pasó del presupuesto de disco. Devuelve cuánto ocupa.
///
/// Mide **cada tantos documentos** y no en cada uno: la medida recorre la caché
/// entera, y hacerla por documento costaría más que lo que vigila. El contador
/// empieza en cero para que la primera vuelta sí mida: una aplicación que abre
/// con el disco ya pasado no debe indexar veinte más antes de enterarse.
fn paso_del_presupuesto(
    db: &Database,
    paths: &crate::paths::AppPaths,
    contador: &mut u32,
    ultima: &Mutex<Option<crate::diagnostics::DerivedStorage>>,
) -> Option<f64> {
    if *contador > 0 {
        *contador -= 1;
        return None;
    }
    let conn = db.connection().ok()?;
    let medida = crate::diagnostics::medir_almacenamiento(&conn, paths)?;
    *ultima.lock().unwrap_or_else(|p| p.into_inner()) = Some(medida);

    // Sin documentos no hay presupuesto que gastar: el porcentaje se mide contra
    // lo que ocupan, y contra cero no significa nada.
    let excedido =
        medida.documents_bytes > 0 && medida.percent_of_documents >= medida.budget_percent;

    /*
     * **Saltarse la medición solo se permite cuando hay margen.**
     *
     * Poner aquí el contador sin mirar -que es lo que se hacía hasta el
     * 2026-09-10- dejaba pasar veinte documentos entre frenazo y frenazo: la
     * llamada que frena reponía el contador, y las veinte siguientes ni medían
     * ni frenaban. Medido sobre la biblioteca real, el registro enseña
     * exactamente veinte documentos indexados entre cada aviso de "indexación
     * detenida", una y otra vez. Eso no es un freno, es marcar el paso, y el
     * innegociable dice que el indexador **se detiene**.
     *
     * Estando por encima se vuelve a medir en cada vuelta. Ahora una
     * vuelta así espera diez minutos o a que alguien avise -cambió el tope, se
     * liberó sitio, se abrió un libro-, en lugar de medir cada treinta segundos
     * recorriendo una caché de ciento cincuenta mil portadas.
     */
    *contador = if excedido { 0 } else { CADA_CUANTOS_SE_MIDE };

    excedido.then_some(medida.percent_of_documents)
}

fn trabajar(
    db: &Database,
    registry: &ProviderRegistry,
    bus: &EventBus,
    estado: &Compartido,
    paths: &crate::paths::AppPaths,
) {
    let mut desde_la_ultima_medida = 0;

    loop {
        let (siguiente, prioritario) = {
            let (cerrojo, aviso) = &estado.cola;
            let mut guardia = cerrojo.lock().unwrap_or_else(|p| p.into_inner());

            loop {
                if guardia.detener {
                    return;
                }
                let libre = !estado.pausado.load(Ordering::SeqCst);
                if libre && (!guardia.prioritarios.is_empty() || !guardia.pendientes.is_empty()) {
                    break;
                }
                // Con espera acotada y no indefinida: si la pausa es por batería,
                // hay que volver a mirar sin que nadie avise, porque enchufar el
                // portátil no genera ningún evento que llegue hasta aquí.
                let (nueva, _) = aviso
                    .wait_timeout(guardia, ESPERA_BATERIA)
                    .unwrap_or_else(|p| p.into_inner());
                guardia = nueva;
            }

            match guardia.prioritarios.pop_front() {
                Some(id) => (Some(id), true),
                None => (guardia.pendientes.pop_front(), false),
            }
        };

        let Some(edition_id) = siguiente else {
            continue;
        };

        // La batería se mira **antes de cada documento** y no una sola vez al
        // arrancar: una indexación larga puede empezar enchufada y terminar en
        // un tren.
        if !power::state().allows_background_work() {
            tracing::info!("indexación detenida por batería baja");
            *estado.motivo.lock().unwrap_or_else(|p| p.into_inner()) = Some(PAUSA_BATERIA);
            devolver_a_la_cola(estado, edition_id, prioritario);
            estado.anunciar(bus, true, Some(PAUSA_BATERIA));
            std::thread::sleep(ESPERA_BATERIA);
            continue;
        }

        // Y el presupuesto de disco, con la misma forma que la batería y por la
        // misma razón: es una condición que cambia mientras se trabaja. **Lo que
        // se acaba de abrir no espera al presupuesto**: la búsqueda cubre
        // siempre lo que se ha leído.
        if !prioritario {
            if let Some(ocupado) =
                paso_del_presupuesto(db, paths, &mut desde_la_ultima_medida, &estado.medida)
            {
                if !estado.sin_sitio.swap(true, Ordering::SeqCst) {
                    tracing::info!(
                        por_ciento = ocupado,
                        "indexación detenida: lo derivado llegó al presupuesto de disco"
                    );
                }
                *estado.motivo.lock().unwrap_or_else(|p| p.into_inner()) = Some(PAUSA_PRESUPUESTO);
                devolver_a_la_cola(estado, edition_id, false);
                estado.anunciar(bus, true, Some(PAUSA_PRESUPUESTO));
                esperar_sitio(estado);
                continue;
            }
            if estado.sin_sitio.swap(false, Ordering::SeqCst) {
                tracing::info!("indexación reanudada: vuelve a haber sitio para lo derivado");
            }
        }

        if !estado.pausado.load(Ordering::SeqCst) {
            *estado.motivo.lock().unwrap_or_else(|p| p.into_inner()) = None;
        }

        match indexar_una(db, registry, &edition_id) {
            Ok(true) => estado.indexados.fetch_add(1, Ordering::SeqCst),
            Ok(false) => estado.omitidos.fetch_add(1, Ordering::SeqCst),
            Err(_) => estado.fallidos.fetch_add(1, Ordering::SeqCst),
        };

        estado.anunciar(bus, false, None);
    }
}

/// Cuánto espera un indexador parado por el presupuesto antes de volver a medir.
///
/// Diez minutos. Lo que libera sitio o sube el tope **avisa** y no espera a esto
/// (ver `Indexer::revisar_presupuesto`); esto es para lo que cambia sin avisar,
/// como que el usuario borre archivos de la caché a mano.
const ESPERA_PRESUPUESTO: std::time::Duration = std::time::Duration::from_secs(600);

fn devolver_a_la_cola(estado: &Compartido, edition_id: String, prioritario: bool) {
    let (cerrojo, aviso) = &estado.cola;
    let mut cola = cerrojo.lock().unwrap_or_else(|p| p.into_inner());
    if prioritario {
        cola.prioritarios.push_front(edition_id);
    } else {
        cola.pendientes.push_front(edition_id);
    }
    aviso.notify_all();
}

/// Espera a que haya motivo para volver a mirar el presupuesto.
///
/// Sale antes del plazo si llega algo prioritario, si alguien pide revisar o si
/// hay que terminar.
fn esperar_sitio(estado: &Compartido) {
    let (cerrojo, aviso) = &estado.cola;
    let mut cola = cerrojo.lock().unwrap_or_else(|p| p.into_inner());
    let limite = std::time::Instant::now() + ESPERA_PRESUPUESTO;
    loop {
        if cola.detener || cola.revisar || !cola.prioritarios.is_empty() {
            cola.revisar = false;
            return;
        }
        let queda = limite.saturating_duration_since(std::time::Instant::now());
        if queda.is_zero() {
            return;
        }
        let (nueva, _) = aviso
            .wait_timeout(cola, queda)
            .unwrap_or_else(|p| p.into_inner());
        cola = nueva;
    }
}

/// Indexa una edición y anota si se puede buscar.
///
/// Devuelve `false` cuando no había nada que indexar, que **no** es un fallo.
fn indexar_una(db: &Database, registry: &ProviderRegistry, edition_id: &str) -> NbResult<bool> {
    let conn = db.connection()?;
    // La ruta se resuelve en la consulta: una raíz sin localizar la deja en
    // NULL, y entonces el documento se trata como no disponible, que es
    // exactamente lo que es.
    let archivo: Option<(Option<String>, String)> = conn
        .query_row(
            &format!(
                "SELECT {}, f.media_type FROM files f
                  WHERE f.edition_id = ?1 AND f.availability = 'available'
                  ORDER BY f.created_at LIMIT 1",
                crate::db::catalog::SQL_RUTA_ABSOLUTA
            ),
            [edition_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();
    drop(conn);

    let Some((Some(ruta), media_type)) = archivo else {
        // Sin archivo disponible no hay nada que indexar, y no es un fallo: el
        // volumen puede estar desconectado y volver mañana. Se anota igualmente,
        // porque la interfaz tiene que poder explicar por qué ese documento no
        // aparece en los resultados.
        anotar(db, edition_id, ARCHIVO_NO_DISPONIBLE);
        return Ok(false);
    };

    let documento = match registry.open(&media_type, std::path::Path::new(&ruta)) {
        Ok(documento) => documento,
        // El archivo está registrado y no está en el disco: la disponibilidad
        // se ha quedado vieja porque alguien desconectó un volumen. Es una
        // omisión, no un fallo; contarlo como fallo le diría al usuario que
        // tres documentos están rotos cuando lo que pasa es que el disco no
        // está enchufado, y `refresh_availability` lo arreglará solo.
        Err(NbError::FileNotFound) => {
            tracing::info!("un archivo registrado no está en el disco al indexar");
            anotar(db, edition_id, ARCHIVO_NO_DISPONIBLE);
            return Ok(false);
        }
        // Sin proveedor para ese formato tampoco hay nada que indexar, y no es
        // culpa del documento: es esta instalación la que no puede.
        Err(NbError::ComponentUnavailable) => {
            anotar(db, edition_id, SIN_MOTOR);
            return Ok(false);
        }
        Err(otro) => return Err(otro),
    };
    let mut conn = db.connection()?;

    match index_edition(&mut conn, edition_id, documento.as_ref()) {
        Ok(_) => {
            let _ = set_status(&conn, edition_id, &SearchStatus::Searchable);
            Ok(true)
        }
        // Un documento sin texto no es un fallo de la indexación: es un cómic o
        // un escaneo sin OCR. El motivo no se puede deducir del error, que dice
        // lo mismo en los dos casos, así que se deduce del formato: un cómic no
        // tendrá texto nunca y un escaneo lo tendría con OCR.
        Err(NbError::FormatUnsupported) => {
            let _ = set_status(
                &conn,
                edition_id,
                &SearchStatus::NotSearchable {
                    reason: motivo_por_formato(&media_type).to_owned(),
                },
            );
            Ok(false)
        }
        Err(otro) => Err(otro),
    }
}

/// Anota un motivo de no-buscable sin dejar que un fallo al anotarlo detenga la
/// indexación del resto de la biblioteca.
fn anotar(db: &Database, edition_id: &str, motivo: &str) {
    let Ok(conn) = db.connection() else { return };
    let _ = set_status(
        &conn,
        edition_id,
        &SearchStatus::NotSearchable {
            reason: motivo.to_owned(),
        },
    );
}

/// Conecta el indexador al bus.
///
/// Lo que entra se encola, y lo que se da de baja se retira del índice en el
/// acto: un índice que sobrevive al documento devuelve resultados que no se
/// pueden abrir.
pub fn subscribe(bus: &EventBus, db: Database, indexador: Arc<Indexer>) {
    bus.subscribe("search-indexer", move |evento| match evento {
        DomainEvent::DocumentImported { edition_id, .. } => indexador.enqueue(edition_id),
        DomainEvent::DocumentDeleted { edition_id } => {
            let Ok(conn) = db.connection() else {
                tracing::warn!("sin conexión para retirar un documento del índice");
                return;
            };
            if let Err(error) = forget_edition(&conn, edition_id) {
                tracing::error!(code = error.code(), "no se pudo retirar del índice");
            }
        }
        _ => {}
    });
}

// ---------------------------------------------------------------------------
// Documentos no indexables
// ---------------------------------------------------------------------------

/// Un cómic: es imagen, y nunca tendrá texto.
pub const SIN_TEXTO_IMAGEN: &str = "IMAGE_ONLY";
/// Un escaneo sin OCR. Es el único motivo que tiene arreglo.
pub const SIN_CAPA_DE_TEXTO: &str = "NO_TEXT_LAYER";
/// El archivo no está accesible ahora mismo.
pub const ARCHIVO_NO_DISPONIBLE: &str = "FILE_UNAVAILABLE";
/// Esta instalación no tiene con qué abrir ese formato.
pub const SIN_MOTOR: &str = "NO_ENGINE";

/// Si un documento se puede buscar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "state")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum SearchStatus {
    /// Todavía no se ha intentado indexar.
    ///
    /// Distinto de "no se puede": mientras la biblioteca se indexa, la interfaz
    /// dice "aún no", que es verdad, en lugar de "no", que sería mentira.
    Pending,
    Searchable,
    /// No se puede buscar, y el motivo dice qué ofrecerle al usuario.
    NotSearchable {
        reason: String,
    },
}

impl SearchStatus {
    /// Indica si el motivo tiene arreglo con OCR.
    ///
    /// Solo el escaneo sin capa de texto. Ofrecer OCR sobre un cómic sería
    /// ofrecer convertir en texto algo que son dibujos.
    #[must_use]
    pub fn is_fixable_with_ocr(&self) -> bool {
        matches!(self, Self::NotSearchable { reason } if reason == SIN_CAPA_DE_TEXTO)
    }
}

/// Anota si una edición se puede buscar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn set_status(conn: &Connection, edition_id: &str, estado: &SearchStatus) -> NbResult<()> {
    let (state, reason) = match estado {
        SearchStatus::Pending => ("pending", None),
        SearchStatus::Searchable => ("searchable", None),
        SearchStatus::NotSearchable { reason } => ("not_searchable", Some(reason.as_str())),
    };

    conn.execute(
        "INSERT INTO search_status (edition_id, state, reason) VALUES (?1, ?2, ?3)
         ON CONFLICT (edition_id) DO UPDATE
           SET state = excluded.state,
               reason = excluded.reason,
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![edition_id, state, reason],
    )
    .map_err(|e| fallo(&e))?;

    Ok(())
}

/// Estado de búsqueda de una edición.
///
/// Una edición de la que no se sabe nada está **pendiente**, no "no buscable":
/// mientras la biblioteca se indexa, decir "no" sería mentir.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn status_of(conn: &Connection, edition_id: &str) -> NbResult<SearchStatus> {
    let fila: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT state, reason FROM search_status WHERE edition_id = ?1",
            [edition_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| fallo(&e))?;

    Ok(match fila {
        Some((estado, motivo)) => match estado.as_str() {
            "searchable" => SearchStatus::Searchable,
            "not_searchable" => SearchStatus::NotSearchable {
                reason: motivo.unwrap_or_else(|| SIN_CAPA_DE_TEXTO.to_owned()),
            },
            _ => SearchStatus::Pending,
        },
        None => SearchStatus::Pending,
    })
}

/// Ediciones que no se pueden buscar, con su motivo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn not_searchable(conn: &Connection) -> NbResult<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare(
            "SELECT edition_id, reason FROM search_status
              WHERE state = 'not_searchable' ORDER BY edition_id",
        )
        .map_err(|e| fallo(&e))?;

    let filas: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| fallo(&e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))?;
    Ok(filas)
}

/// Traduce el tipo de medio de un documento sin texto a su motivo.
///
/// El motivo no se puede deducir del error: `FORMAT_UNSUPPORTED` significa lo
/// mismo para un cómic que para un escaneo, y son cosas distintas para el
/// usuario. Un cómic no tendrá texto nunca; un escaneo lo tendría con OCR.
#[must_use]
pub fn motivo_por_formato(media_type: &str) -> &'static str {
    if media_type == crate::reader::comic::MEDIA_TYPE {
        SIN_TEXTO_IMAGEN
    } else {
        SIN_CAPA_DE_TEXTO
    }
}

// ---------------------------------------------------------------------------
// Búsqueda por alcance
// ---------------------------------------------------------------------------

/// Dónde buscar.
///
/// Los tres alcances de `docs/architecture/search.md`, sección 1.
/// No es un filtro sobre un mismo resultado: son consultas distintas contra
/// índices distintos, porque buscar un libro y buscar dentro de un libro son dos
/// preguntas que ni siquiera devuelven la misma clase de cosa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "scope")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum Scope {
    /// Título, autor y materias. Responde mientras se escribe.
    Catalog,
    /// El texto del documento abierto.
    ///
    /// Lleva el identificador en un campo con nombre y no como variante de un
    /// solo valor: una variante así, dentro de una enumeración etiquetada por
    /// dentro, **no se puede serializar**, y el fallo aparecía al usarla, no al
    /// compilar. Lo encontró la prueba que ahora lo fija.
    #[serde(rename_all = "camelCase")]
    Document { edition_id: String },
    /// El texto de todo lo indexado, opcionalmente acotado a una colección.
    #[serde(rename_all = "camelCase")]
    Corpus { collection_id: Option<String> },
}

/// Un resultado de catálogo: un documento, no un párrafo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct CatalogHit {
    pub edition_id: String,
    /// El archivo con el que se abre, si hay alguno disponible. Ver [`Hit`].
    pub file_id: Option<String>,
    pub title: String,
    /// Quién la firma, para distinguir tres "Cervantes" en la lista.
    pub authors: String,
    /// El título con la coincidencia marcada, generado en SQL.
    pub snippet: String,
    pub rank: f64,
}

/// Lo que devuelve una búsqueda, según dónde se buscara.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", content = "hits")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum Results {
    Catalog(Vec<CatalogHit>),
    Text(Vec<Hit>),
}

impl Results {
    /// Cuántos resultados hay.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Catalog(v) => v.len(),
            Self::Text(v) => v.len(),
        }
    }

    /// Si no hay ninguno.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Busca en el alcance que se le diga.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn search(
    conn: &Connection,
    consulta: &str,
    scope: &Scope,
    limite: usize,
    desde: usize,
) -> NbResult<Results> {
    match scope {
        Scope::Catalog => Ok(Results::Catalog(buscar_catalogo(
            conn, consulta, limite, desde,
        )?)),
        Scope::Document { edition_id } => Ok(Results::Text(buscar(
            conn,
            consulta,
            Some(edition_id),
            limite,
            desde,
        )?)),
        Scope::Corpus { collection_id } => match collection_id {
            None => Ok(Results::Text(buscar(conn, consulta, None, limite, desde)?)),
            Some(coleccion) => Ok(Results::Text(buscar_en_coleccion(
                conn, consulta, coleccion, limite, desde,
            )?)),
        },
    }
}

fn buscar_catalogo(
    conn: &Connection,
    consulta: &str,
    limite: usize,
    desde: usize,
) -> NbResult<Vec<CatalogHit>> {
    let Some(fts) = to_fts_query(consulta) else {
        return Ok(Vec::new());
    };
    let encontrados = consultar_catalogo(conn, &fts, consulta, limite, desde)?;
    if !encontrados.is_empty() || desde > 0 {
        return Ok(encontrados);
    }
    // **Sin nada, se perdona una errata**. "quijte" no encontraba nada en
    // una biblioteca con diez Quijotes. Cada término se recorta a su raíz -dos
    // letras menos, nunca por debajo del mínimo de prefijo- y se busca como
    // prefijo. Solo cuando la búsqueda exacta no dio nada: si dio algo, eso manda.
    match to_fts_query_tolerante(consulta) {
        Some(tolerante) if tolerante != fts => {
            consultar_catalogo(conn, &tolerante, consulta, limite, desde)
        }
        _ => Ok(encontrados),
    }
}

/// La consulta con cada término recortado a prefijo, para tolerar una errata.
fn to_fts_query_tolerante(entrada: &str) -> Option<String> {
    let terminos: Vec<String> = entrada
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let letras: Vec<char> = t.chars().collect();
            let largo = letras
                .len()
                .saturating_sub(2)
                .max(MINIMO_PREFIJO)
                .min(letras.len());
            let raiz: String = letras[..largo].iter().collect();
            format!("\"{}\"*", raiz.replace('"', "\"\""))
        })
        .collect();
    (!terminos.is_empty()).then(|| terminos.join(" "))
}

fn consultar_catalogo(
    conn: &Connection,
    fts: &str,
    consulta: &str,
    limite: usize,
    desde: usize,
) -> NbResult<Vec<CatalogHit>> {
    /*
     * El orden. Antes era solo bm25 con el título diez veces por encima del
     * autor: "cervantes" traía libros titulados *Cervantes* y no los de Cervantes,
     * y "quijote" no ponía *Don Quijote de la Mancha* entre los veinte primeros
     * porque un título largo puntúa menos que uno corto. Ahora va primero el
     * título idéntico a lo escrito, después lo que firma el autor que se
     * escribió, después el título que empieza por ello, y bm25 con título y autor al mismo peso desempata.
     */
    let mut stmt = conn
        .prepare(
            "SELECT f.edition_id, e.title,
                    snippet(catalog_fts, 1, ?1, ?2, '…', 10),
                    bm25(catalog_fts, 10.0, 10.0, 1.0),
                    (SELECT min(a.id) FROM files a
                      WHERE a.edition_id = f.edition_id
                        AND a.availability = 'available') AS archivo,
                    coalesce(f.authors, '') AS autores
               FROM catalog_fts f
               JOIN editions e ON e.id = f.edition_id
              WHERE catalog_fts MATCH ?3
              ORDER BY lower(e.title) = lower(?6) DESC,
                       lower(coalesce(f.authors, '')) LIKE '%' || lower(?6) || '%' DESC,
                       lower(e.title) LIKE lower(?6) || '%' DESC,
                       bm25(catalog_fts, 10.0, 10.0, 1.0)
              LIMIT ?4 OFFSET ?5",
        )
        .map_err(|e| fallo(&e))?;

    let filas: Vec<CatalogHit> = stmt
        .query_map(
            rusqlite::params![
                MARCA_INICIO,
                MARCA_FIN,
                fts,
                i64::try_from(limite).unwrap_or(i64::MAX),
                i64::try_from(desde).unwrap_or(0),
                consulta.trim()
            ],
            |row| {
                Ok(CatalogHit {
                    edition_id: row.get(0)?,
                    title: row.get(1)?,
                    snippet: row.get(2)?,
                    rank: row.get(3)?,
                    file_id: row.get(4)?,
                    authors: row.get(5)?,
                })
            },
        )
        .map_err(|e| fallo(&e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))?;

    Ok(filas)
}

fn buscar_en_coleccion(
    conn: &Connection,
    consulta: &str,
    collection_id: &str,
    limite: usize,
    desde: usize,
) -> NbResult<Vec<Hit>> {
    let Some(fts) = to_fts_query(consulta) else {
        return Ok(Vec::new());
    };

    // El acotado por colección se hace **en SQL**. Traerse la colección entera al
    // frontend para cruzarla allí con los resultados es lo que
    // `performance-budgets.md` prohíbe expresamente: una colección de veinte mil
    // libros viajaría por el puente en cada pulsación de tecla.
    //
    // Las colecciones por regla no guardan miembros, así que aquí se resuelven
    // las manuales; acotar por una colección por regla es F2, cuando el
    // compilador de reglas se pueda reutilizar desde aquí.
    let mut stmt = conn
        .prepare(
            "SELECT c.edition_id, c.locator, bm25(documents_fts),
                    (SELECT min(a.id) FROM files a
                      WHERE a.edition_id = c.edition_id
                        AND a.availability = 'available') AS archivo,
                    (SELECT e.title FROM editions e WHERE e.id = c.edition_id) AS titulo
               FROM documents_fts
               JOIN text_chunks c ON c.rowid = documents_fts.rowid
               JOIN collection_items ci ON ci.edition_id = c.edition_id
              WHERE documents_fts MATCH ?1 AND ci.collection_id = ?4
              ORDER BY bm25(documents_fts)
              LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| fallo(&e))?;

    let filas: Vec<Fila> = stmt
        .query_map(
            rusqlite::params![
                fts,
                i64::try_from(limite).unwrap_or(i64::MAX),
                i64::try_from(desde).unwrap_or(0),
                collection_id
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|e| fallo(&e))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| fallo(&e))?;

    filas
        .into_iter()
        .map(|(edition_id, locator, rank, file_id, titulo)| {
            Ok(Hit {
                edition_id,
                file_id,
                locator: serde_json::from_str(&locator).map_err(|_| NbError::StorageFailure)?,
                // El pasaje lo compone `con_pasajes`, con el documento delante.
                snippet: String::new(),
                rank,
                title: titulo.unwrap_or_default(),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Reconstrucción del índice
// ---------------------------------------------------------------------------

/// Salud del índice invertido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexHealth {
    Healthy,
    /// FTS5 dice que sus estructuras internas no cuadran.
    Corrupt,
}

/// Qué hizo falta para dejar el índice en pie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repair {
    /// Estaba bien; no se tocó nada.
    NotNeeded,
    /// Se reconstruyó el índice invertido **desde los fragmentos ya guardados**,
    /// sin volver a abrir ni un documento. Cuestión de segundos.
    Rebuilt,
    /// Los fragmentos también estaban perdidos: hay que releer los documentos.
    /// Se vacía todo y la indexación de fondo lo rehará.
    NeedsFullReindex,
}

/// Cuántas sondas se leen al comprobar el índice del texto.
///
/// Dieciséis, repartidas por todo el rango de identificadores de fragmento, que
/// es lo que las reparte por segmentos distintos del índice: una corrupción
/// parcial suele afectar a los últimos que se escribieron, no a todos. Cada una
/// cuesta una búsqueda de término exacto: dieciséis búsquedas **además** del
/// `integrity-check`, que sigue haciéndose y sigue costando lo que ocupe el
/// índice. La sonda no lo sustituye; ve lo que aquel no ve.
///
/// No son todas a propósito. Comprobarlas todas volvería a hacer el arranque
/// proporcional al tamaño del índice, y lo que hace que el recorrido completo se
/// pueda pagar es que solo ocurre tras un cierre sucio.
const SONDAS_QUE_SE_LEEN: usize = 16;

/// Toma unas cuantas sondas repartidas por el rango de fragmentos indexados.
///
/// Se eligen por posición y no al azar porque `ORDER BY random()` obliga a
/// recorrer la tabla entera, y sobre el corpus real son ciento cincuenta mil
/// filas para elegir dieciséis. Cada sonda de aquí es una búsqueda por índice.
///
/// Una sonda cuyo fragmento ya no existe se salta en vez de contarse como
/// fallo: eso no es corrupción del índice, es una edición que se retiró.
fn sondas_repartidas(conn: &Connection) -> NbResult<Vec<(String, i64)>> {
    let rango = conn
        .query_row(
            "SELECT min(chunk_rowid), max(chunk_rowid) FROM index_probes",
            [],
            |f| Ok((f.get::<_, Option<i64>>(0)?, f.get::<_, Option<i64>>(1)?)),
        )
        .map_err(|e| fallo(&e))?;
    let (Some(minimo), Some(maximo)) = rango else {
        return Ok(Vec::new());
    };

    let mut sentencia = conn
        .prepare(
            "SELECT p.term, p.chunk_rowid
               FROM index_probes p
              WHERE p.chunk_rowid >= ?1
                AND EXISTS (SELECT 1 FROM text_chunks c WHERE c.rowid = p.chunk_rowid)
              ORDER BY p.chunk_rowid
              LIMIT 1",
        )
        .map_err(|e| fallo(&e))?;

    let ultimo = SONDAS_QUE_SE_LEEN.saturating_sub(1).max(1) as i64;
    let mut sondas: Vec<(String, i64)> = Vec::new();
    for paso in 0..=ultimo {
        let objetivo = minimo + (maximo - minimo) * paso / ultimo;
        let sonda = sentencia
            .query_row([objetivo], |f| {
                Ok((f.get::<_, String>(0)?, f.get::<_, i64>(1)?))
            })
            .ok();
        if let Some(sonda) = sonda {
            if !sondas.contains(&sonda) {
                sondas.push(sonda);
            }
        }
    }
    Ok(sondas)
}

/// Si el índice sigue devolviendo la sonda donde la sonda dice que está.
///
/// Cualquier respuesta que no sea esa -ninguna fila, u otro identificador, o un
/// error de SQLite- significa que el índice ya no contiene lo que se le metió.
fn sonda_responde(conn: &Connection, termino: &str, fila: i64) -> bool {
    // Entrecomillado porque una consulta FTS5 tiene sintaxis propia; el término
    // es alfanumérico por construcción, así que no puede cerrar la comilla.
    let consulta = format!("\"{termino}\"");
    conn.query_row(
        "SELECT 1 FROM documents_fts WHERE documents_fts MATCH ?1 AND rowid = ?2",
        rusqlite::params![consulta, fila],
        |_| Ok(()),
    )
    .is_ok()
}

/// Comprueba que el índice invertido es coherente.
///
/// # Por qué son dos comprobaciones y no una
///
/// Porque el índice del texto no guarda contenido, y eso cambió lo
/// que `integrity-check` puede decir. Sobre una tabla de contenido externo,
/// FTS5 compara el índice con el texto del que salió; sobre una sin contenido no
/// tiene con qué compararlo y solo mira que sus estructuras cuadren consigo
/// mismas. La diferencia no es de matiz: **medido el 2026-09-10 sobre SQLite
/// 3.53.4**, con seiscientos documentos indexados y los ocho bloques del índice
/// puestos a cero, `integrity-check` responde que todo está bien, la búsqueda
/// que devolvía las seiscientas filas devuelve cero **sin dar error**, y la
/// escritura siguiente funciona. Pérdida silenciosa y completa, con la
/// comprobación en verde.
///
/// Así que se comprueban dos cosas distintas:
///
/// 1. `integrity-check`, **solo sobre `catalog_fts`**, que sí tiene contenido
///    externo y por tanto algo con lo que compararse. Cuesta 2 segundos.
/// 2. **Las sondas**: unas cuantas palabras que se guardaron al indexar, fuera
///    del índice, junto al identificador de fila donde tienen que encontrarse.
///    Si el índice ya no las devuelve, el índice ya no contiene lo que se le
///    metió, y da igual que sus estructuras cuadren.
///
/// # Por qué `documents_fts` ya no pasa por `integrity-check`
///
/// Antes también se comprobaba, y aquí ponía que "se conserva
/// porque **lo que ve lo ve gratis**". Esa frase era una suposición, y medirla
/// la desmintió de las dos formas posibles.
///
/// **No es gratis: cuesta 4 minutos y 12 segundos** sobre la base real -9,9 GB,
/// 6.542.712 fragmentos-, medido el 2026-09-12. Y no es tiempo de fondo
/// inofensivo: `integrity-check` es una escritura sobre la tabla virtual, así
/// que SQLite abre una transacción de escritura y **retiene el bloqueo los
/// cuatro minutos enteros**. Durante ellos, toda escritura de la aplicación
/// -guardar el progreso de lectura, guardar una preferencia, aplicar unos
/// metadatos- agota su `busy_timeout` de 5 segundos y falla con
/// `database is locked`, y el pool de cuatro conexiones se agota detrás.
///
/// **Y lo que ve tampoco compensa**, porque es justo lo que dice el párrafo de
/// arriba: sobre una tabla sin contenido, `integrity-check` respondió que todo
/// estaba bien con el índice puesto a ceros. Se pagaban cuatro minutos de
/// aplicación inutilizable por una comprobación que ya se había medido incapaz
/// de detectar la pérdida que importa. Las sondas sí la detectan, y cuestan
/// milisegundos.
///
/// **Cómo se vio.** En el registro del usuario del 2026-09-12: veinte arranques,
/// nueve comprobaciones del índice y **cero cierres limpios**. Cada arranque
/// sucio lanzaba los cuatro minutos, la aplicación no admitía ni una escritura
/// mientras tanto, el usuario la mataba antes de que terminara -ninguna de las
/// nueve llegó a registrar su resultado- y el arranque siguiente volvía a estar
/// sucio. Un círculo cerrado del que no se sale usando la aplicación.
///
/// La sonda es la respuesta a "cómo se comprueba un índice que no puede
/// compararse con nada": sí puede compararse, con una muestra de sí mismo
/// guardada donde su corrupción no llega. Ver `index_probes` en V27.
///
/// # Lo que esta comprobación sigue sin ver
///
/// Una corrupción parcial que no toque ninguna de las dieciséis sondas leídas.
/// Se dice porque una comprobación en la que se confía de más es peor que no
/// tenerla: esto detecta que el índice se ha ido, no que le falte una palabra.
/// La red para lo que se cuele sigue siendo la reconstrucción a petición del
/// usuario, que es el otro entregable.
///
/// Y `integrity-check` sobre una tabla dañada **no es repetible**: la primera
/// llamada dice `Corrupt` y la segunda `Healthy` sobre lo que ha quedado vacío.
/// Por eso [`repair_index`] comprueba una sola vez antes de reparar. Las sondas
/// no tienen ese problema -son deterministas-, pero mientras las dos convivan
/// manda la más frágil.
///
/// # Errors
///
/// No falla por un índice corrupto: eso devuelve [`IndexHealth::Corrupt`].
/// [`NbError::StorageFailure`] queda para un fallo de la propia base.
pub fn verify_index(conn: &Connection) -> NbResult<IndexHealth> {
    // Solo el índice del catálogo. `documents_fts` no pasa por aquí desde
    // la comprobación de integridad, y el motivo -medido- está en la documentación de esta función.
    if conn
        .execute_batch("INSERT INTO catalog_fts(catalog_fts) VALUES('integrity-check')")
        .is_err()
    {
        tracing::warn!(
            tabla = "catalog_fts",
            "el índice de búsqueda no pasa su comprobación"
        );
        return Ok(IndexHealth::Corrupt);
    }

    for (termino, fila) in sondas_repartidas(conn)? {
        if !sonda_responde(conn, &termino, fila) {
            // Ni el término ni la fila: lo que se busca aquí es material con
            // derechos, y no sale a los registros (contrato, sección 5).
            tracing::warn!("el índice del texto ya no devuelve lo que se le metió");
            return Ok(IndexHealth::Corrupt);
        }
    }

    Ok(IndexHealth::Healthy)
}

/// Deja el índice utilizable, con el remedio más barato que sirva.
///
/// # Por qué en dos pasos
///
/// El índice invertido es **derivable de `text_chunks`**, que a su vez es
/// derivable de los documentos. Si solo se ha estropeado el primero, FTS5 lo
/// rehace en segundos sin abrir ni un archivo. Empezar siempre por releer ciento
/// cincuenta mil libros sería tirar horas de trabajo del usuario por un problema
/// que se arregla solo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si ni siquiera se puede vaciar el índice, que
/// significa que el problema no es del índice sino de la base.
/// Cómo se crea el índice invertido del texto.
///
/// Vive aquí **y** en la migración V27, y tiene que ser la misma cadena: la
/// migración lo crea la primera vez y `reset` lo recrea cuando está corrupto. Si
/// las dos se separaran, un índice reparado tendría otra forma que uno recién
/// migrado, y la diferencia solo se notaría en la máquina de alguien. Lo fija una
/// prueba, que comprueba que el archivo de la migración contiene esto.
pub const CREAR_INDICE: &str = "CREATE VIRTUAL TABLE documents_fts USING fts5(\n    text,\n    content = '',\n    contentless_delete = 1,\n    tokenize = \"unicode61 remove_diacritics 2\"\n);";

/// La marca que dice si la sesión anterior terminó como debía.
const CIERRE_LIMPIO: &str = "cierre_limpio";

/// Apunta cómo terminó -o empezó- esta sesión.
///
/// Se pone en falso al arrancar y en cierto al salir. Si la aplicación se cierra
/// de cualquier otra forma -un cierre forzado, un corte de luz, un fallo del
/// sistema- la marca se queda en falso, que es exactamente lo que hay que saber.
pub fn marcar_cierre(conn: &Connection, limpio: bool) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, ?2)",
        rusqlite::params![CIERRE_LIMPIO, if limpio { "true" } else { "false" }],
    );
}

/// Cómo terminó la sesión anterior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CierreAnterior {
    /// Firmó su salida.
    Limpio,
    /// Empezó y no firmó: murió, la cerraron a la fuerza o se fue la luz.
    Sucio,
    /// No hay marca **ni índice**: la base se acaba de crear, o nunca indexó
    /// nada. No hay nada que comprobar, y decir que la sesión anterior "no cerró
    /// bien" sobre un perfil nuevo era falso.
    PrimeraVez,
    /// No hay marca y sí índice: la base viene de una versión que no la
    /// escribía. Ante la duda se comprueba, que es el lado seguro.
    SinMarca,
}

/// Lee cómo terminó la sesión anterior.
///
/// **Tiene que leerse antes de que esta sesión se marque como empezada**, y por
/// eso lo hace `bootstrap` y lo guarda en el estado. Antes lo leía el
/// arranque diferido, que corre después de que `bootstrap` hubiera puesto la marca
/// en falso: en los 54 arranques que guardaba la instalación real, 53 dijeron que
/// la sesión anterior no había cerrado bien y ninguno lo contrario, así que el
/// índice se comprobaba siempre y el arranque solo funcionaba en su prueba.
#[must_use]
pub fn cierre_anterior(conn: &Connection) -> CierreAnterior {
    let marca = conn
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?1",
            [CIERRE_LIMPIO],
            |fila| fila.get::<_, String>(0),
        )
        .optional()
        .unwrap_or(None);

    match marca.as_deref() {
        Some("true") => CierreAnterior::Limpio,
        Some(_) => CierreAnterior::Sucio,
        None => {
            // Si no se puede saber, se comprueba: el lado seguro.
            let hay_indice = conn
                .query_row("SELECT EXISTS (SELECT 1 FROM text_chunks)", [], |f| {
                    f.get::<_, bool>(0)
                })
                .unwrap_or(true);
            if hay_indice {
                CierreAnterior::SinMarca
            } else {
                CierreAnterior::PrimeraVez
            }
        }
    }
}

/// Comprueba el índice **solo si hay motivo para dudar de él**.
///
/// # Por qué no se comprueba siempre
///
/// Porque el `integrity-check` de FTS5 recorre el índice entero, y eso cuesta lo
/// que ocupe: medido el 2026-09-10 sobre la biblioteca real, **5 minutos y 24
/// segundos** con el 13,7 % indexado. Por eso salió del camino de la ventana,
/// así que ya no impide arrancar, pero seguía costando eso en cada sesión y
/// dejando esperando detrás a todo lo demás del arranque diferido.
///
/// Y lo que se estaba comprobando era, casi siempre, que no había pasado nada.
/// Un índice invertido no se corrompe solo: se corrompe cuando el proceso muere
/// a media escritura o cuando el disco falla. Lo primero deja rastro -la sesión
/// no marcó su cierre- y comprobarlo entonces es proporcional a lo que ocurrió,
/// no a lo que ocupa el índice.
///
/// # Lo que esto deja de cubrir, y qué hay en su lugar
///
/// La corrupción **sin** cierre sucio: un sector que se estropea con la
/// aplicación cerrada. Eso ya no se ve al arrancar. Lo que lo ve es la propia
/// búsqueda, que recibe un error de corrupción de SQLite en cuanto consulta, y
/// la reconstrucción a petición que queda puesta. Se dice aquí porque un
/// mecanismo que se recorta tiene que decir qué deja de cubrir.
///
/// # Errors
///
/// Los de [`repair_index`].
pub fn repair_index_si_hace_falta(
    conn: &Connection,
    cierre: CierreAnterior,
) -> NbResult<Option<Repair>> {
    match cierre {
        CierreAnterior::Limpio => {
            tracing::info!("la sesión anterior cerró bien: no se comprueba el índice");
            return Ok(None);
        }
        CierreAnterior::PrimeraVez => {
            tracing::info!("base sin índice todavía: no hay nada que comprobar");
            return Ok(None);
        }
        CierreAnterior::Sucio => {
            tracing::info!("la sesión anterior no cerró bien: se comprueba el índice");
        }
        CierreAnterior::SinMarca => {
            tracing::info!("la base no dice cómo cerró la sesión anterior: se comprueba el índice");
        }
    }
    repair_index(conn).map(Some)
}

pub fn repair_index(conn: &Connection) -> NbResult<Repair> {
    if verify_index(conn)? == IndexHealth::Healthy {
        return Ok(Repair::NotNeeded);
    }

    /*
     * **El paso barato desapareció, y conviene decirlo.**
     *
     * Hasta entonces el índice del texto se podía rehacer desde los fragmentos
     * guardados, en segundos y sin abrir un solo documento. Eso era posible
     * porque los fragmentos llevaban el texto dentro, y el texto pesaba el 52 %
     * de la biblioteca: es justo lo que hubo que quitar para caber en el
     * presupuesto de disco.
     *
     * Con el índice sin contenido no hay de dónde rehacerlo: hay que volver a
     * leer los documentos. Sigue siendo caché y sigue sin impedir abrir la
     * biblioteca, pero cuesta lo que cuesta indexar, no segundos. Lo que baja el
     * coste esperado es que esto solo ocurre tras un cierre sucio.
     *
     * El índice de catálogo sí se rehace solo -es de contenido externo sobre
     * tablas que están ahí-, así que ese paso se conserva.
     */
    let catalogo = rebuild_catalog(conn).is_ok();
    if catalogo && verify_index(conn)? == IndexHealth::Healthy {
        tracing::info!("índice de catálogo reconstruido desde el catálogo");
        return Ok(Repair::Rebuilt);
    }

    tracing::warn!("el índice hay que rehacerlo leyendo los documentos otra vez");
    reset(conn)?;
    Ok(Repair::NeedsFullReindex)
}

/// Ya no rehace nada, y devuelve el error que dice por qué.
///
/// Antes rehacía el índice invertido desde los fragmentos guardados, en
/// segundos y sin abrir un documento, porque `documents_fts` era de contenido
/// externo y los fragmentos llevaban el texto dentro. Ese texto pesaba el 52 %
/// de la biblioteca y hubo que quitarlo para caber en el presupuesto de disco,
/// así que ya no hay de dónde rehacerlo: la reparación es volver a indexar.
///
/// Se conserva la función **fallando** en lugar de borrarla porque el `rebuild`
/// de FTS5 sobre una tabla sin contenido es justamente el error que hay que
/// devolver, y porque quien la busque en el historial encuentre aquí por qué ya
/// no puede hacer lo que hacía.
///
/// # Errors
///
/// Siempre [`NbError::StorageFailure`]: un índice sin contenido no se rehace.
pub fn rebuild_text_index(conn: &Connection) -> NbResult<()> {
    conn.execute_batch("INSERT INTO documents_fts(documents_fts) VALUES('rebuild')")
        .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Repuebla el índice de catálogo desde las tablas del catálogo.
///
/// A mano y no con `'rebuild'` porque `catalog_fts` no es de contenido externo:
/// su contenido lo escriben los disparadores, así que FTS5 no sabe de dónde
/// sacarlo. Aquí se repite la misma consulta que hacen ellos.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn rebuild_catalog(conn: &Connection) -> NbResult<()> {
    conn.execute_batch(
        "DELETE FROM catalog_fts;
         INSERT INTO catalog_fts (edition_id, title, authors, subjects)
         SELECT e.id,
                coalesce(e.title, '') || ' ' || coalesce(e.subtitle, '')
                    || ' ' || coalesce(e.series_title, ''),
                coalesce((SELECT group_concat(c.name, ' ')
                            FROM edition_contributors ec
                            JOIN contributors c ON c.id = ec.contributor_id
                           WHERE ec.edition_id = e.id), ''),
                coalesce((SELECT group_concat(s.label, ' ')
                            FROM edition_subjects es
                            JOIN subjects s ON s.id = es.subject_id
                           WHERE es.edition_id = e.id), '')
           FROM editions e",
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

/// Vacía el índice entero.
///
/// No borra nada del usuario: los fragmentos y el estado de búsqueda son caché,
/// y se rehacen leyendo los documentos. Por eso esta operación no pide
/// confirmación destructiva ni hace copia de seguridad: no hay nada que perder.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn reset(conn: &Connection) -> NbResult<()> {
    /*
     * **El índice se vacía primero, y el orden no es cosmético.**
     *
     * Borrar los fragmentos dispara el disparador que los quita del índice, uno
     * a uno. Sobre un índice corrupto ese borrado falla, y con él falla el
     * `DELETE` entero: vaciar por culpa de la corrupción se volvía imposible
     * justo cuando hacía falta. Con el índice ya vacío, el disparador no
     * encuentra nada que borrar y no puede fallar.
     *
     * Y se **recrea** en lugar de vaciarse. Vaciar una tabla FTS5 exige leer sus
     * estructuras internas, que es justo lo que está roto cuando se llega aquí:
     * ni `'delete-all'` ni `'rebuild'` funcionan sobre un índice corrupto, y
     * `'rebuild'` además no puede funcionar nunca, porque la tabla
     * no tiene contenido del que rehacerse. Tirarla y volver a crearla no
     * depende de que lo de dentro esté sano.
     */
    let _ = conn.execute_batch(&format!(
        "DROP TABLE IF EXISTS documents_fts;\n{CREAR_INDICE}"
    ));

    conn.execute_batch(
        "DELETE FROM text_chunks;
         DELETE FROM index_probes;
         DELETE FROM search_status;",
    )
    .map_err(|e| fallo(&e))?;

    let _ = rebuild_catalog(conn);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_indice_grande_solo_retira_el_prefijo_final_corto() {
        assert_eq!(sin_prefijo_corto("\"luz\"*"), "\"luz\"");
        assert_eq!(
            sin_prefijo_corto("\"historia\" AND \"arte\"*"),
            "\"historia\" AND \"arte\""
        );
        assert_eq!(sin_prefijo_corto("\"árbol\"*"), "\"árbol\"*");
        assert_eq!(sin_prefijo_corto("\"luz\""), "\"luz\"");
    }

    /// La pausa del indexador y el error de la preparación de audio son la misma
    /// condición: el tope de lo derivado. Si alguien renombrara uno de los dos,
    /// la interfaz tendría que saber traducir dos nombres para una sola cosa, y
    /// el que se quedara sin traducir saldría en mayúsculas en pantalla.
    #[test]
    fn la_pausa_y_el_error_del_presupuesto_son_el_mismo_nombre() {
        assert_eq!(
            PAUSA_PRESUPUESTO,
            crate::error::NbError::StorageBudgetReached.code()
        );
    }

    /// Una consulta que no termina nunca por sí sola: solo sale si la cortan.
    const SIN_FIN: &str = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c)
                           SELECT count(*) FROM c";

    #[test]
    fn una_busqueda_nueva_corta_la_que_sigue_corriendo() {
        let buscando = std::sync::Arc::new(BusquedaEnCurso::default());
        let (listo, empezado) = std::sync::mpsc::channel();

        let hilo = std::thread::spawn({
            let buscando = std::sync::Arc::clone(&buscando);
            move || {
                let conn = Connection::open_in_memory().expect("conexión");
                let turno = buscando.empezar(&conn);
                listo.send(()).expect("aviso");
                let resultado = conn.query_row(SIN_FIN, [], |fila| fila.get::<_, i64>(0));
                (resultado.map_err(|e| fallo(&e)), turno.vigente())
            }
        });

        empezado.recv().expect("la primera empezó");
        // Se le deja entrar en la consulta: interrumpir antes de que empiece no
        // demuestra que se corta algo que ya está corriendo.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let otra = Connection::open_in_memory().expect("conexión");
        let segundo = buscando.empezar(&otra);

        let (resultado, seguia_vigente) = hilo.join().expect("hilo");
        assert_eq!(resultado, Err(NbError::Cancelled));
        assert!(
            !seguia_vigente,
            "la primera tiene que saber que la sustituyeron"
        );
        assert!(segundo.vigente());
    }

    #[test]
    fn un_turno_soltado_no_deja_interrumpir_su_conexion_a_quien_venga_despues() {
        let buscando = BusquedaEnCurso::default();
        let conn = Connection::open_in_memory().expect("conexión");
        drop(buscando.empezar(&conn));

        // La conexión ya "volvió al pool". Una búsqueda nueva, en otra conexión,
        // no puede cortarle a esta lo que esté haciendo ahora para otro comando.
        let otra = Connection::open_in_memory().expect("conexión");
        let _turno = buscando.empezar(&otra);
        let cuenta: i64 = conn
            .query_row(
                "SELECT count(*) FROM (SELECT 1 UNION ALL SELECT 2)",
                [],
                |f| f.get(0),
            )
            .expect("la consulta de otro comando no se interrumpe");
        assert_eq!(cuenta, 2);
    }

    /// **Regresión.** El freno del presupuesto detiene, no marca el paso.
    ///
    /// Antes la llamada que frenaba reponía el contador de
    /// mediciones, así que las veinte vueltas siguientes ni medían ni frenaban y
    /// se indexaban veinte documentos más. El registro de la biblioteca real lo
    /// enseña con precisión de reloj: veinte documentos entre cada aviso de
    /// "indexación detenida", cinco veces seguidas. Sobre una noche entera son
    /// miles de documentos por encima del presupuesto que el innegociable dice
    /// que no se pasa.
    #[test]
    fn estando_por_encima_del_presupuesto_se_frena_todas_las_veces_y_no_una_de_cada_veinte() {
        let dir =
            crate::paths::temporal_de_pruebas().join(format!("nb-freno-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        // SAFETY: este binario de prueba usa su propio directorio y lo borra.
        unsafe { std::env::set_var(crate::paths::TEST_DIR_ENV, &dir) };
        let paths = crate::paths::AppPaths::resolve().expect("rutas");

        let db = crate::db::Database::open(&paths.database()).expect("base");
        {
            let conn = db.connection().expect("conexión");
            // Un documento minúsculo: cualquier base pesa más que un byte, así
            // que lo derivado queda por encima del presupuesto sin fabricar nada.
            conn.execute_batch(
                "INSERT INTO works (id, canonical_title) VALUES ('w', 'Obra');
                 INSERT INTO editions (id, work_id, title) VALUES ('e', 'w', 'Edición');
                 INSERT INTO files (id, edition_id, root_id, relative_path, content_hash,
                                    byte_size, media_type, storage_mode)
                 VALUES ('f', 'e', '00000000000000000000000000000001', 'x.epub',
                         'sha256:0000000000000000000000000000000000000000000000000000000000000000',
                         1, 'application/epub+zip', 'managed');",
            )
            .expect("catálogo mínimo");
        }

        let mut contador = 0;
        for vuelta in 0..3 {
            assert!(
                paso_del_presupuesto(&db, &paths, &mut contador, &Mutex::new(None)).is_some(),
                "la vuelta {vuelta} pasó de largo: el freno estaría dejando indexar"
            );
        }

        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
