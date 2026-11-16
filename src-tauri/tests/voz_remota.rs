//! La voz que sale del dispositivo, y lo que la sujeta.
//!
//! Es la cuarta salida que el usuario concede, y la que más pesa: lo que se
//! manda es el **texto del libro**, párrafo a párrafo. Lo que estas pruebas
//! comprueban no es que suene, que eso lo hace un servicio de terceros, sino
//! que **no sale nada que no se haya autorizado** y que lo que sale es lo que se
//! dijo que saldría.

use std::sync::{Arc, Mutex};

use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::net::{HttpTransport, Network, Purpose, Response};
use nb_books_lib::speech::{sintetizar, Destino};

/// Lo que salió: a dónde y con qué cuerpo.
///
/// El cuerpo se guarda además de la dirección porque aquí lo que hay que poder
/// comprobar es justo eso: **qué texto sale de este equipo**.
type Enviados = Arc<Mutex<Vec<(String, String)>>>;

/// Transporte que apunta lo que se le manda y contesta lo que se le diga.
struct Espia {
    enviados: Enviados,
    audio: Vec<u8>,
    estado: u16,
}

impl Espia {
    fn nuevo() -> (Self, Enviados) {
        let enviados = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                enviados: Arc::clone(&enviados),
                audio: b"ID3 audio de mentira".to_vec(),
                estado: 200,
            },
            enviados,
        )
    }
}

impl HttpTransport for Espia {
    fn get(&self, _url: &str) -> NbResult<Response> {
        Err(NbError::NetworkFailure)
    }

    fn post_json(&self, url: &str, cuerpo: &str, _credencial: Option<&str>) -> NbResult<Response> {
        self.enviados
            .lock()
            .expect("registro del espía")
            .push((url.to_owned(), cuerpo.to_owned()));
        Ok(Response {
            status: self.estado,
            content_type: Some("audio/mpeg".to_owned()),
            body: self.audio.clone(),
            location: None,
        })
    }
}

struct Temporal(std::path::PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let ruta = nb_books_lib::paths::temporal_de_pruebas().join(format!(
            "nb-voz-remota-{nombre}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&ruta).expect("carpeta temporal");
        Self(ruta)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn destino() -> Destino {
    Destino {
        host: "voz.example".to_owned(),
        model: "tts-1".to_owned(),
        voice: "alloy".to_owned(),
    }
}

/// **De fábrica no sale ni un párrafo.**
///
/// Es el innegociable, y aquí pesa más que en ninguna otra salida: configurar el
/// servidor es escribir una dirección, y escribir una dirección no es autorizar
/// que se le mande el libro. La comprobación no es que devuelva error, es que el
/// transporte **no vio nada**.
#[test]
fn sin_consentimiento_el_texto_del_libro_no_sale_del_equipo() {
    let tmp = Temporal::nuevo("sin-permiso");
    let db = Database::open_in_memory().expect("base");
    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));

    let resultado = sintetizar(
        &db,
        &red,
        &tmp.0,
        &destino(),
        "Muchos años después, frente al pelotón de fusilamiento.",
        1.0,
    );

    assert!(matches!(resultado, Err(NbError::ConsentRequired)));
    assert!(
        enviados.lock().expect("registro").is_empty(),
        "salió algo del equipo sin permiso"
    );
}

/// Con permiso sale, y sale **lo que se dijo**: ese texto, a ese servidor.
#[test]
fn con_consentimiento_sale_ese_texto_y_a_ese_servidor() {
    let tmp = Temporal::nuevo("con-permiso");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Speech).expect("permiso");
    drop(conn);

    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));

    let archivo =
        sintetizar(&db, &red, &tmp.0, &destino(), "Macondo era entonces.", 1.0).expect("síntesis");

    assert!(archivo.is_file(), "no se guardó el audio");
    let salidas = enviados.lock().expect("registro");
    assert_eq!(salidas.len(), 1);
    assert_eq!(salidas[0].0, "https://voz.example/v1/audio/speech");
    assert!(
        salidas[0].1.contains("Macondo era entonces."),
        "no salió el texto que se pidió: {}",
        salidas[0].1
    );
}

