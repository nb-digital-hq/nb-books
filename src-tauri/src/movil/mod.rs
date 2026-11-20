//! El servidor para NB Books Mobile (ADR-016).
//!
//! HTTPS y WebSocket en la red local, solo para teléfonos emparejados. Es lo
//! único de la aplicación que escucha en la red, igual que `net.rs` es lo único
//! que sale a ella; `scripts/check-network.mjs` lo hace cumplir.
//!
//! Corre en su propio hilo con su propio runtime de tokio: un teléfono que
//! descarga un libro no compite con la ventana ni con la indexación. Apagado, la
//! aplicación no escucha en ningún puerto.
//!
//! Contrato: `docs/architecture/mobile-contract.md`.

pub mod cambios;
pub mod control;
pub mod emparejamiento;
mod http;
pub mod manifiesto;
pub mod repositorio;
mod tls;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;
use ts_rs::TS;

pub use control::Puente;
pub use emparejamiento::Qr;
pub use repositorio::{Seleccion, Telefono};

use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::paths::AppPaths;

/// El nombre que anuncia el servidor: va en `ping`, en el QR y en mDNS.
const NOMBRE: &str = "NB Books";
const TIPO_MDNS: &str = "_nbbooks._tcp.local.";
/// Los puertos: el primero libre. Un rango distinto del de NB Sound, para que
/// las dos aplicaciones puedan estar abiertas a la vez en el mismo equipo.
pub const PUERTOS: RangeInclusive<u16> = 8831..=8899;

/// Lo compartido por todas las peticiones.
pub struct Contexto {
    db: Database,
    rutas: AppPaths,
    emparejamiento: Mutex<emparejamiento::Emparejamiento>,
    puente: Arc<Puente>,
    /// Cuándo se apuntó por última vez que cada teléfono estaba ahí.
    toques: Mutex<HashMap<String, Instant>>,
    /// Avisa a la interfaz de que cambió algo que se ve en Ajustes.
    avisar: Arc<dyn Fn() + Send + Sync>,
    /// Los proveedores de documento, para releer los pasajes de la búsqueda en
    /// directo. Sin ellos la búsqueda responde igual, sin pasajes.
    lectores: Option<Arc<crate::reader::ProviderRegistry>>,
}

/// Lo que ve Ajustes, Teléfonos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "MobileServerState"
)]
pub struct EstadoMovil {
    pub activo: bool,
    /// `host:puerto`, para quien quiera escribirlo a mano en el teléfono.
    pub direccion: Option<String>,
}

/// Lo que se muestra para emparejar un teléfono.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "MobilePairing"
)]
pub struct Emparejar {
    pub qr: Qr,
    /// Para teclearlo en el teléfono si no puede escanear.
    pub codigo: String,
    pub direccion: String,
    pub segundos: u32,
}

/// Dónde escuchar. En la aplicación, la interfaz de la red local y mDNS; en las
/// pruebas, la de bucle y sin anunciarse.
#[derive(Debug, Clone)]
pub struct Opciones {
    pub host: Option<IpAddr>,
    pub mdns: bool,
    pub puertos: RangeInclusive<u16>,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            host: None,
            mdns: true,
            puertos: PUERTOS,
        }
    }
}

struct EnMarcha {
    direccion: SocketAddr,
    huella: String,
    parar: Option<tokio::sync::oneshot::Sender<()>>,
    hilo: Option<JoinHandle<()>>,
    mdns: Option<(mdns_sd::ServiceDaemon, String)>,
}

/// El servidor y su ciclo de vida. Encender y apagar son idempotentes.
pub struct ServidorMovil {
    ctx: Arc<Contexto>,
    opciones: Opciones,
    en_marcha: Mutex<Option<EnMarcha>>,
}

/// La IP por la que sale el tráfico a la red local, que es la que alcanza el
/// teléfono. Conectar un socket UDP no envía nada: solo hace que el sistema
/// elija la interfaz.
fn ip_lan() -> IpAddr {
    let probada = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|s| {
        s.connect((Ipv4Addr::new(10, 255, 255, 255), 1))?;
        s.local_addr()
    });
    match probada {
        Ok(a) if !a.ip().is_loopback() && !a.ip().is_unspecified() => a.ip(),
        _ => IpAddr::V4(Ipv4Addr::LOCALHOST),
    }
}

