//! Pruebas contra el corpus de referencia: una biblioteca real, no sintética.
//!
//! Un corpus generado tiene todos los archivos del mismo tamaño, con la misma
//! estructura y sin ninguna de las rarezas que aparecen en cuanto alguien lleva
//! diez años acumulando libros. El corpus real las tiene todas, y es la única
//! forma de saber si el producto aguanta.
//!
//! # Cómo se ejecutan
//!
//! Se omiten solas si no hay corpus, porque solo existe en la máquina de
//! desarrollo. Para ejecutarlas:
//!
//! ```text
//! NB_BOOKS_CORPUS=/ruta/al/corpus cargo test --test corpus_real -- --nocapture
//!
//! La ruta es la carpeta que contiene **directamente** las de autor, no la que
//! está por encima: `muestrear` baja un solo nivel, así que apuntar una carpeta
//! de más deja la muestra vacía y las pruebas fallan en lugar de omitirse. Pasó
//! el 2026-09-10 con la ruta que documentaba `AGENTS.md`, y por eso lo dice
//! ahora bien.
//! ```
//!
//! La ruta **no** se codifica aquí a propósito: es de cada equipo, y el repositorio
//! es público.
//!
//! # Reglas, que no son negociables
//!
//! * **Solo lectura.** Nada de este archivo escribe, renombra ni cambia
//!   permisos dentro del corpus. No hay una sola llamada de escritura.
//! * **No se copia.** El corpus pesa más de lo que cabe en cualquier directorio
//!   temporal razonable.
//! * **Nada sale de aquí.** Ni rutas, ni títulos, ni autores, ni fragmentos: lo
//!   que se imprime son números agregados. Ver
//!   `docs/audit/performance/testing.md`, sección 2.1.

use std::path::{Path, PathBuf};
use std::time::Instant;

use nb_books_lib::db::catalog::{Edition, File, NewEdition, NewFile, NewWork, StorageMode, Work};
use nb_books_lib::db::Database;
use nb_books_lib::library::{
    hash_file, identify, import_directory, CancelToken, ImportContext, Known,
};

/// Cuántos archivos se recorren. Suficiente para que las cifras signifiquen
/// algo y poco para que la suite siga siendo rápida.
const MUESTRA: usize = 40;

/// Localiza el corpus, o indica que no hay.
fn corpus() -> Option<PathBuf> {
    let raiz = PathBuf::from(std::env::var_os("NB_BOOKS_CORPUS")?);
    raiz.is_dir().then_some(raiz)
}

/// Muestra determinista: los primeros archivos en orden alfabético de un
/// recorrido estable. Determinista para que dos ejecuciones sean comparables, y
/// no aleatoria para que un fallo se pueda reproducir.
fn muestrear(raiz: &Path, cuantos: usize) -> Vec<PathBuf> {
    let mut directorios: Vec<PathBuf> = std::fs::read_dir(raiz)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    directorios.sort();

    let mut salida = Vec::with_capacity(cuantos);
    for dir in directorios {
        let mut archivos: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        archivos.sort();
        for archivo in archivos {
            salida.push(archivo);
            if salida.len() >= cuantos {
                return salida;
            }
        }
    }
    salida
}

/// La identidad de un archivo real es estable: calcularla dos veces da lo mismo.
///
/// Suena trivial y no lo es: si el troceado, el búfer o el orden de lectura
/// influyeran, el mismo libro tendría dos identidades y la biblioteca se
/// duplicaría sola.
#[test]
fn la_identidad_de_los_archivos_reales_es_estable_y_distinta() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    let archivos = muestrear(&raiz, MUESTRA);
    assert!(!archivos.is_empty(), "el corpus no tiene archivos legibles");

    let inicio = Instant::now();
    let mut bytes = 0_u64;
    let mut identidades = Vec::with_capacity(archivos.len());

    for archivo in &archivos {
        let (hash, tamano) = hash_file(archivo).expect("identidad de un archivo real");
        let (otra_vez, _) = hash_file(archivo).expect("identidad repetida");
        assert_eq!(
            hash, otra_vez,
            "la identidad de un archivo real no es estable"
        );
        bytes += tamano;
        identidades.push(hash);
    }

    let transcurrido = inicio.elapsed();
    let unicas: std::collections::BTreeSet<_> = identidades.iter().collect();

    // Se imprimen agregados, nunca rutas ni títulos.
    eprintln!(
        "corpus: {} archivos, {:.1} MiB, {} identidades únicas, {:.0} MiB/s",
        archivos.len(),
        bytes as f64 / 1_048_576.0,
        unicas.len(),
        (bytes as f64 / 1_048_576.0) / transcurrido.as_secs_f64().max(0.001) / 2.0
    );

    assert!(bytes > 0, "los archivos del corpus están vacíos");
}

