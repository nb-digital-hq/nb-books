//! Criterio de aceptación de la.
//!
//! "El archivo exportado es legible e interpretable sin NB Books."
//!
//! "Sin NB Books" es una afirmación difícil de probar desde dentro de NB Books,
//! así que estas pruebas no usan ni uno de los tipos de la exportación: leen el
//! JSON con `serde_json::Value` y buscan los campos **por su nombre del
//! estándar**, como haría un script de quince líneas escrito por alguien que
//! nunca ha visto este código. Si una prueba tuviera que deserializar con
//! nuestros tipos para entender el archivo, el archivo no sería interoperable.
//!
//! Contrato: `docs/architecture/api/annotation-export.md`.

use nb_books_lib::annotations::{self, NewAnnotation};
use nb_books_lib::db::catalog::{AnnotationKind, Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::reader::{Locations, Locator, TextContext};
use serde_json::Value;

fn edicion(db: &Database) -> String {
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
            title: "Edición".to_owned(),
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

fn locator_de_epub() -> Locator {
    Locator::new(
        "OEBPS/cap05.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["#char=120".to_owned()],
            progression: Some(0.42),
            total_progression: Some(0.17),
            position: Some(233),
        },
        Some(
            TextContext::new(
                Some("Muchos años después, "),
                "frente al pelotón de fusilamiento",
                Some(", el coronel"),
            )
            .expect("cita"),
        ),
    )
    .expect("locator")
}

/// Exporta una biblioteca pequeña con algo de cada clase de anotación.
fn exportar() -> Value {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: locator_de_epub(),
        },
    )
    .expect("resaltado");

    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Note,
            body: Some("Aquí empieza todo".to_owned()),
            target: locator_de_epub(),
        },
    )
    .expect("nota");

    annotations::bookmark(
        &conn,
        &edicion,
        &Locator::new(
            "/",
            "application/vnd.comicbook+zip",
            Locations {
                fragments: vec!["page=12".to_owned()],
                progression: Some(0.0),
                total_progression: Some(0.6),
                position: Some(12),
            },
            None,
        )
        .expect("locator de cómic"),
    )
    .expect("marcador");

    annotations::web::export(&conn, &edicion).expect("exportación")
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn el_archivo_se_anuncia_con_el_vocabulario_del_w3c_y_no_con_uno_propio() {
    let pagina = exportar();

    // Quien abra el archivo dentro de diez años sin saber qué lo escribió tiene
    // aquí la definición de todos los campos que siguen.
    assert_eq!(pagina["@context"], "http://www.w3.org/ns/anno.jsonld");
    assert_eq!(pagina["type"], "AnnotationPage");
    assert_eq!(pagina["items"].as_array().expect("items").len(), 3);
}

#[test]
fn cada_anotacion_es_una_annotation_del_estandar_con_su_motivacion() {
    let pagina = exportar();
    let items = pagina["items"].as_array().expect("items");

    for item in items {
        assert_eq!(item["type"], "Annotation");
        assert!(
            item["id"].as_str().expect("id").starts_with("urn:uuid:"),
            "un URN y no una URL: la anotación no se sirve por HTTP en ningún sitio"
        );
        assert!(item["created"].as_str().is_some());
        assert!(item["target"]["source"].as_str().is_some());
    }

    let motivaciones: Vec<&str> = items
        .iter()
        .map(|i| i["motivation"].as_str().expect("motivación"))
        .collect();
    assert!(motivaciones.contains(&"highlighting"));
    assert!(motivaciones.contains(&"commenting"));
    assert!(motivaciones.contains(&"bookmarking"));
}

#[test]
fn la_cita_se_puede_recuperar_sin_saber_nada_de_nb_books() {
    let pagina = exportar();

    // Esto es literalmente lo que haría un script de otra persona: recorrer los
    // selectores buscando el que sabe leer.
    let cita = pagina["items"]
        .as_array()
        .expect("items")
        .iter()
        .find_map(|item| {
            item["target"]["selector"]
                .as_array()?
                .iter()
                .find(|s| s["type"] == "TextQuoteSelector")
                .cloned()
        })
        .expect("alguna anotación tiene cita");

    assert_eq!(cita["exact"], "frente al pelotón de fusilamiento");
    assert_eq!(cita["prefix"], "Muchos años después, ");
    assert_eq!(cita["suffix"], ", el coronel");
}

