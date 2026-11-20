//! Corrección de metadatos por lotes.
//!
//! # El criterio: mil documentos, reversible y sin bloquear
//!
//! **Reversible** se cumple porque no se escribe nada: la extensión propone y
//! el anfitrión registra propuestas. La forma barata de cumplirlo habría sido
//! guardar los valores anteriores para restaurarlos; la que no se rompe es no
//! cambiarlos.
//!
//! **Mil de una vez** se prueba con mil de verdad, no con diez: lo que puede
//! romperse a esa escala es el plazo del invitado, y por eso el lote se trocea.

mod comun;

use std::path::PathBuf;

use comun::{paquete_propio, OBJETIVO_COMPONENTE};
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::extensions::bulk::{self, Documento};
use nb_books_lib::extensions::{Limites, Subsistema};
use nb_books_lib::metadata::{self, EntityType, Field, Source};

const MODULO: &str = "normalizador.wasm";
const QUIEN: &str = "lat.nbdigitallabs.books.metadata.normalizador";

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-lotes-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

struct Montado {
    db: Database,
    subsistema: Subsistema,
}

fn montar(nombre: &str) -> Option<Montado> {
    let raiz = temporal(nombre);
    let paquete = paquete_propio("normalizador", MODULO, &raiz)?;
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");
    subsistema.almacen.instalar(&paquete).expect("instala");
    Some(Montado {
        db: Database::open_in_memory().expect("base"),
        subsistema,
    })
}

