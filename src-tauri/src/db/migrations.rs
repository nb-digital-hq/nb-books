//! Migraciones de esquema.
//!
//! En una aplicación de escritorio la base de datos vive en la máquina del
//! usuario y no tenemos acceso a ella. Si una migración falla, esa persona
//! pierde su biblioteca. De ahí las cuatro reglas de este módulo:
//!
//! 1. **Embebidas en el binario.** Los archivos `.sql` se incrustan en tiempo de
//!    compilación; la aplicación nunca depende de archivos externos.
//! 2. **Transaccionales.** Cada migración se aplica entera o no se aplica.
//! 3. **Solo hacia adelante.** No hay reversión: en escritorio, degradar un
//!    esquema casi siempre significa perder datos.
//! 4. **Respaldo previo.** Antes de tocar una base con datos se guarda todo lo que
//!    no se puede rehacer; el índice del texto va vacío.
//!
//! Además, una versión antigua de la aplicación que encuentra una base de datos
//! más nueva **se niega a operar** en lugar de intentar leerla.
//!
//! Contrato: `docs/architecture/migrations.md`.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{NbError, NbResult};

/// Una migración: número de versión, nombre legible y su SQL embebido.
struct Migration {
    version: u32,
    name: &'static str,
    sql: &'static str,
}

/// Migraciones conocidas, en orden estricto.
///
/// **Nunca se edita una entrada ya publicada.** Corregir un error en una
/// migración aplicada significa añadir una migración nueva.
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "app_metadata",
        sql: include_str!("../../migrations/V1__app_metadata.sql"),
    },
    Migration {
        version: 2,
        name: "bibliographic_core",
        sql: include_str!("../../migrations/V2__bibliographic_core.sql"),
    },
    Migration {
        version: 3,
        name: "metadata_provenance",
        sql: include_str!("../../migrations/V3__metadata_provenance.sql"),
    },
    Migration {
        version: 4,
        name: "settings",
        sql: include_str!("../../migrations/V4__settings.sql"),
    },
    Migration {
        version: 5,
        name: "collections",
        sql: include_str!("../../migrations/V5__collections.sql"),
    },
    Migration {
        version: 6,
        name: "progress",
        sql: include_str!("../../migrations/V6__progress.sql"),
    },
    Migration {
        version: 7,
        name: "immutable_annotations",
        sql: include_str!("../../migrations/V7__immutable_annotations.sql"),
    },
    Migration {
        version: 8,
        name: "search",
        sql: include_str!("../../migrations/V8__search.sql"),
    },
    Migration {
        version: 9,
        name: "search_status",
        sql: include_str!("../../migrations/V9__search_status.sql"),
    },
    Migration {
        version: 10,
        name: "catalog_search",
        sql: include_str!("../../migrations/V10__catalog_search.sql"),
    },
    Migration {
        version: 11,
        name: "library_roots",
        sql: include_str!("../../migrations/V11__library_roots.sql"),
    },
    Migration {
        version: 12,
        name: "linked_notes",
        sql: include_str!("../../migrations/V12__linked_notes.sql"),
    },
    Migration {
        version: 13,
        name: "network_gate",
        sql: include_str!("../../migrations/V13__network_gate.sql"),
    },
    Migration {
        version: 14,
        name: "cover_path",
        sql: include_str!("../../migrations/V14__cover_path.sql"),
    },
    Migration {
        version: 15,
        name: "vocabulary",
        sql: include_str!("../../migrations/V15__vocabulary.sql"),
    },
    Migration {
        version: 16,
        name: "catalogs",
        sql: include_str!("../../migrations/V16__catalogs.sql"),
    },
    Migration {
        version: 17,
        name: "browse_order",
        sql: include_str!("../../migrations/V17__browse_order.sql"),
    },
    Migration {
        version: 18,
        name: "import_runs",
        sql: include_str!("../../migrations/V18__import_runs.sql"),
    },
    Migration {
        version: 19,
        name: "author_sort",
        sql: include_str!("../../migrations/V19__author_sort.sql"),
    },
    Migration {
        version: 20,
        name: "marcas_nuevas",
        sql: include_str!("../../migrations/V20__marcas_nuevas.sql"),
    },
    Migration {
        version: 21,
        name: "extension_origin",
        sql: include_str!("../../migrations/V21__extension_origin.sql"),
    },
    Migration {
        version: 22,
        name: "extension_withdrawals",
        sql: include_str!("../../migrations/V22__extension_withdrawals.sql"),
    },
    Migration {
        version: 23,
        name: "note_boards",
        sql: include_str!("../../migrations/V23__note_boards.sql"),
    },
    Migration {
        version: 24,
        name: "estanterias",
        sql: include_str!("../../migrations/V24__estanterias.sql"),
    },
    Migration {
        version: 25,
        name: "estado_de_busqueda",
        sql: include_str!("../../migrations/V25__estado_de_busqueda.sql"),
    },
    Migration {
        version: 26,
        name: "fragmento_mayor",
        sql: include_str!("../../migrations/V26__fragmento_mayor.sql"),
    },
    Migration {
        version: 27,
        name: "indice_sin_texto",
        sql: include_str!("../../migrations/V27__indice_sin_texto.sql"),
    },
    Migration {
        version: 28,
        name: "ruta_relativa_por_componentes",
        sql: include_str!("../../migrations/V28__ruta_relativa_por_componentes.sql"),
    },
    Migration {
        version: 29,
        name: "filtros_por_indice",
        sql: include_str!("../../migrations/V29__filtros_por_indice.sql"),
    },
    Migration {
        version: 30,
        name: "catalogos_en_claro",
        sql: include_str!("../../migrations/V30__catalogos_en_claro.sql"),
    },
    Migration {
        version: 31,
        name: "tableros_por_columnas",
        sql: include_str!("../../migrations/V31__tableros_por_columnas.sql"),
    },
    Migration {
        version: 32,
        name: "envios_de_ia",
        sql: include_str!("../../migrations/V32__envios_de_ia.sql"),
    },
    Migration {
        version: 33,
        name: "vectores",
        sql: include_str!("../../migrations/V33__vectores.sql"),
    },
    Migration {
        version: 34,
        name: "estudio_generado",
        sql: include_str!("../../migrations/V34__estudio_generado.sql"),
    },
    Migration {
        version: 35,
        name: "audio_preparado",
        sql: include_str!("../../migrations/V35__audio_preparado.sql"),
    },
    Migration {
        version: 36,
        name: "reportes_de_ia",
        sql: include_str!("../../migrations/V36__reportes_de_ia.sql"),
    },
    Migration {
        version: 37,
        name: "copias_exactas",
        sql: include_str!("../../migrations/V37__copias_exactas.sql"),
    },
    Migration {
        version: 38,
        name: "un_autor_una_fila",
        sql: include_str!("../../migrations/V38__un_autor_una_fila.sql"),
    },
    Migration {
        version: 39,
        name: "titulo_comparable",
        sql: include_str!("../../migrations/V39__titulo_comparable.sql"),
    },
    Migration {
        version: 40,
        name: "calibre_provenance",
        sql: include_str!("../../migrations/V40__calibre_provenance.sql"),
    },
    Migration {
        version: 41,
        name: "repaso_espaciado",
        sql: include_str!("../../migrations/V41__repaso_espaciado.sql"),
    },
    Migration {
        version: 42,
        name: "sinopsis",
        sql: include_str!("../../migrations/V42__sinopsis.sql"),
    },
    Migration {
        version: 43,
        name: "desempate_por_titulo",
        sql: include_str!("../../migrations/V43__desempate_por_titulo.sql"),
    },
    Migration {
        version: 44,
        name: "orden_por_publicacion",
        sql: include_str!("../../migrations/V44__orden_por_publicacion.sql"),
    },
    Migration {
        version: 45,
        name: "estanterias_por_idioma_y_formato",
        sql: include_str!("../../migrations/V45__estanterias_por_idioma_y_formato.sql"),
    },
    Migration {
        version: 46,
        name: "dispositivos_moviles",
        sql: include_str!("../../migrations/V46__dispositivos_moviles.sql"),
    },
];