/// **El permiso de un propósito no vale para otro.**
///
/// Autorizar a un servidor a servir un catálogo no lo autoriza a recibir el
/// texto de los libros. Es la razón de que la voz tenga propósito propio, y sin
/// esta prueba la separación sería una intención.
#[test]
fn el_permiso_de_otro_proposito_no_sirve_para_la_voz() {
    let tmp = Temporal::nuevo("otro-proposito");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Catalog).expect("permiso");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Lookup).expect("permiso");
    drop(conn);

    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));

    let resultado = sintetizar(&db, &red, &tmp.0, &destino(), "Un párrafo cualquiera.", 1.0);

    assert!(matches!(resultado, Err(NbError::ConsentRequired)));
    assert!(enviados.lock().expect("registro").is_empty());
}

/// El modo sin red corta también esto, que es lo que significa "corte de
/// emergencia": no admite excepciones por muy autorizado que esté algo.
#[test]
fn el_modo_sin_red_calla_tambien_la_voz_autorizada() {
    let tmp = Temporal::nuevo("sin-red");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Speech).expect("permiso");
    nb_books_lib::settings::set(
        &conn,
        nb_books_lib::settings::SettingKey::OfflineMode,
        &nb_books_lib::settings::SettingValue::Flag(true),
    )
    .expect("interruptor");
    drop(conn);

    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));

    let resultado = sintetizar(&db, &red, &tmp.0, &destino(), "Nada debería salir.", 1.0);

    assert!(matches!(resultado, Err(NbError::NetworkDisabled)));
    assert!(enviados.lock().expect("registro").is_empty());
}

/// **Lo mismo dos veces no se manda dos veces.**
///
/// No es solo ahorro: cada envío es el texto del libro saliendo del equipo, así
/// que el que no hace falta es uno que no debería ocurrir. Volver atrás un
/// párrafo y avanzar de nuevo es lo más normal del mundo leyendo.
#[test]
fn el_mismo_parrafo_no_sale_dos_veces() {
    let tmp = Temporal::nuevo("cache");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Speech).expect("permiso");
    drop(conn);

    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));

    let uno = sintetizar(&db, &red, &tmp.0, &destino(), "El mismo párrafo.", 1.0).expect("una");
    let dos = sintetizar(&db, &red, &tmp.0, &destino(), "El mismo párrafo.", 1.0).expect("otra");

    assert_eq!(uno, dos);
    assert_eq!(
        enviados.lock().expect("registro").len(),
        1,
        "salió dos veces"
    );
}

/// Y con otra voz sí sale otra vez: suena distinto, así que servir el audio
/// guardado sería servir la voz que no es.
#[test]
fn cambiar_de_voz_no_reutiliza_el_audio_de_la_anterior() {
    let tmp = Temporal::nuevo("otra-voz");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Speech).expect("permiso");
    drop(conn);

    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));

    let alloy = sintetizar(&db, &red, &tmp.0, &destino(), "El mismo párrafo.", 1.0).expect("una");
    let otra = Destino {
        voice: "nova".to_owned(),
        ..destino()
    };
    let nova = sintetizar(&db, &red, &tmp.0, &otra, "El mismo párrafo.", 1.0).expect("otra");

    assert_ne!(alloy, nova);
    assert_eq!(enviados.lock().expect("registro").len(), 2);
}

/// Un servidor en la propia máquina se admite en claro, y es lo que permite
/// tener voz neuronal **sin que salga nada del equipo**. Es la excepción de
/// `net::host_de`, comprobada de punta a punta.
#[test]
fn un_servidor_en_la_propia_maquina_vale_y_va_en_claro() {
    let tmp = Temporal::nuevo("local");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "localhost", Purpose::Speech).expect("permiso");
    drop(conn);

    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));
    let propio = Destino {
        host: "http://localhost:8880".to_owned(),
        ..destino()
    };

    sintetizar(&db, &red, &tmp.0, &propio, "En mi propia máquina.", 1.0).expect("síntesis");

    let salidas = enviados.lock().expect("registro");
    assert_eq!(salidas[0].0, "http://localhost:8880/v1/audio/speech");
}

