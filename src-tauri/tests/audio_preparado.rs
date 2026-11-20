//! Preparar el audio de un capítulo y de un libro.
//!
//! # Por qué esto se puede probar sin el modelo
//!
//! Porque lo que decide si la función sirve **no es cómo suena**: es que las
//! marcas caigan donde toca, que cancelar deje lo hecho utilizable, que preparar
//! dos veces sustituya en lugar de acumular y que el tiempo restante salga de lo
//! medido. Todo eso es lógica, y el modelo pesa 327 MB.
//!
//! El sintetizador de aquí produce silencio de una duración proporcional al texto,
//! que es exactamente lo que la preparación necesita de él. Cómo suena la voz de
//! verdad se mide en `voz_neuronal.rs`, con el modelo real delante.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use nb_books_lib::ai::generacion::Documentos;
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::reader::DocumentProvider;
use nb_books_lib::voice::neuronal::{Sintetizador, Sonido};
use nb_books_lib::voice::preparacion::{Alcance, Preparador};

/// Un sintetizador que no carga nada: devuelve silencio de la duración que
/// tendría ese texto dicho a la velocidad medida del modelo.
struct VozDeMentira {
    dichos: AtomicU32,
    /// Si falla, y a partir de qué fragmento: para comprobar qué pasa entonces.
    falla_en: Option<u32>,
    /// Mientras esté puesto, cada fragmento tarda. Sirve para mirar la cola con
    /// algo dentro: sin esto el hilo la vacía antes de que la prueba pregunte, y
    /// lo que se quiere comprobar es justo lo que pasa con trabajo esperando.
    retenida: Arc<std::sync::atomic::AtomicBool>,
}

impl VozDeMentira {
    fn nueva() -> Self {
        Self {
            dichos: AtomicU32::new(0),
            falla_en: None,
            retenida: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// La misma voz, pero lenta mientras alguien tenga el freno puesto.
    fn retenida() -> (Self, Arc<std::sync::atomic::AtomicBool>) {
        let freno = Arc::new(std::sync::atomic::AtomicBool::new(true));
        (
            Self {
                dichos: AtomicU32::new(0),
                falla_en: None,
                retenida: Arc::clone(&freno),
            },
            freno,
        )
    }
}

impl Sintetizador for VozDeMentira {
    fn instalada(&self) -> bool {
        true
    }
    fn revision(&self) -> &'static str {
        "revision-de-prueba"
    }
    fn voz_por_defecto(&self) -> String {
        "ef_dora".to_owned()
    }
    fn factor_medido(&self) -> Option<f32> {
        Some(1.67)
    }
    fn sintetizar(&self, texto: &str, _voz: Option<&str>, _velocidad: f32) -> NbResult<Sonido> {
        while self.retenida.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let dichos = self.dichos.fetch_add(1, Ordering::SeqCst);
        if self.falla_en.is_some_and(|cuando| dichos >= cuando) {
            return Err(NbError::Internal);
        }
        // Diecisiete caracteres por segundo, que es lo medido con la voz real.
        let muestras = texto.chars().count() * 24_000 / 17;
        Ok(Sonido {
            muestras: vec![0.0; muestras],
            muestreo: 24_000,
        })
    }
    fn sintetizar_a_archivo(
        &self,
        carpeta: &Path,
        texto: &str,
        voz: Option<&str>,
        velocidad: f32,
    ) -> NbResult<PathBuf> {
        let sonido = self.sintetizar(texto, voz, velocidad)?;
        let destino = carpeta.join("muestra.opus");
        nb_books_lib::voice::audio::escribir_opus(&destino, &sonido.muestras, sonido.muestreo)?;
        Ok(destino)
    }
}

/// Un documento con dos capítulos, cada uno con sus fragmentos.
struct LibroDePrueba;

impl Documentos for LibroDePrueba {
    fn abrir(&self, _edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        Ok(Box::new(nb_books_lib::reader::fake::con_tres_capitulos()))
    }
}

/// Una base con una edición dentro: las claves ajenas de V35 la exigen, y es lo
/// que hace que borrar el libro se lleve su audio.
///
/// **En archivo y no en memoria**, y no es un detalle: una base en memoria es
/// privada de su conexión, y aquí escriben dos —la prueba y el hilo que prepara—.
/// Con una en memoria cada una vería la suya y lo preparado no aparecería nunca,
/// que es exactamente lo que pasó al escribir esto.
fn base(donde: &Path) -> (Database, String) {
    let db = Database::open(&donde.join("nb-books.sqlite")).expect("base en archivo");
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Cien años de soledad".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Cien años de soledad".to_owned(),
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
    (db, edicion)
}

fn carpeta(nombre: &str) -> PathBuf {
    let ruta = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-audio-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ruta);
    std::fs::create_dir_all(&ruta).expect("carpeta");
    ruta
}

