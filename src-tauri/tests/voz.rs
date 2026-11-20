//! La voz del runtime: la que existe porque el webview de Linux no tiene la suya.
//!
//! Estas pruebas necesitan `speech-dispatcher` instalado. Donde no lo esté se
//! omiten diciéndolo, en lugar de fallar: el runtime **declara** que no puede
//! hablar y esa es la respuesta correcta, no un error de la suite.
//!
//! # Estas pruebas corren a la vez, y que puedan hacerlo es lo que demuestran
//!
//! `speech-dispatcher` es un servidor único de la sesión. Mientras la voz se
//! mandaba con `spd-say`, cada llamada abría una conexión nueva y las órdenes de
//! callar alcanzaban a **todos** sus clientes: dos pruebas en paralelo se
//! cortaban la voz la una a la otra, una mandaba callar, el enunciado de la otra
//! volvía con éxito, y esa otra anunciaba como terminado un tramo que se había
//! cortado. Fallaba una de cada tres veces, y hubo que serializarlas con un
//! candado.
//!
//! Con una conexión propia y sostenida, `STOP self` calla lo de esa conexión y
//! nada más. El candado sobra, y quitarlo es la comprobación: cinco pruebas
//! hablando y callando a la vez, cada una en su conexión, sin pisarse. Es la
//! misma garantía que hace que parar aquí no calle al lector de pantalla de
//! quien esté usando la aplicación.
//!
//! Si alguien vuelve a mandar la voz por un proceso suelto, estas pruebas se
//! volverán intermitentes otra vez, y eso es exactamente lo que tienen que
//! hacer.

use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::voice::{status, Narrator};

fn hay_sintetizador() -> bool {
    status().available
}

/// El estado se **declara**, y distingue las dos situaciones que hay.
#[test]
fn el_estado_de_la_voz_dice_lo_que_hay() {
    let estado = status();
    if estado.available {
        assert!(estado.reason.is_none(), "disponible y con motivo a la vez");
        assert!(!estado.voices.is_empty(), "disponible y sin voces");
    } else {
        // Sin sintetizador o sin voces: son cosas distintas y la interfaz dice
        // una u otra, porque la segunda se arregla instalando una voz.
        let motivo = estado.reason.as_deref();
        assert!(
            matches!(motivo, Some("sin-sintetizador" | "sin-voces")),
            "un motivo que la interfaz no sabe traducir: {motivo:?}"
        );
    }
}

