//! Tableros de notas por columnas con nombre.
//!
//! # Qué es un tablero, y qué no
//!
//! Es una **vista** sobre las notas: dice en qué columna está cada una
//! y en qué orden, y nada más. No guarda notas, no las duplica y borrar el
//! tablero, una columna o una tarjeta no pierde ninguna.
//!
//! # Por qué columnas con nombre y no una rejilla
//!
//! Antes era una rejilla de 6x8, y no se entendía, por una causa concreta: una celda no
//! tiene nombre, así que poner una nota y no en la 4 no decía nada, y
//! un tablero que no dice nada no ordena nada. Una columna **se llama** algo
//! -"Por leer", "Tesis", "Descartado"-, y mover una tarjeta de una a otra es
//! decir algo de esa nota.
//!
//! # Lo que se conserva de la rejilla, que era lo que la justificaba
//!
//! * **Finito.** Un lienzo infinito no puede respetar el presupuesto de 1.500
//!   nodos. Aquí la cota son [`MAXIMO_DE_COLUMNAS`] y [`MAXIMO_DE_TARJETAS`], y lo
//!   que se pinta está acotado por construcción.
//! * **Operable con teclado sin inventar nada.** "La siguiente" significa la
//!   tarjeta de debajo o la columna de al lado, igual que en una rejilla.
//! * **Equivalente en lista sobre los mismos datos.** [`contenido`] devuelve las
//!   columnas con sus tarjetas en orden, y la lista y las columnas se pintan de
//!   esa misma respuesta.
//!
//! # Por qué esto es núcleo y no una extensión
//!
//! Por lo mismo que, y no cambia con la forma: los datos son las notas
//! del usuario, y un tablero es interacción y no contenido. Ver la sección 6.3.1
//! de `plugin-system.md`.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// Columnas de un tablero, como mucho.
///
/// Ocho. Con columnas de unos 15 rem, ocho son dos filas en una ventana ancha:
/// más columnas que eso ya no se leen como un tablero sino como una lista de
/// listas, y para eso están las notas.
pub const MAXIMO_DE_COLUMNAS: usize = 8;

/// Tarjetas de un tablero, como mucho, sumando todas sus columnas.
///
/// Ciento veinte. Sale del presupuesto de nodos y no del gusto: una tarjeta pinta
/// unos seis nodos y una columna unos quince, así que un tablero lleno ronda los
/// ochocientos cincuenta, holgado bajo los 1.500 del documento, que además tiene
/// que pintar la carcasa y la navegación de Estudio. Para más, un tablero enlaza
/// otro.
pub const MAXIMO_DE_TARJETAS: usize = 120;

/// Cuántas palabras del cuerpo viajan como anticipo.
const PALABRAS_DE_ANTICIPO: usize = 16;

/// Un tablero, con cuántas tarjetas tiene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Board {
    pub id: String,
    pub title: String,
    pub created_at: String,
    /// Cuántas tarjetas hay. Es lo que permite decir "vacío" en la lista sin
    /// abrir el tablero.
    pub filled: u32,
}

/// Una columna con sus tarjetas, en orden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct BoardColumn {
    pub id: String,
    pub title: String,
    pub cards: Vec<Card>,
}

/// Una tarjeta: una nota o un enlace a otro tablero, nunca las dos.
///
/// Dos opcionales en lugar de un enumerado porque es lo que el puente sabe
/// serializar sin inventar una envoltura, y la base impide que lleguen las dos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Card {
    pub id: String,
    /// La nota de la tarjeta.
    pub note_id: Option<String>,
    /// Su título, que es lo que se pinta. Viaja resuelto para que la interfaz
    /// no tenga que cruzar dos listas.
    pub title: Option<String>,
    /// Las primeras palabras del cuerpo, para reconocerla sin abrirla.
    pub preview: Option<String>,
    /// El tablero al que lleva, si la tarjeta es un enlace.
    pub links_to: Option<String>,
    /// Y su título.
    pub links_to_title: Option<String>,
}

fn fallo(error: &rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "falló una operación sobre un tablero");
    NbError::StorageFailure
}

/// Un defecto de quien llama y no de quien usa la aplicación.
///
/// La interfaz pinta el tablero con sus límites, así que **no puede** pedir una
/// columna de otro tablero, una novena columna o un nombre vacío. Si llega, el
/// defecto es nuestro, y eso es justo lo que `Internal` significa.
fn defecto(motivo: &'static str) -> NbError {
    tracing::error!(motivo, "petición imposible sobre un tablero");
    NbError::Internal
}

