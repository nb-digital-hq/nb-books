//! Diccionarios instalados: instalar, listar, desinstalar y consultar.
//!
//!
//!
//! # La lista de diccionarios instalados es una carpeta
//!
//! No hay tabla, ni preferencia, ni registro paralelo: un diccionario instalado
//! es un subdirectorio de `paths::dictionaries()`. Eso hace que el criterio de
//! aceptación "desinstalar uno no deja rastro en las preferencias" sea cierto
//! **por construcción** y no por disciplina: no existe un segundo sitio del que
//! haya que acordarse de borrarlo.
//!
//! # Por qué se normaliza al instalar
//!
//! El formato de origen trae el índice en orden ajeno, el desplazamiento en
//! big-endian y las definiciones quizá comprimidas. Convertirlo una vez, al
//! instalar, tiene tres consecuencias que importan:
//!
//! 1. **La validación ocurre antes.** Un diccionario truncado o con marcado que
//!    no entendemos se rechaza al instalarlo, no en la primera consulta.
//! 2. **La consulta es una búsqueda binaria con dos posicionamientos**, sin
//!    cargar el índice entero en memoria ni descomprimir nada.
//! 3. Ningún diccionario entra en el instalador: llegan siempre de fuera.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{NbError, NbResult};

use super::stardict;
use super::{Entry, LookupKind, LookupProvider, Query};

/// Tamaño de un registro del índice normalizado: dos `u32` para la palabra, un
/// `u64` de desplazamiento y un `u32` de tamaño.
const REGISTRO: u64 = 4 + 4 + 8 + 4;

/// Cuántas definiciones se devuelven como mucho para un mismo término.
///
/// Un diccionario puede tener decenas de acepciones homógrafas. Devolverlas
/// todas llenaría el panel de consulta con una lista que nadie lee.
const MAXIMO_ACEPCIONES: usize = 8;

/// Lo que se sabe de un diccionario instalado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "DictionaryManifest"
)]
pub struct Manifest {
    pub id: String,
    /// Nombre de la obra, tal y como lo declara ella.
    pub name: String,
    /// Etiqueta BCP-47. Si el diccionario no la declaraba, la puso el usuario al
    /// instalarlo: sin idioma no se puede decidir cuándo ofrecerlo.
    pub language: String,
    pub word_count: usize,
    pub bytes: u64,
    /// Atribución obligatoria de la obra, cuando su licencia la exige.
    pub attribution: Option<String>,
    /// Segundos desde la época, no una fecha con formato.
    ///
    /// No hay biblioteca de fechas en el árbol y no se añade una por un campo:
    /// darle formato es además trabajo de la interfaz, que es la única que sabe
    /// el idioma y la zona horaria del usuario.
    pub installed_at: u64,
}

fn almacenamiento(detalle: &str) -> NbError {
    tracing::error!(detalle, "fallo en el almacén de diccionarios");
    NbError::StorageFailure
}

