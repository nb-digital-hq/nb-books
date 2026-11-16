//! El cuaderno de estudio: lo generado que alguien decide quedarse.
//!
//! Lo que se comprueba es lo que el plan pide de lo guardado: **editable,
//! descartable y siempre marcado como generado**, y que lo que manda el webview se
//! valide igual que lo que devuelve un modelo.

use nb_books_lib::ai::cuaderno::{self, EdicionDeTarjeta, NuevaTarjeta, NuevoConcepto};
use nb_books_lib::ai::salidas::TipoDeConcepto;
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::reader::{Locations, Locator};

fn edicion(db: &Database, titulo: &str) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: titulo.to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición")
    .id
}

fn locator(href: &str) -> Locator {
    Locator::new(
        href,
        "application/xhtml+xml",
        Locations {
            progression: Some(0.25),
            ..Locations::default()
        },
        None,
    )
    .expect("localizador")
}

fn tarjeta(edicion: &str) -> NuevaTarjeta {
    NuevaTarjeta {
        edition_id: edicion.to_owned(),
        front: "¿Quién imprimió la gramática de 1492?".to_owned(),
        back: "Nebrija la publicó en Salamanca".to_owned(),
        options: None,
        correct: None,
        sources: vec![locator("c1.xhtml")],
        provider: "local".to_owned(),
        template_id: "estudio.tarjetas".to_owned(),
        template_version: 1,
    }
}

fn pregunta(edicion: &str) -> NuevaTarjeta {
    NuevaTarjeta {
        front: "¿En qué año llegó la imprenta a Sevilla?".to_owned(),
        back: "El texto lo fecha en 1477.".to_owned(),
        options: Some(vec![
            "1450".into(),
            "1477".into(),
            "1492".into(),
            "1520".into(),
        ]),
        correct: Some(1),
        template_id: "estudio.preguntas".to_owned(),
        ..tarjeta(edicion)
    }
}

/// Se guarda con su procedencia, se edita sin perderla y se descarta.
#[test]
fn lo_guardado_se_edita_sin_perder_que_es_generado_y_se_descarta() {
    let db = Database::open_in_memory().expect("base");
    let libro = edicion(&db, "Historia de la imprenta");
    let mut conn = db.connection().expect("conexión");

    let ids = cuaderno::guardar_tarjetas(&mut conn, &[tarjeta(&libro), pregunta(&libro)])
        .expect("guardar");
    assert_eq!(ids.len(), 2);

    let guardadas = cuaderno::tarjetas(&conn, Some(&libro), false, 50, 0).expect("listar");
    assert_eq!(guardadas.len(), 2);
    let p = guardadas
        .iter()
        .find(|t| t.options.is_some())
        .expect("pregunta");
    assert_eq!(p.correct, Some(1));
    assert_eq!(p.template, "estudio.preguntas@1");
    assert_eq!(p.title, "Historia de la imprenta");
    assert_eq!(p.sources, vec![locator("c1.xhtml")]);

    let t = guardadas
        .iter()
        .find(|t| t.options.is_none())
        .expect("tarjeta");
    cuaderno::editar_tarjeta(
        &conn,
        &t.id,
        &EdicionDeTarjeta {
            front: "¿Quién escribió la gramática castellana de 1492?".to_owned(),
            back: "Antonio de Nebrija".to_owned(),
            options: None,
            correct: None,
        },
    )
    .expect("editar");
    let editada = cuaderno::tarjetas(&conn, None, false, 50, 0)
        .expect("listar")
        .into_iter()
        .find(|x| x.id == t.id)
        .expect("sigue");
    assert_eq!(editada.back, "Antonio de Nebrija");
    assert!(editada.edited_at.is_some());
    assert_eq!(editada.provider, "local", "editar no borra quién la generó");
    assert_eq!(editada.template, "estudio.tarjetas@1");

    cuaderno::borrar_tarjeta(&conn, &t.id).expect("descartar");
    assert_eq!(
        cuaderno::tarjetas(&conn, None, false, 50, 0)
            .expect("listar")
            .len(),
        1
    );
}

