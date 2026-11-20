//! Exportación de citas a formatos bibliográficos.
//!
//! # El criterio, y qué se puede comprobar de él aquí
//!
//! *"Un gestor bibliográfico de terceros importa el archivo sin intervención
//! manual."* Lo que se puede comprobar en esta máquina es que el archivo tenga
//! la forma que ese gestor espera: las entradas cerradas, los autores separados
//! como BibTeX separa personas, cada campo de RIS en su línea. Lo que **no** se
//! puede comprobar aquí es abrir Zotero y darle a importar, y eso queda
//! declarado en la sección 3 del plan.

mod comun;

use std::path::PathBuf;

use comun::{paquete_propio, OBJETIVO_COMPONENTE};
use nb_books_lib::db::Database;
use nb_books_lib::extensions::citation::{self, Referencia};
use nb_books_lib::extensions::{Limites, Subsistema};

const MODULO: &str = "citas.wasm";
const QUIEN: &str = "lat.nbdigitallabs.books.citas.bibliografia";

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-citas-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

struct Montado {
    subsistema: Subsistema,
}

fn montar(nombre: &str) -> Option<Montado> {
    let raiz = temporal(nombre);
    let paquete = paquete_propio("citas", MODULO, &raiz)?;
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");
    subsistema.almacen.instalar(&paquete).expect("instala");
    Some(Montado { subsistema })
}

fn referencia() -> Referencia {
    Referencia {
        key: "vaswani2017-1".to_owned(),
        kind: "article".to_owned(),
        title: "Attention Is All You Need".to_owned(),
        authors: vec!["Vaswani, Ashish".to_owned(), "Shazeer, Noam".to_owned()],
        year: Some("2017".to_owned()),
        publisher: Some("Editorial de Ejemplo".to_owned()),
        identifiers: vec!["doi:10.48550/arXiv.1706.03762".to_owned()],
        quote: Some("La atención es todo lo que necesitas".to_owned()),
        note: Some("Comprobar la sección 3.2".to_owned()),
    }
}

fn componer(m: &Montado, formato: &str, refs: &[Referencia]) -> String {
    let formatos = citation::formatos(&m.subsistema).expect("formatos");
    let elegido = formatos
        .into_iter()
        .find(|f| f.id == formato)
        .expect("el formato existe");
    citation::componer(
        &m.subsistema.motor,
        &m.subsistema.almacen,
        &elegido,
        refs,
        Limites::default(),
    )
    .expect("compone")
}

#[test]
fn la_extension_aporta_los_dos_formatos_que_declara() {
    let Some(m) = montar("formatos") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let formatos = citation::formatos(&m.subsistema).expect("formatos");

    let ids: Vec<&str> = formatos.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, vec!["bibtex", "ris"]);
    assert!(formatos.iter().all(|f| f.extension == QUIEN));
    // La extensión de archivo la pone la extensión porque es parte del formato.
    assert_eq!(formatos[0].file_extension, "bib");
    assert_eq!(formatos[1].file_extension, "ris");
}

#[test]
fn una_entrada_de_bibtex_lleva_lo_que_bibtex_espera() {
    let Some(m) = montar("bibtex") else {
        return;
    };
    let texto = componer(&m, "bibtex", &[referencia()]);

    assert!(texto.starts_with("@article{vaswani2017-1,"), "{texto}");
    // Los autores unidos por " and ": con comas, "Vaswani, Ashish" se leería
    // como dos personas.
    assert!(
        texto.contains("author = {Vaswani, Ashish and Shazeer, Noam}"),
        "{texto}"
    );
    assert!(texto.contains("year = {2017}"));
    assert!(texto.contains("doi = {10.48550/arXiv.1706.03762}"));
    // La cita literal y la nota propia van en campos distintos: una es del
    // libro y otra de quien lo leyó.
    assert!(texto.contains("annotation = {La atención es todo lo que necesitas}"));
    assert!(texto.contains("note = {Comprobar la sección 3.2}"));
    assert!(texto.trim_end().ends_with('}'), "la entrada se cierra");
}

#[test]
fn una_entrada_de_ris_lleva_cada_campo_en_su_linea_y_se_cierra() {
    let Some(m) = montar("ris") else {
        return;
    };
    let texto = componer(&m, "ris", &[referencia()]);
    let lineas: Vec<&str> = texto.lines().collect();

    assert_eq!(lineas[0], "TY  - JOUR");
    // Un autor por línea, que es como RIS los lleva.
    assert_eq!(lineas.iter().filter(|l| l.starts_with("AU  - ")).count(), 2);
    assert!(lineas.contains(&"PY  - 2017"));
    assert!(lineas.contains(&"DO  - 10.48550/arXiv.1706.03762"));
    assert!(texto.trim_end().ends_with("ER  -"), "{texto}");
}