struct Banco {
    preparador: Arc<Preparador>,
    db: Database,
    destino: PathBuf,
    edicion: String,
}

/// Las rutas de una instalación de mentira, todas dentro de la carpeta temporal.
///
/// La preparación las necesita enteras y no solo la del audio porque
/// mide lo derivado contra lo que ocupan los documentos, y esa medida mira la
/// base, la caché y el audio.
fn rutas(destino: &Path) -> nb_books_lib::paths::AppPaths {
    nb_books_lib::paths::AppPaths {
        data: destino.to_path_buf(),
        config: destino.join("config"),
        cache: destino.join("cache"),
        logs: destino.join("logs"),
    }
}

fn banco(nombre: &str, voz: Arc<dyn Sintetizador>) -> Banco {
    let destino = carpeta(nombre);
    let (db, edicion) = base(&destino);
    let preparador = Preparador::nuevo(
        db.clone(),
        Arc::new(LibroDePrueba),
        voz,
        &rutas(&destino),
        EventBus::new(),
    );
    Banco {
        preparador,
        db,
        destino,
        edicion,
    }
}

fn capitulo(edicion: &str, href: &str) -> Alcance {
    Alcance::Chapter {
        edition_id: edicion.to_owned(),
        hrefs: vec![href.to_owned()],
        from_position: None,
        to_position: None,
        title: "Primer capítulo".to_owned(),
    }
}

fn libro(edicion: &str) -> Alcance {
    Alcance::Book {
        edition_id: edicion.to_owned(),
        title: "Cien años de soledad".to_owned(),
    }
}

