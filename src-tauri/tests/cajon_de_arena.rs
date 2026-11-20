//! El cajón de arena, comprobado con el banco adverso.
//!
//! Aquí es donde el criterio de aceptación se cumple o no: un
//! módulo que intenta leer un archivo o abrir una conexión tiene que ser
//! detenido **por el anfitrión y no por sí mismo**, uno que no devuelve el
//! control tiene que interrumpirse, y uno que pide memoria sin freno tiene que
//! cortarse en su límite.
//!
//! La diferencia entre "detenido por el anfitrión" y "no lo intentó" no se
//! puede ver desde fuera si el módulo se limita a terminar sin ruido. Por eso
//! el anfitrión se queda con la salida estándar del módulo y estas pruebas la
//! leen: lo que se comprueba no es que la lectura fallara, sino que **el
//! módulo dice que le fallo a él**, habiéndolo intentado. Que lo intenta de
//! verdad lo comprueba `banco_de_extensiones.rs` abriendo el binario.
//!
//! Se omiten solas si el objetivo `wasm32-wasip1` no está instalado.

mod comun;

use std::time::{Duration, Instant};

use comun::{modulo, OBJETIVO};
use nb_books_lib::extensions::{Desenlace, Limites, Motor, SALIDA_MAXIMA};

/// Motor y módulo, o nada que comprobar.
fn preparar(nombre: &str) -> Option<(Motor, Vec<u8>)> {
    let bytes = modulo(nombre)?;
    Some((Motor::nuevo().expect("el motor arranca"), bytes))
}

#[test]
fn un_modulo_que_intenta_leer_un_archivo_no_encuentra_ninguno() {
    let Some((motor, bytes)) = preparar("lee-un-archivo") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let ejecucion = motor
        .ejecutar(&bytes, Limites::default())
        .expect("el anfitrión no falla porque el módulo lo intente");

    assert_eq!(ejecucion.desenlace, Desenlace::Termino);
    assert!(
        ejecucion.salida.contains("DETENIDO"),
        "el módulo tenía que decir que no pudo abrir el archivo, y dijo: {}",
        ejecucion.salida
    );
    assert!(
        !ejecucion.salida.contains("CONSEGUIDO"),
        "el módulo leyó un archivo del sistema desde dentro del cajón de arena"
    );
}

#[test]
fn un_modulo_que_pide_un_socket_no_recibe_ninguno() {
    let Some((motor, bytes)) = preparar("abre-una-conexion") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let ejecucion = motor
        .ejecutar(&bytes, Limites::default())
        .expect("el anfitrión no falla porque el módulo lo intente");

    // Aquí la contención no es la falta de la función: el anfitrión implementa
    // `sock_accept`, porque forma parte de WASI. Es la falta del recurso: no
    // hay ningún descriptor heredado sobre el que aceptar nada. Un código de
    // retorno distinto de cero es exactamente eso.
    assert_eq!(ejecucion.desenlace, Desenlace::Termino);
    assert!(
        ejecucion.salida.contains("devolvió") && !ejecucion.salida.contains("devolvió 0"),
        "el módulo obtuvo un socket dentro del cajón de arena: {}",
        ejecucion.salida
    );
}

#[test]
fn un_modulo_que_pide_una_capacidad_que_no_se_ofrece_no_llega_a_instanciarse() {
    let Some((motor, bytes)) = preparar("pide-lo-que-no-existe") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let ejecucion = motor
        .ejecutar(&bytes, Limites::default())
        .expect("el anfitrión no falla porque el módulo lo intente");

    match ejecucion.desenlace {
        Desenlace::CapacidadDenegada { modulo, funcion } => {
            // El nombre viaja porque un autor tiene que poder saber cuál de
            // sus importaciones sobra. Es lo que promete el cajón de arena.
            assert_eq!(modulo, "nb:extension/host");
            assert_eq!(funcion, "leer_documento");
        }
        otro => panic!("tenía que denegarse la capacidad y pasó otra cosa: {otro:?}"),
    }
    assert!(
        ejecucion.salida.is_empty(),
        "no llegó a instanciarse, así que no pudo escribir nada"
    );
}

#[test]
fn un_modulo_de_una_api_que_no_existe_se_rechaza_sin_llegar_a_compilarse() {
    let Some((motor, bytes)) = preparar("version-futura") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let ejecucion = motor
        .ejecutar(&bytes, Limites::default())
        .expect("el anfitrión no falla porque el módulo declare otra versión");

    match ejecucion.desenlace {
        Desenlace::VersionDeApiNoSoportada {
            declarada,
            admitida_minima,
            admitida_maxima,
        } => {
            // Los tres números viajan porque con ellos, y solo con ellos, el
            // autor sabe qué hacer: contra qué compiló y qué se admite. Es lo
            // que promete la sección 2.7 de versioning.md.
            assert_eq!(declarada, 99);
            assert!(admitida_minima <= admitida_maxima);
            assert!(admitida_maxima < declarada);
        }
        otro => panic!("tenía que rechazarse por versión y pasó otra cosa: {otro:?}"),
    }
    assert!(
        ejecucion.salida.is_empty(),
        "se rechaza antes de compilar, así que no puede haber escrito nada"
    );
}

