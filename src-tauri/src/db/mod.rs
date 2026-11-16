//! Capa de persistencia local.
//!
//! SQLite es la fuente de verdad de NB Books. Toda conexión se configura igual,
//! y esa configuración no es opcional: el modo WAL permite lecturas concurrentes
//! sin bloquear escrituras, y su ausencia produce el error `database is locked`
//! que el usuario percibe como una aplicación colgada.
//!
//! Contratos: `docs/architecture/api/schemas.md` y
//! `docs/adr/002-critical-bottlenecks.md`.

pub mod catalog;
pub mod migrations;

use std::path::{Path, PathBuf};
use std::time::Duration;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

use crate::error::{NbError, NbResult};

/// Tiempo que una conexión espera a que se libere un bloqueo antes de fallar.
pub(crate) const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// A partir de cuánto esperar por una conexión se apunta en el registro.
const ESPERA_QUE_SE_DICE: Duration = Duration::from_millis(50);

/// Número máximo de conexiones simultáneas.
///
/// Los comandos corren fuera del hilo de la ventana, en paralelo, y a la vez
/// trabajan el indexador, los metadatos, los vectores y el audio preparado: con
/// un número fijo pequeño, cuatro tareas de fondo bastaban para que la interfaz
/// esperase. En WAL las lecturas no se bloquean entre sí y solo hay un escritor,
/// así que más conexiones no añaden contención de escritura; cada una cuesta su
/// caché de páginas. Se dimensiona por el paralelismo del equipo, con un suelo
/// para que una ráfaga de arranque no espere y un techo para la memoria.
fn max_connections() -> u32 {
    let nucleos = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get);
    u32::try_from(nucleos)
        .unwrap_or(4)
        .clamp(MIN_CONNECTIONS, MAX_CONNECTIONS)
}

/// Suelo del pool: la interfaz y las cuatro tareas de fondo a la vez, con sitio.
const MIN_CONNECTIONS: u32 = 6;
/// Techo del pool: cada conexión lleva su caché de páginas.
const MAX_CONNECTIONS: u32 = 12;

/// Aplica la configuración obligatoria a una conexión del pool.
///
/// Se ejecuta **en cada conexión**, no una sola vez al crear el archivo:
/// `foreign_keys`, `synchronous` y `busy_timeout` son propiedades de la
/// conexión, no del archivo, y SQLite las restablece en cada una.
///
/// Deliberadamente **no** toca `journal_mode`: ver [`prepare_file`].
fn configure(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    // Lo primero, para que cualquier contención posterior espere en lugar de
    // fallar de inmediato.
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Correcto y suficiente en modo WAL; `FULL` cuesta un fsync por escritura.
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    // SQLite las desactiva por defecto. Sin esto, las claves foráneas del
    // esquema son documentación decorativa.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

/// Fija el modo WAL en el archivo, con una única conexión, antes de crear el pool.
///
/// `journal_mode` es una propiedad **persistente del archivo**, no de la
/// conexión: basta con fijarla una vez. Hacerlo desde el inicializador del pool
/// provoca que varias conexiones intenten cambiar el modo del diario a la vez
/// sobre una base recién creada, lo que produce `database is locked` en cada
/// arranque en frío. r2d2 reintenta y acaba funcionando, pero deja un error en
/// el registro y varios cientos de milisegundos de retraso contra un
/// presupuesto de arranque de 200 ms.
///
/// # Errors
///
/// Devuelve [`NbError::StorageFailure`] si el archivo no se puede abrir o si el
/// modo WAL no queda activo.
fn prepare_file(path: &Path) -> NbResult<()> {
    let conn = Connection::open(path).map_err(|error| {
        tracing::error!(error = %error, "no se pudo abrir la base de datos");
        NbError::StorageFailure
    })?;
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|_| NbError::StorageFailure)?;

    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo activar el modo WAL");
            NbError::StorageFailure
        })?;

    if !mode.eq_ignore_ascii_case("wal") {
        tracing::error!(modo = %mode, "el modo WAL no quedó activo");
        return Err(NbError::StorageFailure);
    }

    Ok(())
}

