//! Criterio de aceptación de la salida de red.
//!
//! "Con el interruptor en offline, **ninguna función intenta salir**, y las que
//! dependen de red se omiten en lugar de fallar; una prueba comprueba que no
//! existe ninguna ruta de red que esquive esta capa."
//!
//! La segunda mitad no se puede demostrar con una prueba de Rust: haría falta
//! observar todo el proceso. La hace cumplir `scripts/check-network.mjs`, que
//! rechaza cualquier uso del cliente HTTP fuera de `src/net.rs`, y que se
//! ejecuta en `pnpm verify` y en el gancho de pre-commit. Lo que sí se
//! comprueba aquí es que **la puerta no deja pasar nada** sin consentimiento, y
//! que un transporte que registrase una salida indebida quedaría en evidencia.

use std::sync::{Arc, Mutex};

use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::net::{self, HttpTransport, Network, Outcome, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

/// Transporte que **anota cada salida** en lugar de hacerla.
///
/// Es la pieza que convierte "no debe intentar salir" en algo comprobable: si
/// la puerta dejara pasar algo que no debía, aquí quedaría la huella.
#[derive(Default)]
struct Espia {
    salidas: Arc<Mutex<Vec<String>>>,
    respuesta: Option<Response>,
}

impl Espia {
    fn con_respuesta(cuerpo: &str) -> (Self, Arc<Mutex<Vec<String>>>) {
        let salidas = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                salidas: Arc::clone(&salidas),
                respuesta: Some(Response {
                    status: 200,
                    content_type: Some("text/plain".to_owned()),
                    location: None,
                    body: cuerpo.as_bytes().to_vec(),
                }),
            },
            salidas,
        )
    }
}

impl HttpTransport for Espia {
    fn get(&self, url: &str) -> nb_books_lib::error::NbResult<Response> {
        self.salidas
            .lock()
            .expect("registro del espía")
            .push(url.to_owned());
        self.respuesta.clone().ok_or(NbError::NetworkFailure)
    }
}

fn base() -> Database {
    Database::open_in_memory().expect("base")
}

/// **El criterio, primera mitad.** Con el modo sin red puesto no se intenta
/// salir: el transporte no llega a recibir la petición.
#[test]
fn con_el_modo_sin_red_no_se_intenta_salir_siquiera() {
    let db = base();
    let conn = db.connection().expect("conexión");

    // Consentimiento concedido, para que lo único que impida salir sea el
    // interruptor y no la falta de permiso.
    net::grant(&conn, "catalogo.ejemplo", Purpose::Catalog).expect("consentimiento");
    settings::set(&conn, SettingKey::OfflineMode, &SettingValue::Flag(true)).expect("interruptor");

    let (espia, salidas) = Espia::con_respuesta("no debería llegar aquí");
    let red = Network::new(Box::new(espia));

    let resultado = red.get(&conn, "https://catalogo.ejemplo/opds", Purpose::Catalog);

    assert!(matches!(resultado, Err(NbError::NetworkDisabled)));
    assert!(
        salidas.lock().expect("registro").is_empty(),
        "el interruptor tiene que cortar antes del transporte, no después"
    );
}

/// Sin consentimiento tampoco se sale, y el intento queda registrado: un
/// rechazo es justo lo que el usuario querría poder ver.
#[test]
fn sin_consentimiento_no_se_sale_y_el_intento_queda_registrado() {
    let db = base();
    let conn = db.connection().expect("conexión");

    let (espia, salidas) = Espia::con_respuesta("no debería llegar aquí");
    let red = Network::new(Box::new(espia));

    let resultado = red.get(&conn, "https://catalogo.ejemplo/opds", Purpose::Catalog);

    assert!(matches!(resultado, Err(NbError::ConsentRequired)));
    assert!(salidas.lock().expect("registro").is_empty());

    let registro = net::log(&conn, 10).expect("registro");
    assert_eq!(registro.len(), 1);
    assert_eq!(registro[0].outcome, Outcome::Denied);
    assert_eq!(registro[0].host, "catalogo.ejemplo");
}

