//! Criterio de aceptación de la voz neuronal: el motor vendorizado
//! sintetiza **nuestros** fonemas y se mide lo que tarda.
//!
//! # Por qué se salta sola
//!
//! Los pesos no están en el repositorio —la regla lo prohíbe— y pesan 327 MB, así
//! que esta prueba **no puede ser parte obligatoria de `pnpm verify`**. Se ejecuta
//! apuntando `NB_VOZ_KOKORO` a una carpeta con `model.pth`, `config.json` y
//! `voices/`, y sin esa variable se salta diciendo por qué.
//!
//! # Qué se comprueba, y qué se mide
//!
//! Se comprueba lo que sí es una propiedad del código: que una frase en español
//! pasada por `voice::fonemas` produce sonido con la duración que corresponde a sus
//! fonemas, sin silencio, sin recorte y sin valores no finitos.
//!
//! Y se **mide** el factor de tiempo real: cuánto tarda en sintetizar dividido por
//! lo que dura el audio. Es el número del que depende que la voz neuronal se pueda
//! usar leyendo, y por eso se imprime aunque no haga fallar la prueba: el listón de
//! aquí lo pone la máquina que la ejecute, no el código, y una prueba que falle por
//! la máquina no dice nada de lo que se cambió.

use std::path::PathBuf;
use std::time::Instant;

use candle_core::Device;
use nb_books_lib::voice::fonemas::{self, Variante};
use nb_books_lib::voice::kokoro::{Kokoro, MUESTREO};

/// La carpeta del modelo. Sin ella no hay nada que medir.
const CARPETA: &str = "NB_VOZ_KOKORO";

/// Qué voz usar. La del repositorio del autor que habla español es `ef_dora`.
const VOZ: &str = "NB_VOZ_KOKORO_VOZ";

/// Enciende el registro para ver el desglose por etapas, que es lo que dice dónde
/// se va el tiempo cuando el factor no cuadra.
fn registrar() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("nb_books_lib=debug")),
        )
        .with_writer(std::io::stderr)
        .try_init();
}

fn carpeta() -> Option<PathBuf> {
    let Some(ruta) = std::env::var_os(CARPETA).map(PathBuf::from) else {
        eprintln!("{CARPETA} no está definida: la prueba de la voz neuronal se salta");
        return None;
    };
    if !ruta.join("model.pth").exists() {
        eprintln!("{CARPETA} apunta a una carpeta sin model.pth: se salta");
        return None;
    }
    Some(ruta)
}

/// **El criterio.** Una frase en español, dicha con nuestros fonemas, suena; y lo
/// que tarda en decirse queda medido.
#[test]
fn una_frase_en_espanol_se_sintetiza_con_nuestros_fonemas_y_se_mide_lo_que_tarda() {
    let Some(carpeta) = carpeta() else { return };
    registrar();
    let voz = std::env::var(VOZ).unwrap_or_else(|_| "ef_dora".to_owned());

    let antes = memoria_maxima();
    let modelo = Kokoro::cargar(&carpeta, &Device::Cpu).expect("el modelo se carga");
    let estilo = modelo
        .voz(&carpeta.join("voices"), &voz)
        .expect("la voz se carga");
    eprintln!(
        "voz neuronal: pico del proceso {} MiB con el modelo cargado ({} MiB antes de cargarlo)",
        memoria_maxima() / (1024 * 1024),
        antes / (1024 * 1024)
    );

    let frase = "El veinticinco de abril de mil cuatrocientos noventa y dos, \
                 la imprenta llegó a Sevilla y nadie lo anotó.";
    let fonemas = fonemas::de_texto(frase, Variante::EspanolDeAmerica);
    assert!(
        fonemas::todos_admitidos(&fonemas),
        "los fonemas se salen del vocabulario del modelo: {fonemas}"
    );

    let reloj = Instant::now();
    let muestras = modelo
        .sintetizar(&fonemas, &estilo, 1.0)
        .expect("la síntesis produce sonido");
    let tardanza = reloj.elapsed().as_secs_f64();

    assert!(!muestras.is_empty(), "no salió ni una muestra");
    assert!(
        muestras.iter().all(|m| m.is_finite()),
        "hay muestras que no son números"
    );
    let pico = muestras.iter().fold(0.0f32, |a, m| a.max(m.abs()));
    assert!(pico > 0.05, "el audio salió en silencio: pico {pico}");
    assert!(pico <= 1.01, "el audio sale recortado: pico {pico}");

    #[allow(clippy::cast_precision_loss)]
    let duracion = muestras.len() as f64 / f64::from(MUESTREO);
    // Con unos 100 fonemas, una lectura normal no baja de tres segundos ni pasa de
    // veinte: fuera de ahí la alineación está mal, no la voz.
    assert!(
        (3.0..20.0).contains(&duracion),
        "la frase dura {duracion:.2} s, que no es una lectura de {} fonemas",
        fonemas.chars().count()
    );

    let factor = tardanza / duracion;
    eprintln!(
        "voz neuronal: {duracion:.2} s de audio en {tardanza:.2} s — \
         factor de tiempo real {factor:.2} (voz {voz}, {} hilos)",
        std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get)
    );

    if let Some(destino) = std::env::var_os("NB_VOZ_WAV") {
        escribir_wav(&PathBuf::from(destino), &muestras);
    }
}

