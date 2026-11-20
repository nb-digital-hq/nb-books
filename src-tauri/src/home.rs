//! Lo que la pantalla de Inicio necesita saber, y nada más.
//!
//! # Por qué existe este módulo y no una consulta suelta en el puente
//!
//! ADR-013 hace de Inicio la pantalla de arranque, y eso convierte sus consultas
//! en las del **presupuesto de arranque**: se pagan siempre, incluso cuando el
//! usuario solo quería abrir el libro que estaba leyendo. Tenerlas juntas es lo
//! que permite mirarlas como un conjunto en lugar de descubrir una por una que
//! cada vista añadió la suya.
//!
//! # Cero red y cero IA
//!
//! Todo sale de datos que ya existen: progreso, autores, materias y colecciones.
//! Cada fila es **local, reproducible y explicable**: dice por qué aparece, y eso
//! no es un adorno, es lo que impide que Inicio degenere en un escaparate que
//! empuja en lugar de acompañar. "Reproducible" y no "determinista" desde la
//! enmienda de ADR-013 del 2026-09-12: ver [`filas`].
//!
//! Contrato: `docs/adr/013-information-architecture.md`.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

fn fallo(error: &rusqlite::Error) -> NbError {
    // El mensaje puede citar un título, que es dato del usuario.
    tracing::error!(clase = %error, "consulta de la pantalla de inicio fallida");
    NbError::StorageFailure
}

/// Cuánto se ha leído, medido sobre las sesiones que ya se registraban.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Lectura {
    /// Minutos leídos hoy.
    pub minutes_today: u32,
    /// Minutos leídos en los últimos siete días.
    pub minutes_week: u32,
    /// Días seguidos con alguna sesión, contando hacia atrás desde hoy.
    ///
    /// Se calcula y se muestra, pero **no se presiona con ello**: ADR-013 deja
    /// fuera las rachas con culpa administrada por el software. Es un dato, no
    /// una obligación.
    pub streak_days: u32,
    /// Documentos empezados y sin terminar.
    pub in_progress: u32,
    /// Documentos de la biblioteca.
    pub documents: u32,
}

/// Estadísticas de lectura.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn lectura(conn: &Connection) -> NbResult<Lectura> {
    let minutos = |desde: &str| -> NbResult<u32> {
        conn.query_row(
            "SELECT COALESCE(SUM(
                 (julianday(COALESCE(ended_at, started_at)) - julianday(started_at)) * 1440
             ), 0)
               FROM reading_sessions
              WHERE started_at >= ?1",
            [desde],
            |row| row.get::<_, f64>(0),
        )
        .map(|m| m.max(0.0).round() as u32)
        .map_err(|e| fallo(&e))
    };

    let documents = conn
        .query_row("SELECT count(*) FROM editions", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|e| fallo(&e))?;

    let in_progress = conn
        .query_row(
            "SELECT count(*) FROM progress
              WHERE profile = 'default'
                AND json_extract(locator, '$.locations.totalProgression') >= 0.01
                AND json_extract(locator, '$.locations.totalProgression') < 0.99",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| fallo(&e))?;

    // Los días con sesión, del más reciente hacia atrás. Se traen los treinta
    // últimos y se cuenta la racha aquí: hacerlo en SQL exigiría una recursiva
    // por una cuenta que cabe en diez líneas.
    let mut dias = conn
        .prepare(
            "SELECT DISTINCT date(started_at) FROM reading_sessions
              WHERE started_at >= date('now', '-30 days')
              ORDER BY 1 DESC",
        )
        .map_err(|e| fallo(&e))?;
    let dias: Vec<String> = dias
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| fallo(&e))?
        .collect::<Result<_, _>>()
        .map_err(|e| fallo(&e))?;

    let hoy: String = conn
        .query_row("SELECT date('now')", [], |row| row.get(0))
        .map_err(|e| fallo(&e))?;
    let ayer: String = conn
        .query_row("SELECT date('now', '-1 day')", [], |row| row.get(0))
        .map_err(|e| fallo(&e))?;

    // La racha sigue viva si se leyó hoy o ayer: cortarla a medianoche haría que
    // quien lee de noche perdiera la cuenta por acostarse tarde.
    let mut streak_days = 0_u32;
    if dias.first().is_some_and(|d| *d == hoy || *d == ayer) {
        let mut esperado = dias[0].clone();
        for dia in &dias {
            if *dia != esperado {
                break;
            }
            streak_days += 1;
            esperado = conn
                .query_row("SELECT date(?1, '-1 day')", [&esperado], |row| row.get(0))
                .map_err(|e| fallo(&e))?;
        }
    }

    Ok(Lectura {
        minutes_today: minutos(&hoy)?,
        minutes_week: minutos(
            &conn
                .query_row("SELECT date('now', '-7 days')", [], |r| {
                    r.get::<_, String>(0)
                })
                .map_err(|e| fallo(&e))?,
        )?,
        streak_days,
        in_progress: u32::try_from(in_progress).unwrap_or(u32::MAX),
        documents: u32::try_from(documents).unwrap_or(u32::MAX),
    })
}

