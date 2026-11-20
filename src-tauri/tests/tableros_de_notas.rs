//! Tableros de notas por columnas con nombre.
//!
//! # El criterio, y cómo se comprueba cada mitad
//!
//! *"Todo tablero tiene equivalente en lista, sobre los mismos datos y operable
//! solo con teclado; un tablero lleno respeta el presupuesto de nodos del
//! documento."* La forma cambió -de rejilla a columnas con nombre- y no el
//! criterio.
//!
//! Que sean **los mismos datos** y que el tablero sea **finito** se comprueba aquí:
//! hay una sola lectura, `contenido`, y el runtime no deja pasar de sus cotas. Que
//! se opere **solo con teclado** y que un tablero lleno quepa en el presupuesto se
//! comprueban en `e2e/flujo.spec.ts` y `e2e/rendimiento.spec.ts`, que es donde hay
//! una interfaz de verdad y nodos que contar.

use nb_books_lib::boards::{self, BoardColumn, MAXIMO_DE_COLUMNAS, MAXIMO_DE_TARJETAS};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::notes;

fn base() -> Database {
    Database::open_in_memory().expect("base")
}

fn nota(conn: &rusqlite::Connection, titulo: &str, cuerpo: &str) -> String {
    notes::create(conn, titulo, cuerpo).expect("nota").id
}

fn columnas(nombres: &[&str]) -> Vec<String> {
    nombres.iter().map(|n| (*n).to_owned()).collect()
}

/// Los títulos de cada columna, en orden: la forma más corta de decir qué hay.
fn forma(contenido: &[BoardColumn]) -> Vec<(String, Vec<String>)> {
    contenido
        .iter()
        .map(|c| {
            (
                c.title.clone(),
                c.cards
                    .iter()
                    .map(|t| {
                        t.title
                            .clone()
                            .or_else(|| t.links_to_title.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn un_tablero_nuevo_trae_sus_columnas_y_se_puede_señalar_por_su_nombre() {
    let db = base();
    let mut conn = db.connection().expect("conexión");

    let tablero = boards::crear(
        &mut conn,
        "  Ideas del capítulo 3  ",
        &columnas(&["Por leer", "Leyendo", "Hecho"]),
    )
    .expect("crea");

    // El nombre se recorta: un tablero que se llama " " no se puede señalar.
    assert_eq!(tablero.title, "Ideas del capítulo 3");
    assert_eq!(tablero.filled, 0);
    assert_eq!(boards::listar(&conn, 100, 0).expect("lista").len(), 1);
    assert_eq!(
        forma(&boards::contenido(&conn, &tablero.id).expect("contenido")),
        vec![
            ("Por leer".to_owned(), vec![]),
            ("Leyendo".to_owned(), vec![]),
            ("Hecho".to_owned(), vec![]),
        ]
    );
}

#[test]
fn ni_un_tablero_ni_una_columna_se_crean_sin_nombre() {
    let db = base();
    let mut conn = db.connection().expect("conexión");

    assert!(matches!(
        boards::crear(&mut conn, "   ", &[]),
        Err(NbError::Internal)
    ));
    assert!(matches!(
        boards::crear(&mut conn, "Tablero", &columnas(&["Bien", " "])),
        Err(NbError::Internal)
    ));
    // Y lo que falló no dejó nada a medias: sin la columna vacía tampoco el tablero.
    assert!(boards::listar(&conn, 100, 0).expect("lista").is_empty());
}

#[test]
fn las_tarjetas_salen_en_el_orden_de_su_columna_y_de_una_sola_lectura() {
    // **El criterio.** No hay dos consultas que puedan discrepar: la lista y las
    // columnas se pintan de `contenido`, en orden.
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["Uno", "Dos"]))
        .expect("crea")
        .id;
    let contenido = boards::contenido(&conn, &tablero).expect("contenido");
    let (uno, dos) = (contenido[0].id.clone(), contenido[1].id.clone());

    let primera = nota(&conn, "Primera", "cuerpo");
    let segunda = nota(&conn, "Segunda", "cuerpo");
    let tercera = nota(&conn, "Tercera", "cuerpo");
    boards::poner_nota(&mut conn, &tablero, &dos, &tercera).expect("pone");
    boards::poner_nota(&mut conn, &tablero, &uno, &primera).expect("pone");
    boards::poner_nota(&mut conn, &tablero, &uno, &segunda).expect("pone");

    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido")),
        vec![
            (
                "Uno".to_owned(),
                vec!["Primera".to_owned(), "Segunda".to_owned()]
            ),
            ("Dos".to_owned(), vec!["Tercera".to_owned()]),
        ]
    );
    assert_eq!(
        boards::encontrar(&conn, &tablero)
            .expect("busca")
            .expect("está")
            .filled,
        3
    );
}

#[test]
fn la_tarjeta_trae_el_titulo_y_un_anticipo_para_reconocerla_sin_abrirla() {
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["Ideas"]))
        .expect("crea")
        .id;
    let columna = boards::contenido(&conn, &tablero).expect("contenido")[0]
        .id
        .clone();
    let cuerpo = (1..=20)
        .map(|n| format!("p{n}"))
        .collect::<Vec<_>>()
        .join(" ");
    let larga = nota(&conn, "Nota larga", &cuerpo);
    boards::poner_nota(&mut conn, &tablero, &columna, &larga).expect("pone");

    let contenido = boards::contenido(&conn, &tablero).expect("contenido");
    let anticipo = contenido[0].cards[0].preview.as_deref().expect("anticipo");

    assert!(anticipo.starts_with("p1 p2 p3"));
    assert!(anticipo.ends_with('…'), "se dice que hay más: {anticipo}");
    assert!(!anticipo.contains("p20"));
}

#[test]
fn una_nota_esta_una_sola_vez_en_cada_tablero() {
    // Una nota en dos columnas a la vez contradice lo que dicen las columnas.
    // Ponerla en otra es decir dónde se la quiere ahora: se mueve, no se repite.
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["Uno", "Dos"]))
        .expect("crea")
        .id;
    let contenido = boards::contenido(&conn, &tablero).expect("contenido");
    let (uno, dos) = (contenido[0].id.clone(), contenido[1].id.clone());
    let una = nota(&conn, "Una", "cuerpo");

    boards::poner_nota(&mut conn, &tablero, &uno, &una).expect("pone");
    boards::poner_nota(&mut conn, &tablero, &dos, &una).expect("vuelve a poner");

    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido")),
        vec![
            ("Uno".to_owned(), vec![]),
            ("Dos".to_owned(), vec!["Una".to_owned()]),
        ]
    );

    // Y en otro tablero sí puede estar: cada tablero es su propia vista.
    let otro = boards::crear(&mut conn, "Otro", &columnas(&["Aquí"]))
        .expect("crea")
        .id;
    let aqui = boards::contenido(&conn, &otro).expect("contenido")[0]
        .id
        .clone();
    boards::poner_nota(&mut conn, &otro, &aqui, &una).expect("pone en otro");
}

