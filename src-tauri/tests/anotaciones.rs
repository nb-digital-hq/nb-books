//! Criterio de aceptación de la.
//!
//! "Una prueba comprueba que editar una anotación produce un identificador nuevo
//! y conserva la anterior como dada de baja."
//!
//! La inmutabilidad no es purismo. Es lo que hace que "me he cargado la nota
//! buena" tenga arreglo, que la sincronización no tenga que decidir
//! quién escribió último sobre la misma fila, y que exportar el historial sea
//! exportar filas en lugar de reconstruir un pasado que no se guardó.
//!
//! Contratos: `docs/adr/004-reader-locator-model.md` y
//! `docs/architecture/api/schemas.md`.

use nb_books_lib::annotations::{self, Edit, NewAnnotation};
use nb_books_lib::db::catalog::{AnnotationKind, Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::reader::{Locations, Locator, TextContext};

fn edicion(db: &Database) -> String {
    edicion_titulada(db, "Edición")
}

fn edicion_titulada(db: &Database, titulo: &str) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
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

fn ancla(cita: &str) -> Locator {
    Locator::new(
        "OEBPS/cap01.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["epubcfi(/6/4!/4/2)".to_owned()],
            progression: Some(0.42),
            total_progression: Some(0.17),
            position: Some(233),
        },
        Some(TextContext::new(Some("antes de "), cita, Some(" y después")).expect("cita")),
    )
    .expect("locator")
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn editar_una_anotacion_produce_un_identificador_nuevo_y_conserva_la_anterior() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let mut conn = db.connection().expect("conexión");

    let original = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Note,
            body: Some("Primera versión de la nota".to_owned()),
            target: ancla("el pelotón"),
        },
    )
    .expect("anotación");

    let editada = annotations::edit(
        &mut conn,
        &original.id,
        &Edit {
            body: Some(Some("Versión corregida".to_owned())),
            target: None,
        },
    )
    .expect("edición");

    // Identificador nuevo.
    assert_ne!(editada.id, original.id);
    assert_eq!(editada.supersedes.as_deref(), Some(original.id.as_str()));
    assert_eq!(editada.body.as_deref(), Some("Versión corregida"));

    // La anterior se conserva, dada de baja.
    let anterior = annotations::find(&conn, &original.id)
        .expect("consulta")
        .expect("la anterior sigue en la base");
    assert!(!anterior.is_live());
    assert!(anterior.deleted_at.is_some());
    assert_eq!(
        anterior.body.as_deref(),
        Some("Primera versión de la nota"),
        "lo que decía antes no se toca: para eso se conserva"
    );

    // Y la lista de anotaciones vivas tiene una, no dos.
    let vivas = annotations::list(&conn, &edicion).expect("lista");
    assert_eq!(vivas.len(), 1);
    assert_eq!(vivas[0].id, editada.id);
}

// --- Lo que hace cumplir la regla ------------------------------------------

#[test]
fn la_base_rechaza_editar_una_anotacion_en_sitio() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let anotacion = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Note,
            body: Some("Original".to_owned()),
            target: ancla("una frase"),
        },
    )
    .expect("anotación");

    // La regla no puede vivir solo en Rust: quien entre por SQL directo, por una
    // importación o por una versión futura del código tiene que chocar igual.
    for columna in ["body", "target", "kind", "edition_id"] {
        let resultado = conn.execute(
            &format!("UPDATE annotations SET {columna} = 'x' WHERE id = ?1"),
            [&anotacion.id],
        );
        assert!(
            resultado.is_err(),
            "la base dejó cambiar {columna} en sitio"
        );
    }

    let intacta = annotations::find(&conn, &anotacion.id)
        .expect("consulta")
        .expect("anotación");
    assert_eq!(intacta.body.as_deref(), Some("Original"));
}

#[test]
fn una_anotacion_dada_de_baja_no_se_revive_escribiendo_nulo() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let anotacion = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Bookmark,
            body: None,
            target: ancla("da igual"),
        },
    )
    .expect("anotación");
    annotations::delete(&conn, &anotacion.id).expect("baja");

    // Reaparecer sin rastro es lo que este modelo evita: recuperarla es crear una
    // versión nueva que la suceda, y entonces queda registrado que ocurrió.
    assert!(conn
        .execute(
            "UPDATE annotations SET deleted_at = NULL WHERE id = ?1",
            [&anotacion.id]
        )
        .is_err());
}

#[test]
fn editar_una_anotacion_dada_de_baja_no_es_editar() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let mut conn = db.connection().expect("conexión");

    let anotacion = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Note,
            body: Some("Nota".to_owned()),
            target: ancla("frase"),
        },
    )
    .expect("anotación");
    annotations::delete(&conn, &anotacion.id).expect("baja");

    assert_eq!(
        annotations::edit(&mut conn, &anotacion.id, &Edit::default()).err(),
        Some(NbError::DocumentNotFound),
        "resucitar una anotación se hace a propósito o no se hace"
    );
}

