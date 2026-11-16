//! Recorrer la biblioteca: ordenar, filtrar y agrupar.
//!
//! Contrato:
//! `docs/architecture/ui-components.md`.
//!
//! # Por qué esto vive en Rust y no en la interfaz
//!
//! Porque la biblioteca de referencia tiene 152.080 obras y el presupuesto de
//! nodos del documento es de 1.500. Traer la lista entera al webview para
//! ordenarla allí no es una cuestión de gusto: no cabe. La interfaz pide una
//! ventana de resultados y pinta esa ventana; ordenar y filtrar es trabajo de
//! SQL, que es donde están los índices.
//!
//! # Las estanterías no son una función nueva
//!
//! Agrupar por autor, por serie o por materia es exactamente lo que hace una
//! **colección por regla**, que se evalúa al consultar y por eso nunca
//! queda obsoleta. Aquí no se construye un mecanismo: se presenta el que ya
//! había. Es la lección de la sección 2.4 de la investigación de experiencia de
//! lectura: las quejas sobre la herramienta dominante no son por funciones que
//! falten, son por duplicados, series y lentitud, y tres de las cuatro ya
//! estaban resueltas sin verse en pantalla.

use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::db::catalog::Availability;
use crate::error::{NbError, NbResult};

/// Cuántos elementos puede pedir la interfaz de una vez.
///
/// Acotado porque un `limit` que venga del webview es una entrada no confiable,
/// y porque pedir diez mil filas para pintar veinte es la forma más común de
/// hacer que una lista virtualizada deje de serlo.
pub const MAXIMO_POR_PAGINA: usize = 200;

/// Por qué campo se ordena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum BrowseSort {
    Title,
    Author,
    /// Cuándo entró en la biblioteca.
    AddedAt,
    /// Por dónde va la lectura. Lo no empezado va al final.
    Progress,
    Series,
    /// Cuándo se publicó.
    Published,
    /// Cuándo se leyó por última vez. Lo nunca abierto va al final.
    LastRead,
}

impl BrowseSort {
    /// Expresión de orden. **Siempre desempata por `e.id`**: sin desempate, dos
    /// documentos con el mismo título pueden intercambiarse entre dos consultas
    /// y una lista virtualizada los duplicaría o los perdería al desplazarse.
    const fn expresion(self) -> &'static str {
        match self {
            Self::Title => "e.title COLLATE NOCASE",
            Self::Author => "e.author_sort COLLATE NOCASE",
            Self::AddedAt => "e.created_at",
            Self::Progress => "progresion",
            Self::Series => "e.series_title COLLATE NOCASE",
            Self::Published => "e.published_date",
            Self::LastRead => "ultima_lectura",
        }
    }

    /// Segunda clave de orden, para el orden que necesita dos.
    ///
    /// Solo la serie la tiene, y la necesita de verdad: dentro de una serie
    /// manda el número, y sin él los volúmenes salen por orden alfabético de su
    /// propio título, que en una saga no significa nada.
    ///
    /// Existe porque devolver las dos claves en una sola cadena **no funcionaba
    /// y no se veía**: la expresión se interpolaba como `{clave} AS orden`, y
    /// una cadena con una coma dentro produce dos columnas de las que solo la
    /// última se llama `orden`. Ordenar por serie ordenaba por el número de
    /// volumen y nada más, así que dos sagas distintas se intercalaban por su
    /// numeración. Salió al medir el coste de las páginas, no al leer el código.
    const fn segunda_clave(self) -> Option<&'static str> {
        match self {
            Self::Series => Some("e.series_index"),
            _ => None,
        }
    }
}

/// Qué subconjunto de la biblioteca se mira.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum BrowseFilter {
    All,
    Author(String),
    Series(String),
    Subject(String),
    Collection(String),
    /// Documentos cuyo archivo no está donde debería.
    Unavailable,
    /// Lo que la gente llama duplicado: obras con **el mismo título y el
    /// mismo autor** que otra, sin distinguir mayúsculas ni tildes, y además las
    /// que comparten contenido con otro registro, que solo existen durante una
    /// mudanza o tras restaurar una biblioteca que las tuviera.
    Duplicates,
    /// Los que tienen el libro entero preparado para escuchar.
    ///
    /// Un capítulo preparado no entra: lo que este filtro contesta es "qué puedo
    /// escuchar de principio a fin", y ofrecer un libro del que hay veinte
    /// minutos sería contestar otra pregunta. Lo dejado a medias sí entra -está
    /// marcado `partial`- porque escucharlo hasta donde llegue es exactamente lo
    /// que la interfaz promete de una cancelación.
    Audiobook,
    /// Empezados y sin terminar. Es lo que Inicio llama "sigue leyendo".
    ///
    /// Empezado significa que hay progreso guardado y que no está ni al
    /// principio ni al final: volver a ofrecer un libro terminado sería empujar
    /// a releerlo, y uno sin abrir no es "seguir".
    InProgress,
    /// Por idioma principal: `es` reúne `es`, `es-ES` y `es-MX`.
    Language(String),
    /// Por tipo de medio del archivo, `application/pdf` y demás.
    Format(String),
    /// Por año de publicación, en cuatro cifras.
    Year(String),
}