/// La base no admite algo generado sin decir quién lo generó.
#[test]
fn la_base_no_admite_nada_generado_sin_procedencia() {
    let db = Database::open_in_memory().expect("base");
    let libro = edicion(&db, "Libro");
    let conn = db.connection().expect("conexión");
    let sin_proveedor = conn.execute(
        "INSERT INTO study_cards (edition_id, kind, front, back, sources, provider, template)
         VALUES (?1, 'flashcard', 'a', 'b', '{\"locators\":[]}', '  ', 'estudio.tarjetas@1')",
        [&libro],
    );
    assert!(sin_proveedor.is_err());
    let sin_plantilla = conn.execute(
        "INSERT INTO concepts (edition_id, name, kind, sources, provider, template)
         VALUES (?1, 'x', 'concepto', '{\"locators\":[]}', 'local', '')",
        [&libro],
    );
    assert!(sin_plantilla.is_err());
}

/// Lo que manda el webview se valida como lo que devuelve un modelo, y un lote con
/// una sola mala no guarda ninguna.
#[test]
fn lo_que_no_cumple_no_se_guarda_y_no_deja_nada_a_medias() {
    let db = Database::open_in_memory().expect("base");
    let libro = edicion(&db, "Libro");
    let mut conn = db.connection().expect("conexión");

    let malas = [
        NuevaTarjeta {
            back: "¿Quién imprimió la gramática de 1492?".to_owned(),
            ..tarjeta(&libro)
        },
        NuevaTarjeta {
            options: Some(vec!["a".into(), "A".into(), "b".into()]),
            correct: Some(0),
            ..pregunta(&libro)
        },
        NuevaTarjeta {
            correct: Some(4),
            ..pregunta(&libro)
        },
        NuevaTarjeta {
            options: None,
            ..pregunta(&libro)
        },
        NuevaTarjeta {
            template_version: 99,
            ..tarjeta(&libro)
        },
        NuevaTarjeta {
            template_id: "otra.cosa".to_owned(),
            ..tarjeta(&libro)
        },
        NuevaTarjeta {
            sources: vec![locator("c.xhtml"); 25],
            ..tarjeta(&libro)
        },
        NuevaTarjeta {
            front: "x".repeat(601),
            ..tarjeta(&libro)
        },
    ];
    for mala in malas {
        assert_eq!(
            cuaderno::guardar_tarjetas(&mut conn, &[tarjeta(&libro), mala.clone()]),
            Err(NbError::Internal),
            "{mala:?}"
        );
    }
    assert!(cuaderno::tarjetas(&conn, None, false, 50, 0)
        .expect("listar")
        .is_empty());

    let ids = cuaderno::guardar_tarjetas(&mut conn, &[tarjeta(&libro)]).expect("buena");
    assert_eq!(
        cuaderno::editar_tarjeta(
            &conn,
            &ids[0],
            &EdicionDeTarjeta {
                front: "¿Pregunta?".to_owned(),
                back: String::new(),
                options: Some(vec!["a".into(), "b".into(), "c".into()]),
                correct: Some(0),
            },
        ),
        Err(NbError::Internal),
        "una tarjeta no se convierte en pregunta"
    );
}

fn concepto(edicion: &str, nombre: &str, href: &str) -> NuevoConcepto {
    NuevoConcepto {
        edition_id: edicion.to_owned(),
        name: nombre.to_owned(),
        kind: TipoDeConcepto::Persona,
        description: "Gramático".to_owned(),
        sources: vec![locator(href)],
        provider: "local".to_owned(),
        template_id: "estudio.conceptos".to_owned(),
        template_version: 1,
    }
}