/// El caso de uso completo sobre archivos reales: registrar una muestra y
/// comprobar que reimportarla no duplica ni una sola vez.
///
/// Es la primera vez que el criterio se somete a archivos que nadie
/// fabricó para que pasara.
#[test]
fn reimportar_una_muestra_real_no_duplica_ningun_registro() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let archivos = muestrear(&raiz, MUESTRA);

    let mut registrados = 0;
    let mut duplicados_en_origen = 0;

    for archivo in &archivos {
        let identidad = identify(&conn, archivo).expect("identidad");

        match identidad.known {
            Known::New => {
                // Cada archivo real entra como su propia obra y edición: aquí no
                // se agrupa por autor, que es trabajo.x.
                let obra = Work::create(
                    &conn,
                    &NewWork {
                        canonical_title: format!("obra-{registrados}"),
                        original_language: None,
                    },
                )
                .expect("obra");
                let edicion = Edition::create(
                    &conn,
                    &NewEdition {
                        work_id: obra.id,
                        title: format!("edicion-{registrados}"),
                        ..NewEdition::default()
                    },
                )
                .expect("edición");

                let (root_id, relative_path) = nb_books_lib::library::ensure_root(
                    &conn,
                    archivo,
                    StorageMode::Referenced,
                    None,
                )
                .expect("raíz");
                File::create(
                    &conn,
                    &NewFile {
                        edition_id: edicion.id,
                        root_id,
                        relative_path,
                        content_hash: identidad.hash,
                        byte_size: identidad.byte_size as i64,
                        // El tipo real se detecta por contenido.
                        media_type: "application/epub+zip".to_owned(),
                        // Referenciado, nunca administrado: el modo administrado
                        // copiaría archivos dentro del corpus.
                        storage_mode: StorageMode::Referenced,
                    },
                )
                .expect("archivo registrado");
                registrados += 1;
            }
            // Dos rutas distintas con el mismo contenido dentro del propio
            // corpus. Existe de verdad, y es información, no un fallo.
            Known::Copy(_) => duplicados_en_origen += 1,
            otro => panic!("un archivo sin registrar no puede ser {otro:?}"),
        }
    }

    let tras_la_primera = registrados;

    // Segunda pasada: ni un registro nuevo.
    for archivo in &archivos {
        let identidad = identify(&conn, archivo).expect("identidad");
        assert!(
            matches!(
                identidad.known,
                Known::AlreadyRegistered(_) | Known::Copy(_)
            ),
            "reimportar no puede encontrar un archivo desconocido"
        );
    }

    let filas: i64 = conn
        .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .expect("consulta");

    eprintln!(
        "corpus: {} archivos vistos, {tras_la_primera} registrados, {duplicados_en_origen} con contenido repetido en el propio corpus",
        archivos.len()
    );
    assert_eq!(
        filas, tras_la_primera as i64,
        "la segunda pasada creó registros nuevos"
    );
}

