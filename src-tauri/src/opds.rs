//! Cliente de catálogos OPDS.
//!
//! # Qué resuelve
//!
//! **De dónde salen los libros** sin que inventemos una tienda, y **la
//! biblioteca vacía**: quien instala NB Books hoy se encuentra un producto que
//! hay que llenar antes de que sirva. OPDS es además el protocolo que
//! sirven muchas bibliotecas públicas y universitarias.
//!
//! # Ninguno preconfigurado
//!
//! La tabla de catálogos nace vacía y solo la llena el usuario. Un catálogo
//! puesto de fábrica es una recomendación comercial disfrazada de función.
//!
//! # Un feed es XML de un desconocido
//!
//! Las mismas defensas que el extractor de EPUB, y por el mismo motivo: **sin
//! resolución de entidades externas** —solo las cinco del propio formato—, sin
//! DTD, y con el tamaño y el número de entradas acotados. Un catálogo hostil
//! puede devolver un feed de un gigabyte con un millón de entradas, y eso no
//! puede convertirse en un millón de nodos en el webview.
//!
//! # Lo que se descarga entra por la puerta de siempre
//!
//! Un archivo bajado de un catálogo **no es distinto** de uno que el usuario
//! suelta sobre la ventana: pasa por [`crate::library::import_document`], que
//! reconoce el formato **por contenido** y no por lo que diga el servidor. Un
//! catálogo que anuncia un EPUB y sirve otra cosa se rechaza ahí, y no queda
//! registro de nada.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use quick_xml::events::Event;
use quick_xml::Reader;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::net::{Network, Purpose};

/// Cuántas entradas de un feed se conservan.
///
/// Doscientas. Un catálogo bien hecho pagina; uno que devuelve diez mil
/// entradas de golpe no está paginando, y traerlas todas al webview reventaría
/// el presupuesto de nodos del documento.
const MAXIMO_ENTRADAS: usize = 200;

/// Cuánto puede pesar un feed.
///
/// Cuatro mebibytes de XML son muchísimas entradas. Lo que pase de aquí no es un
/// catálogo: es una respuesta que no queremos analizar.
const MAXIMO_FEED: usize = 4 * 1024 * 1024;

/// Cuánto puede pesar un documento descargado.
///
/// Quinientos mebibytes. Un cómic escaneado grande cabe; una respuesta sin fin,
/// no.
const MAXIMO_DESCARGA: usize = 500 * 1024 * 1024;

/// Elementos cuyo texto se lee. Ver `capturando` en [`analizar`].
const CON_TEXTO: &[&str] = &["title", "name", "summary", "content"];

/// Un catálogo que el usuario añadió.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Catalog {
    pub id: String,
    pub title: String,
    pub url: String,
    pub added_at: String,
}

/// Una entrada de un feed.
///
/// Puede ser de navegación —lleva a otro feed— o de adquisición —lleva a un
/// documento—. Las dos cosas a la vez no: si trae las dos, manda la adquisición,
/// porque es lo que el usuario ha venido a buscar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct FeedEntry {
    pub title: String,
    pub authors: Option<String>,
    pub summary: Option<String>,
    /// Dirección de otro feed, cuando la entrada es de navegación.
    pub navigation: Option<String>,
    /// Dirección del documento, cuando la entrada es de adquisición.
    pub download: Option<String>,
    /// Tipo de medio que **declara** el catálogo.
    ///
    /// No se cree: la importación reconoce el formato por contenido. Está para
    /// que la interfaz pueda decir qué dice que es antes de descargarlo.
    pub media_type: Option<String>,
}

/// Un feed ya analizado.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Feed {
    pub title: String,
    pub entries: Vec<FeedEntry>,
    /// Dirección de la página siguiente, si el catálogo pagina.
    pub next: Option<String>,
}

/// Añade un catálogo.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la dirección no es `https` ni `http` con
///   anfitrión, o si el nombre está vacío. Un catálogo en claro se admite;
///   salir hacia él pide su propio consentimiento.
/// - [`NbError::StorageFailure`] si ya estaba o si la escritura falla.
pub fn add(conn: &Connection, title: &str, url: &str) -> NbResult<Catalog> {
    // Se valida por el mismo camino que la puerta de red: si aquí se aceptara
    // algo que allí se rechaza, el catálogo existiría y no serviría.
    //
    // Validar no es autorizar: anotar una dirección no concede la salida hacia
    // ella. El permiso lo da el usuario, por anfitrión y propósito, desde
    // `ipc::allow_catalog_host`. Lo comprueba, rompiéndolo,
    // `anadir_un_catalogo_no_autoriza_a_salir_a_el`.
    let _ = crate::net::anfitrion_de_catalogo(url)?;

    let titulo = title.trim();
    if titulo.is_empty() {
        tracing::info!("se rechazó un catálogo sin nombre");
        return Err(NbError::PathNotAllowed);
    }

    conn.query_row(
        "INSERT INTO catalogs (title, url) VALUES (?1, ?2)
         RETURNING id, title, url, added_at",
        rusqlite::params![titulo, url.trim()],
        |row| {
            Ok(Catalog {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                added_at: row.get(3)?,
            })
        },
    )
    .map_err(|error| {
        tracing::info!(error = %error, "no se pudo añadir el catálogo");
        NbError::StorageFailure
    })
}

