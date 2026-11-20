//! La procedencia de lo que se publica: sumas, firma y su comprobación.
//!
//! Cierra la parte de Linux, y la sección 5 de
//! `docs/audit/supply-chain.md`.
//!
//! # Qué se firma, exactamente
//!
//! **Un solo archivo: `SHA256SUMS`**, que lista la huella de todo lo demás que
//! va en la publicación -los instaladores, el listado de componentes, los avisos
//! de terceros-. Firmar la lista firma cada archivo a la vez, y deja una sola
//! firma que comprobar en lugar de una por artefacto.
//!
//! # Por qué la firma es cruda y no un archivo propio
//!
//! Porque el criterio es que **un tercero pueda verificar siguiendo
//! las instrucciones publicadas**, y un tercero no tiene por qué compilar este
//! proyecto para fiarse de él. La firma son los 64 bytes de Ed25519 sobre los
//! bytes exactos de `SHA256SUMS`, y la clave pública se publica en PEM: con eso
//! basta `sha256sum -c` y `openssl pkeyutl -verify`, que ya están en cualquier
//! sistema. La herramienta de aquí hace lo mismo y además mira lo que esas dos
//! no miran: que en la carpeta no haya nada **sin listar**.
//!
//! Se descartó firmar con prefijo de propósito, como hace la firma de
//! extensiones: obligaría al tercero a reconstruir el mensaje a mano, que es
//! justo el paso en que uno se equivoca y acaba dando por bueno lo que no
//! comprobó. La separación la da el propio documento -una lista de sumas no es
//! un manifiesto de extensión ni uno de actualización- y la clave, que es otra:
//! la de publicación no firma extensiones.
//!
//! # De dónde sale la confianza en la clave
//!
//! **No de la carpeta publicada.** Quien pueda cambiar los artefactos puede
//! cambiar una clave que viaje con ellos. Por eso la comprobación recibe la
//! clave de fuera -la del repositorio- y aquí no hay ninguna función que la lea
//! de al lado de lo que comprueba.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use sha2::{Digest, Sha256};

use crate::error::{NbError, NbResult};

/// La lista de huellas. El nombre es el que espera `sha256sum -c`.
pub const SUMAS: &str = "SHA256SUMS";

/// La firma de esa lista: 64 bytes, sin envoltorio.
pub const FIRMA: &str = "SHA256SUMS.sig";

/// Lo que antecede a los 32 bytes de una clave pública de Ed25519 en su forma
/// estándar (`SubjectPublicKeyInfo`, RFC 8410). Es fijo: el algoritmo no tiene
/// parámetros.
const CABECERA_SPKI: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// Lo que una comprobación encuentra mal. Vacío es que todo cuadra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Defecto {
    /// No hay `SHA256SUMS` en la carpeta.
    SinSumas,
    /// No hay `SHA256SUMS.sig`.
    SinFirma,
    /// La clave que se dio no es una clave pública de Ed25519 en PEM.
    ClaveIlegible,
    /// La firma no es de esa clave sobre esa lista.
    FirmaNoCorresponde,
    /// Una línea de la lista no es `huella  nombre`.
    LineaIlegible { linea: usize },
    /// La lista nombra un archivo que no está.
    Falta { nombre: String },
    /// El archivo está y su contenido no es el que la lista dice.
    Alterado { nombre: String },
    /// El archivo está en la carpeta y la lista no lo nombra: nadie lo firmó.
    SinListar { nombre: String },
}

impl std::fmt::Display for Defecto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SinSumas => write!(f, "falta {SUMAS}"),
            Self::SinFirma => write!(f, "falta {FIRMA}"),
            Self::ClaveIlegible => write!(f, "la clave pública no es de Ed25519 en PEM"),
            Self::FirmaNoCorresponde => {
                write!(f, "la firma no corresponde a {SUMAS} con esa clave")
            }
            Self::LineaIlegible { linea } => write!(f, "la línea {linea} de {SUMAS} no se lee"),
            Self::Falta { nombre } => write!(f, "{nombre} está en la lista y no en la carpeta"),
            Self::Alterado { nombre } => write!(f, "{nombre} no es el archivo que se firmó"),
            Self::SinListar { nombre } => {
                write!(f, "{nombre} está en la carpeta y nadie lo firmó")
            }
        }
    }
}