#[test]
fn los_selectores_son_varios_y_el_mas_preciso_va_primero() {
    let pagina = exportar();
    let resaltado = pagina["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["motivation"] == "highlighting")
        .expect("resaltado");

    let selectores = resaltado["target"]["selector"]
        .as_array()
        .expect("selectores");

    // Es la cascada de ADR-004 escrita en el vocabulario de otro: quien lea el
    // archivo elige el que sepa aplicar.
    assert_eq!(selectores.len(), 2);
    assert_eq!(
        selectores[0]["type"], "TextPositionSelector",
        "un consumidor sencillo aplica el primero que entiende; el peor primero \
         lo dejaría en un sitio peor del que podíamos ofrecerle"
    );
    assert_eq!(selectores[0]["start"], 120);
    assert_eq!(
        selectores[0]["end"], 153,
        "el final sale de la longitud de la cita, en caracteres"
    );
    assert_eq!(selectores[1]["type"], "TextQuoteSelector");
}

#[test]
fn un_fragmento_de_pagina_declara_a_que_especificacion_se_ajusta() {
    let pagina = exportar();
    let marcador = pagina["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["motivation"] == "bookmarking")
        .expect("marcador");

    let selector = &marcador["target"]["selector"][0];
    assert_eq!(selector["type"], "FragmentSelector");
    assert_eq!(selector["value"], "page=12");
    // Sin `conformsTo`, "page=12" es una cadena cualquiera. Con él, es un
    // fragmento con una definición publicada detrás.
    assert_eq!(selector["conformsTo"], "http://tools.ietf.org/rfc/rfc3778");
}

#[test]
fn el_texto_de_una_nota_viaja_como_cuerpo_textual_del_estandar() {
    let pagina = exportar();
    let nota = pagina["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["motivation"] == "commenting")
        .expect("nota");

    assert_eq!(nota["body"]["type"], "TextualBody");
    assert_eq!(nota["body"]["value"], "Aquí empieza todo");
    assert_eq!(nota["body"]["format"], "text/plain");

    // Y un resaltado sin comentario no lleva cuerpo, en lugar de llevar uno
    // vacío: un cuerpo vacío diría que el usuario escribió una nota en blanco.
    let resaltado = pagina["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["motivation"] == "highlighting")
        .expect("resaltado");
    assert!(resaltado.get("body").is_none());
}

// --- Lo que el estándar no cubre --------------------------------------------

#[test]
fn la_progresion_viaja_fuera_del_vocabulario_estandar() {
    let pagina = exportar();
    let resaltado = pagina["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|i| i["motivation"] == "highlighting")
        .expect("resaltado");

    // El modelo del W3C no tiene selector de progresión. Meterla dentro del
    // vocabulario estándar con un nombre inventado haría que un archivo nuestro
    // pareciera decir algo que el estándar no dice.
    let extra = &resaltado["https://nbdigitallabs.lat/books/ns#locations"];
    assert_eq!(extra["totalProgression"], 0.17);
    assert_eq!(extra["progression"], 0.42);
    assert_eq!(extra["position"], 233);

    // Y quitarla no rompe nada de lo estándar: la cita y la posición siguen ahí.
    let sin_extension: Vec<&String> = resaltado
        .as_object()
        .expect("objeto")
        .keys()
        .filter(|k| !k.starts_with("https://"))
        .collect();
    assert!(sin_extension.iter().any(|k| k.as_str() == "target"));
    assert!(sin_extension.iter().any(|k| k.as_str() == "motivation"));
}

#[test]
fn las_anotaciones_dadas_de_baja_no_se_exportan() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let mut conn = db.connection().expect("conexión");

    let original = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Note,
            body: Some("primera".to_owned()),
            target: locator_de_epub(),
        },
    )
    .expect("anotación");
    let editada = annotations::edit(
        &mut conn,
        &original.id,
        &annotations::Edit {
            body: Some(Some("segunda".to_owned())),
            target: None,
        },
    )
    .expect("edición");

    let pagina = annotations::web::export(&conn, &edicion).expect("exportación");
    let items = pagina["items"].as_array().expect("items");

    // Este archivo sirve para llevarse las anotaciones a otra herramienta, y el
    // historial de versiones no significa nada fuera de aquí. El historial
    // completo viaja en la exportación, que es la que sirve para
    // volver.
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["body"]["value"], "segunda");

    // La versión que sustituyó queda declarada, para quien quiera seguir el hilo.
    assert!(
        items[0]["https://nbdigitallabs.lat/books/ns#locations"]["supersedes"]
            .as_str()
            .expect("supersedes")
            .starts_with("urn:uuid:")
    );
    let _ = editada;
}

#[test]
fn el_identificador_tiene_la_forma_canonica_de_un_uuid() {
    let pagina = exportar();
    let id = pagina["items"][0]["id"].as_str().expect("id");
    let sin_prefijo = id.strip_prefix("urn:uuid:").expect("urn");

    // La base los guarda sin guiones porque los genera con `randomblob`;
    // `urn:uuid` los exige con ellos, y un consumidor que valide el URN
    // rechazaría el archivo entero por esto.
    let trozos: Vec<&str> = sin_prefijo.split('-').collect();
    assert_eq!(
        trozos.iter().map(|t| t.len()).collect::<Vec<_>>(),
        vec![8, 4, 4, 4, 12]
    );
    assert!(sin_prefijo
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == '-'));
}
