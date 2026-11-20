//! Identidad del archivo por contenido, y gestión de documentos.
//!
//! Un archivo se identifica por lo que contiene, no por dónde está. Es lo que
//! permite responder a las dos preguntas que una biblioteca real hace todo el
//! rato: **¿ya tengo esto?** cuando alguien importa dos veces lo mismo, y
//! **¿es el mismo de antes?** cuando alguien mueve una carpeta o cambia de
//! disco. Sin identidad por contenido, lo primero duplica y lo segundo pierde
//! las anotaciones.
//!
//! Contratos: `docs/architecture/metadata-model.md`, sección 4, y
//! `docs/architecture/storage.md`.
//!
//! # Qué hay aquí y qué no
//!
//! Aquí está el cálculo de la identidad y la reconciliación con lo ya conocido.
//! **No** está la importación, que vive en `import_directory`: decidir qué se copia y qué se
//! referencia es suyo, igual que el flujo completo con sus defensas.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::db::catalog::{
    ContentHash, Edition, File, NewEdition, NewFile, NewWork, StorageMode, Work,
};
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};
use crate::metadata::{self, Field, Source};
pub use nb_books_lector::library::{hash_file, hash_reader};

/// Identidad calculada de un archivo, con lo que la base ya sabía de él.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub hash: ContentHash,
    pub byte_size: u64,
    pub known: Known,
}

/// Qué sabía ya la biblioteca sobre este contenido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Known {
    /// Nunca se había visto este contenido.
    New,
    /// Ya está registrado y su ruta sigue siendo la misma.
    AlreadyRegistered(Box<File>),
    /// Ya está registrado con otra ruta que ya no existe: es el mismo archivo,
    /// movido. Reconciliarlo conserva sus metadatos y sus anotaciones.
    Moved(Box<File>),
    /// Ya está registrado con otra ruta que **sí** existe: son dos copias
    /// distintas del mismo contenido, y ambas son legítimas.
    Copy(Box<File>),
}

/// Calcula la identidad de un archivo y la contrasta con lo ya registrado.
///
/// No escribe nada: responde qué es esto, y quien llama decide qué hacer. La
/// separación importa porque la política de importación (copiar, referenciar,
/// rechazar) es, no de aquí.
///
/// # Errors
///
/// Propaga los de [`hash_file`], y [`NbError::StorageFailure`] si la consulta
/// falla.
/// Ruta absoluta de un archivo, o `None` si su raíz no está localizada.
///
/// Es la única forma admitida de obtener la dirección de un archivo: el catálogo
/// guarda la raíz y la ruta dentro de ella, nunca una ruta absoluta. Ver
/// `docs/architecture/storage.md`, sección 2.1.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta de la raíz falla.
pub fn resolve_path(conn: &rusqlite::Connection, file: &File) -> NbResult<Option<PathBuf>> {
    let raiz = crate::db::catalog::LibraryRoot::find(conn, &file.root_id)?;
    Ok(raiz
        .and_then(|r| r.path)
        .map(|base| PathBuf::from(base).join(&file.relative_path)))
}

/// Decide en qué raíz vive una ruta, declarándola si hace falta.
///
/// En modo administrado la raíz es siempre la de la biblioteca. En modo
/// referenciado se busca una raíz cuya ruta sea el directorio del archivo, y si
/// no existe **se declara una**. No es un apaño: es la regla del producto, y es
/// la misma que la migración V11 aplicó hacia atrás. Si el usuario acaba con más
/// raíces de las que quiere, fusionarlas es una operación normal.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la ruta no tiene directorio padre, o si la
/// consulta falla.
pub fn ensure_root(
    conn: &rusqlite::Connection,
    destino: &Path,
    modo: StorageMode,
    raiz_elegida: Option<&Path>,
) -> NbResult<(String, String)> {
    let nombre_archivo = destino
        .file_name()
        .ok_or_else(|| {
            tracing::error!("una ruta sin nombre de archivo no puede registrarse");
            NbError::StorageFailure
        })?
        .to_string_lossy()
        .into_owned();

    if modo == StorageMode::Managed {
        // La ruta de la raíz administrada se toma de donde estamos copiando de
        // verdad, que por definición es la carpeta de biblioteca. Así no puede
        // apuntar a un sitio distinto del que se usa, ni hace falta pasarla por
        // parámetro hasta aquí.
        if let Some(carpeta) = destino.parent() {
            crate::db::catalog::LibraryRoot::sync_managed(
                conn,
                &carpeta.to_string_lossy().replace('\\', "/"),
            )?;
        }
        return Ok((
            crate::db::catalog::MANAGED_ROOT_ID.to_owned(),
            nombre_archivo,
        ));
    }

    /*
     * La raíz es la carpeta que el usuario eligió, si la hubo, y su directorio
     * padre si no. En el primer caso la ruta relativa lleva las subcarpetas
     * dentro, que es lo que permite que una biblioteca con mil carpetas de autor
     * sea **una** raíz y no mil.
     */
    if let Some(elegida) = raiz_elegida {
        if let Ok(relativa) = destino.strip_prefix(elegida) {
            let carpeta = elegida.to_string_lossy().replace('\\', "/");
            let raiz = match crate::db::catalog::LibraryRoot::find_by_path(conn, &carpeta)? {
                Some(existente) => existente,
                None => {
                    let nombre = elegida
                        .file_name()
                        .map_or_else(|| carpeta.clone(), |n| n.to_string_lossy().into_owned());
                    crate::db::catalog::LibraryRoot::create_referenced(conn, &nombre, &carpeta)?
                }
            };
            return Ok((raiz.id, relativa.to_string_lossy().replace('\\', "/")));
        }
        // Un archivo fuera de la carpeta elegida no puede colgar de ella. No es
        // un error: se registra por su propio directorio, como siempre.
        tracing::warn!("el archivo no cuelga de la carpeta elegida; se registra por su directorio");
    }

    let directorio = destino
        .parent()
        .ok_or_else(|| {
            tracing::error!("una ruta sin directorio padre no puede registrarse");
            NbError::StorageFailure
        })?
        .to_string_lossy()
        .replace('\\', "/");

    let raiz = match crate::db::catalog::LibraryRoot::find_by_path(conn, &directorio)? {
        Some(existente) => existente,
        None => {
            // El nombre por defecto es el del propio directorio, que es lo que
            // el usuario reconoce al verlo en la lista de raíces. Renombrarla es
            // suyo.
            let nombre = destino
                .parent()
                .and_then(|d| d.file_name())
                .map_or_else(|| directorio.clone(), |n| n.to_string_lossy().into_owned());
            crate::db::catalog::LibraryRoot::create_referenced(conn, &nombre, &directorio)?
        }
    };

    Ok((raiz.id, nombre_archivo))
}

pub fn identify(conn: &rusqlite::Connection, path: &Path) -> NbResult<FileIdentity> {
    let (hash, byte_size) = hash_file(path)?;
    let conocidos = File::find_by_content_hash(conn, &hash)?;

    // La ruta se compara canonicalizada: `./libro.epub` y `/casa/libro.epub`
    // son el mismo archivo, y tratarlos como distintos duplicaría el registro.
    // Un archivo cuya raíz no está localizada no tiene ruta con la que comparar,
    // y por eso `resolve_path` devuelve `None` en lugar de inventar una.
    let canonica = path.canonicalize().ok();
    let resueltas: Vec<(File, Option<PathBuf>)> = conocidos
        .iter()
        .map(|f| resolve_path(conn, f).map(|r| (f.clone(), r)))
        .collect::<NbResult<Vec<_>>>()?;

    let misma_ruta = |resuelta: &Option<PathBuf>| match (resuelta, &canonica) {
        (Some(registrada), Some(a)) => registrada.canonicalize().is_ok_and(|b| &b == a),
        (Some(registrada), None) => registrada.as_os_str() == path.as_os_str(),
        (None, _) => false,
    };

    let known = if let Some((mismo, _)) = resueltas.iter().find(|(_, r)| misma_ruta(r)) {
        Known::AlreadyRegistered(Box::new(mismo.clone()))
    } else if let Some((otro, resuelta)) = resueltas.first() {
        // Sin raíz localizada no se puede saber si el original sigue ahí, y
        // suponer que sí crearía un duplicado. Se trata como mudanza, que es lo
        // que conserva metadatos y anotaciones.
        if resuelta.as_ref().is_some_and(|r| r.exists()) {
            Known::Copy(Box::new(otro.clone()))
        } else {
            Known::Moved(Box::new(otro.clone()))
        }
    } else {
        Known::New
    };

    Ok(FileIdentity {
        hash,
        byte_size,
        known,
    })
}

/// Apunta un archivo ya registrado a su nueva ruta.
///
/// Es lo que se hace con [`Known::Moved`]: actualizar la ruta en lugar de crear
/// un registro nuevo es lo que conserva los metadatos y las anotaciones que
/// cuelgan de su edición.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si el archivo no está registrado.
/// - [`NbError::StorageFailure`] si la actualización falla, en particular si la
///   ruta nueva ya está ocupada por otro registro.
pub fn relocate(conn: &rusqlite::Connection, file_id: &str, nueva: &Path) -> NbResult<()> {
    relocate_within(conn, file_id, nueva, None)
}

