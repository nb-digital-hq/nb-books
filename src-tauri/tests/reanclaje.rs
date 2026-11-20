//! Criterio de aceptación de la.
//!
//! "Una prueba modifica el documento de forma que el fragmento canónico deja de
//! resolver, y comprueba que la anotación se recoloca por cita y queda marcada
//! como aproximada."
//!
//! El objetivo de la ficha es más ancho que su criterio y es el que manda:
//! **que ninguna anotación se pierda ni se muestre en un lugar equivocado sin
//! avisar**. Son dos promesas distintas y las dos se comprueban aquí: la que se
//! recoloca queda marcada, y la que no se puede colocar de ninguna forma sale
//! igualmente, en `lost`, en lugar de desaparecer.
//!
//! Contrato: `docs/adr/004-reader-locator-model.md`.

use std::io::Write;
use std::path::PathBuf;

use nb_books_lib::annotations::{self, NewAnnotation};
use nb_books_lib::db::catalog::{AnnotationKind, Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::reader::epub::EpubDocument;
use nb_books_lib::reader::{Locations, Locator, Precision, TextContext};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-reanclaje-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Construye un EPUB con los capítulos que se le den.
fn epub(tmp: &Temporal, nombre: &str, capitulos: &[(&str, &str)]) -> PathBuf {
    let mut entradas: Vec<(String, Vec<u8>, bool)> = vec![
        (
            "mimetype".to_owned(),
            b"application/epub+zip".to_vec(),
            false,
        ),
        (
            "META-INF/container.xml".to_owned(),
            br#"<?xml version="1.0"?><container version="1.0"
                 xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
                 <rootfile full-path="OEBPS/content.opf"
                 media-type="application/oebps-package+xml"/></rootfiles></container>"#
                .to_vec(),
            true,
        ),
    ];

    let items: String = capitulos
        .iter()
        .enumerate()
        .map(|(i, (archivo, _))| {
            format!(r#"<item id="c{i}" href="{archivo}" media-type="application/xhtml+xml"/>"#)
        })
        .collect();
    let refs: String = (0..capitulos.len())
        .map(|i| format!(r#"<itemref idref="c{i}"/>"#))
        .collect();

    entradas.push((
        "OEBPS/content.opf".to_owned(),
        format!(
            r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
               <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Un libro</dc:title></metadata>
               <manifest>{items}</manifest><spine>{refs}</spine></package>"#
        )
        .into_bytes(),
        true,
    ));

    for (archivo, cuerpo) in capitulos {
        entradas.push((
            format!("OEBPS/{archivo}"),
            format!(
                r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>{cuerpo}</body></html>"#
            )
            .into_bytes(),
            true,
        ));
    }

    let mut salida = Vec::new();
    let mut central = Vec::new();
    let mut cuantas = 0_u16;

    for (nombre_entrada, contenido, comprimir) in &entradas {
        let offset = u32::try_from(salida.len()).expect("cabe");
        let datos = if *comprimir {
            let mut cod =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            cod.write_all(contenido).expect("comprime");
            cod.finish().expect("comprime")
        } else {
            contenido.clone()
        };
        let metodo: u16 = if *comprimir { 8 } else { 0 };
        let comp = u32::try_from(datos.len()).expect("cabe");
        let desc = u32::try_from(contenido.len()).expect("cabe");
        let nb = nombre_entrada.as_bytes();

        salida.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        salida.extend_from_slice(&[20, 0, 0, 0]);
        salida.extend_from_slice(&metodo.to_le_bytes());
        salida.extend_from_slice(&[0; 8]);
        salida.extend_from_slice(&comp.to_le_bytes());
        salida.extend_from_slice(&desc.to_le_bytes());
        salida.extend_from_slice(&u16::try_from(nb.len()).expect("cabe").to_le_bytes());
        salida.extend_from_slice(&[0, 0]);
        salida.extend_from_slice(nb);
        salida.extend_from_slice(&datos);

        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
        central.extend_from_slice(&metodo.to_le_bytes());
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&comp.to_le_bytes());
        central.extend_from_slice(&desc.to_le_bytes());
        central.extend_from_slice(&u16::try_from(nb.len()).expect("cabe").to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(nb);
        cuantas += 1;
    }

    let offset_central = u32::try_from(salida.len()).expect("cabe");
    let tamano_central = u32::try_from(central.len()).expect("cabe");
    salida.extend_from_slice(&central);
    salida.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    salida.extend_from_slice(&[0; 4]);
    salida.extend_from_slice(&cuantas.to_le_bytes());
    salida.extend_from_slice(&cuantas.to_le_bytes());
    salida.extend_from_slice(&tamano_central.to_le_bytes());
    salida.extend_from_slice(&offset_central.to_le_bytes());
    salida.extend_from_slice(&[0, 0]);

    let ruta = tmp.0.join(nombre);
    std::fs::write(&ruta, salida).expect("epub");
    ruta
}

/// La frase que se anota. Es la misma en las dos versiones del documento: lo que
/// cambia es dónde está.
const CITA: &str = "frente al pelotón de fusilamiento";

const ORIGINAL: &str = "<p>Muchos años después, frente al pelotón de fusilamiento, el coronel \
                        Aureliano Buendía había de recordar aquella tarde remota.</p>";

/// La misma frase con un prólogo delante: el desplazamiento guardado sigue
/// **dentro de rango**, y apunta a otro sitio. Es el caso que hace falta para que
/// la cascada no se quede en el primer escalón creyendo que acertó.
const CON_PROLOGO: &str = "<p>Nota del editor a la presente edición, redactada muchos años \
                           más tarde para acompañar esta reimpresión conmemorativa.</p>\
                           <p>Muchos años después, frente al pelotón de fusilamiento, el coronel \
                           Aureliano Buendía había de recordar aquella tarde remota.</p>";

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

/// Abre un EPUB con el cuerpo que se le dé.
fn documento(tmp: &Temporal, nombre: &str, cuerpo: &str) -> EpubDocument {
    let ruta = epub(tmp, nombre, &[("c1.xhtml", cuerpo)]);
    EpubDocument::open(&ruta).expect("epub")
}

/// Desplazamiento de la cita dentro de un recurso, **en caracteres**.
///
/// En caracteres y no en bytes: es el sistema de coordenadas de los locators, y
/// con acentos los dos números no coinciden. Un `find` a secas devuelve bytes y
/// produciría un ancla que no corrobora ni sobre el documento original.
fn offset_de(doc: &EpubDocument, href: &str, cita: &str) -> usize {
    let texto = doc.plain_text(href).expect("texto");
    let byte = texto.find(cita).expect("la cita está");
    texto[..byte].chars().count()
}

/// El locator tal y como lo produciría anotar la cita en el documento original.
fn anclaje(doc: &EpubDocument) -> Locator {
    let offset = offset_de(doc, "OEBPS/c1.xhtml", CITA);

    Locator::new(
        "OEBPS/c1.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec![format!("#char={offset}")],
            progression: Some(0.1),
            total_progression: Some(0.1),
            position: None,
        },
        Some(TextContext::new(Some("después, "), CITA, Some(", el coronel")).expect("cita")),
    )
    .expect("locator")
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn si_el_fragmento_deja_de_resolver_la_anotacion_se_recoloca_por_cita_y_queda_marcada() {
    let tmp = Temporal::nuevo("criterio");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    // 1. Se anota sobre el documento original.
    let original = documento(&tmp, "original.epub", ORIGINAL);
    let ancla = anclaje(&original);
    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: Some("La frase famosa".to_owned()),
            target: ancla.clone(),
        },
    )
    .expect("anotación");

    // Sobre el documento con el que se anotó, resuelve exacta y no se marca.
    let intacto = annotations::anchor(&conn, &original, &edicion).expect("reanclaje");
    assert_eq!(intacto.anchored[0].precision, Precision::Exact);
    assert!(!intacto.anchored[0].relocated);
    assert!(intacto.lost.is_empty());

    // 2. El documento cambia: le añaden un prólogo delante.
    let modificado = documento(&tmp, "modificado.epub", CON_PROLOGO);
    let informe = annotations::anchor(&conn, &modificado, &edicion).expect("reanclaje");

    assert_eq!(informe.anchored.len(), 1, "no se pierde");
    assert!(informe.lost.is_empty());

    let reanclada = &informe.anchored[0];
    assert_eq!(
        reanclada.precision,
        Precision::Quote,
        "el fragmento ya no corrobora, así que la cascada baja al segundo escalón"
    );
    assert!(
        reanclada.relocated,
        "ya no está donde se anotó, y el usuario tiene derecho a saberlo"
    );
    assert_eq!(informe.relocated(), 1);

    // 3. Y está en el sitio correcto del documento nuevo, no en el viejo.
    let esperado = offset_de(&modificado, "OEBPS/c1.xhtml", CITA);
    let anterior = offset_de(&original, "OEBPS/c1.xhtml", CITA);
    assert_ne!(esperado, anterior, "el prólogo tiene que haberla movido");

    match &reanclada.position {
        nb_books_lib::reader::NativePosition::Fragment { href, offset } => {
            assert_eq!(href, "OEBPS/c1.xhtml");
            assert_eq!(
                *offset as usize, esperado,
                "recolocada sobre la cita, no sobre el desplazamiento viejo"
            );
        }
        otra => panic!("un EPUB ancla por fragmento, no por {otra:?}"),
    }
}

// --- La otra mitad del objetivo: nada se pierde -----------------------------

#[test]
fn una_anotacion_que_no_se_puede_colocar_sale_igualmente_en_lugar_de_desaparecer() {
    let tmp = Temporal::nuevo("perdida");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    // Un ancla a un recurso que no existe en este documento y sin progresión:
    // ninguna vía de la cascada puede colocarla.
    let imposible = Locator::new(
        "OEBPS/capitulo-que-no-existe.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["#char=10".to_owned()],
            progression: None,
            total_progression: None,
            position: None,
        },
        Some(TextContext::new(None, "un texto que no está en el libro", None).expect("cita")),
    )
    .expect("locator");

    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: Some("Se quedó sin sitio".to_owned()),
            target: imposible,
        },
    )
    .expect("anotación");

    let doc = documento(&tmp, "otro.epub", ORIGINAL);
    let informe = annotations::anchor(&conn, &doc, &edicion).expect("reanclaje");

    // "No se puede mostrar aquí" y "no existe" son cosas distintas. Confundirlas
    // es perder trabajo del usuario en silencio, y es lo que ADR-004 prohíbe.
    assert!(informe.anchored.is_empty());
    assert_eq!(informe.lost.len(), 1);
    assert_eq!(
        informe.lost[0].body.as_deref(),
        Some("Se quedó sin sitio"),
        "sale entera, para que la interfaz pueda ofrecerla y el usuario decida"
    );
}