/// La velocidad hace lo que dice: más deprisa dura menos.
#[test]
fn la_velocidad_acorta_el_audio_en_proporcion() {
    let Some(carpeta) = carpeta() else { return };
    let voz = std::env::var(VOZ).unwrap_or_else(|_| "ef_dora".to_owned());
    let modelo = Kokoro::cargar(&carpeta, &Device::Cpu).expect("el modelo se carga");
    let estilo = modelo
        .voz(&carpeta.join("voices"), &voz)
        .expect("la voz se carga");

    let fonemas = fonemas::de_texto(
        "La imprenta llegó a Sevilla y nadie lo anotó.",
        Variante::EspanolDeAmerica,
    );
    let normal = modelo.sintetizar(&fonemas, &estilo, 1.0).expect("normal");
    let deprisa = modelo.sintetizar(&fonemas, &estilo, 1.5).expect("deprisa");

    #[allow(clippy::cast_precision_loss)]
    let proporcion = deprisa.len() as f64 / normal.len() as f64;
    eprintln!(
        "velocidad 1,5: dura el {:.0} % de lo normal",
        proporcion * 100.0
    );
    assert!(
        (0.6..0.75).contains(&proporcion),
        "a 1,5 debería durar dos tercios y dura el {:.0} %",
        proporcion * 100.0
    );
}

/// **El criterio, por la parte que se puede comprobar aquí**: el mismo
/// tramo no se sintetiza dos veces, y el archivo que queda suena.
///
/// La caché es lo que hace posible adelantar el tramo siguiente: quien adelanta
/// pide y tira el resultado, y cuando llega su turno el archivo ya está. Sin
/// caché, adelantar sería sintetizar dos veces y tardaría el doble.
#[test]
fn el_mismo_tramo_no_se_sintetiza_dos_veces() {
    let Some(carpeta) = carpeta() else { return };
    let voz = std::env::var(VOZ).unwrap_or_else(|_| "ef_dora".to_owned());
    let modelos = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-voz-cache-{}", std::process::id()));
    let destino = modelos.join("audio");

    let proveedor = nb_books_lib::voice::neuronal::VozNeuronal::nueva(
        &modelos,
        nb_books_lib::ai::presupuesto::Presupuesto {
            hilos: 4,
            memoria: 2 * 1024 * 1024 * 1024,
        },
    );
    // Sin el modelo instalado en esa carpeta, el proveedor dice que no está en vez
    // de fallar por dentro: es la mitad del criterio que sí depende del código.
    assert!(
        proveedor
            .sintetizar_a_archivo(&destino, "Hola.", Some(&voz), 1.0)
            .is_err(),
        "sin modelo instalado no puede sintetizar"
    );

    // Y con el modelo de verdad, por la carpeta que la variable de entorno señala.
    let real = nb_books_lib::voice::kokoro::Kokoro::cargar(&carpeta, &Device::Cpu)
        .expect("el modelo se carga");
    let estilo = real
        .voz(&carpeta.join("voices"), &voz)
        .expect("la voz se carga");
    let fonemas = fonemas::de_texto("La imprenta llegó a Sevilla.", Variante::EspanolDeAmerica);
    let muestras = real.sintetizar(&fonemas, &estilo, 1.0).expect("suena");

    let archivo = destino.join("prueba.wav");
    nb_books_lib::voice::wav::escribir(&archivo, &muestras, MUESTREO).expect("se escribe");
    let escrito = std::fs::metadata(&archivo).expect("existe").len();
    assert_eq!(
        escrito,
        44 + muestras.len() as u64 * 2,
        "el wav tiene cabecera y dos bytes por muestra"
    );
    let _ = std::fs::remove_dir_all(&modelos);
}

