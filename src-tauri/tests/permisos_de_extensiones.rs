//! Permisos de extensión y su consentimiento.
//!
//! El criterio de aceptación son tres cosas, y las tres se comprueban aquí:
//! que una extensión que amplía sus permisos no se actualiza sin intervención
//! del usuario, que revocarle la red la deja inerte sin romper nada, y que su
//! salida aparece en el registro **con la extensión identificada
//! como origen**.
//!
//! A las tres se añade la que las sostiene y que ninguna dice en voz alta:
//! conceder a una extensión no concede a otra.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nb_books_lib::db::Database;
use nb_books_lib::extensions::manifest::{self, Manifest};
use nb_books_lib::extensions::permissions::{self, Instalacion};
use nb_books_lib::extensions::store::Store;
use nb_books_lib::net::{self, HttpTransport, Network, Origin, Purpose, Response};

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-permisos-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

/// Un paquete que pide salir a los anfitriones que se le digan.
fn paquete(en: &Path, id: &str, version: &str, anfitriones: &[&str]) -> PathBuf {
    let dir = en.join(format!("paquete-{}-{version}", id.replace('.', "-")));
    std::fs::create_dir_all(&dir).expect("paquete");
    let mut manifiesto = serde_json::json!({
        "id": id,
        "name": "Proveedor de prueba",
        "version": version,
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0"
    });
    if !anfitriones.is_empty() {
        let urls: Vec<String> = anfitriones
            .iter()
            .map(|h| format!("https://{h}/api/*"))
            .collect();
        manifiesto["capabilities"] = serde_json::json!(["network:fetch"]);
        manifiesto["permissions"] = serde_json::json!({ "network:fetch": { "urls": urls } });
    }
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    std::fs::write(dir.join("modulo.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    dir
}

fn manifiesto_de(paquete: &Path) -> Manifest {
    let texto = std::fs::read_to_string(paquete.join(manifest::ARCHIVO)).expect("manifiesto");
    manifest::parse(&texto).expect("válido")
}

// ---------------------------------------------------------------------------

#[test]
fn una_extension_que_pide_red_no_entra_hasta_que_alguien_lo_apruebe() {
    let raiz = temporal("aprobar");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.pide", "1.0.0", &["datos.ejemplo.org"]);

    let intento = permissions::instalar(&conn, &almacen, &origen).expect("no falla");

    let Instalacion::FaltaConsentimiento(peticiones) = intento else {
        panic!("no tenía que instalarse sin permiso");
    };
    assert_eq!(peticiones.len(), 1);
    assert_eq!(peticiones[0].host, "datos.ejemplo.org");
    assert!(!peticiones[0].granted);
    // Y no se instaló nada: ni contenido ni generación.
    assert_eq!(almacen.perfil().expect("perfil").generation, 0);

    permissions::conceder(&conn, &manifiesto_de(&origen), "datos.ejemplo.org").expect("concede");
    let segundo = permissions::instalar(&conn, &almacen, &origen).expect("no falla");
    assert!(matches!(segundo, Instalacion::Instalada(_)));
}

#[test]
fn lo_que_se_le_ensena_al_usuario_dice_a_donde_y_que_se_envia() {
    let raiz = temporal("ensenar");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let origen = paquete(&raiz, "org.ejemplo.ensena", "1.0.0", &["uno.ejemplo.org"]);

    let peticiones = permissions::lo_que_pide(&conn, &manifiesto_de(&origen)).expect("pide");

    assert_eq!(peticiones[0].host, "uno.ejemplo.org");
    assert!(!peticiones[0].implies.is_empty());
    // La frase reconoce lo que no se puede prometer. Un texto que dijera "envía
    // el título de la obra" sería mentira: el código no es nuestro.
    assert!(
        peticiones[0].what_is_sent.contains("no podemos acotar"),
        "tiene que decir que no se puede acotar, y dice: {}",
        peticiones[0].what_is_sent
    );
}

/// La propiedad que sostiene a todas las demás.
#[test]
fn conceder_a_una_extension_no_concede_a_otra() {
    let raiz = temporal("dos");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");

    let una = paquete(&raiz, "org.ejemplo.una", "1.0.0", &["comun.ejemplo.org"]);
    let otra = paquete(&raiz, "org.ejemplo.otra", "1.0.0", &["comun.ejemplo.org"]);

    permissions::conceder(&conn, &manifiesto_de(&una), "comun.ejemplo.org").expect("concede");

    assert!(matches!(
        permissions::instalar(&conn, &almacen, &una).expect("no falla"),
        Instalacion::Instalada(_)
    ));
    // El mismo anfitrión, autorizado para la primera. La segunda sigue sin
    // permiso: si heredara, el usuario habría autorizado a algo que no vio.
    assert!(matches!(
        permissions::instalar(&conn, &almacen, &otra).expect("no falla"),
        Instalacion::FaltaConsentimiento(_)
    ));
}

#[test]
fn una_actualizacion_que_pide_un_anfitrion_nuevo_no_entra_sola() {
    let raiz = temporal("actualiza");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");

    let vieja = paquete(&raiz, "org.ejemplo.crece", "1.0.0", &["uno.ejemplo.org"]);
    permissions::conceder(&conn, &manifiesto_de(&vieja), "uno.ejemplo.org").expect("concede");
    permissions::instalar(&conn, &almacen, &vieja).expect("instala");

    let nueva = paquete(
        &raiz,
        "org.ejemplo.crece",
        "2.0.0",
        &["uno.ejemplo.org", "dos.ejemplo.org"],
    );
    let intento = permissions::instalar(&conn, &almacen, &nueva).expect("no falla");

    let Instalacion::FaltaConsentimiento(peticiones) = intento else {
        panic!("una versión que pide más no puede entrar sola");
    };
    // Se le enseña **todo** lo que podrá hacer, no solo la diferencia: decidir
    // sobre la diferencia sin ver el conjunto es decidir a ciegas.
    assert_eq!(peticiones.len(), 2);
    assert_eq!(peticiones.iter().filter(|p| p.granted).count(), 1);
    // Y la versión instalada sigue siendo la de antes.
    let perfil = almacen.perfil().expect("perfil");
    assert_eq!(perfil.extensions[0].version, "1.0.0");
}

#[test]
fn una_actualizacion_que_no_pide_mas_entra_sin_volver_a_preguntar() {
    let raiz = temporal("igual");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");

    let vieja = paquete(&raiz, "org.ejemplo.igual", "1.0.0", &["uno.ejemplo.org"]);
    permissions::conceder(&conn, &manifiesto_de(&vieja), "uno.ejemplo.org").expect("concede");
    permissions::instalar(&conn, &almacen, &vieja).expect("instala");

    let nueva = paquete(&raiz, "org.ejemplo.igual", "2.0.0", &["uno.ejemplo.org"]);
    assert!(matches!(
        permissions::instalar(&conn, &almacen, &nueva).expect("no falla"),
        Instalacion::Instalada(_)
    ));
    assert_eq!(
        almacen.perfil().expect("perfil").extensions[0].version,
        "2.0.0"
    );
}

#[test]
fn no_se_puede_conceder_un_anfitrion_que_la_extension_no_pide() {
    let raiz = temporal("ajeno");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let origen = paquete(&raiz, "org.ejemplo.ajeno", "1.0.0", &["uno.ejemplo.org"]);

    // Un permiso que no corresponde a nada que el usuario haya visto en la
    // lista no es un permiso: es un agujero con nombre de permiso.
    assert!(permissions::conceder(&conn, &manifiesto_de(&origen), "otro.ejemplo.org").is_err());
}

#[test]
fn desinstalar_se_lleva_los_permisos_para_que_reinstalar_vuelva_a_preguntar() {
    let raiz = temporal("desinstalar");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.va", "1.0.0", &["uno.ejemplo.org"]);

    permissions::conceder(&conn, &manifiesto_de(&origen), "uno.ejemplo.org").expect("concede");
    permissions::instalar(&conn, &almacen, &origen).expect("instala");

    assert!(permissions::desinstalar(&conn, &almacen, "org.ejemplo.va").expect("desinstala"));

    assert!(matches!(
        permissions::instalar(&conn, &almacen, &origen).expect("no falla"),
        Instalacion::FaltaConsentimiento(_)
    ));
}

// ---------------------------------------------------------------------------
// La salida, por la misma puerta y con nombre
// ---------------------------------------------------------------------------

/// Transporte que no sale: anota lo que le pidan.
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
            body: b"{}".to_vec(),
        })
    }
}