#[test]
fn sin_cita_ni_fragmento_valido_se_coloca_por_progresion_y_se_avisa() {
    let tmp = Temporal::nuevo("progresion");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    // Un marcador: es un punto, no un trozo de texto, así que no lleva cita.
    let marcador = Locator::new(
        "OEBPS/c1.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["#char=99999".to_owned()],
            progression: Some(0.5),
            total_progression: Some(0.5),
            position: None,
        },
        None,
    )
    .expect("locator");

    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Bookmark,
            body: None,
            target: marcador,
        },
    )
    .expect("marcador");

    let doc = documento(&tmp, "libro.epub", ORIGINAL);
    let informe = annotations::anchor(&conn, &doc, &edicion).expect("reanclaje");

    let colocada = &informe.anchored[0];
    assert_eq!(colocada.precision, Precision::Approximate);
    assert!(colocada.relocated);
    assert!(
        colocada.warn,
        "aquí se ha adivinado: puede estar en otro párrafo, y eso se avisa con más fuerza"
    );
    assert_eq!(informe.warned(), 1);
}

#[test]
fn una_anotacion_dada_de_baja_no_se_reancla() {
    let tmp = Temporal::nuevo("baja");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let original = documento(&tmp, "libro.epub", ORIGINAL);
    let anotacion = annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: anclaje(&original),
        },
    )
    .expect("anotación");
    annotations::delete(&conn, &anotacion.id).expect("baja");

    // El historial se conserva, pero no se pinta: mostrar las versiones que el
    // usuario ya sustituyó llenaría la página de duplicados.
    let informe = annotations::anchor(&conn, &original, &edicion).expect("reanclaje");
    assert!(informe.anchored.is_empty());
    assert!(informe.lost.is_empty());
}