#[test]
fn mover_una_tarjeta_la_lleva_a_su_sitio_sin_perder_ni_repetir_ninguna() {
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["Uno", "Dos"]))
        .expect("crea")
        .id;
    let contenido = boards::contenido(&conn, &tablero).expect("contenido");
    let (uno, dos) = (contenido[0].id.clone(), contenido[1].id.clone());
    for titulo in ["A", "B", "C"] {
        let id = nota(&conn, titulo, "cuerpo");
        boards::poner_nota(&mut conn, &tablero, &uno, &id).expect("pone");
    }
    let tarjeta = |conn: &rusqlite::Connection, titulo: &str| {
        boards::contenido(conn, &tablero)
            .expect("contenido")
            .into_iter()
            .flat_map(|c| c.cards)
            .find(|t| t.title.as_deref() == Some(titulo))
            .expect("tarjeta")
            .id
    };

    // Dentro de la misma columna: la C sube al principio.
    let c = tarjeta(&conn, "C");
    boards::mover_tarjeta(&mut conn, &tablero, &c, &uno, 0).expect("mueve");
    // A otra columna: la A pasa a Dos. Un lugar de más la deja al final.
    let a = tarjeta(&conn, "A");
    boards::mover_tarjeta(&mut conn, &tablero, &a, &dos, 99).expect("mueve");

    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido")),
        vec![
            ("Uno".to_owned(), vec!["C".to_owned(), "B".to_owned()]),
            ("Dos".to_owned(), vec!["A".to_owned()]),
        ]
    );

    // Y bajar una a una hasta el final y volver deja la columna como estaba:
    // es lo que hace el teclado, paso a paso.
    let b = tarjeta(&conn, "B");
    boards::mover_tarjeta(&mut conn, &tablero, &b, &uno, 0).expect("sube");
    boards::mover_tarjeta(&mut conn, &tablero, &b, &uno, 1).expect("baja");
    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido"))[0].1,
        vec!["C".to_owned(), "B".to_owned()]
    );
}