/// Catálogos añadidos, del más reciente al más antiguo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
pub fn list(conn: &Connection) -> NbResult<Vec<Catalog>> {
    let mut stmt = conn
        .prepare("SELECT id, title, url, added_at FROM catalogs ORDER BY added_at DESC, id")
        .map_err(fallo)?;

    stmt.query_map([], |row| {
        Ok(Catalog {
            id: row.get(0)?,
            title: row.get(1)?,
            url: row.get(2)?,
            added_at: row.get(3)?,
        })
    })
    .and_then(Iterator::collect)
    .map_err(fallo)
}

/// Quita un catálogo. Que no estuviera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
pub fn remove(conn: &Connection, id: &str) -> NbResult<()> {
    conn.execute("DELETE FROM catalogs WHERE id = ?1", [id])
        .map(|_| ())
        .map_err(fallo)
}

/// Lo que se ve al abrir un feed.
///
/// No es un `Result` a secas porque dos de sus salidas **no son errores**, sino
/// cosas que el usuario tiene que hacer, y cada una necesita un dato que un código
/// de error no lleva.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "estado", content = "detalle")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum CatalogPage {
    Feed(Feed),
    /// Falta autorizar **este** anfitrión, que puede no ser el del catálogo: es
    /// el de la redirección que se estaba siguiendo. Ver `net::Network::intentar`.
    #[serde(rename_all = "camelCase")]
    NeedsConsent {
        host: String,
        cleartext: bool,
    },
    /// El catálogo pide usuario y contraseña (401 o 403), que todavía no se
    /// admiten. Se dice en lugar de "no se pudo alcanzar", que sería falso: el
    /// servidor contestó.
    NeedsLogin,
}

/// Pide un feed y lo analiza.
///
/// # Errors
///
/// - Los de la puerta de red: [`NbError::NetworkDisabled`] o
///   [`NbError::NetworkFailure`]. La falta de permiso no es un error: vuelve como
///   [`CatalogPage::NeedsConsent`], con el anfitrión que falta.
/// - [`NbError::DocumentCorrupted`] si lo que llega no es un feed analizable. Es
///   deliberadamente distinto de un fallo de red: un catálogo inalcanzable y uno
///   que devuelve basura piden cosas distintas al usuario.
pub fn navegar(db: &crate::db::Database, net: &Arc<Network>, url: &str) -> NbResult<CatalogPage> {
    let conn = db.connection()?;
    let respuesta = match net.intentar(&conn, url, Purpose::Catalog)? {
        crate::net::Intento::Respondio(respuesta) => respuesta,
        crate::net::Intento::FaltaPermiso { host, cleartext } => {
            return Ok(CatalogPage::NeedsConsent { host, cleartext });
        }
    };

    if matches!(respuesta.status, 401 | 403) {
        tracing::info!(status = respuesta.status, "el catálogo pide credenciales");
        return Ok(CatalogPage::NeedsLogin);
    }
    if respuesta.status != 200 {
        tracing::info!(status = respuesta.status, "el catálogo no respondió bien");
        return Err(NbError::NetworkFailure);
    }
    if respuesta.body.len() > MAXIMO_FEED {
        tracing::warn!(bytes = respuesta.body.len(), "el feed pasa del límite");
        return Err(NbError::DocumentCorrupted);
    }

    let texto = String::from_utf8_lossy(&respuesta.body);
    analizar(&texto, url).map(CatalogPage::Feed)
}