/// Crea una edición con el título que se le pase y devuelve su identificador.
fn edicion(conn: &rusqlite::Connection, titulo: &str, editorial: Option<&str>) -> String {
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
            publisher: editorial.map(ToOwned::to_owned),
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

fn revisar(m: &Montado, ids: &[String]) -> bulk::BulkReviewSummary {
    let conn = m.db.connection().expect("conexión");
    let documentos: Vec<Documento> = ids
        .iter()
        .filter_map(|id| bulk::documento_de(&conn, id).expect("documento"))
        .collect();

    bulk::revisar(
        &conn,
        &m.subsistema.motor,
        &m.subsistema.almacen,
        QUIEN,
        &documentos,
        Limites::default(),
    )
    .expect("revisa")
}

#[test]
fn un_titulo_en_mayusculas_enteras_se_propone_en_mayuscula_de_oracion() {
    let Some(m) = montar("mayusculas") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let id = edicion(&conn, "CIEN AÑOS DE SOLEDAD", None);
    drop(conn);

    let resumen = revisar(&m, std::slice::from_ref(&id));
    assert_eq!(resumen.reviewed, 1);
    assert_eq!(resumen.recorded, 1);

    let conn = m.db.connection().expect("conexión");
    let propuestas = metadata::pending(&conn, EntityType::Edition, &id).expect("propuestas");
    assert_eq!(propuestas.len(), 1);
    assert_eq!(
        propuestas[0].proposed_value.as_deref(),
        Some("Cien años de soledad"),
        "en español se propone mayúscula de oración, no de título inglesa"
    );
}

/// **El criterio: reversible.**
#[test]
fn revisar_no_cambia_ni_un_titulo() {
    let Some(m) = montar("no-escribe") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let id = edicion(&conn, "CIEN AÑOS DE SOLEDAD", None);
    drop(conn);

    revisar(&m, std::slice::from_ref(&id));

    let conn = m.db.connection().expect("conexión");
    let titulo: String = conn
        .query_row("SELECT title FROM editions WHERE id = ?1", [&id], |f| {
            f.get(0)
        })
        .expect("título");
    assert_eq!(
        titulo, "CIEN AÑOS DE SOLEDAD",
        "un lote que escribe mil documentos en silencio es lo que no se puede deshacer"
    );
}

#[test]
fn una_edicion_manual_no_se_toca_ni_se_propone_encima() {
    let Some(m) = montar("manual") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let id = edicion(&conn, "CIEN AÑOS DE SOLEDAD", None);
    // Alguien lo escribió a mano, en mayúsculas y a propósito.
    metadata::propose(
        &conn,
        &id,
        Field::Title,
        Source::Manual,
        Some("CIEN AÑOS DE SOLEDAD"),
    )
    .expect("manual");
    drop(conn);

    revisar(&m, std::slice::from_ref(&id));

    let conn = m.db.connection().expect("conexión");
    let titulo: String = conn
        .query_row("SELECT title FROM editions WHERE id = ?1", [&id], |f| {
            f.get(0)
        })
        .expect("título");
    assert_eq!(titulo, "CIEN AÑOS DE SOLEDAD");
    // Y queda como propuesta, que es lo que permite decidir después.
    assert_eq!(
        metadata::pending(&conn, EntityType::Edition, &id)
            .expect("propuestas")
            .len(),
        1
    );
}

#[test]
fn lo_que_no_hay_que_corregir_no_se_propone() {
    // Devolver lista vacía es lo normal: la mayoría de los documentos no tienen
    // nada que corregir, y proponer por proponer enseña a aceptar sin leer.
    let Some(m) = montar("limpio") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let id = edicion(&conn, "Cien Años de Soledad", Some("Sudamericana"));
    drop(conn);

    let resumen = revisar(&m, std::slice::from_ref(&id));
    assert_eq!(resumen.suggested, 0);
    assert_eq!(resumen.recorded, 0);
}

#[test]
fn una_sigla_corta_no_se_toca() {
    // Convertir "NASA" en "Nasa" sería estropearlo. Con dos palabras largas en
    // mayúsculas ya no es una sigla.
    let Some(m) = montar("sigla") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let id = edicion(&conn, "NASA", None);
    drop(conn);

    assert_eq!(revisar(&m, std::slice::from_ref(&id)).suggested, 0);
}

#[test]
fn los_espacios_de_mas_se_arreglan_en_el_titulo_y_en_la_editorial() {
    let Some(m) = montar("espacios") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let id = edicion(&conn, "  El   Aleph ", Some(" Emecé  Editores "));
    drop(conn);

    let resumen = revisar(&m, std::slice::from_ref(&id));
    assert_eq!(resumen.recorded, 2, "el título y la editorial");

    let conn = m.db.connection().expect("conexión");
    let propuestas = metadata::pending(&conn, EntityType::Edition, &id).expect("propuestas");
    let valores: Vec<Option<&str>> = propuestas
        .iter()
        .map(|p| p.proposed_value.as_deref())
        .collect();
    assert!(valores.contains(&Some("El Aleph")), "{valores:?}");
    assert!(valores.contains(&Some("Emecé Editores")), "{valores:?}");
}

/// **El criterio: mil documentos de una vez.**
#[test]
fn mil_documentos_se_revisan_en_lotes_y_ninguno_se_queda_sin_mirar() {
    let Some(m) = montar("mil") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let mut ids = Vec::with_capacity(1000);
    for numero in 0..1000 {
        // La mitad con defecto y la mitad limpios: si todos tuvieran defecto, un
        // normalizador que propusiera siempre pasaría igual.
        let titulo = if numero % 2 == 0 {
            format!("DOCUMENTO DE PRUEBA NUMERO {numero}")
        } else {
            format!("Documento de Prueba Número {numero}")
        };
        ids.push(edicion(&conn, &titulo, None));
    }
    drop(conn);

    let empezo = std::time::Instant::now();
    let resumen = revisar(&m, &ids);
    let tardo = empezo.elapsed();

    assert_eq!(resumen.reviewed, 1000);
    assert_eq!(
        resumen.suggested, 500,
        "solo la mitad tenía algo que corregir"
    );
    assert_eq!(resumen.recorded, 500);
    assert_eq!(
        resumen.failed_batches, 0,
        "ningún lote se pasó de su plazo: por eso se trocea"
    );
    // No es un presupuesto de rendimiento, es una red: si esto tarda un minuto,
    // algo se está haciendo por documento que debería hacerse por lote.
    assert!(
        tardo.as_secs() < 30,
        "mil documentos tardaron {tardo:?}, que es demasiado para un lote"
    );
}

#[test]
fn una_propuesta_sobre_un_documento_que_no_iba_en_el_lote_se_descarta() {
    // La extensión solo puede opinar sobre lo que se le dio. Aceptar un
    // identificador que no mandamos sería dejarla escribir donde quisiera.
    let Some(m) = montar("intruso") else {
        return;
    };
    let conn = m.db.connection().expect("conexión");
    let dentro = edicion(&conn, "TITULO EN MAYUSCULAS ENTERAS", None);
    let fuera = edicion(&conn, "OTRO TITULO EN MAYUSCULAS", None);
    drop(conn);

    revisar(&m, std::slice::from_ref(&dentro));

    let conn = m.db.connection().expect("conexión");
    assert!(
        metadata::pending(&conn, EntityType::Edition, &fuera)
            .expect("propuestas")
            .is_empty(),
        "el que no iba en el lote no recibe propuestas"
    );
}

#[test]
fn el_paquete_pasa_la_puerta_de_revision_del_registro() {
    let Some(paquete) = paquete_propio("normalizador", MODULO, &temporal("revision")) else {
        return;
    };
    let informe = nb_books_lib::extensions::review::revisar(
        &paquete,
        nb_books_lib::extensions::review::Opciones {
            de_primera_parte: true,
            exige_firma: false,
        },
    )
    .expect("se puede revisar");

    assert!(informe.pasa(), "{:?}", informe.hallazgos);
}
