//! La firma del autor de una extensión.
//!
//! Mecanismo del requisito **R13** de la política del registro.
//!
//! # Qué firma, exactamente
//!
//! La **huella del paquete**: el SHA-256 de las rutas y el contenido de todos
//! sus archivos, que es la misma dirección con la que el almacén guarda la
//! extensión. Firmar la huella firma cada archivo y cada ruta a la
//! vez, así que no hay ninguna parte del paquete que quede fuera.
//!
//! La firma no entra en la huella, y no puede entrar: se firma **sobre** ella.
//! Eso tiene un efecto que conviene saber: un paquete tiene la misma dirección
//! de contenido antes y después de firmarse, y volver a firmarlo no lo mueve de
//! sitio en el almacén.
//!
//! # Qué más entra en lo firmado, y por qué
//!
//! El identificador y la versión, además de la huella, y todo con un prefijo
//! que dice para qué es esta firma. Sin el prefijo, una firma hecha para otra
//! cosa con la misma clave podría presentarse aquí; sin el identificador y la
//! versión, una firma legítima de un paquete podría moverse a otro que tuviera
//! el mismo contenido con otro nombre.
//!
//! # De dónde sale la confianza en la clave
//!
//! **No de la firma.** Una firma válida solo dice que quien firmó tenía la
//! clave que viene al lado; cualquiera puede generar un par de claves y firmar
//! lo que quiera. Lo que protege al usuario es que la clave sea **la misma de
//! siempre para esa extensión**: el almacén apunta la clave con la que se
//! instaló, y una actualización firmada por otra —o sin firmar— no entra. Es lo
//! que convierte el vector clásico de la política, la extensión benigna que se
//! vuelve maliciosa en una actualización, en algo que hace falta robarle la
//! clave al autor para conseguir.
//!
//! Cuando exista el registro, la clave se publicará también ahí, y entonces la
//! primera instalación dejará de ser un acto de fe. Hoy no existe, así que se
//! dice en lugar de fingir que la primera vez ya está protegida.

use std::path::Path;

use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// El archivo de firma, dentro del paquete.
pub const ARCHIVO: &str = "firma.json";

/// El único algoritmo admitido.
///
/// Uno solo, y declarado en el archivo: si algún día hay dos, un paquete tendrá
/// que decir cuál usa, y un lector viejo sabrá que no lo entiende en lugar de
/// leer mal la firma.
pub const ALGORITMO: &str = "ed25519";

/// Lo que distingue a esta firma de cualquier otra hecha con la misma clave.
const PROPOSITO: &str = "nb-books:extension:1";

/// El archivo `firma.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Firma {
    pub algoritmo: String,
    /// Clave pública, en hexadecimal. 32 bytes.
    pub clave: String,
    /// La firma, en hexadecimal. 64 bytes.
    pub firma: String,
}

/// En qué estado está la firma de un paquete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
#[serde(tag = "estado", rename_all = "camelCase")]
pub enum SignatureState {
    /// No trae firma. No es lo mismo que traerla mal: instalar algo sin firmar
    /// desde el disco propio es legítimo, y actualizar con algo sin firmar lo
    /// que estaba firmado, no.
    SinFirma,
    /// Firma correcta. Lleva la clave para poder fijarla y compararla después.
    Valida { clave: String },
    /// Trae firma y no vale. El motivo va aparte porque no es lo mismo un
    /// archivo mal escrito que una firma que no corresponde al contenido.
    Invalida { motivo: SignatureFault },
}

/// Por qué no vale una firma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
#[serde(rename_all = "camelCase")]
pub enum SignatureFault {
    /// El archivo no es JSON, o le faltan campos.
    ArchivoIlegible,
    /// Declara un algoritmo que no es el que se admite.
    AlgoritmoDesconocido,
    /// La clave o la firma no son hexadecimal del largo que toca.
    FormatoIlegible,
    /// Todo está bien escrito y la firma **no corresponde** a este paquete.
    NoCorresponde,
}