/// Versión de esquema que esta compilación sabe manejar.
#[must_use]
pub fn target_version() -> u32 {
    target_version_of(MIGRATIONS)
}

fn target_version_of(migrations: &[Migration]) -> u32 {
    migrations.last().map_or(0, |m| m.version)
}

/// Versión de esquema aplicada actualmente en la conexión.
///
/// # Errors
///
/// Devuelve [`NbError::StorageFailure`] si no se puede leer `user_version`.
pub fn current_version(conn: &Connection) -> NbResult<u32> {
    conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map(|v| v.max(0) as u32)
        .map_err(|_| NbError::StorageFailure)
}

/// Aplica las migraciones pendientes.
///
/// `db_path` se usa solo para crear el respaldo previo; puede ser `None` para
/// bases en memoria durante las pruebas.
///
/// # Errors
///
/// - [`NbError::SchemaMismatch`] si la base tiene una versión superior a la que
///   esta compilación conoce. Es el caso de un usuario que abre una versión
///   antigua tras haber usado una nueva: operar sobre ella la corrompería.
/// - [`NbError::StorageFailure`] si una migración falla. La transacción se
///   revierte y el respaldo previo queda en disco.
pub fn run(conn: &mut Connection, db_path: Option<&Path>) -> NbResult<u32> {
    apply(conn, db_path, MIGRATIONS)
}

/// Motor de migraciones, parametrizado por la lista a aplicar.
///
/// La lista es un parámetro y no la constante del módulo por una razón concreta:
/// hasta que exista una segunda migración real, las propiedades que de verdad
/// importan (reversión ante fallo, salto de varias versiones, respaldo previo)
/// solo pueden probarse con listas sintéticas. Probarlas contra el
/// comportamiento de SQLite en lugar de contra este motor daría pruebas que
/// pasan aunque el motor esté roto.
fn apply(conn: &mut Connection, db_path: Option<&Path>, migrations: &[Migration]) -> NbResult<u32> {
    let current = current_version(conn)?;
    let target = target_version_of(migrations);

    if current > target {
        tracing::error!(
            schema_actual = current,
            schema_soportado = target,
            "la base de datos es más nueva que esta versión de la aplicación"
        );
        return Err(NbError::SchemaMismatch);
    }

    if current == target {
        tracing::debug!(schema = current, "esquema al día");
        return Ok(current);
    }

    // Respaldo previo: solo tiene sentido si ya había datos que perder.
    if current > 0 {
        if let Some(path) = db_path {
            backup(path, current)?;
        }
    }

    /*
     * Las claves ajenas se apagan **alrededor** de las migraciones, no dentro.
     *
     * # Por qué, y qué pasa si no
     *
     * SQLite no permite alterar un `CHECK` ni una clave ajena: para cambiarlos
     * hay que reconstruir la tabla —crear la nueva, copiar, soltar la vieja,
     * renombrar—. Y con las claves ajenas activas, soltar la vieja ejecuta un
     * `DELETE` implícito que **dispara los `ON DELETE CASCADE` de quien la
     * referencia**.
     *
     * Medido el 2026-09-06 sobre una base de juguete: la reconstrucción entera
     * dentro de una transacción **confirma sin error** y deja los hijos en cero.
     * Aplicado a `annotations`, eso habría borrado en silencio las citas de notas
     * —los enlaces entre las notas propias de quien lee y los pasajes que citan—
     * reportando éxito. Es exactamente la pérdida que el producto existe para
     * impedir, y de la peor forma posible.
     *
     * `PRAGMA foreign_keys` **no hace nada dentro de una transacción**, así que
     * tiene que estar aquí y no en el SQL de la migración. Es el procedimiento
     * que documenta SQLite, y la otra mitad es la comprobación de abajo: apagar
     * las claves sin comprobar después cambiaría una pérdida ruidosa por una
     * silenciosa, que es peor.
     */
    conn.pragma_update(None, "foreign_keys", "OFF")
        .map_err(|_| NbError::StorageFailure)?;

    let resultado = aplicar_en_orden(conn, migrations, current);

    // Se vuelven a encender pase lo que pase: dejarlas apagadas convertiría el
    // resto de la sesión en una base sin integridad referencial.
    let reencendido = conn
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|_| NbError::StorageFailure);
    resultado?;
    reencendido?;

    // Solo si alguna migración pudo tocar una referencia. La comprobación
    // recorre la base entera: sobre la biblioteca real, 51 GB, eran más de diez
    // minutos sin ventana tras actualizar para añadir una columna y unos índices,
    // que no pueden romper ninguna clave ajena.
    let pendientes = migrations.iter().filter(|m| m.version > current);
    if pendientes.clone().any(|m| !solo_anade(m.sql)) {
        comprobar_referencias(conn)?;
    }

    // La cuenta de sesiones que dan por bueno el esquema nuevo empieza de cero:
    // el respaldo recién escrito es el que protege esta migración.
    let _ = conn.execute(
        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, '0')",
        [SESIONES_TRAS_MIGRAR],
    );

    tracing::info!(schema = target, "esquema actualizado");
    Ok(target)
}

/// Clave de `app_metadata` con cuántas sesiones han terminado bien desde la última
/// migración.
const SESIONES_TRAS_MIGRAR: &str = "sesiones_limpias_tras_migrar";

/// Cuántas sesiones limpias dan por bueno un esquema migrado.
///
/// El respaldo protege una sola cosa: que la migración haya dejado mal la base.
/// Eso se nota en las primeras sesiones, que leen y escriben sobre el esquema
/// nuevo; tres que terminan bien bastan para darlo por bueno, y desde entonces el
/// respaldo solo ocupa disco y cuenta contra el tope de lo derivado.
pub const SESIONES_PARA_RETIRAR_RESPALDO: u32 = 3;

