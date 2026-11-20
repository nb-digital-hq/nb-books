//! Criterio de aceptación de la descarga de modelos.
//!
//! "Un archivo que no coincide con su huella no queda nunca en el directorio de
//! modelos, y una descarga cortada no deja nada que parezca un modelo instalado."
//!
//! Se prueba contra un transporte simulado y no contra Hugging Face: el criterio
//! tiene que poder demostrarse sin red y sin que un servidor de terceros esté
//! vivo, y los casos que importan —un byte cambiado, un servidor que manda de más,
//! una redirección a un anfitrión sin permiso— no se pueden provocar en uno real.
//! La descarga real, de punta a punta, está en la última prueba, que solo corre
//! si se pide con `NB_BOOKS_RED_REAL=1`.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 2.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use nb_books_lib::ai::catalogo::{Agrupacion, Archivo, Arquitectura, Modelo, ANFITRION};
use nb_books_lib::ai::instalacion::{self, Instalacion};
use nb_books_lib::ai::Capability;
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::net::{self, Cabecera, HttpTransport, Network, Outcome, Purpose, Response};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-modelos-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Lo que responde el servidor simulado para una dirección.
#[derive(Clone)]
enum Respuesta {
    Cuerpo(Vec<u8>),
    Redirige(String),
}

type Gancho = Box<dyn Fn() + Send + Sync>;

/// Servidor simulado que **anota cada petición** y sirve en bloques pequeños,
/// para que cancelar y desbordar se ejerciten a mitad de un archivo y no solo al
/// principio.
#[derive(Default)]
struct Servidor {
    respuestas: HashMap<String, Respuesta>,
    peticiones: Arc<Mutex<Vec<String>>>,
    /// Bytes que el destino **aceptó** escribir, sumando todas las descargas.
    aceptados: Arc<Mutex<u64>>,
    /// Se ejecuta a mitad de cada cuerpo servido.
    gancho: Option<Gancho>,
}

impl HttpTransport for Servidor {
    fn get(&self, _url: &str) -> NbResult<Response> {
        Err(NbError::NetworkFailure)
    }

    fn descargar(
        &self,
        url: &str,
        _plazo: std::time::Duration,
        destino: &mut dyn std::io::Write,
        seguir: &mut dyn FnMut(u64) -> bool,
    ) -> NbResult<Cabecera> {
        self.peticiones
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(url.to_owned());

        match self.respuestas.get(url) {
            None => Ok(Cabecera {
                status: 404,
                location: None,
            }),
            Some(Respuesta::Redirige(a)) => Ok(Cabecera {
                status: 302,
                location: Some(a.clone()),
            }),
            Some(Respuesta::Cuerpo(bytes)) => {
                let mut escritos = 0_u64;
                for (i, bloque) in bytes.chunks(7).enumerate() {
                    if i == 1 {
                        if let Some(gancho) = &self.gancho {
                            gancho();
                        }
                    }
                    destino
                        .write_all(bloque)
                        .map_err(|_| NbError::StorageFailure)?;
                    *self.aceptados.lock().unwrap_or_else(|p| p.into_inner()) +=
                        bloque.len() as u64;
                    escritos += bloque.len() as u64;
                    if !seguir(escritos) {
                        return Err(NbError::Cancelled);
                    }
                }
                Ok(Cabecera {
                    status: 200,
                    location: None,
                })
            }
        }
    }
}

/// Un modelo de catálogo diminuto, con las huellas de unos bytes conocidos.
///
/// Los campos del catálogo son `'static` porque el de verdad está compilado; aquí
/// se filtran a propósito, que en una prueba no cuesta nada.
fn modelo_de_prueba(archivos: &[(&str, &[u8])]) -> &'static Modelo {
    let archivos: Vec<Archivo> = archivos
        .iter()
        .map(|(nombre, bytes)| Archivo {
            nombre: Box::leak(nombre.to_string().into_boxed_str()),
            bytes: bytes.len() as u64,
            sha256: Box::leak(hex(&Sha256::digest(bytes)).into_boxed_str()),
            origen: None,
        })
        .collect();

    Box::leak(Box::new(Modelo {
        id: "modelo-de-prueba",
        nombre: "Modelo de Prueba",
        autor: "Nadie",
        repositorio: "nadie/modelo-de-prueba",
        revision: "0123456789012345678901234567890123456789",
        licencia: "Apache-2.0",
        capacidad: Capability::Embedding,
        dimension: 384,
        arquitectura: Arquitectura::XlmRoberta,
        agrupacion: Agrupacion::Cls,
        longitud_maxima: 512,
        idiomas: &["es"],
        memoria: 1,
        memoria_por_texto: 1,
        rasgo: nb_books_lib::ai::catalogo::Rasgo::NoGenera,
        preferencia: 0,
        segundos_de_referencia: 0,
        equipo_recomendado: nb_books_lib::ai::catalogo::Equipo {
            memoria: 0,
            nucleos: 0,
        },
        archivos: Box::leak(archivos.into_boxed_slice()),
    }))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const PESOS: &[u8] = b"pesos del modelo, que en la realidad son doscientos megabytes";
