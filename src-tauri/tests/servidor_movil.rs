//! El servidor para NB Books Mobile, de punta a punta: estas pruebas hacen de
//! teléfono contra el servidor de verdad, con TLS y por un puerto real.
//!
//! Contratos: `docs/architecture/mobile-contract.md` y ADR-016.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::movil::{Opciones, Puente, ServidorMovil};
use nb_books_lib::paths::AppPaths;
use serde_json::{json, Value};
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::crypto::ring::default_provider;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned,
};

/// El teléfono fija la huella del certificado; estas pruebas no comprueban la
/// confianza, sino lo que el servidor contesta, así que aceptan cualquiera.
#[derive(Debug)]
struct SinComprobar;

impl ServerCertVerifier for SinComprobar {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, tokio_rustls::rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &CertificateDer<'_>,
        _: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

struct Respuesta {
    estado: u16,
    cabeceras: String,
    cuerpo: Vec<u8>,
}

impl Respuesta {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.cuerpo).expect("cuerpo JSON")
    }
}

/// Una petición HTTP/1.1 por TLS, a mano: es lo único que hace falta para
/// hablar con el servidor como lo haría el teléfono.
fn pedir(
    puerto: u16,
    metodo: &str,
    ruta: &str,
    token: Option<&str>,
    cuerpo: Option<&Value>,
    extra: &str,
) -> Respuesta {
    let config = ClientConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .expect("versiones")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SinComprobar))
        .with_no_client_auth();
    let conexion = ClientConnection::new(
        Arc::new(config),
        ServerName::try_from("nbbooks.local").expect("nombre"),
    )
    .expect("cliente TLS");
    let tcp = TcpStream::connect(("127.0.0.1", puerto)).expect("conectar");
    let mut tls = StreamOwned::new(conexion, tcp);
    let cuerpo = cuerpo.map(|c| c.to_string()).unwrap_or_default();
    let mut peticion =
        format!("{metodo} {ruta} HTTP/1.1\r\nHost: nbbooks.local\r\nConnection: close\r\n{extra}");
    if let Some(t) = token {
        peticion.push_str(&format!("Authorization: Bearer {t}\r\n"));
    }
    if !cuerpo.is_empty() {
        peticion.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            cuerpo.len()
        ));
    }
    peticion.push_str("\r\n");
    peticion.push_str(&cuerpo);
    tls.write_all(peticion.as_bytes()).expect("enviar");
    let mut todo = Vec::new();
    let _ = tls.read_to_end(&mut todo);
    let fin = todo
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("cabeceras")
        + 4;
    let cabeceras = String::from_utf8_lossy(&todo[..fin]).to_string();
    let estado = cabeceras[9..12].parse().expect("estado");
    Respuesta {
        estado,
        cabeceras,
        cuerpo: todo[fin..].to_vec(),
    }
}