// --- El historial -----------------------------------------------------------

#[test]
fn el_historial_se_recorre_desde_cualquiera_de_sus_versiones() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let mut conn = db.connection().expect("conexión");

    let v1 = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Note,
            body: Some("uno".to_owned()),
            target: ancla("frase"),
        },
    )
    .expect("anotación");
    let v2 = annotations::edit(
        &mut conn,
        &v1.id,
        &Edit {
            body: Some(Some("dos".to_owned())),
            target: None,
        },
    )
    .expect("segunda");
    let v3 = annotations::edit(
        &mut conn,
        &v2.id,
        &Edit {
            body: Some(Some("tres".to_owned())),
            target: None,
        },
    )
    .expect("tercera");

    // Desde la primera, desde la del medio y desde la última: la misma cadena.
    // Quien tenga guardado un identificador viejo, en una exportación de hace un
    // año, sigue pudiendo llegar a la versión actual.
    for punto in [&v1.id, &v2.id, &v3.id] {
        let historial = annotations::history(&conn, punto).expect("historial");
        let cuerpos: Vec<_> = historial
            .iter()
            .map(|a| a.body.clone().unwrap_or_default())
            .collect();
        assert_eq!(cuerpos, vec!["uno", "dos", "tres"]);
        assert!(historial[0].deleted_at.is_some());
        assert!(historial[2].is_live());
    }
}

#[test]
fn editar_solo_el_comentario_conserva_el_ancla() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let mut conn = db.connection().expect("conexión");

    let original = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Highlight,
            body: None,
            target: ancla("frase resaltada"),
        },
    )
    .expect("anotación");

    let editada = annotations::edit(
        &mut conn,
        &original.id,
        &Edit {
            body: Some(Some("ahora con comentario".to_owned())),
            target: None,
        },
    )
    .expect("edición");

    // Es lo que se hace casi siempre: añadir un comentario a un resaltado. Que
    // haya que repetir el ancla sería una invitación a equivocarse con ella.
    assert_eq!(editada.target, original.target);
}

// --- Lo que no se acepta ----------------------------------------------------

#[test]
fn un_resaltado_sin_cita_no_se_puede_reanclar_asi_que_no_se_acepta() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let sin_cita = Locator::new(
        "OEBPS/cap01.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["epubcfi(/6/4!/4/2)".to_owned()],
            progression: Some(0.42),
            total_progression: None,
            position: None,
        },
        None,
    )
    .expect("locator sin cita");

    // Sin cita no hay segundo escalón de la cascada, así que ese resaltado se
    // perdería en cuanto el documento cambiase. Aceptarlo sería prometer un
    // reanclaje que no puede ocurrir.
    assert_eq!(
        annotations::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.clone(),
                kind: AnnotationKind::Highlight,
                body: None,
                target: sin_cita.clone(),
            },
        )
        .err(),
        Some(NbError::PathNotAllowed)
    );

    // Un marcador sí: es un punto, no un trozo de texto.
    assert!(annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Bookmark,
            body: None,
            target: sin_cita,
        },
    )
    .is_ok());
}

#[test]
fn una_anotacion_con_el_ancla_ilegible_es_un_error_y_no_una_sin_ancla() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    // Objeto JSON válido que no es un Locator: pasa el CHECK de la base.
    conn.execute(
        "INSERT INTO annotations (edition_id, kind, target)
         VALUES (?1, 'note', '{\"algo\": 1}')",
        [&edicion],
    )
    .expect("inserción");

    // Colocarla al principio del documento sería mover el trabajo del usuario
    // sin decírselo.
    assert_eq!(
        annotations::list(&conn, &edicion).err(),
        Some(NbError::StorageFailure)
    );
}

#[test]
fn dar_de_baja_dos_veces_no_es_un_error_pero_lo_inexistente_si() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let anotacion = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Note,
            body: None,
            target: ancla("frase"),
        },
    )
    .expect("anotación");

    assert!(annotations::delete(&conn, &anotacion.id).is_ok());
    assert!(
        annotations::delete(&conn, &anotacion.id).is_ok(),
        "la interfaz puede pedirlo dos veces sin saberlo"
    );
    assert_eq!(
        annotations::delete(&conn, "00000000000000000000000000000000").err(),
        Some(NbError::DocumentNotFound)
    );
}

