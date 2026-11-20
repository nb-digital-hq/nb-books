//! **El criterio de aceptación:** ningún documento, anotación ni
//! preferencia se pierde al migrar. Su fallo bloquea la publicación.
//!
//! # Qué significa "la versión publicada anterior" cuando no hay ninguna
//!
//! La ficha dice "una base generada por la versión publicada anterior". Todavía
//! no hay ninguna: la primera publicación es la 1.0.0. Así que la base de partida se
//! **construye** aplicando las migraciones del repositorio hasta una versión
//! concreta y nada más, que es literalmente lo que una versión anterior habría
//! dejado en el disco: el mismo SQL, en el mismo orden, parándose antes.
//!
//! Esto no es una aproximación a la espera de algo mejor. Es lo mismo, y además
//! **no envejece**: el día que haya versiones publicadas, cada una tendrá su
//! número de esquema y esta prueba las cubrirá todas sin tocarla, porque recorre
//! todos los puntos de partida posibles en lugar de uno elegido a mano.
//!
//! # Por qué esta prueba es la que más importa de la subfase
//!
//! Porque es la única defensa real contra perder bibliotecas. Un defecto en
//! cualquier otra parte se nota y se arregla; una migración que pierde
//! anotaciones se nota meses después, cuando ya no hay copia de la que sacarlas.
//!
//! Contrato: `docs/architecture/migrations.md`.

use std::path::{Path, PathBuf};

use nb_books_lib::db::Database;

/// Las migraciones, en orden, tal y como las incluye el binario.
const MIGRACIONES: &[(u32, &str)] = &[
    (1, include_str!("../migrations/V1__app_metadata.sql")),
    (2, include_str!("../migrations/V2__bibliographic_core.sql")),
    (3, include_str!("../migrations/V3__metadata_provenance.sql")),
    (4, include_str!("../migrations/V4__settings.sql")),
    (5, include_str!("../migrations/V5__collections.sql")),
    (6, include_str!("../migrations/V6__progress.sql")),
    (
        7,
        include_str!("../migrations/V7__immutable_annotations.sql"),
    ),
    (8, include_str!("../migrations/V8__search.sql")),
    (9, include_str!("../migrations/V9__search_status.sql")),
    (10, include_str!("../migrations/V10__catalog_search.sql")),
    (11, include_str!("../migrations/V11__library_roots.sql")),
    (12, include_str!("../migrations/V12__linked_notes.sql")),
    (13, include_str!("../migrations/V13__network_gate.sql")),
    (14, include_str!("../migrations/V14__cover_path.sql")),
    (15, include_str!("../migrations/V15__vocabulary.sql")),
    (16, include_str!("../migrations/V16__catalogs.sql")),
    (17, include_str!("../migrations/V17__browse_order.sql")),
    (18, include_str!("../migrations/V18__import_runs.sql")),
    (19, include_str!("../migrations/V19__author_sort.sql")),
    (20, include_str!("../migrations/V20__marcas_nuevas.sql")),
    (21, include_str!("../migrations/V21__extension_origin.sql")),
    (
        22,
        include_str!("../migrations/V22__extension_withdrawals.sql"),
    ),
    (23, include_str!("../migrations/V23__note_boards.sql")),
    (24, include_str!("../migrations/V24__estanterias.sql")),
    (
        25,
        include_str!("../migrations/V25__estado_de_busqueda.sql"),
    ),
    (26, include_str!("../migrations/V26__fragmento_mayor.sql")),
    (27, include_str!("../migrations/V27__indice_sin_texto.sql")),
    (
        28,
        include_str!("../migrations/V28__ruta_relativa_por_componentes.sql"),
    ),
    (
        29,
        include_str!("../migrations/V29__filtros_por_indice.sql"),
    ),
    (
        30,
        include_str!("../migrations/V30__catalogos_en_claro.sql"),
    ),
    (
        31,
        include_str!("../migrations/V31__tableros_por_columnas.sql"),
    ),
    (32, include_str!("../migrations/V32__envios_de_ia.sql")),
    (33, include_str!("../migrations/V33__vectores.sql")),
    (34, include_str!("../migrations/V34__estudio_generado.sql")),
    (35, include_str!("../migrations/V35__audio_preparado.sql")),
    (36, include_str!("../migrations/V36__reportes_de_ia.sql")),
    (37, include_str!("../migrations/V37__copias_exactas.sql")),
    (38, include_str!("../migrations/V38__un_autor_una_fila.sql")),
    (39, include_str!("../migrations/V39__titulo_comparable.sql")),
    (
        40,
        include_str!("../migrations/V40__calibre_provenance.sql"),
    ),
    (41, include_str!("../migrations/V41__repaso_espaciado.sql")),
    (42, include_str!("../migrations/V42__sinopsis.sql")),
    (
        43,
        include_str!("../migrations/V43__desempate_por_titulo.sql"),
    ),
    (
        44,
        include_str!("../migrations/V44__orden_por_publicacion.sql"),
    ),
    (
        45,
        include_str!("../migrations/V45__estanterias_por_idioma_y_formato.sql"),
    ),
    (
        46,
        include_str!("../migrations/V46__dispositivos_moviles.sql"),
    ),
];

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-humo-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn archivo(&self, nombre: &str) -> PathBuf {
        self.0.join(nombre)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Crea una base **como la habría dejado una versión antigua**: sus migraciones
/// aplicadas hasta `hasta` y su `user_version` puesto, y nada más.
fn base_de_version(ruta: &Path, hasta: u32) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(ruta).expect("base");
    conn.pragma_update(None, "journal_mode", "WAL")
        .expect("wal");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("claves");

    for (version, sql) in MIGRACIONES.iter().filter(|(v, _)| *v <= hasta) {
        conn.execute_batch(sql)
            .unwrap_or_else(|e| panic!("migración {version}: {e}"));
        conn.pragma_update(None, "user_version", version)
            .expect("versión");
    }
    conn
}