/// La huella SHA-256 de un archivo, leída a trozos: un instalador pesa más de
/// cien megas y no hace falta tenerlo entero en memoria para sumarlo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si el archivo no se puede leer.
pub fn huella_de(ruta: &Path) -> NbResult<String> {
    let mut archivo = std::fs::File::open(ruta).map_err(|_| NbError::StorageFailure)?;
    let mut hasher = Sha256::new();
    let mut trozo = vec![0_u8; 1 << 16];
    loop {
        let leidos = archivo
            .read(&mut trozo)
            .map_err(|_| NbError::StorageFailure)?;
        if leidos == 0 {
            break;
        }
        hasher.update(&trozo[..leidos]);
    }
    Ok(a_hex(&hasher.finalize()))
}

/// Los archivos de la carpeta que entran en la lista, ordenados.
///
/// Solo archivos, y solo los de primer nivel: una publicación es una carpeta
/// plana, y una subcarpeta ahí es un error de quien la preparó, no algo que
/// firmar en silencio.
fn publicados(carpeta: &Path) -> NbResult<BTreeSet<String>> {
    let mut nombres = BTreeSet::new();
    for entrada in std::fs::read_dir(carpeta).map_err(|_| NbError::StorageFailure)? {
        let entrada = entrada.map_err(|_| NbError::StorageFailure)?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if nombre == SUMAS || nombre == FIRMA {
            continue;
        }
        let tipo = entrada.file_type().map_err(|_| NbError::StorageFailure)?;
        // Un nombre con salto de línea rompería el formato de la lista, y uno
        // con barra invertida lo escapa `sha256sum` de una forma que aquí no se
        // reproduce: se rechazan en lugar de firmarlos mal.
        if !tipo.is_file() || nombre.contains(['\n', '\r', '\\']) {
            return Err(NbError::FormatUnsupported);
        }
        nombres.insert(nombre);
    }
    Ok(nombres)
}

/// El texto de `SHA256SUMS` para una carpeta: una línea por archivo, en el
/// formato de `sha256sum` en modo binario.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si la carpeta o un archivo no se pueden leer.
/// - [`NbError::FormatUnsupported`] si hay algo que no es un archivo, o un
///   nombre que el formato no puede llevar.
pub fn sumar(carpeta: &Path) -> NbResult<String> {
    let mut texto = String::new();
    for nombre in publicados(carpeta)? {
        let huella = huella_de(&carpeta.join(&nombre))?;
        texto.push_str(&huella);
        texto.push_str(" *");
        texto.push_str(&nombre);
        texto.push('\n');
    }
    Ok(texto)
}

/// Firma los bytes de la lista con un par de claves en PKCS#8.
///
/// # Errors
///
/// [`NbError::FormatUnsupported`] si la clave no es un par de Ed25519.
pub fn firmar(sumas: &[u8], pkcs8: &[u8]) -> NbResult<Vec<u8>> {
    let par = Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| NbError::FormatUnsupported)?;
    Ok(par.sign(sumas).as_ref().to_vec())
}

/// La clave pública de un par, en PEM, que es como la lee `openssl`.
///
/// # Errors
///
/// [`NbError::FormatUnsupported`] si la clave no es un par de Ed25519.
pub fn pem_publica(pkcs8: &[u8]) -> NbResult<String> {
    let par = Ed25519KeyPair::from_pkcs8(pkcs8).map_err(|_| NbError::FormatUnsupported)?;
    let mut der = CABECERA_SPKI.to_vec();
    der.extend_from_slice(par.public_key().as_ref());
    Ok(format!(
        "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----\n",
        a_base64(&der)
    ))
}