/// El pico de memoria del proceso, en bytes. Es lo que declara el catálogo, y se
/// mide en vez de calcularse con el tamaño del archivo.
fn memoria_maxima() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|texto| {
            texto
                .lines()
                .find_map(|l| l.strip_prefix("VmHWM:"))
                .and_then(|l| l.split_whitespace().next()?.parse::<u64>().ok())
        })
        .map_or(0, |kib| kib * 1024)
}

/// **La excepción del `.pth`, probada en vez de afirmada.** Un *pickle* con una
/// llamada dentro no ejecuta nada al leerlo con el lector restringido de Candle.
///
/// Esta es la prueba que sostiene lo que dice `voice::catalogo`: el catálogo solo
/// admite formatos que no ejecutan código, y los pesos de la voz son un `.pth`
/// porque es lo único que publica su autor. Si mañana Candle cambiara y ejecutara
/// lo que lee, esto falla aquí y no en la máquina de quien instale la voz.
#[test]
fn un_pickle_con_una_llamada_dentro_no_ejecuta_nada() {
    let temporal = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-pickle-{}", std::process::id()));
    std::fs::create_dir_all(&temporal).expect("carpeta temporal");
    let senal = temporal.join("se-ejecuto");
    let archivo = temporal.join("malicioso.pth");

    // Proto 2, `__builtin__.eval("...")` con REDUCE: exactamente lo que Python
    // ejecutaría al abrirlo.
    let orden = format!("open({:?}, 'w').write('x')", senal.display().to_string());
    let mut pickle = Vec::new();
    pickle.extend_from_slice(b"\x80\x02c__builtin__\neval\nq\x00");
    pickle.push(b'X');
    #[allow(clippy::cast_possible_truncation)]
    let largo = orden.len() as u32;
    pickle.extend_from_slice(&largo.to_le_bytes());
    pickle.extend_from_slice(orden.as_bytes());
    pickle.extend_from_slice(b"q\x01\x85q\x02Rq\x03.");

    std::fs::write(&archivo, envolver_en_zip("malicioso/data.pkl", &pickle)).expect("se escribe");
    // Lo que devuelva da igual: lo que no puede pasar es que la orden ocurra.
    let _ = candle_core::pickle::read_all(&archivo);
    assert!(
        !senal.exists(),
        "el lector de pickle ejecutó lo que había dentro"
    );
    let _ = std::fs::remove_dir_all(&temporal);
}