/// Analiza un feed OPDS y resuelve sus direcciones contra la del propio feed.
///
/// # Errors
///
/// [`NbError::DocumentCorrupted`] si el XML no se deja analizar o si no es un
/// feed. Un documento que no se entiende no se pinta a medias.
pub fn analizar(xml: &str, base: &str) -> NbResult<Feed> {
    let mut lector = Reader::from_str(xml);
    // Estricto con los cierres, al revés que el saneador de EPUB. Allí se afloja
    // porque el marcado real de los libros viene mal cerrado y aun así hay que
    // leerlo; aquí un feed truncado **es** una respuesta corrupta, y aceptarlo a
    // medias sería enseñar la mitad de un catálogo como si fuera entero.
    lector.config_mut().check_end_names = true;

    let mut feed = Feed::default();
    let mut buf = Vec::new();
    let mut ruta: Vec<String> = Vec::new();
    let mut entrada: Option<FeedEntry> = None;
    let mut texto = String::new();
    let mut era_feed = false;
    /*
     * Qué elemento se está leyendo por su texto, si alguno.
     *
     * Hace falta porque `<content type="xhtml">` trae marcado dentro, y sin
     * esto el texto se perdía: limpiando el buffer en cada apertura y en cada
     * cierre, lo que quedaba al cerrar `</content>` era lo que hubiera después
     * del último `</p>`, que es nada. Las fichas de Project Gutenberg llegaban
     * sin resumen por eso.
     *
     * Con la marca, el texto se acumula desde que se abre el elemento que
     * interesa hasta que se cierra **ese mismo**, y los elementos de dentro no
     * lo interrumpen.
     */
    let mut capturando: Option<String> = None;

    loop {
        match lector.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let nombre = local_de(e.name().as_ref());

                if capturando.is_none() && CON_TEXTO.contains(&nombre.as_str()) {
                    capturando = Some(nombre.clone());
                    texto.clear();
                }

                match nombre.as_str() {
                    "feed" => era_feed = true,
                    "link" => aplicar_enlace(&e, base, &mut feed, entrada.as_mut()),
                    "entry" => {
                        entrada = Some(FeedEntry {
                            title: String::new(),
                            authors: None,
                            summary: None,
                            navigation: None,
                            download: None,
                            media_type: None,
                        });
                    }
                    _ => {}
                }
                ruta.push(nombre);
            }
            // Un `<link/>` es vacío: sin esta rama, ninguna entrada tendría
            // dirección y el catálogo se vería como una lista de títulos muertos.
            //
            // El caso de `<link ...></link>` lo atiende la rama de `Start`: no
            // es lo habitual en Atom, pero un catálogo que lo escriba así no
            // está mal formado, y perder por eso todos sus enlaces sería
            // rechazarlo por una cuestión de estilo.
            Ok(Event::Empty(e)) => {
                if local_de(e.name().as_ref()) == "link" {
                    aplicar_enlace(&e, base, &mut feed, entrada.as_mut());
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(trozo) = t.decode() {
                    texto.push_str(&trozo);
                }
            }
            // Solo las cinco entidades del formato. Un feed no puede traer una
            // entidad que apunte a un archivo del disco: es la misma defensa que
            // el extractor de metadatos de EPUB.
            Ok(Event::GeneralRef(r)) => {
                if let Some(c) = crate::epub::resolver_entidad_publica(&r) {
                    texto.push(c);
                }
            }
            Ok(Event::End(e)) => {
                let nombre = local_de(e.name().as_ref());
                ruta.pop();

                // El texto se toma solo al cerrar el elemento que lo abrió: un
                // `</p>` de dentro de un resumen no termina el resumen.
                let contenido = if capturando.as_deref() == Some(nombre.as_str()) {
                    capturando = None;
                    let leido = texto.trim().to_owned();
                    texto.clear();
                    leido
                } else {
                    String::new()
                };

                match (nombre.as_str(), entrada.as_mut()) {
                    ("title", Some(actual)) if actual.title.is_empty() => {
                        actual.title = contenido;
                    }
                    ("title", None) if feed.title.is_empty() => feed.title = contenido,
                    ("summary" | "content", Some(actual)) if actual.summary.is_none() => {
                        actual.summary = Some(contenido).filter(|c| !c.is_empty());
                    }
                    ("name", Some(actual)) => {
                        if !contenido.is_empty() {
                            match &mut actual.authors {
                                Some(previos) => {
                                    previos.push_str(", ");
                                    previos.push_str(&contenido);
                                }
                                None => actual.authors = Some(contenido),
                            }
                        }
                    }
                    ("entry", _) => {
                        if let Some(terminada) = entrada.take() {
                            // Una entrada sin título o sin destino no se puede
                            // ni enseñar ni usar: se descarta en lugar de
                            // pintar una fila que no lleva a ninguna parte.
                            let util = !terminada.title.is_empty()
                                && (terminada.navigation.is_some() || terminada.download.is_some());
                            if util && feed.entries.len() < MAXIMO_ENTRADAS {
                                feed.entries.push(terminada);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::CData(_)) => {}
            // Un feed con DTD se rechaza entero: es la puerta por la que entra
            // una entidad externa, y no hay ningún catálogo legítimo que la
            // necesite.
            Ok(Event::DocType(_)) => {
                tracing::warn!("un feed traía una declaración de tipo de documento");
                return Err(NbError::DocumentCorrupted);
            }
            Err(_) => {
                tracing::warn!("el feed de un catálogo no se deja analizar");
                return Err(NbError::DocumentCorrupted);
            }
        }
        buf.clear();
    }

    if !era_feed {
        tracing::info!("lo que respondió el catálogo no es un feed");
        return Err(NbError::DocumentCorrupted);
    }

    // Elementos abiertos al llegar al final: la respuesta se cortó a mitad. El
    // analizador no lo considera un error —deja de leer y ya está—, así que se
    // comprueba aquí. Enseñar la mitad de un catálogo como si fuera entero es
    // peor que decir que no se pudo leer.
    if !ruta.is_empty() {
        tracing::info!(abiertos = ruta.len(), "el feed llegó cortado");
        return Err(NbError::DocumentCorrupted);
    }

    Ok(feed)
}

/// Qué tan bien nos viene un formato que un catálogo ofrece.
///
/// # Por qué hay que elegir, y no vale quedarse con el último
///
/// Porque una entrada de Project Gutenberg trae **cuatro** enlaces de
/// adquisición —EPUB sin imágenes, Kindle sin imágenes, EPUB3 con imágenes,
/// Kindle con imágenes— y quedarse con el último dejaba puesto el de Kindle. Ese
/// era el defecto real: al pulsar Descargar se bajaban 540.013 bytes de
/// mobipocket, la importación no reconocía el formato —porque no lo es— y la
/// interfaz decía "no era lo que el catálogo decía, así que no se guardó". El
/// catálogo no mentía: se estaba eligiendo mal.
///
/// Cero significa **que no se ofrece**. No es lo mismo que no poder descargarlo:
/// es que ofrecer un botón cuyo único final posible es un rechazo no es ofrecer
/// nada, es prometer. Si una entrada solo trae formatos que no sabemos abrir, no
/// tiene descarga, y si además no lleva a otro feed, no se pinta.
fn prioridad_de(tipo: &str) -> u8 {
    // El tipo puede traer parámetros: `application/epub+zip;charset=utf-8`.
    let limpio = tipo
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match limpio.as_str() {
        // El formato de la casa: reflujo, tipografía y anclaje por selectores.
        "application/epub+zip" | "application/epub" => 3,
        "application/pdf" => 2,
        "application/vnd.comicbook+zip" | "application/x-cbz" => 1,
        // Todo lo demás —mobipocket, texto plano, html, djvu— no lo abre este
        // programa. Ver la nota de arriba: no se ofrece.
        _ => 0,
    }
}

/// Relaciones que llevan a **otro feed que se navega**.
///
/// La lista es blanca y no negra a propósito. Antes valía cualquier enlace de
/// tipo `atom+xml`, y eso convertía en el botón "Abrir" cosas que no llevan a
/// ninguna parte útil: `self`, que es la propia ficha que ya se está viendo, y
/// `related`, que en Ganso y Pulpo es la página del autor. Pulsar Abrir sobre un
/// libro llevaba al autor, y desde fuera eso se ve exactamente como "la lista se
/// reinicia sola".
///
/// Un `rel` vacío también navega: hay catálogos que no lo ponen, y un enlace a
/// un feed dentro de una entrada no puede ser otra cosa.
fn es_navegacion(rel: &str) -> bool {
    matches!(
        rel,
        "" | "subsection"
            | "collection"
            | "http://opds-spec.org/crawlable"
            | "http://opds-spec.org/featured"
    ) || rel.starts_with("http://opds-spec.org/sort/")
}

/// Interpreta un `<link>` y lo coloca donde corresponda.
fn aplicar_enlace(
    e: &quick_xml::events::BytesStart<'_>,
    base: &str,
    feed: &mut Feed,
    entrada: Option<&mut FeedEntry>,
) {
    let mut rel = String::new();
    let mut tipo = String::new();
    let mut href = String::new();

    for atributo in e.attributes().flatten() {
        let clave = String::from_utf8_lossy(atributo.key.as_ref()).to_ascii_lowercase();
        let valor = String::from_utf8_lossy(&atributo.value).into_owned();
        match clave.as_str() {
            "rel" => rel = valor.trim().to_owned(),
            "type" => tipo = valor,
            "href" => href = valor,
            _ => {}
        }
    }

    let Some(url) = resolver(base, &href) else {
        return;
    };

    // Un feed lleva `type="application/atom+xml"`, con o sin perfil; cualquier
    // otra cosa que se pueda adquirir es un documento.
    let es_feed = tipo.starts_with("application/atom+xml");
    let es_adquisicion = rel.starts_with("http://opds-spec.org/acquisition");

    match entrada {
        Some(actual) => {
            if es_adquisicion && !es_feed {
                let prioridad = prioridad_de(&tipo);
                let puesta = actual.media_type.as_deref().map_or(0, prioridad_de);
                // Estrictamente mejor: con dos EPUB gana el primero, que en los
                // catálogos que ofrecen varios es el más ligero.
                if prioridad > 0 && prioridad > puesta {
                    actual.download = Some(url);
                    actual.media_type = Some(tipo);
                }
            } else if es_feed && es_navegacion(&rel) && actual.navigation.is_none() {
                actual.navigation = Some(url);
            }
        }
        None => {
            // Enlaces del feed, no de una entrada. Solo interesa la paginación:
            // el resto son metadatos del propio catálogo.
            if rel == "next" && es_feed {
                feed.next = Some(url);
            }
        }
    }
}

/// Resuelve una dirección relativa contra la del feed.
///
/// Devuelve `None` para cualquier cosa que no acabe siendo `https`. Un feed
/// puede traer `http://`, `file://` o `javascript:` en un `href`, y ninguna de
/// las tres puede convertirse en algo que este programa abra.
///
/// # El único `http://` que sobrevive, y por qué
///
/// El de **el mismo anfitrión que acaba de servir este feed por `https`**, que
/// se reescribe a `https`. No es una concesión: es lo que hace un navegador con
/// HSTS, y aquí el dato que lo justifica ya está en la mano —la conexión segura
/// con ese servidor está hecha y funcionó—. Si el recurso no estuviera también
/// en `https`, la petición falla y se dice; lo que no ocurre nunca es salir en
/// claro.
///
/// Sin esto, Ganso y Pulpo era un catálogo inservible: sirve el feed por `https`
/// y escribe los enlaces de descarga en `http://` desde 2013. Ninguno de sus
/// libros se podía bajar, y como el enlace de descarga desaparecía, la entrada
/// se quedaba con el `related` del autor y el botón "Abrir" llevaba a otro sitio.
/// Comprobado contra el catálogo real: la misma dirección en `https` devuelve
/// 200 y un EPUB de 399.585 bytes.
///
/// Se hace a mano y no con un analizador de URL, igual que en `net::host_de`: es
/// el mismo puñado de líneas, y la política de dependencias dice que no se añade
/// un paquete por menos de cien.
fn resolver(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    // El esquema del feed decide el de lo relativo. Un catálogo en claro
    // enlaza en claro, y reescribir sus rutas a `https` las llevaría a un
    // servidor que no las sirve.
    let (esquema, base) = match base.strip_prefix("https://") {
        Some(resto) => ("https", resto),
        None => ("http", base.strip_prefix("http://")?),
    };
    let (anfitrion, ruta) = base.split_once('/').unwrap_or((base, ""));

    // Absoluta: `https` siempre; `http` del mismo anfitrión, con el esquema del
    // feed; y `http` de otro anfitrión solo desde un feed que ya es en claro, que
    // es donde no hay nada que degradar -y la puerta pedirá el permiso de ese
    // anfitrión aparte-.
    if href.contains("://") || href.starts_with("//") {
        if href.starts_with("https://") {
            return Some(href.to_owned());
        }
        if let Some(resto) = href.strip_prefix("http://") {
            let anfitrion_del_enlace = resto.split('/').next()?;
            if anfitrion_del_enlace.eq_ignore_ascii_case(anfitrion) {
                return Some(format!("{esquema}://{resto}"));
            }
            if esquema == "http" {
                return Some(href.to_owned());
            }
        }
        return None;
    }
    // Un esquema sin barras —`mailto:`, `javascript:`— no es una dirección que
    // vayamos a seguir.
    if let Some((antes, _)) = href.split_once(':') {
        if !antes.contains('/') && !antes.contains('.') {
            return None;
        }
    }

    if let Some(absoluta) = href.strip_prefix('/') {
        return Some(format!("{esquema}://{anfitrion}/{absoluta}"));
    }

    // Relativa al directorio del feed. La ruta se recorta en el último `/`, y la
    // consulta y el ancla de la base no cuentan.
    let ruta = ruta.split(['?', '#']).next().unwrap_or(ruta);
    let carpeta = match ruta.rfind('/') {
        Some(i) => &ruta[..=i],
        None => "",
    };
    Some(format!("{esquema}://{anfitrion}/{carpeta}{href}"))
}

/// Descarga un documento del catálogo y lo deja en un archivo temporal.
///
/// Devuelve la ruta del temporal. **Quien lo llama es responsable de
/// importarlo y de borrarlo**: aquí no se registra nada, precisamente para que
/// un fallo posterior no deje un registro huérfano.
///
/// # Errors
///
/// - Los de la puerta de red.
/// - [`NbError::DocumentCorrupted`] si la respuesta viene vacía o pasa del
///   límite.
/// - [`NbError::StorageFailure`] si no se puede escribir.
pub fn descargar(
    db: &crate::db::Database,
    net: &Arc<Network>,
    carpeta: &Path,
    url: &str,
) -> NbResult<Descarga> {
    let conn = db.connection()?;
    let intento = net.intentar(&conn, url, Purpose::Catalog)?;
    drop(conn);
    let respuesta = match intento {
        crate::net::Intento::Respondio(respuesta) => respuesta,
        // Las descargas viven a menudo en otro servidor que el feed -una red de
        // distribución, un espejo-, y es el permiso de ese el que falta.
        crate::net::Intento::FaltaPermiso { host, cleartext } => {
            return Ok(Descarga::FaltaPermiso { host, cleartext });
        }
    };

    if respuesta.status != 200 {
        tracing::info!(status = respuesta.status, "la descarga no llegó");
        return Err(NbError::NetworkFailure);
    }
    if respuesta.body.is_empty() || respuesta.body.len() > MAXIMO_DESCARGA {
        tracing::warn!(bytes = respuesta.body.len(), "descarga fuera de límites");
        return Err(NbError::DocumentCorrupted);
    }

    std::fs::create_dir_all(carpeta).map_err(|error| {
        tracing::error!(error = %error, "no se pudo preparar la carpeta de descargas");
        NbError::StorageFailure
    })?;

    // El nombre sale del **contenido**, no de la dirección: una dirección puede
    // traer `../` o un nombre de cien caracteres, y el nombre definitivo lo pone
    // la importación de todos modos. Aquí solo hace falta que sea único.
    let nombre = format!(
        "{}.descarga",
        crate::library::hash_reader(respuesta.body.as_slice())?.0
    );
    let destino = carpeta.join(nombre.replace(':', "-"));

    std::fs::write(&destino, &respuesta.body).map_err(|error| {
        tracing::error!(error = %error, "no se pudo escribir la descarga");
        NbError::StorageFailure
    })?;

    Ok(Descarga::Guardada(destino))
}

/// Cómo terminó bajar un documento de un catálogo.
#[derive(Debug)]
pub enum Descarga {
    /// En este temporal, que quien llama importa y borra.
    Guardada(PathBuf),
    /// Falta el permiso de este anfitrión. Ver [`CatalogPage::NeedsConsent`].
    FaltaPermiso { host: String, cleartext: bool },
}

fn local_de(nombre: &[u8]) -> String {
    let sin_prefijo = match nombre.iter().rposition(|b| *b == b':') {
        Some(i) => &nombre[i + 1..],
        None => nombre,
    };
    String::from_utf8_lossy(sin_prefijo).to_ascii_lowercase()
}

fn fallo(error: rusqlite::Error) -> NbError {
    tracing::error!(error = %error, "fallo en el almacenamiento de catálogos");
    NbError::StorageFailure
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "https://catalogo.example/opds/raiz.xml";

    #[test]
    fn una_direccion_relativa_se_resuelve_contra_la_carpeta_del_feed() {
        assert_eq!(
            resolver(BASE, "seccion.xml").as_deref(),
            Some("https://catalogo.example/opds/seccion.xml")
        );
    }

    #[test]
    fn una_direccion_absoluta_de_ruta_se_resuelve_contra_el_anfitrion() {
        assert_eq!(
            resolver(BASE, "/otra/cosa.xml").as_deref(),
            Some("https://catalogo.example/otra/cosa.xml")
        );
    }

    /// Un feed es de un desconocido: puede traer lo que quiera en un `href`, y
    /// nada de eso puede convertirse en algo que este programa abra.
    #[test]
    fn ningun_esquema_que_no_sea_https_sobrevive() {
        for veneno in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "mailto:alguien@example.com",
            "//catalogo.example/libro.epub",
            "",
            "   ",
        ] {
            assert_eq!(resolver(BASE, veneno), None, "se aceptó {veneno:?}");
        }
    }

    /// El caso de Ganso y Pulpo: el feed llega por `https` y sus descargas
    /// están escritas en `http://` del mismo servidor. Se reescriben, que es lo
    /// que hace un navegador con HSTS; lo que no ocurre nunca es salir en claro.
    #[test]
    fn un_enlace_en_claro_del_mismo_anfitrion_se_reescribe_a_seguro() {
        assert_eq!(
            resolver(BASE, "http://catalogo.example/libro.epub").as_deref(),
            Some("https://catalogo.example/libro.epub")
        );
    }

    /// Y el de otro anfitrión no: ahí no hay ningún dato que diga que ese
    /// servidor habla `https`, así que reescribirlo sería adivinar.
    #[test]
    fn un_enlace_en_claro_de_otro_anfitrion_no_se_reescribe() {
        assert_eq!(resolver(BASE, "http://otro.example/libro.epub"), None);
    }

    /// Cuatro formatos en una entrada de Project Gutenberg, y solo dos los abre
    /// este programa. Quedarse con el último dejaba puesto el mobipocket: se
    /// descargaba, la importación no lo reconocía —porque no es un formato
    /// nuestro— y la interfaz decía que el catálogo había servido otra cosa.
    #[test]
    fn entre_varios_formatos_se_elige_el_que_sabemos_abrir() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>Orgullo y prejuicio</title>
            <link type="application/epub+zip" rel="http://opds-spec.org/acquisition"
                  href="/ebooks/1342.epub.noimages"/>
            <link type="application/x-mobipocket-ebook" rel="http://opds-spec.org/acquisition"
                  href="/ebooks/1342.kindle.noimages"/>
            <link type="application/x-mobipocket-ebook" rel="http://opds-spec.org/acquisition"
                  href="/ebooks/1342.kf8.images"/>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert_eq!(
            feed.entries[0].media_type.as_deref(),
            Some("application/epub+zip")
        );
        assert_eq!(
            feed.entries[0].download.as_deref(),
            Some("https://catalogo.example/ebooks/1342.epub.noimages")
        );
    }

    /// Y si no hay ninguno que sepamos abrir, no se ofrece descargar.
    ///
    /// Un botón cuyo único final posible es "no era lo que el catálogo decía"
    /// no ofrece nada: promete. La entrada se queda sin destino y no se pinta.
    #[test]
    fn un_formato_que_no_abrimos_no_se_ofrece() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>Solo para Kindle</title>
            <link type="application/x-mobipocket-ebook" rel="http://opds-spec.org/acquisition"
                  href="/o.mobi"/>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert!(feed.entries.is_empty(), "{:?}", feed.entries);
    }

    /// El otro caso de Ganso y Pulpo: la ficha de un libro trae `related` al
    /// autor y `self` a sí misma, las dos de tipo `atom+xml`. Tomarlas por
    /// navegación convertía "Abrir" en un salto a la página del autor, que desde
    /// fuera se ve como que la lista se reinicia sola.
    #[test]
    fn ni_la_propia_ficha_ni_lo_relacionado_son_navegacion() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>La luciérnaga</title>
            <link rel="http://opds-spec.org/acquisition" type="application/epub+zip"
                  href="http://catalogo.example/descargas/luciernaga.epub"/>
            <link rel="related" type="application/atom+xml;profile=opds-catalog"
                  href="/autores/bacarisse.atom"/>
            <link rel="self" type="application/atom+xml;type=entry"
                  href="/fichas/NB0001.atom"/>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert_eq!(feed.entries[0].navigation, None);
        assert_eq!(
            feed.entries[0].download.as_deref(),
            Some("https://catalogo.example/descargas/luciernaga.epub")
        );
    }

    /// Lo que sí navega, navega.
    #[test]
    fn una_subseccion_lleva_a_su_feed() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>Novedades</title>
            <link rel="subsection" type="application/atom+xml;profile=opds-catalog"
                  href="novedades.atom"/>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert_eq!(
            feed.entries[0].navigation.as_deref(),
            Some("https://catalogo.example/opds/novedades.atom")
        );
    }

    /// Un `<link>` con etiqueta de cierre es un enlace igual que uno vacío.
    #[test]
    fn un_enlace_con_cierre_explicito_cuenta() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>Obra</title>
            <link rel="http://opds-spec.org/acquisition" type="application/epub+zip"
                  href="o.epub"></link>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert_eq!(
            feed.entries[0].download.as_deref(),
            Some("https://catalogo.example/opds/o.epub")
        );
    }

    /// Un resumen en XHTML es un resumen. Las fichas de Project Gutenberg lo
    /// escriben así, y llegaban vacías porque el marcado de dentro cortaba la
    /// lectura del texto.
    #[test]
    fn un_resumen_con_marcado_dentro_no_se_pierde() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>Obra</title>
            <content type="xhtml"><div xmlns="http://www.w3.org/1999/xhtml"><p>Una novela.</p></div></content>
            <link rel="http://opds-spec.org/acquisition" type="application/epub+zip" href="o.epub"/>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert_eq!(feed.entries[0].summary.as_deref(), Some("Una novela."));
    }

    #[test]
    fn una_direccion_https_absoluta_se_acepta_aunque_sea_de_otro_anfitrion() {
        // Otro anfitrión necesita su propio consentimiento, y eso lo hace
        // cumplir la puerta de red. Aquí no se rechaza: se resuelve.
        assert_eq!(
            resolver(BASE, "https://otro.example/libro.epub").as_deref(),
            Some("https://otro.example/libro.epub")
        );
    }

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Biblioteca de ejemplo</title>
  <link rel="next" type="application/atom+xml;profile=opds-catalog" href="pagina2.xml"/>
  <entry>
    <title>Cien años de soledad</title>
    <author><name>Gabriel García Márquez</name></author>
    <summary>Una novela.</summary>
    <link rel="http://opds-spec.org/acquisition"
          type="application/epub+zip" href="obras/cien.epub"/>
  </entry>
  <entry>
    <title>Novedades</title>
    <link rel="subsection"
          type="application/atom+xml;profile=opds-catalog" href="novedades.xml"/>
  </entry>
  <entry>
    <title>Sin destino</title>
  </entry>
