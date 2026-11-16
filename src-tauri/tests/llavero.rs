//! El almacén de credenciales, contra el llavero **de verdad** del sistema.
//!
//! No hay simulacro aquí a propósito. Un doble de prueba comprobaría que se
//! llama a la librería, que es justo lo que no hace falta comprobar: lo que
//! puede fallar es el llavero real —que no haya, que pida desbloqueo, que
//! guarde y no devuelva— y eso solo se ve hablando con él.
//!
//! Por eso mismo, **estas pruebas se saltan solas donde no hay almacén** y lo
//! dicen por la salida. Un servidor de integración continua sin sesión de
//! escritorio no tiene Secret Service, y hacerlas fallar allí sería reportar
//! como defecto del proyecto una propiedad del entorno.

use nb_books_lib::error::NbError;
use nb_books_lib::secrets;

/// Nombre irrepetible por ejecución: dos pruebas en paralelo no pueden pisarse,
/// y una ejecución interrumpida no deja basura que confunda a la siguiente.
fn cuenta(sufijo: &str) -> String {
    format!(
        "prueba-{}-{}-{sufijo}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("el reloj del sistema")
            .as_nanos()
    )
}

/// Si no hay llavero, se dice y se sale. Ver la nota de cabecera.
fn hay_llavero() -> bool {
    if secrets::disponible() {
        return true;
    }
    eprintln!("se salta: este sistema no tiene almacén de credenciales");
    false
}

#[test]
fn lo_que_se_guarda_es_lo_que_se_lee() {
    if !hay_llavero() {
        return;
    }
    let cuenta = cuenta("ida-y-vuelta");
    // Con acentos y símbolos: una clave de API no los lleva, pero el que se
    // rompa con ellos se rompe con cualquier codificación que no sea la nuestra.
    let secreto = "clave-ñ-€-\"comillas\"-123";

    secrets::guardar(&cuenta, secreto).expect("guardar");
    assert_eq!(
        secrets::leer(&cuenta).expect("leer"),
        Some(secreto.to_owned())
    );

    secrets::borrar(&cuenta).expect("borrar");
}

#[test]
fn una_cuenta_sin_credencial_no_es_un_error() {
    if !hay_llavero() {
        return;
    }
    // Es el estado de fábrica de todo servicio. Quien llama tiene que poder
    // distinguirlo de un llavero roto, y por eso es `Ok(None)` y no `Err`.
    let cuenta = cuenta("nunca-escrita");
    assert_eq!(secrets::leer(&cuenta).expect("leer"), None);
}

#[test]
fn borrar_deja_la_cuenta_como_si_nunca_hubiera_tenido_nada() {
    if !hay_llavero() {
        return;
    }
    let cuenta = cuenta("borrado");
    secrets::guardar(&cuenta, "algo").expect("guardar");
    secrets::borrar(&cuenta).expect("borrar");

    assert_eq!(secrets::leer(&cuenta).expect("leer"), None);
    // Y borrar dos veces no falla: quien llama no tiene por qué saber si había.
    secrets::borrar(&cuenta).expect("borrar de nuevo");
}

/// La propiedad que impide mandarle a un servicio la credencial de otro.
///
/// Es la razón por la que la cuenta lleva el anfitrión dentro. Si esta prueba
/// fallara, cambiar el servidor de voz en Ajustes haría que la clave del
/// anterior viajase al nuevo.
#[test]
fn la_credencial_de_un_servicio_no_aparece_bajo_otro() {
    if !hay_llavero() {
        return;
    }
    let uno = secrets::cuenta_de_voz(&cuenta("api.uno.example"));
    let otro = secrets::cuenta_de_voz(&cuenta("api.otro.example"));

    secrets::guardar(&uno, "la-clave-de-uno").expect("guardar");

    assert_eq!(secrets::leer(&otro).expect("leer"), None);
    assert_eq!(
        secrets::leer(&uno).expect("leer"),
        Some("la-clave-de-uno".to_owned())
    );

    secrets::borrar(&uno).expect("limpiar");
}

#[test]
fn sustituir_una_credencial_no_deja_rastro_de_la_anterior() {
    if !hay_llavero() {
        return;
    }
    let cuenta = cuenta("sustitucion");
    secrets::guardar(&cuenta, "la-vieja").expect("guardar la vieja");
    secrets::guardar(&cuenta, "la-nueva").expect("guardar la nueva");

    assert_eq!(
        secrets::leer(&cuenta).expect("leer"),
        Some("la-nueva".to_owned())
    );
    secrets::borrar(&cuenta).expect("limpiar");
}

/// Lo que se rechaza antes de tocar el llavero, haya o no haya.
#[test]
fn lo_que_no_tiene_sentido_se_rechaza_sin_preguntarle_al_sistema() {
    assert_eq!(secrets::guardar("", "x"), Err(NbError::PathNotAllowed));
    assert_eq!(secrets::guardar("cuenta", ""), Err(NbError::PathNotAllowed));
    assert_eq!(
        secrets::guardar("cuenta", &"a".repeat(9 * 1024)),
        Err(NbError::PathNotAllowed)
    );
}