const TOKENIZADOR: &[u8] = b"{\"vocabulario\": \"el tokenizador\"}";

fn modelo() -> &'static Modelo {
    modelo_de_prueba(&[
        ("model.safetensors", PESOS),
        ("tokenizer.json", TOKENIZADOR),
    ])
}

/// Un servidor que sirve cada archivo del modelo tal y como se revisó.
fn servidor_fiel(modelo: &Modelo) -> Servidor {
    let mut servidor = Servidor::default();
    for (archivo, bytes) in modelo.archivos.iter().zip([PESOS, TOKENIZADOR]) {
        servidor
            .respuestas
            .insert(modelo.direccion(archivo), Respuesta::Cuerpo(bytes.to_vec()));
    }
    servidor
}

fn base_con_permiso() -> Database {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    net::grant(&conn, ANFITRION, Purpose::Model).expect("permiso");
    db
}

fn instalar(
    db: &Database,
    red: &Network,
    modelos: &Path,
    modelo: &Modelo,
) -> NbResult<Instalacion> {
    instalacion::instalar(
        db,
        red,
        modelos,
        modelo,
        &AtomicBool::new(false),
        &mut |_, _| {},
    )
}

/// No queda nada que pueda parecer un modelo: ni su carpeta ni la temporal.
fn no_queda_nada(modelos: &Path, modelo: &Modelo) {
    assert!(!instalacion::instalado(modelos, modelo));
    let entradas: Vec<_> = std::fs::read_dir(modelos)
        .map(|e| e.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(
        entradas.is_empty(),
        "quedó algo en el directorio de modelos: {entradas:?}"
    );
}

/// El camino bueno: lo servido coincide y el modelo queda instalado, sin
/// carpeta temporal detrás y con el progreso llegando al total.
#[test]
fn un_modelo_cuyos_archivos_coinciden_con_su_huella_queda_instalado() {
    let tmp = Temporal::nuevo("bueno");
    let db = base_con_permiso();
    let modelo = modelo();
    let red = Network::new(Box::new(servidor_fiel(modelo)));

    let mut ultimo = (0, 0);
    let resultado = instalacion::instalar(
        &db,
        &red,
        &tmp.0,
        modelo,
        &AtomicBool::new(false),
        &mut |hechos, total| ultimo = (hechos, total),
    )
    .expect("instalación");

    assert_eq!(resultado, Instalacion::Instalado);
    assert!(instalacion::instalado(&tmp.0, modelo));
    assert_eq!(
        std::fs::read(instalacion::carpeta(&tmp.0, modelo).join("model.safetensors"))
            .expect("pesos"),
        PESOS
    );
    assert_eq!(
        ultimo,
        (modelo.disco(), modelo.disco()),
        "el progreso no llegó al total"
    );
    let restos: Vec<_> = std::fs::read_dir(&tmp.0)
        .expect("directorio")
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        restos.len(),
        1,
        "quedó una carpeta temporal detrás: {restos:?}"
    );
}

/// **El criterio, primera mitad.** Un byte distinto y no queda nada: ni el
/// modelo, ni su carpeta temporal, ni el archivo que sí coincidía.
#[test]
fn lo_descargado_que_no_coincide_con_su_huella_no_deja_nada() {
    let tmp = Temporal::nuevo("huella");
    let db = base_con_permiso();
    let modelo = modelo();
    let mut servidor = servidor_fiel(modelo);
    let mut alterado = TOKENIZADOR.to_vec();
    alterado[3] ^= 0xFF;
    servidor.respuestas.insert(
        modelo.direccion(&modelo.archivos[1]),
        Respuesta::Cuerpo(alterado),
    );
    let red = Network::new(Box::new(servidor));

    let resultado = instalar(&db, &red, &tmp.0, modelo);

    assert!(matches!(resultado, Err(NbError::DocumentCorrupted)));
    no_queda_nada(&tmp.0, modelo);
}

