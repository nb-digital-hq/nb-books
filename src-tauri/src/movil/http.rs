//! Las rutas HTTP del protocolo `1`.
//!
//! Contrato: `docs/architecture/mobile-contract.md`, sección 4.
//!
//! Todo exige el `device_token` salvo `ping` y `pair`. Lo que se le sirve a un
//! teléfono pasa siempre por su selección: no se le da un archivo, una portada
//! ni una ficha de algo que no eligió recibir, aunque adivine su identificador.

use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::manifiesto::{self, SELECCIONADAS};
use super::repositorio::{self, Dispositivo, Seleccion};
use super::{cambios, control, Contexto, NOMBRE};
use crate::error::{NbError, NbResult};

/// Cada cuánto se apunta, como mucho, que un teléfono está ahí. Cada tramo de una
/// descarga es una petición, y escribir en la base en cada una sería absurdo.
const INTERVALO_TOQUE: std::time::Duration = std::time::Duration::from_secs(15);

/// Lo que se lee de un archivo en cada trozo de la respuesta.
const TROZO: usize = 64 * 1024;

/// Lo más que se acepta en un cuerpo: un envío de cambios lleno de anotaciones
/// largas. Un cuerpo mayor no es de un teléfono que funciona bien.
const CUERPO_MAXIMO: usize = 8 * 1024 * 1024;

pub fn rutas(ctx: Arc<Contexto>) -> Router {
    Router::new()
        .route("/api/v1/ping", get(ping))
        .route("/api/v1/pair", post(emparejar))
        .route("/api/v1/seleccion", get(seleccion).post(fijar_seleccion))
        .route("/api/v1/estanterias", get(estanterias))
        .route("/api/v1/manifest", get(manifiesto))
        .route("/api/v1/catalogo", get(catalogo))
        .route("/api/v1/archivo/{huella}", get(archivo))
        .route("/api/v1/portada/{edicion}", get(portada))
        .route("/api/v1/cambios", post(subir_cambios))
        .route("/api/v1/buscar", get(buscar))
        .route("/api/v1/control", get(control::abrir))
        .fallback(no_existe)
        .layer(axum::extract::DefaultBodyLimit::max(CUERPO_MAXIMO))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&ctx),
            autenticar,
        ))
        .with_state(ctx)
}

fn json<T: Serialize + ?Sized>(estado: StatusCode, cuerpo: &T) -> Response {
    (estado, Json(cuerpo)).into_response()
}

fn error(estado: StatusCode, codigo: &str) -> Response {
    json(estado, &json!({ "error": codigo }))
}

fn respuesta_de(error: NbError) -> Response {
    match error {
        NbError::DocumentNotFound | NbError::FileNotFound => {
            self::error(StatusCode::NOT_FOUND, "no_encontrado")
        }
        NbError::Internal => self::error(StatusCode::BAD_REQUEST, "peticion_invalida"),
        NbError::StorageBudgetReached => self::error(StatusCode::CONFLICT, "limite_alcanzado"),
        _ => self::error(StatusCode::INTERNAL_SERVER_ERROR, "fallo_interno"),
    }
}

/// Las consultas a la base bloquean: van al hilo de bloqueo de tokio, no a los
/// del servidor, que atienden a todos los teléfonos.
async fn bloqueo<T: Send + 'static>(
    f: impl FnOnce() -> NbResult<T> + Send + 'static,
) -> NbResult<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| NbError::Internal)?
}

fn bearer(cabeceras: &HeaderMap) -> Option<String> {
    let valor = cabeceras.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = valor.get(7..)?.trim();
    (valor[..7].eq_ignore_ascii_case("bearer ") && !token.is_empty()).then(|| token.to_owned())
}

/// Apunta que el teléfono está ahí, sin escribir en la base en cada petición.
async fn tocar(ctx: &Arc<Contexto>, id: &str) {
    let toca = {
        let mut t = super::bloquear(&ctx.toques);
        match t.get(id) {
            Some(antes) if antes.elapsed() < INTERVALO_TOQUE => false,
            _ => {
                t.insert(id.to_owned(), Instant::now());
                true
            }
        }
    };
    if toca {
        let (db, id) = (ctx.db.clone(), id.to_owned());
        let _ = bloqueo(move || repositorio::tocar(&db, &id)).await;
        (ctx.avisar)();
    }
}