fn version_objetivo() -> u32 {
    MIGRACIONES.last().expect("hay migraciones").0
}

/// **El criterio, primera mitad: se migra desde cualquier punto de partida.**
///
/// Se recorren todas las versiones intermedias y no solo la anterior. Una
/// aplicación de escritorio se actualiza cuando a su dueño le apetece, así que la
/// base que llega puede ser de cualquier versión pasada, incluida una de hace dos
/// años. Probar solo el último salto comprobaría el caso que menos ocurre.
#[test]
fn desde_cualquier_version_anterior_se_migra_hasta_la_actual() {
    let temporal = Temporal::nuevo("versiones");
    let objetivo = version_objetivo();

    for desde in 1..objetivo {
        let ruta = temporal.archivo(&format!("v{desde}.sqlite"));
        drop(base_de_version(&ruta, desde));

        let db = Database::open(&ruta).unwrap_or_else(|e| {
            panic!("una base de la versión {desde} tiene que poder abrirse: {e:?}")
        });

        assert_eq!(
            db.schema_version().expect("versión"),
            objetivo,
            "una base de la versión {desde} tiene que quedar al día"
        );

        let conn = db.connection().expect("conexión");
        let integridad: String = conn
            .query_row("PRAGMA integrity_check", [], |fila| fila.get(0))
            .expect("integridad");
        assert_eq!(integridad, "ok", "migrar desde {desde} dañó la base");
    }
}

