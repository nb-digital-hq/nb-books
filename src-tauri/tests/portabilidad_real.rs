//! **Criterio de aceptación.**
//!
//! "Exportar e importar un catálogo de ciento cincuenta mil documentos termina,
//! no pierde nada y declara cuánto ocupa y cuánto tarda."
//!
//! # Por qué a esta escala y no con ocho libros
//!
//! Porque lo que se está midiendo es el volumen, y un banco de ocho filas lo
//! colapsa: con ocho, cualquier diseño de exportación funciona, incluido uno que
//! cargue el catálogo entero en memoria y lo convierta en una sola cadena. Con
//! ciento cincuenta mil, esa decisión se nota o revienta. Es el mismo error que
//! el simulador que servía una portada para las 152.080 obras, y por el que
//! `AGENTS.md` sección 5 exige que el banco conserve la dimensión que se mide.
//!
//! # De dónde salen las filas
//!
//! De los nombres del corpus real, recorriendo directorios y **sin abrir un solo
//! archivo**: lo que hace falta aquí es la variedad de las cadenas -longitudes,
//! alfabetos, rarezas de diez años acumulando libros-, no el contenido. Lo que
//! no es real es la huella de contenido, que se fabrica del índice: calcularla
//! de verdad exigiría leer 154 GB para medir una exportación que no los toca.
//!
//! Se omite sola si no hay corpus. Para ejecutarla:
//!
//! ```text
//! NB_BOOKS_CORPUS=/ruta/al/corpus cargo test --test portabilidad_real -- --nocapture
//! ```
//!
//! **Nada sale de aquí**: ni rutas, ni títulos, ni autores. Lo que se imprime
//! son números agregados (`AGENTS.md`, sección 5).

use std::path::{Path, PathBuf};
use std::time::Instant;

