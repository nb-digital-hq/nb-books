//! Una vista aportada por una extensión.
//!
//! El criterio de aceptación tiene dos mitades. La primera —que una extensión
//! puede montar interfaz— se comprueba componiendo una vista de verdad. La
//! segunda —que **una extensión que entra en bucle no congela la aplicación**—
//! se comprueba con una vista que no devuelve el control nunca: si el
//! aislamiento no funcionara, esta prueba no volvería.

mod comun;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use comun::{componente_construido, OBJETIVO_COMPONENTE};
use nb_books_lib::db::Database;
use nb_books_lib::extensions::registry::{Entorno, Registro};
use nb_books_lib::extensions::store::Store;
use nb_books_lib::extensions::view::Block;
use nb_books_lib::extensions::{Limites, Motor};
use nb_books_lib::net::{Network, UreqTransport};

const QUIEN: &str = "lat.nbdigitallabs.books.banco.vista-de-estudio";

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-vista-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

fn paquete(en: &Path) -> Option<PathBuf> {
    let construido = componente_construido()?;
    let origen = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raíz")
        .join("extensiones")
        .join("banco")
        .join("vista-de-estudio");

    let dir = en.join("paquete-vista");
    std::fs::create_dir_all(&dir).expect("paquete");
    std::fs::copy(
        origen.join("nb-extension.json"),
        dir.join("nb-extension.json"),
    )
    .expect("manifiesto");
    std::fs::copy(
        construido.join("vista_de_estudio.wasm"),
        dir.join("vista_de_estudio.wasm"),
    )
    .expect("módulo");
    Some(dir)
}

struct Montado {
    almacen: Store,
    motor: Motor,
    db: Database,
    red: Arc<Network>,
}

fn montar(nombre: &str) -> Option<Montado> {
    let raiz = temporal(nombre);
    let paquete = paquete(&raiz)?;
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen.instalar(&paquete).expect("instala");
    Some(Montado {
        almacen,
        motor: Motor::nuevo().expect("motor"),
        db: Database::open_in_memory().expect("base"),
        red: Arc::new(Network::new(Box::new(UreqTransport::new()))),
    })
}

#[test]
fn las_pestanas_de_una_extension_estan_antes_de_componer_nada() {
    let Some(m) = montar("pestanas") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let registro = Registro::cargar(&m.almacen).expect("registro");

    // Salen del manifiesto. En este punto no se ha compilado nada, y `vistas`
    // no recibe motor con el que hacerlo.
    let vistas = registro.vistas();
    assert_eq!(vistas.len(), 2);
    assert_eq!(vistas[0].0, QUIEN);
    assert_eq!(vistas[0].1.title, "La Que Se Cuelga");
    assert_eq!(vistas[1].1.title, "Resumen de Prueba");
}

#[test]
fn una_extension_compone_su_vista_con_las_cuatro_clases_de_bloque() {
    let Some(m) = montar("compone") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let registro = Registro::cargar(&m.almacen).expect("registro");
    let entorno = Entorno {
        motor: &m.motor,
        almacen: &m.almacen,
        db: &m.db,
        red: &m.red,
    };

    let vista = registro
        .componer_vista(
            &entorno,
            QUIEN,
            "vista-de-estudio.resumen",
            Limites::default(),
        )
        .expect("compone");

    assert_eq!(vista.title, "Resumen de prueba");
    // Las cuatro clases, y en orden. Comprobar solo la primera dejaría sin
    // cubrir tres cuartas partes del contrato.
    assert!(matches!(vista.blocks[0], Block::Heading(_)));
    assert!(matches!(vista.blocks[1], Block::Paragraph(_)));
    match &vista.blocks[2] {
        Block::Items(lineas) => assert_eq!(lineas.len(), 2),
        otro => panic!("tenía que ser una lista: {otro:?}"),
    }
    match &vista.blocks[3] {
        Block::Field((etiqueta, _)) => assert_eq!(etiqueta, "Origen"),
        otro => panic!("tenía que ser un dato con etiqueta: {otro:?}"),
    }
}