/// Los 32 bytes de una clave pública en PEM, o nada si no es de Ed25519.
fn clave_de_pem(pem: &str) -> Option<Vec<u8>> {
    let cuerpo: String = pem
        .lines()
        .map(str::trim)
        .filter(|linea| !linea.starts_with("-----"))
        .collect();
    let der = de_base64(&cuerpo)?;
    let clave = der.strip_prefix(&CABECERA_SPKI)?;
    (clave.len() == 32).then(|| clave.to_vec())
}

/// Comprueba una carpeta publicada contra una clave que viene de fuera.
///
/// Devuelve **todo** lo que encuentra y no solo lo primero: quien verifica una
/// publicación quiere saber qué archivos no valen, no ir descubriéndolos de
/// uno en uno. Si la firma no corresponde, las huellas se comprueban igual,
/// pero no dicen nada: la lista misma puede ser de otro.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la carpeta no se puede leer.
pub fn comprobar(carpeta: &Path, clave_pem: &str) -> NbResult<Vec<Defecto>> {
    let mut defectos = Vec::new();

    let Ok(sumas) = std::fs::read(carpeta.join(SUMAS)) else {
        return Ok(vec![Defecto::SinSumas]);
    };
    match (std::fs::read(carpeta.join(FIRMA)), clave_de_pem(clave_pem)) {
        (Err(_), _) => defectos.push(Defecto::SinFirma),
        (_, None) => defectos.push(Defecto::ClaveIlegible),
        (Ok(firma), Some(clave)) => {
            if UnparsedPublicKey::new(&ED25519, &clave)
                .verify(&sumas, &firma)
                .is_err()
            {
                defectos.push(Defecto::FirmaNoCorresponde);
            }
        }
    }

    let mut listados = BTreeSet::new();
    for (indice, linea) in String::from_utf8_lossy(&sumas).lines().enumerate() {
        let Some((huella, nombre)) = partir(linea) else {
            defectos.push(Defecto::LineaIlegible { linea: indice + 1 });
            continue;
        };
        listados.insert(nombre.to_owned());
        // El nombre sale de la lista, que es de fuera: no puede sacar la
        // comprobación de la carpeta.
        let ruta = carpeta.join(nombre);
        if nombre.contains(['/', '\\']) || nombre == ".." || !ruta.is_file() {
            defectos.push(Defecto::Falta {
                nombre: nombre.to_owned(),
            });
        } else if huella_de(&ruta)? != huella {
            defectos.push(Defecto::Alterado {
                nombre: nombre.to_owned(),
            });
        }
    }

    // `read_dir` y no `publicados`: aquí una subcarpeta o un nombre raro no es
    // un error de quien llama, es un hallazgo.
    let mut presentes = BTreeSet::new();
    for entrada in std::fs::read_dir(carpeta).map_err(|_| NbError::StorageFailure)? {
        let entrada = entrada.map_err(|_| NbError::StorageFailure)?;
        presentes.insert(entrada.file_name().to_string_lossy().into_owned());
    }
    for nombre in presentes {
        if nombre != SUMAS && nombre != FIRMA && !listados.contains(&nombre) {
            defectos.push(Defecto::SinListar { nombre });
        }
    }

    Ok(defectos)
}

/// Parte una línea de `sha256sum`: 64 hexadecimales, un espacio, la marca de
/// modo -espacio o asterisco- y el nombre.
/// Si `firma` es la firma de Ed25519 de `clave_pem` sobre los bytes exactos de
/// `sumas`. Es la misma comprobación que [`comprobar`], para quien tiene la lista
/// en memoria y no en una carpeta: el actualizador.
#[must_use]
pub fn firma_valida(sumas: &[u8], firma: &[u8], clave_pem: &str) -> bool {
    clave_de_pem(clave_pem).is_some_and(|clave| {
        UnparsedPublicKey::new(&ED25519, &clave)
            .verify(sumas, firma)
            .is_ok()
    })
}

/// La huella que la lista da para `nombre`, si lo nombra.
#[must_use]
pub fn huella_listada(sumas: &[u8], nombre: &str) -> Option<String> {
    String::from_utf8_lossy(sumas)
        .lines()
        .filter_map(partir)
        .find(|(_, n)| *n == nombre)
        .map(|(huella, _)| huella.to_owned())
}