/// Un concepto es uno por libro, y el mismo nombre en dos libros es la arista del
/// grafo.
#[test]
fn un_concepto_repetido_junta_sus_fuentes_y_se_reconoce_en_otros_libros() {
    let db = Database::open_in_memory().expect("base");
    let uno = edicion(&db, "Uno");
    let dos = edicion(&db, "Dos");
    let mut conn = db.connection().expect("conexión");

    cuaderno::guardar_conceptos(&mut conn, &[concepto(&uno, "Nebrija", "c1.xhtml")])
        .expect("primero");
    cuaderno::guardar_conceptos(&mut conn, &[concepto(&uno, "nebrija", "c2.xhtml")])
        .expect("otra vez");
    cuaderno::guardar_conceptos(&mut conn, &[concepto(&dos, "Nebrija", "d1.xhtml")])
        .expect("otro libro");

    let del_uno = cuaderno::conceptos(&conn, Some(&uno), 50, 0).expect("listar");
    assert_eq!(del_uno.len(), 1, "se duplicó en el mismo libro");
    assert_eq!(
        del_uno[0].sources,
        vec![locator("c1.xhtml"), locator("c2.xhtml")]
    );
    assert_eq!(del_uno[0].other_editions, 1);

    cuaderno::editar_concepto(
        &conn,
        &del_uno[0].id,
        "Antonio de Nebrija",
        TipoDeConcepto::Persona,
        "Autor",
    )
    .expect("editar");
    let editado = &cuaderno::conceptos(&conn, Some(&uno), 50, 0).expect("listar")[0];
    assert_eq!(editado.name, "Antonio de Nebrija");
    assert!(editado.edited_at.is_some());
    assert_eq!(editado.provider, "local");

    cuaderno::borrar_concepto(&conn, &editado.id).expect("borrar");
    assert_eq!(
        cuaderno::conceptos(&conn, None, 50, 0)
            .expect("listar")
            .len(),
        1
    );
}

/// Lo guardado es trabajo del usuario: viaja en la exportación, con su marca de
/// generado y sus fuentes, y vuelve igual.
#[test]
fn lo_guardado_viaja_en_la_exportacion_con_su_procedencia() {
    use nb_books_lib::portability;

    let origen = Database::open_in_memory().expect("base");
    let libro = edicion(&origen, "Libro");
    {
        let mut conn = origen.connection().expect("conexión");
        let ids = cuaderno::guardar_tarjetas(&mut conn, &[tarjeta(&libro), pregunta(&libro)])
            .expect("tarjetas");
        cuaderno::editar_tarjeta(
            &conn,
            &ids[0],
            &EdicionDeTarjeta {
                front: "¿Quién escribió la gramática?".to_owned(),
                back: "Nebrija".to_owned(),
                options: None,
                correct: None,
            },
        )
        .expect("editar");
        cuaderno::guardar_conceptos(&mut conn, &[concepto(&libro, "Nebrija", "c1.xhtml")])
            .expect("conceptos");
        conn.execute(
            "INSERT INTO ai_sends (protocol, host, model, capability, body, bytes, status)
             VALUES ('openai', 'ia.example', 'm', 'summary', '{\"x\":1}', 7, 200)",
            [],
        )
        .expect("envío");
    }
    let antes = portability::export(&origen.connection().expect("conexión")).expect("exportar");
    assert_eq!(antes.study_cards.len(), 2);
    assert_eq!(antes.concepts.len(), 1);

    let destino = Database::open_in_memory().expect("base");
    portability::import(&mut destino.connection().expect("conexión"), &antes).expect("importar");
    let despues = portability::export(&destino.connection().expect("conexión")).expect("exportar");
    assert_eq!(despues.study_cards, antes.study_cards);
    assert_eq!(despues.concepts, antes.concepts);
    assert_eq!(
        antes.ai_sends.len(),
        1,
        "el registro de envíos también viaja"
    );
    assert_eq!(despues.ai_sends, antes.ai_sends);

    let conn = destino.connection().expect("conexión");
    let restauradas = cuaderno::tarjetas(&conn, None, false, 50, 0).expect("listar");
    assert!(restauradas
        .iter()
        .any(|t| t.edited_at.is_some() && t.provider == "local"));
    assert_eq!(restauradas[0].sources, vec![locator("c1.xhtml")]);
}