use nb_books_lib::db::catalog::{
    ContentHash, Edition, File, NewEdition, NewFile, NewWork, StorageMode, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::library::ensure_root;
use nb_books_lib::portability;

/// El tamaño que pide el criterio.
const OBJETIVO: usize = 150_000;

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-portab-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn base(&self, nombre: &str) -> PathBuf {
        self.0.join(format!("{nombre}.sqlite"))
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn corpus() -> Option<PathBuf> {
    let raiz = PathBuf::from(std::env::var_os("NB_BOOKS_CORPUS")?);
    raiz.is_dir().then_some(raiz)
}

/// Recorre las carpetas de autor y devuelve los nombres que encuentre.
///
/// Dos niveles, que es como está organizado el corpus, y en orden estable para
/// que dos ejecuciones sean comparables.
fn nombres(raiz: &Path, cuantos: usize) -> Vec<(String, String)> {
    let mut autores: Vec<PathBuf> = std::fs::read_dir(raiz)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    autores.sort();

    let mut salida = Vec::with_capacity(cuantos.min(200_000));
    for autor in autores {
        let carpeta = autor
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut archivos: Vec<PathBuf> = std::fs::read_dir(&autor)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        archivos.sort();
        for archivo in archivos {
            if salida.len() >= cuantos {
                return salida;
            }
            let nombre = archivo
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            salida.push((carpeta.clone(), nombre));
        }
    }
    salida
}

/// Escribe el catálogo de golpe.
///
/// En una transacción cada cinco mil filas: una sola para las ciento cincuenta
/// mil mantendría vivo el diario entero, y una por fila pagaría ciento cincuenta
/// mil sincronizaciones de disco. Lo que se mide aquí no es esto, pero tenía que
/// terminar en un tiempo razonable para poder medir lo que sí.
fn poblar(db: &Database, filas: &[(String, String)], raiz: &str) -> usize {
    let conn = db.connection().expect("conexión");
    let root_id = ensure_root(
        &conn,
        Path::new(&format!("{raiz}/marcador.epub")),
        StorageMode::Referenced,
        None,
    )
    .expect("raíz")
    .0;
    drop(conn);

    let mut escritas = 0;
    for lote in filas.chunks(5_000) {
        let mut conn = db.connection().expect("conexión");
        let tx = nb_books_lib::db::write_transaction(&mut conn).expect("transacción");
        for (indice, (autor, titulo)) in lote.iter().enumerate() {
            let obra = Work::create(
                &tx,
                &NewWork {
                    canonical_title: titulo.clone(),
                    original_language: None,
                },
            )
            .expect("obra");
            let edicion = Edition::create(
                &tx,
                &NewEdition {
                    work_id: obra.id,
                    title: titulo.clone(),
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

            // Fabricada del número de fila: la huella de verdad exigiría leer
            // 154 GB para medir algo que no los toca. Lo que importa aquí es que
            // sean todas distintas.
            let semilla = (escritas + indice) as u64;
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&semilla.to_le_bytes());
            File::create(
                &tx,
                &NewFile {
                    edition_id: edicion.id,
                    root_id: root_id.clone(),
                    relative_path: format!("{autor}/{titulo}.epub"),
                    content_hash: ContentHash::from_sha256(&bytes),
                    byte_size: 400_000 + (semilla as i64 % 3_000_000),
                    media_type: "application/epub+zip".to_owned(),
                    storage_mode: StorageMode::Referenced,
                },
            )
            .expect("archivo");
        }
        tx.commit().expect("confirmación");
        escritas += lote.len();
    }
    escritas
}

fn cuantas(conn: &rusqlite::Connection, tabla: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {tabla}"), [], |f| f.get(0))
        .expect("recuento")
}

/// **El criterio.**
#[test]
fn exportar_e_importar_un_catalogo_de_ciento_cincuenta_mil_documentos_no_pierde_nada() {
    let Some(raiz) = corpus() else {
        eprintln!("sin corpus: prueba omitida");
        return;
    };

    let tmp = Temporal::nuevo("escala");
    let filas = nombres(&raiz, OBJETIVO);
    assert!(
        filas.len() >= 1_000,
        "el corpus dio {} nombres: la ruta apunta demasiado arriba y la muestra se queda vacía",
        filas.len()
    );

    let origen = Database::open(&tmp.base("origen")).expect("base de origen");
    let reloj = Instant::now();
    let escritas = poblar(&origen, &filas, &raiz.to_string_lossy());
    let poblado = reloj.elapsed();

    let conn = origen.connection().expect("conexión");
    let reloj = Instant::now();
    let exportado = portability::export(&conn).expect("exportación");
    let exportar = reloj.elapsed();

    let reloj = Instant::now();
    let json = portability::to_json(&exportado).expect("json");
    let serializar = reloj.elapsed();
    let bytes = json.len();
    let ruta = tmp.0.join("biblioteca.json");
    std::fs::write(&ruta, &json).expect("escritura");
    drop(json);
    drop(exportado);
    drop(conn);

    let reloj = Instant::now();
    let leido = portability::from_json(&std::fs::read_to_string(&ruta).expect("lectura"))
        .expect("relectura");
    let deserializar = reloj.elapsed();

    let destino = Database::open(&tmp.base("destino")).expect("base de destino");
    let mut conn = destino.connection().expect("conexión");
    let reloj = Instant::now();
    portability::import(&mut conn, &leido).expect("importación");
    let importar = reloj.elapsed();

    // Lo que pide el criterio: no perder nada.
    assert_eq!(cuantas(&conn, "works"), escritas as i64, "obras");
    assert_eq!(cuantas(&conn, "editions"), escritas as i64, "ediciones");
    assert_eq!(cuantas(&conn, "files"), escritas as i64, "archivos");

    // Y que sean las mismas, no solo las mismas cuantas: si la importación
    // mezclara filas, los recuentos cuadrarían igual.
    let origen_conn = origen.connection().expect("conexión");
    let huellas = |c: &rusqlite::Connection| -> Vec<String> {
        let mut consulta = c
            .prepare("SELECT content_hash FROM files ORDER BY content_hash")
            .expect("consulta");
        let filas: Vec<String> = consulta
            .query_map([], |f| f.get(0))
            .expect("filas")
            .filter_map(Result::ok)
            .collect();
        filas
    };
    assert_eq!(
        huellas(&origen_conn),
        huellas(&conn),
        "las mismas cuantas no basta: tienen que ser las mismas"
    );

    // Lo que pide el criterio: declarar cuánto ocupa y cuánto tarda.
    eprintln!("--- portabilidad, {escritas} documentos");
    eprintln!("poblar la base        {poblado:?}");
    eprintln!("exportar (a memoria)  {exportar:?}");
    eprintln!("serializar a json     {serializar:?}");
    eprintln!("releer el json        {deserializar:?}");
    eprintln!("importar              {importar:?}");
    eprintln!(
        "el archivo ocupa      {:.1} MB ({} bytes por documento)",
        bytes as f64 / 1_048_576.0,
        bytes / escritas.max(1)
    );
}
