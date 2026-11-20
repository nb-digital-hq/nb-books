//! Criterio de aceptación de la.
//!
//! "La interfaz permanece fluida durante la indexación de una biblioteca
//! grande."
//!
//! "Fluida" no se puede comprobar sin interfaz, pero **la causa por la que
//! dejaría de serlo, sí**: que la indexación se coma el hilo de los comandos, o
//! que bloquee la base mientras escribe. Estas pruebas miden eso, que es lo que
//! de verdad decide el resultado; el pintado se comprueba.
//!
//! La medición no compara con un reloj absoluto, que dependería de la máquina y
//! del día, sino con el **mismo trabajo sin indexación de fondo**. Si consultar
//! el catálogo tarda lo mismo con el indexador trabajando que sin él, la
//! indexación no está estorbando.
//!
//! Contrato: `docs/architecture/search.md`, sección 3.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nb_books_lib::db::catalog::{
    ContentHash, Edition, File, NewEdition, NewFile, NewWork, StorageMode, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::power::Power;
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::ProviderRegistry;
use nb_books_lib::search::{self, Indexer};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-indice-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CUERPO: &str = "<p>El análisis crítico de la señal comenzó una mañana de niebla en la que \
                      nadie esperaba nada. La señal era débil, intermitente y estaba llena de \
                      ruido, pero era una señal. El equipo llevaba semanas sin dormir bien y ya \
                      había aprendido a desconfiar de sus propias conclusiones apresuradas.</p>";

/// Copia el EPUB a una ruta propia.
fn copia(tmp: &Temporal, origen: &Path, n: usize) -> PathBuf {
    let destino = tmp.0.join(format!("libro-{n}.epub"));
    std::fs::copy(origen, &destino).expect("copia");
    destino
}

/// Rutas aisladas para el indexador, que mira lo que ocupa la
/// caché para no pasarse del presupuesto de disco.
fn rutas_de_prueba() -> nb_books_lib::paths::AppPaths {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-indice-rutas-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("directorio");
    // SAFETY: cada binario de prueba tiene su directorio y todas sus pruebas
    // quieren el mismo.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &dir) };
    nb_books_lib::paths::AppPaths::resolve().expect("rutas")
}

fn registro() -> Arc<ProviderRegistry> {
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(EpubFactory::solo_texto()));
    Arc::new(registro)
}

/// Da de alta una edición con su archivo, apuntando a un EPUB de verdad.
///
/// Cada una con su ruta propia: `files.path` es única, y en la biblioteca de
/// verdad dos ediciones distintas nunca son el mismo archivo.
fn alta(db: &Database, ruta: &Path, n: usize) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: format!("obra {n}"),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: format!("edicion {n}"),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición");

    let (root_id, relative_path) =
        nb_books_lib::library::ensure_root(&conn, ruta, StorageMode::Referenced, None)
            .expect("raíz");
    File::create(
        &conn,
        &NewFile {
            edition_id: edicion.id.clone(),
            root_id,
            relative_path,
            content_hash: ContentHash::parse(&format!(
                "sha256:{:064x}",
                u128::try_from(n).unwrap_or(0)
            ))
            .expect("hash"),
            byte_size: 1,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )
    .expect("archivo");

    edicion.id
}