/// Apunta un archivo ya registrado a su nueva ruta dentro de la raíz elegida.
///
/// Cuando el hallazgo viene de importar una carpeta o del repaso de una raíz,
/// `raiz_elegida` evita que cada subcarpeta de autor se declare como una raíz
/// nueva en el catálogo.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si el archivo no está registrado.
/// - [`NbError::StorageFailure`] si la actualización falla.
pub fn relocate_within(
    conn: &rusqlite::Connection,
    file_id: &str,
    nueva: &Path,
    raiz_elegida: Option<&Path>,
) -> NbResult<()> {
    let modo = File::find(conn, file_id)?
        .ok_or(NbError::DocumentNotFound)?
        .storage_mode;
    let (root_id, relativa) = ensure_root(conn, nueva, modo, raiz_elegida)?;

    let filas = conn
        .execute(
            "UPDATE files SET root_id = ?2, relative_path = ?3, availability = 'available'
             WHERE id = ?1",
            (file_id, &root_id, &relativa),
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo reubicar el archivo");
            NbError::StorageFailure
        })?;

    if filas == 0 {
        return Err(NbError::DocumentNotFound);
    }

    tracing::info!(
        ruta = %crate::telemetry::redact_path(nueva),
        "archivo reubicado; sus metadatos y anotaciones se conservan"
    );
    Ok(())
}

#[cfg(test)]
mod tests {

    /// Una cabecera de ZIP con su primera entrada, como la lee `sniff`.
    fn zip_que_empieza_por(nombre: &str) -> (Vec<u8>, Vec<u8>) {
        let mut cabecera = b"PK\x03\x04".to_vec();
        cabecera.resize(26, 0);
        cabecera.extend_from_slice(&(nombre.len() as u16).to_le_bytes());
        cabecera.extend_from_slice(&[0, 0]);
        cabecera.extend_from_slice(nombre.as_bytes());
        cabecera.resize(cabecera.len() + 64, 0);
        (cabecera, b"PK\x05\x06".to_vec())
    }

    /// **Un ZIP que no es EPUB no es por eso un cómic**. Los documentos de
    /// oficina son ZIP con imágenes dentro, y entraban como cómics.
    #[test]
    fn un_documento_de_oficina_no_entra_como_comic() {
        use super::{reconocer, Format};
        use crate::error::NbError;
        let (c, z) = zip_que_empieza_por("[Content_Types].xml");
        for ext in ["docx", "pptx", "xlsx", "xlsm", "cbz", "zip"] {
            assert_eq!(
                reconocer(&c, &z, ext),
                Err(NbError::FormatUnsupported),
                "{ext}"
            );
        }
        let (c, z) = zip_que_empieza_por("mimetype");
        assert_eq!(reconocer(&c, &z, "odt"), Err(NbError::FormatUnsupported));
        let (c, z) = zip_que_empieza_por("proyecto/main.py");
        assert_eq!(reconocer(&c, &z, "zip"), Err(NbError::FormatUnsupported));
        let (c, z) = zip_que_empieza_por("001.jpg");
        assert_eq!(reconocer(&c, &z, "zip"), Ok(Format::ComicArchive));
        assert_eq!(reconocer(&c, &z, "cbz"), Ok(Format::ComicArchive));
    }

    /// **Un nombre de archivo no es un título hasta que se presenta como uno.**
    ///
    /// Con el corpus real dentro, 973 ediciones de 152.080 seguían mostrando el
    /// nombre del archivo con sus guiones bajos, que es lo que el usuario ve en la
    /// rejilla. Lo que se fija aquí es qué se cambia y, sobre todo, qué no.
    #[test]
    fn un_nombre_de_archivo_se_presenta_como_titulo() {
        assert_eq!(
            super::titulo_normalizado("Como_tomar_las_mejores_decisiones_en_la_amistad"),
            "Como tomar las mejores decisiones en la amistad"
        );
        // Espacios repetidos y de los extremos: el nombre se queda limpio.
        assert_eq!(
            super::titulo_normalizado("  Dos__palabras  "),
            "Dos palabras"
        );
        // Y lo que **no** se toca, porque cambiarlo sería inventar: el punto de una
        // abreviatura o de un volumen, el guion medio de una edición, y las
        // mayúsculas, que son del autor y no nuestras.
        assert_eq!(
            super::titulo_normalizado("Vol.2 Beta-3 de ORIGEN"),
            "Vol.2 Beta-3 de ORIGEN"
        );
        assert_eq!(super::titulo_normalizado(""), "");
    }
    use super::*;
    use nb_books_lector::library::FRAGMENTO;
    use sha2::{Digest, Sha256};

