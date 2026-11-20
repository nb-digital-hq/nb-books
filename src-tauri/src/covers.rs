//! Miniaturas de portada: lo que la rejilla pinta de verdad.
//!
//! # Por qué existe, con la medida delante
//!
//! Porque la rejilla servía **la portada original**, y las portadas reales de la
//! biblioteca de prueba son de 600x900 y 800x1200 píxeles. La cuadrícula las
//! pinta a 188x282. La diferencia no está en el archivo —un JPEG de 130 KB no es
//! nada— sino en lo que el motor tiene que descodificar para poder pintarlas.
//!
//! Medido el 2026-09-06 en WebKitGTK 2.52.6, ventana de 1600x900, cincuenta
//! portadas **distintas** por página, desde que se insertan hasta que todas han
//! resuelto `decode()`:
//!
//! | 50 por página | Pasar de página | Volver a una ya vista |
//! |---|---|---|
//! | 50 miniaturas distintas de 400 px | ~100 ms | 44 ms |
//! | 50 originales distintos | ~235 ms | 125 ms |
//!
//! Es exactamente lo que hace cualquier sitio web con un catálogo grande, y por
//! eso un catálogo web se recorre mejor que esto: sirve miniaturas.
//!
//! # Lo que se midió mal, dicho para que no se repita
//!
//! Hubo dos intentos malos antes de ese, y los dos por el instrumento.
//!
//! El primero midió el hueco entre fotogramas mientras se desplazaba: 16 ms con
//! miniatura, 40 con el original. No valía, porque en una ventana GTK ese hueco
//! lo marca el compositor del escritorio, así que 40 ms puede significar "cuesta
//! 40 ms" o "el compositor está dando 25 Hz", y la misma configuración daba 16 o
//! 40 en ejecuciones distintas.
//!
//! El segundo sumó los píxeles de cada `<img>` del documento por cuatro bytes y
//! declaró 183 MB de mapas de bits con cincuenta en pantalla. **Tampoco valía**,
//! y el motivo es peor: el banco de pruebas servía **la misma URL para todas las
//! portadas**, así que el motor descodificaba una imagen y reutilizaba su mapa de
//! bits en las cincuenta. La aritmética describía un documento que no existía. La
//! decisión de servir miniaturas resultó ser correcta, pero esa cifra no la
//! demostraba: con una portada repetida, original y miniatura dan los mismos
//! 2 ms.
//!
//! Lo que sí vale es lo de arriba: portadas distintas, en el motor real, contando
//! `decode()` y no fotogramas. Ver `performance-budgets.md` sección 4.3.
//!
//! # Por qué se generan al pedirlas y no al importar
//!
//! Porque así no hace falta migrar nada ni volver a importar la biblioteca: la
//! primera vez que una portada se pinta, se genera su miniatura y queda en la
//! caché. Las portadas que ya había siguen donde estaban y se convierten solas.
//!
//! Y porque la miniatura es **caché y no dato**: se puede borrar entera sin
//! perder nada, que es la definición de lo que va en la caché.
//!
//! # El original ya no se guarda
//!
//! Antes se guardaba la portada original junto a su miniatura, y
//! la original **es una copia de lo que ya está dentro del EPUB**. Medido sobre
//! la instalación real: 152.064 originales, 22,5 GB, 144 KB de media, el 70 % de
//! todo lo derivado; y la interfaz no servía ninguno, porque la rejilla, la ficha
//! e Inicio piden la miniatura. Con el presupuesto de disco en el 20 %, esas
//! copias eran lo que dejaba sin sitio al índice: se paraba con el 18,8 % de la
//! biblioteca buscable.
//!
//! Así que se guarda **solo la miniatura**, a 400 píxeles, que es lo que se pinta
//! en una pantalla de densidad doble. Una portada que no se deja descodificar -un
//! SVG, un formato raro- se guarda tal cual, porque sin ella no habría nada que
//! enseñar y suelen pesar poco. Las originales que ya había las convierte una
//! pasada de fondo, [`compactar`], sin tocar la base.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::error::{NbError, NbResult};