fn anunciar(direccion: SocketAddr) -> Option<(mdns_sd::ServiceDaemon, String)> {
    let daemon = mdns_sd::ServiceDaemon::new().ok()?;
    let host = format!("nbbooks-{}.local.", direccion.port());
    let protocolo = manifiesto::PROTOCOLO.to_string();
    let info = mdns_sd::ServiceInfo::new(
        TIPO_MDNS,
        NOMBRE,
        &host,
        direccion.ip(),
        direccion.port(),
        &[("version", protocolo.as_str()), ("servicio", NOMBRE)][..],
    )
    .ok()?;
    let nombre = info.get_fullname().to_owned();
    if daemon.register(info).is_err() {
        let _ = daemon.shutdown();
        return None;
    }
    Some((daemon, nombre))
}

fn bloquear<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl ServidorMovil {
    pub fn nuevo(
        db: Database,
        rutas: AppPaths,
        puente: Arc<Puente>,
        opciones: Opciones,
        avisar: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            ctx: Arc::new(Contexto {
                db,
                rutas,
                emparejamiento: Mutex::new(emparejamiento::Emparejamiento::default()),
                puente,
                toques: Mutex::new(HashMap::new()),
                avisar: Arc::new(avisar),
                lectores: None,
            }),
            opciones,
            en_marcha: Mutex::new(None),
        }
    }

    /// Con los proveedores de documento, para que la búsqueda en directo
    /// devuelva el pasaje de cada resultado.
    #[must_use]
    pub fn con_lectores(mut self, lectores: Arc<crate::reader::ProviderRegistry>) -> Self {
        if let Some(ctx) = Arc::get_mut(&mut self.ctx) {
            ctx.lectores = Some(lectores);
        }
        self
    }

    /// Enciende el servidor si no lo estaba y espera a que escuche.
    ///
    /// # Errors
    ///
    /// - [`NbError::ComponentUnavailable`] si no hay ningún puerto libre.
    /// - Los de [`tls::cargar`] si la identidad no se puede leer ni crear.
    pub fn encender(&self) -> NbResult<()> {
        let mut en_marcha = bloquear(&self.en_marcha);
        if en_marcha.is_some() {
            return Ok(());
        }
        let (config, huella) = tls::cargar(&self.ctx.rutas.movil())?;
        let host = self.opciones.host.unwrap_or_else(ip_lan);
        let tcp = self
            .opciones
            .puertos
            .clone()
            .find_map(|p| std::net::TcpListener::bind((host, p)).ok())
            .ok_or(NbError::ComponentUnavailable)?;
        tcp.set_nonblocking(true).map_err(|_| NbError::Internal)?;
        let direccion = tcp.local_addr().map_err(|_| NbError::Internal)?;
        let (parar, al_parar) = tokio::sync::oneshot::channel::<()>();
        let (listo_tx, listo_rx) = std::sync::mpsc::channel::<NbResult<()>>();
        let ctx = Arc::clone(&self.ctx);
        let hilo = std::thread::Builder::new()
            .name("servidor-movil".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("servidor-movil-red")
                    .enable_all()
                    .build()
                else {
                    let _ = listo_tx.send(Err(NbError::Internal));
                    return;
                };
                runtime.block_on(async move {
                    let oyente = match tokio::net::TcpListener::from_std(tcp)
                        .and_then(|t| tls::OyenteTls::nuevo(t, config))
                    {
                        Ok(o) => o,
                        Err(_) => {
                            let _ = listo_tx.send(Err(NbError::Internal));
                            return;
                        }
                    };
                    let _ = listo_tx.send(Ok(()));
                    let servido = axum::serve(oyente, http::rutas(ctx))
                        .with_graceful_shutdown(async {
                            let _ = al_parar.await;
                        })
                        .await;
                    if servido.is_err() {
                        tracing::error!("el servidor móvil terminó con error");
                    }
                });
                // Los WebSocket abiertos no esperan a que el teléfono cierre.
                runtime.shutdown_timeout(std::time::Duration::from_secs(2));
            })
            .map_err(|_| NbError::Internal)?;
        listo_rx.recv().map_err(|_| NbError::Internal)??;
        bloquear(&self.ctx.emparejamiento).regenerar()?;
        let mdns = if self.opciones.mdns {
            anunciar(direccion)
        } else {
            None
        };
        if self.opciones.mdns && mdns.is_none() {
            tracing::info!("sin anuncio mDNS: los teléfonos se conectarán por el QR");
        }
        tracing::info!(puerto = direccion.port(), "servidor móvil encendido");
        *en_marcha = Some(EnMarcha {
            direccion,
            huella,
            parar: Some(parar),
            hilo: Some(hilo),
            mdns,
        });
        drop(en_marcha);
        (self.ctx.avisar)();
        Ok(())
    }

    /// Apaga el servidor y espera a que su hilo termine.
    pub fn apagar(&self) {
        let Some(mut e) = bloquear(&self.en_marcha).take() else {
            return;
        };
        if let Some((daemon, nombre)) = e.mdns.take() {
            let _ = daemon.unregister(&nombre);
            let _ = daemon.shutdown();
        }
        if let Some(p) = e.parar.take() {
            let _ = p.send(());
        }
        if let Some(h) = e.hilo.take() {
            let _ = h.join();
        }
        bloquear(&self.ctx.emparejamiento).olvidar();
        self.ctx.puente.soltar();
        tracing::info!("servidor móvil apagado");
        (self.ctx.avisar)();
    }

    /// La dirección en la que escucha, si está encendido.
    pub fn direccion(&self) -> Option<SocketAddr> {
        bloquear(&self.en_marcha).as_ref().map(|e| e.direccion)
    }

    pub fn estado(&self) -> EstadoMovil {
        let direccion = self.direccion();
        EstadoMovil {
            activo: direccion.is_some(),
            direccion: direccion.map(|d| d.to_string()),
        }
    }

    /// Un QR y un código nuevos para emparejar; los anteriores dejan de valer.
    /// Enciende el servidor si estaba apagado.
    ///
    /// # Errors
    ///
    /// Los de [`Self::encender`].
    pub fn emparejar(&self) -> NbResult<Emparejar> {
        self.encender()?;
        let (direccion, huella) = {
            let e = bloquear(&self.en_marcha);
            let e = e.as_ref().ok_or(NbError::ComponentUnavailable)?;
            (e.direccion, e.huella.clone())
        };
        let mut emp = bloquear(&self.ctx.emparejamiento);
        emp.regenerar()?;
        let (token, codigo, resta) = emp.actual().ok_or(NbError::Internal)?;
        let host = direccion.ip().to_string();
        let contenido = serde_json::to_vec(&emparejamiento::ContenidoQr {
            host: &host,
            puerto: direccion.port(),
            token,
            version: manifiesto::PROTOCOLO,
            tls: true,
            tls_fingerprint: &huella,
            servicio: NOMBRE,
        })
        .map_err(|_| NbError::Internal)?;
        Ok(Emparejar {
            qr: emparejamiento::qr(&contenido)?,
            codigo: codigo.to_owned(),
            direccion: direccion.to_string(),
            segundos: u32::try_from(resta.as_secs()).unwrap_or(u32::MAX),
        })
    }

    /// El token del QR vigente. Lo usan las pruebas, que hacen de teléfono.
    pub fn token_vigente(&self) -> Option<String> {
        bloquear(&self.ctx.emparejamiento)
            .actual()
            .map(|(t, _, _)| t.to_owned())
    }

    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn telefonos(&self) -> NbResult<Vec<Telefono>> {
        repositorio::listar(&self.ctx.db, self.ctx.puente.controlador().as_deref())
    }

    /// Revoca un teléfono: su token deja de valer y, si manejaba el lector, deja
    /// de hacerlo en ese momento.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede escribir.
    pub fn revocar(&self, id: &str) -> NbResult<()> {
        repositorio::revocar(&self.ctx.db, id)?;
        self.ctx.puente.expulsar(id);
        tracing::info!("teléfono revocado");
        (self.ctx.avisar)();
        Ok(())
    }

    /// Corta Leer en el PC, sea quien sea el teléfono que lo maneja.
    pub fn cortar_control(&self) {
        self.ctx.puente.soltar();
        (self.ctx.avisar)();
    }
}

impl Drop for ServidorMovil {
    fn drop(&mut self) {
        self.apagar();
    }
}