// ---------------------------------------------------------------------------
// Las filas de Inicio
// ---------------------------------------------------------------------------

/// Cuántas obras hacen falta para que una fila se pinte.
///
/// Ocho. Observación de uso del 2026-09-12: "si vamos a tener cinco libros, es mejor no tener
/// la fila". Una fila que no llena ni una página de su propio carril no invita a
/// recorrerla, y con 152.080 obras siempre hay otra que sí la llena.
pub const MINIMO_POR_FILA: usize = 8;

/// Cuántas obras lleva una fila como mucho. Veinte son tres páginas del carril en
/// una ventana ancha: bastante para recorrer, y la fila entera está a un "Ver
/// todo" de distancia.
pub const OBRAS_POR_FILA: usize = 20;

/// Cuántas obras lleva la fila en lista: diez, que es lo que se lee de un vistazo,
/// y es a la vez su mínimo y su máximo.
pub const EN_LISTA: usize = 10;

const FILA: std::ops::RangeInclusive<usize> = MINIMO_POR_FILA..=OBRAS_POR_FILA;
const LISTA: std::ops::RangeInclusive<usize> = EN_LISTA..=EN_LISTA;

/// Por qué existe una fila. Es lo que se lee en su título, y es un tipo y no un
/// texto porque el texto lo pone la interfaz en el idioma de quien lee, y porque
/// cada fila **dice por qué aparece**, que es la condición de
/// ADR-013 para que Inicio no sea un escaparate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum HomeRowKind {
    /// Lo último que entró en la biblioteca.
    Novedades,
    /// Del mismo autor que algo que se está leyendo.
    #[serde(rename_all = "camelCase")]
    PorqueLees { title: String, author: String },
    /// De una materia de la biblioteca.
    #[serde(rename_all = "camelCase")]
    Materia { key: String, label: String },
    /// De un autor con muchas obras en la biblioteca.
    Autor(String),
    /// De una colección del usuario.
    #[serde(rename_all = "camelCase")]
    Coleccion { id: String, name: String },
    /// Obras que nunca se han abierto.
    Redescubrir,
    /// Una selección al azar para hoy.
    AlAzar,
}

/// Cómo se pinta una fila. Tres formas y no una, para que Inicio no sea la misma
/// hilera de portadas repetida: una fila de portadas grandes da presencia a lo que
/// abre la pantalla, y una lista numerada se lee de otra manera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum HomeRowLayout {
    Portadas,
    Grandes,
    Lista,
}

/// Una fila de Inicio con sus obras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct HomeRow {
    /// Estable entre visitas para la misma fila: la interfaz lo usa como clave.
    pub id: String,
    pub kind: HomeRowKind,
    pub layout: HomeRowLayout,
    pub items: Vec<crate::browse::BrowseItem>,
}

/// Un generador pequeño y reproducible. No hace falta más: lo que se elige al
/// azar son obras de una biblioteca, no una clave.
struct Azar(u64);

