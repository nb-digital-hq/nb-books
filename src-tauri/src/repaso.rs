//! Lo que cambia en el disco se ve sin pedirlo.
//!
//! Añadir o quitar un archivo en una carpeta referenciada tiene que reflejarse en
//! la biblioteca sin que nadie pida un reescaneo, que es justo lo que Kavita no
//! resuelve. Se decidió **midiendo** entre las dos formas conocidas de enterarse, y
//! la medida está en `docs/architecture/storage.md`, sección 2.2:
//!
//! * Un **vigilante** del sistema de archivos (inotify en Linux) pide una marca por
//!   carpeta: sobre las 18.841 del corpus real, 2 MB de memoria del núcleo que no
//!   se reclama y los 18.841 nodos de índice clavados en caché mientras la
//!   aplicación esté abierta. En reposo no despierta a nadie.
//! * Un **repaso incremental** pregunta por la fecha de modificación de cada
//!   carpeta y solo lista las que cambiaron: entre 61 y 124 ms de procesador por
//!   pasada con las 18.841, sin una lectura de disco con la caché caliente, y cero
//!   memoria en reposo.
//!
//! Ganó el repaso, y no solo por la memoria: **el vigilante no ve lo que cambió con
//! la aplicación cerrada**, así que el repaso haría falta de todas formas al
//! arrancar. Un vigilante habría sido un coste añadido a lo que ya había que hacer.
//!
//! # Cómo se entera sin vigilar
//!
//! Añadir o quitar un archivo cambia la fecha de modificación **de su carpeta**.
//! Así que basta con guardar la de cada carpeta de la raíz y, al repasar, pedirla
//! otra vez: solo las que cambiaron se listan, y solo lo que hay en ellas y no en
//! la base se importa. Mover un archivo dentro de la misma carpeta, o reescribirlo,
//! no cambia nada que la biblioteca pinte.
//!
//! La foto de las carpetas vive en la caché y no en la base: es derivada, se
//! regenera sola, y guardarla en la base habría exigido una migración, que aquí
//! copia la base entera. Si falta -la primera vez, o si alguien vacía la
//! caché- el primer repaso **solo la toma**: no importa lo que ya estaba al
//! activarse, porque no se sabe si faltaba por descuido o por decisión, y
//! convertirlo en una importación masiva callada sería decidir por el usuario.
//!
//! # Lo que no hace
//!
//! No marca como ausente una raíz entera que no se encuentra: un disco
//! desconectado no es una biblioteca borrada, y para revisar dónde está todo
//! existe la acción de Ajustes. No toca los archivos: solo pregunta por ellos.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use rusqlite::Connection;

use crate::db::catalog::{Availability, StorageMode};
use crate::db::Database;
use crate::error::{NbError, NbResult};
use crate::events::{DomainEvent, EventBus};
use crate::library::{import_document, CancelToken, ImportContext, ImportOutcome, ImportRequest};

/// Cuánto tiempo tiene que pasar entre dos repasos pedidos por la interfaz.
///
/// La interfaz pide uno al volver a la ventana y al entrar en la biblioteca o en
/// Inicio, y las dos cosas suelen ocurrir seguidas. Tres segundos bastan para que
/// no se hagan dos pasadas por un solo gesto y no son tantos como para que alguien
/// que acaba de copiar un libro y vuelve no lo vea.
pub const ENTRE_REPASOS: Duration = Duration::from_secs(3);

/// Lo que hizo un repaso.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Informe {
    pub raices: u32,
    pub carpetas: u32,
    /// Carpetas cuya fecha cambió y se listaron.
    pub cambiadas: u32,
    pub importados: u32,
    /// Archivos que la base tenía y ya no están.
    pub ausentes: u32,
    /// Archivos que faltaban y han vuelto.
    pub vueltos: u32,
    /// Raíces cuya foto se tomó por primera vez, sin importar nada.
    pub fotos_nuevas: u32,
}

/// A dónde va lo que el repaso encuentra: el bus que lo anuncia y la carpeta de
/// la biblioteca que pide la importación.
#[derive(Clone, Copy)]
struct Destino<'a> {
    bus: &'a EventBus,
    biblioteca: &'a Path,
}

/// El repaso, con su foto por raíz y su freno.
pub struct Repaso {
    carpeta: PathBuf,
    ultimo: Mutex<Option<Instant>>,
    en_marcha: AtomicBool,
}

/// La fecha de modificación de una carpeta, en nanosegundos, comparable.
fn fecha(meta: &std::fs::Metadata) -> Option<u128> {
    meta.modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_nanos())
}