/// El consentimiento es por anfitrión **y por propósito**. Autorizar un servidor
/// a servir un catálogo no lo autoriza a recibir el texto de una selección.
#[test]
fn el_consentimiento_de_un_proposito_no_vale_para_otro() {
    let db = base();
    let conn = db.connection().expect("conexión");
    net::grant(&conn, "servidor.ejemplo", Purpose::Catalog).expect("consentimiento");

    let (espia, salidas) = Espia::con_respuesta("contenido");
    let red = Network::new(Box::new(espia));

    red.get(&conn, "https://servidor.ejemplo/a", Purpose::Catalog)
        .expect("el propósito consentido sí sale");

    let otro = red.get(&conn, "https://servidor.ejemplo/b", Purpose::Lookup);
    assert!(matches!(otro, Err(NbError::ConsentRequired)));

    assert_eq!(
        salidas.lock().expect("registro").len(),
        1,
        "solo debía salir la petición del propósito consentido"
    );
}

/// Revocar corta de inmediato, sin reiniciar nada.
#[test]
fn revocar_el_consentimiento_corta_la_salida() {
    let db = base();
    let conn = db.connection().expect("conexión");
    net::grant(&conn, "servidor.ejemplo", Purpose::Metadata).expect("consentimiento");

    let (espia, _) = Espia::con_respuesta("portada");
    let red = Network::new(Box::new(espia));

    red.get(&conn, "https://servidor.ejemplo/portada", Purpose::Metadata)
        .expect("con permiso sale");

    net::revoke(&conn, "servidor.ejemplo", Purpose::Metadata).expect("revocado");

    assert!(matches!(
        red.get(&conn, "https://servidor.ejemplo/portada", Purpose::Metadata),
        Err(NbError::ConsentRequired)
    ));
}

/// Lo que salió queda registrado con su anfitrión, su propósito y su tamaño, y
/// **sin la ruta**: una ruta puede llevar dentro el título de un documento o el
/// texto que el usuario seleccionó.
#[test]
fn el_registro_dice_que_salio_sin_delatar_que_se_pidio() {
    let db = base();
    let conn = db.connection().expect("conexión");
    net::grant(&conn, "servidor.ejemplo", Purpose::Lookup).expect("consentimiento");

    let (espia, _) = Espia::con_respuesta("definición");
    let red = Network::new(Box::new(espia));

    red.get(
        &conn,
        "https://servidor.ejemplo/buscar?q=palabra-que-el-usuario-selecciono",
        Purpose::Lookup,
    )
    .expect("sale");

    let registro = net::log(&conn, 10).expect("registro");
    assert_eq!(registro[0].outcome, Outcome::Allowed);
    assert_eq!(registro[0].host, "servidor.ejemplo");
    assert_eq!(registro[0].purpose, "lookup");
    assert_eq!(registro[0].status, Some(200));
    assert_eq!(registro[0].bytes, "definición".len() as u64);

    let crudo: String = conn
        .query_row(
            "SELECT group_concat(host || purpose) FROM network_log",
            [],
            |r| r.get(0),
        )
        .expect("consulta");
    assert!(
        !crudo.contains("palabra-que-el-usuario-selecciono"),
        "el registro no puede llevarse el texto del usuario"
    );
}

/// Solo `https`. Una dirección en claro deja el destino y el contenido a la
/// vista de cualquiera en la red.
///
/// **Con una excepción, y no es esta**: un catálogo en claro se admite
/// con su propio consentimiento, porque lo decidió quien tiene los catálogos. Lo
/// que se consulta, lo que se lee en voz alta y lo que manda una extensión siguen
/// sin poder salir en claro hacia internet, y es lo que se comprueba aquí. El
/// caso del catálogo está en `catalogos_opds.rs`.
#[test]
fn una_direccion_que_no_es_https_se_rechaza_antes_de_mirar_el_consentimiento() {
    let db = base();
    let conn = db.connection().expect("conexión");
    net::grant(&conn, "servidor.ejemplo", Purpose::Lookup).expect("consentimiento");

    let (espia, salidas) = Espia::con_respuesta("contenido");
    let red = Network::new(Box::new(espia));

    for direccion in [
        "http://servidor.ejemplo/a",
        "ftp://servidor.ejemplo/a",
        "servidor.ejemplo/a",
        "https:///sin-anfitrion",
    ] {
        assert!(
            matches!(
                red.get(&conn, direccion, Purpose::Lookup),
                Err(NbError::PathNotAllowed)
            ),
            "debía rechazarse: {direccion}"
        );
    }

    assert!(salidas.lock().expect("registro").is_empty());
}

