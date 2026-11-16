//! Criterio de aceptación de los catálogos OPDS.
//!
//! "Un catálogo **inalcanzable**, uno que devuelve **basura** y uno que devuelve
//! **un archivo que no es lo que dice** se rechazan sin dejar registros
//! huérfanos; nada descargado esquiva la validación de formato por contenido."
//!
//! Los tres casos están aquí, y los tres se comprueban de la misma forma: se
//! afirma sobre el **estado de la base después**, no sobre lo que devolvió la
//! llamada. Que una función devuelva error no demuestra que no haya escrito
//! nada, y eso es justo lo que el criterio dice.

use std::sync::{Arc, Mutex};

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::events::EventBus;
use nb_books_lib::library;
use nb_books_lib::net::{self, HttpTransport, Network, Purpose, Response};
use nb_books_lib::opds;

/// Transporte que responde distinto según lo que se le pida.
///
/// Hace falta porque una descarga son **dos** peticiones —el feed y el
/// documento— y el criterio habla de qué pasa cuando la segunda miente.
struct Servidor {
    salidas: Arc<Mutex<Vec<String>>>,
    respuestas: Vec<(String, Response)>,
}

impl Servidor {
    /// Igual que [`Servidor::nuevo`], pero una de las direcciones redirige.
    fn con_redireccion(
        respuestas: Vec<(&str, u16, Vec<u8>)>,
        desde: &str,
        hacia: &str,
    ) -> (Self, Arc<Mutex<Vec<String>>>) {
        let (mut servidor, salidas) = Self::nuevo(respuestas);
        servidor.respuestas.push((
            desde.to_owned(),
            Response {
                status: 302,
                content_type: None,
                body: Vec::new(),
                location: Some(hacia.to_owned()),
            },
        ));
        (servidor, salidas)
    }

    fn nuevo(respuestas: Vec<(&str, u16, Vec<u8>)>) -> (Self, Arc<Mutex<Vec<String>>>) {
        let salidas = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                salidas: Arc::clone(&salidas),
                respuestas: respuestas
                    .into_iter()
                    .map(|(sufijo, status, cuerpo)| {
                        (
                            sufijo.to_owned(),
                            Response {
                                status,
                                content_type: None,
                                body: cuerpo,
                                location: None,
                            },
                        )
                    })
                    .collect(),
            },
            salidas,
        )
    }
}

impl HttpTransport for Servidor {
    fn get(&self, url: &str) -> NbResult<Response> {
        self.salidas.lock().expect("registro").push(url.to_owned());
        self.respuestas
            .iter()
            .find(|(sufijo, _)| url.ends_with(sufijo.as_str()))
            .map(|(_, r)| r.clone())
            // Sin respuesta preparada, el servidor no está: es el catálogo
            // inalcanzable del criterio.
            .ok_or(NbError::NetworkFailure)
    }
}

const RAIZ: &str = "https://catalogo.example/opds/raiz.xml";

const FEED: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Catálogo de ejemplo</title>
  <entry>
    <title>Una obra</title>
    <author><name>Alguien</name></author>
    <link rel="http://opds-spec.org/acquisition"
          type="application/epub+zip" href="obras/una.epub"/>
  </entry>
</feed>"#;

/// Un EPUB mínimo de verdad: la importación reconoce el formato por contenido,
/// así que hace falta contenido de verdad para comprobar que lo reconoce.
fn epub_valido() -> Vec<u8> {
    std::fs::read(ruta_de_fixture("minimo-valido.epub")).expect("fixture de EPUB")
}

fn ruta_de_fixture(nombre: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(nombre)
}

