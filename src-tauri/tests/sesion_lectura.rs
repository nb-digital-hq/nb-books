//! Criterio de aceptación de la.
//!
//! "Abrir un cómic no muestra controles de tipografía; abrir un EPUB sí."
//!
//! Aquí se comprueba la mitad que decide: la sesión transporta lo que el
//! proveedor declara, sin interpretarlo. La mitad que pinta se comprueba en
//! `src/lib/core/reader.test.ts`, y el control visual llega.
//!
//! Lo que estas pruebas defienden no es una lista de botones, es una relación de
//! dependencia: la interfaz **no decide** qué se puede hacer con un documento.
//! Si lo decidiera, el día que entre el audiolibro habría que acordarse de ir a
//! la pantalla para que no ofrezca cambiar la tipografía de algo sin letras.
//!
//! Contrato: `docs/architecture/reader-contracts.md`, sección 4.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nb_books_lib::db::catalog::{Availability, File, StorageMode};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::reader::comic::ComicFactory;
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::progress::ProgressTracker;
use nb_books_lib::reader::{
    NativePosition, Precision, ProviderRegistry, ReadingSession, Rendered, SessionStore,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    _dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    cache: PathBuf,
    db: Database,
    bus: EventBus,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-sesion-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        let cache = dir.join("cache/paginas");
        std::fs::create_dir_all(&scope).expect("origen");
        std::fs::create_dir_all(&library).expect("biblioteca");
        std::fs::create_dir_all(&cache).expect("caché");

        let db = Database::open(&dir.join("nb-books.sqlite")).expect("base");
        Self {
            _dir: dir,
            scope,
            library,
            cache,
            db,
            bus: EventBus::new(),
        }
    }

    /// Copia un fixture al ámbito importable y lo da de alta.
    fn importar(&self, fixture: &str) -> String {
        let destino = self.scope.join(fixture);
        std::fs::copy(fixtures().join(fixture), &destino).expect("copia");

        let mut conn = self.db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: destino,
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importación");

        match resultado {
            ImportOutcome::Imported(datos) => datos.file.id.clone(),
            ImportOutcome::AlreadyInLibrary(archivo) | ImportOutcome::Duplicate(archivo) => {
                archivo.id.clone()
            }
        }
    }

    /// Registrador de progreso con el intervalo de producción.
    fn tracker(&self) -> Arc<ProgressTracker> {
        Arc::new(ProgressTracker::new(self.db.clone()))
    }

    /// Registro con los dos proveedores que el módulo pone a prueba.
    fn registro(&self) -> ProviderRegistry {
        let mut registro = ProviderRegistry::new();
        registro.register(Box::new(EpubFactory::solo_texto()));
        registro.register(Box::new(ComicFactory::new(self.cache.clone())));
        registro
    }
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn abrir_un_comic_no_ofrece_tipografia_y_abrir_un_epub_si() {
    let escenario = Escenario::nuevo("criterio");
    let comic = escenario.importar("minimo-valido.cbz");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();
    let sesion_comic =
        ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
            .expect("sesión de cómic");
    let sesion_libro =
        ReadingSession::open(&escenario.db, &registro, &libro, escenario.tracker(), None)
            .expect("sesión de EPUB");

    assert!(
        !sesion_comic.capabilities().typography,
        "un cómic no tiene tipografía que ajustar"
    );
    assert!(
        sesion_libro.capabilities().typography,
        "un EPUB sí, y es lo que hace que el lector sirva para leer"
    );

    // Y no es solo la tipografía: la diferencia recorre las seis capacidades.
    assert!(!sesion_comic.capabilities().reflow);
    assert!(sesion_libro.capabilities().reflow);
    assert!(!sesion_comic.capabilities().text_extraction);
    assert!(sesion_libro.capabilities().text_extraction);
}