/// Ancho de la miniatura, en píxeles.
///
/// Cuatrocientos, que es el tamaño al que se pinta en una pantalla de densidad
/// doble: la cuadrícula le da 188 píxeles CSS, y 188 por dos son 376. Servir
/// menos se vería suavizado en esas pantallas; servir más es memoria que nadie
/// mira.
///
/// Baja los mapas de bits de 183 MB a 46 con cincuenta portadas en pantalla, que
/// es el factor cuatro que hace que la rejilla quepa en memoria. Trescientos
/// bajaría a 26, y se descartó porque el ahorro extra no compensa ver las
/// portadas borrosas en la mitad de los portátiles que se venden hoy.
const ANCHO: u32 = 400;

/// Calidad del JPEG de salida.
///
/// Ochenta y dos. Por encima, el archivo crece sin que la diferencia se vea en
/// una portada de doscientos píxeles; por debajo aparecen artefactos en los
/// degradados, que las cubiertas tienen a montones.
const CALIDAD: u8 = 82;

/// Cuántos píxeles se admiten en la imagen de origen.
///
/// Cuarenta millones, que es más que cualquier cubierta y menos que una bomba de
/// descompresión: un PNG de dos kilobytes puede declarar 30.000x30.000 y reventar
/// la memoria al descodificarlo. La portada viene de un archivo que trajo el
/// usuario o de un servidor de terceros, así que es entrada no confiable.
const MAXIMO_PIXELES: u64 = 40_000_000;

/// Resuelve las portadas de una página entera, en paralelo.
///
/// # Por qué en paralelo y no una detrás de otra
///
/// Porque generar una miniatura cuesta unos **40 ms** —medido sobre un original
/// de 800x1200— y una página de la biblioteca tiene hasta cien. En serie, la
/// primera vez que se abre una página se esperarían cuatro segundos con la
/// pantalla en blanco; repartido entre los núcleos que haya, medio segundo.
///
/// Solo la primera vez: después las miniaturas están en la caché y esto es leer
/// cien veces si un archivo existe.
///
/// Se usa `std::thread::scope` y no un grupo de hilos ni `rayon`: son hilos que
/// nacen y mueren dentro de esta llamada, y traer una dependencia para repartir
/// un bucle sería justo lo que la política de dependencias no admite.
pub fn para_rejilla_en_lote(portadas: &Path, nombres: &[Option<String>]) -> Vec<Option<PathBuf>> {
    let cuantos = nombres.iter().filter(|n| n.is_some()).count();
    if cuantos == 0 {
        return vec![None; nombres.len()];
    }

    // Un hilo por núcleo, con tope: por encima de ocho se pelean por el disco y
    // por la memoria más de lo que ganan.
    let hilos = std::thread::available_parallelism()
        .map_or(2, std::num::NonZeroUsize::get)
        .clamp(1, 8)
        .min(cuantos);

    let salida: Vec<Mutex<Option<PathBuf>>> =
        (0..nombres.len()).map(|_| Mutex::new(None)).collect();
    let siguiente = AtomicUsize::new(0);

    std::thread::scope(|ambito| {
        for _ in 0..hilos {
            ambito.spawn(|| loop {
                let i = siguiente.fetch_add(1, Ordering::Relaxed);
                let Some(entrada) = nombres.get(i) else { break };
                if let Some(nombre) = entrada {
                    *salida[i].lock().unwrap_or_else(|p| p.into_inner()) =
                        para_rejilla(portadas, nombre);
                }
            });
        }
    });

    salida
        .into_iter()
        .map(|celda| celda.into_inner().unwrap_or_else(|p| p.into_inner()))
        .collect()
}

