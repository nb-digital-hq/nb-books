//! Criterio de aceptación de la. y requisito R24.
//!
//! "En Linux, importar, indexar y abrir usan `O_NOATIME` donde el sistema lo admite
//! y siguen funcionando donde no; una prueba lo comprueba sobre un archivo propio;
//! la documentación dice qué pasa en las otras plataformas."
//!
//! Leer los archivos de una biblioteca referenciada (importar, indexar, abrir)
//! no debe actualizar su hora de acceso (`atime`) en Linux con `relatime`,
//! evitando modificar metadatos temporales de los archivos originales del usuario.
//!
//! Contrato: `docs/architecture/storage.md`, sección 2.5.

use std::fs::FileTimes;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nb_books_lib::db::catalog::{File, StorageMode};
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{
    import_document, resolve_path, ImportContext, ImportOutcome, ImportRequest,
};
use nb_books_lib::paths::{open_noatime, read_bytes_noatime};
use nb_books_lib::reader::epub::EpubFactory;
use nb_books_lib::reader::progress::ProgressTracker;
use nb_books_lib::reader::{ProviderRegistry, ReadingSession};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn directorio_de_prueba(nombre: &str) -> PathBuf {
    // Si existe /run/user/<uid> (montado habitualmente con relatime en escritorios Linux),
    // lo preferimos para verificar el comportamiento de relatime; de lo contrario, temp_dir.
    let base = if Path::new("/run/user/1000").is_dir() {
        PathBuf::from("/run/user/1000")
    } else {
        nb_books_lib::paths::temporal_de_pruebas()
    };
    let dir = base.join(format!("nb-atime-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio de prueba");
    dir
}

struct Limpiador(PathBuf);
impl Drop for Limpiador {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// En Linux, `open_noatime` y `read_bytes_noatime` no alteran la hora de acceso (`atime`).
#[test]
fn open_noatime_preserva_hora_de_acceso_en_archivo_propio() {
    let dir = directorio_de_prueba("noatime-propio");
    let _guard = Limpiador(dir.clone());
    let archivo = dir.join("prueba_atime.txt");

    std::fs::write(&archivo, b"contenido para verificar atime").expect("escritura");

    let ahora = SystemTime::now();
    let pasado_atime = ahora - Duration::from_secs(300);
    let pasado_mtime = ahora - Duration::from_secs(100);

    // Fijamos atime <= mtime, que bajo relatime activaría la actualización de atime al leer sin O_NOATIME.
    let file = std::fs::File::options()
        .write(true)
        .open(&archivo)
        .expect("abrir para set_times");
    file.set_times(
        FileTimes::new()
            .set_accessed(pasado_atime)
            .set_modified(pasado_mtime),
    )
    .expect("fijar tiempos");
    drop(file);

    let meta_inicial = std::fs::metadata(&archivo).expect("metadata inicial");
    let atime_inicial = meta_inicial.accessed().expect("accessed");

    // Leemos con open_noatime
    let mut f = open_noatime(&archivo).expect("open_noatime");
    let mut buffer = Vec::new();
    std::io::Read::read_to_end(&mut f, &mut buffer).expect("leer");
    drop(f);
    assert_eq!(buffer, b"contenido para verificar atime");

    let meta_post_open = std::fs::metadata(&archivo).expect("metadata post open");
    let atime_post_open = meta_post_open.accessed().expect("accessed post open");

    // Solo Linux tiene O_NOATIME; en las demás basta con que funcione.
    if cfg!(target_os = "linux") {
        assert_eq!(
            atime_inicial, atime_post_open,
            "open_noatime con O_NOATIME debe conservar el atime previo"
        );
    }

    // Leemos con read_bytes_noatime
    let bytes = read_bytes_noatime(&archivo).expect("read_bytes_noatime");
    assert_eq!(bytes, b"contenido para verificar atime");

    let meta_post_bytes = std::fs::metadata(&archivo).expect("metadata post bytes");
    let atime_post_bytes = meta_post_bytes.accessed().expect("accessed post bytes");

    // Solo Linux tiene O_NOATIME; en las demás basta con que funcione.
    if cfg!(target_os = "linux") {
        assert_eq!(
            atime_inicial, atime_post_bytes,
            "read_bytes_noatime con O_NOATIME debe conservar el atime previo"
        );
    }
}

/// Cuando O_NOATIME no es admitido (p. ej. archivos del sistema que no pertenecen al usuario,
/// donde Linux devuelve EPERM), open_noatime degrada limpiamente a File::open estándar.
#[test]
fn open_noatime_degrada_con_elegancia_en_archivos_sin_permiso_de_noatime() {
    let hosts = Path::new("/etc/hosts");
    if hosts.exists() {
        let resultado = open_noatime(hosts);
        assert!(
            resultado.is_ok(),
            "debe abrir exitosamente vía degradación elegante"
        );
        let bytes = read_bytes_noatime(hosts);
        assert!(
            bytes.is_ok(),
            "debe leer bytes exitosamente vía degradación elegante"
        );
    }

    let os_release = Path::new("/etc/os-release");
    if os_release.exists() {
        let resultado = open_noatime(os_release);
        assert!(resultado.is_ok(), "debe abrir /etc/os-release sin error");
    }
}

/// Importar, indexar y abrir un libro referenciado no modifica su hora de acceso.
#[test]
fn importar_indexar_y_abrir_libro_referenciado_no_toca_la_hora_de_acceso() {
    let dir = directorio_de_prueba("flujo-completo");
    let _guard = Limpiador(dir.clone());

    let externa = dir.join("externa");
    let library = dir.join("biblioteca");
    let cache = dir.join("cache/paginas");
    std::fs::create_dir_all(&externa).expect("externa");
    std::fs::create_dir_all(&library).expect("biblioteca");
    std::fs::create_dir_all(&cache).expect("cache");

    let db = Database::open(&dir.join("nb-books.sqlite")).expect("base");
    let bus = EventBus::new();
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(EpubFactory::solo_texto()));

    // Copiamos el libro referenciado a la carpeta externa
    let ruta_libro = externa.join("minimo-valido.epub");
    std::fs::copy(fixtures().join("minimo-valido.epub"), &ruta_libro).expect("copia");

    // Fijamos un atime anterior a mtime para activar la actualización en relatime
    let ahora = SystemTime::now();
    let pasado_atime = ahora - Duration::from_secs(600);
    let pasado_mtime = ahora - Duration::from_secs(200);

    let f = std::fs::File::options()
        .write(true)
        .open(&ruta_libro)
        .expect("abrir libro");
    f.set_times(
        FileTimes::new()
            .set_accessed(pasado_atime)
            .set_modified(pasado_mtime),
    )
    .expect("fijar tiempos del libro");
    drop(f);

    let atime_inicial = std::fs::metadata(&ruta_libro)
        .expect("metadata")
        .accessed()
        .expect("accessed inicial");

    // 1. IMPORTAR (StorageMode::Referenced)
    let mut conn = db.connection().expect("conexión");
    let resultado = import_document(
        &mut conn,
        &ImportContext {
            scope: &externa,
            library: &library,
            bus: &bus,
        },
        &ImportRequest {
            source: ruta_libro.clone(),
            storage_mode: StorageMode::Referenced,
            root: Some(externa.clone()),
        },
    )
    .expect("importar");

    let file_id = match resultado {
        ImportOutcome::Imported(datos) => datos.file.id,
        _ => panic!("debía ser Imported"),
    };

    let atime_tras_importar = std::fs::metadata(&ruta_libro)
        .expect("metadata")
        .accessed()
        .expect("accessed tras importar");

    // Solo Linux tiene O_NOATIME; en las demás basta con que funcione.
    if cfg!(target_os = "linux") {
        assert_eq!(
            atime_inicial, atime_tras_importar,
            "la importación de un libro referenciado no debe alterar su atime"
        );
    }

    // 2. INDEXAR (Apertura por el motor de búsqueda a través del registro)
    let archivo_db = File::find(&conn, &file_id)
        .expect("find")
        .expect("archivo existe");
    let ruta_resuelta = resolve_path(&conn, &archivo_db)
        .expect("resolve")
        .expect("localizada");
    let doc_indexador = registry
        .open(&archivo_db.media_type, &ruta_resuelta)
        .expect("abrir para indexar");
    let nav = doc_indexador.navigation().expect("navigation");
    assert!(
        !nav.entries.is_empty(),
        "navegación de indexación con contenido"
    );
    drop(doc_indexador);

    let atime_tras_indexar = std::fs::metadata(&ruta_libro)
        .expect("metadata")
        .accessed()
        .expect("accessed tras indexar");

    // Solo Linux tiene O_NOATIME; en las demás basta con que funcione.
    if cfg!(target_os = "linux") {
        assert_eq!(
            atime_inicial, atime_tras_indexar,
            "la indexación de un libro referenciado no debe alterar su atime"
        );
    }

    // 3. ABRIR (Sesión de lectura en el lector)
    let tracker = Arc::new(ProgressTracker::new(db.clone()));
    let sesion = ReadingSession::open(&db, &registry, &file_id, tracker, None)
        .expect("abrir sesión de lectura");
    let render = sesion.render().expect("render");
    match render {
        nb_books_lib::reader::Rendered::Markup { html, .. } => {
            assert!(!html.is_empty(), "HTML renderizado no vacío");
        }
        _ => panic!("debía ser Markup"),
    }
    drop(sesion);

    let atime_tras_abrir = std::fs::metadata(&ruta_libro)
        .expect("metadata")
        .accessed()
        .expect("accessed tras abrir");

    // Solo Linux tiene O_NOATIME; en las demás basta con que funcione.
    if cfg!(target_os = "linux") {
        assert_eq!(
            atime_inicial, atime_tras_abrir,
            "abrir una sesión de lectura sobre un libro referenciado no debe alterar su atime"
        );
    }
}
