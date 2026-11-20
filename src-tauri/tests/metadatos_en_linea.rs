//! Criterio de aceptación de los metadatos en línea.
//!
//! "Lo obtenido **nunca pisa una edición manual**; rechazar una propuesta no
//! vuelve a preguntar; sin red, la extracción local sigue siendo la
//! vía normal."
//!
//! Ninguna prueba de aquí sale a la red: el transporte es un espía que devuelve
//! la respuesta que se le dé. Lo que se comprueba es la mitad que importa —qué
//! se escribe y qué no— y la dirección que se construye, que es donde un error
//! se convertiría en mandar el título del usuario a otro sitio.

use std::sync::{Arc, Mutex};

use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::metadata::{self, Field, Source, SuggestionStatus};
use nb_books_lib::metadata_online::{self, Candidate, HOST_CATALOGO, HOST_PORTADAS};
use nb_books_lib::net::{self, HttpTransport, Network, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

struct Espia {
    salidas: Arc<Mutex<Vec<String>>>,
    respuesta: Response,
}

impl Espia {
    fn nuevo(cuerpo: &[u8], status: u16) -> (Self, Arc<Mutex<Vec<String>>>) {
        let salidas = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                salidas: Arc::clone(&salidas),
                respuesta: Response {
                    status,
                    content_type: None,
                    location: None,
                    body: cuerpo.to_vec(),
                },
            },
            salidas,
        )
    }
}

impl HttpTransport for Espia {
    fn get(&self, url: &str) -> NbResult<Response> {
        self.salidas
            .lock()
            .expect("registro del espía")
            .push(url.to_owned());
        Ok(self.respuesta.clone())
    }
}

const FICHA: &str = r#"{"numFound":1,"docs":[
    {"title":"Cien años de soledad",
     "author_name":["Gabriel García Márquez"],
     "first_publish_year":1967,
     "publisher":["Sudamericana"],
     "language":["spa"],
     "cover_i":8231856}
]}"#;

fn base() -> Database {
    Database::open_in_memory().expect("base")
}

/// Una edición con el título que trajo el archivo.
fn edicion(conn: &rusqlite::Connection, titulo: &str) -> String {
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
            ..NewEdition::default()
        },
    )
    .expect("edición")
    .id
}

fn candidato() -> Candidate {
    Candidate {
        title: "Cien años de soledad".to_owned(),
        authors: Some("Gabriel García Márquez".to_owned()),
        published_date: Some("1967".to_owned()),
        publisher: Some("Sudamericana".to_owned()),
        language: Some("spa".to_owned()),
        cover_url: Some(format!("https://{HOST_PORTADAS}/b/id/8231856-L.jpg")),
        // Del catálogo del núcleo, no de una extensión.
        source: None,
    }
}

fn titulo_de(conn: &rusqlite::Connection, edition_id: &str) -> String {
    conn.query_row(
        "SELECT title FROM editions WHERE id = ?1",
        [edition_id],
        |row| row.get(0),
    )
    .expect("título")
}

/// **El criterio, primera mitad.** Lo obtenido no pisa una edición manual: queda
/// como propuesta y el valor del usuario sigue en su sitio.
#[test]
fn lo_obtenido_nunca_pisa_una_edicion_manual() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let id = edicion(&conn, "cien anios de soledad");

    // El usuario corrige el título a mano.
    metadata::set_manual(&conn, &id, Field::Title, Some("Cien Años de Soledad")).expect("manual");

    let informe = metadata_online::aplicar(&conn, &id, &candidato()).expect("aplicar");

    assert_eq!(
        titulo_de(&conn, &id),
        "Cien Años de Soledad",
        "el catálogo pisó lo que había escrito el usuario"
    );

    let del_titulo = informe
        .iter()
        .find(|(campo, _)| *campo == Field::Title)
        .expect("hubo resultado para el título");
    assert!(
        matches!(del_titulo.1, metadata::Outcome::Suggested(_)),
        "el valor discrepante tenía que quedar como propuesta, no descartarse"
    );

    let vivas = metadata::pending(&conn, metadata::EntityType::Edition, &id).expect("propuestas");
    assert!(vivas.iter().any(|s| s.field == "title"));
}

/// Y donde el usuario no había decidido nada, sí se completa: esa es la razón de
/// que la función exista.
#[test]
fn donde_no_hay_decision_del_usuario_el_campo_se_completa() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let id = edicion(&conn, "Cien años de soledad");

    metadata_online::aplicar(&conn, &id, &candidato()).expect("aplicar");

    let editorial: Option<String> = conn
        .query_row(
            "SELECT publisher FROM editions WHERE id = ?1",
            [&id],
            |row| row.get(0),
        )
        .expect("editorial");
    assert_eq!(editorial.as_deref(), Some("Sudamericana"));

    // Y queda dicho de dónde salió: sin procedencia, un valor automático sería
    // indistinguible de uno escrito por el usuario.
    let historia = metadata::history(&conn, &id, Field::Publisher).expect("historia");
    assert!(historia.iter().any(|p| p.source == Source::Online));
}