#[test]
fn las_capacidades_las_declara_el_proveedor_y_la_sesion_no_las_toca() {
    let escenario = Escenario::nuevo("transporte");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let sesion = ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
        .expect("sesión");
    let directo = registro
        .open(
            "application/vnd.comicbook+zip",
            &escenario.scope.join("minimo-valido.cbz"),
        )
        .expect("proveedor directo");

    // Si esto se desviara alguna vez, significaría que la sesión decide algo, y
    // entonces habría dos sitios donde vive la respuesta a "¿qué se puede hacer
    // con este documento?".
    assert_eq!(sesion.capabilities(), directo.capabilities());
}

// --- Lo que la sesión tiene que hacer bien ----------------------------------

#[test]
fn abrir_devuelve_capacidades_indice_y_posicion_en_una_sola_respuesta() {
    let escenario = Escenario::nuevo("apertura");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let sesion = ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
        .expect("sesión");
    let info = sesion.info();

    // Todo junto: la alternativa es que la interfaz pida tres cosas y pinte tres
    // veces mientras llegan.
    assert_eq!(info.file_id, comic);
    assert!(!info.edition_id.is_empty());
    assert_eq!(info.media_type, "application/vnd.comicbook+zip");
    assert_eq!(info.navigation.entries.len(), 4);
    assert_eq!(info.locator.locations.position, Some(1));
    assert!(!info.approximate);
}

#[test]
fn un_formato_sin_proveedor_dice_que_falta_un_motor_y_no_que_el_libro_este_roto() {
    let escenario = Escenario::nuevo("sin-motor");
    let pdf = escenario.importar("minimo-valido.pdf");

    // Registro sin PDF: es la instalación de quien no tiene el motor nativo.
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(EpubFactory::solo_texto()));

    assert_eq!(
        ReadingSession::open(&escenario.db, &registro, &pdf, escenario.tracker(), None).err(),
        Some(NbError::ComponentUnavailable),
        "falta un motor, no está roto el documento; la interfaz reacciona distinto"
    );
}

#[test]
fn un_archivo_que_no_esta_registrado_no_es_lo_mismo_que_uno_que_no_se_puede_leer() {
    let escenario = Escenario::nuevo("ausencias");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    // No existe en el catálogo.
    assert_eq!(
        ReadingSession::open(
            &escenario.db,
            &registro,
            "00000000000000000000000000000000",
            escenario.tracker(),
            None
        )
        .err(),
        Some(NbError::DocumentNotFound)
    );

    // Existe, pero su volumen está desconectado. La interfaz dice "conecta el
    // disco", no "este documento no existe".
    let conn = escenario.db.connection().expect("conexión");
    File::set_availability(&conn, &comic, Availability::Missing).expect("marcar");
    assert_eq!(
        ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None).err(),
        Some(NbError::FileNotFound)
    );
}

#[test]
fn moverse_por_el_documento_conserva_la_cita_del_locator() {
    let escenario = Escenario::nuevo("movimiento");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();

    let sesion = ReadingSession::open(&escenario.db, &registro, &libro, escenario.tracker(), None)
        .expect("sesión");
    let destino = sesion.navigation().entries[0].locator.clone();

    let resolucion = sesion.go_to(&destino).expect("movimiento");
    assert_eq!(resolucion.precision, Precision::Exact);

    // Se guarda el locator que pidió quien llama, no uno reconstruido: el
    // original lleva la cita, y perderla dejaría a la siguiente resolución sin
    // el segundo escalón de la cascada de ADR-004.
    assert_eq!(sesion.current().expect("actual"), destino);
}

#[test]
fn una_posicion_aproximada_se_devuelve_marcada_en_lugar_de_tragarsela() {
    let escenario = Escenario::nuevo("aproximada");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let sesion = ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
        .expect("sesión");

    // Un locator guardado cuando el cómic tenía cien páginas.
    let mut viajero = sesion.navigation().entries[3].locator.clone();
    viajero.locations.position = Some(97);
    viajero.locations.total_progression = Some(0.97);

    let resolucion = sesion.go_to(&viajero).expect("movimiento degradado");
    assert_eq!(resolucion.precision, Precision::Approximate);
    assert!(
        resolucion.precision.requires_warning(),
        "quien llama tiene que poder avisar; tragárselo es lo que ADR-004 prohíbe"
    );
}