/// Un servidor que manda más de lo revisado **se corta al pasarse**, no al
/// terminar: si no, uno que mandara diez gigas llenaría el disco antes de que la
/// huella pudiera decir nada. Y el error dice que no coincidió, no que no se pudo
/// escribir, que sería mentir.
#[test]
fn un_servidor_que_manda_mas_de_lo_revisado_se_corta_y_no_deja_nada() {
    let tmp = Temporal::nuevo("desborde");
    let db = base_con_permiso();
    let modelo = modelo();
    let mut servidor = servidor_fiel(modelo);
    let mut de_mas = PESOS.to_vec();
    de_mas.extend(std::iter::repeat_n(b'x', 1_000));
    servidor.respuestas.insert(
        modelo.direccion(&modelo.archivos[0]),
        Respuesta::Cuerpo(de_mas),
    );
    let aceptados = Arc::clone(&servidor.aceptados);
    let red = Network::new(Box::new(servidor));

    let resultado = instalar(&db, &red, &tmp.0, modelo);

    assert!(
        matches!(resultado, Err(NbError::DocumentCorrupted)),
        "resultó {resultado:?}"
    );
    // Lo que distingue "se corta al pasarse" de "se rechaza al terminar": el
    // destino no llegó a aceptar ni un bloque entero de más. Sin el corte, la
    // huella fallaría igual al final y la prueba pasaría por el motivo equivocado,
    // habiendo escrito entero lo que un servidor quisiera mandar.
    assert!(
        *aceptados.lock().expect("aceptados") < PESOS.len() as u64 + 7,
        "se escribió de más antes de cortar"
    );
    no_queda_nada(&tmp.0, modelo);
}

/// **El criterio, segunda mitad.** Cortar a mitad no deja nada que parezca
/// instalado.
#[test]
fn cancelar_a_mitad_de_la_descarga_no_deja_nada() {
    let tmp = Temporal::nuevo("cancelar");
    let db = base_con_permiso();
    let modelo = modelo();
    let red = Network::new(Box::new(servidor_fiel(modelo)));
    let cancelado = AtomicBool::new(false);

    let resultado =
        instalacion::instalar(&db, &red, &tmp.0, modelo, &cancelado, &mut |hechos, _| {
            if hechos > 10 {
                cancelado.store(true, Ordering::SeqCst);
            }
        });

    assert!(matches!(resultado, Err(NbError::Cancelled)));
    no_queda_nada(&tmp.0, modelo);
}

/// Lo que dejó a medias una sesión anterior —un corte de luz— no cuenta como
/// instalado aunque tenga los archivos completos, y se limpia al reintentar.
#[test]
fn una_descarga_a_medias_de_otra_sesion_no_cuenta_y_se_limpia_al_reintentar() {
    let tmp = Temporal::nuevo("a-medias");
    let db = base_con_permiso();
    let modelo = modelo();

    let parcial = tmp.0.join(format!(".{}.parcial", modelo.id));
    std::fs::create_dir_all(&parcial).expect("carpeta temporal");
    std::fs::write(parcial.join("model.safetensors"), PESOS).expect("pesos completos");
    std::fs::write(parcial.join("tokenizer.json"), TOKENIZADOR).expect("tokenizador completo");
    std::fs::write(parcial.join("basura"), b"de la sesion anterior").expect("resto");
    assert!(
        !instalacion::instalado(&tmp.0, modelo),
        "una carpeta temporal no es un modelo"
    );

    let red = Network::new(Box::new(servidor_fiel(modelo)));
    instalar(&db, &red, &tmp.0, modelo).expect("reintento");

    assert!(instalacion::instalado(&tmp.0, modelo));
    assert!(
        !parcial.exists(),
        "el resto de la sesión anterior sigue ahí"
    );
    assert!(!instalacion::carpeta(&tmp.0, modelo).join("basura").exists());
}