#[test]
fn las_columnas_se_renombran_se_reordenan_y_borrarlas_no_pierde_notas() {
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["Uno", "Dos"]))
        .expect("crea")
        .id;
    boards::anadir_columna(&mut conn, &tablero, "Tres").expect("añade");
    let contenido = boards::contenido(&conn, &tablero).expect("contenido");
    let (uno, tres) = (contenido[0].id.clone(), contenido[2].id.clone());
    let una = nota(&conn, "Una", "cuerpo");
    boards::poner_nota(&mut conn, &tablero, &uno, &una).expect("pone");

    boards::renombrar_columna(&conn, &tablero, &tres, "  Descartado ").expect("renombra");
    boards::mover_columna(&mut conn, &tablero, &tres, 0).expect("mueve");
    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido"))
            .into_iter()
            .map(|(t, _)| t)
            .collect::<Vec<_>>(),
        vec!["Descartado", "Uno", "Dos"]
    );

    boards::borrar_columna(&mut conn, &tablero, &uno).expect("borra");
    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido")),
        vec![
            ("Descartado".to_owned(), vec![]),
            ("Dos".to_owned(), vec![]),
        ]
    );
    assert_eq!(
        notes::list(&conn, 100, 0).expect("notas").len(),
        1,
        "una tarjeta señala la nota; borrar la columna no la borra"
    );
}

#[test]
fn una_columna_de_otro_tablero_no_se_puede_tocar_desde_este() {
    // La interfaz pinta el tablero abierto, así que no puede pedir una columna de
    // otro: si llega, el defecto es nuestro, y por eso es `Internal`.
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let uno = boards::crear(&mut conn, "Uno", &columnas(&["A"]))
        .expect("crea")
        .id;
    let dos = boards::crear(&mut conn, "Dos", &columnas(&["B"]))
        .expect("crea")
        .id;
    let de_dos = boards::contenido(&conn, &dos).expect("contenido")[0]
        .id
        .clone();
    let una = nota(&conn, "Una", "cuerpo");

    assert!(matches!(
        boards::poner_nota(&mut conn, &uno, &de_dos, &una),
        Err(NbError::Internal)
    ));
    assert!(matches!(
        boards::renombrar_columna(&conn, &uno, &de_dos, "Otra"),
        Err(NbError::Internal)
    ));
    assert!(matches!(
        boards::borrar_columna(&mut conn, &uno, &de_dos),
        Err(NbError::Internal)
    ));
    assert_eq!(
        forma(&boards::contenido(&conn, &dos).expect("contenido")),
        vec![("B".to_owned(), vec![])]
    );
}

#[test]
fn crear_una_nota_desde_una_columna_la_deja_en_ella_de_una_vez() {
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["Ideas"]))
        .expect("crea")
        .id;
    let ideas = boards::contenido(&conn, &tablero).expect("contenido")[0]
        .id
        .clone();

    let nueva = boards::crear_nota_en(&mut conn, &tablero, &ideas, "Nueva", "escrita aquí")
        .expect("crea la nota en la columna");

    assert_eq!(nueva.title, "Nueva");
    assert_eq!(
        forma(&boards::contenido(&conn, &tablero).expect("contenido")),
        vec![("Ideas".to_owned(), vec!["Nueva".to_owned()])]
    );

    // Y si la tarjeta no se puede poner, la nota tampoco se crea: una nota que
    // aparece sin su tarjeta parecería perdida.
    let otro = boards::crear(&mut conn, "Otro", &columnas(&["X"]))
        .expect("crea")
        .id;
    let de_otro = boards::contenido(&conn, &otro).expect("contenido")[0]
        .id
        .clone();
    assert!(boards::crear_nota_en(&mut conn, &tablero, &de_otro, "Huérfana", "").is_err());
    assert_eq!(notes::list(&conn, 100, 0).expect("notas").len(), 1);
}

