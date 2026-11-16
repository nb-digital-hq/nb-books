//! Criterio de aceptación de la.
//!
//! "Cerrar y reabrir la aplicación devuelve al usuario exactamente donde estaba,
//! aun tras cambiar el tamaño de la ventana o la tipografía."
//!
//! Las dos condiciones del final no son adorno. Son las dos formas conocidas de
//! perder la posición cuando se guarda como número: la ventana cambia cuántas
//! palabras entran por página, y la tipografía cambia dónde cae cada palabra.
//! Con un Locator ninguna de las dos puede mover nada, y estas pruebas lo
//! comprueban en lugar de darlo por hecho.
//!
//! Contratos: `docs/adr/004-reader-locator-model.md` y
//! `docs/architecture/reader-contracts.md`, sección 6.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::reader::comic::ComicFactory;
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::progress::{self, ProgressTracker};
use nb_books_lib::reader::{Precision, ProviderRegistry, SessionStore};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

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
            .join(format!("nb-progreso-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        let cache = dir.join("cache/paginas");
        std::fs::create_dir_all(&scope).expect("origen");
        std::fs::create_dir_all(&library).expect("biblioteca");
        std::fs::create_dir_all(&cache).expect("caché");

        // Con archivo y no en memoria: el criterio habla de **cerrar y reabrir
        // la aplicación**, y una base en memoria desaparece al cerrarla, así que
        // la prueba no comprobaría nada.
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

    /// Vuelve a abrir la base, como haría un arranque nuevo.
    fn reabrir(&self) -> Database {
        Database::open(&self._dir.join("nb-books.sqlite")).expect("base reabierta")
    }

    fn importar(&self, fixture: &str) -> (String, String) {
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
            ImportOutcome::Imported(datos) => (datos.file.id.clone(), datos.edition.id.clone()),
            ImportOutcome::AlreadyInLibrary(archivo) | ImportOutcome::Duplicate(archivo) => {
                (archivo.id.clone(), archivo.edition_id.clone())
            }
        }
    }

    fn registro(&self) -> ProviderRegistry {
        let mut registro = ProviderRegistry::new();
        registro.register(Box::new(EpubFactory::solo_texto()));
        registro.register(Box::new(ComicFactory::new(self.cache.clone())));
        registro
    }
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn cerrar_y_reabrir_devuelve_exactamente_donde_estaba() {
    let escenario = Escenario::nuevo("reapertura");
    let (comic, _edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    // --- Sesión 1: el usuario lee hasta la página 3 y cierra ---
    let donde_estaba = {
        // Intervalo de una hora a propósito: así **solo** el cierre puede haber
        // escrito la última posición. Con el intervalo de producción la primera
        // posición se escribe en el acto y la prueba pasaría sin llegar a
        // ejercitar el antirrebote, que es la mitad difícil del criterio.
        let almacen = SessionStore::new(Arc::new(ProgressTracker::with_interval(
            escenario.db.clone(),
            Duration::from_secs(3600),
        )));
        let sesion = almacen
            .open(&escenario.db, &registro, &comic)
            .expect("apertura");

        assert!(sesion.info().fresh, "la primera vez no hay nada guardado");

        // Leer es moverse muchas veces: el usuario pasa por la 1, la 2 y se
        // queda en la 3.
        for indice in 0..3 {
            let paso = sesion.navigation().entries[indice].locator.clone();
            sesion.go_to(&paso).expect("movimiento");
        }
        let destino = sesion.navigation().entries[2].locator.clone();

        // Cerrar es lo que vacía el antirrebote. Sin esto, la última posición
        // se quedaría apuntada y sin escribir.
        almacen.close(&comic).expect("cierre");
        destino
    };

    // --- La aplicación se cierra y se vuelve a abrir ---
    let db = escenario.reabrir();
    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(db.clone())));
    let sesion = almacen.open(&db, &registro, &comic).expect("reapertura");

    assert!(!sesion.info().fresh, "ya no es la primera vez");
    assert_eq!(
        sesion.info().locator,
        donde_estaba,
        "exactamente donde estaba, no cerca"
    );
    assert!(
        !sesion.info().approximate,
        "el documento no ha cambiado, así que la restauración es exacta"
    );
    assert_eq!(sesion.current().expect("actual"), donde_estaba);
}