struct Temporal(std::path::PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas().join(format!(
            "nb-opds-{nombre}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }
    fn sub(&self, nombre: &str) -> std::path::PathBuf {
        self.0.join(nombre)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn base() -> Database {
    Database::open_in_memory().expect("base")
}

fn con_permiso(db: &Database) {
    let conn = db.connection().expect("conexión");
    net::grant(&conn, "catalogo.example", Purpose::Catalog).expect("permiso");
}

fn cuantos_archivos(db: &Database) -> i64 {
    let conn = db.connection().expect("conexión");
    conn.query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .expect("cuenta")
}

fn cuantas_obras(db: &Database) -> i64 {
    let conn = db.connection().expect("conexión");
    conn.query_row("SELECT count(*) FROM works", [], |r| r.get(0))
        .expect("cuenta")
}

/// El feed de una página, o un fallo de la prueba si la página era otra cosa.
fn feed_de(pagina: NbResult<opds::CatalogPage>) -> opds::Feed {
    match pagina.expect("página") {
        opds::CatalogPage::Feed(feed) => feed,
        otra => panic!("se esperaba un feed y llegó {otra:?}"),
    }
}

/// Descarga e importa como lo hace el comando del puente: temporal, importación
/// por la canalización de siempre, y borrado pase lo que pase.
fn descargar_e_importar(
    db: &Database,
    net: &Arc<Network>,
    tmp: &Temporal,
    url: &str,
) -> NbResult<library::ImportOutcome> {
    let descargas = tmp.sub("descargas");
    let opds::Descarga::Guardada(temporal) = opds::descargar(db, net, &descargas, url)? else {
        return Err(NbError::ConsentRequired);
    };

    let biblioteca = tmp.sub("biblioteca");
    std::fs::create_dir_all(&biblioteca).expect("biblioteca");
    let bus = EventBus::new();
    let mut conn = db.connection().expect("conexión");
    let ctx = library::ImportContext {
        scope: &descargas,
        library: &biblioteca,
        bus: &bus,
    };

    let resultado = library::import_document(
        &mut conn,
        &ctx,
        &library::ImportRequest {
            source: temporal.clone(),
            storage_mode: StorageMode::Managed,
            root: None,
        },
    );
    let _ = std::fs::remove_file(&temporal);
    resultado
}

/// **El criterio, primer caso.** Un catálogo inalcanzable no deja nada detrás.
#[test]
fn un_catalogo_inalcanzable_se_rechaza_sin_dejar_registros() {
    let db = base();
    con_permiso(&db);
    let (servidor, salidas) = Servidor::nuevo(Vec::new());
    let red = Arc::new(Network::new(Box::new(servidor)));

    assert!(matches!(
        opds::navegar(&db, &red, RAIZ),
        Err(NbError::NetworkFailure)
    ));

    // Salió, y el intento quedó registrado: es lo que el usuario querría ver.
    assert_eq!(salidas.lock().expect("salidas").len(), 1);
    assert_eq!(cuantas_obras(&db), 0);
    assert_eq!(cuantos_archivos(&db), 0);
}

/// **El criterio, segundo caso.** Un catálogo que devuelve basura se rechaza, y
/// **con otro código**: inalcanzable y corrupto piden cosas distintas al
/// usuario.
#[test]
fn un_catalogo_que_devuelve_basura_se_rechaza_con_su_propio_codigo() {
    let db = base();
    con_permiso(&db);

    for basura in [
        b"<html><body>hola</body></html>".to_vec(),
        b"{\"no\": \"soy un feed\"}".to_vec(),
        b"\x00\x01\x02\x03".to_vec(),
    ] {
        let (servidor, _) = Servidor::nuevo(vec![("raiz.xml", 200, basura)]);
        let red = Arc::new(Network::new(Box::new(servidor)));

        assert!(matches!(
            opds::navegar(&db, &red, RAIZ),
            Err(NbError::DocumentCorrupted)
        ));
    }

    assert_eq!(cuantas_obras(&db), 0);
    assert_eq!(cuantos_archivos(&db), 0);
}

/// **El criterio, tercer caso y el que más importa.** El catálogo anuncia un
/// EPUB y sirve otra cosa: se rechaza por **contenido**, y no queda registro.
#[test]
fn un_archivo_que_no_es_lo_que_dice_se_rechaza_por_su_contenido() {
    let tmp = Temporal::nuevo("mentira");
    let db = base();
    con_permiso(&db);

    let (servidor, _) = Servidor::nuevo(vec![
        ("raiz.xml", 200, FEED.as_bytes().to_vec()),
        // Dice `application/epub+zip` en el feed y sirve texto plano.
        ("una.epub", 200, b"esto no es un epub, es texto".to_vec()),
    ]);
    let red = Arc::new(Network::new(Box::new(servidor)));

    let feed = feed_de(opds::navegar(&db, &red, RAIZ));
    let descarga = feed.entries[0].download.clone().expect("descarga");
    assert_eq!(
        feed.entries[0].media_type.as_deref(),
        Some("application/epub+zip"),
        "el feed dice que es un EPUB, y eso es justo lo que no nos creemos"
    );

    let resultado = descargar_e_importar(&db, &red, &tmp, &descarga);
    assert!(
        matches!(resultado, Err(NbError::FormatUnsupported)),
        "se aceptó un archivo que no es lo que el catálogo decía: {resultado:?}"
    );

    assert_eq!(cuantas_obras(&db), 0, "quedó una obra huérfana");
    assert_eq!(cuantos_archivos(&db), 0, "quedó un archivo huérfano");
    // Y no quedó copia en la biblioteca.
    let biblioteca = tmp.sub("biblioteca");
    let copias = std::fs::read_dir(&biblioteca)
        .map(|d| d.flatten().count())
        .unwrap_or(0);
    assert_eq!(copias, 0, "quedó una copia de algo que se rechazó");
}

/// Y lo que sí es lo que dice entra, por la misma canalización que un archivo
/// que el usuario suelta sobre la ventana.
#[test]
fn lo_que_si_es_un_documento_entra_por_la_canalizacion_de_siempre() {
    let tmp = Temporal::nuevo("bueno");
    let db = base();
    con_permiso(&db);

    let (servidor, salidas) = Servidor::nuevo(vec![
        ("raiz.xml", 200, FEED.as_bytes().to_vec()),
        ("una.epub", 200, epub_valido()),
    ]);
    let red = Arc::new(Network::new(Box::new(servidor)));

    let feed = feed_de(opds::navegar(&db, &red, RAIZ));
    let descarga = feed.entries[0].download.clone().expect("descarga");

    let resultado = descargar_e_importar(&db, &red, &tmp, &descarga).expect("importación");
    assert!(matches!(resultado, library::ImportOutcome::Imported(_)));

    assert_eq!(cuantas_obras(&db), 1);
    assert_eq!(cuantos_archivos(&db), 1);

    // Dos peticiones y ni una más: el feed y el documento.
    assert_eq!(salidas.lock().expect("salidas").len(), 2);

    // El temporal no se queda: la descarga es un paso, no un almacén.
    let descargas = tmp.sub("descargas");
    let sobrantes = std::fs::read_dir(&descargas)
        .map(|d| d.flatten().count())
        .unwrap_or(0);
    assert_eq!(sobrantes, 0, "quedó un temporal de descarga sin borrar");
}

/// Sin consentimiento no se navega ningún catálogo, aunque esté añadido. Añadir
/// una dirección no es autorizar una salida.
///
/// La prueba se borró el 2026-08-31 junto con la propiedad que vigilaba, cuando
/// `opds::add` pasó a conceder el permiso por su cuenta. Vuelve con ella: es la
/// única guarda de "nada sale del dispositivo sin consentimiento explícito" en
/// este camino, y una regla sin mecanismo no es una regla.
#[test]
fn anadir_un_catalogo_no_autoriza_a_salir_a_el() {
    let db = base();
    let (servidor, salidas) = Servidor::nuevo(vec![("raiz.xml", 200, FEED.as_bytes().to_vec())]);
    let red = Arc::new(Network::new(Box::new(servidor)));

    {
        let conn = db.connection().expect("conexión");
        opds::add(&conn, "Catálogo de ejemplo", RAIZ).expect("añadir");
    }

    assert!(matches!(
        opds::navegar(&db, &red, RAIZ),
        Ok(opds::CatalogPage::NeedsConsent { .. })
    ));
    assert!(salidas.lock().expect("salidas").is_empty());
}

/// Una redirección dentro del mismo anfitrión se sigue, porque ese anfitrión ya
/// estaba autorizado y redirigir no lo cambia.
///
/// Sin esto, la puerta no protegía: impedía funcionar. Casi todo catálogo real
/// redirige —de `www` al dominio pelado, de una ruta sin barra final a la que la
/// lleva— y la respuesta acababa siendo `NetworkFailure`, o sea "no se pudo
/// alcanzar el catálogo" con el catálogo contestando perfectamente.
#[test]
fn una_redireccion_al_mismo_anfitrion_se_sigue() {
    let db = base();
    con_permiso(&db);
    let (servidor, salidas) = Servidor::con_redireccion(
        vec![("destino.xml", 200, FEED.as_bytes().to_vec())],
        "raiz.xml",
        "https://catalogo.example/opds/destino.xml",
    );
    let red = Arc::new(Network::new(Box::new(servidor)));

    let feed = feed_de(opds::navegar(&db, &red, RAIZ));
    assert_eq!(
        feed.entries.len(),
        1,
        "llega el feed de destino, no la respuesta vacía del 302"
    );

    let recorrido = salidas.lock().expect("salidas");
    assert_eq!(
        recorrido.len(),
        2,
        "se pidió la primera y se siguió a la segunda"
    );
    assert!(recorrido[1].ends_with("destino.xml"));
}

/// Una redirección a **otro** anfitrión no se sigue sin permiso para ese otro.
///
/// Es lo que hace que seguir redirecciones no abra un agujero: el destino vuelve
/// a pasar por la puerta, y la puerta no conoce al anfitrión nuevo. Que el
/// primero estuviera autorizado no autoriza a donde te mande.
#[test]
fn una_redireccion_a_otro_anfitrion_pide_permiso_para_ese_otro() {
    let db = base();
    con_permiso(&db);
    let (servidor, salidas) = Servidor::con_redireccion(
        vec![("destino.xml", 200, FEED.as_bytes().to_vec())],
        "raiz.xml",
        "https://otro.example/opds/destino.xml",
    );
    let red = Arc::new(Network::new(Box::new(servidor)));

    /*
     * **Y dice cuál falta, que es el otro**. Antes esto
     * volvía como `CONSENT_REQUIRED` a secas, y la interfaz autorizaba el
     * anfitrión del catálogo, que ya lo estaba: autorizado y sin sincronizar, en
     * bucle. Pasó con `epublibre.gratis`, que redirige a `ww2.epublibre.app`.
     */
    assert_eq!(
        opds::navegar(&db, &red, RAIZ).expect("página"),
        opds::CatalogPage::NeedsConsent {
            host: "otro.example".to_owned(),
            cleartext: false
        }
    );

    // Y no se llegó a pedir nada al anfitrión nuevo: la puerta corta antes.
    let recorrido = salidas.lock().expect("salidas");
    assert_eq!(recorrido.len(), 1);
    assert!(!recorrido[0].contains("otro.example"));
}

/// Y el anfitrión al que se quiso ir queda en el registro con su nombre.
///
/// Sin esto, el usuario ve "falta autorizar" y no puede saber a quién: el
/// catálogo que él añadió sí estaba autorizado. El registro de red es donde se
/// autoriza, así que es donde tiene que aparecer.
#[test]
fn el_anfitrion_de_una_redireccion_denegada_queda_en_el_registro() {
    let db = base();
    con_permiso(&db);
    let (servidor, _) = Servidor::con_redireccion(
        vec![("destino.xml", 200, FEED.as_bytes().to_vec())],
        "raiz.xml",
        "https://otro.example/opds/destino.xml",
    );
    let red = Arc::new(Network::new(Box::new(servidor)));
    let _ = opds::navegar(&db, &red, RAIZ);

    let conn = db.connection().expect("conexión");
    let registro = net::log(&conn, 10).expect("registro");
    let denegado = registro
        .iter()
        .find(|l| l.outcome == net::Outcome::Denied)
        .expect("hay una salida denegada");
    assert_eq!(denegado.host, "otro.example");
}

/// Una cadena de redirecciones sin fin se corta en lugar de dar vueltas.
#[test]
fn una_redireccion_que_no_acaba_se_corta() {
    let db = base();
    con_permiso(&db);
    let (servidor, salidas) =
        Servidor::con_redireccion(vec![], "raiz.xml", "https://catalogo.example/opds/raiz.xml");
    let red = Arc::new(Network::new(Box::new(servidor)));

    assert!(matches!(
        opds::navegar(&db, &red, RAIZ),
        Err(NbError::NetworkFailure)
    ));
    assert!(
        salidas.lock().expect("salidas").len() <= 6,
        "la cadena tiene que cortarse, no seguir saliendo"
    );
}

/// Ninguno viene puesto: la tabla nace vacía.
#[test]
fn no_hay_ningun_catalogo_preconfigurado() {
    let db = base();
    let conn = db.connection().expect("conexión");
    assert!(opds::list(&conn).expect("lista").is_empty());
}

/// Una dirección que no es `https` ni `http` no llega ni a guardarse: si se
/// aceptara aquí, el catálogo existiría y la puerta lo rechazaría siempre.
#[test]
fn un_catalogo_que_no_es_https_ni_http_no_se_guarda() {
    let db = base();
    let conn = db.connection().expect("conexión");

    for mala in [
        "ftp://catalogo.example/opds",
        "file:///etc/passwd",
        "catalogo.example/opds",
        "javascript:alert(1)",
    ] {
        assert!(
            matches!(
                opds::add(&conn, "Prueba", mala),
                Err(NbError::PathNotAllowed)
            ),
            "se guardó {mala:?} como catálogo"
        );
    }
    assert!(opds::list(&conn).expect("lista").is_empty());
}

/// El mismo catálogo dos veces es el mismo catálogo.
#[test]
fn un_catalogo_repetido_no_se_duplica() {
    let db = base();
    let conn = db.connection().expect("conexión");

    opds::add(&conn, "Ejemplo", RAIZ).expect("primera");
    assert!(opds::add(&conn, "Ejemplo otra vez", RAIZ).is_err());
    assert_eq!(opds::list(&conn).expect("lista").len(), 1);
}

/// Un catálogo en claro se admite, con **su propio** consentimiento.
///
/// Se decidió el 2026-09-12. Las dos mitades que importan: tener permiso
/// para el anfitrión por `https` no autoriza a hablarle en claro, y el claro
/// autorizado sí sale. Y la voz, que manda el texto del libro, sigue sin poder
/// salir en claro hacia internet: la excepción es de catálogos.
#[test]
fn un_catalogo_en_claro_pide_su_propio_permiso_y_con_el_sale() {
    const EN_CLARO: &str = "http://catalogo.example/opds/raiz.xml";
    let db = base();
    {
        let conn = db.connection().expect("conexión");
        opds::add(&conn, "En claro", EN_CLARO).expect("un catálogo en claro se guarda");
    }
    con_permiso(&db);
    let (servidor, salidas) = Servidor::nuevo(vec![("raiz.xml", 200, FEED.as_bytes().to_vec())]);
    let red = Arc::new(Network::new(Box::new(servidor)));

    assert_eq!(
        opds::navegar(&db, &red, EN_CLARO).expect("página"),
        opds::CatalogPage::NeedsConsent {
            host: "catalogo.example".to_owned(),
            cleartext: true
        },
        "el permiso por https no vale para el claro"
    );
    assert!(salidas.lock().expect("salidas").is_empty());

    {
        let conn = db.connection().expect("conexión");
        net::grant(&conn, "catalogo.example", Purpose::CatalogCleartext).expect("permiso");
    }
    let feed = feed_de(opds::navegar(&db, &red, EN_CLARO));
    assert_eq!(feed.entries.len(), 1);

    let conn = db.connection().expect("conexión");
    net::grant(&conn, "catalogo.example", Purpose::Speech).expect("permiso de voz");
    assert!(matches!(
        red.get(
            &conn,
            "http://catalogo.example/v1/audio/speech",
            Purpose::Speech
        ),
        Err(NbError::PathNotAllowed)
    ));
}

/// Un catálogo que pide credenciales lo dice, en lugar de "no se pudo
/// alcanzar": el servidor contestó. Pasa con Standard Ebooks, que responde 401.
#[test]
fn un_catalogo_que_pide_credenciales_lo_dice() {
    let db = base();
    con_permiso(&db);
    let (servidor, _) = Servidor::nuevo(vec![("raiz.xml", 401, b"Unauthorized".to_vec())]);
    let red = Arc::new(Network::new(Box::new(servidor)));
    assert_eq!(
        opds::navegar(&db, &red, RAIZ).expect("página"),
        opds::CatalogPage::NeedsLogin
    );
}
