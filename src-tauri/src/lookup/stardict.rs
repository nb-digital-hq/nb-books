//! Lectura del formato StarDict.
//!
//! Contrato:
//! `docs/architecture/reader-contracts.md`, sección 5.2.
//!
//! # Por qué StarDict y no un formato propio
//!
//! Porque es el formato en el que **ya existen** los diccionarios. Lo leen
//! KOReader y GoldenDict, y en él se distribuyen las obras de licencia libre en
//! español y en inglés. Un formato propio nos obligaría a convertir cada obra a
//! mano y dejaría al usuario sin poder traer la suya, que es exactamente la
//! clase de encierro que este proyecto evita.
//!
//! # Qué es un diccionario StarDict
//!
//! Tres archivos con el mismo nombre y distinta extensión:
//!
//! * `.ifo`: metadatos en texto plano, `clave=valor`.
//! * `.idx`: índice, con la palabra terminada en NUL y su desplazamiento y
//!   tamaño en big-endian.
//! * `.dict`: las definiciones, concatenadas. Puede venir comprimido como
//!   `.dict.dz`, que es gzip válido y por tanto lo lee `flate2`, ya presente en
//!   el árbol.
//!
//! # Lo que no se admite, y por qué se rechaza al instalar y no al consultar
//!
//! Un diccionario cuyo contenido no sepamos convertir a texto plano se rechaza
//! **al instalarlo**. Aceptarlo y fallar en cada consulta sería peor: el usuario
//! creería tenerlo instalado y no entendería por qué nunca responde.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{NbError, NbResult};

/// Tipos de contenido de StarDict que sabemos convertir a texto plano.
///
/// `m`, `l` y `t` ya son texto. `g`, `x` y `h` traen marcado —Pango, XDXF y
/// HTML— del que se conserva el texto y se descarta la presentación: el contrato
/// de consulta dice que el cuerpo es texto plano, porque lo que responde un
/// diccionario es contenido de terceros y no se inyecta como marcado.
const TIPOS_ADMITIDOS: &[u8] = b"mlthxg";

/// Lo que declara el `.ifo` de un diccionario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub bookname: String,
    pub wordcount: usize,
    pub idxfilesize: usize,
    /// Idioma declarado, si lo trae. StarDict no lo exige, así que a menudo hay
    /// que pedírselo al usuario al instalar.
    pub language: Option<String>,
    /// Cuando está presente, todas las entradas son de este tipo.
    pub sametypesequence: Option<u8>,
    /// 32 por defecto. Los diccionarios grandes usan 64.
    pub idxoffsetbits: u8,
}

fn corrupto(detalle: &str) -> NbError {
    tracing::info!(detalle, "se rechazó un diccionario mal formado");
    NbError::DocumentCorrupted
}

/// Interpreta un archivo `.ifo`.
///
/// # Errors
///
/// [`NbError::DocumentCorrupted`] si no es un `.ifo`, si le faltan campos
/// obligatorios, o si declara un tipo de contenido que no sabemos convertir.
pub fn parse_info(contenido: &str) -> NbResult<Info> {
    let mut lineas = contenido.lines();

    // La primera línea es una firma fija. Sin ella no es un .ifo, y aceptarlo
    // significaría intentar interpretar cualquier archivo de texto.
    let primera = lineas.next().unwrap_or_default().trim();
    if primera != "StarDict's dict ifo file" {
        return Err(corrupto("el .ifo no lleva su firma"));
    }

    let mut campos = std::collections::BTreeMap::new();
    for linea in lineas {
        if let Some((clave, valor)) = linea.split_once('=') {
            campos.insert(clave.trim().to_owned(), valor.trim().to_owned());
        }
    }

    let obligatorio = |clave: &str| {
        campos
            .get(clave)
            .cloned()
            .ok_or_else(|| corrupto("falta un campo obligatorio del .ifo"))
    };

    let numero = |clave: &str| -> NbResult<usize> {
        obligatorio(clave)?
            .parse()
            .map_err(|_| corrupto("un campo numérico del .ifo no lo es"))
    };

    let sametypesequence = campos
        .get("sametypesequence")
        .and_then(|s| s.as_bytes().first().copied());

    if let Some(tipo) = sametypesequence {
        if !TIPOS_ADMITIDOS.contains(&tipo) {
            return Err(corrupto("tipo de contenido no admitido"));
        }
    }

    let idxoffsetbits = campos
        .get("idxoffsetbits")
        .map_or(Ok(32), |s| s.parse::<u8>())
        .map_err(|_| corrupto("idxoffsetbits no es un número"))?;

    if idxoffsetbits != 32 && idxoffsetbits != 64 {
        return Err(corrupto("idxoffsetbits solo puede ser 32 o 64"));
    }

    Ok(Info {
        bookname: obligatorio("bookname")?,
        wordcount: numero("wordcount")?,
        idxfilesize: numero("idxfilesize")?,
        language: campos
            .get("lang")
            .or_else(|| campos.get("idxlang"))
            .cloned(),
        sametypesequence,
        idxoffsetbits,
    })
}

/// Una entrada del índice: la palabra y dónde está su definición.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub word: String,
    pub offset: u64,
    pub size: u32,
}