/// Lo que la interfaz pide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct BrowseQuery {
    pub sort: BrowseSort,
    pub ascending: bool,
    pub filter: BrowseFilter,
    pub offset: u32,
    pub limit: u32,
}

impl Default for BrowseQuery {
    fn default() -> Self {
        Self {
            sort: BrowseSort::AddedAt,
            ascending: false,
            filter: BrowseFilter::All,
            offset: 0,
            limit: 60,
        }
    }
}

/// Una fila de la biblioteca, con lo justo para pintarla.
///
/// **Sin portada.** Comprobar si existe el archivo de portada de cada elemento
/// serían cinco mil llamadas al disco para pintar veinte filas. La interfaz la
/// pide por edición y trata su ausencia como lo que es: una portada que no está.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct BrowseItem {
    pub edition_id: String,
    pub title: String,
    /// Autores en orden de aparición, ya unidos. Nulo si la obra no declara.
    pub authors: Option<String>,
    pub series_title: Option<String>,
    pub series_index: Option<f64>,
    pub availability: Availability,
    /// Progresión de 0 a 1, o nulo si no se ha empezado.
    pub progression: Option<f64>,
    pub added_at: String,
    /// Cuántos documentos comparten contenido con este, incluido él.
    pub copies: u32,
    /// Archivo que hay que abrir para leer esta edición.
    ///
    /// Nulo cuando ninguno está disponible, y entonces **no se puede abrir**:
    /// una edición puede tener varios archivos, y la lista muestra ediciones. Si
    /// la interfaz eligiera cuál abrir tendría que conocer el orden de
    /// preferencia entre disponibilidades, que es una regla del catálogo y no de
    /// la pantalla.
    pub file_id: Option<String>,
    /// La portada del documento, si traía una.
    ///
    /// En el catálogo se guarda **el nombre** del archivo, porque la caché
    /// cambia de sitio entre equipos y versiones del sistema y una ruta guardada
    /// sería una ruta rota. Al cruzar el puente ya viene resuelta a **ruta
    /// absoluta**, que es lo que el protocolo de assets sabe servir; quien la
    /// resuelve es `browse_library`, que es quien sabe dónde está la caché.
    ///
    /// Nulo cuando el documento no traía portada, que es lo normal en un PDF y
    /// no es un error: entonces se genera una, y una portada generada es una
    /// portada, no un hueco.
    pub cover_path: Option<String>,
    /// Tipo de medio del archivo que se abre, para la columna de formato.
    pub media_type: Option<String>,
    /// Su tamaño en bytes. Como número: ningún libro se acerca a 2^53.
    #[ts(type = "number | null")]
    pub byte_size: Option<i64>,
}

/// Una ventana de resultados y cuántos hay en total.
///
/// El total viaja aparte porque es lo que permite a la lista virtualizada
/// reservar el alto correcto sin haber pedido las filas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct BrowsePage {
    pub total: u32,
    pub items: Vec<BrowseItem>,
}

/// Clase de estantería.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum ShelfKind {
    Author,
    Series,
    Subject,
    Language,
    Format,
    Year,
}

/// Una agrupación con su tamaño.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Shelf {
    pub kind: ShelfKind,
    /// Lo que se le pasa al filtro para abrirla.
    pub key: String,
    pub label: String,
    pub count: u32,
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo al recorrer la biblioteca");
    NbError::StorageFailure
}

/// Trozo `SELECT` común. Se define una vez para que el recuento y la página
/// filtren exactamente igual: si divergieran, la lista reservaría un alto que no
/// corresponde a lo que luego llega.
const DESDE: &str = "FROM editions e";

/// Columnas derivadas que necesitan tanto el orden como la fila.
const DERIVADAS: &str = "
    (SELECT group_concat(c.name, ', ')
       FROM (SELECT ec.edition_id, c2.name
               FROM edition_contributors ec
               JOIN contributors c2 ON c2.id = ec.contributor_id
              WHERE ec.edition_id = e.id AND ec.role = 'author'
              ORDER BY ec.position) c
    ) AS autores,
    (SELECT json_extract(p.locator, '$.locations.totalProgression')
       FROM progress p WHERE p.edition_id = e.id AND p.profile = 'default'
    ) AS progresion,
    (SELECT min(CASE f.availability WHEN 'available' THEN 0
                                    WHEN 'unreadable' THEN 1 ELSE 2 END)
       FROM files f WHERE f.edition_id = e.id
    ) AS disponibilidad,
    (SELECT max(copias) FROM (
        SELECT (SELECT count(*) FROM files g WHERE g.content_hash = f.content_hash) AS copias
          FROM files f WHERE f.edition_id = e.id
     )) AS copias,
    -- `min` y no `ORDER BY ... LIMIT 1`: las dos formas son deterministas, pero
    -- la segunda ordena y la primera resuelve con el índice de `edition_id`. La
    -- diferencia son ocho milisegundos sobre cinco mil documentos, medidos, y el
    -- presupuesto de fotograma son dieciséis.
    (SELECT min(f.id) FROM files f
      WHERE f.edition_id = e.id AND f.availability = 'available'
    ) AS archivo,
    -- Formato y tamaño del primer archivo de la edición. Con `min` y búsqueda por
    -- clave, y no `ORDER BY ... LIMIT 1`: la forma ordenada sacó el recorrido de
    -- cinco mil documentos del presupuesto de fotograma, y lo cazó su prueba.
    (SELECT f.media_type FROM files f
      WHERE f.id = (SELECT min(g.id) FROM files g WHERE g.edition_id = e.id)) AS formato,
    (SELECT f.byte_size FROM files f
      WHERE f.id = (SELECT min(g.id) FROM files g WHERE g.edition_id = e.id)) AS tamano
