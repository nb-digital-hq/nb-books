//! Una fuente de metadatos aportada por una extensión.
//!
//! El criterio: *"los datos obtenidos se registran como sugerencia, nunca
//! sobrescriben edición manual"*. Y la propiedad que lo sostiene, que la ficha
//! no dice pero es la mitad del trabajo: la fuente sale por la **misma puerta**
//! de red que todo lo demás, con **su** permiso, y sin él no llega al
//! transporte.

mod comun;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use comun::{componente_construido, OBJETIVO_COMPONENTE};
use nb_books_lib::db::Database;
use nb_books_lib::extensions::{metadata as fuentes, Limites, Subsistema};
use nb_books_lib::metadata::{self, Field, Source};
use nb_books_lib::net::{self, HttpTransport, Network, Origin, Purpose, Response};

/// Quién es la extensión, y el anfitrión que declara.
const QUIEN: &str = "lat.nbdigitallabs.books.banco.fuente-de-metadatos";
const DESTINO: &str = "datos.ejemplo.org";

/// Lo que responde el servicio simulado, en el formato que la extensión lee.
const FICHA: &str = "titulo=Cien años de soledad\neditorial=Sudamericana\nanio=1967\n";

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-fuente-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

struct Espia {
    salidas: Arc<Mutex<Vec<String>>>,
}

impl HttpTransport for Espia {
    fn get(&self, url: &str) -> nb_books_lib::error::NbResult<Response> {
        self.salidas.lock().expect("candado").push(url.to_owned());
        Ok(Response {
            status: 200,
            content_type: Some("text/plain".to_owned()),
            location: None,
            body: FICHA.as_bytes().to_vec(),
        })
    }
}

/// Copia la extensión de ejemplo del banco a un paquete instalable.
fn paquete(en: &Path) -> Option<PathBuf> {
    let construido = componente_construido()?;
    let origen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raíz")
        .join("extensiones")
        .join("banco")
        .join("fuente-de-metadatos");

    let dir = en.join("paquete-fuente");
    std::fs::create_dir_all(&dir).expect("paquete");
    std::fs::copy(
        origen.join("nb-extension.json"),
        dir.join("nb-extension.json"),
    )
    .expect("manifiesto");
    std::fs::copy(
        construido.join("fuente_de_metadatos.wasm"),
        dir.join("fuente_de_metadatos.wasm"),
    )
    .expect("módulo");
    Some(dir)
}

struct Montado {
    db: Database,
    red: Arc<Network>,
    salidas: Arc<Mutex<Vec<String>>>,
    subsistema: Subsistema,
}

fn montar(nombre: &str) -> Option<Montado> {
    let raiz = temporal(nombre);
    let paquete = paquete(&raiz)?;
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");
    subsistema.almacen.instalar(&paquete).expect("instala");

    let salidas = Arc::new(Mutex::new(Vec::new()));
    Some(Montado {
        db: Database::open_in_memory().expect("base"),
        red: Arc::new(Network::new(Box::new(Espia {
            salidas: Arc::clone(&salidas),
        }))),
        salidas,
        subsistema,
    })
}

fn conceder(db: &Database) {
    let conn = db.connection().expect("conexión");
    net::grant_from(
        &conn,
        &Origin::Extension(QUIEN.to_owned()),
        DESTINO,
        Purpose::Extension,
    )
    .expect("concede");
}

#[test]
fn una_fuente_de_extension_propone_y_su_candidato_dice_de_donde_sale() {
    let Some(m) = montar("propone") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    conceder(&m.db);

    let fuentes = fuentes::fuentes(&m.subsistema, Limites::default()).expect("fuentes");
    assert_eq!(fuentes.len(), 1);

    let candidatos = fuentes[0]
        .buscar(&m.db, &m.red, "Cien años de soledad", None, &[])
        .expect("consulta");

    assert_eq!(candidatos.len(), 1);
    assert_eq!(candidatos[0].title, "Cien años de soledad");
    assert_eq!(candidatos[0].publisher.as_deref(), Some("Sudamericana"));
    // La procedencia viaja con el candidato: sin ella, el de un tercero se
    // vería igual que el del catálogo de siempre.
    assert_eq!(candidatos[0].source.as_deref(), Some(QUIEN));

    // Y salió por la puerta, con su permiso.
    let conn = m.db.connection().expect("conexión");
    let registro = net::log(&conn, 10).expect("registro");
    assert_eq!(registro[0].origin, QUIEN);
    assert_eq!(registro[0].purpose, "extension");
}

/// **El criterio de aceptación.**
#[test]
fn lo_que_propone_una_extension_no_sobrescribe_una_edicion_manual() {
    let Some(m) = montar("manual") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    conceder(&m.db);
    let conn = m.db.connection().expect("conexión");

    // Una edición de verdad: la procedencia se guarda contra el catálogo, así
    // que sin ella no habría nada que sobrescribir ni que proteger.
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
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición")
    .id;

    // Alguien escribió el título a mano.
    metadata::propose(
        &conn,
        &edicion,
        Field::Title,
        Source::Manual,
        Some("El título que puso el usuario"),
    )
    .expect("edición manual");

    let fuentes = fuentes::fuentes(&m.subsistema, Limites::default()).expect("fuentes");
    let candidatos = fuentes[0]
        .buscar(&m.db, &m.red, "Cien años de soledad", None, &[])
        .expect("consulta");

    // Se aplica por el mismo camino que el proveedor del núcleo. Que la fuente
    // sea de un tercero no le da más autoridad: le da la misma.
    nb_books_lib::metadata_online::aplicar(&conn, &edicion, &candidatos[0]).expect("aplica");

    let titulo: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata_provenance
              WHERE entity_id = ?1 AND field = ?2",
            rusqlite::params![&edicion, Field::Title.as_str()],
            |row| row.get(0),
        )
        .expect("procedencia");
    assert_eq!(titulo.as_deref(), Some("El título que puso el usuario"));
}

#[test]
fn sin_permiso_la_fuente_no_llega_al_transporte_y_no_rompe_nada() {
    let Some(m) = montar("sin-permiso") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };

    let fuentes = fuentes::fuentes(&m.subsistema, Limites::default()).expect("fuentes");
    let candidatos = fuentes[0]
        .buscar(&m.db, &m.red, "Cien años de soledad", None, &[])
        .expect("no poder consultar no es una avería");

    // Sin permiso no hay candidatos, y eso no es un fallo: es una fuente que no
    // puede contestar. La aplicación sigue y el catálogo del núcleo también.
    assert!(candidatos.is_empty());
    assert!(
        m.salidas.lock().expect("candado").is_empty(),
        "sin permiso no llega ni al transporte"
    );
}

#[test]
fn una_extension_desactivada_no_aporta_su_fuente() {
    let Some(m) = montar("apagada") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    m.subsistema.almacen.habilitar(QUIEN, false).expect("apaga");

    assert!(fuentes::fuentes(&m.subsistema, Limites::default())
        .expect("fuentes")
        .is_empty());
}