/// Ruta que hay que servir para pintar esta portada en una rejilla, si la hay.
///
/// Devuelve la miniatura. Si todavía no existe y queda la original de antes de
/// la regla de miniaturas, la genera y **retira la original**; si no se puede reducir, sirve la
/// original. Que una portada no se deje reducir no es motivo para no enseñarla.
///
/// # Por qué devuelve nada cuando no hay nada
///
/// Porque la caché es **caché**: un limpiador del sistema puede vaciarla, y el
/// catálogo sigue apuntando el nombre lógico de la portada. Antes
/// esta función devolvía la ruta de la original aunque no existiera, así que la
/// ventana pedía un archivo que no estaba: el protocolo de recursos lo anotaba
/// como error, uno por obra, y la rejilla se llenaba de imágenes rotas. Lo
/// encontró el equipo al vaciar la caché del sistema.
///
/// Devolviendo nada, la interfaz pinta la portada que dibuja ella —que ya existe
/// para los libros sin portada— y el runtime aprovecha para recuperarlas: ver
/// `metadata::recuperar_portadas`.
pub fn para_rejilla(portadas: &Path, nombre: &str) -> Option<PathBuf> {
    let original = portadas.join(nombre);
    let mini = portadas.join(nombre_de_miniatura(nombre));

    if mini.is_file() {
        return Some(mini);
    }
    let Ok(bytes) = std::fs::read(&original) else {
        return None;
    };
    match escribir_miniatura(&bytes, &mini) {
        Ok(()) => {
            let _ = std::fs::remove_file(&original);
            Some(mini)
        }
        Err(motivo) => {
            // A nivel de depuración y sin la ruta: un nombre de archivo de
            // portada lleva dentro el identificador de la edición.
            tracing::debug!(motivo, "no se pudo reducir una portada; se sirve entera");
            Some(original)
        }
    }
}

/// Guarda una portada que llega en bytes: su miniatura, y nada más.
///
/// Es lo que usan la extracción al importar y la descarga de un catálogo. Si no
/// se puede descodificar, se guarda tal cual con su nombre, para no quedarse sin
/// portada que enseñar. Devuelve la ruta de lo que quedó escrito.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn guardar(portadas: &Path, nombre: &str, bytes: &[u8]) -> NbResult<PathBuf> {
    std::fs::create_dir_all(portadas).map_err(|_| NbError::StorageFailure)?;
    let mini = portadas.join(nombre_de_miniatura(nombre));
    if escribir_miniatura(bytes, &mini).is_ok() {
        return Ok(mini);
    }
    let original = portadas.join(nombre);
    // Atómica: la interfaz mira si la portada existe desde otro hilo, y una
    // escritura directa la dejaría ver el archivo a medio llenar.
    crate::paths::write_atomic(&original, |temporal| {
        std::fs::write(temporal, bytes).map_err(|_| NbError::StorageFailure)
    })?;
    Ok(original)
}

/// Lo que dejó una pasada de [`compactar`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Compactacion {
    /// Originales retirados, con su miniatura en su sitio.
    pub retiradas: usize,
    /// Originales que no se dejaron reducir y se quedan.
    pub conservadas: usize,
    /// Lo que se devolvió al disco.
    pub bytes_liberados: u64,
}