    /// Lector que anota la petición de lectura más grande que recibió.
    ///
    /// Es la forma de demostrar que el cálculo es en flujo sin medir memoria del
    /// proceso, que en una prueba es poco fiable: si el búfer creciera con el
    /// tamaño del archivo, este número lo delataría.
    struct LectorVigilado<'a> {
        datos: &'a [u8],
        leidos: usize,
        mayor_peticion: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl Read for LectorVigilado<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.mayor_peticion
                .set(self.mayor_peticion.get().max(buf.len()));
            let restantes = self.datos.len() - self.leidos;
            let n = restantes.min(buf.len());
            buf[..n].copy_from_slice(&self.datos[self.leidos..self.leidos + n]);
            self.leidos += n;
            Ok(n)
        }
    }

    /// Vector de prueba del NIST para SHA-256 sobre la cadena vacía. Si esta
    /// prueba falla, la identidad de toda la biblioteca es otra cosa.
    #[test]
    fn el_hash_coincide_con_el_vector_de_prueba_conocido() {
        let (vacio, tamano) = hash_reader(&b""[..]).expect("hash");
        assert_eq!(
            vacio.as_str(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(tamano, 0);

        let (abc, tamano) = hash_reader(&b"abc"[..]).expect("hash");
        assert_eq!(
            abc.as_str(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(tamano, 3);
    }

    /// **Parte del criterio de aceptación:** el cálculo es en flujo, sin cargar
    /// el archivo entero en memoria.
    #[test]
    fn el_calculo_nunca_pide_mas_de_un_fragmento_de_una_vez() {
        let grande = vec![0xAB_u8; FRAGMENTO * 7 + 13];
        let mayor = std::rc::Rc::new(std::cell::Cell::new(0));

        let (_, tamano) = hash_reader(LectorVigilado {
            datos: &grande,
            leidos: 0,
            mayor_peticion: std::rc::Rc::clone(&mayor),
        })
        .expect("hash");

        assert_eq!(tamano, grande.len() as u64);
        assert!(
            mayor.get() <= FRAGMENTO,
            "se pidió un fragmento de {} bytes, mayor que el búfer de {FRAGMENTO}",
            mayor.get()
        );
    }

    /// El troceado no puede alterar el resultado: si lo hiciera, el mismo
    /// archivo tendría identidades distintas según cómo se leyera.
    #[test]
    fn el_troceado_no_cambia_el_resultado() {
        let datos: Vec<u8> = (0..(FRAGMENTO * 3 + 1)).map(|i| (i % 251) as u8).collect();

        let de_una_vez = {
            let mut hasher = Sha256::new();
            hasher.update(&datos);
            ContentHash::from_sha256(&hasher.finalize().into())
        };
        let (troceado, _) = hash_reader(&datos[..]).expect("hash");

        assert_eq!(troceado, de_una_vez);
    }

    #[test]
    fn dos_contenidos_distintos_dan_identidades_distintas() {
        let (a, _) = hash_reader(&b"El nombre de la rosa"[..]).expect("hash");
        let (b, _) = hash_reader(&b"El nombre de la rose"[..]).expect("hash");

        assert_ne!(a, b);
    }

    #[test]
    fn un_archivo_inexistente_se_distingue_de_uno_ilegible() {
        let ruta = crate::paths::temporal_de_pruebas()
            .join(format!("nb-no-existe-{}", std::process::id()));
        assert!(matches!(hash_file(&ruta), Err(NbError::FileNotFound)));
    }

    #[test]
    fn la_identidad_almacenada_se_relee_igual() {
        let (hash, _) = hash_reader(&b"contenido"[..]).expect("hash");
        let releida = ContentHash::parse(hash.as_str()).expect("legible");

        assert_eq!(releida, hash);
        assert!(hash.as_str().starts_with("sha256:"));
        assert_eq!(hash.as_str().len(), ContentHash::LONGITUD);
    }

    /// Una identidad mal formada en la base solo puede venir de una escritura
    /// que se saltó el tipo. Tratarla como válida propagaría el error.
    #[test]
    fn una_identidad_con_forma_invalida_se_rechaza() {
        for entrada in [
            "",
            "abc",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "md5:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "sha256:E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
            "sha256:zzz0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "sha256:e3b0c44",
        ] {
            assert!(
                ContentHash::parse(entrada).is_err(),
                "la identidad {entrada:?} no debería admitirse"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Detección de formato por contenido
// ---------------------------------------------------------------------------

/// Cuántos bytes se leen del principio para reconocer el formato.
///
/// La firma de un formato vive en sus primeros bytes. Leer más sería leer el
/// documento, y reconocerlo no es leerlo.
const CABECERA: usize = 64;

/// Cuántos bytes se leen del final para comprobar que el archivo está entero.
///
/// El directorio central de un ZIP y el `%%EOF` de un PDF están al final, y su
/// ausencia es lo que distingue un archivo completo de uno que se quedó a medias
/// en una descarga.
const COLA: usize = 4096;

/// Formatos que el sistema reconoce hoy.
///
/// Se reconocen **por contenido**. La extensión es una sugerencia del sistema de
/// archivos, no una afirmación verificable: `extension-falsificada.epub` del
/// corpus es un PDF, y tratarlo como EPUB porque se llama así sería creerle al
/// atacante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Epub,
    ComicArchive,
}

impl Format {
    /// Tipo de medio canónico, tal y como se almacena.
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Epub => "application/epub+zip",
            Self::ComicArchive => "application/vnd.comicbook+zip",
        }
    }

    /// Extensión que le correspondería. Solo para detectar discrepancias.
    const fn extension_esperada(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::ComicArchive => "cbz",
        }
    }
}

/// Reconoce el formato de un archivo leyendo su principio y su final.
///
/// # Errors
///
/// - [`NbError::FileNotFound`] si la ruta no existe.
/// - [`NbError::FormatUnsupported`] si el contenido no es de ningún formato
///   conocido.
/// - [`NbError::DocumentCorrupted`] si es de un formato conocido pero está
///   incompleto: un PDF sin su marca de fin, o un archivo comprimido sin su
///   directorio central.
/// - [`NbError::StorageFailure`] si no se puede leer.
pub fn sniff(path: &Path) -> NbResult<Format> {
    let mut archivo = crate::paths::open_noatime(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            NbError::FileNotFound
        } else {
            NbError::StorageFailure
        }
    })?;

    let tamano = archivo
        .metadata()
        .map_err(|_| NbError::StorageFailure)?
        .len();

    let mut cabecera = vec![0_u8; CABECERA.min(tamano as usize)];
    archivo
        .read_exact(&mut cabecera)
        .map_err(|_| NbError::FormatUnsupported)?;

    let desde = tamano.saturating_sub(COLA as u64);
    archivo
        .seek(SeekFrom::Start(desde))
        .map_err(|_| NbError::StorageFailure)?;
    let mut cola = Vec::with_capacity(COLA);
    archivo
        .take(COLA as u64)
        .read_to_end(&mut cola)
        .map_err(|_| NbError::StorageFailure)?;

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    reconocer(&cabecera, &cola, &extension)
}

/// Parte pura del reconocimiento, para poder probarla sin tocar el disco.
///
/// La extensión solo decide entre un ZIP que es cómic y uno que no lo es: el
/// resto se reconoce por contenido, que es lo que no se puede falsificar.
fn reconocer(cabecera: &[u8], cola: &[u8], extension: &str) -> NbResult<Format> {
    if cabecera.starts_with(b"%PDF-") {
        // Un PDF completo termina con `%%EOF`. Sin él, el archivo se quedó a
        // medias y aceptarlo produciría un documento en la biblioteca que nunca
        // se puede abrir.
        return if contiene(cola, b"%%EOF") {
            Ok(Format::Pdf)
        } else {
            Err(NbError::DocumentCorrupted)
        };
    }

    if cabecera.starts_with(b"PK\x03\x04") {
        // El directorio central va al final: sin él no hay ZIP que abrir.
        if !contiene(cola, b"PK\x05\x06") {
            return Err(NbError::DocumentCorrupted);
        }

        // Un EPUB es un ZIP cuya primera entrada, sin comprimir y en una
        // posición fija, declara su tipo. Es lo que lo distingue de un cómic.
        let es_epub = cabecera.len() >= 58
            && &cabecera[30..38] == b"mimetype"
            && &cabecera[38..58] == b"application/epub+zip";

        if es_epub {
            return Ok(Format::Epub);
        }
        // **Un ZIP que no es EPUB no es por eso un cómic.** Antes
        // lo era: los .docx, .pptx y .xlsx -que son ZIP con imágenes dentro- y
        // cualquier .zip entraban como cómics, 152 en una carpeta de apuntes, y al
        // abrirlos se veían sus imágenes sueltas como páginas. Un cómic se
        // llama .cbz, o es un .zip cuya primera entrada es una imagen; un
        // documento de oficina se delata por su primera entrada.
        let primera = primera_entrada(cabecera)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let de_oficina = primera == "[content_types].xml" || primera == "mimetype";
        let imagen = [".jpg", ".jpeg", ".png", ".webp", ".gif", ".avif"]
            .iter()
            .any(|e| primera.ends_with(e));
        return match extension {
            _ if de_oficina => Err(NbError::FormatUnsupported),
            "cbz" => Ok(Format::ComicArchive),
            "zip" if imagen => Ok(Format::ComicArchive),
            _ => Err(NbError::FormatUnsupported),
        };
    }

    Err(NbError::FormatUnsupported)
}

/// Si el nombre promete un documento. Un `.epub` vacío que no se puede leer es un
/// rechazo que hay que decir; una foto en la misma carpeta no lo es.
fn parece_documento(ruta: &Path) -> bool {
    ruta.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "epub" | "pdf" | "cbz"))
}

/// El nombre de la primera entrada de un ZIP, leído de su cabecera local.
fn primera_entrada(cabecera: &[u8]) -> Option<String> {
    let largo = usize::from(u16::from_le_bytes([*cabecera.get(26)?, *cabecera.get(27)?]));
    let nombre = cabecera.get(30..30 + largo)?;
    Some(String::from_utf8_lossy(nombre).into_owned())
}

fn contiene(heno: &[u8], aguja: &[u8]) -> bool {
    heno.windows(aguja.len()).any(|v| v == aguja)
}

// ---------------------------------------------------------------------------
// Importación
// ---------------------------------------------------------------------------

/// Contexto de una importación: dónde se puede leer, dónde se copia y a quién se
/// le cuenta.
///
/// Va agrupado y no suelto porque los tres viajan siempre juntos y ninguno tiene
/// sentido sin los otros: sin ámbito no hay ruta que validar, sin biblioteca no
/// hay dónde copiar, y sin bus lo que entra no se lo cuenta a nadie.
#[derive(Clone, Copy)]
pub struct ImportContext<'a> {
    /// Único lugar del que se admiten rutas. El webview es zona no confiable.
    pub scope: &'a Path,
    /// Carpeta de la biblioteca administrada.
    pub library: &'a Path,
    pub bus: &'a EventBus,
}

/// Lo que se pide importar.
#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Ruta del archivo. Debe venir de una elección explícita del usuario: el
    /// webview nunca propone rutas por su cuenta.
    pub source: PathBuf,
    pub storage_mode: StorageMode,
    /// Carpeta que el usuario eligió, cuando esto viene de importar una carpeta.
    ///
    /// **Es la raíz de biblioteca que se declara**, y la ruta de cada archivo se
    /// guarda relativa a ella. Sin esto, cada archivo declaraba como raíz su
    /// propio directorio: importar una biblioteca con una carpeta por autor
    /// producía **setecientas diecinueve raíces** —medido sobre la biblioteca
    /// real— donde el usuario había elegido una sola carpeta. Y entonces
    /// "reapuntar la raíz al mudarse de equipo", que es lo que hace que una
    /// biblioteca sobreviva a un cambio de máquina, dejaba de ser una operación
    /// y pasaba a ser setecientas.
    pub root: Option<PathBuf>,
}

/// Resultado de una importación.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportOutcome {
    /// Entró un documento nuevo.
    Imported(Box<Imported>),
    /// El contenido ya estaba en la biblioteca. No se crea nada: se devuelve el
    /// archivo que ya existía, para que la interfaz pueda llevar allí al usuario
    /// en lugar de dejarle creer que ha importado algo.
    AlreadyInLibrary(Box<File>),
    /// El mismo contenido ya está en la biblioteca **con otra ruta que sigue
    /// existiendo**: es otra copia exacta del mismo libro. Tampoco se crea
    /// nada, pero se dice aparte: "ya estaba" es el mismo archivo visto otra vez, y
    /// esto es tener el mismo libro dos veces en el disco, que es lo que quien
    /// importa quiere saber. Se devuelve el archivo que ya estaba.
    Duplicate(Box<File>),
}

/// Lo que quedó registrado al importar.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub work: Work,
    pub edition: Edition,
    pub file: File,
    /// La extensión del archivo no correspondía a su contenido.
    ///
    /// No impide la importación: manda el contenido. Se informa porque casi
    /// siempre significa que alguien renombró un archivo, y a veces que alguien
    /// lo disfrazó.
    pub extension_mismatch: bool,
}