async fn autenticar(
    State(ctx): State<Arc<Contexto>>,
    mut peticion: Request,
    siguiente: Next,
) -> Response {
    let ruta = peticion.uri().path().to_owned();
    if ruta == "/api/v1/pair" {
        return siguiente.run(peticion).await;
    }
    let dispositivo = match bearer(peticion.headers()) {
        Some(token) => {
            let db = ctx.db.clone();
            bloqueo(move || repositorio::por_token(&db, &token))
                .await
                .ok()
                .flatten()
        }
        None => None,
    };
    if ruta == "/api/v1/ping" {
        if let Some(d) = &dispositivo {
            tocar(&ctx, &d.id).await;
        }
        return siguiente.run(peticion).await;
    }
    let Some(d) = dispositivo else {
        return error(StatusCode::UNAUTHORIZED, "no_autorizado");
    };
    tocar(&ctx, &d.id).await;
    peticion.extensions_mut().insert(d);
    siguiente.run(peticion).await
}

async fn no_existe() -> Response {
    error(StatusCode::NOT_FOUND, "no_existe")
}

async fn ping() -> Response {
    json(
        StatusCode::OK,
        &json!({ "ok": true, "servicio": NOMBRE, "version_protocolo": manifiesto::PROTOCOLO }),
    )
}

#[derive(Deserialize)]
struct PeticionPar {
    token: String,
    nombre_dispositivo: String,
    plataforma: String,
}

async fn emparejar(
    State(ctx): State<Arc<Contexto>>,
    cuerpo: Option<Json<PeticionPar>>,
) -> Response {
    let Some(Json(p)) = cuerpo else {
        return error(StatusCode::BAD_REQUEST, "peticion_invalida");
    };
    // Un solo uso: lo que se valida se consume en el mismo cerrojo, para que dos
    // teléfonos con el mismo QR no se emparejen los dos.
    let valido = {
        let mut e = super::bloquear(&ctx.emparejamiento);
        let ok = e.valida(&p.token);
        if ok {
            e.olvidar();
        }
        ok
    };
    if !valido {
        return error(StatusCode::UNAUTHORIZED, "token_invalido_o_expirado");
    }
    let db = ctx.db.clone();
    let resultado =
        bloqueo(move || repositorio::emparejar(&db, &p.nombre_dispositivo, &p.plataforma)).await;
    match resultado {
        Ok((id, token)) => {
            tracing::info!("teléfono emparejado");
            (ctx.avisar)();
            json(
                StatusCode::OK,
                &json!({ "ok": true, "device_token": token, "dispositivo_id": id, "nombre": NOMBRE }),
            )
        }
        Err(e) => respuesta_de(e),
    }
}

async fn seleccion(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
) -> Response {
    let db = ctx.db.clone();
    match bloqueo(move || {
        let conn = db.connection()?;
        repositorio::seleccion(&conn, &d.id)
    })
    .await
    {
        Ok(s) => json(StatusCode::OK, &s),
        Err(e) => respuesta_de(e),
    }
}

async fn fijar_seleccion(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    cuerpo: Option<Json<Seleccion>>,
) -> Response {
    let Some(Json(s)) = cuerpo else {
        return error(StatusCode::BAD_REQUEST, "peticion_invalida");
    };
    let db = ctx.db.clone();
    match bloqueo(move || repositorio::fijar_seleccion(&db, &d.id, &s)).await {
        Ok(ampliada) => json(
            StatusCode::OK,
            &json!({ "ok": true, "reiniciar": ampliada }),
        ),
        Err(e) => respuesta_de(e),
    }
}

