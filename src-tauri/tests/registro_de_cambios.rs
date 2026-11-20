//! Lo que un teléfono emparejado recibe depende de que **toda** escritura que
//! le incumbe quede apuntada en el registro de cambios.
//!
//! Se comprueba sobre el esquema vivo y con SQL directo, no con las funciones
//! de la aplicación: el registro lo alimentan disparadores, y lo que importa es
//! que apunten igual escriba quien escriba, incluida una sentencia con su
//! propia política de conflicto.
//!
//! Contratos: `docs/architecture/mobile-contract.md` y ADR-016.

use nb_books_lib::db::Database;
use rusqlite::Connection;

fn version_de(conn: &Connection, entidad: &str, clave: &str) -> Option<(i64, i64)> {
    conn.query_row(
        "SELECT version, borrado FROM registro_de_cambios WHERE entidad = ?1 AND clave = ?2",
        [entidad, clave],
        |f| Ok((f.get(0)?, f.get(1)?)),
    )
    .ok()
}

type Conexion = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

fn base() -> (Database, Conexion) {
    let db = Database::open_in_memory().expect("base en memoria");
    let conn = db.connection().expect("conexión");
    conn.execute_batch(
        "INSERT INTO works (id, canonical_title) VALUES ('w1', 'Obra');
         INSERT INTO editions (id, work_id, title) VALUES ('e1', 'w1', 'Edición');",
    )
    .expect("catálogo mínimo");
    (db, conn)
}

#[test]
fn cada_cosa_tiene_una_sola_fila_y_su_ultimo_cambio_va_al_final() {
    let (_db, conn) = base();
    let (primera, _) = version_de(&conn, "edicion", "e1").expect("el alta se apunta");

    conn.execute("UPDATE editions SET title = 'Otra' WHERE id = 'e1'", [])
        .expect("editar");
    let (segunda, borrado) = version_de(&conn, "edicion", "e1").expect("sigue apuntada");
    assert!(
        segunda > primera,
        "el cambio tiene que quedar después del alta"
    );
    assert_eq!(borrado, 0);

    let filas: i64 = conn
        .query_row(
            "SELECT count(*) FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = 'e1'",
            [],
            |f| f.get(0),
        )
        .expect("cuenta");
    assert_eq!(
        filas, 1,
        "una cosa, una fila: el registro no crece con cada edición"
    );
}

#[test]
fn cambiar_la_obra_o_la_autoria_cuenta_como_cambio_de_la_edicion() {
    let (_db, conn) = base();
    let (antes, _) = version_de(&conn, "edicion", "e1").expect("alta");
    conn.execute(
        "UPDATE works SET canonical_title = 'Nueva' WHERE id = 'w1'",
        [],
    )
    .expect("obra");
    let (tras_obra, _) = version_de(&conn, "edicion", "e1").expect("apuntada");
    assert!(tras_obra > antes);

    conn.execute_batch(
        "INSERT INTO contributors (id, name) VALUES ('c1', 'Autora');
         INSERT INTO edition_contributors (edition_id, contributor_id, role) VALUES ('e1', 'c1', 'author');",
    )
    .expect("autoría");
    let (tras_autoria, _) = version_de(&conn, "edicion", "e1").expect("apuntada");
    assert!(tras_autoria > tras_obra);
}

/// Dentro de un disparador manda la política de conflicto de la sentencia que lo
/// dispara. Una escritura `INSERT OR IGNORE` de la aplicación no puede
/// silenciar el registro.
#[test]
fn una_escritura_con_su_propia_politica_de_conflicto_tambien_se_apunta() {
    let (_db, conn) = base();
    conn.execute(
        "INSERT INTO collections (id, name, kind) VALUES ('k1', 'Lista', 'manual')",
        [],
    )
    .expect("colección");
    let (antes, _) = version_de(&conn, "coleccion", "k1").expect("alta");
    conn.execute(
        "INSERT OR IGNORE INTO collection_items (collection_id, edition_id) VALUES ('k1', 'e1')",
        [],
    )
    .expect("miembro");
    let (despues, _) = version_de(&conn, "coleccion", "k1").expect("apuntada");
    assert!(despues > antes);
}

#[test]
fn lo_borrado_queda_como_lapida_y_no_desaparece_del_registro() {
    let (_db, conn) = base();
    conn.execute("DELETE FROM editions WHERE id = 'e1'", [])
        .expect("borrar");
    let (_, borrado) = version_de(&conn, "edicion", "e1").expect("la lápida sigue");
    assert_eq!(borrado, 1);
}

#[test]
fn una_version_nunca_se_reutiliza_aunque_se_borren_filas() {
    let (_db, conn) = base();
    let (alta, _) = version_de(&conn, "edicion", "e1").expect("alta");
    conn.execute("DELETE FROM registro_de_cambios", [])
        .expect("vaciar");
    conn.execute("UPDATE editions SET title = 'Otra' WHERE id = 'e1'", [])
        .expect("editar");
    let (nueva, _) = version_de(&conn, "edicion", "e1").expect("apuntada");
    assert!(
        nueva > alta,
        "un teléfono que pide desde {alta} se saltaría {nueva}"
    );
}

/// El estado de una tarjeta se deriva de su base y sus repasos, así que una
/// tarjeta nueva necesita una base: la del estado con el que nace.
#[test]
fn una_tarjeta_nueva_parte_del_estado_con_el_que_nace() {
    let (_db, conn) = base();
    conn.execute(
        "INSERT INTO study_cards (id, edition_id, kind, front, back, sources, provider, template, due_at)
         VALUES ('t1', 'e1', 'flashcard', 'anverso', 'reverso', '{}', 'manual', 'manual', '2026-01-01T00:00:00.000Z')",
        [],
    )
    .expect("tarjeta");
    let base: String = conn
        .query_row(
            "SELECT base_due_at FROM study_cards WHERE id = 't1'",
            [],
            |f| f.get(0),
        )
        .expect("base");
    assert_eq!(base, "2026-01-01T00:00:00.000Z");
}