/// Redirigir a la red de distribución **no autoriza** a la red de distribución.
/// Se pide su propio permiso, con su nombre, y el transporte no llega a
/// pedirle nada hasta que se concede.
#[test]
fn la_red_de_distribucion_necesita_su_propio_permiso() {
    let tmp = Temporal::nuevo("cdn");
    let db = base_con_permiso();
    let modelo = modelo();

    let mut servidor = Servidor::default();
    for (archivo, bytes) in modelo.archivos.iter().zip([PESOS, TOKENIZADOR]) {
        let en_cdn = format!("https://cdn.ejemplo/{}", archivo.nombre);
        servidor.respuestas.insert(
            modelo.direccion(archivo),
            Respuesta::Redirige(en_cdn.clone()),
        );
        servidor
            .respuestas
            .insert(en_cdn, Respuesta::Cuerpo(bytes.to_vec()));
    }
    let peticiones = Arc::clone(&servidor.peticiones);
    let red = Network::new(Box::new(servidor));

    let primero = instalar(&db, &red, &tmp.0, modelo).expect("sin error");
    assert_eq!(
        primero,
        Instalacion::FaltaPermiso {
            host: "cdn.ejemplo".to_owned()
        }
    );
    assert!(
        !peticiones
            .lock()
            .expect("peticiones")
            .iter()
            .any(|p| p.contains("cdn.ejemplo")),
        "se le pidió algo a un anfitrión sin permiso"
    );
    no_queda_nada(&tmp.0, modelo);

    net::grant(
        &db.connection().expect("conexión"),
        "cdn.ejemplo",
        Purpose::Model,
    )
    .expect("permiso");
    assert_eq!(
        instalar(&db, &red, &tmp.0, modelo).expect("instalación"),
        Instalacion::Instalado
    );
}

/// Sin permiso para el anfitrión del catálogo no se sale, y el intento queda
/// anotado: es lo que el usuario querría poder ver.
#[test]
fn sin_permiso_no_se_descarga_y_el_intento_queda_anotado() {
    let tmp = Temporal::nuevo("sin-permiso");
    let db = Database::open_in_memory().expect("base");
    let modelo = modelo();
    let servidor = servidor_fiel(modelo);
    let peticiones = Arc::clone(&servidor.peticiones);
    let red = Network::new(Box::new(servidor));

    assert_eq!(
        instalar(&db, &red, &tmp.0, modelo).expect("sin error"),
        Instalacion::FaltaPermiso {
            host: ANFITRION.to_owned()
        }
    );
    assert!(peticiones.lock().expect("peticiones").is_empty());

    let registro = net::log(&db.connection().expect("conexión"), 10).expect("registro");
    assert_eq!(registro[0].outcome, Outcome::Denied);
    assert_eq!(registro[0].purpose, "model");
    no_queda_nada(&tmp.0, modelo);
}

/// Con el modo sin red no se intenta siquiera, aunque haya permiso.
#[test]
fn con_el_modo_sin_red_no_se_intenta_descargar() {
    let tmp = Temporal::nuevo("sin-red");
    let db = base_con_permiso();
    settings::set(
        &db.connection().expect("conexión"),
        SettingKey::OfflineMode,
        &SettingValue::Flag(true),
    )
    .expect("interruptor");
    let modelo = modelo();
    let servidor = servidor_fiel(modelo);
    let peticiones = Arc::clone(&servidor.peticiones);
    let red = Network::new(Box::new(servidor));

    assert!(matches!(
        instalar(&db, &red, &tmp.0, modelo),
        Err(NbError::NetworkDisabled)
    ));
    assert!(peticiones.lock().expect("peticiones").is_empty());
    no_queda_nada(&tmp.0, modelo);
}

/// Cada salto queda en el registro con lo que movió: la redirección con cero
/// bytes y la red de distribución con los del archivo. Un registro que solo
/// anotara el primer anfitrión escondería a dónde salió de verdad el equipo.
#[test]
fn cada_salto_de_la_descarga_queda_anotado_con_sus_bytes() {
    let tmp = Temporal::nuevo("registro");
    let db = base_con_permiso();
    net::grant(
        &db.connection().expect("conexión"),
        "cdn.ejemplo",
        Purpose::Model,
    )
    .expect("permiso");
    let modelo = modelo_de_prueba(&[("model.safetensors", PESOS)]);

    let mut servidor = Servidor::default();
    let en_cdn = "https://cdn.ejemplo/model.safetensors".to_owned();
    servidor.respuestas.insert(
        modelo.direccion(&modelo.archivos[0]),
        Respuesta::Redirige(en_cdn.clone()),
    );
    servidor
        .respuestas
        .insert(en_cdn, Respuesta::Cuerpo(PESOS.to_vec()));
    let red = Network::new(Box::new(servidor));

    instalar(&db, &red, &tmp.0, modelo).expect("instalación");

    let registro = net::log(&db.connection().expect("conexión"), 10).expect("registro");
    let del_cdn = registro
        .iter()
        .find(|e| e.host == "cdn.ejemplo")
        .expect("salto al cdn");
    let del_catalogo = registro
        .iter()
        .find(|e| e.host == ANFITRION)
        .expect("salto al catálogo");
    assert_eq!(del_cdn.bytes, PESOS.len() as u64);
    assert_eq!(del_cdn.status, Some(200));
    assert_eq!(del_catalogo.status, Some(302));
    assert_eq!(del_catalogo.bytes, 0);
}