/// **El criterio, segunda mitad.** Rechazar una propuesta no vuelve a preguntar.
#[test]
fn rechazar_una_propuesta_no_vuelve_a_preguntar() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let id = edicion(&conn, "cien anios");
    metadata::set_manual(&conn, &id, Field::Title, Some("Cien Años")).expect("manual");

    metadata_online::aplicar(&conn, &id, &candidato()).expect("primera consulta");
    let propuesta = metadata::pending(&conn, metadata::EntityType::Edition, &id)
        .expect("propuestas")
        .into_iter()
        .find(|s| s.field == "title")
        .expect("hay propuesta de título");
    metadata::reject(&conn, &propuesta.id).expect("rechazo");

    // Se consulta otra vez y responde lo mismo, que es lo que pasa de verdad.
    metadata_online::aplicar(&conn, &id, &candidato()).expect("segunda consulta");

    let vivas = metadata::pending(&conn, metadata::EntityType::Edition, &id).expect("propuestas");
    assert!(
        !vivas.iter().any(|s| s.field == "title"),
        "volvió a preguntar por lo que ya se había rechazado"
    );
}

/// El rechazo tampoco abre la puerta por el otro lado: si después se borra la
/// decisión manual, el valor rechazado no se escribe solo.
#[test]
fn lo_rechazado_tampoco_se_escribe_cuando_deja_de_haber_decision_manual() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let id = edicion(&conn, "cien anios");
    metadata::set_manual(&conn, &id, Field::Title, Some("Cien Años")).expect("manual");

    metadata_online::aplicar(&conn, &id, &candidato()).expect("consulta");
    let propuesta = metadata::pending(&conn, metadata::EntityType::Edition, &id)
        .expect("propuestas")
        .into_iter()
        .find(|s| s.field == "title")
        .expect("propuesta");
    metadata::reject(&conn, &propuesta.id).expect("rechazo");

    // Se borra la procedencia manual, que es lo que ocurriría al restaurar una
    // copia antigua o al limpiar el campo.
    conn.execute(
        "DELETE FROM metadata_provenance WHERE entity_id = ?1 AND field = 'title'",
        [&id],
    )
    .expect("borrado");

    metadata_online::aplicar(&conn, &id, &candidato()).expect("consulta");
    assert_eq!(
        titulo_de(&conn, &id),
        "Cien Años",
        "se escribió en silencio un valor que el usuario había rechazado"
    );
}

/// **El criterio, tercera mitad.** Con el modo sin red puesto no se intenta
/// salir, y la extracción local sigue siendo la vía normal: no falla nada suyo.
#[test]
fn con_el_modo_sin_red_no_se_consulta_el_catalogo() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(FICHA.as_bytes(), 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        settings::set(&conn, SettingKey::OfflineMode, &SettingValue::Flag(true)).expect("ajuste");
        net::grant(&conn, HOST_CATALOGO, Purpose::Metadata).expect("permiso");
    }

    assert!(matches!(
        metadata_online::buscar(&db, &red, "Cien años de soledad", None),
        Err(NbError::NetworkDisabled)
    ));
    assert!(salidas.lock().expect("salidas").is_empty());
}

/// Sin consentimiento tampoco, aunque el interruptor esté apagado. De fábrica no
/// sale nada.
#[test]
fn sin_consentimiento_el_catalogo_no_se_consulta() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(FICHA.as_bytes(), 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    assert!(matches!(
        metadata_online::buscar(&db, &red, "Cien años de soledad", None),
        Err(NbError::ConsentRequired)
    ));
    assert!(salidas.lock().expect("salidas").is_empty());
}

/// Con permiso sí, y la dirección se comprueba entera: es donde va el título del
/// usuario.
#[test]
fn con_permiso_se_busca_y_la_direccion_lleva_el_titulo_codificado() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(FICHA.as_bytes(), 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, HOST_CATALOGO, Purpose::Metadata).expect("permiso");
    }

    let candidatos = metadata_online::buscar(&db, &red, "Cien años de soledad", Some("García"))
        .expect("búsqueda");

    let pedidas = salidas.lock().expect("salidas");
    assert_eq!(pedidas.len(), 1);
    assert!(pedidas[0].starts_with("https://openlibrary.org/search.json?"));
    assert!(pedidas[0].contains("title=Cien%20a%C3%B1os%20de%20soledad"));
    assert!(pedidas[0].contains("author=Garc%C3%ADa"));

    assert_eq!(candidatos.len(), 1);
    assert_eq!(candidatos[0].title, "Cien años de soledad");
    assert_eq!(candidatos[0].publisher.as_deref(), Some("Sudamericana"));
    assert_eq!(candidatos[0].published_date.as_deref(), Some("1967"));
    assert_eq!(
        candidatos[0].cover_url.as_deref(),
        Some("https://covers.openlibrary.org/b/id/8231856-L.jpg")
    );
}

