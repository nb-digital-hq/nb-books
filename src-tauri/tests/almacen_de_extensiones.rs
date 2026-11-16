//! El almacén inmutable de extensiones y sus generaciones.
//!
//! El criterio de aceptación dice que una actualización que rompe el arranque
//! se revierte **sin estado intermedio corrupto**, y que eso se comprueba
//! matando el proceso a mitad de la instalación. Aquí se mata de verdad: el
//! binario de pruebas se relanza a sí mismo, instala, y se aborta en un punto
//! nombrado. Un error simulado no probaría lo mismo, porque desenrolla la pila
//! y ejecuta los `Drop`; un corte de luz no hace ninguna de las dos cosas.

use std::path::{Path, PathBuf};
use std::process::Command;

use nb_books_lib::extensions::store::{huella_del_directorio, Store, PUNTO_DE_MUERTE_ENV};

/// Variable con la que el padre le dice al hijo dónde trabajar.
const HIJO_ENV: &str = "NB_BOOKS_PRUEBA_ALMACEN_HIJO";

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-almacen-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

/// Un paquete de extensión mínimo pero válido.
fn paquete(en: &Path, id: &str, version: &str) -> PathBuf {
    let dir = en.join(format!("paquete-{}", id.replace('.', "-")));
    std::fs::create_dir_all(dir.join("wasm")).expect("paquete");
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
    std::fs::write(dir.join("wasm").join("modulo.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    dir
}

#[test]
fn instalar_deja_el_contenido_bajo_su_huella_y_una_generacion_nueva() {
    let raiz = temporal("instalar");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.uno", "1.0.0");

    let instalada = almacen.instalar(&origen).expect("se instala");

    assert_eq!(instalada.id, "org.ejemplo.uno");
    assert_eq!(
        instalada.hash,
        huella_del_directorio(&origen).expect("huella")
    );
    assert!(almacen
        .ruta_de(&instalada)
        .join("nb-extension.json")
        .is_file());

    let perfil = almacen.perfil().expect("perfil");
    assert_eq!(perfil.generation, 1);
    assert_eq!(perfil.extensions, vec![instalada]);
}

#[test]
fn el_mismo_contenido_es_la_misma_carpeta_y_reinstalarlo_no_copia_nada() {
    let raiz = temporal("mismo");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.dos", "1.0.0");

    let primera = almacen.instalar(&origen).expect("primera");
    let marca = almacen.ruta_de(&primera).join("nb-extension.json");
    let cuando = std::fs::metadata(&marca)
        .expect("metadatos")
        .modified()
        .ok();

    let segunda = almacen.instalar(&origen).expect("segunda");

    assert_eq!(
        primera.hash, segunda.hash,
        "el mismo contenido, la misma huella"
    );
    assert_eq!(
        std::fs::metadata(&marca)
            .expect("metadatos")
            .modified()
            .ok(),
        cuando,
        "el contenido del almacén es inmutable: reinstalar no lo reescribe"
    );
}

#[test]
fn los_mismos_archivos_en_sitios_distintos_no_son_el_mismo_paquete() {
    let raiz = temporal("rutas");
    let uno = raiz.join("uno");
    let otro = raiz.join("otro");
    std::fs::create_dir_all(uno.join("c")).expect("uno");
    std::fs::create_dir_all(&otro).expect("otro");

    // Los dos llevan los mismos bytes **y en el mismo orden**: "1" y luego "2".
    // Se construye así a propósito. La primera versión de esta prueba ponía el
    // módulo en `wasm/modulo.wasm` frente a `modulo.wasm`, y entonces el orden
    // alfabético ya difería, así que el contenido entraba en distinto orden y la
    // huella cambiaba sola: la prueba pasaba **por el motivo equivocado** y
    // seguía verde al quitar la ruta del cálculo. Lo encontró una mutación.
    std::fs::write(uno.join("a.txt"), b"1").expect("a");
    std::fs::write(uno.join("c").join("b.txt"), b"2").expect("b");
    std::fs::write(otro.join("a.txt"), b"1").expect("a");
    std::fs::write(otro.join("b.txt"), b"2").expect("b");

    // Sin la ruta dentro de la huella, mover un módulo de carpeta no la
    // cambiaría, y el almacén serviría el contenido viejo creyendo que es el
    // nuevo.
    assert_ne!(
        huella_del_directorio(&uno).expect("huella"),
        huella_del_directorio(&otro).expect("huella")
    );
}

#[test]
fn un_manifiesto_invalido_no_entra_al_almacen_ni_a_medias() {
    let raiz = temporal("invalido");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.cuatro", "1.0.0");
    std::fs::write(origen.join("nb-extension.json"), "{\"name\": \"sin id\"}").expect("roto");

    assert!(almacen.instalar(&origen).is_err());
    assert_eq!(almacen.perfil().expect("perfil").generation, 0);
    assert_eq!(
        std::fs::read_dir(raiz.join("extensiones").join("store"))
            .expect("almacén")
            .count(),
        0,
        "se valida antes de copiar, así que no queda ni un archivo"
    );
}

#[test]
fn una_extension_de_una_api_que_no_hablamos_no_se_instala() {
    let raiz = temporal("api-futura");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let origen = paquete(&raiz, "org.ejemplo.futura", "1.0.0");
    let manifiesto = serde_json::json!({
        "id": "org.ejemplo.futura",
        "name": "De otro siglo",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "99.0",
        "license": "Apache-2.0"
    });
    std::fs::write(
        origen.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");

    // Podría instalarse y fallar al invocarla, pero eso sería dejar entrar algo
    // que no puede funcionar y descubrirlo más tarde.
    let error = almacen.instalar(&origen).expect_err("no se instala");
    assert_eq!(error.code(), "EXTENSION_API_UNSUPPORTED");
    assert_eq!(almacen.perfil().expect("perfil").generation, 0);
}

#[test]
fn no_haber_a_donde_volver_se_dice_con_su_propio_codigo() {
    let raiz = temporal("codigo-atras");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.unica", "1.0.0"))
        .expect("instala");

    // Antes esto era DOCUMENT_NOT_FOUND, que es de documentos y
    // dejaba al usuario leyendo "no se encuentra ese documento" cuando lo que
    // pasaba era otra cosa.
    let error = almacen.volver_atras().expect_err("no hay a dónde");
    assert_eq!(error.code(), "EXTENSION_ROLLBACK_UNAVAILABLE");
}

#[test]
fn desinstalar_no_borra_el_contenido_porque_volver_atras_lo_necesita() {
    let raiz = temporal("desinstalar");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let instalada = almacen
        .instalar(&paquete(&raiz, "org.ejemplo.cinco", "1.0.0"))
        .expect("instala");

    assert!(almacen
        .desinstalar("org.ejemplo.cinco")
        .expect("desinstala"));
    assert!(almacen.perfil().expect("perfil").extensions.is_empty());
    assert!(
        almacen.ruta_de(&instalada).is_dir(),
        "el contenido sigue: quitarla de la generación no es borrarla del almacén"
    );
}

#[test]
fn volver_atras_recupera_la_generacion_anterior_entera() {
    let raiz = temporal("atras");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let vieja = almacen
        .instalar(&paquete(&raiz, "org.ejemplo.seis", "1.0.0"))
        .expect("1.0.0");
    let nueva = almacen
        .instalar(&paquete(&raiz, "org.ejemplo.seis", "2.0.0"))
        .expect("2.0.0");
    assert_eq!(almacen.perfil().expect("perfil").extensions, vec![nueva]);

    let vuelta = almacen.volver_atras().expect("hay a dónde volver");

    assert_eq!(vuelta.generation, 1);
    assert_eq!(vuelta.extensions, vec![vieja]);
    assert_eq!(almacen.perfil().expect("perfil"), vuelta);
}

#[test]
fn sin_generacion_anterior_no_hay_a_donde_volver_y_se_dice() {
    let raiz = temporal("sin-atras");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.siete", "1.0.0"))
        .expect("instala");
    assert!(almacen.volver_atras().is_err());
}

#[test]
fn la_recoleccion_solo_borra_lo_que_no_referencia_ninguna_generacion() {
    let raiz = temporal("recoleccion");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.ocho", "1.0.0"))
        .expect("1.0.0");
    almacen
        .instalar(&paquete(&raiz, "org.ejemplo.ocho", "2.0.0"))
        .expect("2.0.0");

    // Las dos versiones siguen en el almacén: la 1.0.0 la referencia la
    // generación 1, a la que se puede volver.
    assert_eq!(almacen.recolectar().expect("recolecta"), 0);

    // Un resto que no referencia nadie, como el que deja una instalación
    // interrumpida antes de activar.
    let huerfano = raiz.join("extensiones").join("store").join("huerfano");
    std::fs::create_dir_all(&huerfano).expect("huérfano");
    assert_eq!(almacen.recolectar().expect("recolecta"), 1);
    assert!(!huerfano.exists());
}

// ---------------------------------------------------------------------------
// Matar el proceso a mitad de una instalación
// ---------------------------------------------------------------------------

/// El hijo: instala y muere donde le digan. No hace nada si nadie lo invoca.
#[test]
fn hijo_que_instala_y_muere() {
    let Ok(raiz) = std::env::var(HIJO_ENV) else {
        return;
    };
    let raiz = PathBuf::from(raiz);
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let _ = almacen.instalar(&raiz.join("paquete-org-ejemplo-muerte"));
    panic!("tenía que haber muerto antes de llegar aquí");
}

/// Lanza al hijo con el punto de muerte puesto y devuelve si murió de verdad.
fn matar_instalando(raiz: &Path, punto: &str) -> bool {
    let salida = Command::new(std::env::current_exe().expect("binario de pruebas"))
        .args(["--exact", "hijo_que_instala_y_muere", "--test-threads", "1"])
        .env(HIJO_ENV, raiz)
        .env(PUNTO_DE_MUERTE_ENV, punto)
        .output()
        .expect("se lanza el hijo");
    // Abortar no es salir: el proceso no tiene código de salida propio.
    !salida.status.success()
}

#[test]
fn matar_el_proceso_tras_copiar_no_deja_nada_puesto() {
    let raiz = temporal("muerte-copiando");
    Store::abrir(raiz.join("extensiones")).expect("almacén");
    paquete(&raiz, "org.ejemplo.muerte", "1.0.0");

    assert!(
        matar_instalando(&raiz, "tras-copiar"),
        "el hijo tenía que morir"
    );

    let almacen = Store::abrir(raiz.join("extensiones")).expect("se vuelve a abrir");
    let perfil = almacen.perfil().expect("perfil");
    assert_eq!(
        perfil.generation, 0,
        "murió antes del punto de compromiso, así que no hay nada puesto"
    );
    assert!(perfil.extensions.is_empty());

    // Lo que dejó es contenido en el almacén que no referencia nadie: aditivo,
    // inofensivo, y lo retira la recolección.
    assert_eq!(almacen.recolectar().expect("recolecta"), 1);
}

#[test]
fn matar_el_proceso_tras_activar_deja_la_instalacion_hecha_y_el_historico_se_repara() {
    let raiz = temporal("muerte-activando");
    Store::abrir(raiz.join("extensiones")).expect("almacén");
    paquete(&raiz, "org.ejemplo.muerte", "1.0.0");

    assert!(
        matar_instalando(&raiz, "tras-activar"),
        "el hijo tenía que morir"
    );

    // El perfil es una sola escritura atómica, así que está entero.
    let historico = raiz.join("extensiones").join("generations").join("1.json");
    assert!(
        !historico.exists(),
        "murió antes de archivar, así que el histórico todavía no está"
    );

    let almacen = Store::abrir(raiz.join("extensiones")).expect("se vuelve a abrir");
    let perfil = almacen.perfil().expect("perfil");
    assert_eq!(perfil.generation, 1);
    assert_eq!(perfil.extensions.len(), 1);
    assert!(
        historico.exists(),
        "el histórico es derivado del perfil y se rehace al abrir"
    );
    assert_eq!(almacen.generaciones().expect("generaciones"), vec![1]);
}

#[test]
fn el_almacen_del_usuario_vive_en_el_directorio_de_datos() {
    // Es seguro tocar el entorno aquí, y conviene decir por qué: una variable
    // es del proceso y no del hilo, así que dos pruebas que la usen se pisan.
    // En este archivo **ninguna otra** resuelve rutas de la aplicación; todas
    // las demás abren el almacén en una raíz que se les pasa.
    let base = temporal("del-usuario");
    std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &base);

    let almacen = Store::del_usuario().expect("almacén del usuario");
    almacen
        .instalar(&paquete(&base, "org.ejemplo.nueve", "1.0.0"))
        .expect("instala");

    let esperado = base.join("data").join("extensiones");
    assert!(
        esperado.join("active_profile.json").is_file(),
        "el almacén tiene que quedar bajo el directorio de datos, y quedó en otro sitio"
    );

    std::env::remove_var(nb_books_lib::paths::TEST_DIR_ENV);
}

/// **Regresión.** Un paquete que declara un módulo y no lo trae no entra.
///
/// El 2026-09-12 las extensiones propias se instalaron desde su carpeta de
/// fuentes, sin el `.wasm` compilado, y entraron sin queja: cada uso respondía
/// "no se encontró el archivo". Un archivo con el nombre bueno y otro contenido
/// es igual de inútil, así que tampoco entra. La tercera mitad es que el paquete
/// completo sí entra: sin ella, una instalación que lo rechazara todo pasaría.
#[test]
fn un_paquete_sin_el_modulo_que_declara_no_se_instala() {
    let raiz = temporal("sin-modulo");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let dir = raiz.join("incompleta");
    std::fs::create_dir_all(&dir).expect("paquete");
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::json!({
            "id": "org.ejemplo.incompleta",
            "name": "Incompleta",
            "version": "1.0.0",
            "type": "core",
            "apiVersion": "0.1",
            "license": "Apache-2.0",
            "contributes": { "providers": { "bulk-metadata": "revisor.wasm" } }
        })
        .to_string(),
    )
    .expect("manifiesto");

    let rechazo = Err(nb_books_lib::error::NbError::ExtensionManifestInvalid);
    assert_eq!(almacen.instalar(&dir).map(|_| ()), rechazo, "sin el módulo");

    std::fs::write(dir.join("revisor.wasm"), b"no soy un componente").expect("impostor");
    assert_eq!(
        almacen.instalar(&dir).map(|_| ()),
        rechazo,
        "con otro contenido"
    );

    std::fs::write(dir.join("revisor.wasm"), b"\0asm\x01\0\0\0").expect("módulo");
    let instalada = almacen.instalar(&dir).expect("completa, entra");
    assert!(almacen
        .perfil()
        .expect("perfil")
        .extensions
        .iter()
        .any(|e| e.id == instalada.id));
}