/// Recorre un `.idx` entero.
///
/// # Errors
///
/// [`NbError::DocumentCorrupted`] si está truncado, si una palabra no es UTF-8
/// válido, o si el número de entradas no coincide con lo que declara el `.ifo`.
/// Las tres cosas son la misma: el archivo no es lo que dice ser.
pub fn parse_index(bytes: &[u8], info: &Info) -> NbResult<Vec<IndexEntry>> {
    let ancho_offset = usize::from(info.idxoffsetbits / 8);
    let mut entradas = Vec::with_capacity(info.wordcount);
    let mut i = 0usize;

    while i < bytes.len() {
        let fin_palabra = bytes[i..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| corrupto("el índice termina sin cerrar una palabra"))?
            + i;

        let palabra = std::str::from_utf8(&bytes[i..fin_palabra])
            .map_err(|_| corrupto("una palabra del índice no es UTF-8"))?
            .to_owned();

        let cursor = fin_palabra + 1;
        let fin_numeros = cursor + ancho_offset + 4;
        if fin_numeros > bytes.len() {
            return Err(corrupto("el índice está truncado"));
        }

        let offset = if ancho_offset == 8 {
            u64::from_be_bytes(
                bytes[cursor..cursor + 8]
                    .try_into()
                    .map_err(|_| corrupto("desplazamiento ilegible"))?,
            )
        } else {
            u64::from(u32::from_be_bytes(
                bytes[cursor..cursor + 4]
                    .try_into()
                    .map_err(|_| corrupto("desplazamiento ilegible"))?,
            ))
        };

        let size = u32::from_be_bytes(
            bytes[cursor + ancho_offset..fin_numeros]
                .try_into()
                .map_err(|_| corrupto("tamaño ilegible"))?,
        );

        // Una palabra vacía no se puede buscar y rompe el orden del índice.
        if palabra.is_empty() {
            return Err(corrupto("el índice trae una palabra vacía"));
        }

        entradas.push(IndexEntry {
            word: palabra,
            offset,
            size,
        });
        i = fin_numeros;
    }

    if entradas.len() != info.wordcount {
        return Err(corrupto(
            "el índice no tiene las entradas que declara el .ifo",
        ));
    }

    Ok(entradas)
}

/// Convierte el cuerpo de una definición a texto plano.
///
/// Se descarta el marcado, no el texto: `<b>casa</b>` da `casa`. La alternativa
/// era conservarlo y sanearlo después, pero el contrato de consulta dice que el
/// cuerpo es texto plano precisamente para que no haya un "después" en el que
/// alguien se olvide.
#[must_use]
pub fn to_plain_text(bytes: &[u8], tipo: u8) -> String {
    let texto = String::from_utf8_lossy(bytes);
    match tipo {
        b'g' | b'x' | b'h' => quitar_marcado(&texto),
        _ => texto.into_owned(),
    }
}

fn quitar_marcado(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut dentro = false;
    for c in texto.chars() {
        match c {
            '<' => dentro = true,
            '>' => dentro = false,
            _ if !dentro => salida.push(c),
            _ => {}
        }
    }
    salida.trim().to_owned()
}

/// Los tres archivos de un diccionario, ya localizados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    pub ifo: PathBuf,
    pub idx: PathBuf,
    /// El `.dict`, comprimido o no.
    pub dict: PathBuf,
    pub dict_comprimido: bool,
    pub idx_comprimido: bool,
}

/// Localiza los tres archivos a partir del `.ifo`.
///
/// # Errors
///
/// [`NbError::FileNotFound`] si falta el índice o las definiciones. Un
/// diccionario incompleto se rechaza aquí y no se instala a medias.
pub fn locate(ifo: &Path) -> NbResult<Files> {
    let base = ifo.with_extension("");
    let existe = |ruta: &PathBuf| ruta.is_file().then(|| ruta.clone());

    let (idx, idx_comprimido) = existe(&base.with_extension("idx"))
        .map(|r| (r, false))
        .or_else(|| existe(&base.with_extension("idx.gz")).map(|r| (r, true)))
        .ok_or_else(|| {
            tracing::info!("el diccionario no trae índice");
            NbError::FileNotFound
        })?;

    let (dict, dict_comprimido) = existe(&base.with_extension("dict"))
        .map(|r| (r, false))
        .or_else(|| existe(&base.with_extension("dict.dz")).map(|r| (r, true)))
        .ok_or_else(|| {
            tracing::info!("el diccionario no trae definiciones");
            NbError::FileNotFound
        })?;

    Ok(Files {
        ifo: ifo.to_path_buf(),
        idx,
        dict,
        dict_comprimido,
        idx_comprimido,
    })
}

/// Lee un archivo, descomprimiéndolo si viene en gzip.
///
/// `dictzip` es gzip válido con un campo extra para acceso aleatorio, así que un
/// descompresor de gzip lo lee entero sin saber nada de él. Aquí se lee entero
/// **una sola vez, al instalar**: en consulta se lee del archivo ya normalizado.
///
/// # Errors
///
/// [`NbError::DocumentCorrupted`] si el archivo no se deja leer o descomprimir.
pub fn read_maybe_gzip(ruta: &Path, comprimido: bool) -> NbResult<Vec<u8>> {
    let bytes = std::fs::read(ruta).map_err(|error| {
        tracing::info!(error = %error, "no se pudo leer un archivo del diccionario");
        NbError::DocumentCorrupted
    })?;

    if !comprimido {
        return Ok(bytes);
    }

    let mut salida = Vec::new();
    flate2::read::GzDecoder::new(&bytes[..])
        .read_to_end(&mut salida)
        .map_err(|error| {
            tracing::info!(error = %error, "el diccionario comprimido no se deja descomprimir");
            NbError::DocumentCorrupted
        })?;
    Ok(salida)
}