/// Convierte las portadas originales de en miniaturas.
///
/// Recorre la carpeta **sin tocar la base**: el nombre de cada portada no cambia
/// -la miniatura cuelga del mismo nombre-, así que no hay nada que apuntar. Por
/// eso puede correr con la aplicación abierta sin competir por la escritura, que
/// es lo que enseñó la importación real: un trabajo de fondo que toma un cerrojo exclusivo no
/// es trabajo de fondo.
///
/// Se reparte entre los núcleos, con tope, igual que [`para_rejilla_en_lote`], y
/// se detiene si `seguir` dice que no: cerrar la aplicación a mitad deja la
/// carpeta en un estado válido, porque cada original se retira **después** de que
/// su miniatura esté escrita.
pub fn compactar(portadas: &Path, seguir: &(dyn Fn() -> bool + Sync)) -> Compactacion {
    let Ok(entradas) = std::fs::read_dir(portadas) else {
        return Compactacion::default();
    };
    let originales: Vec<PathBuf> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|ruta| {
            ruta.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| !es_miniatura(n) && !n.ends_with(".parcial"))
        })
        .collect();

    let hilos = std::thread::available_parallelism()
        .map_or(2, std::num::NonZeroUsize::get)
        .clamp(1, 8)
        .min(originales.len().max(1));
    let siguiente = AtomicUsize::new(0);
    let retiradas = AtomicUsize::new(0);
    let conservadas = AtomicUsize::new(0);
    let liberados = std::sync::atomic::AtomicU64::new(0);

    std::thread::scope(|ambito| {
        for _ in 0..hilos {
            ambito.spawn(|| loop {
                if !seguir() {
                    break;
                }
                let i = siguiente.fetch_add(1, Ordering::Relaxed);
                let Some(original) = originales.get(i) else {
                    break;
                };
                let Some(nombre) = original.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let mini = portadas.join(nombre_de_miniatura(nombre));
                let tamano = std::fs::metadata(original).map(|m| m.len()).unwrap_or(0);
                let lista = mini.is_file()
                    || std::fs::read(original)
                        .ok()
                        .is_some_and(|bytes| escribir_miniatura(&bytes, &mini).is_ok());
                if lista && std::fs::remove_file(original).is_ok() {
                    retiradas.fetch_add(1, Ordering::Relaxed);
                    liberados.fetch_add(tamano, Ordering::Relaxed);
                } else {
                    conservadas.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    Compactacion {
        retiradas: retiradas.into_inner(),
        conservadas: conservadas.into_inner(),
        bytes_liberados: liberados.into_inner(),
    }
}

/// Nombre de la miniatura de una portada.
///
/// Se le añade un sufijo en lugar de cambiar la extensión sin más, para que
/// `borrar_portadas` siga encontrando las dos por el mismo prefijo cuando se
/// borra una edición.
fn nombre_de_miniatura(nombre: &str) -> String {
    format!("{nombre}.min.jpg")
}

/// Si un nombre de archivo es una miniatura.
///
/// Lo usa el borrado de portadas: sin esto, quitar una edición dejaría su
/// miniatura huérfana en la caché para siempre.
#[must_use]
pub fn es_miniatura(nombre: &str) -> bool {
    nombre.ends_with(".min.jpg")
}

/// Escribe la miniatura de una portada a partir de sus bytes.
///
/// Una portada más estrecha que la miniatura **no se agranda**, pero se escribe
/// igual como JPEG a su tamaño: la miniatura es lo único que se
/// guarda, así que tiene que existir también para las pequeñas.
///
/// Devuelve el motivo del fallo como una cadena corta y **sin datos**: se
/// registra, y un registro no lleva rutas ni nombres.
fn escribir_miniatura(bytes: &[u8], destino: &Path) -> Result<(), &'static str> {
    let mut lector = image::ImageReader::new(std::io::Cursor::new(bytes));
    lector = lector
        .with_guessed_format()
        .map_err(|_| "no se pudo reconocer")?;

    // El límite va **antes** de descodificar, que es lo único que sirve: después
    // ya se habría reservado la memoria que se quería evitar.
    let mut limites = image::Limits::default();
    limites.max_image_width = Some(20_000);
    limites.max_image_height = Some(20_000);
    limites.max_alloc = Some(MAXIMO_PIXELES * 4);
    lector.limits(limites);

    let imagen = lector.decode().map_err(|_| "no se pudo descodificar")?;

    let pequena = if imagen.width() <= ANCHO {
        imagen
    } else {
        let alto =
            (u64::from(imagen.height()) * u64::from(ANCHO) / u64::from(imagen.width())).max(1);
        let alto = u32::try_from(alto).map_err(|_| "proporción imposible")?;
        // `Triangle` y no `Lanczos3`: sobre una reducción de tres a uno la
        // diferencia no se ve a doscientos píxeles, y cuesta la mitad.
        imagen.resize_exact(ANCHO, alto, image::imageops::FilterType::Triangle)
    };

    // A RGB: un JPEG no tiene canal alfa, y guardar una imagen con alfa como
    // JPEG falla en lugar de aplanarla.
    let rgb = pequena.to_rgb8();

    let temporal = destino.with_extension("parcial");
    {
        let archivo = std::fs::File::create(&temporal).map_err(|_| "no se pudo crear")?;
        let mut salida = std::io::BufWriter::new(archivo);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut salida, CALIDAD)
            .encode_image(&rgb)
            .map_err(|_| "no se pudo codificar")?;
    }
    // Renombrar es atómico dentro del mismo sistema de archivos: sin esto, dos
    // peticiones a la vez podrían leer una miniatura a medio escribir.
    std::fs::rename(&temporal, destino).map_err(|_| "no se pudo mover")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Escribe un PNG de un tamaño dado, para poder probar sin ficheros de apoyo.
    fn png(ruta: &Path, ancho: u32, alto: u32) {
        let imagen = image::RgbImage::from_fn(ancho, alto, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        imagen.save(ruta).expect("escribir png");
    }

    fn temporal(nombre: &str) -> PathBuf {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-portadas-{nombre}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("carpeta");
        dir
    }

    /// Lo que hace que la biblioteca se recorra a sesenta fotogramas y no a
    /// veinticinco: lo que se sirve tiene el tamaño al que se pinta.
    #[test]
    fn una_portada_grande_se_sirve_reducida() {
        let dir = temporal("grande");
        png(&dir.join("a.png"), 800, 1200);

        let servida = para_rejilla(&dir, "a.png").expect("hay portada");

        assert!(servida.ends_with("a.png.min.jpg"), "{servida:?}");
        let reducida = image::open(&servida).expect("abrir");
        assert_eq!(reducida.width(), ANCHO);
        assert_eq!(reducida.height(), 600, "la proporción se conserva");
    }

    /// La segunda vez no se vuelve a generar: es caché, y una caché que se
    /// rehace cada vez no es una caché.
    #[test]
    fn la_miniatura_se_reutiliza() {
        let dir = temporal("cache");
        png(&dir.join("b.png"), 600, 900);

        let primera = para_rejilla(&dir, "b.png").expect("hay portada");
        let momento = std::fs::metadata(&primera)
            .and_then(|m| m.modified())
            .expect("fecha");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let segunda = para_rejilla(&dir, "b.png").expect("hay portada");

        assert_eq!(primera, segunda);
        assert_eq!(
            std::fs::metadata(&segunda)
                .and_then(|m| m.modified())
                .expect("fecha"),
            momento,
            "se volvió a escribir"
        );
    }

    /// Una portada que ya es pequeña no se agranda, pero tiene su miniatura a su
    /// tamaño: es lo único que se guarda.
    #[test]
    fn una_portada_pequena_no_se_agranda() {
        let dir = temporal("pequena");
        png(&dir.join("c.png"), 200, 300);

        let servida = para_rejilla(&dir, "c.png").expect("hay portada");

        assert!(servida.ends_with("c.png.min.jpg"), "{servida:?}");
        assert_eq!(image::open(&servida).expect("abrir").width(), 200);
    }

    /// Al servir una original de antes, queda la miniatura y la
    /// original se retira: era una copia de lo que ya está en el libro.
    #[test]
    fn al_reducir_una_original_de_antes_la_original_se_retira() {
        let dir = temporal("retira");
        png(&dir.join("r.png"), 800, 1200);

        let servida = para_rejilla(&dir, "r.png").expect("hay portada");

        assert!(servida.ends_with("r.png.min.jpg"));
        assert!(
            !dir.join("r.png").exists(),
            "la original sigue ocupando disco"
        );
    }

    /// Guardar una portada nueva deja solo su miniatura, y lo que no es una
    /// imagen se guarda tal cual para no quedarse sin portada.
    #[test]
    fn guardar_deja_solo_la_miniatura_y_lo_ilegible_tal_cual() {
        let dir = temporal("guardar");
        let mut bytes = Vec::new();
        image::RgbImage::from_fn(900, 1350, |x, _| image::Rgb([(x % 256) as u8, 40, 90]))
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("png");

        let guardada = guardar(&dir, "g.png", &bytes).expect("guarda");
        assert!(guardada.ends_with("g.png.min.jpg"));
        assert!(!dir.join("g.png").exists());

        let rara = guardar(&dir, "s.svg", b"<svg></svg>").expect("guarda");
        assert!(rara.ends_with("s.svg"));
        assert_eq!(std::fs::read(rara).expect("lee"), b"<svg></svg>");
    }

    /// La pasada de fondo convierte las originales, conserva lo que no se deja
    /// reducir, cuenta lo liberado y se detiene cuando se le pide.
    #[test]
    fn compactar_retira_las_originales_y_conserva_lo_ilegible() {
        let dir = temporal("compactar");
        png(&dir.join("a.png"), 800, 1200);
        png(&dir.join("b.png"), 600, 900);
        std::fs::write(dir.join("c.svg"), b"<svg/>").expect("svg");

        assert_eq!(
            compactar(&dir, &|| false).retiradas,
            0,
            "parada, no toca nada"
        );
        assert!(dir.join("a.png").exists());

        let hecho = compactar(&dir, &|| true);
        assert_eq!(hecho.retiradas, 2);
        assert_eq!(hecho.conservadas, 1);
        assert!(hecho.bytes_liberados > 0);
        assert!(dir.join("a.png.min.jpg").is_file());
        assert!(!dir.join("a.png").exists());
        assert!(dir.join("c.svg").exists());
        // Y repetirla no hace nada: las miniaturas no son originales.
        assert_eq!(compactar(&dir, &|| true).retiradas, 0);
    }

    /// Que una portada no se deje reducir no es motivo para no enseñarla.
    #[test]
    fn lo_que_no_es_una_imagen_se_sirve_tal_cual_en_lugar_de_desaparecer() {
        let dir = temporal("rota");
        std::fs::write(dir.join("d.jpg"), b"esto no es una imagen").expect("escribir");

        let servida = para_rejilla(&dir, "d.jpg").expect("hay portada");

        assert!(servida.ends_with("d.jpg"));
        assert!(!servida.ends_with(".min.jpg"));
    }

    /// **Y una portada que no está no se sirve: se dice que no hay.**
    ///
    /// Cambiado el 2026-09-18, y el motivo es que la versión anterior de esta
    /// prueba fijaba el defecto. Decía que devolver la ruta de un archivo que no
    /// existe estaba bien porque "el webview enseñará el hueco". Lo que enseña es
    /// una imagen rota, y el protocolo de recursos anota un error por cada una:
    /// Se vació la caché del sistema y se encontró la biblioteca entera así.
    ///
    /// Devolviendo nada, la interfaz pinta la portada que dibuja ella —la misma
    /// que para un libro sin portada— y el runtime la recupera del documento en
    /// segundo plano: `metadata::recuperar_portadas`.
    #[test]
    fn una_portada_que_no_esta_no_se_sirve() {
        let dir = temporal("ausente");
        assert_eq!(para_rejilla(&dir, "e.jpg"), None);
    }

    /// Una página entera se resuelve de una vez, y cada portada acaba en su
    /// sitio: repartir el trabajo entre hilos no puede barajar el orden, o la
    /// rejilla enseñaría la portada de otro libro.
    #[test]
    fn un_lote_conserva_el_orden_y_los_huecos() {
        let dir = temporal("lote");
        for i in 0..6 {
            png(&dir.join(format!("l{i}.png")), 400, 600);
        }
        let nombres: Vec<Option<String>> = (0..6)
            .map(|i| {
                if i % 2 == 0 {
                    Some(format!("l{i}.png"))
                } else {
                    None
                }
            })
            .collect();

        let rutas = para_rejilla_en_lote(&dir, &nombres);

        assert_eq!(rutas.len(), 6);
        for (i, ruta) in rutas.iter().enumerate() {
            match (i % 2 == 0, ruta) {
                (true, Some(r)) => assert!(
                    r.to_string_lossy().contains(&format!("l{i}.png")),
                    "la {i} trae {r:?}"
                ),
                (false, None) => {}
                _ => panic!("la posición {i} no corresponde: {ruta:?}"),
            }
        }
    }

    #[test]
    fn un_lote_vacio_no_arranca_ningun_hilo() {
        let dir = temporal("lote-vacio");
        assert_eq!(para_rejilla_en_lote(&dir, &[None, None]), vec![None, None]);
    }

    #[test]
    fn una_miniatura_se_reconoce_por_su_nombre() {
        // Lo usa el borrado de portadas: sin esto, quitar una edición dejaría su
        // miniatura huérfana en la caché para siempre.
        assert!(es_miniatura("a.png.min.jpg"));
        assert!(!es_miniatura("a.png"));
        assert!(!es_miniatura("a.min.png"));
    }

    /// Y la otra mitad, para que la de arriba no pase por no servir nunca: con
    /// la portada en su sitio, se sirve.
    #[test]
    fn con_la_portada_en_la_cache_si_se_sirve() {
        let dir = temporal("vacia");
        png(&dir.join("si-esta.png"), 400, 600);

        assert!(para_rejilla(&dir, "si-esta.png").is_some());
    }
}