#[test]
fn las_dos_marcas_son_distintas_y_hacen_falta_las_dos() {
    // Reanclada por cita: bien situada, pero su documento cambió.
    assert!(Precision::Quote.is_relocated());
    assert!(!Precision::Quote.requires_warning());

    // Reanclada por progresión: puede estar en otro párrafo.
    assert!(Precision::Approximate.is_relocated());
    assert!(Precision::Approximate.requires_warning());

    // Sin mover.
    assert!(!Precision::Exact.is_relocated());
    assert!(!Precision::Exact.requires_warning());
}

// --- Panel de anotaciones y marcadores -----------------------------

/// Crea una anotación anclada a una progresión concreta.
///
/// La progresión es lo que ordena el panel, así que es lo que hay que controlar
/// para comprobar el orden.
fn en_progresion(
    conn: &rusqlite::Connection,
    edicion: &str,
    cita: &str,
    progresion: f64,
) -> nb_books_lib::annotations::Annotation {
    let target = Locator::new(
        "OEBPS/c1.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec![],
            progression: Some(progresion),
            total_progression: Some(progresion),
            position: None,
        },
        Some(TextContext::new(None, cita, None).expect("cita")),
    )
    .expect("locator");

    annotations::create(
        conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.to_owned(),
            kind: AnnotationKind::Highlight,
            body: Some(cita.to_owned()),
            target,
        },
    )
    .expect("anotación")
}