/// **El criterio, segunda mitad: no se pierde nada del usuario.**
///
/// Se parte de la versión anterior a la actual, que es de donde vendrá la
/// primera actualización real, con un documento, una anotación, una preferencia,
/// una colección y una posición de lectura. Después de migrar tienen que estar
/// **exactamente** los mismos valores, no unos parecidos.
#[test]
fn migrar_no_pierde_documentos_ni_anotaciones_ni_preferencias() {
    let temporal = Temporal::nuevo("contenido");
    let ruta = temporal.archivo("biblioteca.sqlite");
    let anterior = version_objetivo() - 1;

    let conn = base_de_version(&ruta, anterior);

    conn.execute_batch(
        r#"
        INSERT INTO works (id, canonical_title)
             VALUES ('w1', 'Cien Años de Soledad');

        INSERT INTO editions (id, work_id, title)
             VALUES ('e1', 'w1', 'Cien Años de Soledad');

        INSERT INTO library_roots (id, name, path, kind)
             VALUES ('r1', 'Mi Disco', '/tmp/nb-humo', 'referenced');

        INSERT INTO files (id, edition_id, root_id, relative_path, content_hash,
                           byte_size, media_type, storage_mode)
             VALUES ('f1', 'e1', 'r1', 'cien.epub', 'abc123', 4096,
                     'application/epub+zip', 'managed');

        INSERT INTO annotations (id, edition_id, kind, body, target)
             VALUES ('a1', 'e1', 'note', 'Lo importante es el hielo',
                     '{"version":1,"href":"c1.xhtml","mediaType":"application/xhtml+xml",
                       "locations":{"progression":0.42},"text":{"highlight":"el hielo"}}');

        INSERT INTO settings (key, value)
             VALUES ('theme', '"pergamino"');

        INSERT INTO collections (id, name, kind)
             VALUES ('c1', 'Para el verano', 'manual');

        INSERT INTO collection_items (collection_id, edition_id, position)
             VALUES ('c1', 'e1', 0);

        INSERT INTO progress (edition_id, profile, locator, precision)
             VALUES ('e1', 'default',
                     '{"version":1,"href":"c3.xhtml","mediaType":"application/xhtml+xml",
                       "locations":{"progression":0.6}}', 'exact');
        "#,
    )
    .expect("contenido de la versión anterior");
    drop(conn);

    // Aquí ocurre la actualización: la versión nueva abre la base de la vieja.
    let db = Database::open(&ruta).expect("la base de la versión anterior se abre");
    assert_eq!(db.schema_version().expect("versión"), version_objetivo());

    let conn = db.connection().expect("conexión");
    let texto = |sql: &str| -> String { conn.query_row(sql, [], |f| f.get(0)).expect("consulta") };

    assert_eq!(
        texto("SELECT canonical_title FROM works WHERE id = 'w1'"),
        "Cien Años de Soledad",
        "la obra"
    );
    assert_eq!(
        texto("SELECT title FROM editions WHERE id = 'e1'"),
        "Cien Años de Soledad",
        "la edición"
    );
    assert_eq!(
        texto("SELECT relative_path FROM files WHERE id = 'f1'"),
        "cien.epub",
        "el archivo"
    );
    assert_eq!(
        texto("SELECT body FROM annotations WHERE id = 'a1'"),
        "Lo importante es el hielo",
        "**la anotación**, que es lo que no se puede recuperar de ninguna otra parte"
    );
    assert_eq!(
        texto("SELECT value FROM settings WHERE key = 'theme'"),
        "\"pergamino\"",
        "la preferencia"
    );
    assert_eq!(
        texto("SELECT name FROM collections WHERE id = 'c1'"),
        "Para el verano",
        "la colección"
    );

    // El ancla de la anotación tiene que llegar entera: sin sus selectores, la
    // anotación existe y no se puede volver a colocar, que para el usuario es lo
    // mismo que haberla perdido.
    let ancla = texto("SELECT target FROM annotations WHERE id = 'a1'");
    assert!(
        ancla.contains("\"progression\":0.42"),
        "la progresión del ancla"
    );
    assert!(ancla.contains("el hielo"), "la cita del ancla");

    // Y la posición de lectura, que es lo primero que se nota si se pierde.
    let posicion = texto("SELECT locator FROM progress WHERE edition_id = 'e1'");
    assert!(posicion.contains("c3.xhtml"), "la posición de lectura");
}