/// Abre una transacción **de escritura**.
///
/// Toda transacción que vaya a escribir se abre con `BEGIN IMMEDIATE`, y no con
/// el `BEGIN DEFERRED` que `Connection::transaction` usa por defecto. La razón
/// no es de estilo:
///
/// En modo WAL, una transacción diferida que empieza leyendo y **después**
/// escribe tiene que ascender su bloqueo de lectura a bloqueo de escritura. Si
/// entre medias otro escritor confirmó algo, SQLite devuelve `SQLITE_BUSY`
/// **de inmediato y sin respetar `busy_timeout`**, porque reintentar dentro de
/// la misma transacción daría una lectura incoherente. `busy_timeout` solo se
/// aplica a quien todavía no ha leído nada.
///
/// El efecto medido antes de arreglarlo: importando una biblioteca real, con la
/// indexación y la extracción de metadatos corriendo a la vez, el registro
/// acumuló **12.156 «database is locked»** en una sola sesión. Los metadatos no
/// llegaban a aplicarse —por eso los títulos se quedaban con el nombre del
/// archivo—, y la interfaz se quedaba esperando.
///
/// `BEGIN IMMEDIATE` toma el bloqueo de escritura desde la primera instrucción.
/// Ahí sí manda `busy_timeout`: quien llega segundo espera su turno en lugar de
/// fallar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede abrir la transacción.
pub fn write_transaction(conn: &mut Connection) -> NbResult<rusqlite::Transaction<'_>> {
    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo abrir una transacción de escritura");
            NbError::StorageFailure
        })
}

/// Abre una lectura que ve **una sola foto** de la base.
///
/// Varias consultas seguidas y sin transacción ven cada una la base del momento
/// en que empiezan. Con la indexación o una importación escribiendo a la vez, eso
/// da resultados que no casan entre sí: una exportación con anotaciones de una
/// edición que llegó después de leer las ediciones, que al restaurarse fallaría
/// por la clave ajena justo el día que hace falta.
///
/// En WAL, una transacción que **solo lee** fija su foto en la primera consulta y
/// no bloquea a ningún escritor. No es la diferida que prohíbe
/// [`write_transaction`]: aquella falla al ascender a escritura, y esta no escribe
/// nunca. Se descarta al soltarla, sin confirmar nada.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede abrir la transacción.
pub fn read_snapshot(conn: &mut Connection) -> NbResult<rusqlite::Transaction<'_>> {
    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo abrir una lectura coherente");
            NbError::StorageFailure
        })
}

/// Acceso a la base de datos local.
#[derive(Clone)]
pub struct Database {
    pool: Pool<SqliteConnectionManager>,
    path: Option<PathBuf>,
}

impl Database {
    /// Abre la base de datos en `path`, la configura y aplica las migraciones
    /// pendientes.
    ///
    /// # Errors
    ///
    /// - [`NbError::SchemaMismatch`] si la base es más nueva que esta versión.
    /// - [`NbError::StorageFailure`] si no se puede abrir o migrar.
    pub fn open(path: &Path) -> NbResult<Self> {
        prepare_file(path)?;
        let manager = SqliteConnectionManager::file(path).with_init(configure);
        Self::build(manager, Some(path.to_path_buf()))
    }

    /// Abre una base en memoria compartida por el pool. Solo para pruebas.
    ///
    /// # Errors
    ///
    /// Devuelve [`NbError::StorageFailure`] si no se puede inicializar.
    pub fn open_in_memory() -> NbResult<Self> {
        /*
         * La base en memoria se comparte entre las conexiones del pool con caché
         * compartida, y ahí un lector bloquea **la tabla** a quien escribe: el
         * escritor recibe `SQLITE_LOCKED` en el acto, sin pasar por
         * `busy_timeout`. En el archivo real, en WAL, un lector nunca estorba a
         * un escritor. Sin esto, una prueba que escribe mientras otro hilo lee la
         * misma tabla falla una de cada cincuenta veces por un bloqueo que la
         * aplicación no puede tener: pasó con `flush_progress` el 2026-11-14.
         * `read_uncommitted` quita ese bloqueo de lectura, que es lo que más se
         * parece a WAL de lo que la caché compartida ofrece. Se descartó
         * reintentar en la prueba: tapaba el síntoma en una y lo dejaba en las
         * demás.
         */
        let manager = SqliteConnectionManager::memory().with_init(|conn| {
            configure(conn)?;
            conn.pragma_update(None, "read_uncommitted", true)
        });
        Self::build(manager, None)
    }

