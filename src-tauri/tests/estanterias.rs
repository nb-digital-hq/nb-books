//! El recuento de cada estantería no puede mentir.
//!
//! Un recuento desnormalizado que solo se mantiene en el caso feliz es peor que
//! no tenerlo: el panel dice que un autor tiene doce libros, le quedan once, y
//! nadie se entera. Así que lo que se comprueba aquí no es que sume bien en el
//! alta -eso es lo fácil-, sino que **después de cualquier movimiento el
//! recuento materializado coincida con contarlo a mano**.
//!
//! La comparación contra el recuento calculado es deliberadamente la misma
//! consulta que hacía el panel antes de V24. Es la única forma de comprobar una
//! caché: contra la fuente de la que sale.

use nb_books_lib::db::catalog::{
    Contributor, ContributorRole, Edition, NewEdition, NewWork, Subject, Work,
};
use nb_books_lib::db::Database;
use rusqlite::Connection;

/// El recuento materializado de una estantería, o cero si no tiene fila.
fn materializado(conn: &Connection, clase: &str, clave: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE((SELECT tally FROM shelf_counts WHERE kind = ?1 AND shelf_key = ?2), 0)",
        (clase, clave),
        |fila| fila.get(0),
    )
    .expect("recuento")
}

/// Comprueba que la tabla materializada dice exactamente lo que dice el
/// catálogo, para las tres clases a la vez.
fn coinciden_con_el_catalogo(conn: &Connection) {
    let calculado: Vec<(String, String, i64)> = {
        let mut stmt = conn
            .prepare(
                "SELECT 'author', c.name, count(*)
                   FROM edition_contributors ec
                   JOIN contributors c ON c.id = ec.contributor_id
                  WHERE ec.role = 'author'
                  GROUP BY c.name
                  UNION ALL
                 SELECT 'series', e.series_title, count(*)
                   FROM editions e
                  WHERE e.series_title IS NOT NULL AND trim(e.series_title) <> ''
                  GROUP BY e.series_title
                  UNION ALL
                 SELECT 'subject', s.id, count(*)
                   FROM edition_subjects es
                   JOIN subjects s ON s.id = es.subject_id
                  GROUP BY s.id
                  ORDER BY 1, 2",
            )
            .expect("consulta");
        stmt.query_map([], |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)))
            .expect("filas")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("filas")
    };

    let guardado: Vec<(String, String, i64)> = {
        let mut stmt = conn
            .prepare("SELECT kind, shelf_key, tally FROM shelf_counts ORDER BY kind, shelf_key")
            .expect("consulta");
        stmt.query_map([], |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)))
            .expect("filas")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("filas")
    };

    assert_eq!(
        guardado, calculado,
        "el recuento materializado dejó de coincidir con el catálogo"
    );
}

/// Da de alta una edición con su obra, y devuelve su identificador.
fn edicion(conn: &Connection, titulo: &str, serie: Option<&str>) -> String {
    let obra = Work::create(
        conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: None,
        },
    )
    .expect("obra");

    Edition::create(
        conn,
        &NewEdition {
            work_id: obra.id,
            title: titulo.to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: serie.map(str::to_owned),
            series_index: serie.map(|_| 1.0),
        },
    )
    .expect("edición")
    .id
}

/// Da de alta a una persona y la enlaza como autora.
fn autor(conn: &Connection, edicion_id: &str, nombre: &str) -> String {
    let persona = Contributor::ensure(conn, nombre).expect("contribuidor");
    Contributor::link(conn, edicion_id, &persona.id, ContributorRole::Author, 0).expect("enlace");
    persona.id
}

#[test]
fn el_alta_y_la_baja_de_una_autoria_mueven_el_recuento() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let primera = edicion(&conn, "Primera", None);
    let persona = autor(&conn, &primera, "Autora");
    assert_eq!(materializado(&conn, "author", "Autora"), 1);

    let segunda = edicion(&conn, "Segunda", None);
    Contributor::link(&conn, &segunda, &persona, ContributorRole::Author, 0).expect("enlace");
    assert_eq!(materializado(&conn, "author", "Autora"), 2);
    coinciden_con_el_catalogo(&conn);

    conn.execute(
        "DELETE FROM edition_contributors WHERE edition_id = ?1",
        [&segunda],
    )
    .expect("baja");
    assert_eq!(materializado(&conn, "author", "Autora"), 1);
    coinciden_con_el_catalogo(&conn);
}