#[test]
fn revocarle_la_red_la_deja_inerte_y_la_salida_queda_con_su_nombre() {
    let raiz = temporal("registro");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.sale", "1.0.0", &["datos.ejemplo.org"]);
    permissions::conceder(&conn, &manifiesto_de(&origen), "datos.ejemplo.org").expect("concede");
    permissions::instalar(&conn, &almacen, &origen).expect("instala");

    let salidas = Arc::new(Mutex::new(Vec::new()));
    let red = Network::new(Box::new(Espia {
        salidas: Arc::clone(&salidas),
    }));
    let quien = permissions::origen("org.ejemplo.sale");

    red.get_from(
        &conn,
        &quien,
        "https://datos.ejemplo.org/api/obra",
        Purpose::Extension,
    )
    .expect("con permiso, sale");
    assert_eq!(salidas.lock().expect("candado").len(), 1);

    // El criterio: aparece en el registro con la extensión como
    // origen. Sin eso, el usuario ve que algo salió y no sabe qué lo mandó.
    let registro = net::log(&conn, 10).expect("registro");
    assert_eq!(registro[0].origin, "org.ejemplo.sale");
    assert_eq!(registro[0].host, "datos.ejemplo.org");
    assert_eq!(registro[0].purpose, "extension");

    // Revocada, queda inerte.
    permissions::revocar(&conn, "org.ejemplo.sale", "datos.ejemplo.org").expect("revoca");
    let rechazada = red.get_from(
        &conn,
        &quien,
        "https://datos.ejemplo.org/api/obra",
        Purpose::Extension,
    );
    assert!(rechazada.is_err());
    assert_eq!(
        salidas.lock().expect("candado").len(),
        1,
        "revocada no llega ni al transporte"
    );

    // Y no rompe nada: la biblioteca sigue ahí y la extensión también.
    assert_eq!(almacen.perfil().expect("perfil").extensions.len(), 1);
    assert!(db.connection().is_ok());
}

