//! Criterio de aceptación de la consulta en línea.
//!
//! "Con el interruptor en offline **se omiten** y el diccionario local sigue
//! respondiendo; el primer uso de cada uno pide consentimiento y dice **qué
//! texto se va a enviar**."
//!
//! Ninguna prueba de aquí sale a la red: el transporte es un espía que anota lo
//! que se le pide y devuelve lo que se le diga. Eso es lo que permite comprobar
//! **qué dirección** se construye, que es justo donde un error se convertiría en
//! mandar el texto del usuario a un sitio equivocado.

use std::sync::{Arc, Mutex};

use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::lookup::online::{Encyclopedia, Translation};
use nb_books_lib::lookup::{Entry, LookupKind, LookupProvider, LookupRegistry, Query, SkipReason};
use nb_books_lib::net::{self, HttpTransport, Network, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

/// Transporte que anota lo que se le pide y responde lo que se le diga.
struct Espia {
    salidas: Arc<Mutex<Vec<String>>>,
    respuesta: Mutex<Option<Response>>,
}

impl Espia {
    fn nuevo(cuerpo: &str, status: u16) -> (Self, Arc<Mutex<Vec<String>>>) {
        let salidas = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                salidas: Arc::clone(&salidas),
                respuesta: Mutex::new(Some(Response {
                    status,
                    content_type: Some("application/json".to_owned()),
                    location: None,
                    body: cuerpo.as_bytes().to_vec(),
                })),
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
        self.respuesta
            .lock()
            .expect("respuesta")
            .clone()
            .ok_or(NbError::NetworkFailure)
    }
}

/// Diccionario local de mentira: no hay obra lexicográfica en las pruebas, y lo
/// que se comprueba aquí es que **sigue respondiendo** cuando los de red no.
struct Local;

impl LookupProvider for Local {
    fn id(&self) -> &str {
        "local"
    }
    fn label(&self) -> &str {
        "Diccionario local"
    }
    fn kind(&self) -> LookupKind {
        LookupKind::Dictionary
    }
    fn languages(&self) -> &[String] {
        &[]
    }
    fn requires_network(&self) -> bool {
        false
    }
    fn lookup(&self, query: &Query) -> NbResult<Vec<Entry>> {
        Ok(vec![Entry {
            provider_id: "local".to_owned(),
            kind: LookupKind::Dictionary,
            headword: query.term.clone(),
            body: "Definición local.".to_owned(),
            source: None,
        }])
    }
}

fn base() -> Database {
    Database::open_in_memory().expect("base")
}

fn consulta(termino: &str, idioma: Option<&str>) -> Query {
    Query {
        term: termino.to_owned(),
        context: None,
        language: idioma.map(str::to_owned),
    }
}

const RESUMEN: &str = r#"{"title":"Alquimia","extract":"La alquimia es una antigua práctica."}"#;

/// **El criterio, primera mitad.** Con el modo sin red puesto, el proveedor en
/// línea se omite —diciendo por qué— y el diccionario local responde igual.
#[test]
fn con_el_modo_sin_red_los_de_linea_se_omiten_y_el_local_responde() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        settings::set(&conn, SettingKey::OfflineMode, &SettingValue::Flag(true)).expect("ajuste");
    }

    let mut registro = LookupRegistry::new();
    registro.register(Box::new(Local));
    registro.register(Box::new(Encyclopedia::new(db.clone(), Arc::clone(&red))));

    // `allow_network` es lo que el comando calcula del interruptor: aquí se
    // pasa falso porque el modo sin red está puesto.
    let resultado = registro
        .lookup(&consulta("alquimia", Some("es")), false)
        .expect("consulta");

    assert_eq!(resultado.entries.len(), 1);
    assert_eq!(resultado.entries[0].provider_id, "local");
    assert_eq!(resultado.skipped.len(), 1);
    assert_eq!(resultado.skipped[0].reason, SkipReason::NetworkDisabled);
    assert!(
        salidas.lock().expect("salidas").is_empty(),
        "con el modo sin red no puede haberse intentado ninguna salida"
    );
}