/// Migrar deja **una copia** de la base antes de tocarla.
///
/// Es la red por debajo de todo lo anterior: si una migración futura resultara
/// destructiva a pesar de estas pruebas, el usuario todavía tendría de dónde
/// sacar su biblioteca. Una migración sin copia previa convierte cualquier
/// defecto en una pérdida definitiva.
#[test]
fn migrar_deja_una_copia_de_lo_que_habia_antes() {
    let temporal = Temporal::nuevo("respaldo");
    let ruta = temporal.archivo("biblioteca.sqlite");
    let anterior = version_objetivo() - 1;

    let conn = base_de_version(&ruta, anterior);
    conn.execute_batch("INSERT INTO works (id, canonical_title) VALUES ('w1', 'Rayuela');")
        .expect("contenido");
    drop(conn);

    let _db = Database::open(&ruta).expect("se abre y migra");

    let copias: Vec<PathBuf> = std::fs::read_dir(temporal.archivo(""))
        .expect("directorio")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("biblioteca") && n.contains("backup"))
        })
        .collect();

    assert!(
        !copias.is_empty(),
        "no se guardó ninguna copia antes de migrar; las que había: {:?}",
        std::fs::read_dir(temporal.archivo(""))
            .expect("directorio")
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect::<Vec<_>>()
    );

    // Y la copia tiene lo que había: una copia ilegible no es una copia.
    let copia = rusqlite::Connection::open(&copias[0]).expect("copia legible");
    let titulo: String = copia
        .query_row(
            "SELECT canonical_title FROM works WHERE id = 'w1'",
            [],
            |f| f.get(0),
        )
        .expect("la copia conserva el contenido");
    assert_eq!(titulo, "Rayuela");
}

/// **Regresión.** Los respaldos no se acumulan: se queda el último.
///
/// Cada respaldo es la base entera, y antes no se borraba ninguno:
/// una instalación acumulaba un duplicado completo por cada salto de versión que
/// hubiera dado. Sobre la biblioteca real, con la base en 24 GB, son 24 GB por
/// copia, y ni el presupuesto de lo derivado los contaba.
#[test]
fn un_respaldo_nuevo_se_lleva_por_delante_los_anteriores() {
    let temporal = Temporal::nuevo("respaldos-viejos");
    let ruta = temporal.archivo("biblioteca.sqlite");

    let conn = base_de_version(&ruta, version_objetivo() - 1);
    conn.execute_batch("INSERT INTO works (id, canonical_title) VALUES ('w1', 'Rayuela');")
        .expect("contenido");
    drop(conn);

    // Dos respaldos de migraciones anteriores, como los que habría acumulado
    // cualquier instalación con unos meses encima.
    for version in [3_u32, 11] {
        let mut vieja = ruta.as_os_str().to_owned();
        vieja.push(format!(".v{version}.backup"));
        std::fs::write(PathBuf::from(&vieja), b"copia antigua").expect("respaldo viejo");
    }

    let _db = Database::open(&ruta).expect("se abre y migra");

    let copias: Vec<String> = std::fs::read_dir(temporal.archivo(""))
        .expect("directorio")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("backup"))
        .collect();

    assert_eq!(
        copias.len(),
        1,
        "tenía que quedar una sola copia y quedaron: {copias:?}"
    );
    assert!(
        copias[0].ends_with(&format!(".v{}.backup", version_objetivo() - 1)),
        "la que queda tiene que ser la de esta migración, y es {}",
        copias[0]
    );
}

