//! Los permisos de una extensión, y su consentimiento.
//!
//! # La regla, en una frase
//!
//! Una extensión no obtiene nada que el usuario no le haya dado **a ella**.
//!
//! Las dos últimas palabras son el módulo entero. El consentimiento de red
//! que existe es por anfitrión y por propósito; si se hubiera
//! reutilizado tal cual, conceder a una extensión salida a `openlibrary.org` se
//! la habría concedido a todas las demás, presentes y futuras. Por eso la
//! migración V21 añade el **origen**, y por eso aquí todo pasa por
//! `net::*_from`.
//!
//! Es la mitad automática del requisito **R5** de la política del registro:
//! lo que no se declara no se puede ejercer, porque el anfitrión no lo ofrece.
//!
//! # Por la misma puerta, no por una segunda
//!
//! La red de una extensión sale por `net.rs`, igual que el catálogo, las
//! portadas, la consulta y la voz. Una segunda puerta sería una segunda
//! superficie de auditoría, y `scripts/check-network.mjs` la rechazaría de
//! todos modos. Lo que aporta este módulo no es una salida: es **quién** la
//! pide y **qué se le enseña al usuario antes**.
//!
//! # Qué impide que una actualización amplíe permisos en silencio
//!
//! [`instalar`]. No es un consejo: es el único camino por el que una extensión
//! entra con permisos. Compara lo que pide el paquete nuevo con lo que ya está
//! concedido **a esa extensión**, y si pide algo más devuelve la lista en lugar
//! de instalar. Quien decide es el usuario, y hasta que decide no hay nada
//! nuevo instalado.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::net::{self, Origin, Purpose};

use super::manifest::{Capability, Manifest};
use super::store::{Installed, Store};

/// Un permiso tal y como se le enseña a quien va a instalar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct PermissionAsk {
    /// Qué capacidad.
    pub capability: Capability,
    /// A qué anfitrión concreto. Vacío para una capacidad sin destino.
    pub host: String,
    /// Qué implica concederlo, en lenguaje llano.
    pub implies: String,
    /// Qué se envía con ese propósito, en lenguaje llano.
    pub what_is_sent: String,
    /// Si ya está concedido a **esta** extensión.
    pub granted: bool,
}

/// Cómo terminó un intento de instalar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instalacion {
    /// Entró.
    Instalada(Installed),
    /// Pide más de lo que tiene concedido. Nada se ha instalado.
    FaltaConsentimiento(Vec<PermissionAsk>),
    /**
     * Al paquete le faltan los módulos que su manifiesto declara.
     *
     * Es un caso aparte y no un error genérico porque **el manifiesto está
     * bien**: lo que falta es el `.wasm` compilado. Antes esto
     * salía como "el manifiesto de la extensión no es válido", que manda a
     * revisar el archivo que no tiene nada malo; Se reportó intentando
     * instalar las extensiones propias desde su carpeta de fuentes, que es
     * justo donde el módulo no está todavía.
     */
    Incompleta(Vec<String>),
}

/// El origen con el que sale una extensión.
#[must_use]
pub fn origen(id: &str) -> Origin {
    Origin::Extension(id.to_owned())
}

/// Lo que un manifiesto pide, con el estado de cada cosa para esa extensión.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el consentimiento.
pub fn lo_que_pide(conn: &Connection, manifiesto: &Manifest) -> NbResult<Vec<PermissionAsk>> {
    let origen = origen(&manifiesto.id);
    let mut peticiones = Vec::new();

    if manifiesto.capabilities.contains(&Capability::NetworkFetch) {
        let detalle = manifiesto.permissions.network_fetch.as_ref();
        for url in detalle.map(|d| d.urls.as_slice()).unwrap_or_default() {
            // El anfitrión se saca con la misma función que usa la puerta, no
            // con una copia: si dos códigos distintos dijeran qué es el
            // anfitrión de una URL, el permiso que se enseña y el que se
            // comprueba podrían no ser el mismo.
            let host = net::host_de(url)?;
            let granted = net::has_consent_from(conn, &origen, &host, Purpose::Extension)?;
            peticiones.push(PermissionAsk {
                capability: Capability::NetworkFetch,
                host,
                implies: Capability::NetworkFetch.en_lenguaje_llano().to_owned(),
                what_is_sent: Purpose::Extension.what_is_sent().to_owned(),
                granted,
            });
        }
    }

    peticiones.sort_by(|a, b| a.host.cmp(&b.host));
    peticiones.dedup_by(|a, b| a.host == b.host && a.capability == b.capability);
    Ok(peticiones)
}