/// **La segunda mitad del criterio.**
///
/// Si el aislamiento no funcionara, esta prueba no volvería nunca. Que vuelva
/// es el resultado.
#[test]
fn una_vista_que_entra_en_bucle_no_congela_a_quien_la_pide() {
    let Some(m) = montar("bucle") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    let registro = Registro::cargar(&m.almacen).expect("registro");
    let entorno = Entorno {
        motor: &m.motor,
        almacen: &m.almacen,
        db: &m.db,
        red: &m.red,
    };
    let limites = Limites {
        tiempo: Duration::from_millis(200),
        ..Limites::default()
    };

    let reloj = Instant::now();
    let resultado = registro.componer_vista(&entorno, QUIEN, "vista-de-estudio.colgada", limites);
    let tardado = reloj.elapsed();

    let error = resultado.expect_err("una vista que no vuelve no puede dar una vista");
    assert_eq!(error.code(), "EXTENSION_LIMIT_EXCEEDED");
    assert!(
        tardado < Duration::from_secs(5),
        "tardó {tardado:?} en cortar un plazo de 200 ms"
    );

    // Y el subsistema sigue sirviendo: la otra vista se compone justo después.
    assert!(registro
        .componer_vista(
            &entorno,
            QUIEN,
            "vista-de-estudio.resumen",
            Limites::default()
        )
        .is_ok());
}

#[test]
fn una_vista_que_el_manifiesto_no_declara_se_rechaza_sin_tocar_su_modulo() {
    // El paquete declara una vista y apunta a un módulo **que no está**. Eso es
    // lo que hace la prueba capaz de fallar: si la comprobación del manifiesto
    // no estuviera, el rechazo vendría de no poder leer el módulo y traería
    // otro código. Con la comprobación, ni se llega a mirar el archivo.
    let raiz = temporal("ajena");
    let dir = raiz.join("paquete-sin-modulo");
    std::fs::create_dir_all(&dir).expect("paquete");
    let manifiesto = serde_json::json!({
        "id": "org.ejemplo.declara-una",
        "name": "Declara una vista",
        "version": "1.0.0",
        "type": "ui",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": {
            "views": [{ "id": "org.ejemplo.declara-una.esta", "title": "Esta sí" }],
            "providers": { "view": "no-existe.wasm" }
        }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");

    // Ahora un paquete sin su módulo no se instala, así que se instala con
    // uno -basta la firma de WebAssembly, que es lo que se mira- y se retira del
    // almacén después: el módulo tiene que **no estar** al invocar.
    std::fs::write(dir.join("no-existe.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let instalada = almacen.instalar(&dir).expect("instala");
    std::fs::remove_file(almacen.ruta_de(&instalada).join("no-existe.wasm")).expect("se retira");
    let motor = Motor::nuevo().expect("motor");
    let db = Database::open_in_memory().expect("base");
    let red = Arc::new(Network::new(Box::new(UreqTransport::new())));
    let registro = Registro::cargar(&almacen).expect("registro");
    let entorno = Entorno {
        motor: &motor,
        almacen: &almacen,
        db: &db,
        red: &red,
    };

    // Lo que el usuario aprobó al instalar es lo que el manifiesto decía.
    let error = registro
        .componer_vista(
            &entorno,
            "org.ejemplo.declara-una",
            "vista.que.no.existe",
            Limites::default(),
        )
        .expect_err("no se compone");
    assert_eq!(error.code(), "COMPONENT_UNAVAILABLE");

    // Y la que sí declara llega hasta el módulo, que no está: otro código.
    let error = registro
        .componer_vista(
            &entorno,
            "org.ejemplo.declara-una",
            "org.ejemplo.declara-una.esta",
            Limites::default(),
        )
        .expect_err("su módulo no está");
    assert_eq!(error.code(), "FILE_NOT_FOUND");
}

#[test]
fn una_extension_desactivada_no_aporta_sus_vistas() {
    let Some(m) = montar("apagada") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO_COMPONENTE}");
        return;
    };
    m.almacen.habilitar(QUIEN, false).expect("apaga");
    let registro = Registro::cargar(&m.almacen).expect("registro");
    assert!(registro.vistas().is_empty());
}