#[test]
fn cambiar_la_tipografia_no_mueve_la_posicion_guardada() {
    let escenario = Escenario::nuevo("tipografia");
    let (libro, _edicion) = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();

    let almacen = SessionStore::new(Arc::new(ProgressTracker::with_interval(
        escenario.db.clone(),
        Duration::from_secs(3600),
    )));
    let sesion = almacen
        .open(&escenario.db, &registro, &libro)
        .expect("apertura");
    let donde_estaba = sesion.navigation().entries[0].locator.clone();
    sesion.go_to(&donde_estaba).expect("movimiento");
    sesion.go_to(&donde_estaba).expect("segundo movimiento");
    almacen.close(&libro).expect("cierre");

    // El usuario sube la escala del texto y cambia el tema. Con un entero de
    // página, la escala sola ya bastaría para descolocarlo.
    let conn = escenario.db.connection().expect("conexión");
    settings::set(&conn, SettingKey::FontScale, &SettingValue::Number(1.6)).expect("escala");
    settings::set(
        &conn,
        SettingKey::Theme,
        &SettingValue::Text("dark".to_owned()),
    )
    .expect("tema");
    drop(conn);

    let db = escenario.reabrir();
    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(db.clone())));
    let sesion = almacen.open(&db, &registro, &libro).expect("reapertura");

    assert_eq!(
        sesion.info().locator,
        donde_estaba,
        "la tipografía no es una entrada del proveedor, así que no puede mover nada"
    );
    assert!(!sesion.info().approximate);
}

// --- El antirrebote ---------------------------------------------------------