/// Las anotaciones de toda la biblioteca, que es lo que pinta la vista de
/// Estudio. Es el mismo dato que el panel del lector con otro alcance.
#[test]
fn la_biblioteca_entera_devuelve_las_anotaciones_con_el_libro_del_que_salieron() {
    let db = Database::open_in_memory().expect("base");
    let uno = edicion_titulada(&db, "Cien Años de Soledad");
    let otro = edicion_titulada(&db, "Pedro Páramo");
    let conn = db.connection().expect("conexión");

    for (edicion, cita) in [(&uno, "el hielo"), (&otro, "vine a Comala")] {
        annotations::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.clone(),
                kind: AnnotationKind::Note,
                body: Some("una nota".to_owned()),
                target: ancla(cita),
            },
        )
        .expect("anotación");
    }

    let todas = annotations::library(&conn, None, 100, 0).expect("biblioteca");
    assert_eq!(todas.len(), 2);

    let titulos: Vec<&str> = todas.iter().map(|a| a.title.as_str()).collect();
    assert!(titulos.contains(&"Cien Años de Soledad"));
    assert!(titulos.contains(&"Pedro Páramo"));
}

/// La regla 1 del contrato de carga: lo que cruza el puente va acotado.
///
/// Sin cota, esta lista la fija quien anota, y quien anota puede llevar diez
/// años haciéndolo. Ver `optimization-catalog.md`, sección 1.3.
#[test]
fn las_anotaciones_de_la_biblioteca_se_piden_por_tramos() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let edicion = edicion(&db);

    for cita in ["primera", "segunda", "tercera"] {
        annotations::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.clone(),
                kind: AnnotationKind::Note,
                body: Some(cita.to_owned()),
                target: ancla(cita),
            },
        )
        .expect("anotación");
    }

    let primera = annotations::library(&conn, None, 2, 0).expect("primer tramo");
    let segunda = annotations::library(&conn, None, 2, 2).expect("segundo tramo");

    assert_eq!(
        primera.len(),
        2,
        "el tramo trae lo que se le pide, no lo que hay"
    );
    assert_eq!(
        segunda.len(),
        1,
        "y el siguiente sigue donde acabó el anterior"
    );

    // Y no repite: dos tramos que devolvieran lo mismo esconderían la mitad.
    let vistas: Vec<&str> = primera
        .iter()
        .chain(segunda.iter())
        .map(|a| a.annotation.id.as_str())
        .collect();
    let mut unicas = vistas.clone();
    unicas.sort_unstable();
    unicas.dedup();
    assert_eq!(
        unicas.len(),
        vistas.len(),
        "un tramo repitió lo del anterior"
    );
}

#[test]
fn las_anotaciones_de_la_biblioteca_van_de_la_mas_reciente_a_la_mas_antigua() {
    // En el lector el orden es de lectura; aquí no puede serlo, porque la
    // progresión de un libro no es comparable con la de otro. Lo único que
    // significa lo mismo entre libros es cuándo se escribió.
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let mut creadas = Vec::new();
    for cita in ["primera", "segunda", "tercera"] {
        creadas.push(
            annotations::create(
                &conn,
                &NewAnnotation {
                    payload: None,
                    edition_id: edicion.clone(),
                    kind: AnnotationKind::Note,
                    body: Some(cita.to_owned()),
                    target: ancla(cita),
                },
            )
            .expect("anotación"),
        );
        // La marca de tiempo tiene milisegundos: sin separarlas, las tres caen
        // en el mismo instante y el orden lo decidiría el desempate por
        // identificador, que no es lo que esta prueba dice comprobar.
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let todas = annotations::library(&conn, None, 100, 0).expect("biblioteca");
    let cuerpos: Vec<&str> = todas
        .iter()
        .filter_map(|a| a.annotation.body.as_deref())
        .collect();
    assert_eq!(cuerpos, vec!["tercera", "segunda", "primera"]);
}

#[test]
fn la_biblioteca_filtra_por_clase_y_no_devuelve_las_dadas_de_baja() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let nota = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Note,
            body: Some("una nota".to_owned()),
            target: ancla("frase"),
        },
    )
    .expect("nota");
    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: ancla("otra frase"),
        },
    )
    .expect("resaltado");

    let resaltados =
        annotations::library(&conn, Some(AnnotationKind::Highlight), 100, 0).expect("resaltados");
    assert_eq!(resaltados.len(), 1);
    assert_eq!(resaltados[0].annotation.kind, AnnotationKind::Highlight);

    annotations::delete(&conn, &nota.id).expect("baja");
    let vivas = annotations::library(&conn, None, 100, 0).expect("vivas");
    assert_eq!(vivas.len(), 1, "las dadas de baja son historial y no salen");
}

#[test]
fn sin_archivo_disponible_no_se_ofrece_con_que_abrirla() {
    // La vista no puede ofrecer un salto que fallaría: sin archivo disponible no
    // hay documento que abrir, y el `file_id` nulo es lo que se lo dice.
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion,
            kind: AnnotationKind::Note,
            body: Some("una nota".to_owned()),
            target: ancla("frase"),
        },
    )
    .expect("anotación");

    let todas = annotations::library(&conn, None, 100, 0).expect("biblioteca");
    assert_eq!(todas[0].file_id, None);
}