struct Banco {
    _dir: PathBuf,
    db: Database,
    servidor: ServidorMovil,
    puerto: u16,
    edicion: String,
    huella: String,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn banco(nombre: &str) -> Banco {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-movil-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let scope = dir.join("origen");
    let library = dir.join("biblioteca");
    std::fs::create_dir_all(&scope).expect("ámbito");
    std::fs::create_dir_all(&library).expect("biblioteca");
    let db = Database::open_in_memory().expect("base");
    let fuente = scope.join("libro.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &fuente).expect("copia");
    let bus = EventBus::new();
    let resultado = import_document(
        &mut db.connection().expect("conexión"),
        &ImportContext {
            scope: &scope,
            library: &library,
            bus: &bus,
        },
        &ImportRequest {
            source: fuente,
            storage_mode: StorageMode::Referenced,
            root: None,
        },
    )
    .expect("importar");
    let ImportOutcome::Imported(importado) = resultado else {
        panic!("tenía que entrar como nuevo");
    };
    let rutas = AppPaths {
        data: dir.join("datos"),
        config: dir.join("config"),
        cache: dir.join("cache"),
        logs: dir.join("logs"),
    };
    let servidor = ServidorMovil::nuevo(
        db.clone(),
        rutas,
        Arc::new(Puente::nuevo()),
        Opciones {
            host: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            mdns: false,
            puertos: 19_831..=19_899,
        },
        || {},
    );
    servidor.encender().expect("encender");
    let puerto = servidor.direccion().expect("dirección").port();
    Banco {
        _dir: dir,
        db,
        servidor,
        puerto,
        edicion: importado.edition.id.clone(),
        huella: importado.file.content_hash.as_str().to_owned(),
    }
}

fn emparejado(b: &Banco) -> String {
    b.servidor.emparejar().expect("emparejar");
    let token = b.servidor.token_vigente().expect("token");
    let r = pedir(
        b.puerto,
        "POST",
        "/api/v1/pair",
        None,
        Some(
            &json!({"token": token, "nombre_dispositivo": "Teléfono de prueba", "plataforma": "android"}),
        ),
        "",
    );
    assert_eq!(r.estado, 200);
    r.json()["device_token"]
        .as_str()
        .expect("device_token")
        .to_owned()
}

#[test]
fn sin_emparejar_solo_responde_ping_y_el_qr_es_de_un_solo_uso() {
    let b = banco("emparejar");
    assert_eq!(
        pedir(b.puerto, "GET", "/api/v1/ping", None, None, "").estado,
        200
    );
    assert_eq!(
        pedir(b.puerto, "GET", "/api/v1/manifest", None, None, "").estado,
        401
    );

    b.servidor.emparejar().expect("emparejar");
    let token = b.servidor.token_vigente().expect("token");
    let par = json!({"token": token, "nombre_dispositivo": "Uno", "plataforma": "android"});
    assert_eq!(
        pedir(b.puerto, "POST", "/api/v1/pair", None, Some(&par), "").estado,
        200
    );
    assert_eq!(
        pedir(b.puerto, "POST", "/api/v1/pair", None, Some(&par), "").estado,
        401,
        "el mismo QR no empareja dos teléfonos"
    );
}

#[test]
fn un_telefono_recibe_lo_que_elige_y_nada_mas() {
    let b = banco("seleccion");
    let token = emparejado(&b);
    let ruta = format!("/api/v1/archivo/{}", b.huella);
    assert_eq!(
        pedir(b.puerto, "GET", &ruta, Some(&token), None, "").estado,
        404,
        "sin elegirlo no se sirve, aunque adivine la huella"
    );

    let r = pedir(
        b.puerto,
        "POST",
        "/api/v1/seleccion",
        Some(&token),
        Some(&json!({"ediciones": [b.edicion]})),
        "",
    );
    assert_eq!(r.json()["reiniciar"], true);

    let m = pedir(
        b.puerto,
        "GET",
        "/api/v1/manifest?since=0",
        Some(&token),
        None,
        "",
    )
    .json();
    assert!(m["cambios"]
        .as_array()
        .expect("cambios")
        .iter()
        .any(|c| c["entidad"] == "edicion" && c["clave"] == b.edicion.as_str()));

    let entero = pedir(b.puerto, "GET", &ruta, Some(&token), None, "");
    assert_eq!(entero.estado, 200);
    let original = std::fs::read(fixtures().join("minimo-valido.epub")).expect("original");
    assert_eq!(entero.cuerpo, original);

    let tramo = pedir(
        b.puerto,
        "GET",
        &ruta,
        Some(&token),
        None,
        "Range: bytes=0-9\r\n",
    );
    assert_eq!(tramo.estado, 206, "la descarga se reanuda por tramos");
    assert_eq!(tramo.cuerpo, original[..10]);
    assert!(tramo
        .cabeceras
        .to_lowercase()
        .contains("content-range: bytes 0-9/"));
}

#[test]
fn lo_que_sube_el_telefono_entra_y_revocarlo_cierra_la_puerta() {
    let b = banco("revocar");
    let token = emparejado(&b);
    pedir(
        b.puerto,
        "POST",
        "/api/v1/seleccion",
        Some(&token),
        Some(&json!({"todo": true})),
        "",
    );
    let envio = json!({"anotaciones": [{
        "id": "0123456789abcdef0123456789abcdef", "edicion": b.edicion, "tipo": "bookmark",
        "locator": {"href": "c1.xhtml", "mediaType": "application/xhtml+xml",
                    "locations": {"progression": 0.5}},
        "creada_en": "2026-01-01T10:00:00.000Z"}]});
    let r = pedir(
        b.puerto,
        "POST",
        "/api/v1/cambios",
        Some(&token),
        Some(&envio),
        "",
    );
    assert_eq!(r.estado, 200);
    assert_eq!(r.json()["resultado"]["rechazados"], json!([]));
    let n: i64 =
        b.db.connection()
            .expect("conexión")
            .query_row("SELECT count(*) FROM annotations", [], |f| f.get(0))
            .expect("cuenta");
    assert_eq!(n, 1);

    let id = b.servidor.telefonos().expect("teléfonos")[0].id.clone();
    b.servidor.revocar(&id).expect("revocar");
    assert_eq!(
        pedir(b.puerto, "GET", "/api/v1/manifest", Some(&token), None, "").estado,
        401
    );
}

#[test]
fn la_busqueda_en_directo_es_la_del_pc_y_dice_si_el_telefono_tiene_el_libro() {
    let b = banco("buscar");
    let mut registro = nb_books_lib::reader::ProviderRegistry::new();
    registro.register(Box::new(
        nb_books_lib::reader::epub::EpubFactory::solo_texto(),
    ));
    let documento = registro
        .open(
            "application/epub+zip",
            &fixtures().join("minimo-valido.epub"),
        )
        .expect("documento");
    nb_books_lib::search::index_edition(
        &mut b.db.connection().expect("conexión"),
        &b.edicion,
        documento.as_ref(),
    )
    .expect("indexar");
    let token = emparejado(&b);

    let r = pedir(
        b.puerto,
        "GET",
        "/api/v1/buscar?q=prueba",
        Some(&token),
        None,
        "",
    )
    .json();
    let resultados = r["resultados"].as_array().expect("resultados");
    assert_eq!(resultados.len(), 1, "el teléfono busca en lo que no tiene");
    assert_eq!(resultados[0]["edicion"], b.edicion.as_str());
    assert_eq!(resultados[0]["seleccionada"], false);

    pedir(
        b.puerto,
        "POST",
        "/api/v1/seleccion",
        Some(&token),
        Some(&json!({"ediciones": [b.edicion]})),
        "",
    );
    let r = pedir(
        b.puerto,
        "GET",
        "/api/v1/buscar?q=prueba",
        Some(&token),
        None,
        "",
    )
    .json();
    assert_eq!(r["resultados"][0]["seleccionada"], true);
}

/// Quita de una respuesta lo que cambia de una ejecución a otra: identificadores,
/// huellas, fechas y versiones del registro. Lo que queda es la forma del
/// contrato y los valores que no dependen del azar.
fn sin_azar(valor: &mut Value) {
    match valor {
        Value::Object(campos) => {
            for (clave, v) in campos.iter_mut() {
                if matches!(
                    clave.as_str(),
                    "version" | "version_actual" | "siguiente" | "tamano" | "bytes"
                ) && v.is_number()
                {
                    *v = json!(0);
                } else {
                    sin_azar(v);
                }
            }
        }
        Value::Array(lista) => lista.iter_mut().for_each(sin_azar),
        Value::String(texto) => {
            let hex = texto.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
            if hex && texto.len() >= 32 {
                *texto = "<id>".to_owned();
            } else if texto.len() >= 20
                && texto.as_bytes().get(4) == Some(&b'-')
                && texto.contains('T')
            {
                *texto = "<fecha>".to_owned();
            }
        }
        _ => {}
    }
}

/// Las respuestas grabadas: lo que NB Books Mobile reproduce en sus pruebas.
///
/// Un cambio del servidor que altere una respuesta hace fallar esta prueba, y
/// eso es lo que se quiere: el contrato solo admite cambios aditivos, y quien
/// cambie una forma tiene que verlo y regrabar a propósito con `NB_GRABAR=1`,
/// llevando el mismo cambio al teléfono.
#[test]
fn las_respuestas_del_contrato_son_las_grabadas() {
    let b = banco("contrato");
    let token = emparejado(&b);
    pedir(
        b.puerto,
        "POST",
        "/api/v1/seleccion",
        Some(&token),
        Some(&json!({"todo": true})),
        "",
    );
    let grabadas = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/contrato-movil");
    let regrabar = std::env::var_os("NB_GRABAR").is_some();
    for (nombre, ruta, autenticada) in [
        ("ping", "/api/v1/ping", false),
        ("seleccion", "/api/v1/seleccion", true),
        ("estanterias", "/api/v1/estanterias", true),
        ("manifest", "/api/v1/manifest?since=0", true),
        ("catalogo", "/api/v1/catalogo", true),
        ("buscar-sin-indice", "/api/v1/buscar?q=prueba", true),
    ] {
        let r = pedir(
            b.puerto,
            "GET",
            ruta,
            autenticada.then_some(token.as_str()),
            None,
            "",
        );
        assert_eq!(r.estado, 200, "{ruta}");
        let mut cuerpo = r.json();
        sin_azar(&mut cuerpo);
        let archivo = grabadas.join(format!("{nombre}.json"));
        let texto = serde_json::to_string_pretty(&cuerpo).expect("json") + "\n";
        if regrabar {
            std::fs::create_dir_all(&grabadas).expect("carpeta");
            std::fs::write(&archivo, &texto).expect("grabar");
        } else {
            let esperado = std::fs::read_to_string(&archivo)
                .unwrap_or_else(|_| panic!("falta {}: grábalo con NB_GRABAR=1", archivo.display()));
            assert_eq!(texto, esperado, "{ruta} ya no responde lo grabado");
        }
    }
}

#[test]
fn leer_en_el_pc_entrega_las_ordenes_validas_y_devuelve_el_estado() {
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-movil-control-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let db = Database::open_in_memory().expect("base");
    let puente = Arc::new(Puente::nuevo());
    let mut ordenes = puente.ordenes();
    let servidor = ServidorMovil::nuevo(
        db,
        AppPaths {
            data: dir.join("datos"),
            config: dir.join("config"),
            cache: dir.join("cache"),
            logs: dir.join("logs"),
        },
        Arc::clone(&puente),
        Opciones {
            host: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            mdns: false,
            puertos: 19_831..=19_899,
        },
        || {},
    );
    servidor.encender().expect("encender");
    let puerto = servidor.direccion().expect("dirección").port();
    servidor.emparejar().expect("emparejar");
    let par = json!({"token": servidor.token_vigente().expect("token"),
                     "nombre_dispositivo": "Mando", "plataforma": "android"});
    let token = pedir(puerto, "POST", "/api/v1/pair", None, Some(&par), "").json()["device_token"]
        .as_str()
        .expect("token")
        .to_owned();
    puente.publicar(json!({"edicion": "e1", "progresion": 0.25}));

    let config = ClientConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .expect("versiones")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SinComprobar))
        .with_no_client_auth();
    let tls = StreamOwned::new(
        ClientConnection::new(
            Arc::new(config),
            ServerName::try_from("nbbooks.local").expect("nombre"),
        )
        .expect("cliente TLS"),
        TcpStream::connect(("127.0.0.1", puerto)).expect("conectar"),
    );
    let mut peticion = format!("wss://nbbooks.local:{puerto}/api/v1/control")
        .into_client_request()
        .expect("petición");
    peticion.headers_mut().insert(
        "Authorization",
        format!("Bearer {token}").parse().expect("cabecera"),
    );
    let (mut ws, _) =
        tokio_tungstenite::tungstenite::client::client(peticion, tls).expect("websocket");