#[test]
fn la_aplicacion_y_una_extension_no_comparten_consentimiento() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    net::grant(&conn, "datos.ejemplo.org", Purpose::Extension).expect("a la aplicación");

    // Concedido a la aplicación, no a la extensión. Si se compartiera, el
    // origen no serviría para nada.
    assert!(!net::has_consent_from(
        &conn,
        &Origin::Extension("org.ejemplo.x".to_owned()),
        "datos.ejemplo.org",
        Purpose::Extension
    )
    .expect("consulta"));
}

/// **Lo que se dice cuando el paquete no trae su módulo** (2026-09-18).
///
/// Se reportó instalando las extensiones propias desde su carpeta de
/// fuentes: la aplicación contestaba "el manifiesto de la extensión no es
/// válido", así que mandaba a revisar un archivo que estaba perfecto. Lo que
/// falta es el `.wasm` compilado, y ahora se dice **con su nombre**.
///
/// Y se comprueba antes que los permisos a propósito: pedirle a alguien que
/// conceda salida a la red para algo que no puede ejecutarse es pedirle que
/// decida sobre lo que no va a pasar.
#[test]
fn a_un_paquete_sin_su_modulo_se_le_dice_que_le_falta_y_cual() {
    let raiz = temporal("incompleta");
    let almacen = Store::abrir(raiz.join("almacen")).expect("almacén");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    // Un paquete que declara un proveedor, con permisos de red y sin el módulo.
    let dir = raiz.join("fuentes");
    std::fs::create_dir_all(&dir).expect("paquete");
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::json!({
            "id": "org.ejemplo.fuentes",
            "name": "Desde las fuentes",
            "version": "1.0.0",
            "type": "core",
            "apiVersion": "0.1",
            "license": "Apache-2.0",
            "capabilities": ["network:fetch"],
            "permissions": { "network:fetch": { "urls": ["https://api.ejemplo/*"] } },
            "contributes": { "providers": { "bulk-metadata": "revisor.wasm" } }
        })
        .to_string(),
    )
    .expect("manifiesto");

    match permissions::instalar(&conn, &almacen, &dir).expect("responde") {
        Instalacion::Incompleta(faltan) => {
            assert_eq!(faltan, vec!["revisor.wasm".to_owned()], "dice cuál falta");
        }
        otra => panic!("se esperaba un paquete incompleto y llegó {otra:?}"),
    }

    // Y con el módulo dentro, sigue el camino de siempre: pide el permiso.
    std::fs::write(dir.join("revisor.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    assert!(
        matches!(
            permissions::instalar(&conn, &almacen, &dir).expect("responde"),
            Instalacion::FaltaConsentimiento(_)
        ),
        "con el módulo, lo que falta es el permiso"
    );
}