";

/// Construye la condición del filtro y sus parámetros.
///
/// # Por qué `e.id IN (..)` y no `EXISTS` correlacionado
///
/// Las dos formas dicen lo mismo y no cuestan lo mismo. Con `EXISTS`, el motor
/// recorre las 152.080 ediciones y pregunta a cada una si es del autor; con
/// `IN`, empieza por el autor —con el índice de V29— y trae sus obras. Medido el
/// 2026-09-12 sobre la base real, la estantería de un autor con 1.248 obras
/// pasaba de 1,5 s por página a 7 ms. Lo fija una prueba que lee el plan de
/// cada filtro, porque en una biblioteca de prueba las dos formas tardan lo
/// mismo y el reloj no distinguiría nada.
fn condicion(filtro: &BrowseFilter) -> (String, Vec<String>) {
    match filtro {
        BrowseFilter::All => (String::new(), Vec::new()),
        BrowseFilter::Author(nombre) => (
            " WHERE e.id IN (SELECT ec.edition_id FROM contributors c
                               JOIN edition_contributors ec ON ec.contributor_id = c.id
                              WHERE c.name = ?1 AND ec.role = 'author')"
                .to_owned(),
            vec![nombre.clone()],
        ),
        BrowseFilter::Series(titulo) => (
            " WHERE e.series_title = ?1".to_owned(),
            vec![titulo.clone()],
        ),
        BrowseFilter::Subject(id) => (
            " WHERE e.id IN (SELECT es.edition_id FROM edition_subjects es
                              WHERE es.subject_id = ?1)"
                .to_owned(),
            vec![id.clone()],
        ),
        BrowseFilter::Collection(id) => (
            " WHERE e.id IN (SELECT ci.edition_id FROM collection_items ci
                              WHERE ci.collection_id = ?1)"
                .to_owned(),
            vec![id.clone()],
        ),
        BrowseFilter::Language(codigo) => (
            // Por rango y no con `LIKE`, que no usa el índice de V45: el punto
            // es el carácter que sigue al guion, así que el rango es `es-*`.
            " WHERE (e.language = ?1 OR (e.language >= ?1 || '-' AND e.language < ?1 || '.'))"
                .to_owned(),
            vec![codigo.clone()],
        ),
        BrowseFilter::Format(tipo) => (
            " WHERE e.id IN (SELECT f.edition_id FROM files f WHERE f.media_type = ?1)"
                .to_owned(),
            vec![tipo.clone()],
        ),
        // Por rango y no por `substr`: así entra por el índice de publicación de V44.
        BrowseFilter::Year(anio) => {
            let siguiente = anio
                .parse::<u32>()
                .map_or_else(|_| anio.clone(), |a| (a + 1).to_string());
            (
                " WHERE e.published_date >= ?1 AND e.published_date < ?2".to_owned(),
                vec![anio.clone(), siguiente],
            )
        }
        // Entra por el índice parcial de V29, que solo guarda lo que no está.
        BrowseFilter::Unavailable => (
            " WHERE e.id IN (SELECT f.edition_id FROM files f
                              WHERE f.availability <> 'available')"
                .to_owned(),
            Vec::new(),
        ),
        // Empezado es al menos un 1 %: con 0,001 entraban libros abiertos y no
        // leídos, que la tarjeta enseñaba al "0 %" bajo "Empezados".
        BrowseFilter::InProgress => (
            " WHERE e.id IN (SELECT p.edition_id FROM progress p
                              WHERE p.profile = 'default'
                                AND json_extract(p.locator, '$.locations.totalProgression') >= 0.01
                                AND json_extract(p.locator, '$.locations.totalProgression') < 0.99)"
                .to_owned(),
            Vec::new(),
        ),
        // El alcance vacío es el libro entero: lo dice V35. Entra por el índice
        // de `audio_sections`, que ya está por edición.
        BrowseFilter::Audiobook => (
            " WHERE e.id IN (SELECT a.edition_id FROM audio_sections a
                              WHERE a.scope_key = '')"
                .to_owned(),
            Vec::new(),
        ),
        /*
         * **Mismo título y mismo autor, o mismo contenido**.
         *
         * Antes solo contaba el contenido, y las
         * copias exactas no entran al importar, así que el filtro casi nunca
         * tenía nada. se decidió que enseñe lo que la gente llama duplicado:
         * otra edición u otro formato del mismo libro. El contenido se queda como
         * segunda vía para la mudanza y la restauración.
         *
         * **Cómo cabe en el presupuesto.** Primero los títulos repetidos, sobre el
         * índice de `title_key` y sin tocar la tabla -sobre la biblioteca real,
         * 7.538 títulos y 21.094 ediciones-, y solo entre esas se agrupa por
         * título y autor. Medido sobre la base real: unos 100 ms. La forma obvia,
         * preguntar a cada edición si otra comparte su título y su autor, tardaba
         * 2,8 s, y con la subconsulta correlacionada por título, 600 ms.
         */
        BrowseFilter::Duplicates => (
            " WHERE e.id IN (
                  WITH candidatas AS MATERIALIZED (
                      SELECT x.id, x.title_key, a.contributor_id
                        FROM editions x
                        JOIN edition_contributors a
                          ON a.edition_id = x.id AND a.role = 'author'
                       WHERE x.title_key IN (SELECT t.title_key FROM editions t
                                              GROUP BY t.title_key HAVING count(*) > 1)
                  )
                  SELECT c.id FROM candidatas c
                   WHERE (c.title_key, c.contributor_id) IN
                         (SELECT title_key, contributor_id FROM candidatas
                           GROUP BY title_key, contributor_id HAVING count(*) > 1)
                  UNION
                  SELECT f.edition_id FROM files f
                   WHERE f.content_hash IN (SELECT g.content_hash FROM files g
                                             GROUP BY g.content_hash HAVING count(*) > 1))"
                .to_owned(),
            Vec::new(),
        ),
    }
}