/// Volver a enlazar la misma autoría no puede contar dos veces.
///
/// Es el defecto que casi entra con V24: `Contributor::link` usaba
/// `INSERT OR REPLACE`, y el borrado de un `REPLACE` **no dispara** el
/// disparador de baja mientras `recursive_triggers` esté apagado. Se disparaba
/// el alta y no la baja, así que reimportar una biblioteca ya importada iba
/// inflando el recuento de cada autor sin que cambiara ni un dato.
#[test]
fn volver_a_enlazar_la_misma_autoria_no_cuenta_dos_veces() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let obra = edicion(&conn, "Obra", None);
    let persona = autor(&conn, &obra, "Autora");

    for posicion in 0..4 {
        Contributor::link(&conn, &obra, &persona, ContributorRole::Author, posicion)
            .expect("enlace repetido");
    }

    assert_eq!(materializado(&conn, "author", "Autora"), 1);
    coinciden_con_el_catalogo(&conn);
}

/// Borrar una obra se lleva sus enlaces por cascada, y la cascada no dispara
/// nada: si el recuento no se resta desde arriba, se queda alto para siempre.
#[test]
fn borrar_una_obra_resta_de_todas_sus_estanterias() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let primera = edicion(&conn, "Primera", Some("Saga"));
    autor(&conn, &primera, "Autora");
    let materia = Subject::ensure(&conn, "local", "Historia").expect("materia");
    Subject::link(&conn, &primera, &materia.id).expect("enlace");

    let segunda = edicion(&conn, "Segunda", Some("Saga"));
    autor(&conn, &segunda, "Autora");
    Subject::link(&conn, &segunda, &materia.id).expect("enlace");

    assert_eq!(materializado(&conn, "author", "Autora"), 2);
    assert_eq!(materializado(&conn, "series", "Saga"), 2);
    assert_eq!(materializado(&conn, "subject", &materia.id), 2);

    conn.execute("PRAGMA foreign_keys = ON", []).expect("fk");
    conn.execute("DELETE FROM editions WHERE id = ?1", [&segunda])
        .expect("baja de la obra");

    assert_eq!(materializado(&conn, "author", "Autora"), 1);
    assert_eq!(materializado(&conn, "series", "Saga"), 1);
    assert_eq!(materializado(&conn, "subject", &materia.id), 1);
    coinciden_con_el_catalogo(&conn);
}

/// Renombrar a alguien mueve su estantería entera al nombre nuevo.
///
/// Antes renombrar podía además **fundir** dos estanterías, porque dos filas
/// de `contributors` podían llevar el mismo nombre. Desde V38 no pueden: renombrar a
/// una persona con el nombre de otra que ya existe es decir que son la misma, y
/// eso lo rechaza la base en lugar de dejar dos filas para un autor.
#[test]
fn renombrar_a_una_persona_mueve_su_estanteria_y_no_puede_duplicar_a_nadie() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let primera = edicion(&conn, "Primera", None);
    let persona = autor(&conn, &primera, "G. G. Márquez");
    let segunda = edicion(&conn, "Segunda", None);
    autor(&conn, &segunda, "G. G. Márquez");
    let tercera = edicion(&conn, "Tercera", None);
    autor(&conn, &tercera, "Gabriel García Márquez");

    conn.execute(
        "UPDATE contributors SET name = 'Gabo' WHERE id = ?1",
        [&persona],
    )
    .expect("renombrar");
    assert_eq!(materializado(&conn, "author", "G. G. Márquez"), 0);
    assert_eq!(materializado(&conn, "author", "Gabo"), 2);
    coinciden_con_el_catalogo(&conn);

    // Con el nombre de otro, en otra grafía, es la misma persona: no entra.
    let repetido = conn.execute(
        "UPDATE contributors SET name = 'GABRIEL GARCIA MARQUEZ' WHERE id = ?1",
        [&persona],
    );
    assert!(
        repetido.is_err(),
        "dos filas para el mismo autor no pueden existir"
    );
    assert_eq!(materializado(&conn, "author", "Gabo"), 2);
}

/// La serie vive en la propia edición: cambiarla mueve dos estanterías.
#[test]
fn cambiar_la_serie_de_una_obra_mueve_las_dos_estanterias() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let obra = edicion(&conn, "Obra", Some("Primera Saga"));
    assert_eq!(materializado(&conn, "series", "Primera Saga"), 1);

    conn.execute(
        "UPDATE editions SET series_title = 'Otra Saga' WHERE id = ?1",
        [&obra],
    )
    .expect("cambio de serie");

    assert_eq!(materializado(&conn, "series", "Primera Saga"), 0);
    assert_eq!(materializado(&conn, "series", "Otra Saga"), 1);
    coinciden_con_el_catalogo(&conn);

    // Y quitarla del todo la saca de la estantería sin dejar una fila a cero.
    conn.execute(
        "UPDATE editions SET series_title = NULL WHERE id = ?1",
        [&obra],
    )
    .expect("sin serie");
    assert_eq!(materializado(&conn, "series", "Otra Saga"), 0);
    coinciden_con_el_catalogo(&conn);
}