/// Sin servicio configurado no se intenta nada: el campo vacío es la decisión de
/// fábrica, no un hueco por rellenar.
#[test]
fn sin_servicio_configurado_no_se_intenta_salir() {
    let tmp = Temporal::nuevo("sin-servicio");
    let db = Database::open_in_memory().expect("base");
    let (espia, enviados) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));
    let vacio = Destino {
        host: String::new(),
        ..destino()
    };

    let resultado = sintetizar(&db, &red, &tmp.0, &vacio, "Cualquier cosa.", 1.0);

    assert!(matches!(resultado, Err(NbError::PathNotAllowed)));
    assert!(enviados.lock().expect("registro").is_empty());
}

/// Lo que sale queda **en el registro que el usuario consulta**, con el
/// anfitrión y el propósito. Sin eso, "observable desde la interfaz" sería una
/// frase.
#[test]
fn la_salida_de_voz_queda_registrada_con_su_proposito() {
    let tmp = Temporal::nuevo("registro");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "voz.example", Purpose::Speech).expect("permiso");
    drop(conn);

    let (espia, _) = Espia::nuevo();
    let red = Arc::new(Network::new(Box::new(espia)));
    sintetizar(&db, &red, &tmp.0, &destino(), "Queda anotado.", 1.0).expect("síntesis");

    let conn = db.connection().expect("conexión");
    let registro = nb_books_lib::net::log(&conn, 10).expect("registro");
    let linea = registro.first().expect("hay una línea");
    assert_eq!(linea.host, "voz.example");
    assert_eq!(linea.purpose, "speech");
}

/// Un servidor de verdad que habla el protocolo, en la propia máquina.
///
/// Atiende **una** petición y se va. Es medio centenar de líneas de biblioteca
/// estándar en lugar de una dependencia, y sirve para lo que ninguna prueba con
/// un transporte simulado puede: comprobar que lo que sale por el socket es lo
/// que un servidor real espera recibir.
fn servidor_de_una_peticion() -> (u16, std::sync::mpsc::Receiver<String>) {
    use std::io::{BufRead, BufReader, Write};

    let escucha = std::net::TcpListener::bind("127.0.0.1:0").expect("puerto libre");
    let puerto = escucha.local_addr().expect("dirección").port();
    let (emisor, receptor) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let Ok((flujo, _)) = escucha.accept() else {
            return;
        };
        let mut lector = BufReader::new(flujo.try_clone().expect("clon"));

        // Cabeceras hasta la línea en blanco, quedándose con la primera línea y
        // el tamaño del cuerpo.
        let mut peticion = String::new();
        let mut largo = 0_usize;
        loop {
            let mut linea = String::new();
            if lector.read_line(&mut linea).unwrap_or(0) == 0 {
                break;
            }
            if let Some(valor) = linea.to_ascii_lowercase().strip_prefix("content-length:") {
                largo = valor.trim().parse().unwrap_or(0);
            }
            if linea.trim().is_empty() {
                break;
            }
            peticion.push_str(&linea);
        }

        let mut cuerpo = vec![0_u8; largo];
        use std::io::Read;
        let _ = lector.read_exact(&mut cuerpo);
        let _ = emisor.send(format!(
            "{peticion}\r\n{}",
            String::from_utf8_lossy(&cuerpo)
        ));

        // Un MP3 de mentira, con su cabecera para que se parezca a uno.
        let audio: Vec<u8> = b"ID3\x03\x00\x00\x00"
            .iter()
            .copied()
            .chain(std::iter::repeat_n(0, 512))
            .collect();
        let mut salida = flujo;
        let _ = write!(
            salida,
            "HTTP/1.1 200 OK\r\ncontent-type: audio/mpeg\r\ncontent-length: {}\r\n\r\n",
            audio.len()
        );
        let _ = salida.write_all(&audio);
        let _ = salida.flush();
    });

    (puerto, receptor)
}