#[test]
fn pintar_la_posicion_actual_devuelve_lo_que_el_formato_es() {
    let escenario = Escenario::nuevo("pintado");
    let comic = escenario.importar("minimo-valido.cbz");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();
    let sesion_comic =
        ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
            .expect("sesión");
    assert!(matches!(
        sesion_comic.render().expect("pintado"),
        Rendered::Image { .. }
    ));

    let sesion_libro =
        ReadingSession::open(&escenario.db, &registro, &libro, escenario.tracker(), None)
            .expect("sesión");
    match sesion_libro.render().expect("pintado") {
        Rendered::Markup { html, .. } => {
            // El saneado ocurre en el runtime, así que lo que llega a la sesión
            // ya no puede ejecutar nada aunque el webview fallase.
            assert!(!html.contains("<script"));
        }
        Rendered::Image { .. } => panic!("un EPUB no es una imagen"),
    }
}

#[test]
fn pedir_texto_de_un_comic_dice_por_que_no_lo_hay() {
    let escenario = Escenario::nuevo("texto");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let sesion = ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
        .expect("sesión");

    // Y no una cadena vacía, que parecería un documento sin contenido. Es la
    // misma distinción que `capabilities().text_extraction` anticipa.
    assert_eq!(sesion.text(200).err(), Some(NbError::FormatUnsupported));
}

#[test]
fn la_sesion_se_puede_compartir_entre_hilos() {
    let escenario = Escenario::nuevo("hilos");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    // Un comando de Tauri corre en el pool de tareas: la sesión la tocan hilos
    // distintos, y si no fuese compartible habría que descubrirlo con
    // la interfaz ya escrita encima.
    let sesion = Arc::new(
        ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
            .expect("sesión"),
    );
    let vistas = Arc::new(Mutex::new(Vec::new()));

    let hilos: Vec<_> = (1..=4_u16)
        .map(|pagina| {
            let sesion = Arc::clone(&sesion);
            let vistas = Arc::clone(&vistas);
            std::thread::spawn(move || {
                let destino = sesion.navigation().entries[pagina as usize - 1]
                    .locator
                    .clone();
                sesion.go_to(&destino).expect("movimiento");
                let pintado = sesion.render().expect("pintado");
                vistas
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(pintado);
            })
        })
        .collect();

    for hilo in hilos {
        hilo.join().expect("hilo");
    }

    assert_eq!(vistas.lock().unwrap_or_else(|p| p.into_inner()).len(), 4);
}

// --- El almacén de sesiones -------------------------------------------------

#[test]
fn abrir_dos_veces_el_mismo_documento_devuelve_la_sesion_donde_estaba() {
    let escenario = Escenario::nuevo("reapertura");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();
    let almacen = SessionStore::new(escenario.tracker());

    let primera = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("primera");
    let destino = primera.navigation().entries[2].locator.clone();
    primera.go_to(&destino).expect("movimiento");

    let segunda = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("segunda");

    // Sin esto, el usuario tendría el mismo libro abierto dos veces en dos
    // posiciones distintas y ninguna de las dos sería "la" posición.
    assert_eq!(segunda.current().expect("actual"), destino);
    assert_eq!(almacen.len().expect("cuenta"), 1);
}

#[test]
fn el_almacen_no_retiene_documentos_indefinidamente() {
    let escenario = Escenario::nuevo("tope");
    let registro = escenario.registro();
    let almacen = SessionStore::new(escenario.tracker());

    // Nueve documentos distintos. Cada sesión retiene el contenedor abierto, así
    // que sin tope una tarde de biblioteca deja doscientos abiertos.
    let mut ids = Vec::new();
    for n in 0..9 {
        let nombre = format!("comic-{n}.cbz");
        let destino = escenario.scope.join(&nombre);
        // Un byte distinto en el comentario final: el contenido tiene que
        // diferir o la identidad los reconocería como el mismo documento.
        let mut bytes = std::fs::read(fixtures().join("minimo-valido.cbz")).expect("fixture");
        let ultimo = bytes.len() - 1;
        bytes[ultimo] = 0;
        bytes.push(n);
        std::fs::write(&destino, &bytes).expect("copia");

        let mut conn = escenario.db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &escenario.scope,
                library: &escenario.library,
                bus: &escenario.bus,
            },
            &ImportRequest {
                source: destino,
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importación");
        let ImportOutcome::Imported(datos) = resultado else {
            panic!("cada uno debe ser nuevo");
        };
        ids.push(datos.file.id.clone());
    }

    for id in &ids {
        almacen
            .open(&escenario.db, &registro, id)
            .expect("apertura");
    }

    assert_eq!(almacen.len().expect("cuenta"), 8, "el tope se respeta");
    assert!(
        almacen.get(&ids[0]).expect("consulta").is_none(),
        "se desaloja el menos usado, no el último en abrirse"
    );
    assert!(almacen.get(&ids[8]).expect("consulta").is_some());
}