#[test]
fn el_panel_va_en_orden_de_lectura_y_no_por_fecha_de_creacion() {
    let tmp = Temporal::nuevo("panel");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    // Se anota al revés de como se lee: primero el final, después el principio.
    // Es lo normal cuando se relee, y una lista por fecha lo dejaría desordenado.
    en_progresion(&conn, &edicion, "aquella tarde remota", 0.9);
    en_progresion(&conn, &edicion, "Muchos años después", 0.1);
    en_progresion(&conn, &edicion, "el coronel", 0.5);

    let doc = documento(&tmp, "libro.epub", ORIGINAL);
    let panel = annotations::outline(&conn, &doc, &edicion).expect("panel");

    let cuerpos: Vec<_> = panel
        .iter()
        .map(|e| e.annotation.body.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        cuerpos,
        vec!["Muchos años después", "el coronel", "aquella tarde remota"],
        "el panel acompaña a quien lee; por fecha habría que buscar en él al pasar de capítulo"
    );
}

#[test]
fn las_que_no_se_pueden_colocar_van_al_final_de_la_misma_lista() {
    let tmp = Temporal::nuevo("panel-perdidas");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    en_progresion(&conn, &edicion, "el coronel", 0.5);
    annotations::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.clone(),
            kind: AnnotationKind::Highlight,
            body: Some("sin sitio".to_owned()),
            target: Locator::new(
                "OEBPS/no-existe.xhtml",
                "application/xhtml+xml",
                Locations {
                    fragments: vec!["#char=1".to_owned()],
                    progression: None,
                    total_progression: None,
                    position: None,
                },
                Some(TextContext::new(None, "texto que no está", None).expect("cita")),
            )
            .expect("locator"),
        },
    )
    .expect("anotación");

    let doc = documento(&tmp, "libro.epub", ORIGINAL);
    let panel = annotations::outline(&conn, &doc, &edicion).expect("panel");

    assert_eq!(panel.len(), 2, "las dos están en la misma lista");
    assert!(!panel[0].lost);
    assert!(panel[1].lost, "la que no tiene sitio va al final, marcada");
    assert!(panel[1].position.is_none());
    // Esconderla en otra pestaña la convertiría en algo que se descubre por
    // casualidad, y la mitad del objetivo era que no se perdiera.
    assert_eq!(panel[1].annotation.body.as_deref(), Some("sin sitio"));
}

#[test]
fn un_marcador_no_lleva_texto_y_se_lista_aparte() {
    let tmp = Temporal::nuevo("marcadores");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let conn = db.connection().expect("conexión");

    let original = documento(&tmp, "libro.epub", ORIGINAL);
    en_progresion(&conn, &edicion, "un resaltado", 0.3);
    let marcador = annotations::bookmark(&conn, &edicion, &anclaje(&original)).expect("marcador");

    assert_eq!(marcador.kind, AnnotationKind::Bookmark);
    assert!(
        marcador.body.is_none(),
        "un marcador no dice nada: solo señala"
    );

    // La interfaz los lista aparte de los resaltados y las notas: son de otra
    // naturaleza, se usan para volver y no para recordar.
    let marcadores =
        annotations::list_of_kind(&conn, &edicion, AnnotationKind::Bookmark).expect("marcadores");
    assert_eq!(marcadores.len(), 1);
    assert_eq!(marcadores[0].id, marcador.id);

    // Y siguen estando en el panel general, que es uno solo.
    assert_eq!(
        annotations::outline(&conn, &original, &edicion)
            .expect("panel")
            .len(),
        2
    );
}