/// **De punta a punta contra un servidor de verdad.**
///
/// Las demás pruebas de este archivo usan un transporte simulado, que comprueba
/// la puerta y no el cable: con un espía, el cliente HTTP podría no mandar el
/// cuerpo, escribir mal la ruta o no poner el tipo de contenido, y todas
/// seguirían en verde. Aquí sale por un socket y lo recibe algo que habla el
/// protocolo, así que se comprueba lo que un servidor de síntesis vería llegar.
///
/// Va contra la propia máquina y en claro a propósito: es exactamente la
/// configuración que se recomienda —un servidor de síntesis levantado en el
/// mismo equipo— y de paso comprueba la excepción de `net::host_de`.
#[test]
fn contra_un_servidor_de_verdad_llega_lo_que_el_protocolo_espera() {
    let tmp = Temporal::nuevo("de-verdad");
    let (puerto, recibido) = servidor_de_una_peticion();

    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    nb_books_lib::net::grant(&conn, "127.0.0.1", Purpose::Speech).expect("permiso");
    drop(conn);

    // El transporte real, no un espía: es lo que se está comprobando.
    let red = Arc::new(Network::new(Box::new(
        nb_books_lib::net::UreqTransport::new(),
    )));
    let destino = Destino {
        host: format!("http://127.0.0.1:{puerto}"),
        model: "kokoro".to_owned(),
        voice: "ef_dora".to_owned(),
    };

    let archivo = sintetizar(
        &db,
        &red,
        &tmp.0,
        &destino,
        "Muchos años después, frente al pelotón de fusilamiento.",
        1.25,
    )
    .expect("síntesis");

    assert!(archivo.is_file(), "no se guardó el audio que devolvió");
    assert!(
        std::fs::metadata(&archivo).expect("tamaño").len() > 0,
        "el audio llegó vacío"
    );

    let peticion = recibido
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("el servidor no recibió nada");

    // La ruta, el método y el tipo: lo que cualquier servidor comprueba antes de
    // mirar el cuerpo.
    assert!(
        peticion.starts_with("POST /v1/audio/speech "),
        "ruta o método inesperados: {}",
        peticion.lines().next().unwrap_or_default()
    );
    assert!(
        peticion
            .to_ascii_lowercase()
            .contains("content-type: application/json"),
        "sin tipo de contenido"
    );

    // Y el cuerpo, con los cinco campos y el texto entero. Los acentos van en
    // UTF-8 tal cual: un libro en español es casi todo acentos.
    for esperado in [
        "\"model\":\"kokoro\"",
        "\"voice\":\"ef_dora\"",
        "\"response_format\":\"mp3\"",
        "\"speed\":1.25",
        "Muchos años después, frente al pelotón de fusilamiento.",
    ] {
        assert!(
            peticion.contains(esperado),
            "falta {esperado} en {peticion}"
        );
    }
}

/// Las tres formas en que alguien escribe la dirección de su servidor.
///
/// **Encontrado usando la aplicación contra un servidor de verdad, no leyendo
/// el código.** El protocolo se documenta en todas partes como `base_url` =
/// `.../v1` —es lo que piden el SDK de OpenAI y las guías de Kokoro-FastAPI,
/// Speaches y cualquier servidor compatible—, así que quien copia su
/// configuración escribe el `/v1` y hacía falta no añadirle otro detrás. Con la
/// dirección duplicada, un servidor que funcionaba respondía 404.
#[test]
fn la_ruta_del_protocolo_no_se_duplica_por_escribirla() {
    let esperado = "http://192.168.100.10:8880/v1/audio/speech";

    for escrito in [
        "http://192.168.100.10:8880",
        "http://192.168.100.10:8880/",
        "http://192.168.100.10:8880/v1",
        "http://192.168.100.10:8880/v1/",
        "http://192.168.100.10:8880/v1/audio/speech",
    ] {
        let destino = Destino {
            host: escrito.to_owned(),
            model: "tts-1".to_owned(),
            voice: "ef_dora".to_owned(),
        };
        assert_eq!(
            destino.punto_de_sintesis(),
            esperado,
            "escrito como {escrito}"
        );
    }
}

/// Un servicio que cuelga el protocolo de una ruta propia sigue funcionando.
///
/// No todos lo sirven desde la raíz: OpenRouter lo publica bajo `/api`. Recortar
/// cualquier ruta rompería ese caso, así que solo se recorta la del propio
/// protocolo.
#[test]
fn una_ruta_propia_del_servicio_se_respeta() {
    let destino = Destino {
        host: "https://api.openrouter.ai/api".to_owned(),
        model: "tts-1".to_owned(),
        voice: "alloy".to_owned(),
    };
    assert_eq!(
        destino.punto_de_sintesis(),
        "https://api.openrouter.ai/api/v1/audio/speech"
    );
}
