//! La extensión de metadatos académicos.
//!
//! # Qué se comprueba aquí y por qué importa que sea esta
//!
//! Es la primera extensión de primera parte, y entra por el mismo camino que
//! una de terceros: manifiesto, permiso de red por anfitrión **y origen**, y
//! candidatos que se proponen. Lo que se prueba no es Crossref: es que la API
//! basta para escribir una fuente de verdad sin tocar el núcleo.
//!
//! **Y no bastaba.** El mundo declaraba `identifiers` y el núcleo
//! los mandaba vacíos, así que una fuente que consulta por DOI —que es lo que
//! distingue a una académica de una general— no podía funcionar. Se arregló al
//! escribirla, que es exactamente para lo que existe esta subfase.

mod comun;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use comun::{paquete_propio, OBJETIVO_COMPONENTE};
use nb_books_lib::db::Database;
use nb_books_lib::extensions::{metadata as fuentes, Limites, Subsistema};
use nb_books_lib::net::{self, HttpTransport, Network, Origin, Purpose, Response};

/// Quién es, y el anfitrión que declara en su manifiesto.
const QUIEN: &str = "lat.nbdigitallabs.books.metadata.academico";
const DESTINO: &str = "api.crossref.org";
const MODULO: &str = "metadatos_academicos.wasm";

/// Una respuesta de Crossref recortada a lo que la extensión lee.
///
/// Es la forma real: el título en lista, los autores en objetos con nombre y
/// apellido separados, y la fecha en partes. Simplificarla habría hecho que la
/// prueba pasara con un analizador que no sirve para la respuesta de verdad.
const RESPUESTA: &str = r#"{
  "status": "ok",
  "message": {
    "title": ["Attention Is All You Need"],
    "author": [
      { "given": "Ashish", "family": "Vaswani" },
      { "given": "Noam", "family": "Shazeer" },
      { "name": "El Consorcio de Ejemplo" }
    ],
    "publisher": "Editorial de Ejemplo",
    "language": "en",
    "issued": { "date-parts": [[2017, 6, 12]] }
  }
}"#;

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-academico-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

struct Espia {
    salidas: Arc<Mutex<Vec<String>>>,
    estado: u16,
}

impl HttpTransport for Espia {
    fn get(&self, url: &str) -> nb_books_lib::error::NbResult<Response> {
        self.salidas.lock().expect("candado").push(url.to_owned());
        Ok(Response {
            status: self.estado,
            content_type: Some("application/json".to_owned()),
            location: None,
            body: RESPUESTA.as_bytes().to_vec(),
        })
    }
}

struct Montado {
    db: Database,
    red: Arc<Network>,
    salidas: Arc<Mutex<Vec<String>>>,
    subsistema: Subsistema,
}