    fn build(manager: SqliteConnectionManager, path: Option<PathBuf>) -> NbResult<Self> {
        let pool = Pool::builder()
            .max_size(max_connections())
            // Sin conexiones ociosas previas: se crean cuando se piden. En una
            // aplicación de escritorio no hay carga que justifique mantenerlas
            // abiertas, y crearlas en paralelo al arrancar solo genera contención.
            .min_idle(Some(0))
            .connection_timeout(BUSY_TIMEOUT)
            .build(manager)
            .map_err(|error| {
                tracing::error!(error = %error, "no se pudo inicializar el pool de conexiones");
                NbError::StorageFailure
            })?;

        let db = Self { pool, path };

        let mut conn = db.connection()?;
        migrations::run(&mut conn, db.path.as_deref())?;

        // La ruta de la raíz administrada es una caché de dónde está instalada
        // la aplicación, no un dato del catálogo. Se reescribe aquí, en cada
        // apertura, para que mover o reinstalar la aplicación se corrija solo.
        // Ver docs/architecture/storage.md, sección 2.1.
        if let Some(library) = db
            .path
            .as_deref()
            .and_then(crate::paths::library_beside_database)
        {
            // Un fallo aquí no impide abrir: es una caché, y el siguiente
            // arranque vuelve a intentarlo. Abortar por no poder refrescarla
            // convertiría una comodidad en un motivo de no arrancar.
            if let Err(error) =
                catalog::LibraryRoot::sync_managed(&conn, &library.to_string_lossy())
            {
                tracing::warn!(error = %error, "no se pudo refrescar la raíz administrada");
            }
        }

        drop(conn);

        Ok(db)
    }

    /// Ruta del archivo de la base, o `None` si vive en memoria.
    #[must_use]
    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// Toma una conexión del pool.
    ///
    /// # Errors
    ///
    /// Devuelve [`NbError::StorageFailure`] si el pool está agotado o la
    /// conexión no se puede establecer.
    pub fn connection(&self) -> NbResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        let inicio = std::time::Instant::now();
        let conexion = self.pool.get().map_err(|error| {
            tracing::error!(error = %error, "no se pudo obtener una conexión del pool");
            NbError::StorageFailure
        })?;
        /*
         * **Esperar por una conexión se dice**.
         *
         * Desde que los comandos dejaron el hilo de la ventana, el pool no
         * serializa nada: es una cola, y quien no encuentra sitio espera. Medido con la
         * ráfaga de arranque de la interfaz -41 órdenes a la vez- sobre una copia de la
         * base real, con cuatro conexiones la peor espera fue de 57 ms y la ráfaga entera
         * de 280 (performance-budgets 4.24): bastan. Pero la ráfaga no es el uso, y el
         * uso de verdad es el que decide. Por encima de lo que se nota -unos tres
         * fotogramas-, queda en el registro con cuántas estaban en uso.
         */
        let espera = inicio.elapsed();
        if espera >= ESPERA_QUE_SE_DICE {
            let estado = self.pool.state();
            tracing::warn!(
                ms = espera.as_millis(),
                en_uso = estado.connections - estado.idle_connections,
                "se esperó por una conexión del pool"
            );
        }
        Ok(conexion)
    }

    /// Versión de esquema aplicada.
    ///
    /// # Errors
    ///
    /// Devuelve [`NbError::StorageFailure`] si no se puede consultar.
    pub fn schema_version(&self) -> NbResult<u32> {
        let conn = self.connection()?;
        migrations::current_version(&conn)
    }
}

/// Cuánto espacio libre dentro del archivo justifica compactarlo.
///
/// Una cuarta parte, y nunca por menos de 64 MB. Por debajo de eso, compactar
/// cuesta más -reescribe la base entera y necesita sitio para una copia- de lo
/// que devuelve.
const HUECO_QUE_MERECE_COMPACTAR: f64 = 0.25;
const MINIMO_PARA_COMPACTAR: u64 = 64 * 1024 * 1024;