/// Instala un diccionario StarDict a partir de su archivo `.ifo`.
///
/// Valida antes de escribir nada y escribe en un directorio temporal que solo se
/// renombra al terminar: una instalación interrumpida no deja un diccionario a
/// medias que luego responda mal.
///
/// # Errors
///
/// - [`NbError::FileNotFound`] si falta alguno de los tres archivos.
/// - [`NbError::DocumentCorrupted`] si está truncado, si el índice no cuadra con
///   lo que declara, o si trae un tipo de contenido que no sabemos convertir.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn install(destino: &Path, ifo: &Path, idioma: Option<&str>) -> NbResult<Manifest> {
    let archivos = stardict::locate(ifo)?;

    let texto_ifo = std::fs::read_to_string(&archivos.ifo).map_err(|error| {
        tracing::info!(error = %error, "no se pudo leer el .ifo");
        NbError::DocumentCorrupted
    })?;
    let info = stardict::parse_info(&texto_ifo)?;

    let idx = stardict::read_maybe_gzip(&archivos.idx, archivos.idx_comprimido)?;
    let entradas = stardict::parse_index(&idx, &info)?;
    let dict = stardict::read_maybe_gzip(&archivos.dict, archivos.dict_comprimido)?;

    // Toda entrada tiene que caber dentro de las definiciones. Un índice que
    // apunta fuera es la forma más común de diccionario truncado, y detectarla
    // aquí evita una lectura fuera de rango en cada consulta.
    for entrada in &entradas {
        let fin = entrada.offset.saturating_add(u64::from(entrada.size));
        if fin > dict.len() as u64 {
            tracing::info!("el índice apunta fuera de las definiciones");
            return Err(NbError::DocumentCorrupted);
        }
    }

    let idioma = idioma
        .map(str::to_owned)
        .or_else(|| info.language.clone())
        .ok_or_else(|| {
            tracing::info!("el diccionario no declara idioma y no se indicó ninguno");
            NbError::DocumentCorrupted
        })?;

    let id = identificador(&info.bookname, &idioma);
    let carpeta = destino.join(&id);
    let temporal = destino.join(format!("{id}.parcial"));
    let _ = std::fs::remove_dir_all(&temporal);
    std::fs::create_dir_all(&temporal)
        .map_err(|_| almacenamiento("no se pudo crear el temporal"))?;

    // El orden de consulta es el nuestro, no el del origen: se ordena por la
    // forma en minúsculas para que buscar no dependa de cómo escribió el usuario.
    let mut ordenadas = entradas;
    ordenadas.sort_by(|a, b| {
        a.word
            .to_lowercase()
            .cmp(&b.word.to_lowercase())
            .then_with(|| a.word.cmp(&b.word))
    });

    let mut words = Vec::new();
    let mut index = Vec::with_capacity(ordenadas.len() * REGISTRO as usize);
    let mut definiciones = Vec::with_capacity(dict.len());

    for entrada in &ordenadas {
        let inicio_palabra = u32::try_from(words.len())
            .map_err(|_| almacenamiento("el diccionario excede el tamaño admitido"))?;
        words.extend_from_slice(entrada.word.as_bytes());

        let bruto =
            &dict[entrada.offset as usize..(entrada.offset + u64::from(entrada.size)) as usize];
        let texto = match info.sametypesequence {
            Some(tipo) => stardict::to_plain_text(bruto, tipo),
            // Sin `sametypesequence`, el primer byte de cada trozo es su tipo.
            None => match bruto.split_first() {
                Some((tipo, resto)) => stardict::to_plain_text(resto, *tipo),
                None => String::new(),
            },
        };

        let inicio_definicion = u64::try_from(definiciones.len())
            .map_err(|_| almacenamiento("el diccionario excede el tamaño admitido"))?;
        definiciones.extend_from_slice(texto.as_bytes());

        index.extend_from_slice(&inicio_palabra.to_le_bytes());
        index.extend_from_slice(
            &u32::try_from(entrada.word.len())
                .map_err(|_| almacenamiento("una palabra excede el tamaño admitido"))?
                .to_le_bytes(),
        );
        index.extend_from_slice(&inicio_definicion.to_le_bytes());
        index.extend_from_slice(
            &u32::try_from(texto.len())
                .map_err(|_| almacenamiento("una definición excede el tamaño admitido"))?
                .to_le_bytes(),
        );
    }

    let manifest = Manifest {
        id: id.clone(),
        name: info.bookname.clone(),
        language: idioma,
        word_count: ordenadas.len(),
        bytes: (words.len() + index.len() + definiciones.len()) as u64,
        attribution: None,
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };

    // Si alguna escritura falla, el temporal se borra: media instalación en
    // disco es lo que este módulo existe para impedir.
    let escrito = (|| -> NbResult<()> {
        escribir(&temporal.join("words.bin"), &words)?;
        escribir(&temporal.join("index.bin"), &index)?;
        escribir(&temporal.join("dict.bin"), &definiciones)?;
        escribir(
            &temporal.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)
                .map_err(|_| almacenamiento("el manifiesto no se deja serializar"))?
                .as_slice(),
        )
    })();

    if let Err(error) = escrito {
        let _ = std::fs::remove_dir_all(&temporal);
        return Err(error);
    }

    // El renombrado es lo último y es atómico dentro del mismo sistema de
    // archivos: quien mire ve el diccionario entero o ninguno.
    let _ = std::fs::remove_dir_all(&carpeta);
    std::fs::rename(&temporal, &carpeta)
        .map_err(|_| almacenamiento("no se pudo confirmar la instalación"))?;

    tracing::info!(
        id = %manifest.id,
        idioma = %manifest.language,
        palabras = manifest.word_count,
        "diccionario instalado"
    );
    Ok(manifest)
}