/// Una base **más nueva** que la aplicación no se toca.
///
/// Es el caso de quien abre su biblioteca con una versión antigua después de
/// haberla usado con una nueva. Migrar hacia atrás no existe, así que lo único
/// correcto es negarse: intentarlo destruiría datos que esta versión no entiende.
#[test]
fn una_base_del_futuro_se_rechaza_en_lugar_de_intentar_arreglarla() {
    let temporal = Temporal::nuevo("futuro");
    let ruta = temporal.archivo("futura.sqlite");

    let conn = base_de_version(&ruta, version_objetivo());
    conn.pragma_update(None, "user_version", version_objetivo() + 5)
        .expect("versión del futuro");
    drop(conn);

    let resultado = Database::open(&ruta);
    assert!(
        resultado.is_err(),
        "una base más nueva que la aplicación tiene que rechazarse"
    );

    // Y sigue ahí, intacta: rechazarla no puede significar romperla.
    let conn = rusqlite::Connection::open(&ruta).expect("la base sigue abriéndose");
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |f| f.get(0))
        .expect("versión");
    assert_eq!(version, version_objetivo() + 5, "no se tocó");
}

/// La lista de arriba tiene que ser **todas** las migraciones.
///
/// Se escribe a mano porque `include_str!` necesita rutas literales, y una lista
/// escrita a mano se queda corta el día que alguien añade una migración y no se
/// acuerda de esto. Entonces la prueba de humo comprobaría el salto hasta la
/// penúltima versión y pasaría en verde sin haber mirado la última, que es justo
/// la que acaba de escribirse y la que más falta hace comprobar.
#[test]
fn la_lista_de_migraciones_de_esta_prueba_no_se_queda_atras() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let cuantas = std::fs::read_dir(&dir)
        .expect("directorio de migraciones")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "sql"))
        .count();

    assert_eq!(
        MIGRACIONES.len(),
        cuantas,
        "hay {cuantas} migraciones en el disco y {} en esta prueba: añade la que falta",
        MIGRACIONES.len()
    );
}

/// **Reconstruir una tabla no puede llevarse lo que cuelga de ella.**
///
/// La V20 amplía la lista cerrada de `kind` en `annotations`, y SQLite no permite
/// alterar un `CHECK`: hay que reconstruir la tabla. Con las claves ajenas
/// activas, soltar la vieja ejecuta un `DELETE` implícito que dispara los
/// `ON DELETE CASCADE` de quien la referencia, y aquí quien la referencia es
/// `note_annotation_links`: los enlaces entre las notas propias de quien lee y
/// los pasajes que citan.
///
/// Medido antes de escribir la migración, sobre una base de juguete: la
/// reconstrucción entera dentro de una transacción **confirma sin error** y deja
/// los hijos en cero. Por eso el ejecutor apaga las claves ajenas alrededor de
/// las migraciones y comprueba `foreign_key_check` al terminar.
///
/// Esta prueba es lo que impide que alguien lo deshaga sin enterarse: pone datos
/// de verdad antes de migrar y comprueba que siguen ahí después.
#[test]
fn reconstruir_annotations_no_se_lleva_los_enlaces_de_las_notas() {
    let temporal = Temporal::nuevo("enlaces");
    let ruta = temporal.archivo("v19.sqlite");

    {
        let conn = base_de_version(&ruta, 19);
        conn.execute_batch(
            "INSERT INTO works (id, canonical_title) VALUES ('w1', 'Obra');
             INSERT INTO editions (id, work_id, title) VALUES ('e1', 'w1', 'Edición');
             INSERT INTO annotations (id, edition_id, kind, target)
                 VALUES ('a1', 'e1', 'highlight', '{\"kind\":\"fragment\"}');
             INSERT INTO notes (id, title, body) VALUES ('n1', 'Una nota', 'cuerpo');
             INSERT INTO note_annotation_links (note_id, annotation_id) VALUES ('n1', 'a1');",
        )
        .expect("datos de partida");
    }

    let db = Database::open(&ruta).expect("la base migra");
    let conn = db.connection().expect("conexión");

    let enlaces: i64 = conn
        .query_row("SELECT count(*) FROM note_annotation_links", [], |f| {
            f.get(0)
        })
        .expect("recuento");
    let anotaciones: i64 = conn
        .query_row("SELECT count(*) FROM annotations", [], |f| f.get(0))
        .expect("recuento");

    assert_eq!(
        enlaces, 1,
        "reconstruir annotations se llevó por delante el enlace de la nota"
    );
    assert_eq!(anotaciones, 1, "la anotación no sobrevivió a la migración");

    // Y las clases nuevas se admiten, que es para lo que se reconstruyó.
    conn.execute(
        "INSERT INTO annotations (id, edition_id, kind, target)
             VALUES ('a2', 'e1', 'underline', '{\"kind\":\"fragment\"}')",
        [],
    )
    .expect("el subrayado tiene que caber");
    conn.execute(
        "INSERT INTO annotations (id, edition_id, kind, target, payload)
             VALUES ('a3', 'e1', 'drawing', '{\"kind\":\"fragment\"}', '{\"trazos\":[]}')",
        [],
    )
    .expect("el trazo libre tiene que caber");

    // Un trazo sin geometría no es un trazo.
    conn.execute(
        "INSERT INTO annotations (id, edition_id, kind, target)
             VALUES ('a4', 'e1', 'drawing', '{\"kind\":\"fragment\"}')",
        [],
    )
    .expect_err("un trazo sin geometría tiene que rechazarse");
}

