//! Criterio de aceptación de la.
//!
//! "Importar mil archivos, cancelar a la mitad y reanudar no duplica ni pierde
//! nada."
//!
//! Es el caso que separa una herramienta de juguete de una que alguien puede
//! usar con su biblioteca de verdad. Una importación que no se puede cancelar
//! obliga a esperar una hora o a matar la aplicación; una que no se puede
//! reanudar convierte cualquier interrupción en empezar de cero.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::library::{import_directory, BulkReport, CancelToken, ImportContext};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    db: Database,
    bus: EventBus,
    avances: Arc<Mutex<Vec<(u32, u32)>>>,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-masiva-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        std::fs::create_dir_all(&scope).expect("ámbito");
        std::fs::create_dir_all(&library).expect("biblioteca");

        let bus = EventBus::new();
        let avances = Arc::new(Mutex::new(Vec::new()));
        let destino = Arc::clone(&avances);
        bus.subscribe("avance", move |evento| {
            if let DomainEvent::BulkImportProgress {
                processed,
                imported,
                ..
            } = evento
            {
                destino
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push((*processed, *imported));
            }
        });

        // Base en archivo y no en memoria, a propósito: la de memoria comparte
        // caché y **no tiene WAL**, así que un lector se bloquea contra la
        // escritura de la importación. La aplicación real usa WAL justamente
        // para que eso no pase (ADR-002), y una prueba de concurrencia sobre una
        // base sin WAL mediría un problema que el producto no tiene.
        let db = Database::open(&dir.join("nb-books.sqlite")).expect("base");

        Self {
            dir,
            scope,
            library,
            db,
            bus,
            avances,
        }
    }

    /// Genera `cuantos` documentos válidos y distintos entre sí.
    ///
    /// Distintos de verdad: si compartieran contenido, la identidad por
    /// contenido los tomaría por duplicados y la prueba no mediría nada.
    fn poblar(&self, subcarpeta: &str, cuantos: usize) -> PathBuf {
        let carpeta = self.scope.join(subcarpeta);
        std::fs::create_dir_all(&carpeta).expect("carpeta");

        let base = std::fs::read(fixtures().join("minimo-valido.pdf")).expect("plantilla");
        for i in 0..cuantos {
            let mut contenido = base.clone();
            // El comentario va después del `%%EOF`, así que el archivo sigue
            // siendo un PDF completo y cada uno tiene identidad propia.
            contenido.extend_from_slice(format!("\n% ejemplar {i:06}\n%%EOF\n").as_bytes());
            std::fs::write(carpeta.join(format!("documento-{i:04}.pdf")), &contenido)
                .expect("documento");
        }
        carpeta
    }

    fn importar(&self, carpeta: &Path, cancel: &CancelToken) -> Result<BulkReport, NbError> {
        let mut conn = self.db.connection().expect("conexión");
        import_directory(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            carpeta,
            StorageMode::Referenced,
            cancel,
        )
    }

    fn filas(&self, tabla: &str) -> i64 {
        self.db
            .connection()
            .expect("conexión")
            .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
            .expect("consulta")
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// **El criterio de aceptación**, con mil archivos de verdad.
#[test]
fn importar_mil_cancelar_a_la_mitad_y_reanudar_no_duplica_ni_pierde_nada() {
    let esc = Escenario::nuevo("mil");
    let carpeta = esc.poblar("mil-documentos", 1000);

    // --- Primera pasada, cancelada a la mitad ---
    let cancel = CancelToken::new();
    let disparador = cancel.clone();

    // Se cancela desde otro hilo, como lo haría el usuario pulsando un botón
    // mientras la importación corre.
    let vigilante = {
        let db = esc.db.clone();
        std::thread::spawn(move || {
            // Un plazo y no un número de vueltas: cuántas caben antes de llegar a
            // quinientos depende de la velocidad del equipo, y con vueltas fijas
            // una máquina cargada se rendía antes de que la importación llegara.
            let plazo = std::time::Instant::now() + std::time::Duration::from_secs(120);
            while std::time::Instant::now() < plazo {
                let hechos: i64 = db
                    .connection()
                    .expect("conexión")
                    .query_row("SELECT count(*) FROM files", [], |r| r.get(0))
                    .expect("consulta");
                if hechos >= 500 {
                    disparador.cancel();
                    return hechos;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            disparador.cancel();
            -1
        })
    };

    let primera = esc.importar(&carpeta, &cancel).expect("primera pasada");
    let cuando_se_canceló = vigilante.join().expect("vigilante");

    assert!(primera.cancelled, "la importación debía quedar cancelada");
    assert!(
        cuando_se_canceló >= 500,
        "la cancelación se pidió antes de tiempo: {cuando_se_canceló}"
    );
    assert!(
        primera.imported < 1000,
        "cancelar a la mitad no puede importarlo todo: {}",
        primera.imported
    );
    assert!(primera.imported >= 500);

    // Lo importado hasta la cancelación está entero: una obra, una edición y un
    // archivo por documento, sin nada a medias.
    let tras_cancelar = esc.filas("files");
    assert_eq!(tras_cancelar, primera.imported as i64);
    assert_eq!(esc.filas("works"), tras_cancelar);
    assert_eq!(esc.filas("editions"), tras_cancelar);

    // --- Reanudación: exactamente lo mismo, otra vez ---
    let segunda = esc
        .importar(&carpeta, &CancelToken::new())
        .expect("reanudación");

    assert!(!segunda.cancelled);
    assert_eq!(segunda.processed, 1000, "la reanudación mira todos");
    assert_eq!(
        segunda.already_in_library, tras_cancelar as usize,
        "lo ya importado se reconoce, no se duplica"
    );
    assert_eq!(
        segunda.imported,
        1000 - tras_cancelar as usize,
        "la reanudación importa exactamente lo que faltaba"
    );
    assert!(segunda.rejected.is_empty());

    // --- Ni duplicado ni perdido ---
    assert_eq!(esc.filas("files"), 1000, "ni uno perdido");
    assert_eq!(esc.filas("works"), 1000, "ni uno duplicado");

    let identidades: i64 = esc
        .db
        .connection()
        .expect("conexión")
        .query_row("SELECT count(DISTINCT content_hash) FROM files", [], |r| {
            r.get(0)
        })
        .expect("consulta");
    assert_eq!(identidades, 1000, "hay contenidos repetidos");

    // --- Y una tercera pasada no cambia nada ---
    let tercera = esc
        .importar(&carpeta, &CancelToken::new())
        .expect("tercera");
    assert_eq!(tercera.imported, 0);
    assert_eq!(tercera.already_in_library, 1000);
    assert_eq!(esc.filas("files"), 1000);
}

/// Un archivo roto entre muchos no puede llevarse por delante a los demás, y el
/// usuario tiene que poder saber cuál falló y por qué.
#[test]
fn un_archivo_rechazado_no_detiene_la_importacion_y_queda_en_el_informe() {
    let esc = Escenario::nuevo("rechazados");
    let carpeta = esc.poblar("mezcla", 5);

    // Tres intrusos: uno truncado, uno de formato desconocido y uno vacío.
    std::fs::copy(fixtures().join("truncado.pdf"), carpeta.join("a-roto.pdf")).expect("copia");
    std::fs::copy(
        fixtures().join("adversario-prompt.txt"),
        carpeta.join("b-texto.txt"),
    )
    .expect("copia");
    std::fs::write(carpeta.join("c-vacio.epub"), b"").expect("vacío");

    let informe = esc
        .importar(&carpeta, &CancelToken::new())
        .expect("importación");

    assert_eq!(informe.processed, 8);
    assert_eq!(informe.imported, 5, "los válidos entran igual");
    // El .txt no es un documento: se deja pasar sin contarlo como rechazo.
    assert_eq!(informe.ignored, 1);
    assert_eq!(informe.rejected.len(), 2);

    let motivos: Vec<(&str, &str)> = informe
        .rejected
        .iter()
        .map(|r| (r.path.as_str(), r.code))
        .collect();
    assert!(motivos.contains(&("a-roto.pdf", "DOCUMENT_CORRUPTED")));
    assert!(motivos.contains(&("c-vacio.epub", "FORMAT_UNSUPPORTED")));

    // Las rutas del informe son relativas: el usuario sabe cuál falló sin que la
    // ruta revele dónde vive.
    assert!(
        informe.rejected.iter().all(|r| !r.path.starts_with('/')),
        "el informe no puede llevar rutas absolutas"
    );
}

/// El avance se publica por el bus, y con números, no con títulos.
#[test]
fn el_avance_se_publica_periodicamente_y_solo_con_numeros() {
    let esc = Escenario::nuevo("avance");
    let carpeta = esc.poblar("documentos", 60);

    esc.importar(&carpeta, &CancelToken::new())
        .expect("importación");

    // Se espera al bus, que despacha en su propio hilo.
    //
    // La condición de espera es **el último avance y no cuántos hay**: con
    // sesenta archivos se publican seis, y esperar solo a que lleguen tres deja
    // leer la lista a medio despachar. La prueba fallaba así una de cada dos
    // veces, afirmando sobre un final que todavía no había llegado, sin que el
    // producto tuviera nada que ver.
    let esperado = Some((60, 60));
    let mut avances = Vec::new();
    for _ in 0..200 {
        avances = esc
            .avances
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if avances.last().copied() == esperado {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    assert!(
        avances.len() >= 3,
        "con 60 archivos debía haber varios avances: {avances:?}"
    );
    assert_eq!(
        avances.last().copied(),
        esperado,
        "el último avance es el total"
    );
    // Y son monótonos: una barra que retrocede es peor que ninguna barra.
    assert!(avances.windows(2).all(|v| v[0].0 <= v[1].0));
}

/// Recorrer una carpeta no puede convertirse en recorrer el disco entero porque
/// alguien dejó un enlace simbólico dentro.
#[cfg(unix)]
#[test]
fn el_recorrido_no_sigue_enlaces_simbolicos() {
    let esc = Escenario::nuevo("enlaces");
    let carpeta = esc.poblar("documentos", 3);

    std::os::unix::fs::symlink(&esc.scope, carpeta.join("bucle")).expect("enlace a un ancestro");
    std::os::unix::fs::symlink("/etc", carpeta.join("fuera")).expect("enlace fuera");

    let informe = esc
        .importar(&carpeta, &CancelToken::new())
        .expect("importación");

    assert_eq!(informe.processed, 3, "solo los tres documentos reales");
}

/// Cancelar antes de empezar no importa nada y lo dice.
#[test]
fn cancelar_antes_de_empezar_no_importa_nada() {
    let esc = Escenario::nuevo("cancelada-ya");
    let carpeta = esc.poblar("documentos", 10);
    let cancel = CancelToken::new();
    cancel.cancel();

    let informe = esc.importar(&carpeta, &cancel).expect("importación");

    assert!(informe.cancelled);
    assert_eq!(informe.processed, 0);
    assert_eq!(esc.filas("files"), 0);
}

/// Una carpeta fuera del ámbito autorizado no se recorre, aunque exista.
#[test]
fn una_carpeta_fuera_del_ambito_se_rechaza() {
    let esc = Escenario::nuevo("fuera");
    let fuera = esc.dir.join("fuera");
    std::fs::create_dir_all(&fuera).expect("carpeta");

    assert_eq!(
        esc.importar(&fuera, &CancelToken::new()),
        Err(NbError::PathNotAllowed)
    );
}

/// **El avance final llega antes que el fin.**
///
/// Regresión de un defecto que se veía en la interfaz y no en ninguna prueba: al
/// revés, quien escucha recibía el fin —y limpiaba el indicador— y justo después
/// un avance que se lo volvía a poner. El panel se quedaba diciendo "importando"
/// sobre una importación terminada, y el usuario no sabía si podía importar otra
/// cosa.
#[test]
fn el_avance_final_llega_antes_que_el_fin_de_la_importacion() {
    let esc = Escenario::nuevo("orden");
    let carpeta = esc.poblar("documentos", 12);

    // Se apunta el orden de los dos eventos, no solo su contenido.
    let orden = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
    let destino = std::sync::Arc::clone(&orden);
    esc.bus.subscribe("orden", move |evento| {
        let nombre = match evento {
            DomainEvent::BulkImportProgress { .. } => "avance",
            DomainEvent::BulkImportFinished { .. } => "fin",
            _ => return,
        };
        destino
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(nombre);
    });

    esc.importar(&carpeta, &CancelToken::new())
        .expect("importación");

    let mut visto = Vec::new();
    for _ in 0..200 {
        visto = orden.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if visto.last() == Some(&"fin") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    assert_eq!(
        visto.last(),
        Some(&"fin"),
        "el fin tiene que ser lo último que se oye: {visto:?}"
    );
    assert!(
        visto.contains(&"avance"),
        "y tiene que haber habido avances antes: {visto:?}"
    );
}

// ---------------------------------------------------------------------------
// lo que sobrevive a cerrar la aplicación.
//
// Reanudar ya era gratis por la identidad de contenido, pero **solo si el
// usuario se acordaba** de volver a lanzarla sobre la misma carpeta; y el
// informe de lo que quedó fuera vivía en el panel de esa ejecución y en ningún
// sitio más. Las dos son la pregunta J12.
// ---------------------------------------------------------------------------

/// Terminar cierra la ejecución, así que no se ofrece continuar lo que ya acabó.
///
/// Es la mitad que evita el aviso molesto: una aplicación que en cada arranque
/// pregunta si continuar algo que terminó enseña a ignorar sus preguntas.
#[test]
fn una_importacion_terminada_no_se_ofrece_continuar() {
    let escenario = Escenario::nuevo("terminada");
    let carpeta = escenario.poblar("lote", 3);

    let informe = escenario
        .importar(&carpeta, &CancelToken::new())
        .expect("importación");
    assert_eq!(informe.imported, 3);

    let conn = escenario.db.connection().expect("conexión");
    assert!(
        nb_books_lib::imports::pendiente(&conn)
            .expect("pendiente")
            .is_none(),
        "una importación que terminó sigue apareciendo como pendiente"
    );
}

/// Una que murió a medias queda abierta, que es lo que permite ofrecerla.
///
/// Se simula abriendo la fila y no cerrándola, que es exactamente lo que ocurre
/// cuando el proceso se cae: nadie escribe nada al morir. Por eso la señal es una
/// fila sin cerrar y no una bandera, que habría que apagar.
#[test]
fn una_importacion_que_murio_a_medias_se_puede_ofrecer_al_volver() {
    let escenario = Escenario::nuevo("muerta");
    let conn = escenario.db.connection().expect("conexión");

    nb_books_lib::imports::abrir(&conn, "/una/carpeta", StorageMode::Referenced).expect("abrir");

    let pendiente = nb_books_lib::imports::pendiente(&conn)
        .expect("pendiente")
        .expect("tenía que haber una a medias");
    assert_eq!(pendiente.root, "/una/carpeta");
    assert_eq!(pendiente.storage_mode, "referenced");
    assert!(pendiente.finished_at.is_none());
}

/// Lo que se rechazó sigue estando cuando el panel ya no está.
#[test]
fn lo_que_la_importacion_rechazo_sigue_estando_despues() {
    let escenario = Escenario::nuevo("rechazos");
    let carpeta = escenario.poblar("lote", 2);
    // Un archivo que no es ningún formato que sepamos abrir: se rechaza, y es
    // justo lo que el usuario necesita poder mirar después.
    std::fs::write(carpeta.join("esto-no-es-un-libro.epub"), b"nada de nada").expect("archivo");

    let informe = escenario
        .importar(&carpeta, &CancelToken::new())
        .expect("importación");
    assert_eq!(informe.imported, 2);
    assert_eq!(informe.rejected.len(), 1);

    let conn = escenario.db.connection().expect("conexión");
    let ultima = nb_books_lib::imports::ultima(&conn)
        .expect("última")
        .expect("tenía que haber una");
    assert_eq!(ultima.rejected, 1);

    let guardados = nb_books_lib::imports::rechazos(&conn, &ultima.id, 100, 0).expect("rechazos");
    assert_eq!(guardados.len(), 1);
    assert!(
        guardados[0].path.contains("esto-no-es-un-libro.epub"),
        "el rechazo guardado no dice qué archivo fue: {}",
        guardados[0].path
    );
    assert!(
        !guardados[0].code.is_empty(),
        "un rechazo sin código no se puede traducir al pintarlo"
    );
}

/// El recuento es el total aunque la lista esté acotada.
///
/// Sin esto, una biblioteca con veinte mil rechazos escribiría veinte mil filas y
/// el informe sería el problema. Con el tope, lo que no se puede es que la cifra
/// mienta: se lee de la ejecución y no de contar las filas guardadas.
#[test]
fn el_recuento_de_rechazados_no_miente_aunque_la_lista_se_acote() {
    let escenario = Escenario::nuevo("tope");
    let conn = escenario.db.connection().expect("conexión");
    let id =
        nb_books_lib::imports::abrir(&conn, "/carpeta", StorageMode::Referenced).expect("abrir");

    let intentados = nb_books_lib::imports::RECHAZOS_POR_EJECUCION + 50;
    for i in 0..intentados {
        nb_books_lib::imports::anotar_rechazo(
            &conn,
            &id,
            &format!("archivo-{i}"),
            "FORMAT_UNSUPPORTED",
        )
        .expect("rechazo");
    }
    nb_books_lib::imports::cerrar(
        &conn,
        &id,
        nb_books_lib::imports::Resumen {
            procesados: u32::try_from(intentados).unwrap_or(u32::MAX),
            rechazados: u32::try_from(intentados).unwrap_or(u32::MAX),
            ..nb_books_lib::imports::Resumen::default()
        },
    )
    .expect("cerrar");

    // El tramo se pide más ancho que el tope de almacenamiento a propósito: lo
    // que esta prueba mide es cuántos rechazos se guardan, no cuántos cabe pedir.
    let guardados = nb_books_lib::imports::rechazos(
        &conn,
        &id,
        u32::try_from(nb_books_lib::imports::RECHAZOS_POR_EJECUCION + 50).unwrap_or(u32::MAX),
        0,
    )
    .expect("rechazos");
    assert_eq!(
        guardados.len(),
        nb_books_lib::imports::RECHAZOS_POR_EJECUCION,
        "la lista de rechazos creció por encima de su tope"
    );

    let ejecucion = nb_books_lib::imports::ultima(&conn)
        .expect("última")
        .expect("tenía que haber una");
    assert_eq!(
        ejecucion.rejected as usize, intentados,
        "el recuento se quedó en el tope en lugar de decir la verdad"
    );
}

/// Descartar lo que ya está cerrado no es un error.
///
/// Es el mismo criterio que `cancel_import`: la interfaz puede pulsarlo justo
/// cuando otra cosa lo cerró, y convertir eso en un fallo solo produce manejo de
/// errores que no arregla nada.
#[test]
fn descartar_una_importacion_ya_cerrada_no_es_un_error() {
    let escenario = Escenario::nuevo("descartar");
    let conn = escenario.db.connection().expect("conexión");
    let id =
        nb_books_lib::imports::abrir(&conn, "/carpeta", StorageMode::Referenced).expect("abrir");

    nb_books_lib::imports::descartar(&conn, &id).expect("primer descarte");
    nb_books_lib::imports::descartar(&conn, &id).expect("descartar dos veces no puede fallar");

    assert!(
        nb_books_lib::imports::pendiente(&conn)
            .expect("pendiente")
            .is_none(),
        "descartar no quitó la importación de las pendientes"
    );
}

/// El historial no crece sin límite.
#[test]
fn el_historial_de_importaciones_no_crece_sin_limite() {
    let escenario = Escenario::nuevo("historial");
    let conn = escenario.db.connection().expect("conexión");

    let cuantas = nb_books_lib::imports::EJECUCIONES_GUARDADAS + 5;
    for i in 0..cuantas {
        let id =
            nb_books_lib::imports::abrir(&conn, &format!("/carpeta-{i}"), StorageMode::Referenced)
                .expect("abrir");
        nb_books_lib::imports::cerrar(&conn, &id, nb_books_lib::imports::Resumen::default())
            .expect("cerrar");
    }

    let total: i64 = conn
        .query_row("SELECT count(*) FROM import_runs", [], |fila| fila.get(0))
        .expect("recuento");
    assert_eq!(
        total as usize,
        nb_books_lib::imports::EJECUCIONES_GUARDADAS,
        "el historial de importaciones creció sin tope"
    );
}

/// **La otra mitad del criterio, y el.**
///
/// Una importación con otro escritor encima no puede dejar ni un
/// `STORAGE_FAILURE`. Es lo que ocurre de verdad: la indexación y la extracción
/// de metadatos escriben mientras la importación recorre la carpeta, y en la
/// instalación real eso dejó fuera **1.207 documentos** que nadie volvió a
/// intentar.
///
/// Dos cosas la sostienen y las dos hacen falta: que ninguna transacción de
/// escritura se abra en diferido y que un fallo de almacenamiento se
/// reintente antes de darlo por rechazado. Sin la segunda, basta con
/// que a un documento le toque esperar más de la cuenta para perderlo.
#[test]
fn importar_con_otro_escritor_encima_no_deja_ningun_rechazo_de_almacenamiento() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let escenario = Escenario::nuevo("contencion");
    let carpeta = escenario.poblar("contencion", 60);

    let parar = Arc::new(AtomicBool::new(false));
    let db_del_otro = escenario.db.clone();
    let señal = Arc::clone(&parar);
    let otro = std::thread::spawn(move || {
        let conn = db_del_otro
            .connection()
            .expect("conexión del otro escritor");
        let mut escritas = 0_usize;
        while !señal.load(Ordering::Acquire) {
            conn.execute(
                "INSERT OR REPLACE INTO app_metadata (key, value) VALUES ('contencion', ?1)",
                [escritas.to_string()],
            )
            .expect("el otro escritor espera su turno, no falla");
            escritas += 1;
        }
        escritas
    });

    let informe = escenario
        .importar(&carpeta, &CancelToken::default())
        .expect("la importación termina");

    parar.store(true, Ordering::Release);
    let escritas = otro.join().expect("el otro escritor termina");

    let de_almacenamiento: Vec<&str> = informe
        .rejected
        .iter()
        .filter(|r| r.code == "STORAGE_FAILURE")
        .map(|r| r.code)
        .collect();

    assert!(
        de_almacenamiento.is_empty(),
        "{} documentos se perdieron por almacenamiento con otro escritor encima",
        de_almacenamiento.len()
    );
    assert_eq!(informe.imported, 60, "no entraron todos");
    assert!(
        escritas > 0,
        "el otro escritor no llegó a escribir: no hubo contención que medir"
    );
}