#[test]
fn un_modulo_que_no_declara_version_no_se_ejecuta() {
    let Some((motor, bytes)) = preparar("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    // Se le quita la declaración cambiando el nombre de la sección en los
    // bytes: "nb:api-version" pasa a "nb:api-versiox", que ya no es la que el
    // anfitrión busca. Es la forma más barata de tener un módulo sin declarar
    // sin añadir un décimo al banco por una conducta que no es suya.
    let mutado = sin_declaracion(&bytes);
    let ejecucion = motor
        .ejecutar(&mutado, Limites::default())
        .expect("el anfitrión no falla");
    assert_eq!(ejecucion.desenlace, Desenlace::SinVersionDeApi);
}

/// Renombra la sección de versión para que el anfitrión no la encuentre.
fn sin_declaracion(wasm: &[u8]) -> Vec<u8> {
    let aguja = b"nb:api-version";
    let posicion = wasm
        .windows(aguja.len())
        .position(|v| v == aguja)
        .expect("el módulo declara su versión");
    let mut copia = wasm.to_vec();
    copia[posicion + aguja.len() - 1] = b'x';
    copia
}

#[test]
fn un_modulo_que_no_devuelve_el_control_se_interrumpe_en_su_plazo() {
    let Some((motor, bytes)) = preparar("entra-en-bucle") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let limites = Limites {
        tiempo: Duration::from_millis(200),
        ..Limites::default()
    };

    let reloj = Instant::now();
    let ejecucion = motor
        .ejecutar(&bytes, limites)
        .expect("el anfitrión aguanta");
    let tardado = reloj.elapsed();

    assert_eq!(ejecucion.desenlace, Desenlace::TiempoAgotado);
    // El plazo se cuenta por época, con una resolución de diez milisegundos,
    // así que puede pasarse un poco. Lo que no puede es no terminar: sin la
    // interrupción, esta prueba no volvería nunca.
    assert!(
        tardado < Duration::from_secs(5),
        "tardó {tardado:?} en interrumpir un plazo de 200 ms"
    );
}

#[test]
fn un_modulo_que_pide_memoria_sin_freno_se_corta_en_su_limite() {
    let Some((motor, bytes)) = preparar("reserva-sin-freno") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let limites = Limites {
        memoria: 8 * 1024 * 1024,
        ..Limites::default()
    };
    let ejecucion = motor
        .ejecutar(&bytes, limites)
        .expect("el anfitrión aguanta");

    // Que se distinga de un fallo cualquiera del módulo es justo lo que aporta
    // el cupo propio: sin su marca, quedarse sin memoria llega aquí como una
    // trampa genérica y sería indistinguible de un error del autor.
    assert_eq!(ejecucion.desenlace, Desenlace::MemoriaAgotada);
}

#[test]
fn el_anfitrion_sigue_utilizable_despues_de_contener_a_los_cinco() {
    let Some(benigno) = modulo("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let motor = Motor::nuevo().expect("el motor arranca");
    let corto = Limites {
        memoria: 8 * 1024 * 1024,
        tiempo: Duration::from_millis(200),
    };

    for hostil in [
        "lee-un-archivo",
        "abre-una-conexion",
        "pide-lo-que-no-existe",
        "entra-en-bucle",
        "reserva-sin-freno",
    ] {
        let bytes = modulo(hostil).expect("el banco está construido");
        motor
            .ejecutar(&bytes, corto)
            .expect("contener a una extensión no es una avería");
    }

    // El innegociable: el fallo de un componente no puede tumbar a otro. Cinco
    // módulos hostiles seguidos, en el mismo motor, y el sexto funciona.
    let ejecucion = motor
        .ejecutar(&benigno, Limites::default())
        .expect("el anfitrión sigue vivo");
    assert_eq!(ejecucion.desenlace, Desenlace::Termino);
    assert!(
        ejecucion.salida.contains("suma = 55"),
        "el módulo benigno tenía que sumar y dijo: {}",
        ejecucion.salida
    );
}

#[test]
fn lo_que_el_modulo_escribe_se_queda_en_el_anfitrion_y_acotado() {
    let Some((motor, bytes)) = preparar("carga-remota-v1") else {
        eprintln!("omitida: falta el objetivo {OBJETIVO}");
        return;
    };
    let ejecucion = motor
        .ejecutar(&bytes, Limites::default())
        .expect("el anfitrión no falla");

    // Que la salida llegue aquí es lo que demuestra que **no se heredó**: si
    // se hubiera heredado, estaría en la consola de la aplicación y este búfer
    // estaría vacío. Y está acotada para que un módulo hablador no pueda hacer
    // crecer la memoria del anfitrión escribiendo.
    assert!(!ejecucion.salida.is_empty());
    assert!(ejecucion.salida.len() <= SALIDA_MAXIMA);
}
