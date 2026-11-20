//! La firma del autor y la clave fijada.
//!
//! # Qué protege esto, exactamente
//!
//! No a la primera instalación: una firma válida solo dice que quien firmó
//! tenía la clave que viene al lado, y cualquiera puede generar un par de
//! claves. Lo que protege es **la actualización**, que es donde vive el vector
//! clásico de la política del registro: la extensión benigna que se vuelve
//! maliciosa en una versión posterior. Con la clave fijada, conseguirlo pide
//! robársela al autor.
//!
//! Por eso las pruebas de aquí abajo se leen en pares: lo que pasa la primera
//! vez y lo que pasa la segunda.

use std::path::{Path, PathBuf};

use nb_books_lib::error::NbError;
use nb_books_lib::extensions::signature::{self, SignatureFault, SignatureState};
use nb_books_lib::extensions::store::{huella_del_directorio, Store};

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-firma-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

/// Un paquete mínimo y válido, con el contenido que se le pida.
fn paquete(en: &Path, id: &str, version: &str, modulo: &[u8]) -> PathBuf {
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
    std::fs::write(dir.join("modulo.wasm"), modulo).expect("módulo");
    dir
}

fn firmar_en(paquete: &Path, id: &str, version: &str, clave: &[u8]) {
    let firma = signature::firmar(paquete, id, version, clave).expect("firmar");
    std::fs::write(
        paquete.join(signature::ARCHIVO),
        serde_json::to_string(&firma).expect("json"),
    )
    .expect("escribir la firma");
}

// ---------------------------------------------------------------------------
// La firma en sí
// ---------------------------------------------------------------------------

#[test]
fn lo_que_se_firma_se_verifica_con_la_clave_publica_de_quien_firmo() {
    let raiz = temporal("ida-y-vuelta");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let clave = signature::crear_clave().expect("clave");

    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);

    assert_eq!(
        signature::verificar(&dir, "com.ejemplo.uno", "1.0.0").expect("verificar"),
        SignatureState::Valida {
            clave: signature::clave_publica(&clave).expect("pública")
        }
    );
}

#[test]
fn cambiar_un_solo_byte_del_paquete_invalida_la_firma() {
    let raiz = temporal("un-byte");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let clave = signature::crear_clave().expect("clave");
    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);

    // Un byte, y en un archivo que el manifiesto ni menciona: la firma cubre la
    // huella entera, no solo lo que alguien se acordó de nombrar.
    std::fs::write(dir.join("modulo.wasm"), b"\0asm\x01\0\0\x01").expect("cambiar");

    assert_eq!(
        signature::verificar(&dir, "com.ejemplo.uno", "1.0.0").expect("verificar"),
        SignatureState::Invalida {
            motivo: SignatureFault::NoCorresponde
        }
    );
}

#[test]
fn una_firma_no_se_puede_mover_a_otra_extension_ni_a_otra_version() {
    // El contenido puede ser idéntico y aun así no es lo mismo: lo firmado
    // incluye el identificador y la versión, así que una firma legítima de un
    // paquete no vale para otro que se llame distinto.
    let raiz = temporal("mover");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let clave = signature::crear_clave().expect("clave");
    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);
    let texto = std::fs::read_to_string(dir.join(signature::ARCHIVO)).expect("firma");
    let huella = huella_del_directorio(&dir).expect("huella");

    assert!(matches!(
        signature::comprobar(&texto, "com.ejemplo.uno", "1.0.0", &huella),
        SignatureState::Valida { .. }
    ));
    assert_eq!(
        signature::comprobar(&texto, "com.ejemplo.otra", "1.0.0", &huella),
        SignatureState::Invalida {
            motivo: SignatureFault::NoCorresponde
        }
    );
    assert_eq!(
        signature::comprobar(&texto, "com.ejemplo.uno", "2.0.0", &huella),
        SignatureState::Invalida {
            motivo: SignatureFault::NoCorresponde
        }
    );
}