/// Cuenta una sesión limpia sobre el esquema actual y, si ya son bastantes,
/// retira el respaldo previo a la migración.
///
/// Se llama al arrancar con cómo terminó la sesión anterior: solo una sesión que
/// cerró bien cuenta. Devuelve si se retiró algo. No falla: que sobre un respaldo
/// es disco desperdiciado, no un motivo para no arrancar.
pub fn retirar_respaldo_si_ya_no_hace_falta(
    conn: &Connection,
    db_path: &Path,
    anterior_limpia: bool,
) -> bool {
    let cuenta: u32 = conn
        .query_row(
            "SELECT value FROM app_metadata WHERE key = ?1",
            [SESIONES_TRAS_MIGRAR],
            |f| f.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SESIONES_PARA_RETIRAR_RESPALDO);
    let cuenta = if anterior_limpia {
        cuenta.saturating_add(1)
    } else {
        cuenta
    };
    let _ = conn.execute(
        "INSERT OR REPLACE INTO app_metadata (key, value) VALUES (?1, ?2)",
        rusqlite::params![SESIONES_TRAS_MIGRAR, cuenta.to_string()],
    );
    if cuenta < SESIONES_PARA_RETIRAR_RESPALDO {
        return false;
    }
    let mut retirados = false;
    if let (Some(directorio), Some(nombre)) = (db_path.parent(), db_path.file_name()) {
        let prefijo = format!("{}.v", nombre.to_string_lossy());
        for entrada in std::fs::read_dir(directorio)
            .into_iter()
            .flatten()
            .flatten()
        {
            let nombre = entrada.file_name();
            let nombre = nombre.to_string_lossy();
            if nombre.starts_with(&prefijo)
                && nombre.ends_with(".backup")
                && std::fs::remove_file(entrada.path()).is_ok()
            {
                retirados = true;
            }
        }
    }
    if retirados {
        tracing::info!(
            sesiones = cuenta,
            "respaldo previo a la migración retirado: el esquema ya se dio por bueno"
        );
    }
    retirados
}

/// Aplica las migraciones pendientes, cada una en su transacción.
fn aplicar_en_orden(
    conn: &mut rusqlite::Connection,
    migrations: &[Migration],
    current: u32,
) -> NbResult<()> {
    for migration in migrations.iter().filter(|m| m.version > current) {
        tracing::info!(
            version = migration.version,
            name = migration.name,
            "aplicando migración"
        );

        let tx = super::write_transaction(conn)?;
        tx.execute_batch(migration.sql).map_err(|error| {
            tracing::error!(
                version = migration.version,
                name = migration.name,
                error = %error,
                "la migración falló; se revierte"
            );
            NbError::StorageFailure
        })?;
        // `user_version` no admite parámetros ligados.
        tx.pragma_update(None, "user_version", migration.version)
            .map_err(|_| NbError::StorageFailure)?;
        tx.commit().map_err(|_| NbError::StorageFailure)?;
    }
    Ok(())
}

/// Si una migración solo añade: columnas, índices o quitar índices. Nada de eso
/// puede dejar una referencia rota, así que no hace falta recorrer la base.
///
/// Se lee el SQL sin comentarios y por sentencias; ante cualquier duda -una
/// sentencia que no se reconoce- se dice que no, y se comprueba.
fn solo_anade(sql: &str) -> bool {
    let sin_comentarios: String = sql
        .lines()
        .map(|l| l.split("--").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    sin_comentarios
        .split(';')
        .map(|s| {
            s.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_uppercase()
        })
        .filter(|s| !s.is_empty())
        .all(|s| {
            s.starts_with("CREATE INDEX ")
                || s.starts_with("CREATE UNIQUE INDEX ")
                || s.starts_with("DROP INDEX ")
                || (s.starts_with("ALTER TABLE ")
                    && s.contains(" ADD COLUMN ")
                    && !s.contains(" REFERENCES "))
        })
}

/// Comprueba que ninguna migración dejó una referencia rota.
///
/// Es la mitad que hace segura la de arriba. Con las claves ajenas apagadas, una
/// migración mal escrita puede dejar filas huérfanas sin que nadie proteste, y el
/// daño no aparecería hasta mucho después y lejos de su causa. `foreign_key_check`
/// recorre la base entera y devuelve una fila por cada violación; aquí basta con
/// saber si hay alguna.
fn comprobar_referencias(conn: &rusqlite::Connection) -> NbResult<()> {
    let violaciones: i64 = conn
        .query_row(
            "SELECT count(*) FROM pragma_foreign_key_check",
            [],
            |fila| fila.get(0),
        )
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo comprobar la integridad referencial");
            NbError::StorageFailure
        })?;

    if violaciones > 0 {
        tracing::error!(
            violaciones,
            "una migración dejó referencias rotas; la base no está íntegra"
        );
        return Err(NbError::SchemaMismatch);
    }
    Ok(())
}

/// Tablas que el respaldo lleva **vacías**, con su esquema: el índice del texto y
/// lo que cuelga de él.
///
/// Son exactamente las que vacía [`crate::search::reset`], y no por casualidad:
/// una base con estas tablas vacías y todo lo demás lleno es un estado que la
/// aplicación ya sabe recuperar, porque es el que deja rehacer el índice entero.
/// El indexador ve las ediciones sin estado de búsqueda y las vuelve a leer. Una
/// prueba compara el respaldo con la base original después de `reset`, así que
/// si una de las dos listas cambia sin la otra, falla.
///
/// **Lo que no está aquí se copia**, y esa es la dirección segura: una tabla nueva
/// entra en el respaldo sin que nadie tenga que acordarse. Equivocarse al revés
/// -dejar fuera algo que no se rehace- sería perder datos del usuario el único
/// día que hace falta el respaldo. El catálogo de búsqueda (`catalog_fts`) también
/// se podría rehacer, pero se copia: ocupa decenas de megas y así restaurar no
/// depende de que alguien se acuerde de reconstruirlo.
const SE_REHACEN: &[&str] = &[
    "documents_fts",
    "text_chunks",
    "chunk_vectors",
    "index_probes",
    "search_status",
];

/// Guarda lo que no se puede rehacer antes de migrar, y se lleva los respaldos
/// anteriores.
///
/// El nombre incluye la versión de origen para que el usuario pueda identificar
/// desde qué estado se hizo la copia.
///
/// # Por qué no es una copia del archivo
///
/// Antes lo era, y tenía dos problemas. El primero, de tamaño: sobre la
/// biblioteca real la base ocupa 23,9 GB, y de ellos 23,3 son el índice del texto
/// y sus fragmentos, que se rehacen leyendo los libros. Cada migración copiaba 24
/// GB para proteger menos de uno, y la copia contaba contra el tope de lo
/// derivado, quitándole sitio al propio índice. se decidió el
/// 2026-09-24 que el respaldo guarde solo lo que no se rehace.
///
/// El segundo era peor, porque no se veía: `fs::copy` copiaba el archivo
/// principal y **no el `-wal`**. En modo WAL lo último que se escribe vive ahí
/// hasta el siguiente punto de control, así que el respaldo podía salir sin las
/// últimas anotaciones y nadie lo habría sabido hasta restaurarlo. Ahora se lee
/// con una conexión propia, que ve la base entera tal como está.
///
/// # Por qué se queda una sola
///
/// Porque lo que el respaldo promete es deshacer **la migración que se va a
/// ejecutar ahora**, y para eso la copia que sirve es la de este momento. Se
/// escribe aparte y se renombra al terminar, y solo entonces se retiran las
/// anteriores: un respaldo a medias nunca sustituye a uno entero, y en ningún
/// instante hay cero.
fn backup(db_path: &Path, from_version: u32) -> NbResult<()> {
    if !db_path.exists() {
        return Ok(());
    }

    let mut backup_path = db_path.as_os_str().to_owned();
    backup_path.push(format!(".v{from_version}.backup"));
    let backup_path = std::path::PathBuf::from(backup_path);
    let mut parcial = backup_path.as_os_str().to_owned();
    parcial.push(".parcial");
    let parcial = std::path::PathBuf::from(parcial);

    let reloj = std::time::Instant::now();
    let _ = std::fs::remove_file(&parcial);
    if let Err(error) = escribir_respaldo(db_path, &parcial, from_version) {
        let _ = std::fs::remove_file(&parcial);
        tracing::error!(error = %error, "no se pudo crear el respaldo previo a la migración");
        return Err(NbError::StorageFailure);
    }
    std::fs::rename(&parcial, &backup_path).map_err(|error| {
        let _ = std::fs::remove_file(&parcial);
        tracing::error!(error = %error, "no se pudo dejar en su sitio el respaldo previo a la migración");
        NbError::StorageFailure
    })?;

    tracing::info!(
        desde_version = from_version,
        respaldo = %crate::telemetry::redact_path(&backup_path),
        bytes = std::fs::metadata(&backup_path).map_or(0, |m| m.len()),
        ms = reloj.elapsed().as_millis(),
        "respaldo previo a la migración creado"
    );

    retirar_respaldos_anteriores(db_path, &backup_path);
    Ok(())
}

