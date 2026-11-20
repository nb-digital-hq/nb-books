//! TLS del servidor móvil: la identidad y el oyente.
//!
//! El teléfono fija la huella del certificado la primera vez (TOFU) y la exige en
//! cada conexión. Por eso la identidad nunca se regenera si existe: la que crea
//! una instalación se guarda para siempre, con una validez que no va a vencer.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::rustls::crypto::ring::default_provider;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;

use crate::error::{NbError, NbResult};

/// Los nombres de los archivos de la identidad, dentro de la carpeta del servidor.
pub const CERTIFICADO: &str = "certificado.pem";
pub const CLAVE: &str = "clave.pem";

/// Base64 estándar, sin dependencias: solo hace falta para leer el PEM del
/// certificado.
fn base64(texto: &str) -> Option<Vec<u8>> {
    let valor = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut salida = Vec::with_capacity(texto.len() * 3 / 4);
    let (mut acumulado, mut bits) = (0u32, 0u32);
    for c in texto.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        acumulado = (acumulado << 6) | valor(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            salida.push((acumulado >> bits) as u8);
            acumulado &= (1 << bits) - 1;
        }
    }
    Some(salida)
}

/// La huella que fija el móvil: SHA-256 del certificado en DER, en hexadecimal
/// minúsculo y sin separadores.
fn huella_sha256(pem: &str) -> Option<String> {
    let inicio = pem.find("-----BEGIN CERTIFICATE-----")? + "-----BEGIN CERTIFICATE-----".len();
    let fin = pem[inicio..].find("-----END CERTIFICATE-----")? + inicio;
    let der = base64(&pem[inicio..fin]).filter(|d| !d.is_empty())?;
    Some(
        Sha256::digest(&der)
            .iter()
            .fold(String::with_capacity(64), |mut s, b| {
                use std::fmt::Write as _;
                let _ = write!(s, "{b:02x}");
                s
            }),
    )
}

/// Escribe una clave privada: se crea ya con `0600` y se renombra al terminar,
/// así que nunca hay una a medias ni legible por otros.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
fn escribir_privado(destino: &Path, contenido: &[u8]) -> NbResult<()> {
    let temporal: PathBuf = destino.with_extension("pem.parcial");
    let mut opciones = std::fs::OpenOptions::new();
    opciones.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opciones.mode(0o600);
    }
    let escrito = opciones.open(&temporal).and_then(|mut f| {
        use std::io::Write;
        f.write_all(contenido)?;
        f.sync_all()
    });
    let colocado = escrito.and_then(|()| std::fs::rename(&temporal, destino));
    colocado.map_err(|error| {
        let _ = std::fs::remove_file(&temporal);
        tracing::error!(error = %error, "no se pudo escribir la clave del servidor móvil");
        NbError::StorageFailure
    })
}

/// Un cliente que abre la conexión y no termina el saludo no puede retener el
/// hilo que acepta a los demás.
const PLAZO_SALUDO: Duration = Duration::from_secs(10);

/// Crea una identidad nueva en `dir`. ECDSA P-256: la admiten todos los
/// teléfonos y su firma es corta.
fn crear(dir: &Path) -> NbResult<()> {
    let mut params = rcgen::CertificateParams::new(vec!["nbbooks.local".to_owned()])
        .map_err(|_| NbError::Internal)?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "NB Books");
    params.not_before = rcgen::date_time_ymd(2026, 1, 1);
    // El teléfono valida la huella, no la fecha; pero un certificado vencido
    // obligaría a regenerarlo, y eso sí rompería los emparejamientos.
    params.not_after = rcgen::date_time_ymd(2125, 1, 1);
    let clave = rcgen::KeyPair::generate().map_err(|_| NbError::Internal)?;
    let cert = params.self_signed(&clave).map_err(|_| NbError::Internal)?;
    std::fs::create_dir_all(dir).map_err(|_| NbError::StorageFailure)?;
    escribir_privado(&dir.join(CLAVE), clave.serialize_pem().as_bytes())?;
    std::fs::write(dir.join(CERTIFICADO), cert.pem()).map_err(|_| NbError::StorageFailure)?;
    tracing::info!("identidad nueva del servidor móvil");
    Ok(())
}