/// Devuelve al sistema el hueco que la base tiene dentro.
///
/// # Por qué hace falta
///
/// Porque SQLite **no encoge el archivo al borrar**: marca las páginas libres y
/// las reutiliza más tarde. Normalmente da igual; deja de darlo cuando una
/// migración libera de golpe la mayor parte del contenido, que es exactamente lo
/// que hace V27 al quitar el texto de `text_chunks`.
///
/// Medido el 2026-09-10 sobre la instalación real, después de esa migración: la
/// base ocupaba **111 MB en disco y 33 en uso**; 77 MB eran hueco. Y como el
/// presupuesto de lo derivado se mide sobre el **archivo** -que es lo que de
/// verdad le quita sitio a quien lo tiene-, el freno del indexador se disparaba
/// al 96,88 % contando espacio que ya no se usaba.
///
/// Se hace aquí y no en la migración a propósito: `VACUUM` no puede correr
/// dentro de una transacción, y las migraciones van dentro de una.
///
/// # Errors
///
/// No devuelve error: no poder compactar no es un fallo del que haya que
/// avisar a nadie. Se registra y se sigue.
pub fn compactar_si_sobra_hueco(conn: &Connection) {
    compactar_con_minimo(conn, MINIMO_PARA_COMPACTAR);
}

