//! La primera extensión compilada contra el mundo WIT.
//!
//! Las nueve del banco intentan salirse del cajón de arena. Esta existe para lo
//! contrario: comprobar que **el camino honesto funciona**. Un aislamiento que
//! solo se prueba con módulos hostiles no dice si uno que se porta bien puede
//! hacer su trabajo.
//!
//! Es además la prueba de que el contrato sirve, escribiendo algo
//! contra él. Y ya sirvió para algo antes de correr: al escribir el invitado se
//! vio que un `activate` sin retorno obligaba a la extensión a pedir una
//! capacidad —la salida estándar— solo para contar cómo le había ido, así que
//! el mundo pasó a devolver un resultado tipado.

mod comun;

use std::sync::{Arc, Mutex};

use comun::{componente, modulo, OBJETIVO, OBJETIVO_COMPONENTE};
use nb_books_lib::db::Database;
use nb_books_lib::extensions::world::{activar, Activacion, Anfitrion};

/// La orden que la extensión de ejemplo declara en su manifiesto.
const ORDEN: &str = "usa-el-mundo.consultar";
use nb_books_lib::extensions::{Limites, Motor};
use nb_books_lib::net::{self, HttpTransport, Network, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

/// El identificador con el que sale el componente del banco.
const QUIEN: &str = "lat.nbdigitallabs.books.banco.usa-el-mundo";

/// El anfitrión al que sale, y el único que se le concede.
const DESTINO: &str = "datos.ejemplo.org";

/// Transporte que anota en lugar de salir.
struct Espia {
    salidas: Arc<Mutex<Vec<String>>>,
}

impl HttpTransport for Espia {
    fn get(&self, url: &str) -> nb_books_lib::error::NbResult<Response> {
        self.salidas.lock().expect("candado").push(url.to_owned());
        Ok(Response {
            status: 200,
            content_type: Some("application/json".to_owned()),
            location: None,
            body: b"{\"ok\":true}".to_vec(),
        })
    }
}

/// Todo lo que hace falta para activar el componente una vez.
struct Escenario {
    db: Database,
    red: Arc<Network>,
    salidas: Arc<Mutex<Vec<String>>>,
    motor: Motor,
    wasm: Vec<u8>,
}

/// Prepara el escenario, o indica que no hay objetivo con el que construirlo.
fn preparado() -> Option<Escenario> {
    let wasm = componente("usa_el_mundo")?;
    let salidas = Arc::new(Mutex::new(Vec::new()));
    Some(Escenario {
        db: Database::open_in_memory().expect("base"),
        red: Arc::new(Network::new(Box::new(Espia {
            salidas: Arc::clone(&salidas),
        }))),
        salidas,
        motor: Motor::nuevo().expect("motor"),
        wasm,
    })
}

#[test]
fn el_componente_se_instancia_y_sale_por_la_puerta_del_anfitrion() {
    let Some(Escenario {
        db,
        red,
        salidas,
        motor,
        wasm,
    }) = preparado()
    else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let conn = db.connection().expect("conexión");
    net::grant_from(
        &conn,
        &net::Origin::Extension(QUIEN.to_owned()),
        DESTINO,
        Purpose::Extension,
    )
    .expect("concede");

    let anfitrion = Anfitrion::para(db.clone(), Arc::clone(&red), QUIEN, Limites::default());
    let resultado = activar(&motor, &wasm, Limites::default(), anfitrion, ORDEN).expect("activa");

    match resultado {
        Activacion::Hecha(Some(dicho)) => assert!(
            dicho.contains("200"),
            "la extensión cuenta cómo le fue, y dijo: {dicho}"
        ),
        otro => panic!("tenía que responder y pasó otra cosa: {otro:?}"),
    }
    assert_eq!(salidas.lock().expect("candado").len(), 1);

    // Y su salida queda en el registro con **su** nombre, que es lo que
    // construye el registro de salidas: ver que algo salió y no saber qué lo mandó deja de ser
    // posible.
    let registro = net::log(&conn, 10).expect("registro");
    assert_eq!(registro[0].origin, QUIEN);
    assert_eq!(registro[0].host, DESTINO);
    assert_eq!(registro[0].purpose, "extension");
}

#[test]
fn sin_permiso_el_componente_recibe_denegado_y_no_llega_al_transporte() {
    let Some(Escenario {
        db,
        red,
        salidas,
        motor,
        wasm,
    }) = preparado()
    else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };

    let anfitrion = Anfitrion::para(db, Arc::clone(&red), QUIEN, Limites::default());
    let resultado = activar(&motor, &wasm, Limites::default(), anfitrion, ORDEN).expect("activa");

    // El motivo llega tipado, no como un fallo genérico: una extensión que
    // sabe que le falta permiso puede pedirlo, y una que solo sabe que algo
    // falló, no.
    assert!(
        matches!(resultado, Activacion::NoPudo(_)),
        "tenía que denegarse: {resultado:?}"
    );
    assert!(
        salidas.lock().expect("candado").is_empty(),
        "sin permiso no llega ni al transporte"
    );
}