#[test]
fn una_nota_de_dos_parrafos_no_rompe_el_archivo_de_ris() {
    // RIS es un formato de línea: un salto dentro de un valor haría que quien
    // lo importe leyera media nota como una etiqueta desconocida.
    let Some(m) = montar("ris-parrafos") else {
        return;
    };
    let mut refs = referencia();
    refs.note = Some("Primera línea.\n\nSegunda línea.".to_owned());

    let texto = componer(&m, "ris", &[refs]);

    assert!(
        texto.contains("N1  - Primera línea. Segunda línea."),
        "{texto}"
    );
    assert_eq!(
        texto.lines().filter(|l| !l.is_empty()).count(),
        texto.lines().filter(|l| l.contains("  - ")).count(),
        "toda línea del archivo es un campo: {texto}"
    );
}

#[test]
fn lo_que_bibtex_interpreta_se_escapa() {
    // Una llave sin escapar rompe el archivo entero para quien lo compile, y
    // una barra invertida se come el carácter siguiente.
    let Some(m) = montar("escape") else {
        return;
    };
    let mut refs = referencia();
    refs.title = "Sobre {llaves} y \\barras".to_owned();

    let texto = componer(&m, "bibtex", &[refs]);

    assert!(
        texto.contains(r"title = {Sobre \{llaves\} y \textbackslash{}barras}"),
        "{texto}"
    );
}

#[test]
fn una_clave_con_espacios_no_sale_del_archivo() {
    // BibTeX no admite espacios en la clave, y una clave rota hace que el
    // archivo entero no compile.
    let Some(m) = montar("clave") else {
        return;
    };
    let mut refs = referencia();
    refs.key = "garcía márquez 1967".to_owned();

    let texto = componer(&m, "bibtex", &[refs]);
    let clave = texto
        .lines()
        .next()
        .expect("primera línea")
        .trim_start_matches("@article{")
        .trim_end_matches(',');

    assert!(!clave.contains(' '), "la clave lleva espacios: {clave}");
}

#[test]
fn un_formato_que_no_aporta_se_dice_y_no_se_inventa() {
    let Some(m) = montar("desconocido") else {
        return;
    };
    let formatos = citation::formatos(&m.subsistema).expect("formatos");
    let mut inventado = formatos[0].clone();
    inventado.id = "csl-json".to_owned();

    let resultado = citation::componer(
        &m.subsistema.motor,
        &m.subsistema.almacen,
        &inventado,
        &[referencia()],
        Limites::default(),
    );

    assert!(matches!(
        resultado,
        Err(nb_books_lib::error::NbError::FormatUnsupported)
    ));
}

#[test]
fn las_referencias_salen_de_las_anotaciones_y_del_catalogo() {
    // Una referencia por anotación: quien cita un libro cita un pasaje, y una
    // entrada por documento obligaría a volver a buscar dónde estaba.
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let obra = nb_books_lib::db::catalog::Work::create(
        &conn,
        &nb_books_lib::db::catalog::NewWork {
            canonical_title: "Cien Años de Soledad".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = nb_books_lib::db::catalog::Edition::create(
        &conn,
        &nb_books_lib::db::catalog::NewEdition {
            work_id: obra.id,
            title: "Cien Años de Soledad".to_owned(),
            subtitle: None,
            publisher: Some("Sudamericana".to_owned()),
            published_date: Some("1967-05-30".to_owned()),
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición");

    let referencias = citation::referencias(&conn, &edicion.id).expect("referencias");

    // Sin anotaciones, una referencia del documento entero.
    assert_eq!(referencias.len(), 1);
    assert_eq!(referencias[0].title, "Cien Años de Soledad");
    assert_eq!(referencias[0].publisher.as_deref(), Some("Sudamericana"));
    // El año, no la fecha entera: una cita lleva año, y recortar en el núcleo
    // evita que cada formato tenga que saber recortar.
    assert_eq!(referencias[0].year.as_deref(), Some("1967"));
    assert_eq!(referencias[0].kind, "book");
}

#[test]
fn el_paquete_pasa_la_puerta_de_revision_del_registro() {
    let Some(paquete) = paquete_propio("citas", MODULO, &temporal("revision")) else {
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

    assert!(
        informe.pasa(),
        "la extensión propia no pasa su propia revisión: {:?}",
        informe.hallazgos
    );
}