/// Espera a que la preparación termine. Sin dormir a ciegas: se pregunta.
fn esperar(preparador: &Preparador) {
    for _ in 0..600 {
        if preparador.en_curso().is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("la preparación no terminó");
}

/// **El criterio**: un capítulo preparado se puede escuchar entero, y
/// cada fragmento dice en qué milisegundo se dice.
#[test]
fn un_capitulo_preparado_trae_su_audio_y_sus_marcas_en_orden() {
    let banco = banco("capitulo", Arc::new(VozDeMentira::nueva()));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    preparador
        .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("empieza");
    esperar(preparador);

    let preparado = preparador
        .preparado(&capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("consulta")
        .expect("está preparado");

    assert_eq!(preparado.status, "ready");
    assert!(preparado.duration_ms > 0, "no duró nada");
    assert!(preparado.bytes > 0, "no ocupa nada");
    assert!(
        Path::new(&preparado.path).is_file(),
        "el archivo no está en disco"
    );
    assert!(!preparado.marks.is_empty(), "no dejó marcas");

    // Las marcas van en orden y sin huecos: cada una empieza donde acabó la
    // anterior, que es lo que permite saltar a un párrafo por su tiempo.
    let mut anterior = 0;
    for marca in &preparado.marks {
        assert_eq!(marca.start_ms, anterior, "hay un hueco entre marcas");
        assert!(marca.end_ms > marca.start_ms, "una marca no dura nada");
        anterior = marca.end_ms;
    }
    assert_eq!(
        anterior, preparado.duration_ms,
        "las marcas no llegan hasta el final del audio"
    );
    // Y cada una lleva el locator de su fragmento, no un número.
    assert!(preparado.marks.iter().all(|m| !m.locator.href.is_empty()));

    let _ = std::fs::remove_dir_all(destino);
}

/// Solo el capítulo pedido: preparar el primero no prepara el libro.
#[test]
fn preparar_un_capitulo_no_prepara_los_demas() {
    let banco = banco("solo-uno", Arc::new(VozDeMentira::nueva()));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    preparador
        .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("empieza");
    esperar(preparador);

    assert!(
        preparador
            .preparado(&capitulo(&banco.edicion, "c2.xhtml"), None)
            .expect("consulta")
            .is_none(),
        "el segundo capítulo aparece preparado sin haberlo pedido"
    );
    let primero = preparador
        .preparado(&capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("consulta")
        .expect("preparado");
    let entero = {
        preparador
            .empezar(libro(&banco.edicion), None)
            .expect("empieza");
        esperar(preparador);
        preparador
            .preparado(&libro(&banco.edicion), None)
            .expect("consulta")
            .expect("preparado")
    };
    assert!(
        entero.duration_ms > primero.duration_ms,
        "el libro entero no dura más que un capítulo"
    );

    let _ = std::fs::remove_dir_all(destino);
}

/// Preparar dos veces lo mismo **sustituye**, y no acumula archivos ni marcas.
#[test]
fn preparar_dos_veces_lo_mismo_no_acumula() {
    let banco = banco("dos-veces", Arc::new(VozDeMentira::nueva()));
    let (preparador, db, destino) = (&banco.preparador, &banco.db, &banco.destino);

    for _ in 0..2 {
        preparador
            .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
            .expect("empieza");
        esperar(preparador);
    }

    let conn = db.connection().expect("conexión");
    let filas: i64 = conn
        .query_row("SELECT count(*) FROM audio_sections", [], |f| f.get(0))
        .expect("cuenta");
    assert_eq!(filas, 1, "quedaron {filas} preparaciones de lo mismo");

    // En `destino/audio`, que es donde `AppPaths::audio` pone lo preparado desde
    // separado de la base para poder medir lo que ocupa sin contarla.
    let archivos = std::fs::read_dir(destino.join("audio"))
        .expect("carpeta")
        .filter(|e| {
            e.as_ref()
                .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "opus"))
        })
        .count();
    assert_eq!(archivos, 1, "quedaron {archivos} archivos de lo mismo");

    let _ = std::fs::remove_dir_all(destino);
}

/// Lo que se dice antes de esperar: fragmentos, cuánto durará, cuánto ocupará y
/// **cuánto tardará con el factor de esta máquina**.
#[test]
fn el_coste_se_dice_antes_de_empezar() {
    let banco = banco("coste", Arc::new(VozDeMentira::nueva()));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    let estimacion = preparador
        .estimar(&libro(&banco.edicion), None)
        .expect("estima");

    assert!(estimacion.fragments > 0);
    assert!(estimacion.characters > 0);
    assert!(estimacion.audio_seconds > 0, "no dura nada");
    assert!(estimacion.bytes > 0, "no ocupa nada");
    // Con factor 1,67, preparar tarda más que escuchar. Es el dato que justifica
    // que esta función exista.
    let trabajo = estimacion.work_seconds.expect("hay factor medido");
    assert!(
        trabajo > estimacion.audio_seconds,
        "preparar salió más rápido que escuchar: {trabajo} contra {}",
        estimacion.audio_seconds
    );
    assert!(!estimacion.already, "dice que ya está y no se ha preparado");

    preparador
        .empezar(libro(&banco.edicion), None)
        .expect("empieza");
    esperar(preparador);
    assert!(
        preparador
            .estimar(&libro(&banco.edicion), None)
            .expect("estima")
            .already,
        "no reconoce lo que acaba de preparar"
    );

    let _ = std::fs::remove_dir_all(destino);
}

/// Cancelar a la mitad **deja lo hecho utilizable**, marcado como parcial.
#[test]
fn cancelar_deja_lo_hecho_y_lo_dice() {
    let banco = banco("cancelar", Arc::new(VozDeMentira::nueva()));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    preparador
        .empezar(libro(&banco.edicion), None)
        .expect("empieza");
    // Se cancela en cuanto haya empezado de verdad, no antes: cancelar sin nada
    // hecho es otro caso, y es el de abajo.
    for _ in 0..200 {
        if preparador.en_curso().is_some_and(|c| c.done > 0) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    preparador.cancelar();
    esperar(preparador);

    if let Some(preparado) = preparador
        .preparado(&libro(&banco.edicion), None)
        .expect("consulta")
    {
        assert_eq!(preparado.status, "partial", "no dice que quedó a medias");
        assert!(!preparado.marks.is_empty(), "no dejó nada utilizable");
        assert!(Path::new(&preparado.path).is_file());
    }

    let _ = std::fs::remove_dir_all(destino);
}

/// Olvidarlo se lleva la fila, las marcas y el archivo: lo derivado que nadie
/// quiere no puede seguir ocupando disco.
#[test]
fn olvidar_lo_preparado_libera_el_disco() {
    let banco = banco("olvidar", Arc::new(VozDeMentira::nueva()));
    let (preparador, db, destino) = (&banco.preparador, &banco.db, &banco.destino);

    preparador
        .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("empieza");
    esperar(preparador);
    let antes = preparador
        .preparado(&capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("consulta")
        .expect("preparado");

    preparador
        .olvidar(&capitulo(&banco.edicion, "c1.xhtml"))
        .expect("olvida");

    assert!(
        !Path::new(&antes.path).is_file(),
        "el archivo sigue ocupando disco"
    );
    let conn = db.connection().expect("conexión");
    let marcas: i64 = conn
        .query_row("SELECT count(*) FROM audio_marks", [], |f| f.get(0))
        .expect("cuenta");
    assert_eq!(marcas, 0, "las marcas se quedaron huérfanas");

    let _ = std::fs::remove_dir_all(destino);
}

/// La muestra sale **del propio libro**, no de una frase de ejemplo.
#[test]
fn la_muestra_se_hace_con_el_texto_del_libro() {
    let banco = banco("muestra", Arc::new(VozDeMentira::nueva()));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    let ruta = preparador
        .muestra(&libro(&banco.edicion), None)
        .expect("muestra");

    assert!(Path::new(&ruta).is_file(), "la muestra no está en disco");
    let bytes = std::fs::read(&ruta).expect("se lee");
    assert_eq!(&bytes[0..4], b"OggS");
    assert!(bytes.len() > 1_000, "la muestra salió vacía");

    let _ = std::fs::remove_dir_all(destino);
}

/// El avance llega por el bus, con el tiempo que falta **medido**, no supuesto.
#[test]
fn el_avance_dice_cuanto_falta_segun_lo_medido() {
    let destino = carpeta("avance");
    let (db, edicion) = base(&destino);
    let bus = EventBus::new();
    let avisos = Arc::new(std::sync::Mutex::new(Vec::new()));
    let suyos = Arc::clone(&avisos);
    bus.subscribe("prueba", move |evento| {
        if let DomainEvent::AudioJobProgress {
            done,
            total,
            remaining_ms,
            ..
        } = evento
        {
            suyos
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push((*done, *total, *remaining_ms));
        }
    });

    let preparador = Preparador::nuevo(
        db,
        Arc::new(LibroDePrueba),
        Arc::new(VozDeMentira::nueva()),
        &rutas(&destino),
        bus,
    );
    preparador.empezar(libro(&edicion), None).expect("empieza");
    esperar(&preparador);

    let avisos = avisos.lock().unwrap_or_else(|p| p.into_inner()).clone();
    assert!(!avisos.is_empty(), "no avisó del avance ni una vez");
    let (ultimo, total, _) = avisos.last().copied().expect("hay avisos");
    assert_eq!(ultimo, total, "el último aviso no llegó al final");
    // Al principio no se dice cuánto falta: el primer fragmento incluye la carga
    // del modelo, y una cuenta atrás de horas que después cae en picado es peor
    // que no decir nada.
    assert!(
        avisos.first().is_some_and(|(_, _, falta)| falta.is_none()),
        "dijo cuánto faltaba antes de tener nada medido"
    );

    let _ = std::fs::remove_dir_all(destino);
}

/// Cuánto ocupan los documentos del catálogo, que es contra lo que se mide el
/// tope de lo derivado. Sin un archivo con tamaño no hay presupuesto que gastar.
fn documentos_de(db: &Database, edicion: &str, bytes: i64) {
    let conn = db.connection().expect("conexión");
    conn.execute(
        "INSERT OR IGNORE INTO library_roots (id, name, path, kind)
              VALUES ('raiz-audio', 'Pruebas', '/tmp/nb-audio-raiz', 'referenced')",
        [],
    )
    .expect("raíz");
    conn.execute(
        "INSERT INTO files (id, edition_id, root_id, relative_path, content_hash,
                            byte_size, media_type, storage_mode, availability)
              VALUES (?1, ?2, 'raiz-audio', ?1, ?1, ?3,
                      'application/epub+zip', 'referenced', 'available')",
        rusqlite::params![format!("f-{edicion}"), edicion, bytes],
    )
    .expect("archivo");
}

/// **El criterio**: lo preparado cabe en el presupuesto de lo derivado,
/// o no se prepara. Y se dice **antes**, no a los treinta minutos de espera.
#[test]
fn lo_que_no_cabe_en_el_presupuesto_no_se_empieza() {
    let banco = banco("sin-sitio", Arc::new(VozDeMentira::nueva()));
    // Un documento de un kilobyte deja un tope de doscientos bytes, que no da ni
    // para la cabecera de un Ogg.
    documentos_de(&banco.db, &banco.edicion, 1_024);

    let estimacion = banco
        .preparador
        .estimar(&libro(&banco.edicion), None)
        .expect("estima");
    assert!(!estimacion.fits, "dice que cabe en doscientos bytes");
    assert_eq!(
        banco.preparador.empezar(libro(&banco.edicion), None),
        Err(NbError::StorageBudgetReached),
        "empezó algo que no cabe"
    );
    assert!(
        banco.preparador.en_curso().is_none(),
        "dejó un trabajo en marcha después de negarse"
    );

    let _ = std::fs::remove_dir_all(&banco.destino);
}

/// Lo encolado cuenta contra el tope aunque todavía no ocupe nada en disco.
///
/// Sin esto, veinte libros pasarían de uno en uno -cada uno cabiendo en lo que
/// queda- y el vigésimo empezaría con el tope pasado por los diecinueve
/// anteriores, que es exactamente el defecto que el presupuesto existe para
/// impedir.
#[test]
fn lo_encolado_ocupa_sitio_aunque_no_este_escrito() {
    let (voz, freno) = VozDeMentira::retenida();
    let banco = banco("cola-que-ocupa", Arc::new(voz));
    // El segundo trabajo es un capítulo, que es la tercera parte del libro: con un
    // margen del veinte por ciento no cabe, y el libro entero sí cabía. El margen
    // no es más fino porque la propia base crece unos cuatro kilobytes entre que
    // se mide y se pide, y un margen por debajo de eso haría fallar la prueba por
    // el tamaño del diario y no por lo que quiere comprobar.
    let estimacion = banco
        .preparador
        .estimar(&libro(&banco.edicion), None)
        .expect("estima");

    // Un tope que deje sitio para el libro y poco más **de los de ahora**: lo que ya
    // ocupa la base cuenta como derivado, así que el tamaño de los documentos se
    // calcula sobre lo medido y no sobre el papel. Escrito a ciegas, el tope daba
    // menos que la propia base y no entraba ni el primero.
    documentos_de(&banco.db, &banco.edicion, 1_024);
    let conn = banco.db.connection().expect("conexión");
    let derivado = nb_books_lib::diagnostics::medir_almacenamiento(&conn, &rutas(&banco.destino))
        .expect("se puede medir")
        .derived_bytes;
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let documentos = ((derivado as f64 + estimacion.bytes as f64 * 1.2) * 100.0 / 20.0) as i64;
    conn.execute(
        "UPDATE files SET byte_size = ?1 WHERE edition_id = ?2",
        rusqlite::params![documentos, &banco.edicion],
    )
    .expect("tamaño");
    drop(conn);

    banco
        .preparador
        .empezar(libro(&banco.edicion), None)
        .expect("el primero cabe");
    assert_eq!(
        banco
            .preparador
            .empezar(capitulo(&banco.edicion, "c1.xhtml"), None),
        Err(NbError::StorageBudgetReached),
        "encoló un segundo trabajo que ya no cabía"
    );

    freno.store(false, Ordering::SeqCst);
    esperar(&banco.preparador);
    let _ = std::fs::remove_dir_all(&banco.destino);
}

/// Una colección encola sus libros, y dice cuántos entraron.
#[test]
fn una_coleccion_encola_los_libros_que_tiene() {
    let (voz, freno) = VozDeMentira::retenida();
    let banco = banco("coleccion", Arc::new(voz));
    let conn = banco.db.connection().expect("conexión");
    let coleccion = nb_books_lib::collections::Collection::create_manual(
        &conn,
        "Para escuchar",
        nb_books_lib::collections::CollectionOrigin::Personal,
    )
    .expect("colección");
    nb_books_lib::collections::Collection::add(&conn, &coleccion.id, &banco.edicion)
        .expect("miembro");
    drop(conn);

    let encoladas = banco
        .preparador
        .encolar_coleccion(&coleccion.id, None)
        .expect("encola");
    assert_eq!(encoladas, 1, "no encoló el libro de la colección");
    assert_eq!(
        banco.preparador.en_curso().map(|a| a.edition_id),
        Some(banco.edicion.clone()),
        "encoló algo que no es el libro de la colección"
    );

    freno.store(false, Ordering::SeqCst);
    esperar(&banco.preparador);
    let _ = std::fs::remove_dir_all(&banco.destino);
}

/// **Quitar lo preparado de un capítulo no se lleva el libro entero.**
///
/// Antes `olvidar` recibía la edición y borraba todo lo de ese libro,
/// así que el botón del capítulo se llevaba también horas de audio del libro.
#[test]
fn quitar_un_capitulo_preparado_deja_el_libro_entero() {
    let banco = banco("quitar-uno", Arc::new(VozDeMentira::nueva()));
    let preparador = &banco.preparador;
    preparador
        .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("capítulo");
    esperar(preparador);
    preparador
        .empezar(libro(&banco.edicion), None)
        .expect("libro");
    esperar(preparador);

    preparador
        .olvidar(&capitulo(&banco.edicion, "c1.xhtml"))
        .expect("olvidar el capítulo");

    assert!(
        preparador
            .preparado(&capitulo(&banco.edicion, "c1.xhtml"), None)
            .expect("consulta")
            .is_none(),
        "el capítulo sigue preparado después de quitarlo"
    );
    assert!(
        preparador
            .preparado(&libro(&banco.edicion), None)
            .expect("consulta")
            .is_some(),
        "quitar el capítulo se llevó el libro entero"
    );
    let _ = std::fs::remove_dir_all(&banco.destino);
}

/// Una voz que entra en pánico, como la conversión a fonemas con "III" hasta el
/// 2026-09-23.
struct VozQueSeRompe;

impl Sintetizador for VozQueSeRompe {
    fn instalada(&self) -> bool {
        true
    }
    fn revision(&self) -> &'static str {
        "revision-de-prueba"
    }
    fn voz_por_defecto(&self) -> String {
        "ef_dora".to_owned()
    }
    fn factor_medido(&self) -> Option<f32> {
        Some(1.67)
    }
    fn sintetizar(&self, _texto: &str, _voz: Option<&str>, _velocidad: f32) -> NbResult<Sonido> {
        panic!("la voz se rompió");
    }
    fn sintetizar_a_archivo(
        &self,
        _carpeta: &Path,
        _texto: &str,
        _voz: Option<&str>,
        _velocidad: f32,
    ) -> NbResult<PathBuf> {
        panic!("la voz se rompió");
    }
}

/// **Un pánico de la voz termina la preparación, no el hilo.** Antes el
/// hilo moría, el trabajo se quedaba en curso para siempre y lo que se encolaba
/// después no empezaba nunca.
#[test]
fn si_la_voz_entra_en_panico_la_preparacion_termina_y_la_siguiente_empieza() {
    let banco = banco("panico", Arc::new(VozQueSeRompe));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    preparador
        .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("empieza");
    esperar(preparador);
    assert!(
        preparador
            .preparado(&capitulo(&banco.edicion, "c1.xhtml"), None)
            .expect("consulta")
            .is_none_or(|p| p.status != "ready"),
        "no puede quedar listo lo que no se dijo"
    );

    // Y la cola sigue viva: lo siguiente empieza y termina.
    preparador
        .empezar(capitulo(&banco.edicion, "c2.xhtml"), None)
        .expect("empieza otra vez");
    esperar(preparador);

    let _ = std::fs::remove_dir_all(destino);
}

/// **El criterio**: lo preparado dice con qué voz y revisión de modelo
/// se hizo. Si la voz o el modelo cambian, no coincide (`matches_current = false`)
/// y la estimación no lo da por ya hecho (`already = false`), sin borrar lo existente.
#[test]
fn lo_preparado_dice_con_que_voz_y_revision_se_hizo_y_avisa_si_cambia() {
    let banco = banco("voz-y-modelo", Arc::new(VozDeMentira::nueva()));
    let (preparador, destino) = (&banco.preparador, &banco.destino);

    let alcance = capitulo(&banco.edicion, "c1.xhtml");
    preparador
        .empezar(alcance.clone(), Some("ef_dora".to_owned()))
        .expect("empieza");
    esperar(preparador);

    // Con la misma voz y modelo: dice la voz, la revisión y coincide.
    let preparado = preparador
        .preparado(&alcance, Some("ef_dora"))
        .expect("consulta")
        .expect("está preparado");
    assert_eq!(preparado.voice, "ef_dora");
    assert_eq!(preparado.model_rev, "revision-de-prueba");
    assert!(preparado.matches_current);

    let estimacion = preparador
        .estimar(&alcance, Some("ef_dora"))
        .expect("estimación");
    assert!(estimacion.already);

    // Si se pregunta con otra voz: sigue devolviendo lo que hay en disco,
    // pero dice con qué voz se hizo y marca que no coincide con la elegida.
    let cambiado = preparador
        .preparado(&alcance, Some("otra_voz"))
        .expect("consulta")
        .expect("sigue preparado");
    assert_eq!(cambiado.voice, "ef_dora");
    assert_eq!(cambiado.model_rev, "revision-de-prueba");
    assert!(
        !cambiado.matches_current,
        "no puede marcar que coincide si la voz es otra"
    );

    // Y la estimación con la voz nueva dice que no está preparado con ella:
    let estimacion_otra = preparador
        .estimar(&alcance, Some("otra_voz"))
        .expect("estimación");
    assert!(!estimacion_otra.already, "con otra voz no está preparado");

    let _ = std::fs::remove_dir_all(destino);
}

/// Lo preparado con otra voz se conserva, porque sigue sonando; lo de una voz que
/// el catálogo ya no tiene se borra solo y se avisa una vez.
#[test]
fn lo_preparado_con_una_voz_retirada_se_borra_y_se_avisa_una_vez() {
    let banco = banco("voz-retirada", Arc::new(VozDeMentira::nueva()));
    let preparador = &banco.preparador;
    preparador
        .empezar(capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("empieza");
    esperar(preparador);
    let preparado = preparador
        .preparado(&capitulo(&banco.edicion, "c1.xhtml"), None)
        .expect("consulta")
        .expect("preparado");

    // Con su voz todavía en el catálogo, no se toca.
    assert_eq!(
        preparador
            .retirar_de_voces_retiradas(&[preparado.voice.as_str()])
            .expect("revisa"),
        0
    );
    assert_eq!(preparador.aviso_de_audio_retirado().expect("aviso"), None);

    // Con la voz retirada, se borra con su archivo y se dice una sola vez.
    assert_eq!(
        preparador
            .retirar_de_voces_retiradas(&["otra-voz"])
            .expect("revisa"),
        1
    );
    assert!(!std::path::Path::new(&preparado.path).exists());
    assert_eq!(
        preparador.aviso_de_audio_retirado().expect("aviso"),
        Some(1)
    );
    assert_eq!(preparador.aviso_de_audio_retirado().expect("aviso"), None);
}