#[test]
fn firmar_no_cambia_la_direccion_de_contenido_del_paquete() {
    // Si la firma entrara en la huella, se estaría firmando algo que cambia al
    // firmarlo. Además, así el mismo paquete ocupa la misma carpeta del almacén
    // antes y después de firmarse, y volver a firmarlo no lo mueve de sitio.
    let raiz = temporal("huella");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let antes = huella_del_directorio(&dir).expect("huella");

    let clave = signature::crear_clave().expect("clave");
    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);

    assert_eq!(huella_del_directorio(&dir).expect("huella"), antes);
}

#[test]
fn una_firma_de_otro_algoritmo_no_se_intenta_verificar() {
    let raiz = temporal("algoritmo");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let huella = huella_del_directorio(&dir).expect("huella");
    let texto = serde_json::json!({
        "algoritmo": "rsa-2048",
        "clave": "00".repeat(32),
        "firma": "00".repeat(64)
    })
    .to_string();

    assert_eq!(
        signature::comprobar(&texto, "com.ejemplo.uno", "1.0.0", &huella),
        SignatureState::Invalida {
            motivo: SignatureFault::AlgoritmoDesconocido
        }
    );
}

#[test]
fn una_clave_del_largo_que_no_es_se_rechaza_por_su_forma() {
    let raiz = temporal("largo");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let huella = huella_del_directorio(&dir).expect("huella");
    let texto = serde_json::json!({
        "algoritmo": "ed25519",
        "clave": "00".repeat(31),
        "firma": "00".repeat(64)
    })
    .to_string();

    assert_eq!(
        signature::comprobar(&texto, "com.ejemplo.uno", "1.0.0", &huella),
        SignatureState::Invalida {
            motivo: SignatureFault::FormatoIlegible
        }
    );
}

#[test]
fn una_clave_mas_larga_de_la_cuenta_no_se_acepta_recortandola() {
    // Esta prueba existe porque la de arriba pasaba por el motivo equivocado.
    // Al quitar la comprobación de largo, una clave **corta** se sigue
    // rechazando sola —al leerla se acaba el texto—, así que la mutación no
    // tumbaba nada. Lo que la comprobación de largo evita de verdad es lo
    // contrario: una clave **larga**, de la que se leerían los 32 primeros
    // bytes y se ignoraría el resto en silencio.
    let raiz = temporal("larga");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let clave = signature::crear_clave().expect("clave");
    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);

    let texto = std::fs::read_to_string(dir.join(signature::ARCHIVO)).expect("firma");
    let huella = huella_del_directorio(&dir).expect("huella");
    let mut firma: serde_json::Value = serde_json::from_str(&texto).expect("json");
    let publica = signature::clave_publica(&clave).expect("pública");
    firma["clave"] = serde_json::Value::String(format!("{publica}00"));

    assert_eq!(
        signature::comprobar(&firma.to_string(), "com.ejemplo.uno", "1.0.0", &huella),
        SignatureState::Invalida {
            motivo: SignatureFault::FormatoIlegible
        },
        "la clave lleva dos caracteres de más, y con ellos ya no es esa clave"
    );
}

// ---------------------------------------------------------------------------
// La clave fijada, que es lo que protege la actualización
// ---------------------------------------------------------------------------

#[test]
fn instalar_algo_firmado_fija_la_clave_de_quien_lo_firmo() {
    let raiz = temporal("fijar");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let clave = signature::crear_clave().expect("clave");
    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);

    let instalada = almacen.instalar(&dir).expect("se instala");

    assert_eq!(
        instalada.key.as_deref(),
        Some(signature::clave_publica(&clave).expect("pública").as_str())
    );
}

#[test]
fn la_actualizacion_del_mismo_autor_entra_y_la_de_otro_no() {
    let raiz = temporal("actualizar");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let autor = signature::crear_clave().expect("clave");
    let impostor = signature::crear_clave().expect("clave");

    let primera = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    firmar_en(&primera, "com.ejemplo.uno", "1.0.0", &autor);
    almacen.instalar(&primera).expect("primera");

    // La misma extensión, la misma versión de API, el mismo todo salvo quién
    // firma. Es la única diferencia, y es la que decide.
    let suplantada = paquete(&raiz, "com.ejemplo.uno", "2.0.0", b"\0asm\x01\0\0\0");
    firmar_en(&suplantada, "com.ejemplo.uno", "2.0.0", &impostor);
    assert!(matches!(
        almacen.instalar(&suplantada),
        Err(NbError::ExtensionSignatureMismatch)
    ));

    let legitima = paquete(&raiz, "com.ejemplo.uno", "3.0.0", b"\0asm\x01\0\0\0");
    firmar_en(&legitima, "com.ejemplo.uno", "3.0.0", &autor);
    let instalada = almacen.instalar(&legitima).expect("la del autor sí entra");
    assert_eq!(instalada.version, "3.0.0");
}