/// La ruta de una carpeta dentro de la raíz, con `/` como separador, que es como
/// guarda la base las rutas relativas. La raíz misma es la cadena vacía.
fn relativa(raiz: &Path, carpeta: &Path) -> Option<String> {
    let resto = carpeta.strip_prefix(raiz).ok()?;
    Some(resto.to_string_lossy().replace('\\', "/"))
}

fn unir(raiz: &Path, relativa: &str) -> PathBuf {
    if relativa.is_empty() {
        raiz.to_path_buf()
    } else {
        raiz.join(relativa)
    }
}

/// Lo que hay directamente dentro de una carpeta: archivos y subcarpetas, sin
/// seguir enlaces, igual que la importación.
fn listar(carpeta: &Path) -> std::io::Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut archivos = Vec::new();
    let mut subcarpetas = Vec::new();
    for entrada in std::fs::read_dir(carpeta)?.filter_map(Result::ok) {
        let ruta = entrada.path();
        let Ok(meta) = std::fs::symlink_metadata(&ruta) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            subcarpetas.push(ruta);
        } else if meta.is_file() {
            archivos.push(ruta);
        }
    }
    archivos.sort();
    subcarpetas.sort();
    Ok((archivos, subcarpetas))
}

/// La foto de una raíz: la fecha de cada carpeta, por su ruta relativa.
type Foto = HashMap<String, u128>;

fn leer_foto(ruta: &Path) -> Option<Foto> {
    let texto = std::fs::read_to_string(ruta).ok()?;
    let mut foto = Foto::new();
    for linea in texto.lines() {
        let (fecha, carpeta) = linea.split_once('\t')?;
        foto.insert(carpeta.to_owned(), fecha.parse().ok()?);
    }
    Some(foto)
}

fn escribir_foto(ruta: &Path, foto: &Foto) {
    let mut carpetas: Vec<_> = foto.iter().collect();
    carpetas.sort();
    let mut texto = String::with_capacity(carpetas.len() * 40);
    for (carpeta, fecha) in carpetas {
        texto.push_str(&fecha.to_string());
        texto.push('\t');
        texto.push_str(carpeta);
        texto.push('\n');
    }
    // Atómica, como toda escritura de la caché: una foto a medias haría que el
    // siguiente repaso creyera cambiadas carpetas que no cambiaron.
    let escrita = crate::paths::write_atomic(ruta, |temporal| {
        std::fs::write(temporal, &texto).map_err(|_| NbError::StorageFailure)
    });
    if escrita.is_err() {
        // Sin foto el siguiente repaso la vuelve a tomar: se pierde un aviso, no
        // un dato.
        tracing::warn!("no se pudo guardar la foto de las carpetas de una raíz");
    }
}

/// Toma la foto entera de una raíz, recorriendo todas sus carpetas.
fn fotografiar(raiz: &Path) -> Foto {
    let mut foto = Foto::new();
    let mut pendientes = vec![raiz.to_path_buf()];
    while let Some(carpeta) = pendientes.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&carpeta) else {
            continue;
        };
        let (Some(momento), Some(clave)) = (fecha(&meta), relativa(raiz, &carpeta)) else {
            continue;
        };
        foto.insert(clave, momento);
        if let Ok((_, subcarpetas)) = listar(&carpeta) {
            pendientes.extend(subcarpetas);
        }
    }
    foto
}

/// Un archivo de la base que cuelga directamente de una carpeta.
struct Conocido {
    id: String,
    edition_id: String,
    nombre: String,
    disponible: bool,
}