fn nombre(titulo: &str) -> NbResult<&str> {
    let recortado = titulo.trim();
    if recortado.is_empty() {
        // Un tablero o una columna sin nombre no se pueden señalar, y lo que dice
        // una columna es precisamente su nombre.
        return Err(defecto("nombre vacío"));
    }
    Ok(recortado)
}

/// Crea un tablero con sus primeras columnas.
///
/// Las columnas las pone quien lo crea, en su idioma: un tablero vacío de
/// columnas no se puede usar, y el runtime no sabe cómo se llaman "Ideas" o "Por
/// revisar" en el idioma de la interfaz.
///
/// # Errors
///
/// - [`NbError::Internal`] si el nombre viene vacío o sobran columnas.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn crear(conn: &mut Connection, title: &str, columnas: &[String]) -> NbResult<Board> {
    let titulo = nombre(title)?;
    if columnas.len() > MAXIMO_DE_COLUMNAS {
        return Err(defecto("demasiadas columnas al crear"));
    }
    let nombres = columnas
        .iter()
        .map(|c| nombre(c))
        .collect::<NbResult<Vec<_>>>()?;

    let tx = crate::db::write_transaction(conn)?;
    let id = identificador(&tx, "brd")?;
    tx.execute(
        "INSERT INTO note_boards (id, title) VALUES (?1, ?2)",
        rusqlite::params![id, titulo],
    )
    .map_err(|e| fallo(&e))?;
    for (posicion, columna) in nombres.iter().enumerate() {
        tx.execute(
            "INSERT INTO note_board_columns (id, board_id, title, position)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![identificador(&tx, "col")?, id, columna, posicion as i64],
        )
        .map_err(|e| fallo(&e))?;
    }
    tx.commit().map_err(|e| fallo(&e))?;

    encontrar(conn, &id)?.ok_or(NbError::DocumentNotFound)
}

/// Un tablero por su identificador.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn encontrar(conn: &Connection, id: &str) -> NbResult<Option<Board>> {
    conn.query_row(
        "SELECT b.id, b.title, b.created_at,
                (SELECT COUNT(*) FROM note_board_cards c WHERE c.board_id = b.id)
           FROM note_boards b WHERE b.id = ?1",
        [id],
        de_fila,
    )
    .optional()
    .map_err(|e| fallo(&e))
}

fn de_fila(fila: &rusqlite::Row<'_>) -> rusqlite::Result<Board> {
    Ok(Board {
        id: fila.get(0)?,
        title: fila.get(1)?,
        created_at: fila.get(2)?,
        filled: fila.get::<_, i64>(3)?.try_into().unwrap_or(u32::MAX),
    })
}

/// Todos los tableros, del más reciente al más antiguo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn listar(conn: &Connection, limite: u32, desde: u32) -> NbResult<Vec<Board>> {
    let mut consulta = conn
        .prepare(
            "SELECT b.id, b.title, b.created_at,
                    (SELECT COUNT(*) FROM note_board_cards c WHERE c.board_id = b.id)
               FROM note_boards b ORDER BY b.created_at DESC, b.title
              LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| fallo(&e))?;
    let filas = consulta
        .query_map(rusqlite::params![limite, desde], de_fila)
        .map_err(|e| fallo(&e))?;
    filas.collect::<Result<Vec<_>, _>>().map_err(|e| fallo(&e))
}

/// Borra un tablero. Las notas siguen donde estaban.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn borrar(conn: &Connection, id: &str) -> NbResult<bool> {
    let quitados = conn
        .execute("DELETE FROM note_boards WHERE id = ?1", [id])
        .map_err(|e| fallo(&e))?;
    Ok(quitados > 0)
}