/// El anfitrión se extrae sin puerto, sin credenciales y en minúsculas: si no,
/// un mismo servidor pediría consentimiento varias veces según cómo se escriba.
#[test]
fn el_anfitrion_se_normaliza_para_que_el_consentimiento_no_dependa_de_como_se_escriba() {
    assert_eq!(
        net::host_de("https://Ejemplo.ORG/ruta").expect("host"),
        "ejemplo.org"
    );
    assert_eq!(
        net::host_de("https://ejemplo.org:8443/x").expect("host"),
        "ejemplo.org"
    );
    assert_eq!(
        net::host_de("https://usuario:clave@ejemplo.org/x").expect("host"),
        "ejemplo.org"
    );
    assert_eq!(
        net::host_de("https://ejemplo.org").expect("host"),
        "ejemplo.org"
    );
    assert!(net::host_de("https://").is_err());
}

/// `http` en claro dentro de la propia red, y en ningún caso hacia internet.
///
/// **Cambió el 2026-09-02, y el motivo fue un servidor real que no se podía
/// usar.** La regla anterior solo admitía `localhost`, y el caso que aparece de
/// verdad es un servidor de síntesis en una máquina virtual o en otro equipo de
/// la casa: `http://192.168.100.10:8880`. Exigirle `https` es exigir montar una
/// autoridad certificadora e instalar su raíz en el sistema para hablar con una
/// máquina que está a un metro, y el resultado de eso no es más seguridad: es
/// que la función no se usa.
///
/// Lo que **no** cambió, y es lo que esta prueba protege: hacia internet en
/// claro no se sale, y una dirección privada solo vale escrita como número. Un
/// nombre lo resuelve el DNS, y quien controle esa zona decidiría a dónde va el
/// texto del libro.
#[test]
fn el_texto_en_claro_se_admite_en_la_red_propia_y_nunca_hacia_internet() {
    for propia in [
        "http://localhost:8880/v1/audio/speech",
        "http://127.0.0.1:8880/x",
        "http://LocalHost/x",
        // El caso que motivó el cambio: una máquina virtual con IP fija.
        "http://192.168.100.10:8880/v1/audio/speech",
        "http://10.0.0.5/x",
        "http://172.16.3.4:8880/x",
        // Enlace local, que es lo que sale sin servidor de direcciones.
        "http://169.254.1.1/x",
        // IPv6: bucle, local única y enlace local.
        "http://[::1]/x",
        "http://[::1]:8880/x",
        "http://[fd00::1]:8880/x",
        "http://[fe80::1]/x",
    ] {
        assert!(
            net::host_de(propia).is_ok(),
            "se rechazó la red propia: {propia}"
        );
    }

    for fuera in [
        "http://ejemplo.org/x",
        // El anfitrión entero, no un prefijo: esto es un dominio de cualquiera.
        "http://localhost.ejemplo.org/x",
        // Direcciones públicas, por mucho que se parezcan a las privadas.
        "http://172.32.0.1/x",
        "http://11.0.0.1/x",
        "http://193.168.1.10/x",
        // Y por nombre no, aunque hoy resuelva a una dirección privada.
        "http://mi-servidor.example/x",
        "http://narrator-tts.local/x",
    ] {
        assert!(
            net::host_de(fuera).is_err(),
            "se aceptó en claro hacia fuera: {fuera}"
        );
    }
}

/// El anfitrión de una dirección IPv6 se lee entera, con o sin puerto.
///
/// Antes se partía por el último `:`, que en IPv6 es parte de la dirección:
/// `[::1]` sin puerto quedaba en `[:` y no coincidía con nada, así que la
/// excepción de claro hacia uno mismo no se aplicaba nunca por esa vía aunque
/// la lista la nombrase.
#[test]
fn una_direccion_ipv6_se_lee_entera() {
    assert_eq!(net::host_de("http://[::1]/x").expect("host"), "::1");
    assert_eq!(net::host_de("http://[::1]:8880/x").expect("host"), "::1");
    assert_eq!(
        net::host_de("https://[2001:db8::1]:443/x").expect("host"),
        "2001:db8::1"
    );
}