impl Azar {
    fn siguiente(&mut self) -> u64 {
        // splitmix64: cada semilla da una secuencia distinta y bien repartida.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Un punto de partida en el espacio de identificadores, que son hexadecimales
    /// aleatorios de 32 caracteres. Leer desde ahí en orden **es** una muestra al
    /// azar, y cuesta una búsqueda en un índice en lugar de ordenar la biblioteca
    /// entera por `random()`, que es lo que habría hecho falta sin esto.
    fn inicio(&mut self) -> String {
        format!("{:016x}{:016x}", self.siguiente(), self.siguiente())
    }

    fn baraja<T>(&mut self, lista: &mut [T]) {
        for i in (1..lista.len()).rev() {
            let j = usize::try_from(self.siguiente() % (i as u64 + 1)).unwrap_or(0);
            lista.swap(i, j);
        }
    }
}

/// Lee identificadores desde un punto al azar, dando la vuelta si no llegan.
///
/// `sql` recibe el punto de partida como `?1` (y como `?2` el primer parámetro
/// propio, si lo hay) y tiene que filtrar con `>= ?1` en un orden de índice.
fn muestra(
    conn: &Connection,
    sql_desde: &str,
    sql_antes: &str,
    parametro: Option<&str>,
    azar: &mut Azar,
    cuantos: usize,
) -> NbResult<Vec<String>> {
    let inicio = azar.inicio();
    let leer = |sql: &str| -> NbResult<Vec<String>> {
        let mut stmt = conn.prepare(sql).map_err(|e| fallo(&e))?;
        let limite = i64::try_from(cuantos).unwrap_or(i64::MAX);
        let mut parametros: Vec<&dyn rusqlite::ToSql> = vec![&inicio];
        if let Some(p) = &parametro {
            parametros.push(p);
        }
        parametros.push(&limite);
        stmt.query_map(parametros.as_slice(), |f| f.get(0))
            .and_then(Iterator::collect::<rusqlite::Result<Vec<String>>>)
            .map_err(|e| fallo(&e))
    };
    let mut ids = leer(sql_desde)?;
    if ids.len() < cuantos {
        ids.extend(leer(sql_antes)?);
    }
    Ok(ids)
}

/// El nombre con el que la importación agrupa las obras sin un autor claro. No es
/// un autor, es la ausencia de uno, y una fila "de Varios autores" no dice nada.
const VARIOS_AUTORES: &str = "Varios autores";

/// Las obras de un autor, barajadas.
///
/// **Por nombre y no por persona, y es un defecto medido.** La importación crea
/// una fila de `contributors` por obra: en la base real, un autor con 23 obras
/// son 24 personas con una obra cada una. Buscar por identificador daba una sola
/// obra, y la fila "porque lees" no llegaba nunca al mínimo. Por nombre usa
/// `idx_contributors_nombre` (V29), y hasta el autor con más obras de la
/// biblioteca real, 1.977, son unas dos mil búsquedas por índice.
fn obras_de_autor(conn: &Connection, nombre: &str, azar: &mut Azar) -> NbResult<Vec<String>> {
    let mut obras: Vec<String> = conn
        .prepare(
            "SELECT DISTINCT ec.edition_id FROM contributors c
               JOIN edition_contributors ec ON ec.contributor_id = c.id
              WHERE c.name = ?1 AND ec.role = 'author'",
        )
        .and_then(|mut s| {
            s.query_map([nombre], |f| f.get(0))
                .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;
    azar.baraja(&mut obras);
    Ok(obras)
}

/// Lo que ya se ha puesto en otra fila no se repite: la misma obra dos veces en
/// una pantalla es exactamente lo que se describió como "duplicados".
struct Repartidas {
    vistas: std::collections::HashSet<String>,
    empezadas: std::collections::HashSet<String>,
}

impl Repartidas {
    /// Se queda con las obras de una fila, entre las que caben en `cuantas`, o con
    /// ninguna.
    ///
    /// Si no llegan al mínimo, la fila no se pinta y sus obras **quedan libres**
    /// para otra: una fila descartada que se hubiera quedado con cinco obras las
    /// habría quitado de la pantalla sin enseñarlas.
    fn quedarse(
        &mut self,
        candidatas: Vec<String>,
        sin_empezadas: bool,
        cuantas: &std::ops::RangeInclusive<usize>,
    ) -> Vec<String> {
        let mut elegidas = Vec::new();
        let mut en_esta = std::collections::HashSet::new();
        for id in candidatas {
            if elegidas.len() >= *cuantas.end() {
                break;
            }
            if self.vistas.contains(&id)
                || (sin_empezadas && self.empezadas.contains(&id))
                || !en_esta.insert(id.clone())
            {
                continue;
            }
            elegidas.push(id);
        }
        if elegidas.len() < *cuantas.start() {
            return Vec::new();
        }
        self.vistas.extend(elegidas.iter().cloned());
        elegidas
    }
}

/// Una fila antes de resolver sus obras: su identificador, su clase, su forma y
/// los identificadores de las obras que se quedó.
type FilaPorPintar = (String, HomeRowKind, HomeRowLayout, Vec<String>);

/// Las filas de Inicio para una visita.
///
/// # Qué cambia en cada visita y qué no (enmienda de ADR-013, 2026-09-12)
///
/// La `semilla` la pone la interfaz al entrar en Inicio. Con ella cambia **qué
/// materias y qué autores** se enseñan y qué obras salen en las filas de
/// descubrir; lo que no cambia es lo que responde a lo que el usuario hace:
/// novedades y "porque lees". ADR-013 pedía recomendaciones deterministas para
/// que Inicio no empujara; se pidió que no se quedara quieta, y la enmienda
/// las hizo **reproducibles**: la misma semilla da la misma pantalla, y cada fila
/// sigue diciendo por qué aparece.
///
/// # Por qué cabe en la biblioteca real
///
/// Ninguna fila ordena la biblioteca: las novedades salen del índice de fecha, las
/// muestras al azar de un rango del índice de identificadores (ver
/// [`Azar::inicio`]), y las materias y autores de los recuentos materializados de
/// V24. Cada fila son unas pocas búsquedas por índice y veinte filas de pintar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn filas(conn: &Connection, semilla: u64) -> NbResult<Vec<HomeRow>> {
    let mut azar = Azar(semilla);
    let empezadas: std::collections::HashSet<String> = conn
        .prepare("SELECT edition_id FROM progress WHERE profile = 'default'")
        .and_then(|mut s| {
            s.query_map([], |f| f.get(0))
                .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;
    let mut repartidas = Repartidas {
        vistas: std::collections::HashSet::new(),
        empezadas,
    };
    let mut salida: Vec<FilaPorPintar> = Vec::new();

    // 1. Novedades: lo último que entró. Fija entre visitas.
    let novedades: Vec<String> = conn
        .prepare("SELECT id FROM editions ORDER BY created_at DESC, id LIMIT ?1")
        .and_then(|mut s| {
            s.query_map([i64::try_from(OBRAS_POR_FILA * 2).unwrap_or(40)], |f| {
                f.get(0)
            })
            .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;
    // 2. Porque lees: del autor de lo último que se ha leído, hasta dos libros.
    let recientes: Vec<(String, String)> = conn
        .prepare(
            "SELECT p.edition_id, e.title FROM progress p JOIN editions e ON e.id = p.edition_id
              WHERE p.profile = 'default'
              ORDER BY p.updated_at DESC LIMIT 4",
        )
        .and_then(|mut s| {
            s.query_map([], |f| Ok((f.get(0)?, f.get(1)?)))
                .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;
    let mut porque = Vec::new();
    let mut autores_usados = std::collections::HashSet::new();
    for (edicion, titulo) in recientes {
        if porque.len() >= 2 {
            break;
        }
        let nombre: Option<String> = conn
            .query_row(
                "SELECT c.name FROM edition_contributors ec
                   JOIN contributors c ON c.id = ec.contributor_id
                  WHERE ec.edition_id = ?1 AND ec.role = 'author'
                  ORDER BY ec.position LIMIT 1",
                [&edicion],
                |f| f.get(0),
            )
            .ok();
        let Some(nombre) = nombre else {
            continue;
        };
        if nombre == VARIOS_AUTORES || !autores_usados.insert(nombre.clone()) {
            continue;
        }
        let candidatas = obras_de_autor(conn, &nombre, &mut azar)?;
        porque.push((
            format!("porque-{edicion}"),
            HomeRowKind::PorqueLees {
                title: titulo,
                author: nombre,
            },
            candidatas,
        ));
    }
    // Lo que responde a lo que se lee se reparte **antes** que las novedades,
    // aunque se pinte después: en una biblioteca pequeña las novedades se
    // quedarían con todo, y la fila del autor que se está leyendo, que es la que
    // más dice, se quedaría vacía.
    let porque: Vec<(String, HomeRowKind, Vec<String>)> = porque
        .into_iter()
        .map(|(id, kind, candidatas)| (id, kind, repartidas.quedarse(candidatas, true, &FILA)))
        .collect();
    let ids = repartidas.quedarse(novedades, true, &FILA);
    salida.push((
        "novedades".into(),
        HomeRowKind::Novedades,
        HomeRowLayout::Portadas,
        ids,
    ));

    // 3. Materias: cuatro de las dieciséis mayores, distintas en cada visita.
    let mut materias: Vec<(String, String)> = conn
        .prepare(
            "SELECT shelf_key, label FROM shelf_counts
              WHERE kind = 'subject' AND tally >= ?1
              ORDER BY tally DESC, label LIMIT 16",
        )
        .and_then(|mut s| {
            s.query_map([i64::try_from(MINIMO_POR_FILA).unwrap_or(8)], |f| {
                Ok((f.get(0)?, f.get(1)?))
            })
            .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;
    azar.baraja(&mut materias);
    materias.truncate(4);

    // 4. Autores con muchas obras: tres de los doscientos mayores, sin repetir el
    // de una fila "porque lees".
    let minimo = i64::try_from(MINIMO_POR_FILA).unwrap_or(8);
    let total_autores: i64 = conn
        .query_row(
            "SELECT count(*) FROM (SELECT 1 FROM shelf_counts
              WHERE kind = 'author' AND tally >= ?1 AND label <> ?2 LIMIT 200)",
            rusqlite::params![minimo, VARIOS_AUTORES],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))?;
    let mut autores: Vec<String> = Vec::new();
    // Seis intentos para tres autores: dos pueden caer en el mismo, o en el de
    // "porque lees", y no merece la pena insistir más para una fila.
    for _ in 0..6 {
        if total_autores == 0 || autores.len() >= 3 {
            break;
        }
        let desplazamiento =
            i64::try_from(azar.siguiente() % u64::try_from(total_autores).unwrap_or(1))
                .unwrap_or(0);
        let nombre: Option<String> = conn
            .query_row(
                "SELECT label FROM shelf_counts
                  WHERE kind = 'author' AND tally >= ?1 AND label <> ?2
                  ORDER BY tally DESC, label LIMIT 1 OFFSET ?3",
                rusqlite::params![minimo, VARIOS_AUTORES, desplazamiento],
                |f| f.get(0),
            )
            .ok();
        if let Some(nombre) = nombre {
            if !autores.contains(&nombre) && !autores_usados.contains(&nombre) {
                autores.push(nombre);
            }
        }
    }

    // 5. Colecciones del usuario, las que tengan con qué llenar una fila.
    let colecciones: Vec<(String, String)> = conn
        .prepare(
            "SELECT c.id, c.name FROM collections c
              WHERE c.kind = 'manual'
                AND (SELECT count(*) FROM collection_items ci WHERE ci.collection_id = c.id) >= ?1
              ORDER BY c.updated_at DESC LIMIT 3",
        )
        .and_then(|mut s| {
            s.query_map([i64::try_from(MINIMO_POR_FILA).unwrap_or(8)], |f| {
                Ok((f.get(0)?, f.get(1)?))
            })
            .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;

    // El orden de la pantalla: lo que responde a lo que se hace, entrelazado con
    // lo que invita a descubrir, para que no sean dos bloques.
    let mut porque = porque.into_iter();
    if let Some((id, kind, ids)) = porque.next() {
        salida.push((id, kind, HomeRowLayout::Portadas, ids));
    }

    let mut materias = materias.into_iter();
    let mut fila_de_materia = |repartidas: &mut Repartidas,
                               azar: &mut Azar,
                               layout: HomeRowLayout|
     -> NbResult<Option<FilaPorPintar>> {
        let Some((clave, etiqueta)) = materias.next() else {
            return Ok(None);
        };
        let candidatas = muestra(
            conn,
            "SELECT edition_id FROM edition_subjects
              WHERE edition_id >= ?1 AND subject_id = ?2
              ORDER BY edition_id LIMIT ?3",
            "SELECT edition_id FROM edition_subjects
              WHERE edition_id < ?1 AND subject_id = ?2
              ORDER BY edition_id LIMIT ?3",
            Some(&clave),
            azar,
            OBRAS_POR_FILA * 2,
        )?;
        Ok(Some((
            format!("materia-{clave}"),
            HomeRowKind::Materia {
                key: clave,
                label: etiqueta,
            },
            layout,
            repartidas.quedarse(candidatas, true, &FILA),
        )))
    };

    if let Some(fila) = fila_de_materia(&mut repartidas, &mut azar, HomeRowLayout::Grandes)? {
        salida.push(fila);
    }

    let fila_de_autor =
        |nombre: &str, repartidas: &mut Repartidas, azar: &mut Azar| -> NbResult<FilaPorPintar> {
            let candidatas = obras_de_autor(conn, nombre, azar)?;
            Ok((
                format!("autor-{nombre}"),
                HomeRowKind::Autor(nombre.to_owned()),
                HomeRowLayout::Portadas,
                repartidas.quedarse(candidatas, true, &FILA),
            ))
        };

    let mut autores = autores.into_iter();
    if let Some(nombre) = autores.next() {
        salida.push(fila_de_autor(&nombre, &mut repartidas, &mut azar)?);
    }

    // Lo que nunca se ha abierto, en portadas grandes.
    let candidatas = muestra(
        conn,
        "SELECT id FROM editions WHERE id >= ?1 ORDER BY id LIMIT ?2",
        "SELECT id FROM editions WHERE id < ?1 ORDER BY id LIMIT ?2",
        None,
        &mut azar,
        OBRAS_POR_FILA * 3,
    )?;
    let ids = repartidas.quedarse(candidatas, true, &FILA);
    salida.push((
        "redescubrir".into(),
        HomeRowKind::Redescubrir,
        HomeRowLayout::Portadas,
        ids,
    ));

    if let Some(fila) = fila_de_materia(&mut repartidas, &mut azar, HomeRowLayout::Portadas)? {
        salida.push(fila);
    }

    if let Some((id, kind, ids)) = porque.next() {
        salida.push((id, kind, HomeRowLayout::Portadas, ids));
    }

    for (id, nombre) in colecciones {
        let candidatas: Vec<String> = conn
            .prepare(
                "SELECT edition_id FROM collection_items WHERE collection_id = ?1
                  ORDER BY position, added_at LIMIT ?2",
            )
            .and_then(|mut s| {
                s.query_map(
                    rusqlite::params![&id, i64::try_from(OBRAS_POR_FILA * 2).unwrap_or(40)],
                    |f| f.get(0),
                )
                .and_then(Iterator::collect::<rusqlite::Result<_>>)
            })
            .map_err(|e| fallo(&e))?;
        // En una colección sí caben las empezadas: es del usuario, y la ordenó él.
        let ids = repartidas.quedarse(candidatas, false, &FILA);
        salida.push((
            format!("coleccion-{id}"),
            HomeRowKind::Coleccion { id, name: nombre },
            HomeRowLayout::Portadas,
            ids,
        ));
    }

    if let Some(fila) = fila_de_materia(&mut repartidas, &mut azar, HomeRowLayout::Portadas)? {
        salida.push(fila);
    }

    // Una selección para hoy, en lista: diez, que es lo que se lee de un vistazo.
    let candidatas = muestra(
        conn,
        "SELECT id FROM editions WHERE id >= ?1 ORDER BY id LIMIT ?2",
        "SELECT id FROM editions WHERE id < ?1 ORDER BY id LIMIT ?2",
        None,
        &mut azar,
        OBRAS_POR_FILA * 2,
    )?;
    let ids = repartidas.quedarse(candidatas, true, &LISTA);
    salida.push((
        "al-azar".into(),
        HomeRowKind::AlAzar,
        HomeRowLayout::Lista,
        ids,
    ));

    if let Some(nombre) = autores.next() {
        salida.push(fila_de_autor(&nombre, &mut repartidas, &mut azar)?);
    }

    if let Some(fila) = fila_de_materia(&mut repartidas, &mut azar, HomeRowLayout::Grandes)? {
        salida.push(fila);
    }

    if let Some(nombre) = autores.next() {
        salida.push(fila_de_autor(&nombre, &mut repartidas, &mut azar)?);
    }

    // Y lo que no llena una fila no se pinta. La lista, con diez, es la excepción
    // por construcción: su forma es de diez.
    salida
        .into_iter()
        .filter(|(_, _, layout, ids)| {
            let minimo = if *layout == HomeRowLayout::Lista {
                EN_LISTA
            } else {
                MINIMO_POR_FILA
            };
            ids.len() >= minimo
        })
        .map(|(id, kind, layout, ids)| {
            Ok(HomeRow {
                id,
                kind,
                layout,
                items: crate::browse::items_por_id(conn, &ids)?,
            })
        })
        .collect()
}