/// Importa un documento con todas sus defensas.
///
/// El orden importa y no es casual:
///
/// 1. Se acota la ruta al ámbito autorizado, se canonicaliza y se reconoce el
///    formato **por contenido**. Nada se escribe todavía.
/// 2. Se calcula la identidad y se contrasta con lo ya conocido. Un duplicado
///    termina aquí, sin tocar nada.
/// 3. En modo administrado, la copia va primero a un archivo temporal. Si algo
///    falla después, se borra y no queda copia parcial en la biblioteca.
/// 4. El registro entero va en una transacción.
/// 5. Solo cuando la transacción está confirmada, la copia toma su nombre
///    definitivo y se emite el evento. Ningún consumidor observa estado a medias.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la ruta queda fuera del ámbito.
/// - [`NbError::FileNotFound`], [`NbError::FormatUnsupported`] o
///   [`NbError::DocumentCorrupted`] según lo que falle al reconocer el archivo.
/// - [`NbError::StorageFailure`] si falla la copia o el registro. En ese caso no
///   queda ni registro huérfano ni copia parcial.
pub fn import_document(
    conn: &mut rusqlite::Connection,
    ctx: &ImportContext<'_>,
    request: &ImportRequest,
) -> NbResult<ImportOutcome> {
    // 1. La ruta, acotada y canónica. El webview es zona no confiable, así que
    //    esto va antes que cualquier otra cosa.
    let origen = crate::security::resolve_within(ctx.scope, &request.source)?;
    let formato = sniff(&origen)?;

    let extension_mismatch = origen
        .extension()
        .and_then(|e| e.to_str())
        .is_none_or(|e| !e.eq_ignore_ascii_case(formato.extension_esperada()));

    if extension_mismatch {
        tracing::info!(
            formato = formato.media_type(),
            "la extensión no corresponde al contenido; manda el contenido"
        );
    }

    // 2. Identidad. Un duplicado no llega a escribir nada.
    let identidad = identify(conn, &origen)?;
    match &identidad.known {
        Known::AlreadyRegistered(ya) => {
            return Ok(ImportOutcome::AlreadyInLibrary(ya.clone()));
        }
        Known::Copy(original) => return Ok(ImportOutcome::Duplicate(original.clone())),
        Known::Moved(movido) => {
            let movido = movido.clone();
            relocate_within(conn, &movido.id, &origen, request.root.as_deref())?;
            return Ok(ImportOutcome::AlreadyInLibrary(movido));
        }
        Known::New => {}
    }

    // 3. La copia, si toca, a un nombre temporal.
    let destino = match request.storage_mode {
        StorageMode::Referenced => origen.clone(),
        StorageMode::Managed => {
            copiar_a_la_biblioteca(&origen, ctx.library, &identidad.hash, formato)?
        }
    };
    let temporal = destino.with_extension("parcial");

    // 4. El registro, entero o nada.
    let titulo = titulo_provisional(&origen);
    let resultado = registrar(conn, &identidad, formato, &destino, &titulo, request);

    let Ok((work, edition, file)) = resultado else {
        // 5. Limpieza: si el registro falló, la copia no puede quedarse.
        if request.storage_mode == StorageMode::Managed {
            let _ = std::fs::remove_file(&destino);
            let _ = std::fs::remove_file(&temporal);
        }
        return Err(resultado.expect_err("el camino de error"));
    };

    ctx.bus.emit(DomainEvent::DocumentImported {
        edition_id: edition.id.clone(),
        file_id: file.id.clone(),
        media_type: formato.media_type().to_owned(),
    });

    Ok(ImportOutcome::Imported(Box::new(Imported {
        work,
        edition,
        file,
        extension_mismatch,
    })))
}

/// Título con el que el documento entra en la biblioteca.
///
/// Es el nombre del archivo, sin extensión, y es **provisional**: se registra con
/// procedencia `File`, de modo que cuando se extraiga el título real de los
/// metadatos internos lo sustituya sin preguntar. Y si el usuario lo corrige
/// antes, gana el suyo, porque eso es lo que decide la procedencia.
fn titulo_provisional(origen: &Path) -> String {
    let nombre = titulo_normalizado(origen.file_stem().and_then(|s| s.to_str()).unwrap_or(""));

    if nombre.is_empty() {
        "Documento sin título".to_owned()
    } else {
        nombre
    }
}

/// Un nombre de archivo, presentable como título.
///
/// # Qué hace y qué no
///
/// Cambia los guiones bajos por espacios, junta los espacios repetidos y quita
/// los de los extremos. **Nada más**: no toca mayúsculas, no quita palabras, no
/// adivina autor. El nombre de un archivo lleva información que su dueño puso, y
/// una normalización que interpreta acaba borrando lo que alguien quiso decir.
///
/// # Por qué el guion bajo, y solo él
///
/// Porque es el único caso sin ambigüedad: en un título nunca significa nada, y
/// en un nombre de archivo casi siempre es el espacio que el sistema de archivos
/// no quería. `Como_tomar_las_mejores_decisiones` es una frase con guiones, no un
/// identificador. El punto y el guion medio se quedan como están: `Vol.2` y
/// `Beta-3` son cosas distintas de `Vol 2` y `Beta 3`, y cambiarlos sería
/// inventarse el título de alguien.
///
/// Se aplica al importar y, para lo ya importado, en la pasada de
/// `metadata::normalizar_titulos_de_archivo`.
pub(crate) fn titulo_normalizado(nombre: &str) -> String {
    nombre
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Borra las copias a medias que dejó una importación que no llegó a terminar.
///
/// # Qué se limpia y por qué solo esto
///
/// Una importación administrada copia a `<hash>.parcial` y después renombra. Si
/// el proceso muere **durante la copia**, lo que queda es un `.parcial`: no lo
/// confunde nadie con un documento, y por eso el nombre temporal existe, pero
/// tampoco se lo lleva nadie nunca. En una biblioteca que se importa por tandas
/// de miles, eso son archivos de megas acumulándose sin que aparezcan en ninguna
/// parte de la aplicación.
///
/// Si muere **entre el renombrado y el registro**, en cambio, no hay nada que
/// limpiar y esa es una propiedad del modelo, no una casualidad: el nombre en la
/// biblioteca es la identidad del contenido, así que el archivo huérfano ocupa
/// exactamente el sitio que volvería a ocupar al reimportarlo, y la siguiente
/// pasada lo sobrescribe. La identidad por contenido, que ya hacía gratis la
/// reanudación, hace gratis también esto.
///
/// # Por qué al arrancar y no al importar
///
/// Porque al arrancar no hay ninguna copia en vuelo, y al empezar una
/// importación sí puede haberla: la de otra que siga corriendo. Borrarle el
/// temporal a quien está copiando convertiría esta limpieza en la causa de
/// justo el fallo que evita.
pub fn limpiar_copias_a_medias(library: &Path) -> usize {
    let Ok(entradas) = std::fs::read_dir(library) else {
        // Que no exista es lo normal antes de la primera importación.
        return 0;
    };

    let mut borradas = 0;
    for entrada in entradas.filter_map(Result::ok) {
        let ruta = entrada.path();
        if ruta.extension().is_some_and(|e| e == "parcial") && std::fs::remove_file(&ruta).is_ok() {
            borradas += 1;
        }
    }

    if borradas > 0 {
        // Sin ruta ni nombre: el innegociable de registro no admite ninguna de
        // las dos cosas, y para saber que pasó basta la cuenta.
        tracing::info!(
            borradas,
            "se limpiaron copias a medias de importaciones anteriores"
        );
    }
    borradas
}

fn copiar_a_la_biblioteca(
    origen: &Path,
    library: &Path,
    hash: &ContentHash,
    formato: Format,
) -> NbResult<PathBuf> {
    std::fs::create_dir_all(library).map_err(|error| {
        tracing::error!(error = %error, "no se pudo preparar la carpeta de biblioteca");
        NbError::StorageFailure
    })?;

    // El nombre en la biblioteca administrada es la identidad del contenido, no
    // el nombre original: así dos archivos con el mismo nombre no colisionan, y
    // el nombre no puede llevar caracteres hostiles del sistema de archivos.
    let nombre = format!("{}.{}", &hash.as_str()[7..], formato.extension_esperada());
    let destino = library.join(nombre);
    let temporal = destino.with_extension("parcial");

    // A un temporal primero: si la copia se interrumpe, lo que queda a medias no
    // tiene el nombre definitivo y nada lo confunde con un documento.
    std::fs::copy(origen, &temporal).map_err(|error| {
        tracing::error!(error = %error, "no se pudo copiar el documento a la biblioteca");
        let _ = std::fs::remove_file(&temporal);
        NbError::StorageFailure
    })?;

    std::fs::rename(&temporal, &destino).map_err(|error| {
        tracing::error!(error = %error, "no se pudo colocar la copia en la biblioteca");
        let _ = std::fs::remove_file(&temporal);
        NbError::StorageFailure
    })?;

    Ok(destino)
}

fn registrar(
    conn: &mut rusqlite::Connection,
    identidad: &FileIdentity,
    formato: Format,
    destino: &Path,
    titulo: &str,
    request: &ImportRequest,
) -> NbResult<(Work, Edition, File)> {
    // La raíz se resuelve **dentro** de la transacción: declararla fuera dejaría
    // una raíz huérfana si el registro falla después.
    let tx = crate::db::write_transaction(conn)?;

    let (root_id, relativa) =
        ensure_root(&tx, destino, request.storage_mode, request.root.as_deref())?;

    let work = Work::create(
        &tx,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: None,
        },
    )?;

    let edition = Edition::create(
        &tx,
        &NewEdition {
            work_id: work.id.clone(),
            title: titulo.to_owned(),
            ..NewEdition::default()
        },
    )?;

    let file = File::create(
        &tx,
        &NewFile {
            edition_id: edition.id.clone(),
            root_id: root_id.clone(),
            relative_path: relativa.clone(),
            content_hash: identidad.hash.clone(),
            byte_size: i64::try_from(identidad.byte_size).unwrap_or(i64::MAX),
            media_type: formato.media_type().to_owned(),
            storage_mode: request.storage_mode,
        },
    )?;

    // El título entra con procedencia declarada: viene del archivo, así que la
    // extracción de metadatos podrá mejorarlo sin preguntar, y una
    // corrección del usuario seguirá ganando sobre ambos. Se declara en lugar de
    // proponerse porque el valor ya está escrito, en el mismo `INSERT` de la
    // edición.
    metadata::record_origin(&tx, &edition.id, Field::Title, Source::File, Some(titulo))?;

    tx.commit().map_err(|error| {
        tracing::error!(error = %error, "no se pudo confirmar la importación");
        NbError::StorageFailure
    })?;

    Ok((work, edition, file))
}