#[test]
fn una_tarjeta_lleva_a_otro_tablero_pero_nunca_al_suyo() {
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let uno = boards::crear(&mut conn, "Uno", &columnas(&["A"]))
        .expect("crea")
        .id;
    let dos = boards::crear(&mut conn, "Dos", &[]).expect("crea").id;
    let columna = boards::contenido(&conn, &uno).expect("contenido")[0]
        .id
        .clone();

    boards::poner_enlace(&mut conn, &uno, &columna, &dos).expect("enlaza");
    let contenido = boards::contenido(&conn, &uno).expect("contenido");
    assert_eq!(contenido[0].cards[0].note_id, None);
    assert_eq!(
        contenido[0].cards[0].links_to.as_deref(),
        Some(dos.as_str())
    );
    assert_eq!(contenido[0].cards[0].links_to_title.as_deref(), Some("Dos"));

    // Una tarjeta que lleva al tablero donde está no lleva a ninguna parte.
    assert!(matches!(
        boards::poner_enlace(&mut conn, &uno, &columna, &uno),
        Err(NbError::Internal)
    ));
}

#[test]
fn borrar_una_nota_la_quita_de_los_tableros_y_no_deja_una_tarjeta_rota() {
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["A"]))
        .expect("crea")
        .id;
    let columna = boards::contenido(&conn, &tablero).expect("contenido")[0]
        .id
        .clone();
    let una = nota(&conn, "Una", "cuerpo");
    boards::poner_nota(&mut conn, &tablero, &columna, &una).expect("pone");

    notes::delete(&conn, &una).expect("borra");

    assert!(
        boards::contenido(&conn, &tablero).expect("contenido")[0]
            .cards
            .is_empty(),
        "un tablero no puede señalar a lo que ya no está"
    );
}

#[test]
fn borrar_un_tablero_no_pierde_ni_una_nota() {
    // Un tablero es una **vista** sobre las notas, no un sitio donde vivan.
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let tablero = boards::crear(&mut conn, "Tablero", &columnas(&["A"]))
        .expect("crea")
        .id;
    let columna = boards::contenido(&conn, &tablero).expect("contenido")[0]
        .id
        .clone();
    let una = nota(&conn, "Una", "cuerpo");
    boards::poner_nota(&mut conn, &tablero, &columna, &una).expect("pone");
    let tarjeta = boards::contenido(&conn, &tablero).expect("contenido")[0].cards[0]
        .id
        .clone();
    assert!(boards::quitar_tarjeta(&mut conn, &tablero, &tarjeta).expect("quita"));
    boards::poner_nota(&mut conn, &tablero, &columna, &una).expect("vuelve a poner");

    assert!(boards::borrar(&conn, &tablero).expect("borra"));

    assert_eq!(notes::list(&conn, 100, 0).expect("notas").len(), 1);
    assert!(boards::listar(&conn, 100, 0).expect("lista").is_empty());
}

#[test]
fn el_tablero_es_finito_y_no_deja_pasar_de_sus_cotas() {
    // Un lienzo infinito no tiene forma de respetar el presupuesto: siempre cabe
    // una nota más. Aquí el runtime no deja pasar de las cotas, así que lo que se
    // pinta está acotado por construcción; los nodos pintados se cuentan en
    // `e2e/rendimiento.spec.ts`.
    let db = base();
    let mut conn = db.connection().expect("conexión");
    let nombres: Vec<String> = (0..MAXIMO_DE_COLUMNAS).map(|n| format!("C{n}")).collect();
    let tablero = boards::crear(&mut conn, "Lleno", &nombres)
        .expect("crea")
        .id;
    assert!(matches!(
        boards::anadir_columna(&mut conn, &tablero, "Una más"),
        Err(NbError::Internal)
    ));
    assert!(matches!(
        boards::crear(
            &mut conn,
            "Demasiadas",
            &[nombres.clone(), columnas(&["X"])].concat()
        ),
        Err(NbError::Internal)
    ));

    let primera = boards::contenido(&conn, &tablero).expect("contenido")[0]
        .id
        .clone();
    for n in 0..MAXIMO_DE_TARJETAS {
        let id = nota(&conn, &format!("Nota {n}"), "cuerpo");
        boards::poner_nota(&mut conn, &tablero, &primera, &id).expect("pone");
    }
    let sobra = nota(&conn, "Sobra", "cuerpo");
    assert!(matches!(
        boards::poner_nota(&mut conn, &tablero, &primera, &sobra),
        Err(NbError::Internal)
    ));
    assert_eq!(
        boards::encontrar(&conn, &tablero)
            .expect("busca")
            .expect("está")
            .filled as usize,
        MAXIMO_DE_TARJETAS
    );

    // Unos seis nodos por tarjeta y quince por columna: lo que hace que la cuenta
    // de `rendimiento.spec.ts` no dependa de la suerte.
    const { assert!(MAXIMO_DE_TARJETAS * 6 + MAXIMO_DE_COLUMNAS * 15 < 1500) };
}