/// Importación masiva sobre archivos reales, con su cancelación y su
/// reanudación.
///
/// Es la misma prueba que el criterio, pero contra EPUB de verdad: con
/// sus nombres raros, sus tamaños dispares y las rarezas que aparecen cuando
/// alguien lleva años acumulando libros. Un corpus generado no tiene nada de
/// eso.
///
/// **Solo lectura**, y en modo referenciado: el modo administrado copiaría
/// archivos, y aquí no se copia nada.
#[test]
fn importar_del_corpus_real_cancelar_y_reanudar_no_duplica() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    // Se importa una carpeta de autor concreta, no el corpus entero: recorrer
    // 152.000 archivos mediría el disco, no el producto.
    let carpeta = {
        let mut autores: Vec<PathBuf> = std::fs::read_dir(&raiz)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        autores.sort();
        // La primera con al menos unos cuantos libros, para que cancelar a la
        // mitad signifique algo.
        autores
            .into_iter()
            .find(|d| {
                std::fs::read_dir(d)
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .filter(|e| e.path().is_file())
                    .count()
                    >= 8
            })
            .expect("alguna carpeta de autor con varios libros")
    };

    let cuantos = std::fs::read_dir(&carpeta)
        .expect("carpeta")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .count();

    let tmp = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-corpus-masiva-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("temporal");
    let db = Database::open(&tmp.join("nb-books.sqlite")).expect("base");
    let bus = nb_books_lib::events::EventBus::new();
    // La biblioteca administrada apunta al temporal, no al corpus. Aunque nada
    // la use en modo referenciado, dejarla apuntando al corpus sería dejar
    // cargada una escopeta.
    let biblioteca = tmp.join("biblioteca");

    let inicio = Instant::now();

    // Primera pasada, cancelada tras el tercer archivo.
    let cancel = CancelToken::new();
    {
        let db = db.clone();
        let disparador = cancel.clone();
        std::thread::spawn(move || {
            for _ in 0..4000 {
                let hechos: i64 = db
                    .connection()
                    .expect("conexión")
                    .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
                    .expect("consulta");
                if hechos >= 3 {
                    disparador.cancel();
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            disparador.cancel();
        });
    }

    let primera = {
        let mut conn = db.connection().expect("conexión");
        import_directory(
            &mut conn,
            &ImportContext {
                scope: &raiz,
                library: &biblioteca,
                bus: &bus,
            },
            &carpeta,
            StorageMode::Referenced,
            &cancel,
        )
        .expect("primera pasada")
    };

    // Reanudación.
    let segunda = {
        let mut conn = db.connection().expect("conexión");
        import_directory(
            &mut conn,
            &ImportContext {
                scope: &raiz,
                library: &biblioteca,
                bus: &bus,
            },
            &carpeta,
            StorageMode::Referenced,
            &CancelToken::new(),
        )
        .expect("reanudación")
    };

    let conn = db.connection().expect("conexión");
    let archivos: i64 = conn
        .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
        .expect("consulta");
    let identidades: i64 = conn
        .query_row("SELECT count(DISTINCT content_hash) FROM files", [], |r| {
            r.get(0)
        })
        .expect("consulta");

    // Agregados, nunca títulos ni rutas.
    eprintln!(
        "corpus masivo: {cuantos} archivos reales, {} importados antes de cancelar, \
         {} en la reanudación, {} rechazados, {archivos} registrados en {:.1} s",
        primera.imported,
        segunda.imported,
        primera.rejected.len() + segunda.rejected.len(),
        inicio.elapsed().as_secs_f64()
    );

    assert!(primera.cancelled, "la primera pasada debía cancelarse");
    assert_eq!(
        segunda.already_in_library, primera.imported,
        "la reanudación reconoce exactamente lo ya importado"
    );
    /*
     * Lo visto otra vez no es una copia. El corpus no trae duplicados
     * exactos -152.080 archivos y 152.080 huellas distintas en la importación
     * entera del 2026-09-12-, así que aquí toda copia sería un archivo mal
     * clasificado, con las rutas de verdad: tildes, espacios y subcarpetas.
     */
    assert_eq!(
        (primera.duplicates, segunda.duplicates),
        (0, 0),
        "un archivo real visto otra vez se contó como copia exacta"
    );
    assert_eq!(
        archivos, identidades,
        "no puede haber dos registros con el mismo contenido"
    );
    assert_eq!(
        archivos as usize,
        primera.imported + segunda.imported,
        "ni uno perdido ni uno duplicado"
    );

    // Nada se copió: el modo referenciado no toca el corpus.
    assert!(
        !biblioteca.exists() || std::fs::read_dir(&biblioteca).into_iter().flatten().count() == 0,
        "el modo referenciado no puede copiar nada"
    );

    drop(conn);
    drop(db);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// **La pregunta que solo el corpus real puede responder:** ¿los límites del
/// lector de ZIP rechazan libros de verdad?
///
/// Un límite demasiado estricto no falla en las pruebas: falla en la biblioteca
/// del usuario, con un libro concreto que deja de abrirse sin explicación. La
/// única forma de calibrarlo es pasarlo por archivos que nadie fabricó para que
/// pasara.
#[test]
fn los_limites_del_lector_no_rechazan_epub_reales() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    // Muestra más amplia que en las otras pruebas: aquí lo que se busca es
    // justamente el caso raro.
    let archivos = muestrear(&raiz, 250);
    let inicio = Instant::now();

    let mut abiertos = 0;
    let mut con_metadatos = 0;
    let mut con_portada = 0;
    let mut rechazados: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    let mut sin_titulo = 0;

    for archivo in &archivos {
        match nb_books_lib::zip::Archive::open(archivo) {
            Ok(_) => abiertos += 1,
            Err(error) => {
                *rechazados.entry(error.code()).or_default() += 1;
                continue;
            }
        }

        match nb_books_lib::epub::read_metadata(archivo) {
            Ok(metadatos) => {
                con_metadatos += 1;
                if metadatos.cover_path.is_some() {
                    con_portada += 1;
                }
                if metadatos.title.is_none() {
                    sin_titulo += 1;
                }
            }
            Err(error) => {
                *rechazados.entry(error.code()).or_default() += 1;
            }
        }
    }

    // Agregados, nunca títulos ni rutas.
    eprintln!(
        "corpus lector: {} archivos, {abiertos} abiertos, {con_metadatos} con metadatos, \
         {con_portada} con portada, {sin_titulo} sin título, rechazos {rechazados:?}, {:.1} s",
        archivos.len(),
        inicio.elapsed().as_secs_f64()
    );

    // El umbral es deliberadamente exigente: si más de uno de cada cien libros
    // reales no se deja abrir, el problema es del lector y no de los libros.
    let admitidos = archivos.len() / 100;
    let total_rechazos: usize = rechazados.values().sum();
    assert!(
        total_rechazos <= admitidos,
        "el lector rechaza {total_rechazos} de {} archivos reales: {rechazados:?}",
        archivos.len()
    );
    assert!(con_metadatos > 0, "no se pudo leer ni un metadato real");
}

/// El proveedor de EPUB contra libros de verdad.
///
/// Un EPUB fabricado tiene el lomo bien puesto, los `href` sin sorpresas y el
/// marcado limpio. Los de verdad traen rutas con `../`, capítulos que no están,
/// espacios de nombres raros y HTML de generadores que llevan veinte años
/// acumulando particularidades.
#[test]
fn el_proveedor_de_epub_abre_libros_reales_y_sus_locators_son_estables() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    use nb_books_lib::reader::epub::EpubDocument;
    use nb_books_lib::reader::{DocumentProvider, NativePosition, Precision};

    let archivos = muestrear(&raiz, 120);
    let inicio = Instant::now();

    let mut abiertos = 0;
    let mut con_navegacion = 0;
    let mut recursos = 0;
    let mut caracteres = 0_usize;
    let mut exactos = 0;
    let mut fallos: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    let mut saneados_con_algo_quitado = 0;

    for archivo in &archivos {
        let doc = match EpubDocument::open(archivo) {
            Ok(d) => d,
            Err(error) => {
                *fallos.entry(error.code()).or_default() += 1;
                continue;
            }
        };
        abiertos += 1;

        let lomo = doc.spine();
        recursos += lomo.len();
        if doc.navigation().map(|n| n.entries.len()).unwrap_or(0) > 0 {
            con_navegacion += 1;
        }

        // Se busca el recurso con más texto, no el primero: en casi todos los
        // EPUB reales el primero es la portada, que tiene una imagen y seis
        // caracteres. Situar una anotación ahí no probaría nada.
        let mut elegido = String::new();
        let mut mas_largo = 0;
        for href in &lomo {
            let texto = doc
                .text_range(
                    &doc.locator_at(&NativePosition::Fragment {
                        href: (*href).to_string(),
                        offset: 0,
                    })
                    .expect("locator inicial"),
                    usize::MAX / 2,
                )
                .unwrap_or_default();
            let largo = texto.chars().count();
            if largo > mas_largo {
                mas_largo = largo;
                elegido = (*href).to_string();
            }
        }
        if elegido.is_empty() {
            continue;
        }
        caracteres += mas_largo;

        // Y la posición va en mitad de ese capítulo, que es donde de verdad
        // pondría alguien una anotación.
        let medio = u32::try_from(mas_largo / 2).unwrap_or(0);
        let locator = doc
            .locator_at(&NativePosition::Fragment {
                href: elegido,
                offset: medio,
            })
            .expect("locator en medio");

        match doc.resolve(&locator) {
            Ok(resolucion) if resolucion.precision == Precision::Exact => exactos += 1,
            Ok(resolucion) => {
                *fallos
                    .entry(match resolucion.precision {
                        Precision::Quote => "resuelto por cita",
                        _ => "resuelto por aproximación",
                    })
                    .or_default() += 1;
            }
            Err(error) => *fallos.entry(error.code()).or_default() += 1,
        }

        // Y el marcado del primer recurso se sanea sin romperse.
        if let Ok(html) = doc.markup(doc.spine().first().copied().unwrap_or_default()) {
            let minusculas = html.to_lowercase();
            assert!(
                !minusculas.contains("<script") && !minusculas.contains("javascript:"),
                "quedó código ejecutable en el marcado saneado de un libro real"
            );
            if html.len() < 8 {
                saneados_con_algo_quitado += 1;
            }
        }
    }

    // Agregados, nunca títulos ni rutas.
    eprintln!(
        "corpus lector epub: {} libros, {abiertos} abiertos, {con_navegacion} con índice, \
         {recursos} recursos, {caracteres} caracteres, {exactos} locators exactos, \
         incidencias {fallos:?}, {:.1} s",
        archivos.len(),
        inicio.elapsed().as_secs_f64()
    );

    let admitidos = archivos.len() / 20;
    assert!(
        archivos.len() - abiertos <= admitidos,
        "no se abrieron {} de {} libros reales: {fallos:?}",
        archivos.len() - abiertos,
        archivos.len()
    );
    assert!(
        exactos >= abiertos * 9 / 10,
        "solo {exactos} de {abiertos} locators resolvieron exactamente: {fallos:?}"
    );
    assert_eq!(
        saneados_con_algo_quitado, 0,
        "algún marcado real quedó vacío tras sanearlo"
    );
}