/// El plan de la consulta que cuenta un filtro. Solo para las pruebas que lo
/// vigilan: ver [`condicion`].
#[doc(hidden)]
#[must_use]
pub fn plan_del_recuento(conn: &Connection, filtro: &BrowseFilter) -> String {
    let (donde, parametros) = condicion(filtro);
    let Ok(mut stmt) = conn.prepare(&format!(
        "EXPLAIN QUERY PLAN SELECT count(*) {DESDE}{donde}"
    )) else {
        return String::new();
    };
    let refs: Vec<&dyn rusqlite::ToSql> = parametros
        .iter()
        .map(|p| p as &dyn rusqlite::ToSql)
        .collect();
    stmt.query_map(refs.as_slice(), |fila| fila.get::<_, String>(3))
        .map(|filas| filas.filter_map(Result::ok).collect::<Vec<_>>().join(" | "))
        .unwrap_or_default()
}

/// Cuántas obras hay detrás de cada filtro fijo de la biblioteca.
///
/// Existe para no ofrecer un filtro que no lleva a nada: con la biblioteca sana,
/// "No disponibles" es cero, y "Duplicados" casi, y una ficha que al pulsarla vacía la
/// vista es un control que miente sobre lo que hay. se pidió el
/// 2026-09-12 como regla general: lo que no tiene elementos no se enseña.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct LibraryFacets {
    pub all: u32,
    pub in_progress: u32,
    pub duplicates: u32,
    pub unavailable: u32,
    /// Cuántos libros están preparados para escuchar. Con ninguno, el
    /// filtro no se ofrece: es la misma regla que con los duplicados.
    pub audiobook: u32,
}

/// Cuenta los filtros fijos. Cada recuento entra por su índice.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn facets(conn: &Connection) -> NbResult<LibraryFacets> {
    let contar = |filtro: &BrowseFilter| -> NbResult<u32> {
        let (donde, _) = condicion(filtro);
        conn.query_row(&format!("SELECT count(*) {DESDE}{donde}"), [], |fila| {
            fila.get::<_, i64>(0)
        })
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
        .map_err(|e| fallo(&e))
    };
    Ok(LibraryFacets {
        all: contar(&BrowseFilter::All)?,
        in_progress: contar(&BrowseFilter::InProgress)?,
        duplicates: contar(&BrowseFilter::Duplicates)?,
        unavailable: contar(&BrowseFilter::Unavailable)?,
        audiobook: contar(&BrowseFilter::Audiobook)?,
    })
}

fn fila(row: &Row<'_>) -> rusqlite::Result<BrowseItem> {
    let disponibilidad: Option<i64> = row.get("disponibilidad")?;
    Ok(BrowseItem {
        edition_id: row.get("id")?,
        title: row.get("title")?,
        authors: row.get("autores")?,
        series_title: row.get("series_title")?,
        series_index: row.get("series_index")?,
        // Sin archivo, el documento está en el catálogo pero no en el disco.
        availability: match disponibilidad {
            Some(0) => Availability::Available,
            Some(1) => Availability::Unreadable,
            _ => Availability::Missing,
        },
        progression: row.get("progresion")?,
        added_at: row.get("created_at")?,
        copies: u32::try_from(row.get::<_, i64>("copias").unwrap_or(1)).unwrap_or(1),
        file_id: row.get("archivo")?,
        cover_path: row.get("cover_path")?,
        media_type: row.get("formato")?,
        byte_size: row.get("tamano")?,
    })
}

/// Las filas de pintar de unas ediciones concretas, en el orden en que se piden.
///
/// Es la misma fila que la biblioteca -con sus autores, su progreso y su archivo-
/// para las listas que no salen de un filtro sino de una elección, como las filas
/// de Inicio. Una edición que ya no exista simplemente no vuelve.
///
/// Cota: las que se pidan, que es quien llama quien las acota.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn items_por_id(conn: &Connection, ids: &[String]) -> NbResult<Vec<BrowseItem>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let marcas = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT e.id, e.title, e.series_title, e.series_index, e.created_at, e.cover_path,
            {DERIVADAS}
           FROM editions e
          WHERE e.id IN ({marcas})"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| fallo(&e))?;
    let mut items: Vec<BrowseItem> = stmt
        .query_map(rusqlite::params_from_iter(ids.iter()), fila)
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(|e| fallo(&e))?;
    let lugar = |id: &str| {
        ids.iter()
            .position(|pedido| pedido == id)
            .unwrap_or(usize::MAX)
    };
    items.sort_by_key(|item| lugar(&item.edition_id));
    Ok(items)
}