// ---------------------------------------------------------------------------
// Disponibilidad
// ---------------------------------------------------------------------------

/// Resultado de revisar la disponibilidad de la biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct AvailabilityReport {
    pub checked: usize,
    pub available: usize,
    /// La ruta ya no existe: volumen desconectado, carpeta movida, archivo
    /// borrado desde fuera de la aplicación.
    pub missing: usize,
    /// La ruta existe pero no se puede leer: permisos, soporte dañado, montaje
    /// en solo escritura.
    pub unreadable: usize,
    /// Cuántos de los que faltan son administrados.
    ///
    /// Un referenciado que falta es normal: el usuario desconectó un disco. Un
    /// **administrado** que falta es otra cosa, porque de esa copia respondemos
    /// nosotros, y significa que algo la borró por detrás.
    pub managed_missing: usize,
}

/// Comprueba dónde está realmente un archivo y actualiza su estado.
///
/// No borra nada ni oculta nada: un archivo que falta deja su documento en la
/// biblioteca, con sus metadatos y sus anotaciones intactos. Es la degradación
/// elegante de la regla 1 de la Constitución aplicada al caso más común de
/// todos, que es desconectar un disco externo.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si el archivo no está registrado.
/// - [`NbError::StorageFailure`] si la consulta o la actualización fallan.
pub fn refresh_availability(
    conn: &rusqlite::Connection,
    file_id: &str,
) -> NbResult<crate::db::catalog::Availability> {
    let ruta: Option<String> = conn
        .query_row(
            &format!(
                "SELECT {} FROM files f WHERE f.id = ?1",
                crate::db::catalog::SQL_RUTA_ABSOLUTA
            ),
            [file_id],
            |row| row.get(0),
        )
        .optional_o_error()?;

    // Sin raíz localizada no hay nada que comprobar en disco: el documento está
    // ausente hasta que el usuario reapunte su raíz.
    let estado = ruta.map_or(crate::db::catalog::Availability::Missing, |r| {
        comprobar(Path::new(&r))
    });
    File::set_availability(conn, file_id, estado)?;
    Ok(estado)
}

/// Cuántos archivos cuelgan de una raíz, y cuántos de ellos no están.
///
/// Se cuenta en SQL y no recorriendo la lista: una raíz puede tener ciento
/// cincuenta mil archivos, y traerlos para contarlos sería traer la biblioteca
/// entera a memoria para decir un número.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn cuentas_de_raiz(conn: &rusqlite::Connection, root_id: &str) -> NbResult<(u32, u32)> {
    conn.query_row(
        "SELECT count(*), sum(CASE WHEN availability <> 'available' THEN 1 ELSE 0 END)
           FROM files WHERE root_id = ?1",
        [root_id],
        |row| {
            let total: i64 = row.get(0)?;
            let ausentes: Option<i64> = row.get(1)?;
            Ok((
                u32::try_from(total).unwrap_or(u32::MAX),
                u32::try_from(ausentes.unwrap_or(0)).unwrap_or(u32::MAX),
            ))
        },
    )
    .map_err(|error| {
        tracing::error!(error = %error, "no se pudieron contar los archivos de una raíz");
        NbError::StorageFailure
    })
}

/// Cuántos cambios de disponibilidad se escriben por transacción.
///
/// Escribir uno por transacción sobre ciento cincuenta mil archivos exige
/// ciento cincuenta mil confirmaciones del diario WAL y tarda minutos; una sola
/// transacción gigante retendría el turno frente a otros escritores. En lotes
/// de mil, cada confirmación dura milisegundos y cede el paso entre tandas.
const LOTE_DISPONIBILIDAD: usize = 1000;

/// Revisa la biblioteca entera.
///
/// Recorre las rutas consultando metadatos del sistema de archivos, no
/// contenido: comprobar si un archivo está no puede costar lo que costaría
/// leerlo. Aun así es trabajo de disco, y sobre una biblioteca grande pertenece
/// al segundo plano, no a la ruta de arranque.
///
/// Solo escribe las filas cuyo estado haya cambiado, y lo hace en lotes con
/// [`crate::db::write_transaction`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn refresh_all_availability(conn: &mut rusqlite::Connection) -> NbResult<AvailabilityReport> {
    use crate::db::catalog::Availability;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT f.id, {}, f.storage_mode, f.availability FROM files f ORDER BY f.id",
            crate::db::catalog::SQL_RUTA_ABSOLUTA
        ))
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo listar la biblioteca");
            NbError::StorageFailure
        })?;

    let filas: Vec<(String, Option<String>, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .and_then(|f| f.collect())
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo listar la biblioteca");
            NbError::StorageFailure
        })?;
    drop(stmt);

    let mut informe = AvailabilityReport::default();
    let mut cambios: Vec<(String, Availability)> = Vec::new();

    for (id, ruta, modo, actual) in filas {
        let estado = ruta.map_or(Availability::Missing, |r| comprobar(Path::new(&r)));
        informe.checked += 1;
        match estado {
            Availability::Available => informe.available += 1,
            Availability::Missing => {
                informe.missing += 1;
                if modo == StorageMode::Managed.as_str() {
                    informe.managed_missing += 1;
                }
            }
            Availability::Unreadable => informe.unreadable += 1,
        }
        if estado.as_str() != actual {
            cambios.push((id, estado));
        }
    }

    for lote in cambios.chunks(LOTE_DISPONIBILIDAD) {
        let tx = crate::db::write_transaction(conn)?;
        {
            let mut actualizar = tx
                .prepare_cached("UPDATE files SET availability = ?2 WHERE id = ?1")
                .map_err(|error| {
                    tracing::error!(error = %error, "no se pudo preparar el cambio de disponibilidad");
                    NbError::StorageFailure
                })?;
            for (id, estado) in lote {
                actualizar
                    .execute(rusqlite::params![id, estado.as_str()])
                    .map_err(|error| {
                        tracing::error!(error = %error, "no se pudo actualizar la disponibilidad");
                        NbError::StorageFailure
                    })?;
            }
        }
        tx.commit().map_err(|error| {
            tracing::error!(error = %error, "no se pudo confirmar el lote de disponibilidad");
            NbError::StorageFailure
        })?;
    }

    if informe.managed_missing > 0 {
        // No se registra cuál: la ruta es un dato del usuario. El número basta
        // para que la interfaz lo cuente y el diagnóstico lo recoja.
        tracing::warn!(
            cuantos = informe.managed_missing,
            "faltan archivos de la biblioteca administrada"
        );
    }

    Ok(informe)
}

/// Distingue las tres situaciones que la interfaz necesita distinguir.
fn comprobar(ruta: &Path) -> crate::db::catalog::Availability {
    use crate::db::catalog::Availability;

    // Existir no es lo mismo que poder leerse: un montaje sin permisos, o un
    // soporte con un sector dañado, existen y no se dejan abrir. Se abre sin
    // leer: una sola llamada al sistema distingue los tres estados sin hacer un
    // `stat` previo redundante.
    match crate::paths::open_noatime(ruta) {
        Ok(_) => Availability::Available,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Availability::Missing,
        Err(_) => Availability::Unreadable,
    }
}

/// Convierte "no hay fila" en el error de dominio correspondiente.
trait FilaObligatoria<T> {
    fn optional_o_error(self) -> NbResult<T>;
}