fn escribir(destino: &Path, bytes: &[u8]) -> NbResult<()> {
    std::fs::write(destino, bytes).map_err(|error| {
        tracing::error!(error = %error, "no se pudo escribir un archivo del diccionario");
        NbError::StorageFailure
    })
}

/// Identificador estable y seguro como nombre de carpeta.
fn identificador(bookname: &str, idioma: &str) -> String {
    use sha2::{Digest, Sha256};

    let babosa: String = bookname
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();

    // El sufijo evita que dos obras con nombres parecidos, o con caracteres que
    // la babosa aplana, se pisen la carpeta.
    let huella = Sha256::digest(format!("{bookname}\u{0}{idioma}").as_bytes());
    let corto: String = format!("{huella:x}").chars().take(8).collect();

    format!("{}-{idioma}-{corto}", babosa.trim_matches('-'))
}

/// Diccionarios instalados, en orden estable.
///
/// Un subdirectorio sin manifiesto legible se **ignora** en lugar de romper el
/// listado: una instalación interrumpida no puede dejar la consulta inservible.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la carpeta existe y no se deja leer.
pub fn list(carpeta: &Path) -> NbResult<Vec<Manifest>> {
    if !carpeta.exists() {
        return Ok(Vec::new());
    }

    let entradas =
        std::fs::read_dir(carpeta).map_err(|_| almacenamiento("no se pudo listar la carpeta"))?;

    let mut manifiestos: Vec<Manifest> = entradas
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| std::fs::read_to_string(e.path().join("manifest.json")).ok())
        .filter_map(|texto| serde_json::from_str(&texto).ok())
        .collect();

    manifiestos.sort_by(|a, b| a.language.cmp(&b.language).then_with(|| a.id.cmp(&b.id)));
    Ok(manifiestos)
}

/// Desinstala un diccionario. Que no estuviera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la carpeta existe y no se deja borrar.
pub fn uninstall(carpeta: &Path, id: &str) -> NbResult<()> {
    let destino = carpeta.join(id);
    if !destino.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&destino).map_err(|_| almacenamiento("no se pudo desinstalar"))?;
    tracing::info!(id, "diccionario desinstalado");
    Ok(())
}

/// Un diccionario instalado, listo para consultar.
pub struct Dictionary {
    manifest: Manifest,
    carpeta: PathBuf,
    idiomas: Vec<String>,
}