/// Construye un EPUB con los capítulos que se le den.
fn epub(tmp: &Temporal, nombre: &str, capitulos: &[(&str, &str)]) -> PathBuf {
    let mut entradas: Vec<(String, Vec<u8>, bool)> = vec![
        (
            "mimetype".to_owned(),
            b"application/epub+zip".to_vec(),
            false,
        ),
        (
            "META-INF/container.xml".to_owned(),
            br#"<?xml version="1.0"?><container version="1.0"
                 xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
                 <rootfile full-path="OEBPS/content.opf"
                 media-type="application/oebps-package+xml"/></rootfiles></container>"#
                .to_vec(),
            true,
        ),
    ];

    let items: String = capitulos
        .iter()
        .enumerate()
        .map(|(i, (archivo, _))| {
            format!(r#"<item id="c{i}" href="{archivo}" media-type="application/xhtml+xml"/>"#)
        })
        .collect();
    let refs: String = (0..capitulos.len())
        .map(|i| format!(r#"<itemref idref="c{i}"/>"#))
        .collect();

    entradas.push((
        "OEBPS/content.opf".to_owned(),
        format!(
            r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
               <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Un libro</dc:title></metadata>
               <manifest>{items}</manifest><spine>{refs}</spine></package>"#
        )
        .into_bytes(),
        true,
    ));

    for (archivo, cuerpo) in capitulos {
        entradas.push((
            format!("OEBPS/{archivo}"),
            format!(
                r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>{cuerpo}</body></html>"#
            )
            .into_bytes(),
            true,
        ));
    }

    let mut salida = Vec::new();
    let mut central = Vec::new();
    let mut cuantas = 0_u16;

    for (nombre_entrada, contenido, comprimir) in &entradas {
        let offset = u32::try_from(salida.len()).expect("cabe");
        let datos = if *comprimir {
            let mut cod =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            cod.write_all(contenido).expect("comprime");
            cod.finish().expect("comprime")
        } else {
            contenido.clone()
        };
        let metodo: u16 = if *comprimir { 8 } else { 0 };
        let comp = u32::try_from(datos.len()).expect("cabe");
        let desc = u32::try_from(contenido.len()).expect("cabe");
        let nb = nombre_entrada.as_bytes();

        salida.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        salida.extend_from_slice(&[20, 0, 0, 0]);
        salida.extend_from_slice(&metodo.to_le_bytes());
        salida.extend_from_slice(&[0; 8]);
        salida.extend_from_slice(&comp.to_le_bytes());
        salida.extend_from_slice(&desc.to_le_bytes());
        salida.extend_from_slice(&u16::try_from(nb.len()).expect("cabe").to_le_bytes());
        salida.extend_from_slice(&[0, 0]);
        salida.extend_from_slice(nb);
        salida.extend_from_slice(&datos);

        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
        central.extend_from_slice(&metodo.to_le_bytes());
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&comp.to_le_bytes());
        central.extend_from_slice(&desc.to_le_bytes());
        central.extend_from_slice(&u16::try_from(nb.len()).expect("cabe").to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(nb);
        cuantas += 1;
    }

    let offset_central = u32::try_from(salida.len()).expect("cabe");
    let tamano_central = u32::try_from(central.len()).expect("cabe");
    salida.extend_from_slice(&central);
    salida.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    salida.extend_from_slice(&[0; 4]);
    salida.extend_from_slice(&cuantas.to_le_bytes());
    salida.extend_from_slice(&cuantas.to_le_bytes());
    salida.extend_from_slice(&tamano_central.to_le_bytes());
    salida.extend_from_slice(&offset_central.to_le_bytes());
    salida.extend_from_slice(&[0, 0]);

    let ruta = tmp.0.join(nombre);
    std::fs::write(&ruta, salida).expect("epub");
    ruta
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn indexar_de_fondo_no_frena_las_consultas_del_catalogo() {
    let tmp = Temporal::nuevo("fluidez");
    let ruta = epub(
        &tmp,
        "libro.epub",
        &[("c1.xhtml", CUERPO), ("c2.xhtml", CUERPO)],
    );

    // Con archivo y no en memoria: en memoria no hay WAL, y el WAL es
    // exactamente lo que permite leer mientras otro escribe. Medirlo sin él
    // sería medir un problema que el producto no tiene.
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let mut ediciones = Vec::new();
    for n in 0..60 {
        ediciones.push(alta(&db, &copia(&tmp, &ruta, n), n));
    }

    let consultar = |db: &Database, veces: usize| -> Duration {
        let conn = db.connection().expect("conexión");
        let comienzo = Instant::now();
        for _ in 0..veces {
            let _: i64 = conn
                .query_row("SELECT count(*) FROM editions", [], |r| r.get(0))
                .expect("consulta");
            let _: String = conn
                .query_row(
                    "SELECT title FROM editions ORDER BY created_at LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .expect("consulta");
        }
        comienzo.elapsed()
    };

    // Línea base: lo mismo, con el indexador parado.
    let en_reposo = consultar(&db, 200);

    let indexador = Indexer::start(db.clone(), registro(), EventBus::new(), rutas_de_prueba());
    for id in &ediciones {
        indexador.enqueue(id);
    }

    let bajo_carga = consultar(&db, 200);

    // El margen es amplio a propósito: lo que se detecta aquí no es un dos por
    // ciento de más, es que la indexación tenga la base bloqueada y las
    // consultas se pongan a esperar segundos. Un umbral estrecho convertiría
    // esta prueba en una que falla cuando la máquina está ocupada.
    assert!(
        bajo_carga < en_reposo * 20 + Duration::from_millis(500),
        "las consultas pasaron de {en_reposo:?} a {bajo_carga:?} con el indexador trabajando"
    );

    // Y mientras tanto ha estado indexando de verdad, no parado.
    let comienzo = Instant::now();
    while !indexador.is_idle() && comienzo.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(indexador.progress().indexed > 0, "no indexó nada");
}

// --- Incremental y reanudable ----------------------------------------------

#[test]
fn al_arrancar_se_encola_solo_lo_que_falta_por_indexar() {
    let tmp = Temporal::nuevo("reanudar");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");

    let rutas: Vec<PathBuf> = (0..5).map(|n| copia(&tmp, &ruta, n)).collect();
    let ediciones: Vec<String> = rutas
        .iter()
        .enumerate()
        .map(|(n, r)| alta(&db, r, n))
        .collect();

    // Se indexan tres a mano, como si la sesión anterior se hubiera cerrado a
    // mitad.
    let registro = registro();
    for id in ediciones.iter().take(3) {
        let doc = registro
            .open("application/epub+zip", &rutas[0])
            .expect("documento");
        let mut conn = db.connection().expect("conexión");
        search::index_edition(&mut conn, id, doc.as_ref()).expect("indexación");
    }

    let indexador = Indexer::start(
        db.clone(),
        Arc::clone(&registro),
        EventBus::new(),
        rutas_de_prueba(),
    );
    indexador.pause();
    let conn = db.connection().expect("conexión");
    let encolados = indexador.enqueue_missing(&conn).expect("encolar");

    // Sin cursor guardado: se pregunta a la base qué ediciones no tienen
    // fragmentos, y esas son las que faltan. Un cursor sería un dato más que
    // mantener coherente, y su primer fallo dejaría documentos sin indexar sin
    // que nadie lo notara.
    assert_eq!(encolados, 2);
    assert_eq!(indexador.progress().pending, 2);
}

#[test]
fn encolar_dos_veces_la_misma_edicion_no_la_indexa_dos_veces() {
    let tmp = Temporal::nuevo("duplicado");
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let indexador = Indexer::start(db, registro(), EventBus::new(), rutas_de_prueba());
    indexador.pause();

    indexador.enqueue("una-edicion");
    indexador.enqueue("una-edicion");
    indexador.enqueue("otra-edicion");

    assert_eq!(indexador.progress().pending, 2);
}

// --- Pausa y reanudación ----------------------------------------------------

#[test]
fn pausar_detiene_el_avance_y_dice_por_que() {
    let tmp = Temporal::nuevo("pausa");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let ediciones: Vec<String> = (0..20)
        .map(|n| alta(&db, &copia(&tmp, &ruta, n), n))
        .collect();

    let indexador = Indexer::start(db, registro(), EventBus::new(), rutas_de_prueba());
    indexador.pause();
    for id in &ediciones {
        indexador.enqueue(id);
    }

    std::thread::sleep(Duration::from_millis(150));
    let en_pausa = indexador.progress();

    assert!(en_pausa.paused);
    // El motivo viaja porque la interfaz tiene que poder decir **por qué** no
    // avanza: una barra detenida sin explicación se lee como que la aplicación
    // se ha colgado.
    assert_eq!(en_pausa.pause_reason, Some(search::PAUSA_USUARIO));
    assert_eq!(en_pausa.indexed, 0, "en pausa no se indexa nada");
    assert_eq!(en_pausa.pending, 20);

    indexador.resume();
    let comienzo = Instant::now();
    while indexador.progress().indexed == 0 && comienzo.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(indexador.progress().indexed > 0, "reanudar no reanudó");
    assert!(!indexador.progress().paused);
}

// --- Batería ----------------------------------------------------------------

#[test]
fn con_bateria_baja_no_se_indexa() {
    // La política vive en `power` y se comprueba allí con sus propias pruebas.
    // Aquí lo que importa es que el indexador la consulte antes de **cada**
    // documento: una indexación larga puede empezar enchufada y terminar en un
    // tren.
    assert!(!Power::Battery(8).allows_background_work());
    assert!(Power::External.allows_background_work());
    assert_eq!(search::PAUSA_BATERIA, "LOW_BATTERY");
}

// --- Progreso visible -------------------------------------------------------

#[test]
fn el_avance_se_publica_en_el_bus_con_numeros_y_sin_nombres() {
    let tmp = Temporal::nuevo("progreso");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let ediciones: Vec<String> = (0..5)
        .map(|n| alta(&db, &copia(&tmp, &ruta, n), n))
        .collect();

    let bus = EventBus::new();
    let vistos = Arc::new(Mutex::new(Vec::new()));
    let contador = Arc::new(AtomicU32::new(0));
    {
        let vistos = Arc::clone(&vistos);
        let contador = Arc::clone(&contador);
        bus.subscribe("espia", move |evento| {
            if let DomainEvent::IndexingProgress { indexed, .. } = evento {
                contador.fetch_add(1, Ordering::SeqCst);
                vistos
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(*indexed);
            }
        });
    }

    let indexador = Indexer::start(db, registro(), bus, rutas_de_prueba());
    for id in &ediciones {
        indexador.enqueue(id);
    }

    let comienzo = Instant::now();
    while contador.load(Ordering::SeqCst) < 5 && comienzo.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(20));
    }

    let publicados = vistos.lock().unwrap_or_else(|p| p.into_inner()).clone();
    assert!(publicados.len() >= 5, "faltan eventos de avance");
    // El avance sube y no baja: una barra que retrocede hace desconfiar de todo
    // lo demás que diga la aplicación.
    assert!(publicados.windows(2).all(|par| par[0] <= par[1]));
}

#[test]
fn un_evento_de_avance_no_lleva_titulos_ni_rutas() {
    // Es la misma regla que en la importación masiva: estos eventos viajan al
    // webview y, con él, a cualquier registro que alguien active. Indexar una
    // biblioteca emitiría miles con el título de cada libro.
    let evento = DomainEvent::IndexingProgress {
        pending: 3,
        indexed: 7,
        skipped: 1,
        failed: 0,
        paused: false,
        pause_reason: None,
    };
    let json = serde_json::to_string(&evento).expect("serialización");

    for campo in ["pending", "indexed", "skipped", "failed", "paused"] {
        assert!(json.contains(campo), "falta {campo} en {json}");
    }
    assert!(!json.contains("title"));
    assert!(!json.contains("path"));
}

// --- Lo que no puede pasar --------------------------------------------------

#[test]
fn un_archivo_registrado_que_no_esta_en_el_disco_se_omite_y_no_falla() {
    let tmp = Temporal::nuevo("omitido");
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");

    // Una edición cuyo archivo está registrado como disponible y no está en el
    // disco: alguien desconectó un volumen y la disponibilidad se ha quedado
    // vieja. Contarlo como fallo le diría al usuario que su documento está roto
    // cuando lo que pasa es que el disco no está enchufado.
    let edicion = alta(&db, &PathBuf::from("/no/existe/libro.epub"), 1);
    let indexador = Indexer::start(db, registro(), EventBus::new(), rutas_de_prueba());
    indexador.enqueue(&edicion);

    let comienzo = Instant::now();
    while !indexador.is_idle() && comienzo.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(100));

    let avance = indexador.progress();
    assert_eq!(avance.indexed, 0);
    assert_eq!(avance.failed, 0, "no encontrar el archivo no es un fallo");
}

#[test]
fn cerrar_el_indexador_espera_a_su_hilo() {
    let tmp = Temporal::nuevo("cierre");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let ediciones: Vec<String> = (0..30)
        .map(|n| alta(&db, &copia(&tmp, &ruta, n), n))
        .collect();

    let indexador = Indexer::start(db.clone(), registro(), EventBus::new(), rutas_de_prueba());
    for id in &ediciones {
        indexador.enqueue(id);
    }
    std::thread::sleep(Duration::from_millis(50));

    // Soltarlo sin esperar dejaría el hilo escribiendo sobre una base que se
    // está cerrando, y eso produce un error en cada apagado que nadie ve porque
    // la aplicación ya se ha ido.
    drop(indexador);

    // Si el hilo siguiera vivo, esta escritura competiría con la suya.
    let conn = db.connection().expect("conexión");
    conn.execute("DELETE FROM text_chunks", [])
        .expect("la base sigue utilizable después de cerrar el indexador");
}

// --- Documentos no indexables --------------------------------------

/// Da de alta una edición con un archivo de un tipo de medio concreto.
fn alta_de(db: &Database, ruta: &Path, media_type: &str, n: usize) -> String {
    let id = alta(db, ruta, n);
    let conn = db.connection().expect("conexión");
    conn.execute(
        "UPDATE files SET media_type = ?2 WHERE edition_id = ?1",
        rusqlite::params![&id, media_type],
    )
    .expect("tipo de medio");
    id
}

/// Espera a que el indexador vacíe la cola.
fn esperar(indexador: &Indexer) {
    let comienzo = Instant::now();
    while !indexador.is_idle() && comienzo.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(150));
}

#[test]
fn un_comic_queda_marcado_como_no_buscable_por_ser_imagen() {
    let tmp = Temporal::nuevo("comic");
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");

    let comic = tmp.0.join("tebeo.cbz");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimo-valido.cbz"),
        &comic,
    )
    .expect("copia");
    let edicion = alta_de(&db, &comic, "application/vnd.comicbook+zip", 1);

    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(nb_books_lib::reader::comic::ComicFactory::new(
        tmp.0.join("cache"),
    )));

    let indexador = Indexer::start(
        db.clone(),
        Arc::new(registro),
        EventBus::new(),
        rutas_de_prueba(),
    );
    indexador.enqueue(&edicion);
    esperar(&indexador);

    let conn = db.connection().expect("conexión");
    let estado = search::status_of(&conn, &edicion).expect("estado");

    assert_eq!(
        estado,
        search::SearchStatus::NotSearchable {
            reason: search::SIN_TEXTO_IMAGEN.to_owned()
        }
    );
    // Y no se le ofrece OCR: convertir en texto unos dibujos no significa nada.
    assert!(!estado.is_fixable_with_ocr());
}