/// Lo que se puede elegir: colecciones y estanterías con su tamaño. Es lo que
/// pinta la pantalla de selección del teléfono, y no depende de lo elegido.
///
/// Cota: las estanterías de V24 son una por autor, serie o materia, y se
/// devuelven las más grandes primero hasta [`MAX_ESTANTERIAS`] por clase.
async fn estanterias(State(ctx): State<Arc<Contexto>>) -> Response {
    const MAX_ESTANTERIAS: i64 = 2_000;
    let db = ctx.db.clone();
    let resultado = bloqueo(move || {
        let conn = db.connection()?;
        let fila = |sql: &str| -> NbResult<serde_json::Value> {
            let mut c = conn.prepare(sql).map_err(|_| NbError::StorageFailure)?;
            let filas = c
                .query_map([MAX_ESTANTERIAS], |f| f.get::<_, String>(0))
                .map_err(|_| NbError::StorageFailure)?;
            let lista = filas
                .map(|f| {
                    let texto = f.map_err(|_| NbError::StorageFailure)?;
                    serde_json::from_str::<serde_json::Value>(&texto).map_err(|_| NbError::Internal)
                })
                .collect::<NbResult<Vec<_>>>()?;
            Ok(serde_json::Value::Array(lista))
        };
        let clase = |kind: &str| {
            fila(
                &format!(
                    "SELECT json_object('clave', shelf_key, 'nombre', label, 'libros', tally)
                       FROM shelf_counts WHERE kind = '{kind}'
                      ORDER BY tally DESC, label LIMIT ?1"
                ),
            )
        };
        Ok(json!({
            "colecciones": fila(
                "SELECT json_object('id', k.id, 'nombre', k.name, 'tipo', k.kind,
                          'libros', (SELECT count(*) FROM collection_items WHERE collection_id = k.id))
                   FROM collections k ORDER BY k.name LIMIT ?1",
            )?,
            "autores": clase("author")?,
            "series": clase("series")?,
            "materias": clase("subject")?,
            "total": conn.query_row("SELECT count(*) FROM editions", [], |f| f.get::<_, i64>(0))
                .map_err(|_| NbError::StorageFailure)?,
        }))
    })
    .await;
    match resultado {
        Ok(v) => json(StatusCode::OK, &v),
        Err(e) => respuesta_de(e),
    }
}

#[derive(Deserialize)]
struct PeticionManifiesto {
    #[serde(default)]
    since: i64,
    limit: Option<u32>,
}

async fn manifiesto(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    Query(q): Query<PeticionManifiesto>,
) -> Response {
    let db = ctx.db.clone();
    let limite = q.limit.unwrap_or(manifiesto::PAGINA_POR_DEFECTO);
    match bloqueo(move || {
        let conn = db.connection()?;
        manifiesto::pagina(&conn, &d.id, q.since, limite)
    })
    .await
    {
        Ok(p) => json(StatusCode::OK, &p),
        Err(e) => respuesta_de(e),
    }
}

#[derive(Deserialize)]
struct PeticionCatalogo {
    /// Texto a buscar en título y autoría. Vacío: todo.
    #[serde(default)]
    q: String,
    #[serde(default)]
    desde: u32,
    limit: Option<u32>,
}