/// Una portada que no viene del anfitrión de portadas no se descarga, aunque el
/// anfitrión que la sirve esté autorizado para otra cosa. La dirección viene del
/// webview, que es zona no confiable.
#[test]
fn una_portada_de_otro_anfitrion_se_rechaza_antes_de_salir() {
    let tmp = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-portadas-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    let db = base();
    let (espia, salidas) = Espia::nuevo(&[0xFF, 0xD8, 0xFF, 0xE0], 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    let id = {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "evil.example", Purpose::Metadata).expect("permiso");
        edicion(&conn, "Obra")
    };

    assert!(matches!(
        metadata_online::descargar_portada(
            &db,
            &red,
            &tmp,
            &id,
            "https://evil.example/b/id/1-L.jpg"
        ),
        Err(NbError::PathNotAllowed)
    ));
    assert!(salidas.lock().expect("salidas").is_empty());
    let _ = std::fs::remove_dir_all(&tmp);
}

/// La portada se guarda con la extensión que dice su **contenido**, y queda
/// apuntada en la edición.
#[test]
fn la_portada_descargada_queda_en_la_cache_y_apuntada_en_la_edicion() {
    let tmp = nb_books_lib::paths::temporal_de_pruebas().join(format!(
        "nb-portadas-ok-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);

    let db = base();
    // Bytes de un PNG: la dirección dice `.jpg`, y lo que manda es el contenido.
    let (espia, _salidas) = Espia::nuevo(b"\x89PNG\r\n\x1a\n0123456789", 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    let id = {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, HOST_PORTADAS, Purpose::Metadata).expect("permiso");
        edicion(&conn, "Obra")
    };

    let nombre = metadata_online::descargar_portada(
        &db,
        &red,
        &tmp,
        &id,
        &format!("https://{HOST_PORTADAS}/b/id/7-L.jpg"),
    )
    .expect("descarga");

    assert_eq!(nombre, format!("{id}.png"));
    assert!(tmp.join(&nombre).exists());

    let conn = db.connection().expect("conexión");
    let apuntada: Option<String> = conn
        .query_row(
            "SELECT cover_path FROM editions WHERE id = ?1",
            [&id],
            |row| row.get(0),
        )
        .expect("consulta");
    assert_eq!(apuntada.as_deref(), Some(nombre.as_str()));

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Lo que llega como portada y no es una imagen conocida se rechaza. Un SVG
/// entra aquí: puede traer script dentro.
#[test]
fn lo_que_no_es_una_imagen_conocida_no_se_guarda_como_portada() {
    let tmp = nb_books_lib::paths::temporal_de_pruebas().join(format!(
        "nb-portadas-mala-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);

    let db = base();
    let (espia, _salidas) = Espia::nuevo(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>", 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    let id = {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, HOST_PORTADAS, Purpose::Metadata).expect("permiso");
        edicion(&conn, "Obra")
    };

    assert!(matches!(
        metadata_online::descargar_portada(
            &db,
            &red,
            &tmp,
            &id,
            &format!("https://{HOST_PORTADAS}/b/id/7-L.jpg")
        ),
        Err(NbError::NetworkFailure)
    ));
    assert!(!tmp.exists() || std::fs::read_dir(&tmp).into_iter().flatten().count() == 0);

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Aceptar una propuesta la convierte en decisión del usuario, no en otro valor
/// automático: si no, la siguiente consulta la volvería a discutir.
#[test]
fn aceptar_una_propuesta_la_convierte_en_decision_del_usuario() {
    let db = base();
    let conn = db.connection().expect("conexión");
    let id = edicion(&conn, "cien anios");
    metadata::set_manual(&conn, &id, Field::Title, Some("Cien Años")).expect("manual");

    metadata_online::aplicar(&conn, &id, &candidato()).expect("consulta");
    let propuesta = metadata::pending(&conn, metadata::EntityType::Edition, &id)
        .expect("propuestas")
        .into_iter()
        .find(|s| s.field == "title")
        .expect("propuesta");
    assert_eq!(propuesta.status, SuggestionStatus::Pending);

    metadata::accept(&conn, &propuesta.id).expect("aceptar");
    assert_eq!(titulo_de(&conn, &id), "Cien años de soledad");

    let historia = metadata::history(&conn, &id, Field::Title).expect("historia");
    assert_eq!(
        historia.first().map(|p| p.source),
        Some(Source::Manual),
        "aceptar tiene que quedar registrado como decisión del usuario"
    );
}