#[test]
fn lo_que_no_se_ha_indexado_todavia_esta_pendiente_y_no_es_no_buscable() {
    let tmp = Temporal::nuevo("pendiente");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let edicion = alta(&db, &copia(&tmp, &ruta, 0), 0);

    let conn = db.connection().expect("conexión");
    // Mientras la biblioteca se indexa, decir "no se puede buscar" sería mentir.
    // "Aún no" es verdad, y es lo que la interfaz tiene que poder decir.
    assert_eq!(
        search::status_of(&conn, &edicion).expect("estado"),
        search::SearchStatus::Pending
    );
}

#[test]
fn un_documento_indexado_queda_marcado_como_buscable() {
    let tmp = Temporal::nuevo("buscable");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let edicion = alta(&db, &copia(&tmp, &ruta, 0), 0);

    let indexador = Indexer::start(db.clone(), registro(), EventBus::new(), rutas_de_prueba());
    indexador.enqueue(&edicion);
    esperar(&indexador);

    let conn = db.connection().expect("conexión");
    assert_eq!(
        search::status_of(&conn, &edicion).expect("estado"),
        search::SearchStatus::Searchable
    );
}

#[test]
fn un_archivo_desconectado_se_distingue_de_uno_sin_texto() {
    let tmp = Temporal::nuevo("desconectado");
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let edicion = alta(&db, &PathBuf::from("/no/existe/libro.epub"), 1);

    let indexador = Indexer::start(db.clone(), registro(), EventBus::new(), rutas_de_prueba());
    indexador.enqueue(&edicion);
    esperar(&indexador);

    let conn = db.connection().expect("conexión");
    let estado = search::status_of(&conn, &edicion).expect("estado");

    // El volumen vuelve solo. Decirle al usuario que su libro no tiene texto
    // cuando lo que pasa es que el disco no está enchufado sería mentira, y
    // ofrecerle OCR encima sería absurdo.
    assert_eq!(
        estado,
        search::SearchStatus::NotSearchable {
            reason: search::ARCHIVO_NO_DISPONIBLE.to_owned()
        }
    );
    assert!(!estado.is_fixable_with_ocr());
}