/// Devuelve una ventana de la biblioteca.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
/// Cuántas iniciales se ofrecen como mucho. Las 26 letras, `#` y las iniciales
/// con tilde caben de sobra.
pub const MAXIMO_DE_ANCLAS: u32 = 64;

/// Una letra del índice alfabético y dónde empieza en la lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct LetterAnchor {
    /// La letra en mayúscula, o `#` para lo que empieza por cifra o signo.
    pub letter: String,
    /// Posición de la primera obra con esa inicial, contando desde cero.
    pub offset: u32,
}

/// Las iniciales de la lista ordenada por título o autor, con su posición.
///
/// Sirve para saltar con anclas alfabéticas sin recorrer páginas. Solo tiene
/// sentido con esos dos órdenes; con cualquier otro devuelve una lista vacía.
/// Es una sola consulta agrupada: sobre la biblioteca entera son tantas filas
/// como iniciales distintas, nunca tantas como obras.
///
/// Cota: [`MAXIMO_DE_ANCLAS`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn letter_anchors(conn: &Connection, query: &BrowseQuery) -> NbResult<Vec<LetterAnchor>> {
    let clave = match query.sort {
        BrowseSort::Title => "e.title",
        BrowseSort::Author => "e.author_sort",
        _ => return Ok(Vec::new()),
    };
    let (donde, parametros) = condicion(&query.filter);
    // Cada grupo es una inicial tal como la ordena la lista (NOCASE solo pliega
    // ASCII, así que "Á" no va con la "A" sino detrás de la "Z", y aquí también):
    // se ordenan los grupos por su primera clave y no por la letra, para que la
    // posición de cada ancla sea exactamente donde empieza en la lista. Las cifras
    // y los signos ASCII van juntos en `#`.
    let sql = format!(
        "SELECT CASE WHEN upper(substr({clave}, 1, 1)) BETWEEN 'A' AND 'Z'
                       OR unicode(substr({clave}, 1, 1)) > 127
                     THEN upper(substr({clave}, 1, 1)) ELSE '#' END AS l,
                count(*), min({clave} COLLATE NOCASE) AS m
           {DESDE}{donde}
          GROUP BY l ORDER BY m {direccion} LIMIT {MAXIMO_DE_ANCLAS}",
        direccion = if query.ascending { "ASC" } else { "DESC" }
    );
    let refs: Vec<&dyn rusqlite::ToSql> = parametros
        .iter()
        .map(|p| p as &dyn rusqlite::ToSql)
        .collect();
    let cuentas: Vec<(String, u32)> = conn
        .prepare(&sql)
        .map_err(|e| fallo(&e))?
        .query_map(refs.as_slice(), |f| {
            Ok((f.get::<_, String>(0)?, f.get::<_, u32>(1)?))
        })
        .map_err(|e| fallo(&e))?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| fallo(&e))?;
    // SQLite solo pasa a mayúsculas el ASCII: "á" y "Á" llegan como grupos
    // distintos. Se nombran en mayúscula aquí y, si dos quedan con el mismo
    // nombre, se ofrece el primero, que es donde empieza esa letra en la lista.
    let mut acumulado = 0_u32;
    let mut anclas: Vec<LetterAnchor> = Vec::new();
    for (letra, n) in cuentas {
        let letter: String = letra.to_uppercase();
        if !anclas.iter().any(|a| a.letter == letter) {
            anclas.push(LetterAnchor {
                letter,
                offset: acumulado,
            });
        }
        acumulado += n;
    }
    Ok(anclas)
}