/// Escribe en `destino` una base con el esquema entero de `origen` y las filas de
/// todo lo que no está en [`SE_REHACEN`].
///
/// Se construye como base principal de su propia conexión, y no adjunta a la que
/// va a migrar, por una razón práctica: así el SQL guardado en `sqlite_master`
/// sirve tal cual para crear cada objeto, sin reescribirlo para añadirle un
/// esquema, que es la clase de manipulación de texto que falla con el primer
/// nombre entre comillas.
fn escribir_respaldo(origen: &Path, destino: &Path, version: u32) -> rusqlite::Result<()> {
    let ruta = origen
        .to_str()
        .ok_or_else(|| rusqlite::Error::InvalidPath(origen.to_path_buf()))?;

    let mut conn = Connection::open(destino)?;
    conn.busy_timeout(super::BUSY_TIMEOUT)?;
    conn.execute("ATTACH DATABASE ?1 AS origen", [ruta])?;

    {
        // De escritura desde el principio: la regla de `db::write_transaction`
        // vale también aquí, aunque esta conexión no compita con nadie.
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        // Las tablas de sombra de FTS5 las crea su tabla virtual; crearlas a mano
        // antes fallaría por duplicadas.
        let sombra: Vec<String> = tx
            .prepare(
                "SELECT name FROM pragma_table_list WHERE schema = 'origen' AND type = 'shadow'",
            )?
            .query_map([], |fila| fila.get(0))?
            .collect::<rusqlite::Result<_>>()?;

        let objetos: Vec<(String, String, String)> = tx
            .prepare(
                "SELECT type, name, sql FROM origen.sqlite_master
                  WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
                  ORDER BY rowid",
            )?
            .query_map([], |fila| Ok((fila.get(0)?, fila.get(1)?, fila.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;

        let tablas: Vec<&(String, String, String)> = objetos
            .iter()
            .filter(|(tipo, nombre, _)| tipo == "table" && !sombra.contains(nombre))
            .collect();

        for (_, _, sql) in &tablas {
            tx.execute_batch(sql)?;
        }

        for (_, nombre, _) in &tablas {
            if SE_REHACEN.contains(&nombre.as_str()) {
                continue;
            }
            // Sin las columnas ocultas: las de una tabla virtual y las generadas
            // no admiten que se les escriba.
            let columnas: Vec<String> = tx
                .prepare("SELECT name FROM pragma_table_xinfo(?1, 'origen') WHERE hidden = 0")?
                .query_map([nombre], |fila| fila.get::<_, String>(0))?
                .map(|c| c.map(|c| format!("\"{}\"", c.replace('"', "\"\""))))
                .collect::<rusqlite::Result<_>>()?;
            let lista = columnas.join(", ");
            let tabla = nombre.replace('"', "\"\"");
            tx.execute_batch(&format!(
                "INSERT INTO main.\"{tabla}\" ({lista}) SELECT {lista} FROM origen.\"{tabla}\";"
            ))?;
        }

        // Los índices después de las filas, que es más barato que mantenerlos
        // durante la copia; y los disparadores al final, porque antes dispararían
        // con cada fila copiada y duplicarían lo que mantienen (el catálogo de
        // búsqueda, las cuentas de las estanterías).
        for tipo in ["index", "view", "trigger"] {
            for (_, _, sql) in objetos.iter().filter(|(t, _, _)| t == tipo) {
                tx.execute_batch(sql)?;
            }
        }

        // Lo que llevaba cada `AUTOINCREMENT`, para que un identificador no se
        // reutilice tras restaurar.
        let hay_secuencias: bool = tx.query_row(
            "SELECT count(*) > 0 FROM main.sqlite_master WHERE name = 'sqlite_sequence'",
            [],
            |fila| fila.get(0),
        )?;
        if hay_secuencias {
            tx.execute_batch(
                "DELETE FROM main.sqlite_sequence;
                 INSERT INTO main.sqlite_sequence (name, seq)
                      SELECT name, seq FROM origen.sqlite_sequence;",
            )?;
        }

        tx.pragma_update(Some("main"), "user_version", version)?;
        tx.commit()?;
    }

    conn.execute_batch("DETACH DATABASE origen")?;

    // Lo que no se copió no puede dejar referencias rotas: las tablas vacías solo
    // apuntan hacia fuera. Si alguna vez las deja, el respaldo no vale.
    let rotas: i64 = conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |f| {
        f.get(0)
    })?;
    if rotas > 0 {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some(format!("{rotas} referencias rotas en el respaldo")),
        ));
    }
    Ok(())
}