#[test]
fn lo_que_estaba_firmado_no_se_actualiza_con_algo_sin_firmar() {
    // Quitar la firma no puede ser la forma de saltarse la firma.
    let raiz = temporal("sin-firmar");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let autor = signature::crear_clave().expect("clave");

    let primera = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    firmar_en(&primera, "com.ejemplo.uno", "1.0.0", &autor);
    almacen.instalar(&primera).expect("primera");

    let desnuda = paquete(&raiz, "com.ejemplo.uno", "2.0.0", b"\0asm\x01\0\0\0");
    assert!(matches!(
        almacen.instalar(&desnuda),
        Err(NbError::ExtensionSignatureMismatch)
    ));
}

#[test]
fn lo_que_nunca_estuvo_firmado_se_instala_y_se_actualiza_igual() {
    // El control de las dos anteriores. Instalar desde el disco propio algo sin
    // firmar es legítimo -así entran las extensiones que uno escribe- y
    // prohibirlo no protegería de nada: quien pone el paquete es quien lo
    // instala.
    let raiz = temporal("nunca-firmada");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");

    let primera = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let instalada = almacen.instalar(&primera).expect("primera");
    assert_eq!(instalada.key, None);

    let segunda = paquete(&raiz, "com.ejemplo.uno", "2.0.0", b"\0asm\x01\0\0\0");
    assert_eq!(
        almacen.instalar(&segunda).expect("segunda").version,
        "2.0.0"
    );
}

#[test]
fn una_firma_rota_no_entra_ni_siquiera_la_primera_vez() {
    // Traer una firma que no vale no es lo mismo que no traer ninguna: alguien
    // quiso firmar esto y lo que hay no corresponde al contenido.
    let raiz = temporal("rota");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let dir = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    let clave = signature::crear_clave().expect("clave");
    firmar_en(&dir, "com.ejemplo.uno", "1.0.0", &clave);
    std::fs::write(dir.join("modulo.wasm"), b"\0asm\x01\0\0\x01").expect("cambiar");

    assert!(matches!(
        almacen.instalar(&dir),
        Err(NbError::ExtensionSignatureMismatch)
    ));
    assert!(almacen.perfil().expect("perfil").extensions.is_empty());
}

#[test]
fn la_extension_rechazada_no_deja_nada_en_el_almacen() {
    // Se comprueba la firma **antes** de copiar: rechazar después dejaría el
    // contenido en el almacén y obligaría a recogerlo.
    let raiz = temporal("sin-basura");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let autor = signature::crear_clave().expect("clave");
    let impostor = signature::crear_clave().expect("clave");

    let primera = paquete(&raiz, "com.ejemplo.uno", "1.0.0", b"\0asm\x01\0\0\0");
    firmar_en(&primera, "com.ejemplo.uno", "1.0.0", &autor);
    almacen.instalar(&primera).expect("primera");
    let generaciones_antes = almacen.generaciones().expect("generaciones").len();

    let suplantada = paquete(&raiz, "com.ejemplo.uno", "2.0.0", b"\0asm\x01\0\0\0");
    firmar_en(&suplantada, "com.ejemplo.uno", "2.0.0", &impostor);
    let huella = huella_del_directorio(&suplantada).expect("huella");
    let _ = almacen.instalar(&suplantada);

    assert_eq!(
        almacen.generaciones().expect("generaciones").len(),
        generaciones_antes,
        "un rechazo no crea una generación"
    );
    assert!(
        !raiz
            .join("extensiones")
            .join("contenido")
            .join(&huella)
            .exists(),
        "ni deja el contenido copiado"
    );
}