/// "Todo el catálogo" del teléfono: el catálogo entero del PC, paginado, sin
/// copiarlo. Cada ficha dice si el teléfono ya la tiene seleccionada.
async fn catalogo(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    Query(q): Query<PeticionCatalogo>,
) -> Response {
    const MAXIMO: u32 = 200;
    let db = ctx.db.clone();
    let limite = q.limit.unwrap_or(60).clamp(1, MAXIMO);
    let resultado = bloqueo(move || {
        let conn = db.connection()?;
        let patron = format!("%{}%", q.q.trim().replace(['%', '_'], ""));
        let sql = format!(
            "WITH sel(id) AS ({SELECCIONADAS})
             SELECT json_object('id', e.id, 'titulo', e.title,
                      'autores', json((SELECT json_group_array(c.name) FROM edition_contributors ec
                                    JOIN contributors c ON c.id = ec.contributor_id
                                   WHERE ec.edition_id = e.id AND ec.role = 'author')),
                      'portada', json(CASE WHEN e.cover_path IS NULL THEN 'false' ELSE 'true' END),
                      'seleccionada', json(CASE WHEN e.id IN sel THEN 'true' ELSE 'false' END))
               FROM editions e
              WHERE ?2 = '%%' OR e.title LIKE ?2 OR e.author_sort LIKE ?2
              ORDER BY e.title_key, e.id
              LIMIT ?3 OFFSET ?4"
        );
        let mut c = conn
            .prepare_cached(&sql)
            .map_err(|_| NbError::StorageFailure)?;
        let filas = c
            .query_map(
                rusqlite::params![d.id, patron, i64::from(limite), i64::from(q.desde)],
                |f| f.get::<_, String>(0),
            )
            .map_err(|_| NbError::StorageFailure)?;
        filas
            .map(|f| {
                f.map_err(|_| NbError::StorageFailure)
                    .and_then(|t| serde_json::from_str(&t).map_err(|_| NbError::Internal))
            })
            .collect::<NbResult<Vec<serde_json::Value>>>()
    })
    .await;
    match resultado {
        Ok(fichas) => json(StatusCode::OK, &json!({ "fichas": fichas })),
        Err(e) => respuesta_de(e),
    }
}

/// La ruta en disco de lo que se pide, si el teléfono tiene derecho a ello.
async fn ruta_de_archivo(
    ctx: &Arc<Contexto>,
    dispositivo: &str,
    huella: &str,
) -> NbResult<(PathBuf, String)> {
    let (db, dispositivo, huella) = (ctx.db.clone(), dispositivo.to_owned(), huella.to_owned());
    bloqueo(move || {
        let conn = db.connection()?;
        let id: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT f.id FROM files f
                      WHERE f.content_hash = ?2 AND f.availability = 'available'
                        AND f.edition_id IN ({SELECCIONADAS})
                      LIMIT 1"
                ),
                [&dispositivo, &huella],
                |f| f.get(0),
            )
            .optional()
            .map_err(|_| NbError::StorageFailure)?;
        // Aunque adivine una huella, un teléfono no recibe lo que no eligió.
        let id = id.ok_or(NbError::DocumentNotFound)?;
        let archivo =
            crate::db::catalog::File::find(&conn, &id)?.ok_or(NbError::DocumentNotFound)?;
        let ruta = crate::library::resolve_path(&conn, &archivo)?.ok_or(NbError::FileNotFound)?;
        Ok((ruta, archivo.media_type.clone()))
    })
    .await
}

async fn archivo(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    Path(huella): Path<String>,
    cabeceras: HeaderMap,
) -> Response {
    match ruta_de_archivo(&ctx, &d.id, &huella).await {
        Ok((ruta, tipo)) => {
            servir(&ruta, &tipo, Some(("x-nb-books-hash", huella)), &cabeceras).await
        }
        Err(e) => respuesta_de(e),
    }
}

/// La miniatura de la portada, la misma que pinta la rejilla del PC: es del
/// tamaño de una pantalla de teléfono y pesa una fracción del original.
async fn portada(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    Path(edicion): Path<String>,
    cabeceras: HeaderMap,
) -> Response {
    let (db, portadas) = (ctx.db.clone(), ctx.rutas.cache.join("portadas"));
    let resultado = bloqueo(move || {
        let conn = db.connection()?;
        let nombre: Option<String> = conn
            .query_row(
                &format!(
                    "SELECT cover_path FROM editions
                      WHERE id = ?2 AND id IN ({SELECCIONADAS}) AND cover_path IS NOT NULL"
                ),
                [&d.id, &edicion],
                |f| f.get(0),
            )
            .optional()
            .map_err(|_| NbError::StorageFailure)?;
        let nombre = nombre.ok_or(NbError::DocumentNotFound)?;
        // El nombre lo escribió la aplicación, pero se acota igual: lo que se
        // sirve a la red no puede salir de la carpeta de portadas. Se exige un
        // nombre de archivo suelto y no que exista: la rejilla del escritorio
        // reduce la portada a miniatura y borra el original, y comprobar el
        // original dejaba al teléfono sin portadas.
        let suelto = std::path::Path::new(&nombre)
            .components()
            .map(|c| matches!(c, std::path::Component::Normal(_)))
            .collect::<Vec<_>>();
        if suelto != [true] {
            return Err(NbError::PathNotAllowed);
        }
        crate::covers::para_rejilla(&portadas, &nombre).ok_or(NbError::FileNotFound)
    })
    .await;
    match resultado {
        Ok(ruta) => {
            let tipo = if ruta
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                "image/png"
            } else {
                "image/jpeg"
            };
            servir(&ruta, tipo, None, &cabeceras).await
        }
        Err(e) => respuesta_de(e),
    }
}