/// Los archivos que la base cree que hay directamente en una carpeta de la raíz.
///
/// Se pide por rango sobre `(root_id, relative_path)`, que es único y por tanto
/// tiene índice: una carpeta de ciento cincuenta mil archivos no se recorre para
/// saber qué hay en una de ellas.
fn conocidos(conn: &Connection, root_id: &str, carpeta: &str) -> NbResult<Vec<Conocido>> {
    let (desde, hasta) = if carpeta.is_empty() {
        (String::new(), "\u{10FFFF}".to_owned())
    } else {
        (format!("{carpeta}/"), format!("{carpeta}0"))
    };
    let mut consulta = conn
        .prepare_cached(
            "SELECT id, edition_id, relative_path, availability FROM files
              WHERE root_id = ?1 AND relative_path >= ?2 AND relative_path < ?3",
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo preparar la consulta del repaso");
            NbError::StorageFailure
        })?;
    let filas = consulta
        .query_map(rusqlite::params![root_id, desde, hasta], |fila| {
            Ok((
                fila.get::<_, String>(0)?,
                fila.get::<_, String>(1)?,
                fila.get::<_, String>(2)?,
                fila.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo leer lo conocido de una carpeta");
            NbError::StorageFailure
        })?;

    let prefijo = desde.len();
    let mut salida = Vec::new();
    for fila in filas.filter_map(Result::ok) {
        let (id, edition_id, ruta, disponibilidad) = fila;
        let nombre = &ruta[prefijo..];
        // Solo los de esta carpeta: los de sus subcarpetas se miran cuando toque
        // a la suya.
        if nombre.contains('/') {
            continue;
        }
        salida.push(Conocido {
            id,
            edition_id,
            nombre: nombre.to_owned(),
            disponible: disponibilidad == Availability::Available.as_str(),
        });
    }
    Ok(salida)
}

/// Las raíces referenciadas que están localizadas.
fn raices(conn: &Connection) -> NbResult<Vec<(String, String)>> {
    let mut consulta = conn
        .prepare(
            "SELECT id, path FROM library_roots WHERE kind = 'referenced' AND path IS NOT NULL",
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudieron listar las raíces");
            NbError::StorageFailure
        })?;
    let filas = consulta
        .query_map([], |fila| Ok((fila.get(0)?, fila.get(1)?)))
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudieron leer las raíces");
            NbError::StorageFailure
        })?;
    Ok(filas.filter_map(Result::ok).collect())
}

impl Repaso {
    /// Un repaso que guarda sus fotos en esa carpeta de la caché.
    #[must_use]
    pub fn nuevo(carpeta: PathBuf) -> Self {
        Self {
            carpeta,
            ultimo: Mutex::new(None),
            en_marcha: AtomicBool::new(false),
        }
    }

    fn ruta_de_foto(&self, root_id: &str) -> PathBuf {
        self.carpeta.join(format!("{root_id}.carpetas"))
    }

    /// Borra la foto guardada de una raíz para que el siguiente repaso la tome
    /// de nuevo desde su nueva ubicación.
    pub fn olvidar_foto(&self, root_id: &str) {
        let _ = std::fs::remove_file(self.ruta_de_foto(root_id));
    }

    /// Repasa si no se repasó hace un momento y no hay otro en marcha.
    ///
    /// Devuelve `None` cuando no repasa: una importación en curso, otro repaso en
    /// marcha o uno hace menos de [`ENTRE_REPASOS`]. Una importación ya recorre las
    /// carpetas que importa, y repasar a la vez sería competir por la misma base.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde. Un archivo que no se
    /// deja importar no es un error del repaso: se registra y se sigue.
    pub fn si_toca(
        &self,
        db: &Database,
        bus: &EventBus,
        biblioteca: &Path,
        importacion: &CancelToken,
    ) -> NbResult<Option<Informe>> {
        if importacion.is_running() {
            return Ok(None);
        }
        {
            let mut ultimo = self.ultimo.lock().map_err(|_| NbError::Internal)?;
            if ultimo.is_some_and(|antes| antes.elapsed() < ENTRE_REPASOS) {
                return Ok(None);
            }
            *ultimo = Some(Instant::now());
        }
        if self.en_marcha.swap(true, Ordering::SeqCst) {
            return Ok(None);
        }
        let resultado = self.repasar(db, bus, biblioteca);
        self.en_marcha.store(false, Ordering::SeqCst);
        resultado.map(Some)
    }