/// Lo mismo, con el mínimo como parámetro.
///
/// Existe para poder comprobarlo sin fabricar una base de cien megabytes: lo que
/// hay que verificar es que el hueco vuelve al disco, y eso se ve igual de bien
/// con dos megabytes que con cien. El mínimo de verdad lo pone quien llama.
fn compactar_con_minimo(conn: &Connection, minimo: u64) {
    let paginas: i64 = conn
        .query_row("PRAGMA page_count", [], |f| f.get(0))
        .unwrap_or(0);
    let libres: i64 = conn
        .query_row("PRAGMA freelist_count", [], |f| f.get(0))
        .unwrap_or(0);
    let tamano: i64 = conn
        .query_row("PRAGMA page_size", [], |f| f.get(0))
        .unwrap_or(0);

    if paginas <= 0 || libres <= 0 || tamano <= 0 {
        return;
    }

    let hueco = u64::try_from(libres * tamano).unwrap_or(0);
    let proporcion = libres as f64 / paginas as f64;

    if hueco < minimo || proporcion < HUECO_QUE_MERECE_COMPACTAR {
        return;
    }

    let reloj = std::time::Instant::now();

    /*
     * **Y después se consolida el diario, o no sirve de nada.**
     *
     * En WAL, `VACUUM` reconstruye la base dentro del diario y el archivo
     * principal no encoge hasta que alguien lo pasa: sin esta línea, compactar
     * devolvía `Ok` y el disco seguía ocupado igual. Se vio al escribir la
     * prueba, y habría pasado desapercibido en producción porque la operación
     * no falla.
     */
    match conn.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);") {
        Ok(()) => tracing::info!(
            megabytes = hueco / (1024 * 1024),
            ms = reloj.elapsed().as_millis(),
            "base compactada: el hueco vuelve al disco"
        ),
        Err(error) => tracing::warn!(error = %error, "no se pudo compactar la base"),
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn el_pool_deja_sitio_a_la_interfaz_con_el_fondo_ocupado() {
        let n = super::max_connections();
        assert!((super::MIN_CONNECTIONS..=super::MAX_CONNECTIONS).contains(&n));
        // Las cuatro tareas de fondo (índice, metadatos, vectores, audio) más la
        // interfaz tienen que caber sin esperar.
        assert!(n >= 5);
    }

    use super::*;

    /// Los pragmas son por conexión: si esta prueba falla, una conexión tomada
    /// más tarde del pool podría no tener claves foráneas activas.
    #[test]
    fn cada_conexion_del_pool_lleva_la_configuracion_obligatoria() {
        let db = Database::open_in_memory().expect("base en memoria");

        for _ in 0..3 {
            let conn = db.connection().expect("conexión");

            let foreign_keys: i64 = conn
                .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
                .expect("consulta");
            assert_eq!(foreign_keys, 1, "las claves foráneas deben estar activas");

            let synchronous: i64 = conn
                .query_row("PRAGMA synchronous", [], |r| r.get(0))
                .expect("consulta");
            assert_eq!(synchronous, 1, "synchronous debe ser NORMAL");
        }
    }

    #[test]
    fn abrir_aplica_las_migraciones() {
        let db = Database::open_in_memory().expect("base en memoria");
        assert_eq!(
            db.schema_version().expect("legible"),
            migrations::target_version()
        );
    }

    #[test]
    fn una_base_en_archivo_usa_el_modo_wal() {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-books-wal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("directorio");
        let path = dir.join("prueba.sqlite");

        let db = Database::open(&path).expect("base en archivo");
        let conn = db.connection().expect("conexión");
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("consulta");

        assert_eq!(mode.to_lowercase(), "wal");

        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// El objetivo real del modo WAL: escrituras concurrentes desde varios hilos
    /// sin que aparezca `database is locked`.
    #[test]
    fn las_escrituras_concurrentes_no_se_bloquean() {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-books-conc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("directorio");
        let path = dir.join("prueba.sqlite");

        let db = Database::open(&path).expect("base en archivo");

        std::thread::scope(|scope| {
            for hilo in 0..4 {
                let db = db.clone();
                scope.spawn(move || {
                    for i in 0..25 {
                        let conn = db.connection().expect("conexión");
                        conn.execute(
                            "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, ?2)",
                            (format!("hilo_{hilo}_{i}"), "x"),
                        )
                        .expect("escritura sin bloqueo visible");
                    }
                });
            }
        });

        let conn = db.connection().expect("conexión");
        let filas: i64 = conn
            .query_row(
                "SELECT count(*) FROM app_metadata WHERE key LIKE 'hilo_%'",
                [],
                |r| r.get(0),
            )
            .expect("consulta");
        assert_eq!(filas, 100);

        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Regresión.** El hueco que deja una migración vuelve al disco.
    ///
    /// SQLite no encoge el archivo al borrar: marca páginas libres. Da igual
    /// hasta que una migración libera la mayor parte del contenido -V27, al
    /// quitar el texto- y entonces el archivo se queda con el tamaño de antes.
    /// Medido sobre la instalación real: 111 MB en disco, 33 en uso. Y como el
    /// presupuesto de lo derivado se mide sobre el archivo, el freno del
    /// indexador se disparaba contando ese hueco.
    #[test]
    fn el_hueco_grande_de_la_base_vuelve_al_disco() {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-books-hueco-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("directorio");
        let path = dir.join("prueba.sqlite");
        let _ = std::fs::remove_file(&path);

        let db = Database::open(&path).expect("base");
        let conn = db.connection().expect("conexión");
        // Un par de megabytes: bastante para que el hueco se note.
        conn.execute_batch(
            "CREATE TABLE bulto (id INTEGER PRIMARY KEY, relleno BLOB);
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<400)
             INSERT INTO bulto (relleno) SELECT randomblob(6000) FROM n;",
        )
        .expect("bulto");

        /*
         * El diario se consolida antes de medir. En WAL lo escrito vive en el
         * archivo `-wal` hasta que alguien lo pasa a la base, así que medir sin
         * esto daría los 4 KB de una base recién creada y la comparación no
         * significaría nada. Pasó al escribir esta prueba.
         */
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .expect("consolidar");
        let antes = std::fs::metadata(&path).expect("tamaño").len();
        assert!(antes > 1_000_000, "el bulto tenía que estar ya en la base");

        conn.execute_batch("DELETE FROM bulto; PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("borrado");
        assert_eq!(
            std::fs::metadata(&path).expect("tamaño").len(),
            antes,
            "borrar no encoge el archivo: ese es justamente el defecto"
        );

        // Con un mínimo pequeño, que es lo que permite probarlo sin fabricar
        // cien megabytes.
        compactar_con_minimo(&conn, 1_000_000);

        let despues = std::fs::metadata(&path).expect("tamaño").len();
        assert!(
            despues < antes / 2,
            "la base ocupaba {antes} y sigue ocupando {despues}: el hueco no volvió al disco"
        );

        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Reabrir una base existente no debe reaplicar migraciones ni perder datos.
    #[test]
    fn reabrir_conserva_los_datos() {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-books-reabrir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("directorio");
        let path = dir.join("prueba.sqlite");

        {
            let db = Database::open(&path).expect("primera apertura");
            let conn = db.connection().expect("conexión");
            conn.execute(
                "INSERT INTO app_metadata (key, value) VALUES ('prueba', 'valor')",
                [],
            )
            .expect("escritura");
        }

        let db = Database::open(&path).expect("segunda apertura");
        let conn = db.connection().expect("conexión");
        let valor: String = conn
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'prueba'",
                [],
                |r| r.get(0),
            )
            .expect("el dato debe sobrevivir");
        assert_eq!(valor, "valor");

        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
