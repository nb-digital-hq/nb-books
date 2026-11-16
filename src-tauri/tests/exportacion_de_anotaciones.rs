//! Criterio de aceptación de la.
//!
//! "Se exporta [...] en Markdown y en texto plano, con su cita y su localizador."
//!
//! El JSON lo entienden otras herramientas; estos dos archivos los lee una
//! persona. Así que las pruebas no miran estructura sino lo que quien lo abre tiene
//! que encontrar: cada cita, cada nota, dónde está cada una -para una persona y para
//! una máquina-, en el orden del libro y en el idioma de la interfaz.
//!
//! Contrato: `docs/architecture/api/annotation-export.md`.

mod comun;

use comun::estado::{estado_aislado, invocar};
use nb_books_lib::annotations::texto::{exportar, Formato};
use nb_books_lib::annotations::{self, NewAnnotation};
use nb_books_lib::db::catalog::{AnnotationKind, Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::reader::{Locations, Locator, TextContext};

fn edicion(db: &Database) -> String {
    let conn = db.connection().expect("conexión");
    edicion_en(&conn)
}

fn edicion_en(conn: &rusqlite::Connection) -> String {
    let obra = Work::create(
        conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    Edition::create(
        conn,
        &NewEdition {
            work_id: obra.id,
            title: "Cien años de soledad".to_owned(),
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

fn sitio(href: &str, capitulo: &str, char: u32, total: f64, cita: &str) -> Locator {
    Locator::new(
        href,
        "application/xhtml+xml",
        Locations {
            fragments: vec![format!("#char={char}")],
            progression: None,
            total_progression: Some(total),
            position: None,
        },
        Some(TextContext::new(None, cita, None).expect("cita")),
    )
    .expect("locator")
    .with_title(capitulo)
}

/// Una edición con tres anotaciones creadas **fuera del orden del libro**: la última
/// del libro primero. Si la exportación siguiera el orden de creación, lo diría.
fn biblioteca() -> (Database, String) {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");
    let crear = |kind, body: Option<&str>, target| {
        annotations::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.clone(),
                kind,
                body: body.map(str::to_owned),
                target,
            },
        )
        .expect("anotación");
    };
    crear(
        AnnotationKind::Note,
        Some("Aquí se cierra el círculo"),
        sitio(
            "OEBPS/cap20.xhtml",
            "Capítulo 20",
            900,
            0.97,
            "estaba escrito",
        ),
    );
    crear(
        AnnotationKind::Highlight,
        None,
        sitio(
            "OEBPS/cap01.xhtml",
            "Capítulo 1",
            120,
            0.012,
            "frente al pelotón de fusilamiento",
        ),
    );
    crear(
        AnnotationKind::Underline,
        None,
        sitio(
            "OEBPS/cap01.xhtml",
            "Capítulo 1",
            400,
            0.03,
            "Macondo era entonces",
        ),
    );
    drop(conn);
    (db, edicion)
}

fn texto_de(formato: Formato, idioma: &str) -> String {
    let (db, edicion) = biblioteca();
    let conn = db.connection().expect("conexión");
    exportar(&conn, &edicion, formato, idioma).expect("exportación")
}

#[test]
fn el_markdown_lleva_cada_cita_nota_y_localizador_en_el_orden_del_libro() {
    let md = texto_de(Formato::Markdown, "es");

    assert!(md.starts_with("# Cien años de soledad\n"), "{md}");
    assert!(md.contains("*3 anotaciones*"), "{md}");
    // Cada cita como cita de Markdown, que es lo que un visor pinta como tal.
    for cita in [
        "frente al pelotón de fusilamiento",
        "Macondo era entonces",
        "estaba escrito",
    ] {
        assert!(
            md.contains(&format!("> {cita}\n")),
            "falta la cita {cita}:\n{md}"
        );
    }
    assert!(md.contains("\nAquí se cierra el círculo\n"), "{md}");
    // El localizador exacto, con el que otra herramienta vuelve al sitio, y el
    // porcentaje, con el que lo encuentra una persona.
    assert!(md.contains("`OEBPS/cap01.xhtml#char=120`"), "{md}");
    assert!(md.contains("`OEBPS/cap20.xhtml#char=900`"), "{md}");
    assert!(md.contains("*Nota · 97 %*"), "{md}");

    let posicion = |aguja: &str| md.find(aguja).unwrap_or_else(|| panic!("falta {aguja}"));
    assert!(posicion("pelotón") < posicion("Macondo"), "{md}");
    assert!(posicion("Macondo") < posicion("estaba escrito"), "{md}");
    // Un encabezado por capítulo, no uno por anotación.
    assert_eq!(md.matches("## Capítulo 1\n").count(), 1, "{md}");
    assert_eq!(md.matches("## Capítulo 20\n").count(), 1, "{md}");
}

#[test]
fn el_texto_plano_no_lleva_marcas_de_markdown_y_conserva_lo_mismo() {
    let txt = texto_de(Formato::Texto, "es");

    assert!(
        txt.starts_with("Cien años de soledad\n3 anotaciones\n"),
        "{txt}"
    );
    assert!(
        !txt.contains("# ") && !txt.contains("> ") && !txt.contains('`'),
        "{txt}"
    );
    assert!(txt.contains("«frente al pelotón de fusilamiento»"), "{txt}");
    assert!(txt.contains("Nota: Aquí se cierra el círculo"), "{txt}");
    assert!(
        txt.contains("[Subrayado · 3 % · OEBPS/cap01.xhtml#char=400]"),
        "{txt}"
    );
    assert!(txt.find("pelotón") < txt.find("estaba escrito"), "{txt}");
}

#[test]
fn la_exportacion_se_escribe_en_el_idioma_de_la_interfaz() {
    let en = texto_de(Formato::Texto, "en");
    assert!(en.contains("3 annotations"), "{en}");
    assert!(en.contains("[Highlight · 1 %"), "{en}");
    assert!(en.contains("Note: Aquí se cierra el círculo"), "{en}");
    assert!(!en.contains("Resaltado"), "{en}");

    let pt = texto_de(Formato::Markdown, "pt-BR");
    assert!(pt.contains("*3 anotações*"), "{pt}");
    assert!(pt.contains("*Sublinhado · 3 %*"), "{pt}");
}

#[test]
fn las_anotaciones_borradas_no_salen_y_un_libro_sin_ellas_no_falla() {
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");
    let md = exportar(&conn, &edicion, Formato::Markdown, "es").expect("vacía");
    assert!(md.contains("*0 anotaciones*"), "{md}");

    let borrada = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: sitio(
                "OEBPS/cap02.xhtml",
                "Capítulo 2",
                10,
                0.1,
                "lo que ya no está",
            ),
        },
    )
    .expect("anotación");
    annotations::delete(&conn, &borrada.id).expect("borrar");
    let md = exportar(&conn, &edicion, Formato::Markdown, "es").expect("exportación");
    assert!(!md.contains("lo que ya no está"), "{md}");
}

#[test]
fn solo_markdown_y_text_son_formatos_de_texto() {
    assert_eq!(Formato::de("markdown"), Some(Formato::Markdown));
    assert_eq!(Formato::de("text"), Some(Formato::Texto));
    // Lo demás cae en el JSON de W3C, que es lo que había: un formato mal escrito no
    // produce un archivo vacío ni un error.
    assert_eq!(Formato::de("json"), None);
    assert_eq!(Formato::de("Markdown"), None);
}

/// El comando que ya existía, pedido como lo pide el webview: con `format` escribe
/// el texto, y sin él sigue escribiendo el JSON de W3C, que no se rompe.
#[test]
fn el_comando_del_puente_escribe_cada_formato_donde_se_le_dice() {
    let (estado, guard) = estado_aislado("exportacion-de-anotaciones");
    let edicion = {
        let conn = estado.db.connection().expect("conexión");
        let edicion = edicion_en(&conn);
        annotations::create(
            &conn,
            &NewAnnotation {
                payload: None,
                edition_id: edicion.clone(),
                kind: AnnotationKind::Note,
                body: Some("Una nota".to_owned()),
                target: sitio("OEBPS/cap03.xhtml", "Capítulo 3", 55, 0.2, "la cita"),
            },
        )
        .expect("anotación");
        edicion
    };
    let app = tauri::test::mock_builder()
        .manage(estado)
        .invoke_handler(tauri::generate_handler![
            nb_books_lib::ipc::save_annotations
        ])
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("la aplicación de prueba se construye");
    let ventana = tauri::WebviewWindowBuilder::new(&app, "principal", Default::default())
        .build()
        .expect("la ventana de prueba se crea");

    let fuera = guard.path().join("exportado");
    std::fs::create_dir_all(&fuera).expect("carpeta de destino");
    for (formato, archivo) in [
        (Some("markdown"), "a.md"),
        (Some("text"), "a.txt"),
        (None, "a.json"),
    ] {
        let destino = fuera.join(archivo);
        invocar(
            &ventana,
            "save_annotations",
            serde_json::json!({
                "editionId": edicion,
                "path": destino.to_string_lossy(),
                "format": formato,
                "language": "es",
            }),
        )
        .unwrap_or_else(|e| panic!("{archivo}: {e}"));
        let escrito = std::fs::read_to_string(&destino).expect("el archivo existe");
        match formato {
            Some("markdown") => {
                assert!(escrito.contains("> la cita\n"), "{escrito}");
                assert!(escrito.contains("`OEBPS/cap03.xhtml#char=55`"), "{escrito}");
            }
            Some(_) => assert!(escrito.contains("«la cita»"), "{escrito}"),
            None => {
                let json: serde_json::Value = serde_json::from_str(&escrito).expect("JSON");
                assert_eq!(json["type"], "AnnotationPage", "{escrito}");
            }
        }
    }
}