/// Las columnas de un tablero con sus tarjetas, **en orden**.
///
/// Es la única lectura del tablero: las columnas y la lista equivalente se pintan
/// de esta misma respuesta, así que no pueden decir cosas distintas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn contenido(conn: &Connection, board_id: &str) -> NbResult<Vec<BoardColumn>> {
    let mut columnas: Vec<BoardColumn> = conn
        .prepare(
            "SELECT id, title FROM note_board_columns
              WHERE board_id = ?1 ORDER BY position, id",
        )
        .and_then(|mut s| {
            s.query_map([board_id], |f| {
                Ok(BoardColumn {
                    id: f.get(0)?,
                    title: f.get(1)?,
                    cards: Vec::new(),
                })
            })
            .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))?;

    let mut consulta = conn
        .prepare(
            "SELECT t.column_id, t.id, t.note_id, n.title, n.body, t.links_to, d.title
               FROM note_board_cards t
               JOIN note_board_columns c ON c.id = t.column_id
               LEFT JOIN notes n ON n.id = t.note_id
               LEFT JOIN note_boards d ON d.id = t.links_to
              WHERE t.board_id = ?1
              ORDER BY c.position, c.id, t.position, t.id",
        )
        .map_err(|e| fallo(&e))?;
    let tarjetas = consulta
        .query_map([board_id], |f| {
            let cuerpo: Option<String> = f.get(4)?;
            Ok((
                f.get::<_, String>(0)?,
                Card {
                    id: f.get(1)?,
                    note_id: f.get(2)?,
                    title: f.get(3)?,
                    preview: cuerpo.map(|texto| anticipo(&texto)),
                    links_to: f.get(5)?,
                    links_to_title: f.get(6)?,
                },
            ))
        })
        .map_err(|e| fallo(&e))?;
    for fila in tarjetas {
        let (columna, tarjeta) = fila.map_err(|e| fallo(&e))?;
        if let Some(destino) = columnas.iter_mut().find(|c| c.id == columna) {
            destino.cards.push(tarjeta);
        }
    }
    Ok(columnas)
}

/// Las primeras palabras de un cuerpo, para reconocer la nota sin abrirla.
fn anticipo(cuerpo: &str) -> String {
    let palabras: Vec<&str> = cuerpo
        .split_whitespace()
        .take(PALABRAS_DE_ANTICIPO)
        .collect();
    let texto = palabras.join(" ");
    if cuerpo.split_whitespace().count() > PALABRAS_DE_ANTICIPO {
        format!("{texto}…")
    } else {
        texto
    }
}

/// Añade una columna al final.
///
/// # Errors
///
/// - [`NbError::Internal`] si el nombre viene vacío o el tablero ya tiene todas
///   las columnas que caben.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn anadir_columna(conn: &mut Connection, board_id: &str, title: &str) -> NbResult<()> {
    let titulo = nombre(title)?;
    let tx = crate::db::write_transaction(conn)?;
    let hay = columnas_de(&tx, board_id)?;
    if hay.len() >= MAXIMO_DE_COLUMNAS {
        return Err(defecto("no caben más columnas"));
    }
    tx.execute(
        "INSERT INTO note_board_columns (id, board_id, title, position)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            identificador(&tx, "col")?,
            board_id,
            titulo,
            hay.len() as i64
        ],
    )
    .map_err(|e| fallo(&e))?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Cambia el nombre de una columna.
///
/// # Errors
///
/// - [`NbError::Internal`] si el nombre viene vacío o la columna no es del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn renombrar_columna(
    conn: &Connection,
    board_id: &str,
    column_id: &str,
    title: &str,
) -> NbResult<()> {
    let titulo = nombre(title)?;
    let cambiadas = conn
        .execute(
            "UPDATE note_board_columns SET title = ?1 WHERE id = ?2 AND board_id = ?3",
            rusqlite::params![titulo, column_id, board_id],
        )
        .map_err(|e| fallo(&e))?;
    if cambiadas == 0 {
        return Err(defecto("la columna no es de este tablero"));
    }
    Ok(())
}

/// Borra una columna y sus tarjetas. **Las notas siguen existiendo**: una
/// tarjeta solo las señala.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn borrar_columna(conn: &mut Connection, board_id: &str, column_id: &str) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    let mut columnas = columnas_de(&tx, board_id)?;
    let Some(lugar) = columnas.iter().position(|c| c == column_id) else {
        return Err(defecto("la columna no es de este tablero"));
    };
    tx.execute("DELETE FROM note_board_columns WHERE id = ?1", [column_id])
        .map_err(|e| fallo(&e))?;
    columnas.remove(lugar);
    renumerar_columnas(&tx, &columnas)?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Lleva una columna a otro lugar del tablero. Un lugar de más la deja al final.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn mover_columna(
    conn: &mut Connection,
    board_id: &str,
    column_id: &str,
    lugar: usize,
) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    let mut columnas = columnas_de(&tx, board_id)?;
    let Some(desde) = columnas.iter().position(|c| c == column_id) else {
        return Err(defecto("la columna no es de este tablero"));
    };
    let columna = columnas.remove(desde);
    columnas.insert(lugar.min(columnas.len()), columna);
    renumerar_columnas(&tx, &columnas)?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Pone una nota al final de una columna.