impl<T> FilaObligatoria<T> for rusqlite::Result<T> {
    fn optional_o_error(self) -> NbResult<T> {
        match self {
            Ok(valor) => Ok(valor),
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(NbError::DocumentNotFound),
            Err(error) => {
                tracing::error!(error = %error, "consulta de disponibilidad fallida");
                Err(NbError::StorageFailure)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Importación masiva
// ---------------------------------------------------------------------------

/// Estado de la importación masiva: si hay una en curso y si se pidió parar.
///
/// Dos `AtomicBool` compartidos y no un canal: son señales de un solo sentido
/// que se consultan entre archivo y archivo, y montar un canal para eso sería
/// infraestructura sin consumidor.
///
/// El de "en curso" existe porque **nada impedía lanzar dos importaciones a la
/// vez**. Pulsar el botón dos veces arrancaba dos hilos recorriendo la misma
/// carpeta contra la misma base: el doble de trabajo, el doble de contención, y
/// un avance que salta hacia atrás porque hay dos contadores distintos
/// escribiendo en el mismo sitio.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelado: std::sync::Arc<std::sync::atomic::AtomicBool>,
    en_curso: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CancelToken {
    /// Crea un testigo sin cancelar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserva la importación si no hay ninguna en marcha.
    ///
    /// Devuelve `false` cuando ya hay una: quien llama debe decírselo al usuario
    /// en lugar de arrancar otra. Es un intercambio atómico, así que dos hilos
    /// que lleguen a la vez no pueden reservarla los dos.
    #[must_use]
    pub fn try_begin(&self) -> bool {
        self.en_curso
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
    }

    /// Libera la reserva. Se llama **siempre** al terminar, con éxito o sin él.
    pub fn finish(&self) {
        self.en_curso
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Indica si hay una importación en marcha.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.en_curso.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Pide que la importación se detenga en cuanto termine el archivo en curso.
    /// Vuelve a dejarlo sin cancelar.
    ///
    /// Lo necesita quien reutiliza el mismo testigo entre importaciones: sin
    /// esto, una cancelación anterior pararía la siguiente antes de que mirase
    /// el primer archivo.
    pub fn reset(&self) {
        self.cancelado
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn cancel(&self) {
        self.cancelado
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Indica si se ha pedido la cancelación.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelado.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Un archivo que no entró, y por qué.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Rejected {
    /// Ruta relativa a la carpeta importada.
    ///
    /// Relativa y no absoluta: el usuario necesita saber cuál de sus archivos
    /// falló, y la parte de la ruta que no aporta nada para eso es justamente la
    /// que revela dónde vive y cómo se llama su cuenta.
    pub path: String,
    /// Código canónico del error, para que la interfaz lo traduzca.
    pub code: &'static str,
}

/// Resultado de una importación masiva.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BulkReport {
    pub processed: usize,
    pub imported: usize,
    /// Ya estaban en la biblioteca. En una reanudación son la mayoría, y no son
    /// un problema: son la prueba de que reanudar no duplica.
    pub already_in_library: usize,
    /// Copias exactas, en otra ruta, de algo que ya estaba. No entran:
    /// se cuentan y se dicen.
    pub duplicates: usize,
    /// Archivos que no son documentos -imágenes, código, hojas de cálculo- y que
    /// se dejan pasar sin más. No son rechazos: nadie pidió que entraran.
    pub ignored: usize,
    pub rejected: Vec<Rejected>,
    /// Se pidió cancelar y quedaron archivos sin mirar.
    pub cancelled: bool,
}

/// Cada cuántos archivos se publica el avance.
///
/// Emitir uno por archivo llenaría el bus de eventos que nadie puede pintar tan
/// rápido; no emitir ninguno dejaría al usuario mirando una barra quieta durante
/// una hora.
const CADA: usize = 25;

/// Cuántas veces se reintenta un documento que falló por almacenamiento.
///
/// **Un fallo de almacenamiento durante una importación masiva casi nunca es
/// del documento**: es que otro escritor tenía el turno. La indexación y la
/// extracción de metadatos escriben a la vez que esto, y con ciento cincuenta
/// mil archivos por delante, que a uno le toque esperar más de la cuenta es
/// cuestión de tiempo, no de suerte.
///
/// Antes ese documento se apuntaba como rechazado y **no volvía a
/// intentarse nunca**: quedaba fuera de la biblioteca para siempre por un
/// problema que había durado milisegundos. La importación real del 2026-09-09
/// dejó fuera 1.207 así.
///
/// Tres y no más: si a la cuarta sigue fallando, ya no es un turno ocupado y
/// contarlo como rechazo es lo correcto. Reintentar indefinidamente convertiría
/// una base rota en una importación que no termina nunca.
const REINTENTOS: u32 = 3;

/// Lo que se espera entre reintentos, multiplicado por el número de intento.
///
/// Corto a propósito: quien tiene el turno lo suelta al confirmar, y eso son
/// milisegundos. Esperar más sería castigar a la importación por un problema que
/// ya pasó.
const ESPERA_ENTRE_REINTENTOS: std::time::Duration = std::time::Duration::from_millis(50);

/// Ejecuta `trabajo`, reintentando **solo** si falló por almacenamiento.
///
/// Devuelve el resultado y cuántos reintentos hicieron falta, que es lo que
/// permite contar los recuperados sin adivinarlo.
///
/// Los demás códigos no se reintentan a propósito: un EPUB roto seguirá roto a
/// la tercera, y reintentarlo solo gastaría el tiempo de quien espera. La
/// diferencia entre "esto no se pudo escribir ahora" y "esto no se puede
/// importar" es justo la que decide si un documento vuelve a intentarse.
fn con_reintentos<T>(mut trabajo: impl FnMut() -> NbResult<T>) -> (NbResult<T>, u32) {
    let mut intentos = 0;
    loop {
        let resultado = trabajo();
        if !matches!(resultado, Err(NbError::StorageFailure)) || intentos >= REINTENTOS {
            return (resultado, intentos);
        }
        intentos += 1;
        std::thread::sleep(ESPERA_ENTRE_REINTENTOS * intentos);
    }
}

/// Importa todos los documentos de una carpeta.
///
/// **Reanudable sin llevar registro de por dónde iba.** No hace falta: la
/// identidad por contenido hace que volver a pasar por un archivo ya
/// importado sea barato y sin efecto. Reanudar es, literalmente, volver a
/// ejecutar lo mismo. Un cursor de progreso persistido habría sido más código y
/// una fuente más de estado que puede quedar mal.
///
/// **Cancelable entre archivo y archivo**, nunca a mitad de uno: interrumpir una
/// copia dejaría exactamente la copia parcial que el producto existe para evitar.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si la carpeta queda fuera del ámbito, o
/// [`NbError::StorageFailure`] si no se puede recorrer. Un archivo que falla no
/// aborta la importación: se anota en el informe y se sigue, porque una carpeta
/// con un archivo roto entre mil no puede impedir que entren los otros
/// novecientos noventa y nueve.
pub fn import_directory(
    conn: &mut rusqlite::Connection,
    ctx: &ImportContext<'_>,
    directory: &Path,
    storage_mode: StorageMode,
    cancel: &CancelToken,
) -> NbResult<BulkReport> {
    let raiz = crate::security::resolve_within(ctx.scope, directory)?;

    /*
     * La ejecución se apunta **antes de recorrer la carpeta**, y no al terminar.
     *
     * Recorrer ciento cincuenta mil archivos tarda, y si el proceso muere ahí
     * dentro, apuntar al final no habría apuntado nada: terminar es justo lo que
     * no ocurre en el caso que este registro existe para cubrir. Con la fila ya
     * abierta, al volver a arrancar se puede ofrecer continuar.
     *
     * Si abrirla falla no se aborta la importación: importar es lo que el usuario
     * pidió, y llevar la cuenta es un servicio. Se sigue sin registro y se dice en
     * el log.
     */
    let ejecucion = match crate::imports::abrir(&*conn, &raiz.to_string_lossy(), storage_mode) {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(code = error.code(), "la importación no se pudo registrar");
            None
        }
    };

    let archivos = match recorrer(&raiz) {
        Ok(archivos) => archivos,
        Err(error) => {
            // Se cierra lo que se abrió: una carpeta que no se puede recorrer no
            // es una importación a medias que ofrecer continuar, es una que no
            // llegó a empezar.
            if let Some(id) = &ejecucion {
                let _ = crate::imports::cerrar(
                    &*conn,
                    id,
                    crate::imports::Resumen {
                        cancelada: true,
                        ..crate::imports::Resumen::default()
                    },
                );
            }
            return Err(error);
        }
    };
    let total = archivos.len();

    let mut informe = BulkReport::default();
    // Cuántos entraron gracias a un reintento. Se registra al terminar, porque
    // es la cifra que dice si la contención sigue ahí.
    let mut recuperados = 0_u32;

    // Un primer aviso antes de empezar: así la interfaz sabe cuántos hay desde
    // el segundo cero, en lugar de esperar a los veinticinco primeros para
    // enterarse de que la cosa va para largo.
    emitir_avance(ctx.bus, &informe, total);

    for archivo in archivos {
        if cancel.is_cancelled() {
            informe.cancelled = true;
            break;
        }

        informe.processed += 1;

        let peticion = ImportRequest {
            source: archivo.clone(),
            storage_mode,
            // Todos cuelgan de la carpeta que el usuario eligió: una raíz
            // para la importación entera, no una por subcarpeta.
            root: Some(raiz.clone()),
        };

        // Un fallo de almacenamiento se reintenta antes de darlo por rechazado.
        let (resultado, intentos) = con_reintentos(|| import_document(conn, ctx, &peticion));

        if intentos > 0 && resultado.is_ok() {
            // Se cuenta en agregado y sin ninguna ruta: es dato del usuario.
            recuperados += 1;
        }

        match resultado {
            Ok(ImportOutcome::Imported(_)) => informe.imported += 1,
            Ok(ImportOutcome::AlreadyInLibrary(_)) => informe.already_in_library += 1,
            Ok(ImportOutcome::Duplicate(original)) => {
                informe.duplicates += 1;
                if let Some(id) = &ejecucion {
                    let relativa = archivo.strip_prefix(&raiz).unwrap_or(&archivo);
                    let _ = crate::imports::anotar_copia(
                        &*conn,
                        id,
                        &relativa.to_string_lossy(),
                        &original.id,
                    );
                }
            }
            // Lo que no es un documento no se rechaza: se deja pasar. Contarlo
            // como rechazo convertía una carpeta de apuntes en "888 rechazados",
            // y el aviso escondía los pocos que de verdad fallaron.
            Err(NbError::FormatUnsupported) if !parece_documento(&archivo) => informe.ignored += 1,
            Err(error) => {
                let rechazo = Rejected {
                    path: archivo
                        .strip_prefix(&raiz)
                        .unwrap_or(&archivo)
                        .to_string_lossy()
                        .into_owned(),
                    code: error.code(),
                };
                // Se apunta en disco además de en el informe en memoria: el
                // informe muere con el panel de esta ejecución, y saber qué quedó
                // fuera es justo lo que no se podía después de cerrarlo.
                if let Some(id) = &ejecucion {
                    let _ = crate::imports::anotar_rechazo(&*conn, id, &rechazo.path, rechazo.code);
                }
                informe.rejected.push(rechazo);
            }
        }

        if informe.processed % CADA == 0 {
            emitir_avance(ctx.bus, &informe, total);
        }
    }

    // **El avance final va antes que el fin, y el orden importa.**
    //
    // Al revés, que es como estaba, la interfaz recibía el fin —y limpiaba el
    // indicador de progreso— y justo después un avance que se lo volvía a
    // poner. El panel se quedaba diciendo "importando" para siempre sobre una
    // importación terminada, y el usuario no sabía si podía importar otra cosa.
    //
    // El avance final sigue emitiéndose porque la última tanda casi nunca cae en
    // el múltiplo que dispara los avances periódicos: sin él, el indicador se
    // quedaba parado unos documentos antes del total.
    emitir_avance(ctx.bus, &informe, total);

    /*
     * La fila se cierra **antes** de emitir el fin, y el orden importa.
     *
     * La interfaz reacciona al fin consultando qué pasó; si el evento llegara
     * primero, encontraría la ejecución todavía abierta y ofrecería continuar una
     * importación que acaba de terminar. Es el mismo motivo por el que el avance
     * final va antes que el fin, unas líneas más arriba.
     *
     * El recuento de rechazados es el **total**, no el número de filas que se
     * llegaron a guardar: por encima del tope se dejan de escribir y la cifra
     * sigue siendo la de verdad.
     */
    if let Some(id) = &ejecucion {
        let _ = crate::imports::cerrar(
            &*conn,
            id,
            crate::imports::Resumen {
                procesados: u32::try_from(informe.processed).unwrap_or(u32::MAX),
                importados: u32::try_from(informe.imported).unwrap_or(u32::MAX),
                ya_estaban: u32::try_from(informe.already_in_library).unwrap_or(u32::MAX),
                rechazados: u32::try_from(informe.rejected.len()).unwrap_or(u32::MAX),
                copias: u32::try_from(informe.duplicates).unwrap_or(u32::MAX),
                cancelada: informe.cancelled,
            },
        );
    }

    // El final se emite siempre, incluso cuando no hubo nada que importar: sin
    // él, la interfaz no puede distinguir una importación terminada de una que
    // va lenta, y se quedaría esperando un avance que no va a llegar.
    ctx.bus.emit(DomainEvent::BulkImportFinished {
        processed: u32::try_from(informe.processed).unwrap_or(u32::MAX),
        imported: u32::try_from(informe.imported).unwrap_or(u32::MAX),
        skipped: u32::try_from(informe.already_in_library).unwrap_or(u32::MAX),
        rejected: u32::try_from(informe.rejected.len()).unwrap_or(u32::MAX),
        duplicates: u32::try_from(informe.duplicates).unwrap_or(u32::MAX),
        cancelled: informe.cancelled,
    });

    // Solo números: una carpeta de mil documentos produciría mil líneas con el
    // título de cada uno.
    tracing::info!(
        procesados = informe.processed,
        importados = informe.imported,
        ya_estaban = informe.already_in_library,
        rechazados = informe.rejected.len(),
        // Los que entraron a la segunda o la tercera. Si esta cifra crece, la
        // contención de escritura sigue ahí aunque ya no se pierda nada.
        recuperados,
        cancelada = informe.cancelled,
        "importación masiva terminada"
    );

    Ok(informe)
}

fn emitir_avance(bus: &EventBus, informe: &BulkReport, total: usize) {
    bus.emit(DomainEvent::BulkImportProgress {
        processed: u32::try_from(informe.processed).unwrap_or(u32::MAX),
        imported: u32::try_from(informe.imported).unwrap_or(u32::MAX),
        skipped: u32::try_from(informe.already_in_library).unwrap_or(u32::MAX),
        rejected: u32::try_from(informe.rejected.len()).unwrap_or(u32::MAX),
        duplicates: u32::try_from(informe.duplicates).unwrap_or(u32::MAX),
        total: u32::try_from(total).unwrap_or(u32::MAX),
    });
}

/// Lista los archivos de una carpeta y sus subcarpetas, en orden estable.
///
/// El orden es alfabético a propósito: una importación que recorre en el orden
/// que le da el sistema de archivos no se puede reproducir, y una que no se
/// puede reproducir no se puede depurar cuando falla en el archivo número
/// ochocientos.
///
/// No sigue enlaces simbólicos: un enlace que apunte a la raíz del disco
/// convertiría "importar una carpeta" en "recorrer el sistema entero".
fn recorrer(raiz: &Path) -> NbResult<Vec<PathBuf>> {
    let mut salida = Vec::new();
    let mut pendientes = vec![raiz.to_path_buf()];

    while let Some(dir) = pendientes.pop() {
        let entradas = std::fs::read_dir(&dir).map_err(|error| {
            tracing::error!(error = %error, "no se pudo recorrer la carpeta");
            NbError::StorageFailure
        })?;

        let mut archivos = Vec::new();
        let mut subdirectorios = Vec::new();

        for entrada in entradas.filter_map(Result::ok) {
            let ruta = entrada.path();
            // `symlink_metadata` no sigue el enlace: es lo que permite saber que
            // lo es en lugar de mirar a dónde apunta.
            let Ok(meta) = std::fs::symlink_metadata(&ruta) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                subdirectorios.push(ruta);
            } else if meta.is_file() {
                archivos.push(ruta);
            }
        }

        archivos.sort();
        salida.extend(archivos);

        subdirectorios.sort();
        // Se apilan al revés para que salgan en orden alfabético.
        subdirectorios.reverse();
        pendientes.extend(subdirectorios);
    }

    Ok(salida)
}

// ---------------------------------------------------------------------------
// Bajas
// ---------------------------------------------------------------------------

/// Qué se hace con el archivo al dar de baja un documento.
///
/// Es un parámetro obligatorio y no un valor por defecto a propósito: borrar el
/// archivo de alguien es irreversible, y quien llama tiene que haberlo decidido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePolicy {
    /// El archivo se queda donde está. Sale del catálogo, no del disco.
    Keep,
    /// Se borra **solo si es una copia administrada**.
    ///
    /// Un archivo referenciado no se borra nunca, aunque se pida: vive fuera de
    /// la biblioteca porque el usuario decidió que viviera ahí, y puede estar en
    /// una carpeta compartida, en un disco de otra persona o dentro de otra
    /// aplicación. Del que respondemos es del que copiamos nosotros.
    DeleteManagedCopy,
}

/// Lo que se llevó por delante una baja.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deleted {
    pub files_removed: usize,
    pub annotations_removed: usize,
    /// La copia administrada que se borró del disco, si se pidió y procedía.
    pub file_deleted: Option<PathBuf>,
    pub covers_removed: usize,
    /// La obra se fue con su última edición.
    pub work_removed: bool,
    pub contributors_swept: usize,
    pub subjects_swept: usize,
}

/// Da de baja un documento y limpia todo lo que colgaba de él.
///
/// El orden es el que evita el peor caso: **primero la base, en una transacción,
/// y solo después los archivos de caché**. Al revés, un fallo al confirmar
/// dejaría un documento en el catálogo sin su portada, que es un documento roto
/// a la vista del usuario. Una portada huérfana, en cambio, es un archivo
/// regenerable que no molesta a nadie.
///
/// Se barren además los contribuidores y las materias que dejan de estar
/// enlazados a nada. Es distinto de quitar una etiqueta a mano, que no las
/// barre: dar de baja un documento es decir "esto y todo lo suyo se va", y
/// quitar una etiqueta no lo es.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la edición no existe.
/// - [`NbError::StorageFailure`] si la transacción falla; en ese caso no se
///   borra nada, ni de la base ni del disco.
pub fn delete_document(
    conn: &mut rusqlite::Connection,
    bus: &EventBus,
    edition_id: &str,
    covers_dir: &Path,
    policy: FilePolicy,
) -> NbResult<Deleted> {
    let mut informe = Deleted::default();

    // Lo que hay que saber **antes** de borrar, porque después no se puede
    // consultar: qué archivos tenía y de qué obra colgaba.
    let archivos = File::list_for_edition(conn, edition_id)?;
    if archivos.is_empty() && File::list_for_edition(conn, edition_id)?.is_empty() {
        // Una edición sin archivos puede existir; lo que no puede es no existir.
        let existe: bool =
            conn.query_row(
                "SELECT count(*) FROM editions WHERE id = ?1",
                [edition_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| {
                tracing::error!(error = %error, "no se pudo comprobar la edición");
                NbError::StorageFailure
            })? > 0;
        if !existe {
            return Err(NbError::DocumentNotFound);
        }
    }

    let work_id: String = conn
        .query_row(
            "SELECT work_id FROM editions WHERE id = ?1",
            [edition_id],
            |row| row.get(0),
        )
        .map_err(|_| NbError::DocumentNotFound)?;

    let a_borrar_del_disco = match policy {
        FilePolicy::Keep => None,
        FilePolicy::DeleteManagedCopy => archivos
            .iter()
            .find(|f| f.storage_mode == StorageMode::Managed)
            .map(|f| resolve_path(conn, f))
            .transpose()?
            .flatten(),
    };

    // --- La base, entera o nada ---
    let tx = crate::db::write_transaction(conn)?;

    informe.annotations_removed = tx
        .query_row(
            "SELECT count(*) FROM annotations WHERE edition_id = ?1",
            [edition_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| NbError::StorageFailure)? as usize;
    informe.files_removed = archivos.len();

    // El resto cae por cascada: archivos, anotaciones, identificadores, enlaces
    // de contribuidor y de materia, y pertenencias a colecciones. La procedencia
    // y las propuestas las retiran los disparadores de la V3.
    tx.execute("DELETE FROM editions WHERE id = ?1", [edition_id])
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo dar de baja la edición");
            NbError::StorageFailure
        })?;

    // Una obra sin ediciones no es nada: no se puede abrir, ni buscar, ni
    // mostrar. Se va con la última.
    let quedan: i64 = tx
        .query_row(
            "SELECT count(*) FROM editions WHERE work_id = ?1",
            [&work_id],
            |row| row.get(0),
        )
        .map_err(|_| NbError::StorageFailure)?;
    if quedan == 0 {
        tx.execute("DELETE FROM works WHERE id = ?1", [&work_id])
            .map_err(|_| NbError::StorageFailure)?;
        informe.work_removed = true;
    }

    informe.contributors_swept = tx
        .execute(
            "DELETE FROM contributors WHERE id NOT IN
                 (SELECT contributor_id FROM edition_contributors)",
            [],
        )
        .map_err(|_| NbError::StorageFailure)?;

    informe.subjects_swept = tx
        .execute(
            "DELETE FROM subjects WHERE id NOT IN (SELECT subject_id FROM edition_subjects)",
            [],
        )
        .map_err(|_| NbError::StorageFailure)?;

    tx.commit().map_err(|error| {
        tracing::error!(error = %error, "no se pudo confirmar la baja");
        NbError::StorageFailure
    })?;

    // --- Y ahora sí, el disco ---
    if let Some(ruta) = a_borrar_del_disco {
        match std::fs::remove_file(&ruta) {
            Ok(()) => informe.file_deleted = Some(ruta),
            // El registro ya no está, así que el archivo no es de nadie. Se
            // avisa y se sigue: fallar aquí no devolvería el documento.
            Err(error) => tracing::warn!(
                clase = %error.kind(),
                "no se pudo borrar la copia administrada; queda huérfana en la biblioteca"
            ),
        }
    }

    informe.covers_removed = borrar_portadas(covers_dir, edition_id);

    bus.emit(DomainEvent::DocumentDeleted {
        edition_id: edition_id.to_owned(),
    });

    tracing::info!(
        archivos = informe.files_removed,
        anotaciones = informe.annotations_removed,
        portadas = informe.covers_removed,
        obra_retirada = informe.work_removed,
        "documento dado de baja"
    );

    Ok(informe)
}

/// Borra las portadas de una edición, sea cual sea su extensión.
///
/// Y **también sus miniaturas**. Una miniatura se llama `<portada>.min.jpg`, así
/// que su raíz no es el identificador de la edición sino el nombre entero de la
/// portada: comparar por la raíz las dejaba huérfanas en la caché para siempre,
/// una por cada obra borrada.
fn borrar_portadas(covers_dir: &Path, edition_id: &str) -> usize {
    let Ok(entradas) = std::fs::read_dir(covers_dir) else {
        return 0;
    };

    entradas
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|nombre| {
                    // El punto importa: sin él, borrar la edición `abc` se
                    // llevaría por delante las portadas de `abcdef`.
                    nombre
                        .strip_prefix(edition_id)
                        .is_some_and(|resto| resto.starts_with('.'))
                })
        })
        .filter(|e| std::fs::remove_file(e.path()).is_ok())
        .count()
}