pub fn browse(conn: &Connection, query: &BrowseQuery) -> NbResult<BrowsePage> {
    let (donde, parametros) = condicion(&query.filter);
    let limite = (query.limit as usize).min(MAXIMO_POR_PAGINA);

    let total: i64 = {
        let sql = format!("SELECT count(*) {DESDE}{donde}");
        let refs: Vec<&dyn rusqlite::ToSql> = parametros
            .iter()
            .map(|p| p as &dyn rusqlite::ToSql)
            .collect();
        conn.query_row(&sql, refs.as_slice(), |row| row.get(0))
            .map_err(|e| fallo(&e))?
    };

    // El orden se compone aquí y no se recibe: una dirección de orden que venga
    // del webview como texto sería una vía de inyección, y una lista blanca de
    // dos valores no la deja abierta.
    let direccion = if query.ascending { "ASC" } else { "DESC" };

    // **Primero se decide qué página es, y después se rellena.** La consulta
    // interna ordena sobre las cinco mil filas calculando *solo* la columna por
    // la que ordena; la externa calcula el resto para las sesenta que quedan.
    // Escrito de la forma obvia, con las cuatro subconsultas en el SELECT de
    // fuera, SQLite las evaluaba para toda la biblioteca antes de recortar: 45
    // ms contra un presupuesto de fotograma de 16. Lo midió la prueba de
    /*
     * Ordenar por autor no une con nadie: la clave vive en `editions`.
     *
     * Hubo dos intentos antes. Una subconsulta correlacionada obligaba a resolver
     * el autor de cada fila una a una —29 ms sobre cinco mil, contra 16 de
     * presupuesto—. Una unión doble lo resolvía de una vez y bajaba de ahí, pero
     * sobre ciento cincuenta mil costaba **60 ms**, y ningún índice lo arregla:
     * con la clave al otro lado de dos uniones, el motor une primero y ordena
     * después, así que construye un árbol temporal se ponga lo que se ponga. Se
     * probó con un índice sobre el nombre y quedó en 79, peor.
     *
     * La salida es materializar la clave en `editions` y mantenerla con
     * disparadores: `V19__author_sort.sql` dice cómo y por qué son cuatro. Es la
     * respuesta a la pregunta J11.
     *
     * `autores` sigue existiendo para **enseñar** los autores, que es otra cosa:
     * ahí van todos y aquí solo el principal.
     */
    let (union, clave) = match query.sort {
        BrowseSort::Progress => (
            " LEFT JOIN progress p ON p.edition_id = e.id AND p.profile = 'default'",
            "json_extract(p.locator, '$.locations.totalProgression')",
        ),
        BrowseSort::LastRead => (
            " LEFT JOIN progress p ON p.edition_id = e.id AND p.profile = 'default'",
            "p.updated_at",
        ),
        otro => ("", otro.expresion()),
    };

    // La segunda clave viaja en su propia columna, y no pegada a la primera. Es
    // lo que arregla el orden por serie: ver `segunda_clave`.
    let (segunda_select, segunda_order) = match query.sort.segunda_clave() {
        Some(expresion) => (
            format!(", {expresion} AS orden2"),
            format!(", pagina.orden2 {direccion}"),
        ),
        None => (String::new(), String::new()),
    };

    // Con "Duplicados", lo repetido va junto: se agrupa por el título comparable
    // y, dentro de cada grupo, manda el orden elegido. Sin esto, ordenando por
    // fecha de alta las dos ediciones de un mismo libro quedan a páginas de
    // distancia, y el filtro existe para ponerlas una al lado de la otra.
    let (grupo_select, grupo_interno, grupo_externo) = match query.filter {
        BrowseFilter::Duplicates => (
            ", e.title_key AS grupo",
            "grupo ASC, ",
            "pagina.grupo ASC, ",
        ),
        _ => ("", "", ""),
    };

    let sql = format!(
        "SELECT e.id, e.title, e.series_title, e.series_index, e.created_at, e.cover_path,
            {DERIVADAS}
           FROM (SELECT e.id AS pid, {clave} AS orden{segunda_select}{grupo_select}
                   {DESDE}{union}{donde}
                  ORDER BY {grupo_interno}orden {direccion}{segunda_order_interno}{desempate}, e.id ASC
                  LIMIT {limite} OFFSET {}) AS pagina
           JOIN editions e ON e.id = pagina.pid
          ORDER BY {grupo_externo}pagina.orden {direccion}{segunda_order}{desempate}, e.id ASC",
        query.offset,
        // A igual clave, por título y siempre de la A a la Z. Toda la
        // importación inicial comparte fecha de alta, y desempatar por el
        // identificador -un hash- o en la dirección elegida dejaba "Añadido"
        // convertido en un orden alfabético al revés a partir de la primera fila.
        // Solo en los dos órdenes que tienen índice para ello (V43): la serie ya
        // desempata por su número y el progreso no tiene índice que salvar.
        desempate = if matches!(
            query.sort,
            BrowseSort::AddedAt | BrowseSort::Author | BrowseSort::Published
        ) {
            ", e.title COLLATE NOCASE ASC"
        } else {
            ""
        },
        segunda_order_interno = match query.sort.segunda_clave() {
            Some(_) => format!(", orden2 {direccion}"),
            None => String::new(),
        }
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| fallo(&e))?;
    let refs: Vec<&dyn rusqlite::ToSql> = parametros
        .iter()
        .map(|p| p as &dyn rusqlite::ToSql)
        .collect();

    let items = stmt
        .query_map(refs.as_slice(), fila)
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(|e| fallo(&e))?;

    Ok(BrowsePage {
        total: u32::try_from(total).unwrap_or(u32::MAX),
        items,
    })
}

/// Lo que se le pide a [`shelves`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ShelfQuery {
    /// Qué clase de estantería se pide. Una a la vez: cada sección pide la suya
    /// y así el límite se reparte entre las tres en lugar de gastarlo la primera.
    pub kind: ShelfKind,
    /// Texto por el que buscar en el nombre. Vacío devuelve las más numerosas.
    pub search: String,
    /// Cuántas devolver como mucho.
    pub limit: u32,
}

/// Una página de estanterías, con cuántas hay en total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ShelfPage {
    pub shelves: Vec<Shelf>,
    /// Cuántas cumplen la búsqueda, se devuelvan o no. Es lo que permite decir
    /// "veinte de tres mil" en lugar de enseñar veinte como si fueran todas.
    pub total: u32,
}

/// Cuántas estanterías se devuelven como mucho, pida lo que pida quien llama.
///
/// Doscientas. Es un tope del runtime y no de la interfaz porque el motivo es
/// del runtime: el resultado cruza el puente y se convierte en nodos del
/// documento, y una biblioteca real tiene decenas de miles de autores. Una
/// petición sin límite no es una petición: es una forma de que el webview se
/// quede sin memoria.
const MAXIMO_ESTANTERIAS: u32 = 200;