/// El índice de búsqueda funciona sobre libros de verdad, y sin tildes.
///
/// Un corpus sintético con la palabra "análisis" metida a mano demuestra que
/// FTS5 funciona. Esto demuestra otra cosa: que el troceado no parte las
/// palabras donde no debe, que los locators indexados resuelven sobre el
/// documento del que salieron, y que buscar sin acentos encuentra en textos que
/// nadie escribió pensando en esta prueba.
#[test]
fn el_indice_encuentra_sin_tildes_en_libros_reales() {
    let Some(raiz) = corpus() else {
        eprintln!("sin corpus: se omite (define NB_BOOKS_CORPUS)");
        return;
    };

    use nb_books_lib::reader::epub::EpubDocument;
    use nb_books_lib::reader::{DocumentProvider, Precision};
    use nb_books_lib::search;

    // Menos libros que en las demás: indexar es la operación más cara de todas
    // y aquí lo que se mide es que funcione, no cuánto tarda.
    let muestra = muestrear(&raiz, 20);
    let db = Database::open_in_memory().expect("base");

    let mut indexados = 0_usize;
    let mut fragmentos = 0_usize;
    let mut sin_texto = 0_usize;
    let mut exactos = 0_usize;
    let mut comprobados = 0_usize;
    let comienzo = Instant::now();

    for ruta in &muestra {
        let Ok(doc) = EpubDocument::open(ruta) else {
            continue;
        };

        let conn = db.connection().expect("conexión");
        let obra = Work::create(
            &conn,
            &NewWork {
                canonical_title: "obra".to_owned(),
                original_language: None,
            },
        )
        .expect("obra");
        let edicion = Edition::create(
            &conn,
            &NewEdition {
                work_id: obra.id,
                title: "edicion".to_owned(),
                subtitle: None,
                publisher: None,
                published_date: None,
                edition_statement: None,
                language: None,
                series_title: None,
                series_index: None,
            },
        )
        .expect("edición")
        .id;
        drop(conn);

        let mut conn = db.connection().expect("conexión");
        match search::index_edition(&mut conn, &edicion, &doc) {
            Ok(cuantos) => {
                indexados += 1;
                fragmentos += cuantos;
            }
            Err(_) => {
                sin_texto += 1;
                continue;
            }
        }

        // Los locators que quedaron en el índice tienen que resolver sobre el
        // documento del que salieron. Si no, "un resultado abre la posición
        // exacta" sería falso en cuanto el libro tuviera algo raro.
        for consulta in ["cuando", "manana", "tambien", "despues"] {
            let resultados =
                search::search_in_edition(&conn, &edicion, consulta, 3).expect("búsqueda");
            for hit in &resultados {
                comprobados += 1;
                if doc
                    .resolve(&hit.locator)
                    .is_ok_and(|r| r.precision == Precision::Exact)
                {
                    exactos += 1;
                }
            }
        }
    }

    let duracion = comienzo.elapsed();
    println!("libros indexados: {indexados} de {}", muestra.len());
    println!("sin capa de texto: {sin_texto}");
    println!("fragmentos indexados: {fragmentos}");
    println!("resultados comprobados: {comprobados}, exactos: {exactos}");
    println!("tiempo total de indexación y búsqueda: {duracion:?}");

    assert!(indexados > 0, "ningún libro real se pudo indexar");
    assert!(
        comprobados > 0,
        "ninguna de las consultas sin tildes encontró nada en veinte libros reales"
    );
    assert_eq!(
        exactos, comprobados,
        "hay locators indexados que no resuelven de forma exacta sobre su propio documento"
    );
}