    /// Repasa todas las raíces referenciadas, sin freno.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn repasar(&self, db: &Database, bus: &EventBus, biblioteca: &Path) -> NbResult<Informe> {
        let mut conn = db.connection()?;
        let mut informe = Informe::default();
        for (root_id, ruta) in raices(&conn)? {
            let raiz = PathBuf::from(&ruta);
            // Una raíz que no está no se toca: puede ser un disco desconectado.
            if !raiz.is_dir() {
                continue;
            }
            informe.raices += 1;
            self.repasar_raiz(
                &mut conn,
                &Destino { bus, biblioteca },
                (&root_id, &raiz),
                &mut informe,
            )?;
        }
        tracing::info!(
            raices = informe.raices,
            carpetas = informe.carpetas,
            cambiadas = informe.cambiadas,
            importados = informe.importados,
            ausentes = informe.ausentes,
            vueltos = informe.vueltos,
            "repaso de las raíces terminado"
        );
        Ok(informe)
    }

    fn repasar_raiz(
        &self,
        conn: &mut Connection,
        destino: &Destino<'_>,
        (root_id, raiz): (&str, &Path),
        informe: &mut Informe,
    ) -> NbResult<()> {
        let Destino { bus, biblioteca } = *destino;
        let ruta_de_foto = self.ruta_de_foto(root_id);
        let Some(mut foto) = leer_foto(&ruta_de_foto) else {
            // La primera vez solo se mira: ver el comentario del módulo.
            let foto = fotografiar(raiz);
            informe.carpetas += u32::try_from(foto.len()).unwrap_or(u32::MAX);
            informe.fotos_nuevas += 1;
            escribir_foto(&ruta_de_foto, &foto);
            return Ok(());
        };

        let mut cambio = false;
        // Las carpetas se miran en orden estable, para que un repaso se pueda
        // reproducir cuando falla en la carpeta número ochocientos.
        let mut pendientes: Vec<String> = foto.keys().cloned().collect();
        pendientes.sort();
        let mut vistas = HashSet::new();

        while let Some(clave) = pendientes.pop() {
            if !vistas.insert(clave.clone()) {
                continue;
            }
            informe.carpetas += 1;
            let carpeta = unir(raiz, &clave);
            let meta = std::fs::symlink_metadata(&carpeta)
                .ok()
                .filter(|m| m.is_dir());
            let antes = foto.get(&clave).copied();
            let ahora = meta.as_ref().and_then(fecha);

            if ahora.is_some() && ahora == antes {
                continue;
            }
            informe.cambiadas += 1;
            cambio = true;

            let Some(ahora) = ahora else {
                // La carpeta ya no está: lo que la base tenía en ella, falta.
                foto.remove(&clave);
                for conocido in conocidos(conn, root_id, &clave)? {
                    marcar(conn, bus, &conocido, Availability::Missing, informe)?;
                }
                continue;
            };
            foto.insert(clave.clone(), ahora);

            let Ok((archivos, subcarpetas)) = listar(&carpeta) else {
                continue;
            };
            // Una subcarpeta nueva todavía no está en la foto: se mira en esta
            // misma pasada, y su fecha no puede coincidir con ninguna guardada.
            for sub in subcarpetas {
                if let Some(rel) = relativa(raiz, &sub) {
                    if !foto.contains_key(&rel) {
                        foto.insert(rel.clone(), 0);
                        pendientes.push(rel);
                    }
                }
            }

            let presentes: HashSet<String> = archivos
                .iter()
                .filter_map(|a| a.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect();
            let sabidos = conocidos(conn, root_id, &clave)?;
            let nombres: HashSet<&str> = sabidos.iter().map(|c| c.nombre.as_str()).collect();

            for conocido in &sabidos {
                match (presentes.contains(&conocido.nombre), conocido.disponible) {
                    (false, true) => marcar(conn, bus, conocido, Availability::Missing, informe)?,
                    (true, false) => {
                        marcar(conn, bus, conocido, Availability::Available, informe)?;
                    }
                    _ => {}
                }
            }

            for archivo in &archivos {
                let Some(nombre) = archivo.file_name().map(|n| n.to_string_lossy()) else {
                    continue;
                };
                if nombres.contains(nombre.as_ref()) {
                    continue;
                }
                let ctx = ImportContext {
                    scope: raiz,
                    library: biblioteca,
                    bus,
                };
                match import_document(
                    conn,
                    &ctx,
                    &ImportRequest {
                        source: archivo.clone(),
                        storage_mode: StorageMode::Referenced,
                        root: Some(raiz.to_path_buf()),
                    },
                ) {
                    Ok(ImportOutcome::Imported(_)) => informe.importados += 1,
                    // Una copia exacta que aparece en una raíz no entra, igual
                    // que al importar; el repaso no tiene informe que enseñar, y
                    // la copia se cuenta la próxima vez que se importe la carpeta.
                    Ok(ImportOutcome::AlreadyInLibrary(_) | ImportOutcome::Duplicate(_)) => {}
                    // Lo que no es un documento -una portada suelta, un `.opf`-
                    // se rechaza aquí igual que en una importación, y se vuelve a
                    // mirar solo si su carpeta cambia otra vez.
                    Err(error) => tracing::debug!(
                        codigo = error.code(),
                        "el repaso encontró un archivo que no se importa"
                    ),
                }
            }
        }

        if cambio {
            escribir_foto(&ruta_de_foto, &foto);
        }
        Ok(())
    }
}

fn marcar(
    conn: &Connection,
    bus: &EventBus,
    conocido: &Conocido,
    estado: Availability,
    informe: &mut Informe,
) -> NbResult<()> {
    crate::db::catalog::File::set_availability(conn, &conocido.id, estado)?;
    if estado == Availability::Available {
        informe.vueltos += 1;
    } else {
        informe.ausentes += 1;
    }
    bus.emit(DomainEvent::AvailabilityChanged {
        edition_id: conocido.edition_id.clone(),
    });
    Ok(())
}