/// El interruptor viene apagado, y aun así no sale nada: la garantía de verdad
/// es el consentimiento, no el interruptor.
#[test]
fn de_fabrica_no_sale_nada_aunque_el_interruptor_este_apagado() {
    let db = base();
    let conn = db.connection().expect("conexión");

    assert!(
        !net::is_offline(&conn).expect("preferencia"),
        "el modo sin red viene apagado"
    );

    let (espia, salidas) = Espia::con_respuesta("contenido");
    let red = Network::new(Box::new(espia));

    assert!(matches!(
        red.get(&conn, "https://cualquiera.ejemplo/x", Purpose::Update),
        Err(NbError::ConsentRequired)
    ));
    assert!(salidas.lock().expect("registro").is_empty());
}

/// De fábrica no viene ningún permiso concedido: el usuario tiene que autorizar
/// cada anfitrión y cada propósito explícitamente.
#[test]
fn de_fabrica_no_hay_permisos_concedidos() {
    let db = base();
    let conn = db.connection().expect("conexión");

    let permisos = net::consents(&conn).expect("permisos");
    assert!(
        permisos.is_empty(),
        "de fábrica no puede venir ningún permiso de red concedido"
    );
}

/// Cada propósito dice qué envía, en lenguaje llano. Es lo que hay que enseñar
/// antes de pedir el consentimiento, y vive junto al propósito para que no se
/// quede viejo cuando el propósito cambie.
#[test]
fn cada_proposito_explica_que_envia() {
    for proposito in [
        Purpose::Catalog,
        Purpose::Metadata,
        Purpose::Lookup,
        Purpose::Update,
    ] {
        assert!(
            !proposito.what_is_sent().is_empty(),
            "{proposito:?} no explica qué envía"
        );
    }
}

/// **Regresión del 2026-09-16.** Todo propósito se puede leer desde el
/// puente y tiene su etiqueta en los tres idiomas.
///
/// Hasta ese día cuatro de los ocho no tenían etiqueta —`catalog-cleartext`,
/// `registry`, `speech` y `extension`—, y la sección de Red de Ajustes pintaba la
/// clave cruda: `proposito-speech`. Y `registry` no se podía leer desde el puente,
/// así que su permiso no se podía revocar. Los dos defectos venían de lo mismo:
/// listas escritas a mano en tres sitios, que se quedaron atrás cuando se añadía
/// un propósito. Ahora hay una sola, `Purpose::TODOS`, y esta prueba ata a ella lo
/// demás.
#[test]
fn todo_proposito_se_puede_leer_desde_el_puente_y_tiene_etiqueta() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/i18n/locales");

    /*
     * Las etiquetas viven en el grupo de Ajustes, que **ahora viaja
     * con esa vista** y no en el diccionario del arranque: eran once kilobytes
     * dentro de un paquete de 250. Los tres idiomas están en ese mismo archivo.
     *
     * Aquí se comprueba que **cada propósito tiene su etiqueta tres veces**, una
     * por bloque de idioma. Contar es más burdo que leer el objeto, y es lo que
     * se puede hacer desde Rust sin un intérprete de TypeScript; que las tres
     * digan cosas distintas lo comprueba `i18n.test.ts`, y que ninguna falte lo
     * exige además el tipo `Record<Idioma, typeof es>`.
     */
    let ajustes = std::fs::read_to_string(raiz.join("ajustes.ts")).expect("textos de ajustes");
    for proposito in Purpose::TODOS {
        let clave = format!("'proposito-{}':", proposito.as_str());
        assert_eq!(
            ajustes.matches(&clave).count(),
            3,
            "el propósito {} no tiene etiqueta en los tres idiomas",
            proposito.as_str()
        );
    }

    for proposito in Purpose::TODOS {
        assert_eq!(
            net::purpose_de(proposito.as_str()).expect("se lee desde el puente"),
            proposito
        );
        assert!(!proposito.what_is_sent().is_empty());
    }
}