fn montar(nombre: &str, estado: u16) -> Option<Montado> {
    let raiz = temporal(nombre);
    let paquete = paquete_propio("metadatos-academicos", MODULO, &raiz)?;
    let subsistema = Subsistema::arrancar(raiz.join("extensiones")).expect("subsistema");
    subsistema.almacen.instalar(&paquete).expect("instala");

    let salidas = Arc::new(Mutex::new(Vec::new()));
    Some(Montado {
        db: Database::open_in_memory().expect("base"),
        red: Arc::new(Network::new(Box::new(Espia {
            salidas: Arc::clone(&salidas),
            estado,
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

fn fuente(m: &Montado) -> fuentes::FuenteDeExtension {
    let mut todas = fuentes::fuentes(&m.subsistema, Limites::default()).expect("fuentes");
    assert_eq!(todas.len(), 1, "la extensión aporta una fuente");
    todas.remove(0)
}

#[test]
fn con_un_doi_devuelve_la_ficha_del_articulo() {
    let Some(m) = montar("doi", 200) else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    conceder(&m.db);

    let candidatos = fuente(&m)
        .buscar(
            &m.db,
            &m.red,
            "Un título cualquiera",
            None,
            &["doi:10.48550/arXiv.1706.03762".to_owned()],
        )
        .expect("consulta");

    assert_eq!(candidatos.len(), 1);
    let ficha = &candidatos[0];
    assert_eq!(ficha.title, "Attention Is All You Need");
    // Apellido y nombre, separados por punto y coma: con comas, un apellido
    // compuesto se leería como dos personas.
    assert_eq!(
        ficha.authors.as_deref(),
        Some("Vaswani, Ashish; Shazeer, Noam; El Consorcio de Ejemplo")
    );
    // Solo el año: componer una fecha con más precisión de la que la fuente da
    // sería inventarla.
    assert_eq!(ficha.published_date.as_deref(), Some("2017"));
    assert_eq!(ficha.publisher.as_deref(), Some("Editorial de Ejemplo"));
    // La procedencia viaja con el candidato: sin ella, el de una extensión se
    // vería igual que el del catálogo de siempre.
    assert_eq!(ficha.source.as_deref(), Some(QUIEN));

    // Y salió por la puerta, con su permiso y por su origen.
    let salidas = m.salidas.lock().expect("candado");
    assert_eq!(salidas.len(), 1);
    assert_eq!(
        salidas[0],
        "https://api.crossref.org/works/10.48550/arXiv.1706.03762"
    );
}

#[test]
fn el_doi_se_reconoce_escrito_de_las_tres_formas_en_que_llega() {
    // Quien cataloga no escribe siempre la misma forma, y rechazar dos de las
    // tres convertiría la extensión en algo que "a veces no funciona".
    let Some(m) = montar("formas", 200) else {
        return;
    };
    conceder(&m.db);
    let fuente = fuente(&m);

    for forma in [
        "10.48550/arXiv.1706.03762",
        "doi:10.48550/arXiv.1706.03762",
        "https://doi.org/10.48550/arXiv.1706.03762",
    ] {
        let candidatos = fuente
            .buscar(&m.db, &m.red, "Título", None, &[forma.to_owned()])
            .expect("consulta");
        assert_eq!(candidatos.len(), 1, "no reconoció {forma}");
    }

    let salidas = m.salidas.lock().expect("candado");
    assert_eq!(salidas.len(), 3);
    assert!(
        salidas
            .iter()
            .all(|u| u == "https://api.crossref.org/works/10.48550/arXiv.1706.03762"),
        "las tres formas piden la misma dirección: {salidas:?}"
    );
}

#[test]
fn sin_identificador_no_gasta_ni_una_peticion() {
    // Una fuente académica no adivina por título: Crossref devolvería
    // coincidencias aproximadas, y proponer una ficha aproximada sobre un
    // documento bien catalogado es peor que no proponer nada. Además, salir a
    // la red gasta el permiso del usuario en algo que no puede responder.
    let Some(m) = montar("sin-doi", 200) else {
        return;
    };
    conceder(&m.db);

    let candidatos = fuente(&m)
        .buscar(
            &m.db,
            &m.red,
            "Cien años de soledad",
            Some("García Márquez"),
            &["isbn:9788437604947".to_owned()],
        )
        .expect("consulta");

    assert!(candidatos.is_empty(), "un ISBN no es un DOI");
    assert!(
        m.salidas.lock().expect("candado").is_empty(),
        "y no se sale a la red a preguntar lo que no se puede preguntar"
    );
}

/// **La primera mitad del criterio de la ficha.**
#[test]
fn denegarle_el_permiso_la_deja_inerte_y_no_rompe_la_biblioteca() {
    let Some(m) = montar("sin-permiso", 200) else {
        return;
    };
    // Sin conceder nada: la extensión está instalada y activa, y su salida no.

    let candidatos = fuente(&m)
        .buscar(
            &m.db,
            &m.red,
            "Título",
            None,
            &["doi:10.1000/ejemplo".to_owned()],
        )
        .expect("consultar no es un error: es una fuente que hoy no contesta");

    assert!(candidatos.is_empty());
    assert!(
        m.salidas.lock().expect("candado").is_empty(),
        "no llega ni al transporte: la puerta la para antes"
    );

    // Y el rechazo queda anotado, que es lo que permite verlo desde la
    // interfaz en lugar de tener que deducirlo.
    let conn = m.db.connection().expect("conexión");
    let registro = net::log(&conn, 10).expect("registro");
    assert_eq!(registro.len(), 1);
    assert_eq!(registro[0].origin, QUIEN);
    assert_eq!(registro[0].outcome, nb_books_lib::net::Outcome::Denied);
}

/// **La segunda mitad del criterio.**
#[test]
fn desinstalarla_no_deja_ni_una_peticion_viva() {
    let Some(m) = montar("desinstalar", 200) else {
        return;
    };
    conceder(&m.db);

    // Existe, consulta y sale.
    fuente(&m)
        .buscar(
            &m.db,
            &m.red,
            "Título",
            None,
            &["doi:10.1000/ejemplo".to_owned()],
        )
        .expect("consulta");
    assert_eq!(m.salidas.lock().expect("candado").len(), 1);

    assert!(
        m.subsistema.almacen.desinstalar(QUIEN).expect("desinstala"),
        "estaba instalada"
    );

    // Ya no hay fuente que consultar: no es que falle, es que no está.
    let despues = fuentes::fuentes(&m.subsistema, Limites::default()).expect("fuentes");
    assert!(despues.is_empty());
    assert_eq!(
        m.salidas.lock().expect("candado").len(),
        1,
        "y no sale ninguna petición más"
    );
}

#[test]
fn un_doi_que_no_esta_indexado_es_no_saber_y_no_un_fallo() {
    // Un 404 de Crossref significa que ese DOI no está. Tratarlo como error
    // haría que la interfaz enseñara un aviso por un documento que
    // sencillamente no es un artículo indexado.
    let Some(m) = montar("cuatrocientos-cuatro", 404) else {
        return;
    };
    conceder(&m.db);

    let candidatos = fuente(&m)
        .buscar(
            &m.db,
            &m.red,
            "Título",
            None,
            &["doi:10.1000/no-existe".to_owned()],
        )
        .expect("consulta");

    assert!(candidatos.is_empty());
}

#[test]
fn el_paquete_pasa_la_puerta_de_revision_del_registro() {
    // Ninguna extensión de primera parte recibe trato especial: entra por el
    // mismo camino que una de terceros. Lo único que se le pasa aparte es que
    // el envío es nuestro, que es lo que la autoriza a usar el espacio de
    // nombres reservado.
    let Some(paquete) = paquete_propio("metadatos-academicos", MODULO, &temporal("revision"))
    else {
        return;
    };
    let informe = nb_books_lib::extensions::review::revisar(
        &paquete,
        nb_books_lib::extensions::review::Opciones {
            de_primera_parte: true,
            // Se firma al publicar, con una clave que no vive en el
            // repositorio. Aquí se revisa el contenido.
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