/// Borra las copias previas a la que se acaba de escribir.
///
/// No devuelve error: que sobre una copia vieja es un desperdicio de disco, no un
/// fallo que deba impedir migrar.
fn retirar_respaldos_anteriores(db_path: &Path, actual: &Path) {
    let (Some(directorio), Some(nombre)) = (db_path.parent(), db_path.file_name()) else {
        return;
    };
    let prefijo = format!("{}.v", nombre.to_string_lossy());

    let Ok(entradas) = std::fs::read_dir(directorio) else {
        return;
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta == actual {
            continue;
        }
        let nombre = entrada.file_name();
        let nombre = nombre.to_string_lossy();
        if nombre.starts_with(&prefijo) && nombre.ends_with(".backup") {
            match std::fs::remove_file(&ruta) {
                Ok(()) => tracing::info!(
                    respaldo = %crate::telemetry::redact_path(&ruta),
                    "respaldo anterior retirado: ya hay uno más reciente"
                ),
                Err(error) => {
                    tracing::warn!(error = %error, "no se pudo retirar un respaldo anterior");
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas_de_solo_anade {
    use super::solo_anade;

    /// Añadir columnas e índices no recorre la base para comprobar claves;
    /// cualquier otra cosa, sí.
    #[test]
    fn solo_lo_que_no_puede_romper_una_clave_se_salta_la_comprobacion() {
        assert!(solo_anade(include_str!(
            "../../migrations/V42__sinopsis.sql"
        )));
        assert!(solo_anade(include_str!(
            "../../migrations/V43__desempate_por_titulo.sql"
        )));
        assert!(!solo_anade(
            "ALTER TABLE a ADD COLUMN b TEXT REFERENCES c(id);"
        ));
        assert!(!solo_anade("UPDATE a SET b = 1;"));
        assert!(!solo_anade("CREATE TABLE x (id TEXT);"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memoria() -> Connection {
        Connection::open_in_memory().expect("base en memoria")
    }

    #[test]
    fn una_base_vacia_migra_hasta_la_version_objetivo() {
        let mut conn = memoria();
        assert_eq!(current_version(&conn).expect("legible"), 0);

        let version = run(&mut conn, None).expect("migra");

        assert_eq!(version, target_version());
        assert_eq!(current_version(&conn).expect("legible"), target_version());
    }

    #[test]
    fn migrar_dos_veces_es_idempotente() {
        let mut conn = memoria();
        run(&mut conn, None).expect("primera");
        let filas: i64 = conn
            .query_row("SELECT count(*) FROM app_metadata", [], |r| r.get(0))
            .expect("consulta");

        run(&mut conn, None).expect("segunda");

        let filas_despues: i64 = conn
            .query_row("SELECT count(*) FROM app_metadata", [], |r| r.get(0))
            .expect("consulta");
        assert_eq!(
            filas, filas_despues,
            "la segunda ejecución no debe reinsertar"
        );
    }

    /// Aplica solo las migraciones hasta `hasta`, para poder poblar una base con
    /// la forma antigua y comprobar qué le hace la migración siguiente.
    fn migrar_hasta(conn: &mut Connection, hasta: u32) {
        let subconjunto: Vec<&Migration> =
            MIGRATIONS.iter().filter(|m| m.version <= hasta).collect();
        for m in subconjunto {
            // Por el mismo camino que la migración de verdad: una ayuda de
            // prueba que abre la transacción de otra forma prueba otra cosa.
            let tx = crate::db::write_transaction(conn).expect("transacción");
            tx.execute_batch(m.sql).expect("migración");
            tx.pragma_update(None, "user_version", m.version)
                .expect("versión");
            tx.commit().expect("confirmación");
        }
    }

    /// La migración que quita las rutas absolutas no puede perder
    /// ninguna ubicación: raíz más ruta relativa tiene que reconstruir
    /// exactamente la ruta que había.
    #[test]
    fn la_migracion_de_raices_reconstruye_todas_las_ubicaciones() {
        let mut conn = memoria();
        migrar_hasta(&mut conn, 10);

        conn.execute(
            "INSERT INTO works (id, canonical_title) VALUES ('w', 'Obra')",
            [],
        )
        .expect("obra");
        conn.execute(
            "INSERT INTO editions (id, work_id, title) VALUES ('e', 'w', 'Edición')",
            [],
        )
        .expect("edición");

        // Casos que de verdad aparecen: dos referenciados en la misma carpeta,
        // uno en otra, un administrado, y una ruta de Windows.
        let heredados = [
            ("f1", "/home/ana/Libros/Spinoza/Etica.epub", "referenced"),
            ("f2", "/home/ana/Libros/Spinoza/Tratado.epub", "referenced"),
            ("f3", "/mnt/disco/Comics/uno.cbz", "referenced"),
            (
                "f4",
                "/home/ana/.local/share/nb-books/biblioteca/sha-abc.epub",
                "managed",
            ),
            ("f5", r"C:\Users\Ana\Libros\uno.epub", "referenced"),
        ];
        for (i, (id, ruta, modo)) in heredados.iter().enumerate() {
            conn.execute(
                "INSERT INTO files (id, edition_id, path, content_hash, byte_size, media_type,
                                    storage_mode)
                 VALUES (?1, 'e', ?2, ?3, 1, 'application/epub+zip', ?4)",
                rusqlite::params![id, ruta, format!("sha256:{i:064x}"), modo],
            )
            .expect("archivo heredado");
        }

        run(&mut conn, None).expect("migración a la versión objetivo");

        // Cada archivo reconstruye su ruta original. El administrado no: su raíz
        // se resuelve en ejecución, y eso es lo que se comprueba aparte.
        let mut stmt = conn
            .prepare(
                "SELECT f.id,
                        (SELECT r.path FROM library_roots r WHERE r.id = f.root_id),
                        f.relative_path
                   FROM files f ORDER BY f.id",
            )
            .expect("consulta");
        let filas: Vec<(String, Option<String>, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .expect("filas")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("recogida");
        drop(stmt);

        assert_eq!(filas.len(), heredados.len(), "la migración perdió archivos");

        for (id, raiz, relativa) in filas {
            let (_, original, modo) = heredados
                .iter()
                .find(|(h, _, _)| *h == id)
                .expect("archivo conocido");
            let normalizada = original.replace('\\', "/");

            if *modo == "managed" {
                assert!(
                    raiz.is_none(),
                    "la raíz administrada no guarda ruta todavía"
                );
                assert_eq!(relativa, "sha-abc.epub");
            } else {
                let reconstruida = format!("{}/{}", raiz.expect("raíz localizada"), relativa);
                assert_eq!(
                    reconstruida, normalizada,
                    "la ubicación de {id} no se reconstruye"
                );
            }
        }

        // Dos archivos de la misma carpeta comparten raíz; los de carpetas
        // distintas, no. Es la misma regla que rige al importar.
        let raices: i64 = conn
            .query_row(
                "SELECT count(*) FROM library_roots WHERE kind = 'referenced'",
                [],
                |r| r.get(0),
            )
            .expect("recuento");
        assert_eq!(raices, 3, "una raíz por carpeta distinta, ni más ni menos");
    }

    /// El caso del usuario que instala una versión antigua tras haber usado una
    /// nueva. Operar sobre esa base la corrompería.
    #[test]
    fn se_niega_a_operar_sobre_una_base_mas_nueva() {
        let mut conn = memoria();
        conn.pragma_update(None, "user_version", target_version() + 1)
            .expect("escribible");

        let resultado = run(&mut conn, None);

        assert!(matches!(resultado, Err(NbError::SchemaMismatch)));
    }

    /// Migraciones sintéticas: son las únicas con las que se pueden ejercitar
    /// el salto de versiones, la reversión y el respaldo mientras solo exista
    /// una migración real.
    const SANAS: &[Migration] = &[
        Migration {
            version: 1,
            name: "una",
            sql: "CREATE TABLE una (id INTEGER PRIMARY KEY) STRICT;",
        },
        Migration {
            version: 2,
            name: "dos",
            sql: "CREATE TABLE dos (id INTEGER PRIMARY KEY) STRICT;",
        },
        Migration {
            version: 3,
            name: "tres",
            sql: "CREATE TABLE tres (id INTEGER PRIMARY KEY) STRICT;",
        },
    ];

    /// La segunda migración crea una tabla y después falla: es el caso que
    /// distingue una migración transaccional de una que deja la base a medias.
    const CON_FALLO: &[Migration] = &[
        Migration {
            version: 1,
            name: "una",
            sql: "CREATE TABLE una (id INTEGER PRIMARY KEY) STRICT;",
        },
        Migration {
            version: 2,
            name: "rota",
            sql: "CREATE TABLE a_medias (id INTEGER PRIMARY KEY) STRICT;\n\
                  INSERT INTO tabla_que_no_existe VALUES (1);",
        },
    ];

    fn existe_tabla(conn: &Connection, nombre: &str) -> bool {
        conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [nombre],
            |r| r.get::<_, i64>(0),
        )
        .expect("consulta")
            > 0
    }

    /// Si una migración falla a mitad, la base debe quedar exactamente como
    /// estaba tras la última migración correcta: ni tablas a medias ni versión
    /// adelantada. Ejercita [`apply`], no la semántica de SQLite.
    #[test]
    fn una_migracion_fallida_no_deja_estado_intermedio() {
        let mut conn = memoria();

        let resultado = apply(&mut conn, None, CON_FALLO);

        assert!(matches!(resultado, Err(NbError::StorageFailure)));
        assert_eq!(
            current_version(&conn).expect("legible"),
            1,
            "la versión no puede avanzar por una migración que falló"
        );
        assert!(
            existe_tabla(&conn, "una"),
            "la migración correcta anterior debe conservarse"
        );
        assert!(
            !existe_tabla(&conn, "a_medias"),
            "la tabla creada dentro de la migración fallida debe revertirse"
        );
    }

    /// El caso del usuario que se salta versiones: instala la 1.0 y actualiza
    /// directamente a la 3.0 sin pasar por la intermedia.
    #[test]
    fn migrar_saltando_versiones_aplica_todas_las_pendientes() {
        let mut conn = memoria();

        let version = apply(&mut conn, None, SANAS).expect("migra desde vacío");

        assert_eq!(version, 3);
        for tabla in ["una", "dos", "tres"] {
            assert!(existe_tabla(&conn, tabla), "falta la tabla {tabla}");
        }
    }

    /// Una base ya migrada parcialmente solo aplica lo que le falta, sin
    /// reejecutar lo aplicado.
    #[test]
    fn desde_una_version_intermedia_solo_se_aplica_lo_pendiente() {
        let mut conn = memoria();
        apply(&mut conn, None, &SANAS[..1]).expect("primera migración");
        conn.execute("INSERT INTO una (id) VALUES (1)", [])
            .expect("dato de usuario");

        let version = apply(&mut conn, None, SANAS).expect("migra el resto");

        assert_eq!(version, 3);
        let filas: i64 = conn
            .query_row("SELECT count(*) FROM una", [], |r| r.get(0))
            .expect("consulta");
        assert_eq!(
            filas, 1,
            "reejecutar la migración 1 habría vaciado la tabla"
        );
    }

    /// Antes de tocar una base que ya tiene datos se copia el archivo. Es la
    /// última red si una migración destruye información.
    #[test]
    fn se_respalda_antes_de_migrar_una_base_con_datos() {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-mig-respaldo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        let path = dir.join("prueba.sqlite");

        let mut conn = Connection::open(&path).expect("base en archivo");
        apply(&mut conn, Some(&path), &SANAS[..1]).expect("primera migración");

        apply(&mut conn, Some(&path), SANAS).expect("segunda tanda");

        let respaldo = dir.join("prueba.sqlite.v1.backup");
        assert!(
            respaldo.exists(),
            "debe existir el respaldo previo a la migración"
        );

        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Una base vacía no tiene nada que perder: respaldarla solo produce
    /// archivos de cero bytes que confunden al usuario.
    #[test]
    fn una_base_vacia_no_genera_respaldo() {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-mig-sin-respaldo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        let path = dir.join("prueba.sqlite");

        let mut conn = Connection::open(&path).expect("base en archivo");
        apply(&mut conn, Some(&path), SANAS).expect("migra desde vacío");

        assert!(!dir.join("prueba.sqlite.v0.backup").exists());

        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Directorio propio de una prueba, que se borra al terminar.
    struct Temporal(std::path::PathBuf);

    impl Temporal {
        fn nuevo(nombre: &str) -> Self {
            let dir = crate::paths::temporal_de_pruebas()
                .join(format!("nb-mig-{nombre}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("directorio");
            Self(dir)
        }
    }

    impl Drop for Temporal {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Lo que el usuario no puede recuperar de ninguna otra parte, en el esquema
    /// de cualquier versión desde la 12. Pasa por los disparadores del catálogo y
    /// de las estanterías, que es lo que más fácil se duplica al copiar.
    const DEL_USUARIO: &str = r#"
        INSERT INTO works (id, canonical_title) VALUES ('w1', 'Cien Años de Soledad');
        INSERT INTO editions (id, work_id, title) VALUES ('e1', 'w1', 'Cien Años de Soledad');
        INSERT INTO contributors (id, name, sort_name)
             VALUES ('p1', 'Gabriel García Márquez', 'García Márquez, Gabriel');
        INSERT INTO edition_contributors (edition_id, contributor_id, role, position)
             VALUES ('e1', 'p1', 'author', 0);
        INSERT INTO annotations (id, edition_id, kind, body, target)
             VALUES ('a1', 'e1', 'note', 'Lo importante es el hielo',
                     '{"version":1,"href":"c1.xhtml","mediaType":"application/xhtml+xml",
                       "locations":{"progression":0.42},"text":{"highlight":"el hielo"}}');
        INSERT INTO notes (id, title, body) VALUES ('n1', 'Macondo', 'Ver [[Hielo]]');
        INSERT INTO settings (key, value) VALUES ('theme', '"pergamino"');
    "#;

    /// El índice de un libro: fragmentos, su texto indexado, el estado de
    /// búsqueda y la sonda. Lo que pesa, y lo que se rehace.
    fn indexar(conn: &Connection, fragmentos: usize) {
        for n in 0..fragmentos {
            // Palabras distintas en cada fragmento, como en un libro: repetir la
            // misma daría un índice diminuto y la prueba de tamaño no mediría nada.
            let relleno: String = (0..120).map(|i| format!("palabra{n}x{i} ")).collect();
            conn.execute(
                "INSERT INTO text_chunks (edition_id, locator)
                 VALUES ('e1', '{\"version\":1,\"href\":\"c1.xhtml\",\"mediaType\":\"application/xhtml+xml\",\"locations\":{}}')",
                [],
            )
            .expect("fragmento");
            let fila = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO documents_fts (rowid, text) VALUES (?1, ?2)",
                rusqlite::params![fila, format!("{relleno} {n}")],
            )
            .expect("índice");
        }
        conn.execute_batch(
            "INSERT OR REPLACE INTO search_status (edition_id, state) VALUES ('e1', 'searchable');
             INSERT OR REPLACE INTO index_probes (edition_id, term, chunk_rowid)
                  SELECT 'e1', 'palabra', min(rowid) FROM text_chunks;
             INSERT OR IGNORE INTO chunk_vectors (chunk_id, edition_id, model, scale, vector)
                  SELECT id, 'e1', 'modelo', 1.0, x'0102' FROM text_chunks LIMIT 1;",
        )
        .expect("estado del índice");
    }

    /// Una base en archivo y en modo WAL, como la de verdad, al día y poblada.
    fn base_poblada(ruta: &Path) -> Connection {
        let mut conn = Connection::open(ruta).expect("base");
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("wal");
        run(&mut conn, None).expect("migra");
        conn.execute_batch(DEL_USUARIO).expect("contenido");
        indexar(&conn, 200);
        conn
    }

    fn contar(conn: &Connection, tabla: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM \"{tabla}\""), [], |f| {
            f.get(0)
        })
        .expect("cuenta")
    }

    /// **El criterio: el respaldo es exactamente la base con el índice
    /// vaciado.** Ni una fila del usuario menos, ni una fila del índice más.
    ///
    /// Se compara contra lo que deja `search::reset`, que es el estado del que la
    /// aplicación ya sabe recuperarse -rehacer el índice entero-, y no contra una
    /// lista escrita aquí: si alguien añade una tabla al índice y no a
    /// [`SE_REHACEN`], o al revés, esta prueba lo dice.
    #[test]
    fn el_respaldo_es_la_base_con_el_indice_vaciado() {
        let tmp = Temporal::nuevo("respaldo-exacto");
        let ruta = tmp.0.join("biblioteca.sqlite");
        let conn = base_poblada(&ruta);

        backup(&ruta, target_version()).expect("respaldo");

        // La referencia: una copia de la base a la que se le vacía el índice.
        let referencia = tmp.0.join("referencia.sqlite");
        conn.execute("VACUUM INTO ?1", [referencia.to_str().expect("ruta")])
            .expect("copia");
        let vaciada = Connection::open(&referencia).expect("referencia");
        vaciada
            .pragma_update(None, "foreign_keys", "ON")
            .expect("claves");
        crate::search::reset(&vaciada).expect("vaciar el índice");

        let respaldo = Connection::open(
            tmp.0
                .join(format!("biblioteca.sqlite.v{}.backup", target_version())),
        )
        .expect("respaldo");

        let tablas: Vec<(String, String)> = vaciada
            .prepare(
                "SELECT name, type FROM pragma_table_list
                  WHERE schema = 'main' AND type IN ('table', 'virtual')
                    AND name NOT LIKE 'sqlite_%'",
            )
            .expect("tablas")
            .query_map([], |f| Ok((f.get(0)?, f.get(1)?)))
            .expect("consulta")
            .collect::<rusqlite::Result<_>>()
            .expect("filas");
        assert!(tablas.len() > 40, "se esperaba el esquema entero");

        for (tabla, _) in &tablas {
            assert_eq!(
                contar(&respaldo, tabla),
                contar(&vaciada, tabla),
                "la tabla {tabla} no tiene en el respaldo lo mismo que en la base con el índice vaciado"
            );
        }

        // Y no solo cuántas: las mismas filas, en las tablas de verdad.
        respaldo
            .execute(
                "ATTACH DATABASE ?1 AS ref",
                [referencia.to_str().expect("ruta")],
            )
            .expect("adjuntar");
        for (tabla, tipo) in tablas.iter().filter(|(_, tipo)| tipo == "table") {
            let distintas: i64 = respaldo
                .query_row(
                    &format!(
                        "SELECT (SELECT count(*) FROM (SELECT * FROM main.\"{tabla}\" EXCEPT SELECT * FROM ref.\"{tabla}\"))
                              + (SELECT count(*) FROM (SELECT * FROM ref.\"{tabla}\" EXCEPT SELECT * FROM main.\"{tabla}\"))"
                    ),
                    [],
                    |f| f.get(0),
                )
                .unwrap_or_else(|e| panic!("{tabla} ({tipo}): {e}"));
            assert_eq!(distintas, 0, "la tabla {tabla} no tiene las mismas filas");
        }

        assert_eq!(contar(&respaldo, "annotations"), 1, "la anotación");
        assert_eq!(contar(&respaldo, "text_chunks"), 0, "el índice va vacío");
        assert_eq!(
            contar(&respaldo, "catalog_fts"),
            1,
            "el catálogo va una vez, no duplicado por los disparadores"
        );
        let version: u32 = respaldo
            .query_row("PRAGMA user_version", [], |f| f.get(0))
            .expect("versión");
        assert_eq!(
            version,
            target_version(),
            "el respaldo dice de qué versión es"
        );
    }

    /// **Un defecto encontrado de paso.** En modo WAL lo último que se
    /// escribe vive en el `-wal` hasta el siguiente punto de control, y copiar el
    /// archivo principal lo dejaba fuera. Aquí nada se ha llevado todavía al
    /// archivo principal, así que una copia del archivo no tendría la nota.
    #[test]
    fn el_respaldo_lleva_lo_que_todavia_esta_en_el_wal() {
        let tmp = Temporal::nuevo("respaldo-wal");
        let ruta = tmp.0.join("biblioteca.sqlite");
        let mut conn = Connection::open(&ruta).expect("base");
        conn.pragma_update(None, "journal_mode", "WAL")
            .expect("wal");
        conn.pragma_update(None, "wal_autocheckpoint", 0)
            .expect("sin puntos de control");
        run(&mut conn, None).expect("migra");
        conn.execute_batch(DEL_USUARIO).expect("contenido");

        let en_el_wal =
            std::fs::metadata(tmp.0.join("biblioteca.sqlite-wal")).map_or(0, |m| m.len());
        assert!(
            en_el_wal > 0,
            "la prueba necesita que lo escrito siga en el -wal"
        );

        backup(&ruta, target_version()).expect("respaldo");

        let respaldo = Connection::open(
            tmp.0
                .join(format!("biblioteca.sqlite.v{}.backup", target_version())),
        )
        .expect("respaldo");
        let cuerpo: String = respaldo
            .query_row("SELECT body FROM annotations WHERE id = 'a1'", [], |f| {
                f.get(0)
            })
            .expect("la anotación tiene que estar en el respaldo");
        assert_eq!(cuerpo, "Lo importante es el hielo");
        drop(conn);
    }

    /// Lo que la motiva: el índice es casi toda la base, y no viaja.
    #[test]
    fn el_respaldo_no_carga_con_el_indice() {
        let tmp = Temporal::nuevo("respaldo-tamano");
        let ruta = tmp.0.join("biblioteca.sqlite");
        let conn = base_poblada(&ruta);
        indexar(&conn, 3_000);

        let entera = tmp.0.join("entera.sqlite");
        conn.execute("VACUUM INTO ?1", [entera.to_str().expect("ruta")])
            .expect("copia entera");
        backup(&ruta, target_version()).expect("respaldo");

        let pesa = |nombre: &str| std::fs::metadata(tmp.0.join(nombre)).expect("tamaño").len();
        let respaldo = pesa(&format!("biblioteca.sqlite.v{}.backup", target_version()));
        let base = pesa("entera.sqlite");
        assert!(
            respaldo * 4 < base,
            "el respaldo ({respaldo} B) tendría que ser una fracción de la base ({base} B)"
        );
    }

    /// **Restaurar funciona**: el respaldo de una versión anterior se abre, migra
    /// hasta la actual, conserva lo del usuario y deja el índice por rehacer.
    #[test]
    fn restaurar_el_respaldo_abre_migra_y_deja_el_indice_por_rehacer() {
        let tmp = Temporal::nuevo("respaldo-restaurar");
        let ruta = tmp.0.join("biblioteca.sqlite");
        let anterior = target_version() - 1;
        {
            let mut conn = Connection::open(&ruta).expect("base");
            conn.pragma_update(None, "journal_mode", "WAL")
                .expect("wal");
            migrar_hasta(&mut conn, anterior);
            conn.execute_batch(DEL_USUARIO).expect("contenido");
            indexar(&conn, 50);
        }

        // La actualización: abrirla con esta versión respalda y migra.
        drop(crate::db::Database::open(&ruta).expect("migra"));
        let respaldo = tmp.0.join(format!("biblioteca.sqlite.v{anterior}.backup"));
        assert!(respaldo.exists(), "migrar tiene que dejar el respaldo");
        assert!(
            !tmp.0
                .join(format!("biblioteca.sqlite.v{anterior}.backup.parcial"))
                .exists(),
            "no puede quedar un respaldo a medias"
        );

        // Restaurar es poner el respaldo en el sitio de la base.
        let restaurada = tmp.0.join("restaurada.sqlite");
        std::fs::copy(&respaldo, &restaurada).expect("restaurar");
        let db = crate::db::Database::open(&restaurada).expect("el respaldo se abre");
        assert_eq!(db.schema_version().expect("versión"), target_version());

        let conn = db.connection().expect("conexión");
        let cuerpo: String = conn
            .query_row("SELECT body FROM annotations WHERE id = 'a1'", [], |f| {
                f.get(0)
            })
            .expect("la anotación");
        assert_eq!(cuerpo, "Lo importante es el hielo");
        let integridad: String = conn
            .query_row("PRAGMA integrity_check", [], |f| f.get(0))
            .expect("integridad");
        assert_eq!(integridad, "ok");

        // Sin estado de búsqueda, la edición vuelve a la cola del indexador, que
        // es como se rehace el índice.
        let pendientes: i64 = conn
            .query_row(
                "SELECT count(*) FROM editions e
                   LEFT JOIN search_status s ON s.edition_id = e.id
                  WHERE s.edition_id IS NULL OR s.state = 'pending'",
                [],
                |f| f.get(0),
            )
            .expect("cola");
        assert_eq!(pendientes, 1, "la edición tiene que volver a indexarse");
    }

    /// **V38 funde a los autores repetidos** sin que ninguna obra pierda una
    /// autoría, y deja en su sitio lo que se mantiene por disparador -las
    /// estanterías, el orden por autor y el catálogo de búsqueda-, que es lo que se
    /// rompe al mover vínculos a mano.
    #[test]
    fn v38_funde_los_autores_repetidos_sin_perder_ninguna_autoria() {
        let mut conn = memoria();
        migrar_hasta(&mut conn, 37);
        conn.execute_batch(
            "INSERT INTO works (id, canonical_title) VALUES ('w', 'Obra');
             INSERT INTO editions (id, work_id, title)
                  VALUES ('e1', 'w', 'Uno'), ('e2', 'w', 'Dos'), ('e3', 'w', 'Tres'), ('e4', 'w', 'Cuatro');
             INSERT INTO contributors (id, name, orcid)
                  VALUES ('a1', 'Allan Pease', NULL), ('a2', 'Allan Pease', NULL),
                         ('a3', 'Allan pease', NULL), ('z1', 'Slavoj Žižek', NULL),
                         ('z2', 'Slavoj Zizek', NULL), ('o1', 'Ana Orcid', '0000-0002-1825-0097'),
                         ('o2', 'Ana Orcid', NULL);
             INSERT INTO edition_contributors (edition_id, contributor_id, role, position)
                  VALUES ('e1', 'a1', 'author', 0), ('e2', 'a2', 'author', 0),
                         ('e3', 'a3', 'author', 0), ('e3', 'a1', 'author', 1),
                         ('e4', 'z1', 'author', 0), ('e2', 'z2', 'translator', 1),
                         ('e1', 'o1', 'editor', 1), ('e4', 'o2', 'editor', 1);",
        )
        .expect("una biblioteca de antes de V38");

        run(&mut conn, None).expect("V38");

        let ids: Vec<String> = conn
            .prepare("SELECT id FROM contributors ORDER BY id")
            .expect("consulta")
            .query_map([], |f| f.get(0))
            .expect("filas")
            .collect::<rusqlite::Result<_>>()
            .expect("ids");
        assert_eq!(
            ids,
            ["a1", "o1", "o2", "z1"],
            "se queda el que más obras tiene y, a igualdad, el primero; el del ORCID no se funde"
        );

        let autorias = |edicion: &str| -> Vec<(String, String)> {
            conn.prepare(
                "SELECT contributor_id, role FROM edition_contributors
                  WHERE edition_id = ?1 ORDER BY role, contributor_id",
            )
            .expect("consulta")
            .query_map([edicion], |f| Ok((f.get(0)?, f.get(1)?)))
            .expect("filas")
            .collect::<rusqlite::Result<_>>()
            .expect("autorías")
        };
        let par = |a: &str, b: &str| (a.to_owned(), b.to_owned());
        assert_eq!(autorias("e1"), [par("a1", "author"), par("o1", "editor")]);
        assert_eq!(
            autorias("e2"),
            [par("a1", "author"), par("z1", "translator")]
        );
        assert_eq!(
            autorias("e3"),
            [par("a1", "author")],
            "el vínculo repetido sobra"
        );
        assert_eq!(autorias("e4"), [par("z1", "author"), par("o2", "editor")]);

        // Las estanterías cuentan lo mismo que el catálogo.
        let estanterias: Vec<(String, i64)> = conn
            .prepare("SELECT shelf_key, tally FROM shelf_counts WHERE kind = 'author' ORDER BY shelf_key")
            .expect("consulta")
            .query_map([], |f| Ok((f.get(0)?, f.get(1)?)))
            .expect("filas")
            .collect::<rusqlite::Result<_>>()
            .expect("estanterías");
        assert_eq!(
            estanterias,
            [
                ("Allan Pease".to_owned(), 3),
                ("Slavoj Žižek".to_owned(), 1)
            ]
        );

        // El catálogo de búsqueda dice el nombre que se quedó.
        let autores: String = conn
            .query_row(
                "SELECT authors FROM catalog_fts WHERE edition_id = 'e2'",
                [],
                |f| f.get(0),
            )
            .expect("catálogo");
        assert!(autores.contains("Slavoj Žižek"), "{autores}");
        let orden: String = conn
            .query_row(
                "SELECT author_sort FROM editions WHERE id = 'e3'",
                [],
                |f| f.get(0),
            )
            .expect("orden");
        assert_eq!(orden, "Allan Pease");

        // Y a partir de aquí no entra otra fila para él.
        let otra = conn.execute("INSERT INTO contributors (name) VALUES ('ALLAN PEASE')", []);
        assert!(otra.is_err());
    }

    #[test]
    fn las_versiones_de_las_migraciones_son_estrictamente_crecientes() {
        let mut previa = 0;
        for migration in MIGRATIONS {
            assert!(
                migration.version > previa,
                "la migración {} rompe el orden",
                migration.name
            );
            previa = migration.version;
        }
    }
}