/// Un ZIP de una sola entrada sin comprimir, que es lo que envuelve a un `.pth`.
fn envolver_en_zip(nombre: &str, datos: &[u8]) -> Vec<u8> {
    fn crc32(datos: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for byte in datos {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    #[allow(clippy::cast_possible_truncation)]
    let largo = datos.len() as u32;
    #[allow(clippy::cast_possible_truncation)]
    let largo_nombre = nombre.len() as u16;
    let crc = crc32(datos);
    let mut zip = Vec::new();
    zip.extend_from_slice(b"PK\x03\x04\x14\x00\x00\x00\x00\x00\x00\x00\x00\x00");
    zip.extend_from_slice(&crc.to_le_bytes());
    zip.extend_from_slice(&largo.to_le_bytes());
    zip.extend_from_slice(&largo.to_le_bytes());
    zip.extend_from_slice(&largo_nombre.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip.extend_from_slice(nombre.as_bytes());
    zip.extend_from_slice(datos);
    #[allow(clippy::cast_possible_truncation)]
    let inicio_central = zip.len() as u32;
    zip.extend_from_slice(b"PK\x01\x02\x14\x00\x14\x00\x00\x00\x00\x00\x00\x00\x00\x00");
    zip.extend_from_slice(&crc.to_le_bytes());
    zip.extend_from_slice(&largo.to_le_bytes());
    zip.extend_from_slice(&largo.to_le_bytes());
    zip.extend_from_slice(&largo_nombre.to_le_bytes());
    zip.extend_from_slice(&[0u8; 8]);
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(&0u32.to_le_bytes());
    zip.extend_from_slice(nombre.as_bytes());
    #[allow(clippy::cast_possible_truncation)]
    let tamano_central = zip.len() as u32 - inicio_central;
    zip.extend_from_slice(b"PK\x05\x06\x00\x00\x00\x00\x01\x00\x01\x00");
    zip.extend_from_slice(&tamano_central.to_le_bytes());
    zip.extend_from_slice(&inicio_central.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes());
    zip
}

/// Escribe un WAV de 16 bits para poder escucharlo. **Nunca en el repositorio.**
fn escribir_wav(destino: &PathBuf, muestras: &[f32]) {
    use std::io::Write;
    let datos: Vec<u8> = muestras
        .iter()
        .flat_map(|m| {
            #[allow(clippy::cast_possible_truncation)]
            let v = (m.clamp(-1.0, 1.0) * 32767.0) as i16;
            v.to_le_bytes()
        })
        .collect();
    #[allow(clippy::cast_possible_truncation)]
    let n = datos.len() as u32;
    let mut archivo = std::fs::File::create(destino).expect("el wav se crea");
    let mut cabecera = Vec::new();
    cabecera.extend_from_slice(b"RIFF");
    cabecera.extend_from_slice(&(36 + n).to_le_bytes());
    cabecera.extend_from_slice(b"WAVEfmt ");
    cabecera.extend_from_slice(&16u32.to_le_bytes());
    cabecera.extend_from_slice(&1u16.to_le_bytes());
    cabecera.extend_from_slice(&1u16.to_le_bytes());
    cabecera.extend_from_slice(&MUESTREO.to_le_bytes());
    cabecera.extend_from_slice(&(MUESTREO * 2).to_le_bytes());
    cabecera.extend_from_slice(&2u16.to_le_bytes());
    cabecera.extend_from_slice(&16u16.to_le_bytes());
    cabecera.extend_from_slice(b"data");
    cabecera.extend_from_slice(&n.to_le_bytes());
    archivo.write_all(&cabecera).expect("cabecera");
    archivo.write_all(&datos).expect("datos");
}

/// **Un párrafo del largo de un fragmento se dice entero**, por el mismo camino que
/// la aplicación: `VozNeuronal`, con nuestros fonemas.
///
/// El modelo admite como mucho 512 fichas por llamada (`max_position_embeddings`),
/// y los fragmentos que se preparan miden hasta 1.920 caracteres
/// (`search::FRAGMENTO`). Antes nada troceaba, así que el primer
/// párrafo largo de un capítulo real hacía fallar la síntesis y con ella la
/// preparación entera. Ninguna prueba lo veía: la de preparar usa una voz simulada,
/// y la frase de calibración es corta.
///
/// Pide `NB_VOZ_MODELOS` apuntando a la carpeta de modelos, la que contiene
/// `kokoro-82m/`.
#[test]
fn un_parrafo_del_largo_de_un_fragmento_se_dice_entero() {
    let Some(modelos) = std::env::var_os("NB_VOZ_MODELOS").map(PathBuf::from) else {
        eprintln!("NB_VOZ_MODELOS no está definida: se salta");
        return;
    };
    let voz = nb_books_lib::voice::neuronal::VozNeuronal::nueva(
        &modelos,
        nb_books_lib::ai::presupuesto::Presupuesto::de_fabrica(
            nb_books_lib::ai::hardware::nucleos(),
        ),
    );
    let parrafo = "La biblioteca del puerto guardaba los libros de cuentas de tres siglos, \
        y quien los leía con cuidado encontraba en ellos algo más que números: los barcos que \
        llegaron tarde, los inviernos en que el río se heló, las familias que pagaron la sal \
        con trigo porque no tenían moneda. El archivero, que había llegado joven y se quedó \
        cuarenta años, decía que un libro de cuentas era la única crónica que nadie escribe \
        para ser leída, y que por eso no miente. Anotaba al margen, con lápiz y letra pequeña, \
        lo que iba descubriendo: que en mil setecientos ochenta y ocho el precio del aceite se \
        triplicó en una semana, que un mismo apellido aparece en los registros durante ciento \
        doce años y desaparece de golpe, que las multas por pesar mal la harina se cobraban \
        siempre en marzo. Nadie le pidió nunca aquellas notas, y cuando se jubiló las dejó en \
        una caja con una etiqueta que decía, sin más, lo que no está en las cuentas. Su \
        sucesora abrió la caja veinte años después, buscando otra cosa, y pasó el invierno \
        entero leyéndola. De ahí salió el primer libro sobre la historia del puerto que no \
        empezaba por los reyes ni por las guerras, sino por el precio de la sal, y que se \
        sigue leyendo en las escuelas de la comarca.";
    // Más de mil caracteres son más de mil fonemas: el doble de lo que el modelo
    // admite de una vez, que es lo que esta prueba necesita que ocurra.
    assert!(
        parrafo.chars().count() > 1_000,
        "el párrafo de prueba tiene que pasar de largo lo que el modelo admite"
    );

    let sonido = voz
        .sintetizar(parrafo, Some("ef_dora"), 1.0)
        .expect("un párrafo largo tiene que sintetizarse entero");
    #[allow(clippy::cast_precision_loss)]
    let duracion = sonido.muestras.len() as f64 / f64::from(sonido.muestreo);
    // Unos diecisiete caracteres por segundo, medido: un párrafo de más de mil
    // caracteres dura casi un minuto. Si dura mucho menos, se ha dicho a medias.
    #[allow(clippy::cast_precision_loss)]
    let esperada = parrafo.chars().count() as f64 / 17.0;
    eprintln!("párrafo largo: {duracion:.1} s de audio, se esperaban unos {esperada:.0}");
    // Con `NB_VOZ_OPUS`, lo deja escrito con **nuestro** codificador, el mismo que el
    // de lo preparado: sirve para escucharlo y para comprobarlo en el WebKitGTK real
    // con `scripts/comprobar-el-motor.py`.
    if let Some(destino) = std::env::var_os("NB_VOZ_OPUS") {
        nb_books_lib::voice::audio::escribir_opus(
            &PathBuf::from(destino),
            &sonido.muestras,
            sonido.muestreo,
        )
        .expect("escribir el Opus");
    }
    assert!(
        duracion > esperada * 0.6,
        "se dijo a medias: {duracion:.1} s para un texto de unos {esperada:.0}"
    );
}

/// **Todas las voces del catálogo suenan, no solo la primera**, y una que no es de
/// este modelo cae en la de fábrica en lugar de fallar.
///
/// Antes solo se cargaba la primera voz, y elegir Alex o Santa hacía
/// fallar cada síntesis con "no disponible". Y el ajuste `voice_name` es el mismo
/// para la voz del sistema, así que puede traer el nombre de una de speech-dispatcher.
#[test]
fn cualquier_voz_del_catalogo_suena_y_una_ajena_cae_en_la_de_fabrica() {
    let Some(modelos) = std::env::var_os("NB_VOZ_MODELOS").map(PathBuf::from) else {
        eprintln!("NB_VOZ_MODELOS no está definida: se salta");
        return;
    };
    let voz = nb_books_lib::voice::neuronal::VozNeuronal::nueva(
        &modelos,
        nb_books_lib::ai::presupuesto::Presupuesto::de_fabrica(
            nb_books_lib::ai::hardware::nucleos(),
        ),
    );
    for id in ["ef_dora", "em_alex", "em_santa", "Spanish (Latin America)"] {
        let sonido = voz
            .sintetizar("La imprenta llegó a Sevilla.", Some(id), 1.0)
            .unwrap_or_else(|e| panic!("la voz {id} no suena: {e:?}"));
        assert!(!sonido.muestras.is_empty(), "la voz {id} no dijo nada");
    }
}