    let leer = |ws: &mut tokio_tungstenite::tungstenite::WebSocket<_>| -> Value {
        match ws.read().expect("mensaje") {
            Message::Text(t) => serde_json::from_str(&t).expect("json"),
            otro => panic!("se esperaba texto: {otro:?}"),
        }
    };
    let primero = leer(&mut ws);
    assert_eq!(primero["tipo"], "estado");
    assert_eq!(
        primero["estado"]["progresion"], 0.25,
        "al conectar recibe dónde está el lector"
    );

    ws.send(Message::Text(
        r#"{"tipo":"comando","id":"pagina-siguiente"}"#.into(),
    ))
    .expect("enviar");
    assert_eq!(leer(&mut ws)["aceptada"], true);
    assert_eq!(
        ordenes.try_recv().expect("orden entregada"),
        nb_books_lib::movil::control::Orden::Comando {
            id: "pagina-siguiente".to_owned()
        }
    );

    ws.send(Message::Text(
        r#"{"tipo":"comando","id":"borrar-biblioteca"}"#.into(),
    ))
    .expect("enviar");
    assert_eq!(
        leer(&mut ws)["aceptada"],
        false,
        "solo las órdenes del lector"
    );
    assert!(ordenes.try_recv().is_err());
    assert!(
        puente.controlador().is_some(),
        "manda quien dio la última orden"
    );