#[derive(Deserialize)]
struct PeticionBusqueda {
    q: String,
    #[serde(default)]
    desde: usize,
    limit: Option<usize>,
}

/// La búsqueda de texto completo, en directo: es la del PC sobre toda la
/// biblioteca, con su índice, que el teléfono no tiene. Cada resultado dice si
/// el teléfono ya tiene la edición seleccionada.
///
/// Cota: [`MAXIMO`] resultados por página.
async fn buscar(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    Query(q): Query<PeticionBusqueda>,
) -> Response {
    const MAXIMO: usize = 50;
    let limite = q.limit.unwrap_or(20).clamp(1, MAXIMO);
    let (db, lectores) = (ctx.db.clone(), ctx.lectores.clone());
    let resultado = bloqueo(move || {
        let conn = db.connection()?;
        let hits = crate::search::search_corpus(&conn, &q.q, limite, q.desde)?;
        let hits = match &lectores {
            Some(r) => crate::search::con_pasajes(&conn, r, &q.q, hits),
            None => hits,
        };
        let mut sel = conn
            .prepare_cached(&format!("SELECT ?2 IN ({SELECCIONADAS})"))
            .map_err(|_| NbError::StorageFailure)?;
        hits.into_iter()
            .map(|h| {
                let seleccionada: bool = sel
                    .query_row([&d.id, &h.edition_id], |f| f.get(0))
                    .map_err(|_| NbError::StorageFailure)?;
                Ok(json!({
                    "edicion": h.edition_id,
                    "titulo": h.title,
                    "locator": h.locator,
                    "pasaje": h.snippet,
                    "seleccionada": seleccionada,
                }))
            })
            .collect::<NbResult<Vec<_>>>()
    })
    .await;
    match resultado {
        Ok(r) => json(StatusCode::OK, &json!({ "resultados": r })),
        Err(e) => respuesta_de(e),
    }
}

async fn subir_cambios(
    State(ctx): State<Arc<Contexto>>,
    Extension(d): Extension<Dispositivo>,
    cuerpo: Option<Json<cambios::Envio>>,
) -> Response {
    let Some(Json(envio)) = cuerpo else {
        return error(StatusCode::BAD_REQUEST, "peticion_invalida");
    };
    let db = ctx.db.clone();
    match bloqueo(move || cambios::aplicar(&db, &d.id, &envio)).await {
        Ok(r) => json(StatusCode::OK, &json!({ "ok": true, "resultado": r })),
        Err(e) => respuesta_de(e),
    }
}

/// Un `Range: bytes=` de un solo tramo: `a-b`, `a-` y `-n` (los últimos n
/// bytes). `None` si no hay cabecera; `Err` si no se puede atender.
fn tramo(cabeceras: &HeaderMap, tamano: u64) -> Option<Result<(u64, u64), ()>> {
    let valor = cabeceras.get(header::RANGE)?.to_str().ok()?;
    let Some(espec) = valor.trim().strip_prefix("bytes=") else {
        return Some(Err(()));
    };
    let Some((a, b)) = espec.split_once('-') else {
        return Some(Err(()));
    };
    let numero = |s: &str| -> Result<Option<u64>, ()> {
        if s.is_empty() {
            Ok(None)
        } else if s.bytes().all(|c| c.is_ascii_digit()) {
            s.parse().map(Some).map_err(|_| ())
        } else {
            Err(())
        }
    };
    let (Ok(a), Ok(b)) = (numero(a), numero(b)) else {
        return Some(Err(()));
    };
    let ultimo = tamano.saturating_sub(1);
    let (inicio, fin) = match (a, b) {
        (Some(a), Some(b)) if a > b => return Some(Err(())),
        (Some(a), b) => (a, b.map_or(ultimo, |b| b.min(ultimo))),
        (None, Some(n)) => (tamano.saturating_sub(n), ultimo),
        (None, None) => return Some(Err(())),
    };
    if inicio >= tamano {
        return Some(Err(()));
    }
    Some(Ok((inicio, fin)))
}