/// Instala un paquete, o devuelve lo que falta por conceder.
///
/// **Es el único camino por el que una extensión entra.** Si pide algo que no
/// tiene concedido, no se instala nada: ni el contenido, ni una generación
/// nueva. El usuario decide con la lista delante y vuelve a llamar.
///
/// # Errors
///
/// [`NbError::ExtensionManifestInvalid`] si el manifiesto no es válido;
/// [`NbError::StorageFailure`] si falla el disco o la base.
pub fn instalar(
    conn: &Connection,
    almacen: &Store,
    paquete: &std::path::Path,
) -> NbResult<Instalacion> {
    let texto = std::fs::read_to_string(paquete.join(super::manifest::ARCHIVO))
        .map_err(|_| NbError::DocumentNotFound)?;
    let manifiesto = super::manifest::parse(&texto).map_err(|defectos| {
        tracing::warn!(
            defectos = defectos.len(),
            "manifiesto de extensión inválido"
        );
        NbError::FormatUnsupported
    })?;

    // Antes de los permisos: pedirle a alguien que conceda salida a la red para
    // una extensión que no puede ejecutarse sería pedirle que decida sobre algo
    // que no va a pasar.
    let faltan = super::store::Store::le_faltan_modulos(paquete, &manifiesto);
    if !faltan.is_empty() {
        tracing::info!(
            faltan = faltan.len(),
            "el paquete no trae los módulos que declara"
        );
        return Ok(Instalacion::Incompleta(faltan));
    }

    let peticiones = lo_que_pide(conn, &manifiesto)?;
    if peticiones.iter().any(|p| !p.granted) {
        // Nada se instala. Que la lista incluya también lo ya concedido es a
        // propósito: lo que se le enseña al usuario es **todo** lo que la
        // extensión podrá hacer, no solo la diferencia, porque decidir sobre la
        // diferencia sin ver el conjunto es decidir a ciegas.
        return Ok(Instalacion::FaltaConsentimiento(peticiones));
    }

    Ok(Instalacion::Instalada(almacen.instalar(paquete)?))
}

/// Concede a una extensión salida a un anfitrión.
///
/// Solo concede lo que su manifiesto declara: un anfitrión que la extensión no
/// pide no se puede autorizar, porque entonces el permiso no correspondería a
/// nada que el usuario haya visto.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si ese anfitrión no está en el manifiesto;
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn conceder(conn: &Connection, manifiesto: &Manifest, host: &str) -> NbResult<()> {
    let pedidos = lo_que_pide(conn, manifiesto)?;
    if !pedidos.iter().any(|p| p.host == host) {
        tracing::info!("se intentó conceder un anfitrión que la extensión no pide");
        return Err(NbError::PathNotAllowed);
    }
    net::grant_from(conn, &origen(&manifiesto.id), host, Purpose::Extension)
}

/// Revoca un anfitrión concreto.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn revocar(conn: &Connection, id: &str, host: &str) -> NbResult<()> {
    net::revoke_from(conn, &origen(id), host, Purpose::Extension)
}

/// Desinstala una extensión y **se lleva sus permisos**.
///
/// Los permisos no pueden sobrevivir a la extensión: si sobrevivieran,
/// reinstalarla más tarde saldría a la red sin volver a preguntar, y el usuario
/// habría concedido a una cosa que ya no recuerda.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si falla el disco o la base.
pub fn desinstalar(conn: &Connection, almacen: &Store, id: &str) -> NbResult<bool> {
    let quitada = almacen.desinstalar(id)?;
    net::revoke_all_from(conn, &origen(id))?;
    // Y lo que dejó puesto en las preferencias: un tema o un modo de lectura
    // elegidos siguen apuntando a esta extensión después de quitarla, y eso es
    // rastro. Ver `personalization::olvidar` y el criterio.
    super::personalization::olvidar(conn, id)?;
    Ok(quitada)
}