///
/// **Si la nota ya estaba en el tablero, se mueve** en lugar de repetirse: una
/// nota en dos columnas a la vez contradice lo que dicen las columnas, y quien la
/// pone en otra está diciendo dónde la quiere ahora.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero o no caben más
///   tarjetas.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn poner_nota(
    conn: &mut Connection,
    board_id: &str,
    column_id: &str,
    note_id: &str,
) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    poner_en(&tx, board_id, column_id, Contenido::Nota(note_id))?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Pone al final de una columna un enlace a otro tablero.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero, no caben más
///   tarjetas, o el tablero se enlaza a sí mismo: una tarjeta que lleva al
///   tablero donde está no lleva a ninguna parte.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn poner_enlace(
    conn: &mut Connection,
    board_id: &str,
    column_id: &str,
    destino: &str,
) -> NbResult<()> {
    if destino == board_id {
        return Err(defecto("un tablero enlazado consigo mismo"));
    }
    let tx = crate::db::write_transaction(conn)?;
    poner_en(&tx, board_id, column_id, Contenido::Enlace(destino))?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Crea una nota y la pone al final de una columna, **en una sola escritura**.
///
/// Es el gesto que faltaba: para usar un tablero había que ir a Notas, escribir,
/// volver y colocar. Si la tarjeta no se puede poner, la nota tampoco se crea; una
/// nota que aparece sin que su tarjeta aparezca parecería perdida.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero o no caben más
///   tarjetas.
/// - Lo que devuelva [`crate::notes::create`].
pub fn crear_nota_en(
    conn: &mut Connection,
    board_id: &str,
    column_id: &str,
    title: &str,
    body: &str,
) -> NbResult<crate::notes::Note> {
    let tx = crate::db::write_transaction(conn)?;
    let nota = crate::notes::create(&tx, title, body)?;
    poner_en(&tx, board_id, column_id, Contenido::Nota(&nota.id))?;
    tx.commit().map_err(|e| fallo(&e))?;
    Ok(nota)
}

/// Lleva una tarjeta a un lugar de una columna, la misma u otra. Un lugar de más
/// la deja al final.
///
/// # Errors
///
/// - [`NbError::Internal`] si la tarjeta o la columna no son del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn mover_tarjeta(
    conn: &mut Connection,
    board_id: &str,
    card_id: &str,
    column_id: &str,
    lugar: usize,
) -> NbResult<()> {
    let tx = crate::db::write_transaction(conn)?;
    if !columnas_de(&tx, board_id)?.iter().any(|c| c == column_id) {
        return Err(defecto("la columna no es de este tablero"));
    }
    let origen: String = tx
        .query_row(
            "SELECT column_id FROM note_board_cards WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![card_id, board_id],
            |f| f.get(0),
        )
        .optional()
        .map_err(|e| fallo(&e))?
        .ok_or_else(|| defecto("la tarjeta no es de este tablero"))?;

    let mut de_origen = tarjetas_de(&tx, &origen)?;
    de_origen.retain(|t| t != card_id);
    let mut de_destino = if origen == column_id {
        de_origen.clone()
    } else {
        tarjetas_de(&tx, column_id)?
    };
    de_destino.insert(lugar.min(de_destino.len()), card_id.to_owned());

    if origen != column_id {
        renumerar_tarjetas(&tx, &origen, &de_origen)?;
    }
    renumerar_tarjetas(&tx, column_id, &de_destino)?;
    tx.commit().map_err(|e| fallo(&e))
}

/// Quita una tarjeta del tablero. La nota sigue existiendo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn quitar_tarjeta(conn: &mut Connection, board_id: &str, card_id: &str) -> NbResult<bool> {
    let tx = crate::db::write_transaction(conn)?;
    let columna: Option<String> = tx
        .query_row(
            "SELECT column_id FROM note_board_cards WHERE id = ?1 AND board_id = ?2",
            rusqlite::params![card_id, board_id],
            |f| f.get(0),
        )
        .optional()
        .map_err(|e| fallo(&e))?;
    let Some(columna) = columna else {
        return Ok(false);
    };
    tx.execute("DELETE FROM note_board_cards WHERE id = ?1", [card_id])
        .map_err(|e| fallo(&e))?;
    let quedan = tarjetas_de(&tx, &columna)?;
    renumerar_tarjetas(&tx, &columna, &quedan)?;
    tx.commit().map_err(|e| fallo(&e))?;
    Ok(true)
}