/// Cada tarjeta guarda cuándo toca repasarla, contestarla la programa según
/// lo que se recordó, estudio enseña las que tocan hoy, y viaja al exportar.
#[test]
fn repaso_espaciado_programa_y_filtra_tarjetas_y_viaja_al_exportar() {
    use nb_books_lib::ai::repaso_espaciado::CalificacionRepaso;

    let db = Database::open_in_memory().expect("base");
    let libro = edicion(&db, "Filosofía del lenguaje");
    let mut conn = db.connection().expect("conexión");

    let ids = cuaderno::guardar_tarjetas(&mut conn, &[tarjeta(&libro), pregunta(&libro)])
        .expect("guardar");
    assert_eq!(ids.len(), 2);

    // 1. Estado inicial: programadas para hoy, sin repeticiones previas.
    let cuentas = cuaderno::cuentas_de_tarjetas(&conn, Some(&libro)).expect("cuentas");
    assert_eq!(cuentas.total, 2);
    assert_eq!(cuentas.due, 2, "las tarjetas nuevas tocan hoy");

    let pendientes = cuaderno::tarjetas(&conn, Some(&libro), true, 50, 0).expect("pendientes");
    assert_eq!(pendientes.len(), 2);
    assert_eq!(pendientes[0].repetitions, 0);
    assert_eq!(pendientes[0].interval_days, 0);
    assert!((pendientes[0].ease_factor - 2.5).abs() < f64::EPSILON);
    assert!(pendientes[0].last_reviewed_at.is_none());

    // 2. Repasar la primera tarjeta con Good -> pasa a 1 día en el futuro.
    let respondida = cuaderno::responder_tarjeta(&mut conn, &ids[0], CalificacionRepaso::Good)
        .expect("responder");
    assert_eq!(respondida.repetitions, 1);
    assert_eq!(respondida.interval_days, 1);
    assert!((respondida.ease_factor - 2.5).abs() < f64::EPSILON);
    assert!(respondida.last_reviewed_at.is_some());

    // 3. Ahora sólo queda 1 pendiente hoy, aunque el total sigue siendo 2.
    let cuentas_despues = cuaderno::cuentas_de_tarjetas(&conn, Some(&libro)).expect("cuentas");
    assert_eq!(cuentas_despues.total, 2);
    assert_eq!(
        cuentas_despues.due, 1,
        "solo una tarjeta sigue pendiente hoy"
    );

    let solo_hoy = cuaderno::tarjetas(&conn, Some(&libro), true, 50, 0).expect("pendientes");
    assert_eq!(solo_hoy.len(), 1);
    assert_eq!(solo_hoy[0].id, ids[1]);

    let todas = cuaderno::tarjetas(&conn, Some(&libro), false, 50, 0).expect("todas");
    assert_eq!(todas.len(), 2);

    // 4. Exportar y verificar que los campos de repaso espaciado viajan intactos.
    let exportado = nb_books_lib::portability::export(&conn).expect("exportar");
    let destino = Database::open_in_memory().expect("base destino");
    let mut conn_destino = destino.connection().expect("conexión destino");
    nb_books_lib::portability::import(&mut conn_destino, &exportado).expect("importar");

    let cuentas_restauradas =
        cuaderno::cuentas_de_tarjetas(&conn_destino, Some(&libro)).expect("cuentas restauradas");
    assert_eq!(cuentas_restauradas.total, 2);
    assert_eq!(cuentas_restauradas.due, 1);

    let restaurada = cuaderno::tarjeta(&conn_destino, &ids[0]).expect("tarjeta restaurada");
    assert_eq!(restaurada.repetitions, 1);
    assert_eq!(restaurada.interval_days, 1);
    assert!((restaurada.ease_factor - 2.5).abs() < f64::EPSILON);
    assert!(restaurada.last_reviewed_at.is_some());
    assert_eq!(restaurada.due_at, respondida.due_at);
}
