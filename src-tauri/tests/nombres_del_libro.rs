//! Los nombres del libro, con dónde aparecen: local y sin IA.
//!
//! El criterio: se construye con FTS5 y sin llamar a ningún modelo; con la IA
//! apagada funciona igual; y el coste en disco entra en el presupuesto o no se
//! guarda. Aquí: que los nombres salen del texto, que los sitios salen del índice
//! de búsqueda cuando está y del recorrido cuando no, que no se escribe ni una fila,
//! y que en todo el camino no hay un modelo: el estado de este runtime no tiene
//! ninguno instalado. La medida sobre el corpus real está en `corpus_real.rs`.

mod comun;

use std::sync::Arc;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::progress::ProgressTracker;
use nb_books_lib::reader::{ProviderFactory, ProviderRegistry, SessionStore};

const CONTENEDOR: &str = r#"<?xml version="1.0"?><container version="1.0"
  xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
  <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles></container>"#;

const PAQUETE: &str = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Una familia</dc:title></metadata>
  <manifest>
    <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"/>
    <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#;

/// Texto propio, con nombres que se repiten, un nombre compuesto con conector, una
/// palabra que solo va a principio de frase y otra que se nombra una vez.
const CAPITULO_1: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>
  <p>Hacía calor cuando José Arcadio Buendía llegó al pueblo con Úrsula de la Vega.</p>
  <p>Entonces el pueblo se llamó Macondo, y en Macondo no llovía nunca.</p>
  <p>Mañana lloverá, dijo José Arcadio Buendía. Mañana, repitió.</p>
  <p>Úrsula de la Vega no le creyó. Un gitano llamado Melquíades pasó por allí.</p>
</body></html>"#;

const CAPITULO_2: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>
  <p>Años después, José Arcadio Buendia volvió a Macondo sin su tilde.</p>
  <p>Y Úrsula de la Vega le esperaba. Mañana, pensó, mañana.</p>
</body></html>"#;

struct Libro {
    dir: std::path::PathBuf,
    db: Database,
    archivo: String,
    edicion: String,
    registro: ProviderRegistry,
}

impl Drop for Libro {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn libro(nombre: &str) -> Libro {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-nombres-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let origen = dir.join("origen");
    std::fs::create_dir_all(&origen).expect("origen");
    let ruta = origen.join("familia.epub");
    std::fs::write(
        &ruta,
        comun::zip_sin_comprimir(&[
            ("mimetype", b"application/epub+zip"),
            ("META-INF/container.xml", CONTENEDOR.as_bytes()),
            ("OEBPS/content.opf", PAQUETE.as_bytes()),
            ("OEBPS/c1.xhtml", CAPITULO_1.as_bytes()),
            ("OEBPS/c2.xhtml", CAPITULO_2.as_bytes()),
        ]),
    )
    .expect("libro");
    let db = Database::open(&dir.join("nb.sqlite")).expect("base");
    let (archivo, edicion) = {
        let mut conn = db.connection().expect("conexión");
        match import_document(
            &mut conn,
            &ImportContext {
                scope: &origen,
                library: &dir.join("biblioteca"),
                bus: &EventBus::new(),
            },
            &ImportRequest {
                source: ruta,
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importación")
        {
            ImportOutcome::Imported(d) => (d.file.id.clone(), d.edition.id.clone()),
            ImportOutcome::AlreadyInLibrary(f) | ImportOutcome::Duplicate(f) => {
                (f.id.clone(), f.edition_id.clone())
            }
        }
    };
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(EpubFactory::solo_texto()));
    Libro {
        dir,
        db,
        archivo,
        edicion,
        registro,
    }
}

/// Cuántas veces ha escrito **otra** conexión en la base desde que esta se abrió.
///
/// `data_version` cambia cuando otra conexión confirma una escritura, que es
/// exactamente lo que se quiere saber: si construir el índice escribió algo.
fn escrituras_ajenas(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("PRAGMA data_version", [], |f| f.get(0))
        .expect("versión")
}

#[test]
fn los_nombres_salen_del_texto_y_los_sitios_del_indice_de_busqueda() {
    let libro = libro("indexado");
    {
        // El libro está indexado, como lo deja el indexador de fondo.
        let mut conn = libro.db.connection().expect("conexión");
        let documento = EpubFactory::solo_texto()
            .open(&libro.dir.join("origen/familia.epub"))
            .expect("abierto");
        nb_books_lib::search::index_edition(&mut conn, &libro.edicion, documento.as_ref())
            .expect("indexado");
    }
    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(libro.db.clone())));
    let sesion = almacen
        .open(&libro.db, &libro.registro, &libro.archivo)
        .expect("abierta");

    let testigo = libro.db.connection().expect("conexión");
    let antes = escrituras_ajenas(&testigo);
    let indice = sesion.entities().expect("nombres");

    let nombres: Vec<(&str, u32)> = indice
        .entities
        .iter()
        .map(|e| (e.name.as_str(), e.mentions))
        .collect();
    // El compuesto con su conector es uno, y la forma sin tilde cuenta como él.
    assert!(
        nombres.contains(&("José Arcadio Buendía", 3)),
        "{nombres:?}"
    );
    assert!(nombres.contains(&("Úrsula de la Vega", 3)), "{nombres:?}");
    assert!(nombres.contains(&("Macondo", 3)), "{nombres:?}");
    // "Mañana" solo va a principio de frase o en minúscula: no es un nombre. Y
    // Melquíades aparece una vez: por debajo del mínimo.
    assert!(!nombres.iter().any(|(n, _)| *n == "Mañana"), "{nombres:?}");
    assert!(
        !nombres.iter().any(|(n, _)| *n == "Melquíades"),
        "{nombres:?}"
    );

    // Los sitios, en el orden del libro y en los dos capítulos: el índice encuentra
    // también "Buendia" sin tilde, porque tokeniza sin tildes.
    let sitios = sesion
        .entity_places("José Arcadio Buendía")
        .expect("sitios");
    assert!(
        sitios.from_index,
        "con el libro indexado, los sitios salen del índice"
    );
    let capitulos: Vec<&str> = sitios.places.iter().map(|l| l.href.as_str()).collect();
    assert!(capitulos.contains(&"OEBPS/c1.xhtml") && capitulos.contains(&"OEBPS/c2.xhtml"));
    // Y encuentra lo que encontraría la búsqueda, sin distinguir tildes ni
    // mayúsculas: "MACONDO" en un título en versales es el mismo nombre.
    let versales = sesion.entity_places("MACONDO").expect("sitios");
    assert!(versales.from_index && !versales.places.is_empty());

    // Y no se guardó nada: calcularlo no escribe ni una página de la base.
    assert_eq!(
        escrituras_ajenas(&testigo),
        antes,
        "el índice de nombres no escribe nada: no cuesta disco"
    );
}

#[test]
fn sin_el_libro_indexado_los_sitios_salen_del_recorrido_y_se_dice() {
    let libro = libro("sin-indice");
    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(libro.db.clone())));
    let sesion = almacen
        .open(&libro.db, &libro.registro, &libro.archivo)
        .expect("abierta");
    let indice = sesion.entities().expect("nombres");
    assert!(indice.entities.iter().any(|e| e.name == "Macondo"));
    let sitios = sesion.entity_places("Macondo").expect("sitios");
    assert!(
        !sitios.from_index,
        "sin índice, los sitios son los del recorrido, y se dice"
    );
    assert!(!sitios.places.is_empty());
}