/// **La rejilla de V23 pasa a columnas sin perder el sitio de nada (V31).**
///
/// Cada columna usada de la rejilla es una columna con nombre, en el mismo orden,
/// y sus celdas son sus tarjetas de arriba abajo: lo que estaba a la izquierda
/// sigue a la izquierda y lo que estaba arriba sigue arriba. Una nota que estaba
/// en dos celdas del mismo tablero se queda en la primera en orden de lectura,
/// porque en columnas una nota está una sola vez en cada tablero.
#[test]
fn la_rejilla_de_un_tablero_pasa_a_columnas_sin_perder_el_sitio_de_nada() {
    let temporal = Temporal::nuevo("tableros");
    let ruta = temporal.archivo("v30.sqlite");

    {
        let conn = base_de_version(&ruta, 30);
        conn.execute_batch(
            "INSERT INTO notes (id, title, body) VALUES ('n1', 'Arriba', ''), ('n2', 'Abajo', ''),
                                                        ('n3', 'Derecha', '');
             INSERT INTO note_boards (id, title) VALUES ('b1', 'Tablero'), ('b2', 'Otro');
             INSERT INTO note_board_cells (board_id, row_index, col_index, note_id, links_to) VALUES
                 ('b1', 4, 1, 'n2', NULL),
                 ('b1', 0, 1, 'n1', NULL),
                 ('b1', 2, 6, 'n3', NULL),
                 ('b1', 5, 6, 'n1', NULL),
                 ('b1', 3, 6, NULL, 'b2');",
        )
        .expect("datos de partida");
    }

    let db = Database::open(&ruta).expect("la base migra");
    let conn = db.connection().expect("conexión");
    let contenido = nb_books_lib::boards::contenido(&conn, "b1").expect("contenido");

    let forma: Vec<(String, Vec<String>)> = contenido
        .iter()
        .map(|c| {
            (
                c.title.clone(),
                c.cards
                    .iter()
                    .map(|t| {
                        t.title
                            .clone()
                            .or_else(|| t.links_to_title.clone())
                            .unwrap_or_default()
                    })
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        forma,
        vec![
            (
                "Columna 2".to_owned(),
                vec!["Arriba".to_owned(), "Abajo".to_owned()]
            ),
            (
                "Columna 7".to_owned(),
                vec!["Derecha".to_owned(), "Otro".to_owned()]
            ),
        ],
        "la nota repetida se queda en su primera celda y el orden se conserva"
    );
    let vieja: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'note_board_cells'",
            [],
            |f| f.get(0),
        )
        .expect("esquema");
    assert_eq!(vieja, 0, "la rejilla no se queda como una tabla muerta");
}