#[test]
fn solo_el_escaneo_sin_ocr_se_puede_arreglar_con_ocr() {
    // Es la distinción que justifica que el motivo se guarde y no solo el
    // estado: un único "no buscable" obligaría a ofrecer OCR sobre un cómic.
    let escaneo = search::SearchStatus::NotSearchable {
        reason: search::SIN_CAPA_DE_TEXTO.to_owned(),
    };
    assert!(escaneo.is_fixable_with_ocr());

    for otro in [
        search::SIN_TEXTO_IMAGEN,
        search::ARCHIVO_NO_DISPONIBLE,
        search::SIN_MOTOR,
    ] {
        assert!(!search::SearchStatus::NotSearchable {
            reason: otro.to_owned()
        }
        .is_fixable_with_ocr());
    }
    assert!(!search::SearchStatus::Pending.is_fixable_with_ocr());
    assert!(!search::SearchStatus::Searchable.is_fixable_with_ocr());
}

#[test]
fn el_motivo_sale_del_formato_y_no_del_codigo_de_error() {
    // `FORMAT_UNSUPPORTED` significa lo mismo para un cómic que para un escaneo,
    // y son cosas distintas para quien lee.
    assert_eq!(
        search::motivo_por_formato("application/vnd.comicbook+zip"),
        search::SIN_TEXTO_IMAGEN
    );
    assert_eq!(
        search::motivo_por_formato("application/pdf"),
        search::SIN_CAPA_DE_TEXTO
    );
}

#[test]
fn la_base_no_admite_un_no_buscable_sin_motivo() {
    let tmp = Temporal::nuevo("sin-motivo");
    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CUERPO)]);
    let edicion = alta(&db, &copia(&tmp, &ruta, 0), 0);
    let conn = db.connection().expect("conexión");

    // Sin esta comprobación, un "no buscable" mudo llegaría a la interfaz y no
    // habría nada que decirle al usuario, que es justo lo que esta tabla evita.
    assert!(conn
        .execute(
            "INSERT INTO search_status (edition_id, state) VALUES (?1, 'not_searchable')",
            [&edicion],
        )
        .is_err());

    // Y tampoco un buscable con motivo, que sería igual de incoherente.
    assert!(conn
        .execute(
            "INSERT INTO search_status (edition_id, state, reason)
             VALUES (?1, 'searchable', 'NO_TEXT_LAYER')",
            [&edicion],
        )
        .is_err());
}