/// El mensaje que se firma.
fn mensaje(id: &str, version: &str, huella: &str) -> Vec<u8> {
    format!("{PROPOSITO}\n{id}\n{version}\n{huella}").into_bytes()
}

/// Comprueba la firma de un paquete que está en el disco.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede calcular la huella del paquete.
pub fn verificar(paquete: &Path, id: &str, version: &str) -> NbResult<SignatureState> {
    let ruta = paquete.join(ARCHIVO);
    if !ruta.exists() {
        return Ok(SignatureState::SinFirma);
    }
    let huella = super::store::huella_del_directorio(paquete)?;
    let Ok(texto) = std::fs::read_to_string(&ruta) else {
        return Ok(SignatureState::Invalida {
            motivo: SignatureFault::ArchivoIlegible,
        });
    };
    Ok(comprobar(&texto, id, version, &huella))
}

/// La comprobación sin disco, que es lo que se puede probar sin montar nada.
#[must_use]
pub fn comprobar(texto: &str, id: &str, version: &str, huella: &str) -> SignatureState {
    let Ok(firma) = serde_json::from_str::<Firma>(texto) else {
        return SignatureState::Invalida {
            motivo: SignatureFault::ArchivoIlegible,
        };
    };
    if firma.algoritmo != ALGORITMO {
        return SignatureState::Invalida {
            motivo: SignatureFault::AlgoritmoDesconocido,
        };
    }
    let (Some(clave), Some(bytes)) = (de_hex(&firma.clave, 32), de_hex(&firma.firma, 64)) else {
        return SignatureState::Invalida {
            motivo: SignatureFault::FormatoIlegible,
        };
    };
    match UnparsedPublicKey::new(&ED25519, &clave).verify(&mensaje(id, version, huella), &bytes) {
        Ok(()) => SignatureState::Valida { clave: firma.clave },
        Err(_) => SignatureState::Invalida {
            motivo: SignatureFault::NoCorresponde,
        },
    }
}

/// Firma un paquete del disco con una clave en formato PKCS#8.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si no se puede leer el paquete.
/// - [`NbError::FormatUnsupported`] si la clave no es un par de Ed25519.
pub fn firmar(paquete: &Path, id: &str, version: &str, pkcs8: &[u8]) -> NbResult<Firma> {
    let huella = super::store::huella_del_directorio(paquete)?;
    let par = Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| NbError::FormatUnsupported)?;
    let firma = par.sign(&mensaje(id, version, huella.as_str()));
    Ok(Firma {
        algoritmo: ALGORITMO.to_owned(),
        clave: a_hex(par.public_key().as_ref()),
        firma: a_hex(firma.as_ref()),
    })
}

/// Genera un par de claves nuevo, en PKCS#8.
///
/// # Errors
///
/// [`NbError::Internal`] si el sistema no da aleatoriedad, que es un fallo del
/// sistema y no del autor.
pub fn crear_clave() -> NbResult<Vec<u8>> {
    let azar = SystemRandom::new();
    let documento = Ed25519KeyPair::generate_pkcs8(&azar).map_err(|_| NbError::Internal)?;
    Ok(documento.as_ref().to_vec())
}

/// La clave pública de un par en PKCS#8, en hexadecimal.
///
/// # Errors
///
/// [`NbError::FormatUnsupported`] si el archivo no es un par de Ed25519.
pub fn clave_publica(pkcs8: &[u8]) -> NbResult<String> {
    let par = Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| NbError::FormatUnsupported)?;
    Ok(a_hex(par.public_key().as_ref()))
}

fn a_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut salida, byte| {
            use std::fmt::Write as _;
            let _ = write!(salida, "{byte:02x}");
            salida
        },
    )
}

/// Hexadecimal a bytes, exigiendo el largo exacto.
///
/// El largo se comprueba aquí y no después: una clave de otro tamaño no es una
/// clave con un problema, es otra cosa.
fn de_hex(texto: &str, largo: usize) -> Option<Vec<u8>> {
    if texto.len() != largo * 2 {
        return None;
    }
    (0..largo)
        .map(|i| u8::from_str_radix(texto.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}