enum Contenido<'a> {
    Nota(&'a str),
    Enlace(&'a str),
}

/// Pone algo al final de una columna, moviéndolo si ya estaba en el tablero.
/// Va dentro de la transacción de quien llama.
fn poner_en(
    conn: &Connection,
    board_id: &str,
    column_id: &str,
    contenido: Contenido<'_>,
) -> NbResult<()> {
    if !columnas_de(conn, board_id)?.iter().any(|c| c == column_id) {
        return Err(defecto("la columna no es de este tablero"));
    }
    let (columna_nota, columna_enlace, valor) = match contenido {
        Contenido::Nota(nota) => ("note_id", "links_to", nota),
        Contenido::Enlace(destino) => ("links_to", "note_id", destino),
    };
    let ya: Option<String> = conn
        .query_row(
            &format!("SELECT id FROM note_board_cards WHERE board_id = ?1 AND {columna_nota} = ?2"),
            rusqlite::params![board_id, valor],
            |f| f.get(0),
        )
        .optional()
        .map_err(|e| fallo(&e))?;
    if let Some(tarjeta) = ya {
        // Ya estaba: se lleva al final de la columna pedida.
        let origen: String = conn
            .query_row(
                "SELECT column_id FROM note_board_cards WHERE id = ?1",
                [&tarjeta],
                |f| f.get(0),
            )
            .map_err(|e| fallo(&e))?;
        let mut de_origen = tarjetas_de(conn, &origen)?;
        de_origen.retain(|t| *t != tarjeta);
        renumerar_tarjetas(conn, &origen, &de_origen)?;
        let mut de_destino = tarjetas_de(conn, column_id)?;
        de_destino.retain(|t| *t != tarjeta);
        de_destino.push(tarjeta);
        return renumerar_tarjetas(conn, column_id, &de_destino);
    }

    let total: i64 = conn
        .query_row(
            "SELECT count(*) FROM note_board_cards WHERE board_id = ?1",
            [board_id],
            |f| f.get(0),
        )
        .map_err(|e| fallo(&e))?;
    if usize::try_from(total).unwrap_or(usize::MAX) >= MAXIMO_DE_TARJETAS {
        return Err(defecto("no caben más tarjetas"));
    }
    let al_final = tarjetas_de(conn, column_id)?.len();
    conn.execute(
        &format!(
            "INSERT INTO note_board_cards (id, board_id, column_id, position, {columna_nota}, {columna_enlace})
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)"
        ),
        rusqlite::params![
            identificador(conn, "crd")?,
            board_id,
            column_id,
            al_final as i64,
            valor
        ],
    )
    .map_err(|e| fallo(&e))?;
    Ok(())
}

fn columnas_de(conn: &Connection, board_id: &str) -> NbResult<Vec<String>> {
    conn.prepare("SELECT id FROM note_board_columns WHERE board_id = ?1 ORDER BY position, id")
        .and_then(|mut s| {
            s.query_map([board_id], |f| f.get(0))
                .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))
}

fn tarjetas_de(conn: &Connection, column_id: &str) -> NbResult<Vec<String>> {
    conn.prepare("SELECT id FROM note_board_cards WHERE column_id = ?1 ORDER BY position, id")
        .and_then(|mut s| {
            s.query_map([column_id], |f| f.get(0))
                .and_then(Iterator::collect::<rusqlite::Result<_>>)
        })
        .map_err(|e| fallo(&e))
}

/// Deja las posiciones seguidas desde cero. Con huecos o repetidas, "la tarjeta
/// de debajo" dejaría de estar bien definida, y mover con el teclado depende de
/// eso.
fn renumerar_columnas(conn: &Connection, columnas: &[String]) -> NbResult<()> {
    for (posicion, columna) in columnas.iter().enumerate() {
        conn.execute(
            "UPDATE note_board_columns SET position = ?1 WHERE id = ?2",
            rusqlite::params![posicion as i64, columna],
        )
        .map_err(|e| fallo(&e))?;
    }
    Ok(())
}

fn renumerar_tarjetas(conn: &Connection, column_id: &str, tarjetas: &[String]) -> NbResult<()> {
    for (posicion, tarjeta) in tarjetas.iter().enumerate() {
        conn.execute(
            "UPDATE note_board_cards SET column_id = ?1, position = ?2 WHERE id = ?3",
            rusqlite::params![column_id, posicion as i64, tarjeta],
        )
        .map_err(|e| fallo(&e))?;
    }
    Ok(())
}

/// Identificador nuevo: un prefijo que dice qué es y dieciséis bytes al azar.
///
/// Al azar y no por reloj, que es lo que había: crear un tablero con tres columnas
/// escribe tres filas en el mismo instante, y con el reloj podían chocar. El azar
/// lo pone SQLite, que ya lo tiene, en lugar de traer una dependencia para esto.
fn identificador(conn: &Connection, prefijo: &str) -> NbResult<String> {
    conn.query_row(
        "SELECT ?1 || '-' || lower(hex(randomblob(16)))",
        [prefijo],
        |f| f.get(0),
    )
    .map_err(|e| fallo(&e))
}