/// La configuración TLS y la huella que se pone en el QR. Crea la identidad si no
/// la hay.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si no se puede leer o crear.
/// - [`NbError::DocumentCorrupted`] si el certificado o la clave no son válidos.
pub fn cargar(dir: &Path) -> NbResult<(Arc<ServerConfig>, String)> {
    let (cert, clave) = (dir.join(CERTIFICADO), dir.join(CLAVE));
    if !(cert.is_file() && clave.is_file()) {
        crear(dir)?;
    }
    let pem = std::fs::read(&cert).map_err(|_| NbError::StorageFailure)?;
    let huella = huella_sha256(&String::from_utf8_lossy(&pem)).ok_or(NbError::DocumentCorrupted)?;
    let certs = CertificateDer::pem_slice_iter(&pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| NbError::DocumentCorrupted)?;
    let clave = PrivateKeyDer::from_pem_file(&clave).map_err(|_| NbError::DocumentCorrupted)?;
    let mut config = ServerConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|_| NbError::Internal)?
        .with_no_client_auth()
        .with_single_cert(certs, clave)
        .map_err(|_| NbError::DocumentCorrupted)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok((Arc::new(config), huella))
}

/// Acepta conexiones y hace el saludo TLS de cada una en su propia tarea, para que
/// un cliente lento no detenga a los demás; axum recibe solo las que terminaron.
pub struct OyenteTls {
    listas: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    direccion: SocketAddr,
}

impl OyenteTls {
    pub fn nuevo(tcp: TcpListener, config: Arc<ServerConfig>) -> std::io::Result<Self> {
        let direccion = tcp.local_addr()?;
        let (tx, listas) = mpsc::channel(64);
        let aceptor = TlsAcceptor::from(config);
        tokio::spawn(async move {
            loop {
                let (socket, origen) = match tcp.accept().await {
                    Ok(c) => c,
                    Err(_) => {
                        // Sin descriptores libres, por ejemplo: se espera y se sigue.
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let (aceptor, enviar) = (aceptor.clone(), tx.clone());
                tokio::spawn(async move {
                    if let Ok(Ok(tls)) =
                        tokio::time::timeout(PLAZO_SALUDO, aceptor.accept(socket)).await
                    {
                        let _ = enviar.send((tls, origen)).await;
                    }
                });
                if tx.is_closed() {
                    return;
                }
            }
        });
        Ok(Self { listas, direccion })
    }
}

impl axum::serve::Listener for OyenteTls {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.listas.recv().await {
            Some(c) => c,
            // El aceptador terminó: no habrá más conexiones.
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(self.direccion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_identidad_creada_no_cambia_entre_arranques() {
        let dir = std::env::temp_dir().join(format!("nb-tls-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (_, a) = cargar(&dir).unwrap();
        let (_, b) = cargar(&dir).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(dir.join(CLAVE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(modo & 0o777, 0o600, "la clave no la lee nadie más");
        }
    }

    /// Generado con `openssl req -x509 -newkey ec`; la huella es la que da
    /// `openssl x509 -outform DER | sha256sum`, que es la que fija el móvil.
    const PEM: &str = "-----BEGIN CERTIFICATE-----
MIIBdjCCAR2gAwIBAgIUZPZsZ9VsrYuKotahhnllZVXw8TkwCgYIKoZIzj0EAwIw
ETEPMA0GA1UEAwwGcHJ1ZWJhMB4XDTI2MTAzMDA2MzE1OFoXDTI2MTAzMTA2MzE1
OFowETEPMA0GA1UEAwwGcHJ1ZWJhMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE
LAKVsFdtak2wRhnmpITrbhX6/y4NVMwbYJN0QmYvK+6jw9u1sd7XPMl1Wb6aFgF8
4LHYN0CgUIDgyaFWbQL836NTMFEwHQYDVR0OBBYEFJCJGotQuO+WyeysG5veiE00
rGYYMB8GA1UdIwQYMBaAFJCJGotQuO+WyeysG5veiE00rGYYMA8GA1UdEwEB/wQF
MAMBAf8wCgYIKoZIzj0EAwIDRwAwRAIgcvZ/HOfHYaVKFYH+kEdED+cO2U9Z09Xe
G+s3P5dBDiwCIAkDwvxlEaIYjVbclt9bmP/P/ZHo785YxIN91JGJYuz1
-----END CERTIFICATE-----
";
    const HUELLA: &str = "80d352cdef15e453af7343f4c58f8b4c384f349d94396c5c50f34bb53ae948b1";

    fn temporal(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nb-identidad-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn la_huella_es_la_misma_que_calcula_openssl() {
        assert_eq!(huella_sha256(PEM).as_deref(), Some(HUELLA));
        assert_eq!(huella_sha256("no es un certificado"), None);
    }

    #[test]
    fn la_clave_privada_se_escribe_sin_que_otros_la_lean() {
        let dir = temporal("privada");
        let clave = dir.join(CLAVE);
        escribir_privado(&clave, b"clave privada").unwrap();
        assert_eq!(std::fs::read_to_string(&clave).unwrap(), "clave privada");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&clave).unwrap().permissions().mode();
            assert_eq!(modo & 0o077, 0, "la clave no la lee nadie más");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