/// **El criterio, segunda mitad.** Sin consentimiento no sale nada, y lo que se
/// omite no se llama fallo: se dice que falta autorizar **ese** destino.
#[test]
fn sin_consentimiento_se_omite_diciendo_a_que_destino_falta_autorizar() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    let mut registro = LookupRegistry::new();
    registro.register(Box::new(Local));
    registro.register(Box::new(Encyclopedia::new(db.clone(), Arc::clone(&red))));

    let resultado = registro
        .lookup(&consulta("alquimia", Some("es-CO")), true)
        .expect("consulta");

    assert_eq!(resultado.skipped.len(), 1);
    let omitido = &resultado.skipped[0];
    assert_eq!(
        omitido.reason,
        SkipReason::NetworkNotAllowed,
        "falta de permiso no es un fallo: es una decisión que nadie ha tomado"
    );
    assert_eq!(
        omitido.host.as_deref(),
        Some("es.wikipedia.org"),
        "sin el destino, la interfaz podría decir que falta un permiso y no para qué"
    );
    assert!(salidas.lock().expect("salidas").is_empty());

    // Y el diccionario local no se entera de nada.
    assert_eq!(resultado.entries.len(), 1);
    assert_eq!(resultado.entries[0].provider_id, "local");
}

/// Con el permiso concedido sí sale, y sale **a donde debe**: la dirección se
/// comprueba entera, porque un error ahí manda el texto del usuario a otro sitio.
#[test]
fn con_el_permiso_concedido_la_enciclopedia_responde_y_la_direccion_es_la_esperada() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "es.wikipedia.org", Purpose::Lookup).expect("permiso");
    }

    let wiki = Encyclopedia::new(db.clone(), Arc::clone(&red));
    let entradas = wiki
        .lookup(&consulta("alquimia", Some("es")))
        .expect("consulta");

    assert_eq!(
        salidas.lock().expect("salidas").as_slice(),
        ["https://es.wikipedia.org/api/rest_v1/page/summary/alquimia"]
    );
    assert_eq!(entradas.len(), 1);
    assert_eq!(entradas[0].kind, LookupKind::Encyclopedia);
    assert_eq!(entradas[0].headword, "Alquimia");
    assert!(entradas[0].body.starts_with("La alquimia"));
    assert!(
        entradas[0]
            .source
            .as_deref()
            .is_some_and(|s| s.contains("CC BY-SA")),
        "el contenido es de terceros y su licencia exige atribución"
    );
}

/// El permiso de un anfitrión no vale para otro: consultar en español no
/// autoriza a consultar en portugués.
#[test]
fn el_permiso_de_una_wikipedia_no_vale_para_otra() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "es.wikipedia.org", Purpose::Lookup).expect("permiso");
    }

    let wiki = Encyclopedia::new(db.clone(), Arc::clone(&red));
    assert!(matches!(
        wiki.lookup(&consulta("alquimia", Some("pt"))),
        Err(NbError::ConsentRequired)
    ));
    assert!(salidas.lock().expect("salidas").is_empty());
}

/// Un permiso para consultar no puede convertirse en un permiso para descargar.
#[test]
fn el_permiso_de_consultar_no_sirve_para_otro_proposito() {
    let db = base();
    let (espia, _salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "es.wikipedia.org", Purpose::Catalog).expect("permiso");
    }

    let wiki = Encyclopedia::new(db, Arc::clone(&red));
    assert!(matches!(
        wiki.lookup(&consulta("alquimia", Some("es"))),
        Err(NbError::ConsentRequired)
    ));
}

/// El idioma lo escribe quien hizo el EPUB. Sin comprobarlo, el destino de la
/// petición lo elegiría un tercero.
#[test]
fn un_idioma_manipulado_no_produce_ninguna_salida() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        // Se concede todo lo concebible: si aun así no sale, es porque la
        // dirección no llegó a construirse.
        net::grant(&conn, "es.evil.com", Purpose::Lookup).expect("permiso");
        net::grant(&conn, "evil.com", Purpose::Lookup).expect("permiso");
    }

    let wiki = Encyclopedia::new(db, Arc::clone(&red));
    let entradas = wiki
        .lookup(&consulta("alquimia", Some("es.evil.com")))
        .expect("no es un fallo: es una consulta que no se puede atender");

    assert!(entradas.is_empty());
    assert!(
        salidas.lock().expect("salidas").is_empty(),
        "un idioma manipulado no puede convertirse en un anfitrión"
    );
}