</feed>"#;

    #[test]
    fn un_feed_se_analiza_en_entradas_de_descarga_y_de_navegacion() {
        let feed = analizar(FEED, BASE).expect("feed");

        assert_eq!(feed.title, "Biblioteca de ejemplo");
        assert_eq!(
            feed.next.as_deref(),
            Some("https://catalogo.example/opds/pagina2.xml")
        );
        // La tercera entrada no lleva a ninguna parte: se descarta en lugar de
        // pintar una fila muerta.
        assert_eq!(feed.entries.len(), 2);

        let obra = &feed.entries[0];
        assert_eq!(obra.title, "Cien años de soledad");
        assert_eq!(obra.authors.as_deref(), Some("Gabriel García Márquez"));
        assert_eq!(
            obra.download.as_deref(),
            Some("https://catalogo.example/opds/obras/cien.epub")
        );
        assert_eq!(obra.media_type.as_deref(), Some("application/epub+zip"));
        assert_eq!(obra.navigation, None);

        let seccion = &feed.entries[1];
        assert_eq!(
            seccion.navigation.as_deref(),
            Some("https://catalogo.example/opds/novedades.xml")
        );
        assert_eq!(seccion.download, None);
    }

    #[test]
    fn lo_que_no_es_un_feed_se_rechaza() {
        for basura in [
            "<html><body>no soy un catálogo</body></html>",
            "{\"esto\": \"es json\"}",
            "",
            "<feed><entry><title>sin cerrar",
        ] {
            assert!(
                matches!(analizar(basura, BASE), Err(NbError::DocumentCorrupted)),
                "se aceptó {basura:?} como feed"
            );
        }
    }

    /// La puerta por la que entra una entidad externa. No hay ningún catálogo
    /// legítimo que necesite una declaración de tipo de documento.
    #[test]
    fn un_feed_con_dtd_se_rechaza_entero() {
        let hostil = r#"<?xml version="1.0"?>
<!DOCTYPE feed [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<feed xmlns="http://www.w3.org/2005/Atom"><title>&xxe;</title></feed>"#;
        assert!(matches!(
            analizar(hostil, BASE),
            Err(NbError::DocumentCorrupted)
        ));
    }

    /// Un catálogo puede devolver diez mil entradas de golpe. Traerlas todas al
    /// webview reventaría el presupuesto de nodos del documento.
    #[test]
    fn el_numero_de_entradas_esta_acotado() {
        let mut xml =
            String::from("<feed xmlns=\"http://www.w3.org/2005/Atom\"><title>Muchas</title>");
        for i in 0..(MAXIMO_ENTRADAS + 50) {
            xml.push_str(&format!(
                "<entry><title>Obra {i}</title>\
                 <link rel=\"http://opds-spec.org/acquisition\" \
                 type=\"application/epub+zip\" href=\"o{i}.epub\"/></entry>"
            ));
        }
        xml.push_str("</feed>");

        let feed = analizar(&xml, BASE).expect("feed");
        assert_eq!(feed.entries.len(), MAXIMO_ENTRADAS);
    }

    #[test]
    fn una_entrada_con_varios_autores_los_junta_en_orden() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>T</title>
          <entry><title>Obra</title>
            <author><name>Primera</name></author>
            <author><name>Segunda</name></author>
            <link rel="http://opds-spec.org/acquisition" type="application/epub+zip"
                  href="o.epub"/>
          </entry></feed>"#;
        let feed = analizar(xml, BASE).expect("feed");
        assert_eq!(feed.entries[0].authors.as_deref(), Some("Primera, Segunda"));
    }
}