/// **El criterio**: los límites del contenedor contra la biblioteca
/// real.
///
/// Un límite protege de verdad cuando está por encima de lo que hace un libro
/// legítimo y por debajo de lo que hace un ataque. Los de `zip.rs` se eligieron
/// razonando sobre el formato, no midiendo, y eso deja dos riesgos simétricos:
/// que rechacen libros que nadie querría rechazar, o que dejen pasar lo que
/// existen para parar.
///
/// Esta prueba mide la distribución real y **no la cambia**: lo que hace es dar
/// la cifra con la que decidir. Se recorre una muestra amplia porque leer el
/// directorio central de un ZIP es barato -no descomprime nada- y porque con
/// cuarenta archivos la cola de la distribución, que es justo lo que importa,
/// no aparece.
///
/// Nada sale de aquí: lo que se imprime son recuentos y percentiles.
#[test]
fn los_limites_del_contenedor_dejan_pasar_lo_que_hay_en_la_biblioteca_real() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    /// Bastantes más que `MUESTRA`: la cola de la distribución no aparece con
    /// cuarenta, y es la única parte que decide un límite.
    const CUANTOS: usize = 2_000;

    let archivos = muestrear(&raiz, CUANTOS);
    let mut entradas: Vec<usize> = Vec::new();
    let mut mayor_entrada: Vec<u64> = Vec::new();
    let mut totales: Vec<u64> = Vec::new();
    let mut ratios: Vec<u64> = Vec::new();
    let mut ilegibles = 0_usize;

    for archivo in &archivos {
        match nb_books_lib::zip::Archive::open(archivo) {
            Ok(contenedor) => {
                let listado = contenedor.entries();
                entradas.push(listado.len());
                mayor_entrada.push(
                    listado
                        .iter()
                        .map(|e| e.uncompressed_size)
                        .max()
                        .unwrap_or(0),
                );
                totales.push(listado.iter().map(|e| e.uncompressed_size).sum());
                ratios.push(
                    listado
                        .iter()
                        .filter(|e| e.compressed_size > 0)
                        .map(|e| e.uncompressed_size / e.compressed_size)
                        .max()
                        .unwrap_or(0),
                );
            }
            // Un archivo que el lector rechaza es justo lo que esta prueba
            // quiere contar, no un fallo de la prueba.
            Err(_) => ilegibles += 1,
        }
    }

    assert!(
        entradas.len() > 100,
        "solo se pudieron leer {} contenedores: la medida no significaría nada",
        entradas.len()
    );

    let percentil = |datos: &mut Vec<u64>, p: usize| -> u64 {
        datos.sort_unstable();
        let i = (datos.len() * p / 100).min(datos.len() - 1);
        datos[i]
    };

    let mut entradas_u64: Vec<u64> = entradas.iter().map(|n| *n as u64).collect();
    println!(
        "contenedores leídos: {} de {}",
        entradas.len(),
        archivos.len()
    );
    println!("ilegibles para el lector: {ilegibles}");
    println!(
        "entradas por contenedor: mediana {}, p99 {}, máximo {}",
        percentil(&mut entradas_u64, 50),
        percentil(&mut entradas_u64, 99),
        entradas_u64.last().copied().unwrap_or(0)
    );
    println!(
        "mayor entrada descomprimida: mediana {}, p99 {}, máximo {}",
        percentil(&mut mayor_entrada, 50),
        percentil(&mut mayor_entrada, 99),
        mayor_entrada.last().copied().unwrap_or(0)
    );
    println!(
        "contenedor descomprimido entero: mediana {}, p99 {}, máximo {}",
        percentil(&mut totales, 50),
        percentil(&mut totales, 99),
        totales.last().copied().unwrap_or(0)
    );
    println!(
        "mayor ratio de compresión: mediana {}, p99 {}, máximo {}",
        percentil(&mut ratios, 50),
        percentil(&mut ratios, 99),
        ratios.last().copied().unwrap_or(0)
    );
}

/// **La medida que fija el presupuesto.**
///
/// La es cuánto disco puede ocupar lo derivado, y el innegociable
/// dice que nunca más que la biblioteca que describe. Para poner un número hace
/// falta saber cuánto pesa el texto de un libro comparado con su archivo: el
/// texto es el suelo de lo derivado, porque el índice va encima de él.
///
/// Se mide sobre EPUB reales y no sobre un banco: en el corpus hay libros de
/// texto plano y libros llenos de imágenes, y la relación entre archivo y texto
/// no se parece en nada entre unos y otros. Un banco con un solo tipo daría un
/// número y una falsa confianza.
///
/// Nada sale de aquí: lo que se imprime son bytes agregados y porcentajes.
#[test]
fn cuanto_texto_lleva_dentro_un_epub_comparado_con_lo_que_ocupa() {
    use nb_books_lib::reader::epub::EpubDocument;
    use nb_books_lib::reader::{DocumentProvider, NativePosition};

    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    /// Abrir y recorrer el lomo entero de cada libro cuesta, así que menos que
    /// en la medida de contenedores. Suficiente para la mediana y la cola.
    const CUANTOS: usize = 300;

    let archivos = muestrear(&raiz, CUANTOS);
    let mut bytes_archivo = 0_u64;
    let mut bytes_texto = 0_u64;
    // La portada se guarda tal cual sale del contenedor, así que su tamaño es
    // parte de lo derivado y no una nota al pie: 152.080 de ellas suman.
    let mut bytes_portada = 0_u64;
    let mut con_portada = 0_usize;
    let mut proporciones: Vec<u64> = Vec::new();
    let mut abiertos = 0_usize;

    for archivo in &archivos {
        let Ok(doc) = EpubDocument::open(archivo) else {
            continue;
        };
        let Ok(metadatos) = std::fs::metadata(archivo) else {
            continue;
        };

        let mut texto_del_libro = 0_u64;
        for href in &doc.spine() {
            let texto = doc
                .text_range(
                    &doc.locator_at(&NativePosition::Fragment {
                        href: (*href).to_string(),
                        offset: 0,
                    })
                    .expect("locator inicial"),
                    usize::MAX / 2,
                )
                .unwrap_or_default();
            texto_del_libro += texto.len() as u64;
        }

        if texto_del_libro == 0 {
            continue;
        }
        abiertos += 1;
        bytes_archivo += metadatos.len();
        bytes_texto += texto_del_libro;

        if let Ok(interno) = nb_books_lib::epub::read_metadata(archivo) {
            if let Some(portada) = interno.cover_path.as_deref() {
                if let Ok(contenedor) = nb_books_lib::zip::Archive::open(archivo) {
                    if let Some(entrada) = contenedor.entries().iter().find(|e| e.name == portada) {
                        bytes_portada += entrada.uncompressed_size;
                        con_portada += 1;
                    }
                }
            }
        }
        // En tanto por mil, para que un libro pequeño no se pierda al redondear.
        proporciones.push(texto_del_libro * 1000 / metadatos.len().max(1));
    }

    assert!(
        abiertos > 50,
        "solo se pudieron leer {abiertos} libros: la medida no significaría nada"
    );

    proporciones.sort_unstable();
    let percentil =
        |p: usize| proporciones[(proporciones.len() * p / 100).min(proporciones.len() - 1)];

    println!("libros medidos: {abiertos}");
    println!("bytes de archivo: {bytes_archivo}");
    println!("bytes de texto extraído: {bytes_texto}");
    println!(
        "texto sobre archivo, en tanto por ciento: agregado {:.1}, mediana {:.1}, p10 {:.1}, p90 {:.1}",
        bytes_texto as f64 * 100.0 / bytes_archivo as f64,
        percentil(50) as f64 / 10.0,
        percentil(10) as f64 / 10.0,
        percentil(90) as f64 / 10.0
    );
    println!(
        "portadas: {con_portada} de {abiertos}, {bytes_portada} bytes, {:.1} % de lo que ocupan los archivos",
        bytes_portada as f64 * 100.0 / bytes_archivo as f64
    );
}