impl Dictionary {
    /// Abre un diccionario ya instalado.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si la carpeta no contiene un
    /// diccionario legible.
    pub fn open(carpeta: &Path, id: &str) -> NbResult<Self> {
        let destino = carpeta.join(id);
        let texto = std::fs::read_to_string(destino.join("manifest.json")).map_err(|_| {
            tracing::info!(id, "no hay diccionario instalado con ese identificador");
            NbError::ComponentUnavailable
        })?;
        let manifest: Manifest = serde_json::from_str(&texto).map_err(|_| {
            tracing::info!(id, "el manifiesto del diccionario no es legible");
            NbError::ComponentUnavailable
        })?;

        Ok(Self {
            idiomas: vec![manifest.language.clone()],
            manifest,
            carpeta: destino,
        })
    }

    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Busca un término. Devuelve las acepciones encontradas, en orden.
    ///
    /// La búsqueda es binaria sobre el índice normalizado, con dos
    /// posicionamientos por sonda: el índice no se carga en memoria, que sobre
    /// un diccionario de medio millón de palabras serían decenas de megabytes
    /// por diccionario instalado.
    fn buscar(&self, termino: &str) -> NbResult<Vec<String>> {
        let objetivo = termino.trim().to_lowercase();
        if objetivo.is_empty() {
            return Ok(Vec::new());
        }

        let mut index = abrir(&self.carpeta.join("index.bin"))?;
        let mut words = abrir(&self.carpeta.join("words.bin"))?;
        let total = index
            .metadata()
            .map_err(|_| almacenamiento("índice ilegible"))?
            .len()
            / REGISTRO;

        let leer_palabra = |index: &mut std::fs::File,
                            words: &mut std::fs::File,
                            i: u64|
         -> NbResult<(String, u64, u32)> {
            let mut registro = [0u8; REGISTRO as usize];
            index
                .seek(SeekFrom::Start(i * REGISTRO))
                .and_then(|_| index.read_exact(&mut registro))
                .map_err(|_| almacenamiento("índice truncado"))?;

            let off_palabra = u32::from_le_bytes(registro[0..4].try_into().unwrap_or_default());
            let len_palabra = u32::from_le_bytes(registro[4..8].try_into().unwrap_or_default());
            let off_def = u64::from_le_bytes(registro[8..16].try_into().unwrap_or_default());
            let len_def = u32::from_le_bytes(registro[16..20].try_into().unwrap_or_default());

            let mut bytes = vec![0u8; len_palabra as usize];
            words
                .seek(SeekFrom::Start(u64::from(off_palabra)))
                .and_then(|_| words.read_exact(&mut bytes))
                .map_err(|_| almacenamiento("palabras truncadas"))?;

            Ok((
                String::from_utf8_lossy(&bytes).into_owned(),
                off_def,
                len_def,
            ))
        };

        // Primer registro cuya palabra en minúsculas no es menor que el objetivo.
        let mut bajo = 0u64;
        let mut alto = total;
        while bajo < alto {
            let medio = bajo + (alto - bajo) / 2;
            let (palabra, _, _) = leer_palabra(&mut index, &mut words, medio)?;
            if palabra.to_lowercase() < objetivo {
                bajo = medio + 1;
            } else {
                alto = medio;
            }
        }

        let mut definiciones = Vec::new();
        let mut dict = abrir(&self.carpeta.join("dict.bin"))?;
        let mut i = bajo;

        while i < total && definiciones.len() < MAXIMO_ACEPCIONES {
            let (palabra, off, len) = leer_palabra(&mut index, &mut words, i)?;
            if palabra.to_lowercase() != objetivo {
                break;
            }

            let mut bytes = vec![0u8; len as usize];
            dict.seek(SeekFrom::Start(off))
                .and_then(|_| dict.read_exact(&mut bytes))
                .map_err(|_| almacenamiento("definiciones truncadas"))?;
            definiciones.push(String::from_utf8_lossy(&bytes).into_owned());
            i += 1;
        }

        Ok(definiciones)
    }
}

fn abrir(ruta: &Path) -> NbResult<std::fs::File> {
    std::fs::File::open(ruta).map_err(|error| {
        tracing::warn!(error = %error, "falta un archivo del diccionario instalado");
        NbError::ComponentUnavailable
    })
}

impl LookupProvider for Dictionary {
    fn id(&self) -> &str {
        &self.manifest.id
    }

    fn label(&self) -> &str {
        &self.manifest.name
    }

    fn kind(&self) -> LookupKind {
        LookupKind::Dictionary
    }

    fn languages(&self) -> &[String] {
        &self.idiomas
    }

    fn requires_network(&self) -> bool {
        false
    }

    fn lookup(&self, query: &Query) -> NbResult<Vec<Entry>> {
        Ok(self
            .buscar(&query.term)?
            .into_iter()
            .map(|body| Entry {
                provider_id: self.manifest.id.clone(),
                kind: LookupKind::Dictionary,
                headword: query.term.trim().to_owned(),
                body,
                source: self.manifest.attribution.clone(),
            })
            .collect())
    }
}