/// Cambiar la etiqueta de una materia no la convierte en otra materia.
#[test]
fn renombrar_una_materia_cambia_lo_que_se_ve_y_no_lo_que_se_cuenta() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let obra = edicion(&conn, "Obra", None);
    let materia = Subject::ensure(&conn, "local", "Historia").expect("materia");
    Subject::link(&conn, &obra, &materia.id).expect("enlace");

    conn.execute(
        "UPDATE subjects SET label = 'Historia Contemporánea' WHERE id = ?1",
        [&materia.id],
    )
    .expect("renombrar");

    let etiqueta: String = conn
        .query_row(
            "SELECT label FROM shelf_counts WHERE kind = 'subject' AND shelf_key = ?1",
            [&materia.id],
            |fila| fila.get(0),
        )
        .expect("etiqueta");

    assert_eq!(etiqueta, "Historia Contemporánea");
    assert_eq!(materializado(&conn, "subject", &materia.id), 1);
    coinciden_con_el_catalogo(&conn);
}

/// La premisa de V24, comprobada contra la versión de SQLite que se compila.
///
/// La documentación de SQLite dice que una acción en cascada dispara los
/// disparadores **solo** si `recursive_triggers` está activo, y aquí no lo está.
/// En 3.53.4 no es cierto: los dispara igual. V24 depende de eso -por eso sus
/// bajas viven sobre las tablas de enlace y no sobre `editions`-, así que la
/// afirmación se comprueba en vez de creerse.
///
/// Si una versión futura vuelve al comportamiento documentado, esto falla y dice
/// qué hay que rehacer, en lugar de dejar los recuentos altos en silencio.
#[test]
fn una_baja_en_cascada_dispara_los_disparadores_de_la_tabla_que_cuelga() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let apagados: i64 = conn
        .query_row("PRAGMA recursive_triggers", [], |fila| fila.get(0))
        .expect("pragma");
    assert_eq!(
        apagados, 0,
        "esta comprobación solo significa algo con los disparadores recursivos apagados"
    );

    let obra = edicion(&conn, "Obra", None);
    autor(&conn, &obra, "Autora");
    assert_eq!(materializado(&conn, "author", "Autora"), 1);

    conn.execute("DELETE FROM editions WHERE id = ?1", [&obra])
        .expect("baja de la obra");

    assert_eq!(
        materializado(&conn, "author", "Autora"),
        0,
        "la cascada no disparó el disparador de baja: V24 tiene que restar desde `editions`"
    );
}

/// Idioma, formato y año se cuentan al vuelo y abren su filtro. El idioma
/// agrupa por la lengua principal, y el año va del más reciente al más antiguo.
#[test]
fn idioma_formato_y_anio_son_estanterias_que_abren_su_filtro() {
    use nb_books_lib::browse::{
        self, BrowseFilter, BrowseQuery, BrowseSort, ShelfKind, ShelfQuery,
    };

    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let a = edicion(&conn, "Uno", None);
    let b = edicion(&conn, "Dos", None);
    let c = edicion(&conn, "Tres", None);
    // Una fecha imposible no hace estantería: es una errata del archivo.
    let errata = edicion(&conn, "Cuatro", None);
    conn.execute(
        "UPDATE editions SET published_date = '2965-01-01' WHERE id = ?1",
        [&errata],
    )
    .expect("errata");
    for (id, idioma, fecha) in [
        (&a, "es", "2019-03-01"),
        (&b, "es-MX", "2019"),
        (&c, "en", "2001-01-01"),
    ] {
        conn.execute(
            "UPDATE editions SET language = ?2, published_date = ?3 WHERE id = ?1",
            (id, idioma, fecha),
        )
        .expect("datos");
    }

    let pedir = |kind| {
        browse::shelves(
            &conn,
            &ShelfQuery {
                kind,
                search: String::new(),
                limit: 20,
            },
        )
        .expect("estanterías")
    };
    let idiomas = pedir(ShelfKind::Language);
    assert_eq!(idiomas.total, 2);
    assert_eq!(
        (idiomas.shelves[0].key.as_str(), idiomas.shelves[0].count),
        ("es", 2)
    );
    let anios: Vec<_> = pedir(ShelfKind::Year)
        .shelves
        .into_iter()
        .map(|s| s.key)
        .collect();
    assert_eq!(anios, ["2019", "2001"]);

    let contar = |filter| {
        browse::browse(
            &conn,
            &BrowseQuery {
                sort: BrowseSort::Title,
                ascending: true,
                filter,
                offset: 0,
                limit: 10,
            },
        )
        .expect("recorrido")
        .total
    };
    assert_eq!(contar(BrowseFilter::Language("es".into())), 2);
    assert_eq!(contar(BrowseFilter::Year("2019".into())), 2);
    assert_eq!(contar(BrowseFilter::Year("2001".into())), 1);
    assert_eq!(contar(BrowseFilter::Format("application/pdf".into())), 0);
}