    servidor.cortar_control();
    assert!(
        puente.controlador().is_none(),
        "cortar deja el lector sin mando"
    );
}

#[test]
fn la_portada_se_sirve_aunque_la_rejilla_ya_la_redujera_a_miniatura() {
    let b = banco("portada");
    let token = emparejado(&b);
    pedir(
        b.puerto,
        "POST",
        "/api/v1/seleccion",
        Some(&token),
        Some(&json!({"todo": true})),
        "",
    );
    let portadas = b._dir.join("cache").join("portadas");
    std::fs::create_dir_all(&portadas).expect("portadas");
    // Como la deja la rejilla del escritorio: solo la miniatura, sin original.
    std::fs::write(portadas.join("tapa.jpg.min.jpg"), b"\xff\xd8\xff miniatura").expect("mini");
    let conn = b.db.connection().expect("conexión");
    conn.execute(
        "UPDATE editions SET cover_path = 'tapa.jpg' WHERE id = ?1",
        [&b.edicion],
    )
    .expect("portada");
    let ruta = format!("/api/v1/portada/{}", b.edicion);
    let r = pedir(b.puerto, "GET", &ruta, Some(&token), None, "");
    assert_eq!(r.estado, 200, "la miniatura basta para servir la portada");
    assert_eq!(r.cuerpo, b"\xff\xd8\xff miniatura");

    conn.execute(
        "UPDATE editions SET cover_path = '../fuera.jpg' WHERE id = ?1",
        [&b.edicion],
    )
    .expect("ruta");
    assert_ne!(
        pedir(b.puerto, "GET", &ruta, Some(&token), None, "").estado,
        200,
        "un nombre con ruta no sale de la carpeta de portadas"
    );
}