/// Los nombres que lista `SHA256SUMS`, en su orden.
#[must_use]
pub fn nombres_listados(sumas: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(sumas)
        .lines()
        .filter_map(partir)
        .map(|(_, nombre)| nombre.to_owned())
        .collect()
}

/// La huella SHA-256 de unos bytes, en hexadecimal.
#[must_use]
pub fn huella_de_bytes(bytes: &[u8]) -> String {
    a_hex(&Sha256::digest(bytes))
}

fn partir(linea: &str) -> Option<(&str, &str)> {
    let huella = linea.get(..64)?;
    let resto = linea.get(64..)?;
    let nombre = resto
        .strip_prefix(" *")
        .or_else(|| resto.strip_prefix("  "))?;
    let valida = huella
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (valida && !nombre.is_empty()).then_some((huella, nombre))
}

fn a_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut salida, byte| {
            let _ = write!(salida, "{byte:02x}");
            salida
        },
    )
}

const ALFABETO: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64 estándar con relleno. A mano y no con una dependencia: son cuarenta
/// y cuatro bytes de una clave, y traer un paquete para eso es justo lo que la
/// política de dependencias pide no hacer.
fn a_base64(bytes: &[u8]) -> String {
    let mut salida = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for trio in bytes.chunks(3) {
        let n = u32::from(trio[0]) << 16
            | u32::from(*trio.get(1).unwrap_or(&0)) << 8
            | u32::from(*trio.get(2).unwrap_or(&0));
        for (posicion, desplazamiento) in [18_u32, 12, 6, 0].into_iter().enumerate() {
            if posicion <= trio.len() {
                salida.push(char::from(ALFABETO[((n >> desplazamiento) & 63) as usize]));
            } else {
                salida.push('=');
            }
        }
    }
    salida
}