#[cfg(test)]
mod limpieza {
    use super::*;

    /// Lo que dejó una copia interrumpida se va, y lo que es un documento no.
    ///
    /// Las dos mitades importan por igual: una limpieza que se llevara también
    /// los documentos sería un defecto de pérdida, que es justo lo que esta fase
    /// existe para no tener.
    #[test]
    fn una_copia_a_medias_se_limpia_y_un_documento_no() {
        // Carpeta propia por proceso, que es como lo hacen las demás pruebas de
        // este módulo: el proyecto no trae dependencia de temporales.
        let biblioteca =
            crate::paths::temporal_de_pruebas().join(format!("nb-limpieza-{}", std::process::id()));
        std::fs::create_dir_all(&biblioteca).expect("carpeta");

        std::fs::write(biblioteca.join("abc123.epub.parcial"), b"a medias").expect("temporal");
        std::fs::write(biblioteca.join("def456.epub"), b"entero").expect("documento");

        assert_eq!(limpiar_copias_a_medias(&biblioteca), 1);
        assert!(!biblioteca.join("abc123.epub.parcial").exists());
        assert!(
            biblioteca.join("def456.epub").exists(),
            "la limpieza no puede llevarse un documento"
        );

        std::fs::remove_dir_all(&biblioteca).ok();
    }