/// Sirve un archivo entero, o el tramo pedido con 206, y 416 si el tramo no
/// existe. Se envía a trozos: un PDF escaneado de un gigabyte no se carga en
/// memoria para mandarlo.
async fn servir(
    ruta: &PathBuf,
    tipo: &str,
    extra: Option<(&'static str, String)>,
    cabeceras: &HeaderMap,
) -> Response {
    let Ok(meta) = tokio::fs::metadata(ruta).await else {
        return error(StatusCode::NOT_FOUND, "no_encontrado");
    };
    let tamano = meta.len();
    let mut r = Response::builder()
        .header(header::CONTENT_TYPE, tipo)
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some((nombre, valor)) = &extra {
        if let Ok(v) = HeaderValue::from_str(valor) {
            r = r.header(*nombre, v);
        }
    }
    let (estado, inicio, largo) = match tramo(cabeceras, tamano) {
        None => (StatusCode::OK, 0, tamano),
        Some(Ok((a, b))) => {
            r = r.header(header::CONTENT_RANGE, format!("bytes {a}-{b}/{tamano}"));
            (StatusCode::PARTIAL_CONTENT, a, b - a + 1)
        }
        Some(Err(())) => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{tamano}"))
                .body(Body::empty())
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    };
    let Ok(mut abierto) = tokio::fs::File::open(ruta).await else {
        return error(StatusCode::NOT_FOUND, "no_encontrado");
    };
    if abierto.seek(SeekFrom::Start(inicio)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let flujo = futures_util::stream::unfold((abierto, largo), |(mut f, quedan)| async move {
        if quedan == 0 {
            return None;
        }
        let mut buf = vec![0u8; TROZO.min(usize::try_from(quedan).unwrap_or(TROZO))];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((
                    Ok::<_, std::io::Error>(Bytes::from(buf)),
                    (f, quedan - n as u64),
                ))
            }
            Err(e) => Some((Err(e), (f, 0))),
        }
    });
    r.status(estado)
        .header(header::CONTENT_LENGTH, largo)
        .body(Body::from_stream(flujo))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn con_rango(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::RANGE, HeaderValue::from_str(v).expect("cabecera"));
        h
    }

    #[test]
    fn los_tramos_se_leen_como_dice_http() {
        assert_eq!(tramo(&HeaderMap::new(), 100), None);
        assert_eq!(tramo(&con_rango("bytes=0-9"), 100), Some(Ok((0, 9))));
        assert_eq!(tramo(&con_rango("bytes=90-"), 100), Some(Ok((90, 99))));
        assert_eq!(tramo(&con_rango("bytes=-10"), 100), Some(Ok((90, 99))));
        assert_eq!(tramo(&con_rango("bytes=50-500"), 100), Some(Ok((50, 99))));
        assert_eq!(tramo(&con_rango("bytes=100-"), 100), Some(Err(())));
        assert_eq!(tramo(&con_rango("bytes=9-0"), 100), Some(Err(())));
        assert_eq!(tramo(&con_rango("lineas=0-9"), 100), Some(Err(())));
    }

    #[test]
    fn el_token_va_en_la_cabecera_de_autorizacion() {
        let mut h = HeaderMap::new();
        assert_eq!(bearer(&h), None);
        h.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer abc"),
        );
        assert_eq!(bearer(&h).as_deref(), Some("abc"));
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic abc"));
        assert_eq!(bearer(&h), None);
    }
}