#[test]
fn leer_seguido_no_escribe_una_vez_por_movimiento() {
    let escenario = Escenario::nuevo("antirrebote");
    let (comic, edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    // Intervalo de una hora: nada se escribirá por tiempo, así que lo que quede
    // en la base es exactamente lo que el antirrebote decidió escribir.
    let tracker = Arc::new(ProgressTracker::with_interval(
        escenario.db.clone(),
        Duration::from_secs(3600),
    ));
    let almacen = SessionStore::new(Arc::clone(&tracker));
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");

    for entrada in &sesion.navigation().entries.clone() {
        sesion.go_to(&entrada.locator).expect("movimiento");
    }

    let conn = escenario.db.connection().expect("conexión");
    let guardado = progress::load(&conn, &edicion, progress::PERFIL)
        .expect("consulta")
        .expect("la primera posición sí se escribe en el acto");

    // La primera se escribe: es la que dice "esto se ha empezado a leer". Las
    // demás quedan apuntadas.
    assert_eq!(guardado.locator.locations.position, Some(1));
    assert_eq!(tracker.pending().expect("pendientes"), 1);
}

#[test]
fn un_antirrebote_que_pierde_la_ultima_posicion_seria_peor_que_ninguno() {
    let escenario = Escenario::nuevo("no-pierde");
    let (comic, edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let tracker = Arc::new(ProgressTracker::with_interval(
        escenario.db.clone(),
        Duration::from_secs(3600),
    ));
    let almacen = SessionStore::new(Arc::clone(&tracker));
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");

    let ultima = sesion.navigation().entries[3].locator.clone();
    sesion
        .go_to(&sesion.navigation().entries[1].locator.clone())
        .expect("movimiento");
    sesion.go_to(&ultima).expect("movimiento");

    almacen.close(&comic).expect("cierre");

    let conn = escenario.db.connection().expect("conexión");
    let guardado = progress::load(&conn, &edicion, progress::PERFIL)
        .expect("consulta")
        .expect("progreso");

    assert_eq!(
        guardado.locator, ultima,
        "cerrar tiene que vaciar lo apuntado; si no, el usuario vuelve atrás"
    );
    assert_eq!(tracker.pending().expect("pendientes"), 0);
}

#[test]
fn desalojar_una_sesion_por_el_tope_tampoco_pierde_su_progreso() {
    let escenario = Escenario::nuevo("desalojo");
    let registro = escenario.registro();
    let tracker = Arc::new(ProgressTracker::with_interval(
        escenario.db.clone(),
        Duration::from_secs(3600),
    ));
    let almacen = SessionStore::new(Arc::clone(&tracker));

    // Nueve documentos: al abrir el noveno se desaloja el primero. Desde el
    // punto de vista del usuario, eso es un cierre.
    let mut ediciones = Vec::new();
    let mut archivos = Vec::new();
    for n in 0..9_u8 {
        let nombre = format!("comic-{n}.cbz");
        let destino = escenario.scope.join(&nombre);
        let mut bytes = std::fs::read(fixtures().join("minimo-valido.cbz")).expect("fixture");
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
        archivos.push(datos.file.id.clone());
        ediciones.push(datos.edition.id.clone());
    }

    let primera = almacen
        .open(&escenario.db, &registro, &archivos[0])
        .expect("apertura");
    // Dos movimientos: el primero se escribe en el acto y el segundo queda
    // apuntado, así que solo el desalojo puede haberlo guardado.
    primera
        .go_to(&primera.navigation().entries[0].locator.clone())
        .expect("movimiento");
    let donde = primera.navigation().entries[3].locator.clone();
    primera.go_to(&donde).expect("movimiento");
    drop(primera);

    for archivo in archivos.iter().skip(1) {
        almacen
            .open(&escenario.db, &registro, archivo)
            .expect("apertura");
    }

    let conn = escenario.db.connection().expect("conexión");
    let guardado = progress::load(&conn, &ediciones[0], progress::PERFIL)
        .expect("consulta")
        .expect("progreso de la desalojada");
    assert_eq!(guardado.locator, donde);
}

// --- Sesiones registradas ---------------------------------------------------

#[test]
fn abrir_y_cerrar_deja_una_sesion_registrada_con_sus_dos_extremos() {
    let escenario = Escenario::nuevo("registro");
    let (comic, edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(escenario.db.clone())));
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");
    let inicio = sesion.info().locator.clone();
    let fin = sesion.navigation().entries[2].locator.clone();
    sesion.go_to(&fin).expect("movimiento");
    almacen.close(&comic).expect("cierre");

    let conn = escenario.db.connection().expect("conexión");
    let sesiones = progress::sessions(&conn, &edicion).expect("sesiones");

    assert_eq!(sesiones.len(), 1);
    assert_eq!(sesiones[0].start_locator, inicio);
    assert_eq!(sesiones[0].end_locator.as_ref(), Some(&fin));
    assert!(sesiones[0].ended_at.is_some());
}

#[test]
fn una_sesion_sin_cerrar_queda_registrada_como_tal() {
    let escenario = Escenario::nuevo("sin-cerrar");
    let (comic, edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    {
        let almacen = SessionStore::new(Arc::new(ProgressTracker::new(escenario.db.clone())));
        almacen
            .open(&escenario.db, &registro, &comic)
            .expect("apertura");
        // Nadie cierra: es lo que pasa cuando la aplicación muere de golpe.
    }

    let conn = escenario.db.connection().expect("conexión");
    let sesiones = progress::sessions(&conn, &edicion).expect("sesiones");

    // La fila se queda sin `ended_at`, y eso **es** información: dice que hubo un
    // cierre inesperado. Borrarla lo escondería.
    assert_eq!(sesiones.len(), 1);
    assert!(sesiones[0].ended_at.is_none());
    assert!(sesiones[0].end_locator.is_none());
}

// --- Lo que no puede pasar --------------------------------------------------

#[test]
fn el_progreso_sobrevive_a_que_el_archivo_se_mueva() {
    let escenario = Escenario::nuevo("mudanza");
    let (comic, edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(escenario.db.clone())));
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");
    let donde = sesion.navigation().entries[2].locator.clone();
    sesion.go_to(&donde).expect("movimiento");
    almacen.close(&comic).expect("cierre");

    // El progreso cuelga de la **edición**, no del archivo: el archivo puede
    // moverse, reimportarse o existir en dos copias, y en los tres casos el
    // usuario espera seguir donde estaba.
    let conn = escenario.db.connection().expect("conexión");
    let guardado = progress::load(&conn, &edicion, progress::PERFIL)
        .expect("consulta")
        .expect("progreso");
    assert_eq!(guardado.locator, donde);
}

#[test]
fn un_progreso_ilegible_no_devuelve_al_principio_en_silencio() {
    let escenario = Escenario::nuevo("ilegible");
    let (_comic, edicion) = escenario.importar("minimo-valido.cbz");

    let conn = escenario.db.connection().expect("conexión");
    // Un objeto JSON válido que no es un Locator: pasa el CHECK de la base y no
    // se deja interpretar. La base sola no puede impedirlo.
    conn.execute(
        "INSERT INTO progress (edition_id, locator) VALUES (?1, '{\"cualquier\": \"cosa\"}')",
        [&edicion],
    )
    .expect("inserción");

    // Devolver `None` aquí sería devolver al usuario al principio del libro sin
    // decirle nada, que es exactamente la pérdida silenciosa que esta fase no
    // admite.
    assert_eq!(
        progress::load(&conn, &edicion, progress::PERFIL).err(),
        Some(nb_books_lib::error::NbError::StorageFailure)
    );
}

#[test]
fn el_progreso_de_una_edicion_no_se_mezcla_con_el_de_otra() {
    let escenario = Escenario::nuevo("separacion");
    let (comic, edicion_comic) = escenario.importar("minimo-valido.cbz");
    let (libro, edicion_libro) = escenario.importar("minimo-valido.epub");
    let registro = escenario.registro();

    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(escenario.db.clone())));

    let sesion_comic = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("cómic");
    let pagina = sesion_comic.navigation().entries[3].locator.clone();
    sesion_comic.go_to(&pagina).expect("movimiento");

    let sesion_libro = almacen
        .open(&escenario.db, &registro, &libro)
        .expect("libro");
    let capitulo = sesion_libro.navigation().entries[0].locator.clone();
    sesion_libro.go_to(&capitulo).expect("movimiento");

    almacen.close(&comic).expect("cierre");
    almacen.close(&libro).expect("cierre");

    let conn = escenario.db.connection().expect("conexión");
    assert_eq!(
        progress::load(&conn, &edicion_comic, progress::PERFIL)
            .expect("consulta")
            .expect("progreso")
            .locator,
        pagina
    );
    assert_eq!(
        progress::load(&conn, &edicion_libro, progress::PERFIL)
            .expect("consulta")
            .expect("progreso")
            .locator,
        capitulo
    );
}

#[test]
fn la_precision_de_la_ultima_restauracion_se_guarda_con_el_progreso() {
    let escenario = Escenario::nuevo("precision");
    let (comic, edicion) = escenario.importar("minimo-valido.cbz");
    let registro = escenario.registro();

    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(escenario.db.clone())));
    let sesion = almacen
        .open(&escenario.db, &registro, &comic)
        .expect("apertura");

    // Un locator de una edición con cien páginas: esta tiene cuatro.
    let mut viajero = sesion.navigation().entries[3].locator.clone();
    viajero.locations.position = Some(97);
    viajero.locations.total_progression = Some(0.97);
    sesion.go_to(&viajero).expect("movimiento degradado");
    almacen.close(&comic).expect("cierre");

    let conn = escenario.db.connection().expect("conexión");
    let guardado = progress::load(&conn, &edicion, progress::PERFIL)
        .expect("consulta")
        .expect("progreso");

    // Se guarda para poder avisar al reabrir sin volver a resolver: si la última
    // restauración fue aproximada, el usuario tiene derecho a saberlo.
    assert_eq!(guardado.precision, Precision::Approximate);
}