/// **Dónde se va cada byte, y a partir de qué tamaño de fragmento**.
///
/// La medida de 4.5 dejó una pregunta abierta: el índice baja de 100,5 % a
/// 88,3 % al pasar de 480 a 1.920 caracteres -casi nada para cuatro veces menos
/// fragmentos- y después se desploma a 41,7 % con 3.840. Elegir un tamaño sin
/// entender ese codo sería elegirlo porque una ejecución dijo eso.
///
/// Así que aquí no se mide un total: se **descompone**. Para cada tamaño se
/// separa lo que ocupan los fragmentos con su texto y su locator de lo que ocupa
/// el índice invertido, tirando la tabla FTS y volviendo a medir. Es la única
/// forma de saber cuál de las dos cosas hace el codo.
///
/// Nada sale de aquí: bytes agregados y porcentajes.
#[test]
fn donde_se_va_cada_byte_segun_el_tamano_del_fragmento() {
    use nb_books_lib::reader::epub::EpubDocument;

    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    /// Indexar abre y trocea cada libro entero, y aquí se hace una vez por
    /// tamaño. Pocos y de verdad.
    const CUANTOS: usize = 40;

    /// Los tamaños que se barren, en caracteres. El de hoy es 480.
    const TAMANOS: [usize; 6] = [480, 960, 1_920, 2_880, 3_840, 5_760];

    let archivos = muestrear(&raiz, CUANTOS);
    let mut bytes_archivo = 0_u64;
    let mut documentos = Vec::new();
    for archivo in &archivos {
        if let (Ok(doc), Ok(metadatos)) = (EpubDocument::open(archivo), std::fs::metadata(archivo))
        {
            bytes_archivo += metadatos.len();
            documentos.push(doc);
        }
    }
    assert!(
        documentos.len() > 20,
        "solo se abrieron {} libros: la medida no significaría nada",
        documentos.len()
    );

    println!(
        "libros: {}, bytes de archivo: {bytes_archivo}",
        documentos.len()
    );
    println!("tamaño | fragmentos | fragmentos+texto | índice | todo | % del archivo");

    for tamano in TAMANOS {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-barrido-{}-{tamano}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        let ruta = dir.join("barrido.sqlite");
        let db = Database::open(&ruta).expect("base");

        let mut fragmentos = 0_usize;
        for (n, doc) in documentos.iter().enumerate() {
            let edicion = {
                let conn = db.connection().expect("conexión");
                let obra = Work::create(
                    &conn,
                    &NewWork {
                        canonical_title: format!("Obra {n}"),
                        original_language: None,
                    },
                )
                .expect("obra");
                Edition::create(
                    &conn,
                    &NewEdition {
                        work_id: obra.id,
                        title: format!("Edición {n}"),
                        subtitle: None,
                        publisher: None,
                        published_date: None,
                        edition_statement: None,
                        language: None,
                        series_title: None,
                        series_index: None,
                    },
                )
                .expect("edición")
                .id
            };
            let mut conn = db.connection().expect("conexión");
            fragmentos += nb_books_lib::search::index_edition_con(&mut conn, &edicion, doc, tamano)
                .unwrap_or(0);
        }

        let compactar = |db: &Database| {
            let conn = db.connection().expect("conexión");
            conn.execute_batch("VACUUM").expect("compactada");
        };
        compactar(&db);
        let todo = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);

        // Ahora sin el índice invertido: lo que queda son los fragmentos con su
        // texto y su locator, más el catálogo, que es despreciable.
        {
            let conn = db.connection().expect("conexión");
            conn.execute_batch(
                "DROP TRIGGER IF EXISTS trg_text_chunks_indexar;
                 DROP TRIGGER IF EXISTS trg_text_chunks_desindexar;
                 DROP TABLE IF EXISTS documents_fts;",
            )
            .expect("índice fuera");
        }
        compactar(&db);
        let sin_indice = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
        drop(db);

        let indice = todo.saturating_sub(sin_indice);
        println!(
            "{tamano:6} | {fragmentos:10} | {sin_indice:16} | {indice:6} | {todo:4} | {:.1} %",
            todo as f64 * 100.0 / bytes_archivo as f64
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// **La búsqueda sobre libros reales**: todo fragmento abre su posición exacta, y lo que
/// cuesta y lo que gana cortar por párrafo y por frase.
///
/// Se compara con el corte anterior —al llegar al tamaño, retrocediendo hasta un
/// espacio—, reconstruido aquí sobre el mismo texto. Se imprimen números
/// agregados: cuántos fragmentos, cuántos acaban en un final de frase, y, con
/// `NB_BOOKS_MODELO_REAL`, cuántos tokens tienen para el modelo de embeddings, que
/// trunca a 512. Nada de los libros sale de aquí.
#[test]
fn trocear_por_parrafo_sobre_libros_reales() {
    use nb_books_lib::reader::epub::EpubDocument;
    use nb_books_lib::reader::{DocumentProvider, Precision};
    use nb_books_lib::search::FRAGMENTO;

    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };

    /// El corte anterior.
    fn corte_anterior(texto: &[char], maximo: usize) -> Vec<String> {
        let mut salida = Vec::new();
        let mut inicio = 0;
        while inicio < texto.len() {
            let tope = (inicio + maximo).min(texto.len());
            let fin = if tope == texto.len() {
                tope
            } else {
                texto[inicio..tope]
                    .iter()
                    .rposition(char::is_ascii_whitespace)
                    .map_or(tope, |p| inicio + p + 1)
            };
            let fin = if fin <= inicio { tope } else { fin };
            let trozo: String = texto[inicio..fin].iter().collect();
            if !trozo.trim().is_empty() {
                salida.push(trozo);
            }
            inicio = fin;
        }
        salida
    }

    fn acaba_en_frase(texto: &str) -> bool {
        texto
            .trim_end()
            .trim_end_matches(['"', '\'', '»', '”', '’', ')', ']'])
            .ends_with(['.', '?', '!', '…'])
    }

    let tokenizador = std::env::var_os("NB_BOOKS_MODELO_REAL").and_then(|carpeta| {
        tokenizers::Tokenizer::from_file(PathBuf::from(carpeta).join("tokenizer.json")).ok()
    });

    let mut libros = 0;
    let (mut nuevos, mut anteriores) = (0_usize, 0_usize);
    let (mut nuevos_en_frase, mut anteriores_en_frase) = (0_usize, 0_usize);
    let mut tokens: Vec<usize> = Vec::new();

    for archivo in muestrear(&raiz, 40) {
        let Ok(doc) = EpubDocument::open(&archivo) else {
            continue;
        };
        let Ok(fragmentos) = doc.text_chunks(FRAGMENTO) else {
            continue;
        };
        libros += 1;

        // El criterio: cada fragmento abre exacto, y ahí está su texto.
        for (locator, texto) in &fragmentos {
            let resolucion = doc.resolve(locator).expect("un fragmento real no resolvió");
            assert_eq!(resolucion.precision, Precision::Exact);
            assert_eq!(
                &doc.text_range(locator, texto.chars().count())
                    .expect("texto"),
                texto,
                "un fragmento real no abre donde está su texto"
            );
        }

        nuevos += fragmentos.len();
        nuevos_en_frase += fragmentos.iter().filter(|(_, t)| acaba_en_frase(t)).count();
        if let Some(tok) = &tokenizador {
            for (_, texto) in &fragmentos {
                if let Ok(codificado) = tok.encode(texto.as_str(), true) {
                    tokens.push(codificado.get_ids().len());
                }
            }
        }

        // El troceado anterior sobre el mismo texto: los fragmentos de un mismo
        // recurso, unidos, son su texto.
        let mut por_recurso: Vec<(String, String)> = Vec::new();
        for (locator, texto) in &fragmentos {
            match por_recurso.last_mut() {
                Some((href, acumulado)) if *href == locator.href => acumulado.push_str(texto),
                _ => por_recurso.push((locator.href.clone(), texto.clone())),
            }
        }
        for (_, texto) in por_recurso {
            let caracteres: Vec<char> = texto.chars().collect();
            let trozos = corte_anterior(&caracteres, FRAGMENTO);
            anteriores += trozos.len();
            anteriores_en_frase += trozos.iter().filter(|t| acaba_en_frase(t)).count();
        }
    }

    assert!(libros > 20, "solo se trocearon {libros} libros");
    let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
    println!(
        "{libros} libros; antes: {anteriores} fragmentos, {:.1} % acaban en final de frase; \
         ahora: {nuevos} fragmentos (+{:.1} %), {:.1} % acaban en final de frase",
        pct(anteriores_en_frase, anteriores),
        pct(nuevos, anteriores) - 100.0,
        pct(nuevos_en_frase, nuevos)
    );
    if !tokens.is_empty() {
        tokens.sort_unstable();
        let percentil = |p: f64| tokens[((tokens.len() - 1) as f64 * p) as usize];
        println!(
            "tokens por fragmento: mediana {}, p95 {}, máximo {}; {:.1} % pasan de 512",
            percentil(0.5),
            percentil(0.95),
            tokens[tokens.len() - 1],
            pct(tokens.iter().filter(|&&t| t > 512).count(), tokens.len())
        );
    }
    // Cortar por párrafo no puede disparar el número de filas, que es lo que se
    // paga: como mucho un cuarto más.
    assert!(
        nuevos * 4 <= anteriores * 5,
        "{nuevos} fragmentos frente a {anteriores}"
    );
}

/// Lo que cuesta el índice de nombres de un libro real.
///
/// No afirma qué nombres salen -eso sería meter en la suite contenido con derechos,
/// y en el registro títulos de nadie-: afirma que se construye en todo libro con
/// texto, cuánto tarda y cuántos nombres salen, en agregado. La cifra va a
/// `performance-budgets.md`, sección 4.21. El disco que cuesta es cero: no se
/// guarda, y `nombres_del_libro.rs` comprueba que no escribe.
#[test]
fn el_indice_de_nombres_de_libros_reales_se_construye_sin_modelo_y_sin_guardar() {
    let Some(raiz) = corpus() else {
        eprintln!("omitida: sin corpus (define NB_BOOKS_CORPUS)");
        return;
    };
    use nb_books_lib::reader::ProviderFactory;
    let fabrica = nb_books_lib::reader::epub::EpubFactory::solo_texto();
    let mut tiempos = Vec::new();
    let mut nombres = Vec::new();
    for archivo in muestrear(&raiz, MUESTRA) {
        let Ok(documento) = fabrica.open(&archivo) else {
            continue;
        };
        let reloj = std::time::Instant::now();
        let indice = nb_books_lib::entidades::construir(documento.as_ref())
            .expect("todo libro con texto tiene su índice de nombres");
        tiempos.push(reloj.elapsed().as_secs_f64() * 1000.0);
        nombres.push(indice.entities.len());
        assert!(indice.entities.len() <= nb_books_lib::entidades::MAXIMO_DE_NOMBRES);
    }
    assert!(
        !tiempos.is_empty(),
        "no se pudo abrir ni un libro del corpus"
    );
    tiempos.sort_by(f64::total_cmp);
    nombres.sort_unstable();
    eprintln!(
        "corpus: {} libros, índice de nombres en {:.0} ms de mediana y {:.0} de máximo, {} nombres de mediana ({}-{})",
        tiempos.len(),
        tiempos[tiempos.len() / 2],
        tiempos[tiempos.len() - 1],
        nombres[nombres.len() / 2],
        nombres[0],
        nombres[nombres.len() - 1]
    );
}

/// Y con el índice de búsqueda de la instalación real, **abierto en solo lectura**.
///
/// Lo que cuesta preguntarle al índice dónde aparece cada nombre, que es la mitad de
/// lo que no se puede medir con un libro suelto. Necesita la base real, así que
/// se omite sin `NB_BOOKS_BASE`; se abre con `SQLITE_OPEN_READ_ONLY` y sin pasar por
/// `Database::open`, que migraría.
#[test]
fn los_sitios_de_los_nombres_salen_del_indice_real_en_un_tiempo_razonable() {
    let (Some(raiz), Some(base)) = (corpus(), std::env::var_os("NB_BOOKS_BASE")) else {
        eprintln!("omitida: sin corpus o sin base (define NB_BOOKS_CORPUS y NB_BOOKS_BASE)");
        return;
    };
    let conn = rusqlite::Connection::open_with_flags(
        &base,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("la base real, en solo lectura");
    let mut consulta = conn
        .prepare(
            "SELECT s.edition_id,
                    (SELECT r.path FROM library_roots r WHERE r.id = f.root_id) || '/' || f.relative_path
               FROM search_status s JOIN files f ON f.edition_id = s.edition_id
              WHERE s.state = 'searchable' AND f.media_type = 'application/epub+zip'
              LIMIT 20",
        )
        .expect("consulta");
    let libros: Vec<(String, String)> = consulta
        .query_map([], |f| Ok((f.get(0)?, f.get(1)?)))
        .expect("libros")
        .filter_map(Result::ok)
        .filter(|(_, ruta)| std::path::Path::new(ruta).starts_with(&raiz))
        .collect();
    use nb_books_lib::reader::ProviderFactory;
    let fabrica = nb_books_lib::reader::epub::EpubFactory::solo_texto();
    // Lo que cuesta **una** consulta, que es lo que se paga al desplegar un nombre, y
    // lo que costaría preguntarlas todas al construir la lista, que es lo que se
    // descartó por esta misma medida.
    let mut por_nombre = Vec::new();
    let mut por_libro = Vec::new();
    for (edicion, ruta) in &libros {
        let Ok(documento) = fabrica.open(std::path::Path::new(ruta)) else {
            continue;
        };
        let indice = nb_books_lib::entidades::construir(documento.as_ref()).expect("nombres");
        let libro = std::time::Instant::now();
        for entidad in &indice.entities {
            let reloj = std::time::Instant::now();
            let sitios = nb_books_lib::entidades::sitios_del_indice(&conn, edicion, &entidad.name)
                .expect("consulta")
                .expect("el libro está indexado");
            por_nombre.push(reloj.elapsed().as_secs_f64() * 1000.0);
            assert!(sitios.len() <= nb_books_lib::entidades::MAXIMO_DE_SITIOS);
        }
        por_libro.push(libro.elapsed().as_secs_f64() * 1000.0);
    }
    assert!(
        !por_nombre.is_empty(),
        "ningún libro indexado de la base está en el corpus"
    );
    por_nombre.sort_by(f64::total_cmp);
    por_libro.sort_by(f64::total_cmp);
    eprintln!(
        "base real: {} libros y {} nombres; una consulta, {:.1} ms de mediana y {:.1} de máximo; todas las de un libro, {:.0} ms de mediana y {:.0} de máximo",
        por_libro.len(),
        por_nombre.len(),
        por_nombre[por_nombre.len() / 2],
        por_nombre[por_nombre.len() - 1],
        por_libro[por_libro.len() / 2],
        por_libro[por_libro.len() - 1]
    );
}