    /// Sin carpeta de biblioteca no hay nada que limpiar, y no es un error.
    ///
    /// Es el estado normal antes de la primera importación: arrancar no puede
    /// fallar por eso.
    #[test]
    fn sin_carpeta_de_biblioteca_no_pasa_nada() {
        let ausente =
            crate::paths::temporal_de_pruebas().join(format!("nb-sin-nada-{}", std::process::id()));
        assert_eq!(limpiar_copias_a_medias(&ausente), 0);
    }
}

#[cfg(test)]
mod reintentos {
    use super::{con_reintentos, REINTENTOS};
    use crate::error::NbError;
    use std::cell::Cell;

    /// Un fallo de almacenamiento durante una importación masiva casi nunca es
    /// del documento: es que otro escritor tenía el turno. Si no se reintenta,
    /// ese documento queda fuera de la biblioteca para siempre por un problema
    /// que duró milisegundos. Pasó con 1.207 el 2026-09-09.
    #[test]
    fn un_fallo_de_almacenamiento_se_reintenta_hasta_que_entra() {
        let intentos = Cell::new(0);
        let (resultado, reintentos) = con_reintentos(|| {
            intentos.set(intentos.get() + 1);
            if intentos.get() < 3 {
                Err(NbError::StorageFailure)
            } else {
                Ok("entró")
            }
        });

        assert_eq!(resultado, Ok("entró"));
        assert_eq!(reintentos, 2, "hicieron falta dos reintentos, no más");
    }

    /// Reintentar indefinidamente convertiría una base rota en una importación
    /// que no termina nunca. A la cuarta ya no es un turno ocupado.
    #[test]
    fn lo_que_falla_siempre_acaba_contandose_como_rechazo() {
        let (resultado, reintentos) = con_reintentos(|| Err::<(), _>(NbError::StorageFailure));

        assert_eq!(resultado, Err(NbError::StorageFailure));
        assert_eq!(reintentos, REINTENTOS);
    }

    /// **La distinción que decide si un documento vuelve a intentarse.** Un EPUB
    /// roto seguirá roto a la tercera, y reintentarlo solo gasta el tiempo de
    /// quien espera; peor aún, escondería que el formato no se admite detrás de
    /// una espera.
    #[test]
    fn un_formato_que_no_se_admite_no_se_reintenta_ni_una_vez() {
        let veces = Cell::new(0);
        let (resultado, reintentos) = con_reintentos(|| {
            veces.set(veces.get() + 1);
            Err::<(), _>(NbError::FormatUnsupported)
        });

        assert_eq!(resultado, Err(NbError::FormatUnsupported));
        assert_eq!(reintentos, 0);
        assert_eq!(
            veces.get(),
            1,
            "se intentó más de una vez algo que no va a cambiar"
        );
    }
}