#[test]
fn cerrar_lo_que_no_esta_abierto_no_es_un_error() {
    let escenario = Escenario::nuevo("cierre");
    let almacen = SessionStore::new(escenario.tracker());
    // La interfaz puede llamar a esto al desmontar un componente sin saber si
    // llegó a abrirse. Convertirlo en fallo solo produce manejo de errores que
    // no arregla nada.
    assert!(almacen.close("no-existe").is_ok());
    assert!(almacen.is_empty().expect("vacío"));
}

// --- Salto a una anotación -----------------------------------------

#[test]
fn saltar_a_un_marcador_mueve_la_lectura_y_el_progreso() {
    use nb_books_lib::annotations;

    let escenario = Escenario::nuevo("salto");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();
    let almacen = SessionStore::new(escenario.tracker());
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");

    // El usuario va a la página 3 y pone un marcador ahí.
    let pagina3 = sesion.navigation().entries[2].locator.clone();
    sesion.go_to(&pagina3).expect("movimiento");
    let conn = escenario.db.connection().expect("conexión");
    let marcador = annotations::bookmark(
        &conn,
        &sesion.info().edition_id,
        &sesion.current().expect("actual"),
    )
    .expect("marcador");
    drop(conn);

    // Se va a otro sitio y vuelve por el marcador.
    sesion
        .go_to(&sesion.navigation().entries[0].locator.clone())
        .expect("movimiento");
    let resolucion = sesion.go_to_annotation(&marcador.id).expect("salto");

    assert_eq!(resolucion.position, NativePosition::Page(3));
    assert_eq!(sesion.current().expect("actual"), pagina3);
}

#[test]
fn no_se_puede_saltar_a_una_anotacion_de_otro_documento() {
    use nb_books_lib::annotations;

    let escenario = Escenario::nuevo("salto-ajeno");
    let comic = escenario.importar("minimo-valido.cbz");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();
    let almacen = SessionStore::new(escenario.tracker());

    let sesion_libro = almacen
        .open(&escenario.db, &registro, &libro)
        .expect("libro");
    let conn = escenario.db.connection().expect("conexión");
    let ajena = annotations::bookmark(
        &conn,
        &sesion_libro.info().edition_id,
        &sesion_libro.current().expect("actual"),
    )
    .expect("marcador del libro");
    drop(conn);

    let sesion_comic = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("cómic");

    // Sin esta comprobación, saltar a una anotación de otro libro colocaría la
    // lectura en un punto que no significa nada en este documento, y el locator
    // resolvería por progresión sin que nada dijera que es de otro sitio.
    assert_eq!(
        sesion_comic.go_to_annotation(&ajena.id).err(),
        Some(NbError::DocumentNotFound)
    );
}

#[test]
fn no_se_puede_saltar_a_una_anotacion_dada_de_baja() {
    use nb_books_lib::annotations;

    let escenario = Escenario::nuevo("salto-baja");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();
    let almacen = SessionStore::new(escenario.tracker());
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");

    let conn = escenario.db.connection().expect("conexión");
    let marcador = annotations::bookmark(
        &conn,
        &sesion.info().edition_id,
        &sesion.current().expect("actual"),
    )
    .expect("marcador");
    annotations::delete(&conn, &marcador.id).expect("baja");
    drop(conn);

    assert_eq!(
        sesion.go_to_annotation(&marcador.id).err(),
        Some(NbError::DocumentNotFound)
    );
}