/// Una palabra sin artículo responde 404, y eso no es un fallo del proveedor:
/// es que no hay artículo. Llamarlo fallo dejaría al panel diciendo "no
/// respondió" cuando respondió perfectamente.
#[test]
fn una_palabra_sin_articulo_devuelve_vacio_y_no_un_error() {
    let db = base();
    let (espia, _salidas) = Espia::nuevo(r#"{"type":"not_found"}"#, 404);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "es.wikipedia.org", Purpose::Lookup).expect("permiso");
    }

    let wiki = Encyclopedia::new(db, red);
    assert!(wiki
        .lookup(&consulta("noexistealgo", Some("es")))
        .expect("no es un error")
        .is_empty());
}

/// El traductor manda **el párrafo**, no la palabra suelta: sin la frase, la
/// acepción es la equivocada la mitad de las veces.
#[test]
fn el_traductor_envia_el_contexto_cuando_lo_hay() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(r#"{"translatedText":"The colonel remembered."}"#, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "libre.example", Purpose::Lookup).expect("permiso");
    }

    let traductor =
        Translation::new(db, Arc::clone(&red), "libre.example", "en").expect("configurado");
    let mut query = consulta("recordó", Some("es"));
    query.context = Some("El coronel recordó aquella tarde.".to_owned());

    let entradas = traductor.lookup(&query).expect("consulta");

    let pedidas = salidas.lock().expect("salidas");
    assert_eq!(pedidas.len(), 1);
    assert!(
        pedidas[0].contains("El%20coronel%20record%C3%B3"),
        "no viajó el contexto: {}",
        pedidas[0]
    );
    assert!(pedidas[0].contains("target=en"));
    assert_eq!(entradas.len(), 1);
    assert_eq!(entradas[0].kind, LookupKind::Translation);
    assert!(
        entradas[0]
            .source
            .as_deref()
            .is_some_and(|s| s.contains("automática")),
        "una traducción automática y una definición no valen lo mismo"
    );
}

/// Un término con `&` o `?` dentro no puede cambiar la petición: es texto del
/// documento, así que es entrada no confiable.
#[test]
fn un_termino_con_signos_no_puede_alterar_la_peticion() {
    let db = base();
    let (espia, salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "es.wikipedia.org", Purpose::Lookup).expect("permiso");
    }

    let wiki = Encyclopedia::new(db, red);
    let _ = wiki.lookup(&consulta("a?b&c/../d", Some("es")));

    let pedidas = salidas.lock().expect("salidas");
    assert_eq!(
        pedidas[0],
        "https://es.wikipedia.org/api/rest_v1/page/summary/a%3Fb%26c%2F..%2Fd"
    );
}

/// Autorizar pasa por el registro, y el registro solo reconoce los destinos de
/// sus proveedores. Sin esta comprobación, el webview sería el sitio donde se
/// decide a dónde sale la aplicación.
#[test]
fn el_registro_solo_reconoce_los_destinos_de_sus_proveedores() {
    let db = base();
    let (espia, _salidas) = Espia::nuevo(RESUMEN, 200);
    let red = Arc::new(Network::new(Box::new(espia)));

    let mut registro = LookupRegistry::new();
    registro.register(Box::new(Local));
    registro.register(Box::new(Encyclopedia::new(db.clone(), Arc::clone(&red))));
    registro.register(Box::new(
        Translation::new(db, red, "libre.example", "es").expect("configurado"),
    ));

    assert!(registro.algun_proveedor_usa("es.wikipedia.org"));
    assert!(registro.algun_proveedor_usa("libre.example"));

    for ajeno in [
        "evil.com",
        "es.wikipedia.org.evil.com",
        "libre.example.evil.com",
        "wikipedia.org",
    ] {
        assert!(
            !registro.algun_proveedor_usa(ajeno),
            "el registro reconoció {ajeno:?}, que no es de ningún proveedor"
        );
    }
}

/// Lo que se envía se dice antes de enviarlo, y el texto vive junto al
/// propósito para que no se quede viejo cuando el propósito cambie.
#[test]
fn cada_proposito_dice_que_texto_se_envia() {
    assert!(Purpose::Lookup.what_is_sent().contains("selecciones"));
    assert!(Purpose::Catalog.what_is_sent().contains("catálogo"));
    assert!(Purpose::Metadata.what_is_sent().contains("título"));
}