/// **Regresión de la lección del 2026-09-12**, aplicada antes de que ocurra.
///
/// Una descarga dura minutos y el pool tiene cuatro conexiones. Si la puerta
/// retuviera una durante la transferencia, dos descargas y un poco de uso
/// dejarían la aplicación sin poder guardar el progreso de lectura, que es
/// exactamente el defecto con otra causa. A mitad de la transferencia
/// se piden **las cuatro** a la vez: si la puerta tuviera una, la cuarta
/// esperaría cinco segundos y fallaría.
#[test]
fn descargar_no_retiene_ninguna_conexion_del_pool_mientras_transfiere() {
    let tmp = Temporal::nuevo("pool");
    let db = base_con_permiso();
    let modelo = modelo();

    let pudo = Arc::new(Mutex::new(Vec::<bool>::new()));
    let mut servidor = servidor_fiel(modelo);
    let (db_del_gancho, pudo_del_gancho) = (db.clone(), Arc::clone(&pudo));
    servidor.gancho = Some(Box::new(move || {
        let conexiones: Vec<_> = (0..4).map(|_| db_del_gancho.connection()).collect();
        pudo_del_gancho
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(conexiones.iter().all(Result::is_ok));
    }));
    let red = Network::new(Box::new(servidor));

    instalar(&db, &red, &tmp.0, modelo).expect("instalación");

    let pudo = pudo.lock().expect("resultado").clone();
    assert!(!pudo.is_empty(), "el gancho no llegó a ejecutarse");
    assert!(
        pudo.iter().all(|&p| p),
        "durante la transferencia no quedaban las cuatro conexiones libres"
    );
}

/// Quitar un modelo se lleva también lo que hubiera quedado a medias.
#[test]
fn desinstalar_se_lleva_el_modelo_y_lo_que_quedara_a_medias() {
    let tmp = Temporal::nuevo("desinstalar");
    let db = base_con_permiso();
    let modelo = modelo();
    let red = Network::new(Box::new(servidor_fiel(modelo)));
    instalar(&db, &red, &tmp.0, modelo).expect("instalación");
    std::fs::create_dir_all(tmp.0.join(format!(".{}.parcial", modelo.id))).expect("resto");

    instalacion::desinstalar(&tmp.0, modelo).expect("desinstalación");

    no_queda_nada(&tmp.0, modelo);
    // Y desinstalar lo que no está no es un error.
    instalacion::desinstalar(&tmp.0, modelo).expect("desinstalar dos veces");
}

/// **La descarga real, de punta a punta**, contra Hugging Face y con el modelo
/// del catálogo. No forma parte de la verificación —necesita red y descarga 223
/// MB—, así que se salta sola salvo con `NB_BOOKS_RED_REAL=1`.
///
/// Existe porque lo simulado no puede comprobar lo que solo hace el real: que
/// `ureq` acepte el plazo por petición, que la redirección de Hugging Face vaya a
/// un anfitrión que se puede autorizar, y que las huellas del catálogo sean las
/// de los bytes que sirve.
#[test]
fn la_descarga_real_del_modelo_del_catalogo_instala_lo_revisado() {
    if std::env::var("NB_BOOKS_RED_REAL").as_deref() != Ok("1") {
        eprintln!("se salta: pide NB_BOOKS_RED_REAL=1 para descargar de verdad");
        return;
    }

    let tmp = Temporal::nuevo("real");
    let db = base_con_permiso();
    let modelo = &nb_books_lib::ai::catalogo::GRANITE_107M_MULTILINGUAL;
    let red = Network::new(Box::new(net::UreqTransport::new()));

    let mut resultado = instalar(&db, &red, &tmp.0, modelo).expect("primer intento");
    // La red de distribución tiene nombre por región: se autoriza la que pida.
    // Con tope: una redirección que pidiera un anfitrión nuevo cada vez no puede
    // dejar la prueba dando vueltas.
    for _ in 0..3 {
        let Instalacion::FaltaPermiso { host } = resultado else {
            break;
        };
        eprintln!("autorizando el anfitrión al que redirige: {host}");
        net::grant(&db.connection().expect("conexión"), &host, Purpose::Model).expect("permiso");
        resultado = instalar(&db, &red, &tmp.0, modelo).expect("reintento");
    }

    assert_eq!(resultado, Instalacion::Instalado);
    assert!(instalacion::instalado(&tmp.0, modelo));
}
