//! Retirada de una extensión y aviso a quien la tenía.
//!
//! # Qué exige el criterio
//!
//! Que retirar una extensión de prueba **produzca el aviso en un cliente que la
//! tenía**. Aquí el cliente es de verdad —el almacén, la base y la puerta de
//! red del proyecto— y el registro es un servidor de una petición en esta misma
//! máquina. Lo que no se puede montar aquí es el registro **publicado**, y eso
//! sigue declarado como no verificable en la sección 3 del plan de la fase.
//!
//! # Por qué hay un servidor de verdad y no solo un transporte simulado
//!
//! Porque el transporte simulado no comprueba lo que más importa de esta
//! función después del aviso: que al preguntar **no salga de aquí nada sobre
//! lo que hay instalado**. Eso se ve en la petición que recibe el servidor, no
//! en la que se le pide al espía.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::extensions::store::Store;
use nb_books_lib::extensions::withdrawals::{
    afectadas, aplicar, comprobar, guardados, Lista, Withdrawal, WithdrawalReason,
};
use nb_books_lib::net::{self, HttpTransport, Network, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-retirada-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

fn paquete(en: &Path, id: &str, version: &str) -> PathBuf {
    let dir = en.join(format!("paquete-{}-{version}", id.replace('.', "-")));
    std::fs::create_dir_all(&dir).expect("paquete");
    let manifiesto = serde_json::json!({
        "id": id,
        "name": "Extensión de prueba",
        "version": version,
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0"
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    std::fs::write(dir.join("modulo.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    dir
}

fn lista(json: serde_json::Value) -> Lista {
    serde_json::from_value(json).expect("la lista se lee")
}

/// Un transporte que anota lo que se le pide y contesta lo mismo siempre.
struct Espia {
    pedidas: Arc<Mutex<Vec<String>>>,
    cuerpo: String,
}

impl HttpTransport for Espia {
    fn get(&self, url: &str) -> nb_books_lib::error::NbResult<Response> {
        self.pedidas
            .lock()
            .expect("registro del espía")
            .push(url.to_owned());
        Ok(Response {
            status: 200,
            content_type: Some("application/json".to_owned()),
            location: None,
            body: self.cuerpo.as_bytes().to_vec(),
        })
    }
}

fn espia(cuerpo: &serde_json::Value) -> (Network, Arc<Mutex<Vec<String>>>) {
    let pedidas = Arc::new(Mutex::new(Vec::new()));
    let red = Network::new(Box::new(Espia {
        pedidas: Arc::clone(&pedidas),
        cuerpo: cuerpo.to_string(),
    }));
    (red, pedidas)
}

// ---------------------------------------------------------------------------
// A quién alcanza una retirada
// ---------------------------------------------------------------------------

#[test]
fn una_retirada_alcanza_a_la_version_que_nombra_y_no_a_las_demas() {
    let raiz = temporal("versiones");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.uno", "1.0.0"))
        .expect("instalar");

    let instaladas = almacen.perfil().expect("perfil").extensions;
    let alcanza = lista(serde_json::json!({
        "generated": "2026-09-08T00:00:00Z",
        "withdrawn": [{ "id": "com.ejemplo.uno", "versions": ["1.0.0"], "reason": "breach" }]
    }));
    let no_alcanza = lista(serde_json::json!({
        "generated": "2026-09-08T00:00:00Z",
        "withdrawn": [{ "id": "com.ejemplo.uno", "versions": ["2.0.0"], "reason": "breach" }]
    }));

    assert_eq!(afectadas(&alcanza, &instaladas).len(), 1);
    assert!(
        afectadas(&no_alcanza, &instaladas).is_empty(),
        "retirar la 2.0.0 no dice nada de la 1.0.0, que es la que hay instalada"
    );
}

#[test]
fn el_comodin_alcanza_a_cualquier_version() {
    let raiz = temporal("comodin");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.uno", "7.3.1"))
        .expect("instalar");

    let instaladas = almacen.perfil().expect("perfil").extensions;
    let todas = lista(serde_json::json!({
        "withdrawn": [{ "id": "com.ejemplo.uno", "versions": ["*"], "reason": "malicious" }]
    }));

    let avisos = afectadas(&todas, &instaladas);
    assert_eq!(avisos.len(), 1);
    assert_eq!(
        avisos[0].version, "7.3.1",
        "el aviso dice la versión que hay"
    );
}

#[test]
fn solo_lo_malicioso_apaga() {
    let raiz = temporal("apaga");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.mala", "1.0.0"))
        .expect("instalar");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.vieja", "1.0.0"))
        .expect("instalar");

    let instaladas = almacen.perfil().expect("perfil").extensions;
    let dos = lista(serde_json::json!({
        "withdrawn": [
            { "id": "com.ejemplo.mala", "versions": ["*"], "reason": "malicious" },
            { "id": "com.ejemplo.vieja", "versions": ["*"], "reason": "abandoned" }
        ]
    }));

    let avisos = afectadas(&dos, &instaladas);
    let mala = avisos
        .iter()
        .find(|a| a.extension == "com.ejemplo.mala")
        .expect("la maliciosa");
    let vieja = avisos
        .iter()
        .find(|a| a.extension == "com.ejemplo.vieja")
        .expect("la abandonada");

    assert!(mala.disabled);
    assert!(
        !vieja.disabled,
        "apagar por todo enseñaría a reactivar sin leer, y entonces apagar dejaría de significar algo"
    );
}

#[test]
fn un_motivo_que_no_se_conoce_avisa_y_no_apaga() {
    // Un registro futuro con motivos nuevos no puede apagar media biblioteca de
    // extensiones sin que nadie lo haya decidido.
    let raiz = temporal("motivo-raro");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.uno", "1.0.0"))
        .expect("instalar");

    let instaladas = almacen.perfil().expect("perfil").extensions;
    let rara = lista(serde_json::json!({
        "withdrawn": [{ "id": "com.ejemplo.uno", "versions": ["*"], "reason": "lo-que-sea" }]
    }));

    let avisos = afectadas(&rara, &instaladas);
    assert_eq!(avisos.len(), 1);
    assert_eq!(avisos[0].reason, WithdrawalReason::Breach);
    assert!(!avisos[0].disabled);
}

// ---------------------------------------------------------------------------
// Lo que se guarda, y lo que se apaga
// ---------------------------------------------------------------------------

#[test]
fn el_aviso_sobrevive_a_cerrar_la_aplicacion() {
    // Sin guardarlo, al volver a abrir la extensión aparecería apagada y sin
    // explicación, que es la pérdida silenciosa que el producto existe para
    // impedir.
    let raiz = temporal("guardado");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.mala", "1.0.0"))
        .expect("instalar");

    let avisos = vec![Withdrawal {
        extension: "com.ejemplo.mala".to_owned(),
        version: "1.0.0".to_owned(),
        reason: WithdrawalReason::Malicious,
        detail: "Enviaba lo que leías".to_owned(),
        date: "2026-09-08".to_owned(),
        disabled: true,
    }];
    aplicar(&conn, &almacen, &avisos).expect("aplicar");

    let guardado = guardados(&conn).expect("leer");
    assert_eq!(guardado, avisos);

    let instalada = almacen.perfil().expect("perfil").extensions;
    assert!(
        !instalada[0].enabled,
        "y la extensión queda apagada, no desinstalada"
    );
    assert_eq!(instalada.len(), 1, "apagar no es desinstalar");
}

#[test]
fn una_extension_que_deja_de_estar_retirada_desaparece_del_aviso() {
    let raiz = temporal("reemplazo");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.uno", "1.0.0"))
        .expect("instalar");

    aplicar(
        &conn,
        &almacen,
        &[Withdrawal {
            extension: "com.ejemplo.uno".to_owned(),
            version: "1.0.0".to_owned(),
            reason: WithdrawalReason::Breach,
            detail: String::new(),
            date: String::new(),
            disabled: false,
        }],
    )
    .expect("aplicar");
    assert_eq!(guardados(&conn).expect("leer").len(), 1);

    aplicar(&conn, &almacen, &[]).expect("aplicar sin nada");
    assert!(
        guardados(&conn).expect("leer").is_empty(),
        "una comprobación reemplaza lo anterior entero"
    );
}

// ---------------------------------------------------------------------------
// La comprobación, con la puerta de red de verdad
// ---------------------------------------------------------------------------

#[test]
fn sin_registro_configurado_no_hay_a_quien_preguntar() {
    let raiz = temporal("sin-registro");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let (red, pedidas) = espia(&serde_json::json!({}));

    assert!(matches!(
        comprobar(&conn, &red, &almacen, ""),
        Err(NbError::ComponentUnavailable)
    ));
    assert!(
        pedidas.lock().expect("registro").is_empty(),
        "y no se intenta salir a ninguna parte"
    );
}

#[test]
fn sin_consentimiento_no_se_pregunta() {
    let raiz = temporal("sin-permiso");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let (red, _) = espia(&serde_json::json!({ "withdrawn": [] }));

    assert!(matches!(
        comprobar(&conn, &red, &almacen, "registro.ejemplo"),
        Err(NbError::ConsentRequired)
    ));
}

#[test]
fn al_preguntar_no_sale_nada_sobre_lo_que_hay_instalado() {
    // Preguntar por las extensiones instaladas sería más barato en bytes y le
    // contaría al registro qué tiene cada usuario. Se descarga la lista entera
    // y se compara aquí.
    let raiz = temporal("privacidad");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.secreta", "1.0.0"))
        .expect("instalar");
    net::grant(&conn, "registro.ejemplo", Purpose::Registry).expect("consentimiento");

    let (red, pedidas) = espia(&serde_json::json!({ "withdrawn": [] }));
    comprobar(&conn, &red, &almacen, "registro.ejemplo").expect("comprobar");

    let pedidas = pedidas.lock().expect("registro");
    assert_eq!(pedidas.len(), 1);
    assert_eq!(pedidas[0], "https://registro.ejemplo/retiradas.json");
    assert!(
        !pedidas[0].contains("secreta"),
        "la dirección no lleva ni el identificador de lo instalado"
    );
}

/// Un servidor que sirve la lista una vez y devuelve lo que le pidieron.
///
/// Medio centenar de líneas de biblioteca estándar en lugar de una dependencia,
/// y sirve para lo que ningún transporte simulado puede: ver la petición
/// completa que sale por el socket.
fn registro_de_una_peticion(cuerpo: String) -> (u16, std::sync::mpsc::Receiver<String>) {
    let escucha = std::net::TcpListener::bind("127.0.0.1:0").expect("puerto libre");
    let puerto = escucha.local_addr().expect("dirección").port();
    let (emisor, receptor) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let Ok((flujo, _)) = escucha.accept() else {
            return;
        };
        let mut lector = BufReader::new(flujo.try_clone().expect("clon"));
        let mut peticion = String::new();
        loop {
            let mut linea = String::new();
            if lector.read_line(&mut linea).unwrap_or(0) == 0 {
                break;
            }
            let acabo = linea.trim().is_empty();
            peticion.push_str(&linea);
            if acabo {
                break;
            }
        }
        let _ = emisor.send(peticion);

        let mut salida = flujo;
        let _ = write!(
            salida,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
            cuerpo.len()
        );
        let _ = salida.flush();
    });

    (puerto, receptor)
}

#[test]
fn retirar_una_extension_produce_el_aviso_en_un_cliente_que_la_tenia() {
    // El criterio de aceptación, con un servidor de verdad al otro lado.
    let raiz = temporal("criterio");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.mala", "1.0.0"))
        .expect("instalar");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.buena", "1.0.0"))
        .expect("instalar");

    let cuerpo = serde_json::json!({
        "generated": "2026-09-08T10:00:00Z",
        "withdrawn": [{
            "id": "com.ejemplo.mala",
            "versions": ["*"],
            "reason": "malicious",
            "detail": "Enviaba lo que leías a un tercero",
            "date": "2026-09-08"
        }]
    })
    .to_string();
    let (puerto, peticiones) = registro_de_una_peticion(cuerpo);

    // El consentimiento es por anfitrión, sin puerto: es lo que autoriza quien
    // lo concede, y dos puertos de la misma máquina no son dos destinos
    // distintos para quien decide.
    let anfitrion = format!("127.0.0.1:{puerto}");
    net::grant(&conn, "127.0.0.1", Purpose::Registry).expect("consentimiento");
    let red = Network::new(Box::new(nb_books_lib::net::UreqTransport::new()));

    let avisos =
        comprobar(&conn, &red, &almacen, &format!("http://{anfitrion}")).expect("comprobar");

    assert_eq!(avisos.len(), 1, "solo la retirada, no todas las instaladas");
    assert_eq!(avisos[0].extension, "com.ejemplo.mala");
    assert_eq!(avisos[0].reason, WithdrawalReason::Malicious);
    assert!(
        avisos[0].detail.contains("un tercero"),
        "y con lo que cuenta el registro"
    );
    assert!(avisos[0].disabled);

    // La retirada llega hasta lo que estaba corriendo: la maliciosa queda
    // apagada y la otra sigue encendida.
    let perfil = almacen.perfil().expect("perfil");
    let mala = perfil
        .extensions
        .iter()
        .find(|e| e.id == "com.ejemplo.mala")
        .expect("la maliciosa");
    let buena = perfil
        .extensions
        .iter()
        .find(|e| e.id == "com.ejemplo.buena")
        .expect("la otra");
    assert!(!mala.enabled);
    assert!(buena.enabled, "retirar una no toca a las demás");

    // Y el aviso sigue ahí sin volver a preguntar.
    assert_eq!(guardados(&conn).expect("leer").len(), 1);

    // Lo que salió por el socket: una petición y nada de lo instalado dentro.
    let peticion = peticiones
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("el servidor recibió la petición");
    assert!(peticion.starts_with("GET /retiradas.json "), "{peticion}");
    assert!(
        !peticion.contains("com.ejemplo"),
        "la petición no dice qué hay instalado: {peticion}"
    );
}

#[test]
fn una_lista_que_no_se_entiende_no_borra_lo_que_ya_se_sabia() {
    // Un registro que devuelve basura no puede dejar al usuario sin el aviso
    // que ya tenía: eso convertiría un fallo del servidor en una forma de
    // silenciar avisos.
    let raiz = temporal("basura");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "com.ejemplo.mala", "1.0.0"))
        .expect("instalar");
    aplicar(
        &conn,
        &almacen,
        &[Withdrawal {
            extension: "com.ejemplo.mala".to_owned(),
            version: "1.0.0".to_owned(),
            reason: WithdrawalReason::Malicious,
            detail: String::new(),
            date: String::new(),
            disabled: true,
        }],
    )
    .expect("aplicar");

    net::grant(&conn, "registro.ejemplo", Purpose::Registry).expect("consentimiento");
    let red = Network::new(Box::new(Espia {
        pedidas: Arc::new(Mutex::new(Vec::new())),
        cuerpo: "esto no es la lista".to_owned(),
    }));

    assert!(matches!(
        comprobar(&conn, &red, &almacen, "registro.ejemplo"),
        Err(NbError::DocumentCorrupted)
    ));
    assert_eq!(
        guardados(&conn).expect("leer").len(),
        1,
        "lo que ya se sabía sigue ahí"
    );
}

#[test]
fn con_el_modo_sin_red_no_se_pregunta_al_registro() {
    let raiz = temporal("sin-red");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    net::grant(&conn, "registro.ejemplo", Purpose::Registry).expect("consentimiento");
    settings::set(&conn, SettingKey::OfflineMode, &SettingValue::Flag(true)).expect("interruptor");

    let (red, pedidas) = espia(&serde_json::json!({ "withdrawn": [] }));

    assert!(matches!(
        comprobar(&conn, &red, &almacen, "registro.ejemplo"),
        Err(NbError::NetworkDisabled)
    ));
    assert!(pedidas.lock().expect("registro").is_empty());
}