fn de_base64(texto: &str) -> Option<Vec<u8>> {
    let limpio = texto.trim_end_matches('=');
    let mut salida = Vec::with_capacity(limpio.len() * 3 / 4);
    let mut acumulado = 0_u32;
    let mut bits = 0_u32;
    for byte in limpio.bytes() {
        let valor = ALFABETO.iter().position(|&c| c == byte)?;
        acumulado = (acumulado << 6) | u32::try_from(valor).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            salida.push(u8::try_from((acumulado >> bits) & 0xff).ok()?);
        }
    }
    Some(salida)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extensions::signature::crear_clave;

    /// Una publicación de mentira, firmada, con su clave pública.
    fn publicada(nombre: &str) -> (std::path::PathBuf, String) {
        let carpeta = crate::paths::temporal_de_pruebas()
            .join(format!("nb-artefactos-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&carpeta);
        std::fs::create_dir_all(&carpeta).expect("carpeta");
        std::fs::write(carpeta.join("nb-books_0.1.1_amd64.deb"), b"un instalador").expect("deb");
        std::fs::write(carpeta.join("nb-books_1.0.0.cdx.json"), b"{}").expect("sbom");

        let clave = crear_clave().expect("clave");
        let sumas = sumar(&carpeta).expect("sumas");
        std::fs::write(carpeta.join(SUMAS), &sumas).expect("lista");
        std::fs::write(
            carpeta.join(FIRMA),
            firmar(sumas.as_bytes(), &clave).expect("firma"),
        )
        .expect("archivo de firma");
        (carpeta, pem_publica(&clave).expect("pem"))
    }

    #[test]
    fn una_publicacion_intacta_no_tiene_defectos() {
        let (carpeta, pem) = publicada("intacta");
        assert_eq!(comprobar(&carpeta, &pem).expect("comprobación"), vec![]);
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn un_artefacto_cambiado_se_nombra() {
        let (carpeta, pem) = publicada("alterado");
        std::fs::write(carpeta.join("nb-books_0.1.1_amd64.deb"), b"otro instalador")
            .expect("cambio");
        assert_eq!(
            comprobar(&carpeta, &pem).expect("comprobación"),
            vec![Defecto::Alterado {
                nombre: "nb-books_0.1.1_amd64.deb".to_owned()
            }]
        );
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn rehacer_la_lista_tras_cambiar_un_artefacto_rompe_la_firma() {
        // Lo que haría quien cambia un instalador y sabe que hay sumas: las
        // recalcula. Las huellas vuelven a cuadrar y lo único que lo delata es
        // la firma, que es para lo que está.
        let (carpeta, pem) = publicada("resumado");
        std::fs::write(carpeta.join("nb-books_0.1.1_amd64.deb"), b"otro instalador")
            .expect("cambio");
        std::fs::write(carpeta.join(SUMAS), sumar(&carpeta).expect("sumas")).expect("lista");
        assert_eq!(
            comprobar(&carpeta, &pem).expect("comprobación"),
            vec![Defecto::FirmaNoCorresponde]
        );
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn la_firma_de_otra_clave_no_vale_aunque_sea_valida() {
        let (carpeta, _) = publicada("otra-clave");
        let otra = pem_publica(&crear_clave().expect("clave")).expect("pem");
        assert_eq!(
            comprobar(&carpeta, &otra).expect("comprobación"),
            vec![Defecto::FirmaNoCorresponde]
        );
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn un_archivo_que_nadie_firmo_se_dice_y_uno_que_falta_tambien() {
        let (carpeta, pem) = publicada("de-mas-y-de-menos");
        std::fs::write(carpeta.join("extra.AppImage"), b"colado").expect("extra");
        std::fs::remove_file(carpeta.join("nb-books_1.0.0.cdx.json")).expect("borrado");
        assert_eq!(
            comprobar(&carpeta, &pem).expect("comprobación"),
            vec![
                Defecto::Falta {
                    nombre: "nb-books_1.0.0.cdx.json".to_owned()
                },
                Defecto::SinListar {
                    nombre: "extra.AppImage".to_owned()
                },
            ]
        );
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn sin_firma_o_con_una_clave_que_no_lo_es_no_se_da_por_buena() {
        let (carpeta, pem) = publicada("sin-firma");
        assert_eq!(
            comprobar(&carpeta, "no es una clave").expect("comprobación"),
            vec![Defecto::ClaveIlegible]
        );
        std::fs::remove_file(carpeta.join(FIRMA)).expect("borrado");
        assert_eq!(
            comprobar(&carpeta, &pem).expect("comprobación"),
            vec![Defecto::SinFirma]
        );
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn la_lista_no_puede_sacar_la_comprobacion_de_la_carpeta() {
        let (carpeta, pem) = publicada("fuera");
        let huella = "0".repeat(64);
        std::fs::write(carpeta.join(SUMAS), format!("{huella} *../fuera\n")).expect("lista");
        let defectos = comprobar(&carpeta, &pem).expect("comprobación");
        assert!(defectos.contains(&Defecto::Falta {
            nombre: "../fuera".to_owned()
        }));
        let _ = std::fs::remove_dir_all(&carpeta);
    }

    #[test]
    fn el_pem_va_y_vuelve_con_la_misma_clave() {
        let clave = crear_clave().expect("clave");
        let par = Ed25519KeyPair::from_pkcs8(&clave).expect("par");
        let pem = pem_publica(&clave).expect("pem");
        assert_eq!(
            clave_de_pem(&pem).expect("clave"),
            par.public_key().as_ref()
        );
    }

    #[test]
    fn las_lineas_de_sha256sum_se_leen_en_sus_dos_modos() {
        let huella = "a".repeat(64);
        assert_eq!(
            partir(&format!("{huella} *uno.deb")),
            Some((huella.as_str(), "uno.deb"))
        );
        assert_eq!(
            partir(&format!("{huella}  dos con espacios.rpm")),
            Some((huella.as_str(), "dos con espacios.rpm"))
        );
        assert_eq!(partir(&format!("{huella} uno.deb")), None);
        assert_eq!(partir("corta  uno.deb"), None);
    }
}