/// Estanterías de una clase: autores, series o materias, con su tamaño.
///
/// # Por qué esto busca en lugar de listar
///
/// Porque listarlas no se puede. Con 152.080 obras hay decenas de miles de
/// autores, y traerlos para elegir uno es el mismo error que traer la biblioteca
/// entera para enseñar una página. Sin búsqueda, la única forma de ofrecerlas
/// sería un desplegable de miles de elementos, que no es un gesto: es una
/// búsqueda peor hecha. Es la respuesta a la pregunta I8, y el mismo criterio
/// que decidió H6.
///
/// Sin texto se devuelven **las más numerosas**, que es lo útil de partida: una
/// estantería con doscientos libros importa más que una con uno. El desempate
/// por nombre las mantiene estables entre consultas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn shelves(conn: &Connection, peticion: &ShelfQuery) -> NbResult<ShelfPage> {
    /*
     * Se lee del recuento materializado (`shelf_counts`, V24) y no del catálogo.
     *
     * Antes esto agrupaba y contaba sobre las tablas de enlace, y el `LIMIT`
     * recortaba el resultado pero no el trabajo: para saber cuáles son las veinte
     * mayores hay que contar las 55.740. Medido el 2026-09-10 sobre la biblioteca
     * real: 4.141 ms el panel de autores, con árbol temporal para el GROUP BY y
     * otro para el ORDER BY, contra un presupuesto de fotograma de 16 ms. Es el
     * mismo caso que `author_sort` y la misma salida: materializar lo
     * que hay que ordenar, porque ningún índice puede evitar un GROUP BY.
     *
     * El filtro por texto va sobre `etiqueta` y el orden lo da el índice, así que
     * no queda ningún árbol temporal. Lo comprueba `recorrido_biblioteca.rs`
     * leyendo el plan, que es la comprobación que no depende de la máquina.
     */
    let clase = match peticion.kind {
        ShelfKind::Author => "author",
        ShelfKind::Series => "series",
        ShelfKind::Subject => "subject",
        ShelfKind::Language | ShelfKind::Format | ShelfKind::Year => {
            return estanterias_al_vuelo(conn, peticion);
        }
    };

    let patron = patron_de(&peticion.search);
    let tope = i64::from(peticion.limit.clamp(1, MAXIMO_ESTANTERIAS));

    let total: i64 = conn
        .query_row(
            "SELECT count(*) FROM shelf_counts WHERE kind = ?1 AND label LIKE ?2 ESCAPE '\\'",
            rusqlite::params![clase, &patron],
            |row| row.get(0),
        )
        .map_err(|e| fallo(&e))?;

    let sql = "SELECT shelf_key, label, tally FROM shelf_counts
                WHERE kind = ?1 AND label LIKE ?2 ESCAPE '\\'
                ORDER BY tally DESC, label, shelf_key
                LIMIT ?3";
    let mut stmt = conn.prepare(sql).map_err(|e| fallo(&e))?;
    let shelves = stmt
        .query_map(rusqlite::params![clase, &patron, tope], |row| {
            Ok(Shelf {
                kind: peticion.kind,
                key: row.get(0)?,
                label: row.get(1)?,
                count: u32::try_from(row.get::<_, i64>(2)?).unwrap_or(u32::MAX),
            })
        })
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(|e| fallo(&e))?;

    Ok(ShelfPage {
        shelves,
        total: u32::try_from(total).unwrap_or(u32::MAX),
    })
}

/// Idioma, formato y año se cuentan al vuelo, sin materializar.
///
/// Son pocas -una decena de idiomas, tres formatos, unos cientos de años- y el
/// coste es un recorrido de la tabla: medido sobre la base real, de 70 a 400 ms,
/// fuera del hilo de la ventana. Materializarlas como V24 pediría una migración
/// y disparadores en tres tablas para ahorrar una espera que solo se paga al
/// abrir la sección. La cota de filas la da la propia clase, y aun así se recorta
/// con el mismo tope que las demás.
fn estanterias_al_vuelo(conn: &Connection, peticion: &ShelfQuery) -> NbResult<ShelfPage> {
    let agrupada = match peticion.kind {
        ShelfKind::Language => {
            "SELECT lower(CASE WHEN instr(language, '-') > 0
                               THEN substr(language, 1, instr(language, '-') - 1)
                               ELSE language END) AS clave, count(*) AS n
               FROM editions WHERE language IS NOT NULL AND trim(language) <> ''
              GROUP BY clave"
        }
        ShelfKind::Format => {
            "SELECT media_type AS clave, count(DISTINCT edition_id) AS n
               FROM files GROUP BY clave"
        }
        _ => {
            "SELECT substr(published_date, 1, 4) AS clave, count(*) AS n
               FROM editions
              WHERE published_date GLOB '[0-9][0-9][0-9][0-9]*'
                -- Un año futuro es una errata del archivo, como 2965, y al ir del
                -- más reciente al más antiguo salían las primeras.
                AND substr(published_date, 1, 4) BETWEEN '1000' AND strftime('%Y', 'now')
              GROUP BY clave"
        }
    };
    let patron = patron_de(&peticion.search);
    let tope = i64::from(peticion.limit.clamp(1, MAXIMO_ESTANTERIAS));
    // El año se ordena por año, del más reciente; las otras, por tamaño.
    let orden = if peticion.kind == ShelfKind::Year {
        "clave DESC"
    } else {
        "n DESC, clave"
    };
    let sql = format!(
        "SELECT clave, n, count(*) OVER () FROM ({agrupada})
          WHERE clave LIKE ?1 ESCAPE '\\'
          ORDER BY {orden} LIMIT ?2"
    );
    let mut total = 0_i64;
    let mut stmt = conn.prepare(&sql).map_err(|e| fallo(&e))?;
    let shelves = stmt
        .query_map(rusqlite::params![&patron, tope], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .and_then(Iterator::collect::<rusqlite::Result<Vec<_>>>)
        .map_err(|e| fallo(&e))?
        .into_iter()
        .map(|(clave, n, todas)| {
            total = todas;
            Shelf {
                kind: peticion.kind,
                key: clave.clone(),
                label: clave,
                count: u32::try_from(n).unwrap_or(u32::MAX),
            }
        })
        .collect();
    Ok(ShelfPage {
        shelves,
        total: u32::try_from(total).unwrap_or(u32::MAX),
    })
}