// --- Lo que la superficie de lectura pide a la sesión --------------

/// **Pintar no es moverse.**
///
/// La superficie pinta la página siguiente antes de que el usuario llegue a
/// ella. Si pedirla moviera la posición, adelantar una página adelantaría el
/// progreso de algo que nadie ha leído, y al volver el usuario aparecería más
/// allá de donde se quedó.
#[test]
fn pintar_otra_posicion_no_mueve_la_lectura() {
    let escenario = Escenario::nuevo("pintar-sin-mover");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();
    let sesion = ReadingSession::open(&escenario.db, &registro, &libro, escenario.tracker(), None)
        .expect("sesión");

    let antes = sesion.current().expect("posición");
    let mas_adelante = sesion
        .locator_for_quote("corpus")
        .expect("la cita está en el documento");
    assert_ne!(mas_adelante, antes, "la cita está en otro punto");

    sesion.render_at(&mas_adelante).expect("pintado");

    assert_eq!(
        sesion.current().expect("posición"),
        antes,
        "pintar otra página no puede cambiar dónde está leyendo el usuario"
    );
}

/// La cita se ancla con la posición actual como pista, sin que nadie la aporte.
///
/// Quien selecciona está leyendo aquí: obligar a la interfaz a mandar la pista
/// sería pedirle un dato que el runtime ya tiene, y dejarle la posibilidad de
/// mandar otro.
#[test]
fn anclar_una_cita_usa_la_posicion_actual_como_pista() {
    let escenario = Escenario::nuevo("ancla");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();
    let sesion = ReadingSession::open(&escenario.db, &registro, &libro, escenario.tracker(), None)
        .expect("sesión");

    let locator = sesion.locator_for_quote("Texto de prueba").expect("ancla");

    let contexto = locator.text.as_ref().expect("la cita viaja");
    assert_eq!(contexto.highlight, "Texto de prueba");

    // Y una cita que no está en el documento no produce anotación: sin esto, la
    // anotación nacería imposible de reanclar.
    assert_eq!(
        sesion.locator_for_quote("esto no está escrito aquí").err(),
        Some(NbError::DocumentNotFound)
    );
}

/// Un cómic no tiene texto, y anclar una cita sobre él se declara imposible.
#[test]
fn anclar_una_cita_sobre_un_comic_dice_que_el_formato_no_tiene_texto() {
    let escenario = Escenario::nuevo("ancla-comic");
    let comic = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();
    let sesion = ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
        .expect("sesión");

    assert_eq!(
        sesion.locator_for_quote("lo que sea").err(),
        Some(NbError::FormatUnsupported)
    );
    // Y tampoco tiene capa de texto que superponer, por el mismo motivo.
    assert_eq!(
        sesion
            .text_layer(&sesion.current().expect("posición"))
            .err(),
        Some(NbError::FormatUnsupported)
    );
}

/// Lo que se pinta como imagen sabe cuánto mide antes de que llegue la imagen.
#[test]
fn una_pagina_de_comic_declara_su_tamano_y_un_epub_declara_que_no_lo_tiene() {
    let escenario = Escenario::nuevo("tamano-sesion");
    let comic = escenario.importar("minimo-valido.cbz");
    let libro = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();

    let sesion_comic =
        ReadingSession::open(&escenario.db, &registro, &comic, escenario.tracker(), None)
            .expect("sesión");
    let medida = sesion_comic
        .page_size(&sesion_comic.current().expect("posición"))
        .expect("tamaño");
    assert!(medida.width > 0 && medida.height > 0);

    let sesion_libro =
        ReadingSession::open(&escenario.db, &registro, &libro, escenario.tracker(), None)
            .expect("sesión");
    assert_eq!(
        sesion_libro
            .page_size(&sesion_libro.current().expect("posición"))
            .err(),
        Some(NbError::FormatUnsupported),
        "un EPUB no tiene tamaño de página: lo tiene la ventana"
    );
}
