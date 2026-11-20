//! Criterio de aceptación de los proveedores externos.
//!
//! "El primer envío a un proveedor nuevo muestra qué se va a enviar y exige
//! consentimiento; el envío queda registrado localmente y es auditable por el
//! usuario."
//!
//! Lo que estas pruebas comprueban no es que un modelo responda bien, que eso lo
//! hace un servicio de terceros, sino tres cosas que dependen solo de nosotros:
//! **que no sale nada sin que alguien haya visto qué sale y lo haya autorizado**,
//! **que lo que sale es exactamente lo que se enseñó y lo que queda registrado**, y
//! **que un proveedor externo nunca se usa sin que lo nombren**.
//!
//! Contrato: `docs/architecture/ai/privacy.md`.

use std::sync::{Arc, Mutex};

use nb_books_lib::ai::externo::{self, Configuracion, ExternalProvider, Protocolo};
use nb_books_lib::ai::presupuesto::Presupuesto;
use nb_books_lib::ai::subsistema::{Prueba, Subsistema};
use nb_books_lib::ai::{envios, AiService, Availability, Capability, Choice, Request};
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::events::EventBus;
use nb_books_lib::monitor::Monitor;
use nb_books_lib::net::{self, HttpTransport, Network, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

/// Lo que salió: dirección, cuerpo y cabeceras.
#[derive(Debug, Clone)]
struct Salida {
    url: String,
    cuerpo: String,
    cabeceras: Vec<(String, String)>,
}

type Salidas = Arc<Mutex<Vec<Salida>>>;

/// Transporte que apunta lo que se le manda y contesta lo que se le diga.
struct Espia {
    salidas: Salidas,
    estado: u16,
    cuerpo: Vec<u8>,
}

impl HttpTransport for Espia {
    fn get(&self, _url: &str) -> NbResult<Response> {
        Err(NbError::NetworkFailure)
    }

    fn post_json(&self, url: &str, cuerpo: &str, credencial: Option<&str>) -> NbResult<Response> {
        self.post_json_con_cabeceras(url, cuerpo, credencial, &[])
    }

    fn post_json_con_cabeceras(
        &self,
        url: &str,
        cuerpo: &str,
        _credencial: Option<&str>,
        cabeceras: &[(&str, &str)],
    ) -> NbResult<Response> {
        self.salidas.lock().expect("salidas").push(Salida {
            url: url.to_owned(),
            cuerpo: cuerpo.to_owned(),
            cabeceras: cabeceras
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
        });
        Ok(Response {
            status: self.estado,
            content_type: Some("application/json".to_owned()),
            body: self.cuerpo.clone(),
            location: None,
        })
    }
}

const RESPUESTA_DE_ANTHROPIC: &[u8] = br#"{"content":[{"type":"text","text":"listo"}]}"#;

struct Montaje {
    db: Database,
    red: Arc<Network>,
    ia: Subsistema,
    salidas: Salidas,
    _modelos: Temporal,
}

struct Temporal(std::path::PathBuf);

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn montar(nombre: &str, estado: u16, cuerpo: &[u8]) -> Montaje {
    let modelos = Temporal(
        nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-externos-{nombre}-{}", std::process::id())),
    );
    let salidas: Salidas = Arc::new(Mutex::new(Vec::new()));
    let red = Arc::new(Network::new(Box::new(Espia {
        salidas: Arc::clone(&salidas),
        estado,
        cuerpo: cuerpo.to_vec(),
    })));
    let db = Database::open_in_memory().expect("base");
    let ia = Subsistema::arrancar(
        &modelos.0,
        Monitor::new(EventBus::new()),
        Presupuesto::de_fabrica(8),
    );
    Montaje {
        db,
        red,
        ia,
        salidas,
        _modelos: modelos,
    }
}

fn configurar(m: &Montaje, protocolo: &str, host: &str, modelo: &str) {
    let conn = m.db.connection().expect("conexión");
    for (clave, valor) in [
        (SettingKey::AiProvider, protocolo),
        (SettingKey::AiHost, host),
        (SettingKey::AiModel, modelo),
    ] {
        settings::set(&conn, clave, &SettingValue::Text(valor.to_owned())).expect("preferencia");
    }
    drop(conn);
    m.ia.configurar_externo(&m.db, &m.red).expect("configurar");
}

fn envios(m: &Montaje) -> Vec<envios::EnvioRegistrado> {
    envios::listar(&m.db.connection().expect("conexión"), 100).expect("registro")
}

/// **La primera mitad del criterio.** El primer envío a un proveedor nuevo no
/// envía nada: devuelve el mensaje entero que enviaría, con a dónde y qué sale.
/// Nada ha llegado al transporte y nada se ha registrado como enviado.
#[test]
fn el_primer_envio_ensena_lo_que_saldria_y_no_envia_nada() {
    let m = montar("primero", 200, RESPUESTA_DE_ANTHROPIC);
    configurar(&m, "anthropic", "api.anthropic.com", "claude-un-modelo");

    let prueba = m.ia.probar(&m.db).expect("probar");

    let Prueba::NeedsConsent {
        host,
        what_is_sent,
        body,
    } = prueba
    else {
        panic!("sin consentimiento se esperaba enseñar qué saldría, y fue {prueba:?}");
    };
    assert_eq!(host, "api.anthropic.com");
    assert_eq!(what_is_sent, Purpose::Ai.what_is_sent());
    assert!(body.contains("claude-un-modelo"));
    assert!(body.contains("prueba de conexión"));

    assert!(
        m.salidas.lock().expect("salidas").is_empty(),
        "salió algo antes de autorizarlo"
    );
    assert!(envios(&m).is_empty());
}

/// **La segunda mitad.** Autorizado, sale **exactamente** lo que se enseñó, con las
/// cabeceras del protocolo, y queda registrado tal cual salió: lo que el usuario
/// audita es lo que recibió el servicio, byte a byte.
#[test]
fn autorizado_sale_lo_que_se_enseno_y_queda_registrado_tal_cual() {
    let m = montar("autorizado", 200, RESPUESTA_DE_ANTHROPIC);
    configurar(
        &m,
        "anthropic",
        "https://api.anthropic.com/v1",
        "claude-un-modelo",
    );

    let Prueba::NeedsConsent { body: ensenado, .. } = m.ia.probar(&m.db).expect("probar") else {
        panic!("se esperaba enseñar antes de enviar");
    };
    m.ia.autorizar_externo(&m.db).expect("autorizar");

    let prueba = m.ia.probar(&m.db).expect("probar autorizado");
    assert_eq!(
        prueba,
        Prueba::Answered {
            host: "api.anthropic.com".to_owned(),
            model: "claude-un-modelo".to_owned(),
            text: "listo".to_owned(),
        }
    );

    let salidas = m.salidas.lock().expect("salidas").clone();
    assert_eq!(salidas.len(), 1);
    let salida = &salidas[0];
    assert_eq!(salida.url, "https://api.anthropic.com/v1/messages");
    assert!(salida
        .cabeceras
        .contains(&("anthropic-version".to_owned(), "2023-06-01".to_owned())));

    // Lo enseñado es lo enviado: el mismo JSON, solo que legible.
    let ensenado: serde_json::Value = serde_json::from_str(&ensenado).expect("JSON enseñado");
    let enviado: serde_json::Value = serde_json::from_str(&salida.cuerpo).expect("JSON enviado");
    assert_eq!(ensenado, enviado, "se enseñó una cosa y se envió otra");

    let registro = envios(&m);
    assert_eq!(registro.len(), 1);
    assert_eq!(
        registro[0].body, salida.cuerpo,
        "lo registrado no es lo que salió"
    );
    assert_eq!(registro[0].host, "api.anthropic.com");
    assert_eq!(registro[0].protocol, "anthropic");
    assert_eq!(registro[0].status, Some(200));
    assert!(!registro[0].body.to_ascii_lowercase().contains("bearer"));
}

/// Retirar el permiso en la sección de Red vuelve a enseñar antes de enviar: el
/// consentimiento es el de la puerta de red, no una marca aparte que se quede
/// puesta.
#[test]
fn retirar_el_permiso_vuelve_a_ensenar_antes_de_enviar() {
    let m = montar("retirar", 200, RESPUESTA_DE_ANTHROPIC);
    configurar(&m, "anthropic", "api.anthropic.com", "claude-un-modelo");
    m.ia.autorizar_externo(&m.db).expect("autorizar");
    assert!(matches!(m.ia.probar(&m.db), Ok(Prueba::Answered { .. })));

    net::revoke(
        &m.db.connection().expect("conexión"),
        "api.anthropic.com",
        Purpose::Ai,
    )
    .expect("retirar");

    assert!(matches!(
        m.ia.probar(&m.db),
        Ok(Prueba::NeedsConsent { .. })
    ));
    assert_eq!(
        m.salidas.lock().expect("salidas").len(),
        1,
        "salió sin permiso"
    );
}

/// Con el modo sin red puesto no sale nada, aunque esté autorizado, y no se anota
/// ningún envío: no hay contenido que auditar.
#[test]
fn el_modo_sin_red_no_deja_salir_ni_anota() {
    let m = montar("sin-red", 200, RESPUESTA_DE_ANTHROPIC);
    configurar(&m, "openai", "api.openai.com", "gpt-un-modelo");
    m.ia.autorizar_externo(&m.db).expect("autorizar");
    settings::set(
        &m.db.connection().expect("conexión"),
        SettingKey::OfflineMode,
        &SettingValue::Flag(true),
    )
    .expect("sin red");

    assert_eq!(m.ia.probar(&m.db), Err(NbError::NetworkDisabled));
    assert!(m.salidas.lock().expect("salidas").is_empty());
    assert!(envios(&m).is_empty());
}

/// Un rechazo del servicio —una clave que no vale— se dice con su código, y el
/// envío queda registrado: salió, aunque no lo aceptaran.
#[test]
fn un_rechazo_del_servicio_se_dice_y_queda_registrado() {
    let m = montar(
        "rechazo",
        401,
        br#"{"error":{"type":"authentication_error","message":"x"}}"#,
    );
    configurar(&m, "openai", "api.openai.com/v1", "gpt-un-modelo");
    m.ia.autorizar_externo(&m.db).expect("autorizar");

    assert_eq!(
        m.ia.probar(&m.db),
        Ok(Prueba::Rejected {
            host: "api.openai.com".to_owned(),
            status: 401
        })
    );
    let salidas = m.salidas.lock().expect("salidas").clone();
    assert_eq!(salidas[0].url, "https://api.openai.com/v1/chat/completions");
    assert!(salidas[0].cabeceras.is_empty());
    let registro = envios(&m);
    assert_eq!(registro.len(), 1);
    assert_eq!(registro[0].status, Some(401));
}

/// Una respuesta que no es lo que el protocolo dice no se toma por una, **y no
/// tumba nada**: es un error con su código, y el envío queda registrado.
#[test]
fn una_respuesta_rota_es_un_error_y_no_una_respuesta() {
    let m = montar("rota", 200, b"<html>no es JSON</html>");
    configurar(&m, "anthropic", "api.anthropic.com", "claude-un-modelo");
    m.ia.autorizar_externo(&m.db).expect("autorizar");

    assert_eq!(m.ia.probar(&m.db), Err(NbError::NetworkFailure));
    assert_eq!(envios(&m).len(), 1);
}

/// Borrar la dirección en Ajustes deja de permitir enviar **en ese momento**, y
/// las funciones vuelven a pedir a lo local.
#[test]
fn quitar_el_proveedor_deja_de_enviar_sin_reiniciar() {
    let m = montar("quitar", 200, RESPUESTA_DE_ANTHROPIC);
    let nombrado = Choice::Provider(externo::ID.to_owned());
    configurar(&m, "anthropic", "api.anthropic.com", "claude-un-modelo");
    assert_eq!(m.ia.eleccion(), nombrado);
    assert!(m
        .ia
        .disponibilidad(Capability::Summary, &nombrado)
        .is_ready());

    configurar(&m, "anthropic", "", "claude-un-modelo");

    // Del servicio, no solo de Ajustes: nombrarlo ya no lleva a ninguna parte.
    assert!(!m
        .ia
        .disponibilidad(Capability::Summary, &nombrado)
        .is_ready());

    assert_eq!(m.ia.probar(&m.db), Ok(Prueba::NotConfigured));
    assert_eq!(m.ia.eleccion(), Choice::Auto);
    assert!(matches!(
        m.ia.autorizar_externo(&m.db),
        Err(NbError::PathNotAllowed)
    ));
}

/// **Local primero, estructural.** Un proveedor externo registrado no atiende el
/// camino automático —salir del dispositivo exige nombrarlo— y no genera
/// embeddings ni nombrándolo: los vectores son siempre locales.
#[test]
fn un_proveedor_externo_nunca_atiende_lo_automatico_ni_los_embeddings() {
    let m = montar("local-primero", 200, RESPUESTA_DE_ANTHROPIC);
    let proveedor = ExternalProvider::nuevo(
        Configuracion {
            protocolo: Protocolo::Anthropic,
            host: "api.anthropic.com".to_owned(),
            modelo: "claude-un-modelo".to_owned(),
        },
        m.db.clone(),
        Arc::clone(&m.red),
    )
    .expect("proveedor");
    let mut servicio = AiService::new();
    servicio.register(Box::new(proveedor));

    assert!(!servicio
        .availability(Capability::Summary, &Choice::Auto)
        .is_ready());
    let nombrado = Choice::Provider(externo::ID.to_owned());
    assert!(matches!(
        servicio.availability(Capability::Summary, &nombrado),
        Availability::Ready(_)
    ));
    assert!(servicio.embed(&["hola".to_owned()], &nombrado).is_err());

    // Y nombrarlo sin permiso tampoco saca nada: la puerta lo impide.
    let peticion = Request {
        capability: Capability::Summary,
        instruction: "Resume.".to_owned(),
        passages: Vec::new(),
        task: String::new(),
        language: None,
    };
    assert_eq!(
        servicio.generate(&peticion, &nombrado),
        Err(NbError::ConsentRequired)
    );
    assert!(m.salidas.lock().expect("salidas").is_empty());
}