/// Patrón `LIKE` a partir de lo que se escribió.
///
/// Los comodines del propio `LIKE` se escapan: quien escribe `100%` en el campo
/// de búsqueda está buscando un texto, no pidiendo que coincida con todo. Sin
/// esto, un `%` suelto devolvía la biblioteca entera y un `_` colaba un comodín
/// de un carácter donde nadie lo esperaba.
fn patron_de(texto: &str) -> String {
    let limpio = texto.trim();
    if limpio.is_empty() {
        return "%".to_owned();
    }
    let escapado = limpio
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escapado}%")
}

// ---------------------------------------------------------------------------
// Ficha de una edición
// ---------------------------------------------------------------------------

/// Una copia de la edición en el disco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct FileDetails {
    pub media_type: String,
    #[ts(type = "number")]
    pub byte_size: i64,
    /// Ruta relativa a su raíz: dice qué archivo es sin enseñar dónde vive la
    /// cuenta, como en los rechazos de una importación.
    pub relative_path: String,
    /// El nombre de la raíz, para distinguir dos copias en dos discos.
    pub root: String,
    pub available: bool,
    /// Copia que la aplicación hizo en su carpeta: es la única que se puede
    /// borrar del disco al quitar el documento.
    pub managed: bool,
}

/// Lo que la base sabe de una edición además de lo que pinta la rejilla:
/// editorial, fecha, idioma, sinopsis, materias y copias.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct EditionDetails {
    pub publisher: Option<String>,
    pub published_date: Option<String>,
    pub language: Option<String>,
    /// La sinopsis del propio documento, si la trae.
    pub description: Option<String>,
    /// Cota: [`MAXIMO_DE_MATERIAS`].
    pub subjects: Vec<String>,
    /// Cota: [`MAXIMO_DE_COPIAS`].
    pub files: Vec<FileDetails>,
}

/// Cuántas materias se enseñan en una ficha.
pub const MAXIMO_DE_MATERIAS: usize = 20;
/// Cuántas copias se enseñan en una ficha.
pub const MAXIMO_DE_COPIAS: usize = 20;

/// La ficha de una edición.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla, y
/// [`NbError::DocumentNotFound`] si la edición no existe.
pub fn details(conn: &Connection, edition_id: &str) -> NbResult<EditionDetails> {
    let fallo = |_| NbError::StorageFailure;
    let (publisher, published_date, language, description) = conn
        .query_row(
            "SELECT nullif(trim(publisher), ''), nullif(trim(published_date), ''),
                    nullif(trim(language), ''), nullif(trim(description), '')
               FROM editions WHERE id = ?1",
            [edition_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => NbError::DocumentNotFound,
            _ => NbError::StorageFailure,
        })?;
    let subjects = conn
        .prepare(
            "SELECT s.label FROM edition_subjects es JOIN subjects s ON s.id = es.subject_id
              WHERE es.edition_id = ?1 ORDER BY s.label LIMIT ?2",
        )
        .map_err(fallo)?
        .query_map(
            rusqlite::params![edition_id, MAXIMO_DE_MATERIAS as i64],
            |r| r.get(0),
        )
        .map_err(fallo)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(fallo)?;
    let files = conn
        .prepare(
            "SELECT f.media_type, f.byte_size, f.relative_path, coalesce(r.name, ''),
                    f.availability = 'available', f.storage_mode = 'managed'
               FROM files f LEFT JOIN library_roots r ON r.id = f.root_id
              WHERE f.edition_id = ?1 ORDER BY f.created_at LIMIT ?2",
        )
        .map_err(fallo)?
        .query_map(
            rusqlite::params![edition_id, MAXIMO_DE_COPIAS as i64],
            |r| {
                Ok(FileDetails {
                    media_type: r.get(0)?,
                    byte_size: r.get(1)?,
                    relative_path: r.get(2)?,
                    root: r.get(3)?,
                    available: r.get(4)?,
                    managed: r.get(5)?,
                })
            },
        )
        .map_err(fallo)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(fallo)?;
    Ok(EditionDetails {
        publisher,
        published_date,
        language,
        description,
        subjects,
        files,
    })
}