#[test]
fn con_el_modo_sin_red_el_componente_recibe_su_propio_motivo() {
    let Some(Escenario {
        db,
        red,
        salidas,
        motor,
        wasm,
    }) = preparado()
    else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let conn = db.connection().expect("conexión");
    net::grant_from(
        &conn,
        &net::Origin::Extension(QUIEN.to_owned()),
        DESTINO,
        Purpose::Extension,
    )
    .expect("concede");
    settings::set(&conn, SettingKey::OfflineMode, &SettingValue::Flag(true)).expect("sin red");

    let anfitrion = Anfitrion::para(db.clone(), Arc::clone(&red), QUIEN, Limites::default());
    let resultado = activar(&motor, &wasm, Limites::default(), anfitrion, ORDEN).expect("activa");

    // Con permiso concedido y el interruptor puesto, el motivo tiene que ser
    // "no hay red" y no "no tienes permiso": son dos acciones distintas para
    // quien escribió la extensión, y confundirlas la haría pedir un permiso que
    // ya tiene.
    assert!(
        matches!(resultado, Activacion::NoPudo(_)),
        "tenía que no poder salir: {resultado:?}"
    );
    assert!(salidas.lock().expect("candado").is_empty());
}

#[test]
fn un_modulo_suelto_no_es_un_componente_del_mundo() {
    let Some(suelto) = modulo("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let db = Database::open_in_memory().expect("base");
    let red = Arc::new(Network::new(Box::new(Espia {
        salidas: Arc::new(Mutex::new(Vec::new())),
    })));
    let motor = Motor::nuevo().expect("motor");

    let anfitrion = Anfitrion::para(db, red, QUIEN, Limites::default());
    let resultado =
        activar(&motor, &suelto, Limites::default(), anfitrion, ORDEN).expect("no revienta");

    // Los dos caminos conviven, y no se confunden: un módulo de `wasip1` no
    // cumple el mundo, y el anfitrión lo dice en lugar de intentarlo.
    assert!(
        matches!(resultado, Activacion::Detenido(_)),
        "un módulo suelto no es un componente: {resultado:?}"
    );
}

#[test]
fn una_orden_que_el_componente_no_conoce_se_rechaza_en_lugar_de_hacer_algo() {
    let Some(Escenario {
        db,
        red,
        salidas,
        motor,
        wasm,
    }) = preparado()
    else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };

    let anfitrion = Anfitrion::para(db, Arc::clone(&red), QUIEN, Limites::default());
    let resultado = activar(
        &motor,
        &wasm,
        Limites::default(),
        anfitrion,
        "orden.que.no.existe",
    )
    .expect("activa");

    // El manifiesto y el código discreparían, y eso es un defecto de la
    // extensión. Decirlo es mejor que hacer algo por si acaso, que es lo que
    // haría un punto de entrada sin nombre.
    assert!(
        matches!(resultado, Activacion::NoPudo(_)),
        "una orden desconocida no se ejecuta: {resultado:?}"
    );
    assert!(salidas.lock().expect("candado").is_empty());
}