/// Hablar avisa por el bus cuando termina.
///
/// El aviso llega por ahí y no como respuesta del comando porque hablar tarda lo
/// que dura el párrafo: esperar en el comando dejaría la llamada abierta minutos.
#[test]
fn al_terminar_un_tramo_se_avisa_por_el_bus() {
    if !hay_sintetizador() {
        eprintln!("sin sintetizador en esta máquina; se omite");
        return;
    }

    let bus = EventBus::new();
    let avisos = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
    let destino = std::sync::Arc::clone(&avisos);
    bus.subscribe("voz", move |evento| {
        if let DomainEvent::VoiceFinished { block } = evento {
            destino
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(*block);
        }
    });

    let narrador = Narrator::start(bus);
    narrador.speak(7, "hola".to_owned(), 2.0, None);

    for _ in 0..200 {
        if !avisos.lock().unwrap_or_else(|p| p.into_inner()).is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    assert_eq!(
        avisos.lock().unwrap_or_else(|p| p.into_inner()).as_slice(),
        &[7],
        "tiene que avisar del bloque que se le pidió"
    );
}

/// **Volver a hablar después de parar arranca enseguida.**
///
/// Lo encontró la suite y en la aplicación se notaba así: pulsar parar y volver
/// a dar a leer tardaba **hasta un párrafo entero**. El hilo de la voz es uno
/// solo, y mientras esperaba el fin del tramo que ya nadie quería no atendía la
/// orden de parar ni la de volver a hablar, que estaban detrás en la cola.
///
/// El umbral es generoso a propósito: el párrafo que se corta dura unos nueve
/// segundos, así que dos son de sobra para distinguir "arranca" de "espera a que
/// termine el anterior", y no falla por el ruido de una máquina cargada.
///
/// **Dos mecanismos sostienen esto, y romper uno solo no la tumba.** Uno es que
/// `stop` calla en el servidor desde el hilo de quien pulsa, y esa cancelación
/// llega como un aviso que saca al hilo de la espera. El otro es que el propio
/// bucle mira si alguien paró, y hace falta cuando el primero no puede: cuando
/// aún no hay conexión abierta, o cuando el servidor tarda en avisar. Que sean
/// redundantes es deliberado; comprobado rompiendo los dos a la vez, que sí la
/// hace fallar.
#[test]
fn volver_a_hablar_tras_parar_no_espera_al_parrafo_que_se_corto() {
    if !hay_sintetizador() {
        eprintln!("sin sintetizador en esta máquina; se omite");
        return;
    }

    let bus = EventBus::new();
    let avisos = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
    let destino = std::sync::Arc::clone(&avisos);
    bus.subscribe("voz", move |evento| {
        if let DomainEvent::VoiceFinished { block } = evento {
            destino
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(*block);
        }
    });

    let narrador = Narrator::start(bus);
    // Un párrafo largo, y se le deja empezar: así el hilo está de verdad
    // esperando su fin cuando llega la orden de parar.
    narrador.speak(
        1,
        "Muchos años después, frente al pelotón de fusilamiento, el coronel \
         Aureliano Buendía había de recordar aquella tarde remota en que su \
         padre lo llevó a conocer el hielo."
            .to_owned(),
        1.0,
        None,
    );
    std::thread::sleep(std::time::Duration::from_millis(600));

    let inicio = std::time::Instant::now();
    narrador.stop();
    narrador.speak(2, "Ya.".to_owned(), 1.0, None);

    for _ in 0..60 {
        if avisos
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&2)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let tardanza = inicio.elapsed();

    let recibidos = avisos.lock().unwrap_or_else(|p| p.into_inner());

    /*
     * **La aserción que hace el trabajo es esta, y no la del reloj.**
     *
     * El bucle de arriba espera como mucho tres segundos —sesenta vueltas de
     * cincuenta milisegundos— y el párrafo cortado dura muchos más. Así que si
     * `stop` no hubiera cancelado nada, el segundo tramo no habría llegado dentro
     * del bucle y esto falla. Es una comprobación estructural: llegó o no llegó.
     */
    assert!(
        recibidos.contains(&2),
        "el segundo tramo no llegó a decirse: parar no canceló el párrafo anterior"
    );

    /*
     * Y el reloj, con un umbral **por encima del presupuesto del bucle**.
     *
     * Antes exigía menos de dos segundos, que es *menos* de lo que el propio
     * bucle puede tardar en dar por buena una espera correcta: la prueba podía
     * fallar teniendo éxito, y lo hizo el 2026-09-06 con la máquina cargada,
     * midiendo 2,76 s. Sola pasa en 1,56 tres veces de tres.
     *
     * Un umbral que cae dentro del ruido del instrumento no separa nada. Este
     * tiene que quedar entre las dos poblaciones: una espera correcta acaba
     * dentro del bucle, y esperar al párrafo entero tarda muchísimo más.
     */
    assert!(
        tardanza < std::time::Duration::from_secs(5),
        "volver a hablar esperó al párrafo cortado: {tardanza:?}"
    );
}

/// **Parar para de verdad, y para enseguida.**
///
/// Es la prueba que encontró un defecto propio: la primera versión sacaba el
/// proceso del cerrojo para esperarlo, así que parar no encontraba a quién matar
/// y la voz seguía hablando hasta el final del párrafo.
#[test]
fn parar_corta_la_voz_en_lugar_de_esperar_al_final_del_parrafo() {
    if !hay_sintetizador() {
        eprintln!("sin sintetizador en esta máquina; se omite");
        return;
    }

    let bus = EventBus::new();
    let terminados = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
    let destino = std::sync::Arc::clone(&terminados);
    bus.subscribe("voz", move |evento| {
        if matches!(evento, DomainEvent::VoiceFinished { .. }) {
            *destino.lock().unwrap_or_else(|p| p.into_inner()) += 1;
        }
    });

    let narrador = Narrator::start(bus);
    // Un párrafo largo y lento: sin cortar, tardaría bastante más que la espera.
    narrador.speak(
        1,
        "Muchos años después frente al pelotón de fusilamiento el coronel \
         Aureliano Buendía había de recordar aquella tarde remota."
            .to_owned(),
        0.75,
        None,
    );

    std::thread::sleep(std::time::Duration::from_millis(400));
    let inicio = std::time::Instant::now();
    narrador.stop();
    let tardanza = inicio.elapsed();

    assert!(
        tardanza < std::time::Duration::from_millis(500),
        "parar tiene que volver enseguida, no esperar al párrafo: {tardanza:?}"
    );

    /*
     * Y **la voz se calla de verdad**, que es lo que se estaba comprobando.
     *
     * La espera es más larga que la frase entera a propósito: con menos, la
     * prueba pasaba también con el defecto que venía a cazar, porque el proceso
     * seguía hablando y su aviso llegaba después de que la prueba hubiera
     * terminado. Una prueba que no espera lo que tarda lo que mide no está
     * midiendo nada, y esta se comprobó rompiéndola.
     */
    std::thread::sleep(std::time::Duration::from_secs(9));
    assert_eq!(
        *terminados.lock().unwrap_or_else(|p| p.into_inner()),
        0,
        "el tramo se cortó, así que no puede anunciarse como terminado"
    );
}

/// **Parar antes de que empiece también para.**
///
/// Entre mandar hablar y que el hilo llegue a lanzar el sintetizador hay un
/// hueco. Dentro de él no hay proceso que matar, y la orden de parar se queda
/// detrás de la de hablar en la misma cola: el hilo lanzaba el sintetizador
/// después de haber recibido la orden de parar, leía el párrafo entero y lo
/// anunciaba como terminado.
///
/// Se veía con la máquina cargada —la suite entera en paralelo lo destapaba una
/// vez de cada tantas— y en la aplicación es el caso de pulsar escuchar y
/// cambiar de idea enseguida, que no es raro en absoluto.
///
/// Aquí no se espera nada entre las dos llamadas a propósito: es la condición
/// que la prueba viene a reproducir.
#[test]
fn parar_justo_despues_de_empezar_no_deja_que_empiece() {
    if !hay_sintetizador() {
        eprintln!("sin sintetizador en esta máquina; se omite");
        return;
    }

    let bus = EventBus::new();
    let terminados = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
    let destino = std::sync::Arc::clone(&terminados);
    bus.subscribe("voz", move |evento| {
        if matches!(evento, DomainEvent::VoiceFinished { .. }) {
            *destino.lock().unwrap_or_else(|p| p.into_inner()) += 1;
        }
    });

    let narrador = Narrator::start(bus);
    narrador.speak(1, "Una frase corta.".to_owned(), 1.0, None);
    narrador.stop();

    // Más de lo que tarda la frase: si se hubiera dicho, el aviso habría llegado.
    std::thread::sleep(std::time::Duration::from_secs(4));
    assert_eq!(
        *terminados.lock().unwrap_or_else(|p| p.into_inner()),
        0,
        "se paró antes de empezar, así que no hay tramo que terminar"
    );
}

/// Y después de parar se puede volver a hablar.
///
/// La generación que invalida lo pedido no puede dejar la voz muda para
/// siempre: quien para y vuelve a darle a escuchar tiene que oír algo.
#[test]
fn despues_de_parar_se_puede_volver_a_hablar() {
    if !hay_sintetizador() {
        eprintln!("sin sintetizador en esta máquina; se omite");
        return;
    }

    let bus = EventBus::new();
    let avisos = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u32>::new()));
    let destino = std::sync::Arc::clone(&avisos);
    bus.subscribe("voz", move |evento| {
        if let DomainEvent::VoiceFinished { block } = evento {
            destino
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(*block);
        }
    });

    let narrador = Narrator::start(bus);
    narrador.speak(1, "Primera.".to_owned(), 1.0, None);
    narrador.stop();
    narrador.speak(2, "Segunda.".to_owned(), 1.0, None);

    for _ in 0..80 {
        if avisos
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&2)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    /*
     * Lo que se comprueba es que el **segundo** tramo se dijo y se anunció.
     *
     * No que el primero no lo hiciera, y esa distinción costó entender un fallo
     * intermitente. Con la voz por un proceso, arrancarlo tardaba lo bastante
     * como para que `stop` siempre llegara antes; hablando el protocolo, el
     * encolado es inmediato y el servidor puede dar por terminado un enunciado
     * de una palabra antes de que la orden de parar salga del hilo de al lado.
     *
     * Y si terminó, terminó: anunciarlo es correcto. Exigir que un `stop`
     * posterior lo borre sería pedir que la aplicación reescriba el pasado, y
     * quien lo pedía era esta prueba, no el producto. Que un tramo **que aún
     * suena** se calle y no se anuncie lo comprueba
     * `parar_corta_la_voz_en_lugar_de_esperar_al_final_del_parrafo`, con un
     * párrafo lo bastante largo como para que no haya carrera que valga.
     */
    let recibidos = avisos.lock().unwrap_or_else(|p| p.into_inner());
    assert!(
        recibidos.contains(&2),
        "después de parar, hablar tiene que volver a funcionar: {recibidos:?}"
    );
}
