//! Criterio de aceptación de la.
//!
//! "Importar una carpeta con duplicados exactos los reporta como tales, con su
//! cuenta, y no crea dos entradas."
//!
//! La identidad por contenido ya los reconocía, y los contaba junto a lo
//! que "ya estaba". Aquí se comprueba que ahora se cuentan aparte en los tres sitios
//! donde se leen -el informe, el avance y el fin que publica el bus, y la ejecución
//! guardada que sobrevive a cerrar el panel-, que cada copia dice de qué es copia, y
//! que no entra ni una segunda entrada.
//!
//! **Por qué no se comprueba importando el corpus real**, como pedía la ficha: el
//! corpus no trae duplicados exactos. Su importación del 2026-09-12 procesó 152.080
//! archivos, importó 152.080 y no encontró ninguno que ya estuviera, y la base tiene
//! 152.080 huellas SHA-256 distintas para 152.080 archivos. Está en
//! `docs/architecture/storage.md`, sección 2.3.
//!
//! Contrato: `docs/architecture/storage.md`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::imports;
use nb_books_lib::library::{import_directory, BulkReport, CancelToken, ImportContext};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    db: Database,
    bus: EventBus,
    eventos: Arc<Mutex<Vec<DomainEvent>>>,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-copias-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        std::fs::create_dir_all(&scope).expect("ámbito");
        std::fs::create_dir_all(&library).expect("biblioteca");
        let bus = EventBus::new();
        let eventos = Arc::new(Mutex::new(Vec::new()));
        let destino = Arc::clone(&eventos);
        bus.subscribe("copias", move |evento| {
            if matches!(
                evento,
                DomainEvent::BulkImportProgress { .. } | DomainEvent::BulkImportFinished { .. }
            ) {
                destino
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(evento.clone());
            }
        });
        let db = Database::open(&dir.join("nb-books.sqlite")).expect("base");
        Self {
            dir,
            scope,
            library,
            db,
            bus,
            eventos,
        }
    }

    /// Escribe `contenido` en `relativa`, dentro de la carpeta que se importa.
    fn escribir(&self, relativa: &str, contenido: &[u8]) {
        let ruta = self.scope.join("carpeta").join(relativa);
        std::fs::create_dir_all(ruta.parent().expect("padre")).expect("carpeta");
        std::fs::write(ruta, contenido).expect("archivo");
    }

    fn importar(&self) -> BulkReport {
        let mut conn = self.db.connection().expect("conexión");
        import_directory(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &self.scope.join("carpeta"),
            StorageMode::Referenced,
            &CancelToken::new(),
        )
        .expect("importación")
    }

    fn filas(&self, tabla: &str) -> i64 {
        self.db
            .connection()
            .expect("conexión")
            .query_row(&format!("SELECT count(*) FROM {tabla}"), [], |r| r.get(0))
            .expect("consulta")
    }

    /// El último fin publicado, **esperándolo**: el bus entrega a cada suscriptor
    /// desde su propio hilo, así que al volver la importación el fin puede no haber
    /// llegado todavía. Leerlo sin esperar fallaba una de cada tres veces.
    fn ultimo_fin(&self) -> DomainEvent {
        let tope = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let fin = self
                .eventos
                .lock()
                .expect("eventos")
                .iter()
                .rev()
                .find(|e| matches!(e, DomainEvent::BulkImportFinished { .. }))
                .cloned();
            match fin {
                Some(fin) => return fin,
                None if std::time::Instant::now() < tope => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                None => panic!("el fin de la importación no llegó al suscriptor"),
            }
        }
    }

    /// Se lee después de [`Self::ultimo_fin`]: un suscriptor recibe en orden, así
    /// que con el fin ya han llegado todos los avances.
    fn ultimo_avance(&self) -> DomainEvent {
        self.eventos
            .lock()
            .expect("eventos")
            .iter()
            .rev()
            .find(|e| matches!(e, DomainEvent::BulkImportProgress { .. }))
            .cloned()
            .expect("hubo avance")
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Un PDF válido con identidad propia: el comentario va tras el `%%EOF`.
fn pdf(ejemplar: &str) -> Vec<u8> {
    let mut contenido = std::fs::read(fixtures().join("minimo-valido.pdf")).expect("plantilla");
    contenido.extend_from_slice(format!("\n% {ejemplar}\n%%EOF\n").as_bytes());
    contenido
}

/// Una carpeta con tres libros distintos, uno de ellos tres veces y otro dos, en
/// subcarpetas distintas y con nombres distintos: como llegan de verdad.
fn carpeta_con_copias(esc: &Escenario) {
    esc.escribir("autor-a/uno.pdf", &pdf("uno"));
    esc.escribir("autor-a/copias/uno (1).pdf", &pdf("uno"));
    esc.escribir("otro autor/uno-otra-vez.pdf", &pdf("uno"));
    esc.escribir("autor-b/dos.pdf", &pdf("dos"));
    esc.escribir("descargas/dos.pdf", &pdf("dos"));
    esc.escribir("autor-c/tres.pdf", &pdf("tres"));
}

#[test]
fn las_copias_exactas_se_cuentan_aparte_y_no_crean_entradas() {
    let esc = Escenario::nuevo("cuenta");
    carpeta_con_copias(&esc);

    let informe = esc.importar();
    assert_eq!(informe.processed, 6);
    assert_eq!(informe.imported, 3);
    assert_eq!(informe.duplicates, 3, "{informe:?}");
    // Lo que ya estaba es otra cosa: aquí no había nada antes.
    assert_eq!(informe.already_in_library, 0, "{informe:?}");
    assert!(informe.rejected.is_empty(), "{informe:?}");

    assert_eq!(esc.filas("works"), 3);
    assert_eq!(esc.filas("editions"), 3);
    assert_eq!(esc.filas("files"), 3);
}

#[test]
fn el_bus_publica_las_copias_en_el_avance_y_en_el_fin() {
    let esc = Escenario::nuevo("bus");
    carpeta_con_copias(&esc);
    esc.importar();

    let DomainEvent::BulkImportFinished {
        duplicates,
        skipped,
        imported,
        ..
    } = esc.ultimo_fin()
    else {
        unreachable!()
    };
    assert_eq!((imported, skipped, duplicates), (3, 0, 3));
    let DomainEvent::BulkImportProgress { duplicates, .. } = esc.ultimo_avance() else {
        unreachable!()
    };
    assert_eq!(duplicates, 3);
}

#[test]
fn la_ejecucion_guardada_dice_cuantas_y_de_que_es_copia_cada_una() {
    let esc = Escenario::nuevo("guardada");
    carpeta_con_copias(&esc);
    esc.importar();

    let conn = esc.db.connection().expect("conexión");
    let ejecucion = imports::ultima(&conn)
        .expect("consulta")
        .expect("hay ejecución");
    assert_eq!(ejecucion.duplicates, 3);
    assert_eq!(ejecucion.already_in_library, 0);

    let copias = imports::copias(&conn, &ejecucion.id, 50, 0).expect("copias");
    let mut vistas: Vec<(String, String)> = copias
        .iter()
        .map(|c| (c.path.clone(), c.original_path.clone()))
        .collect();
    vistas.sort();
    // La copia, relativa a la carpeta elegida como los rechazos; el original,
    // relativo a su raíz. El recorrido es alfabético, así que el que entra es el
    // primero por orden, y las otras son copias de él.
    assert_eq!(
        vistas,
        vec![
            (
                "autor-a/copias/uno (1).pdf".to_owned(),
                "autor-a/uno.pdf".to_owned()
            ),
            ("descargas/dos.pdf".to_owned(), "autor-b/dos.pdf".to_owned()),
            (
                "otro autor/uno-otra-vez.pdf".to_owned(),
                "autor-a/uno.pdf".to_owned()
            ),
        ]
    );
    assert!(copias.iter().all(|c| !c.original_title.trim().is_empty()));
}

/// Reanudar o repetir la importación no convierte lo que ya estaba en copias ni
/// las copias en entradas: cada cosa sigue siendo lo que es.
#[test]
fn repetir_la_importacion_distingue_lo_que_ya_estaba_de_las_copias() {
    let esc = Escenario::nuevo("repetir");
    carpeta_con_copias(&esc);
    esc.importar();

    let segunda = esc.importar();
    assert_eq!(segunda.imported, 0);
    assert_eq!(segunda.already_in_library, 3, "{segunda:?}");
    assert_eq!(segunda.duplicates, 3, "{segunda:?}");
    assert_eq!(esc.filas("files"), 3);
}

/// Si lo que ya estaba sale de la biblioteca, la lista deja de decir que algo es
/// copia suya: la fila se va por la cascada, y el recuento, que es histórico, queda.
#[test]
fn una_copia_de_algo_que_ya_no_esta_deja_de_listarse() {
    let esc = Escenario::nuevo("cascada");
    carpeta_con_copias(&esc);
    esc.importar();

    let conn = esc.db.connection().expect("conexión");
    let ejecucion = imports::ultima(&conn)
        .expect("consulta")
        .expect("ejecución");
    conn.execute(
        "DELETE FROM editions WHERE id = (SELECT edition_id FROM files WHERE relative_path = ?1)",
        ["autor-a/uno.pdf"],
    )
    .expect("borrar");
    let copias = imports::copias(&conn, &ejecucion.id, 50, 0).expect("copias");
    assert_eq!(copias.len(), 1, "{copias:?}");
    assert_eq!(copias[0].path, "descargas/dos.pdf");
    assert_eq!(
        imports::ultima(&conn)
            .expect("consulta")
            .expect("ejecución")
            .duplicates,
        3
    );
}

/// La lista se acota y el recuento no: por encima del tope se deja de guardar, y
/// la cifra sigue siendo la de verdad.
#[test]
fn la_lista_de_copias_se_acota_y_el_recuento_no() {
    let esc = Escenario::nuevo("tope");
    let conn = esc.db.connection().expect("conexión");
    let run = imports::abrir(&conn, "/origen", StorageMode::Referenced).expect("ejecución");
    // Un archivo registrado del que colgar las copias.
    esc.escribir("original.pdf", &pdf("original"));
    drop(conn);
    esc.importar();
    let conn = esc.db.connection().expect("conexión");
    let file_id: String = conn
        .query_row("SELECT id FROM files", [], |f| f.get(0))
        .expect("archivo");
    for i in 0..(imports::COPIAS_POR_EJECUCION + 5) {
        imports::anotar_copia(&conn, &run, &format!("copia-{i}.pdf"), &file_id).expect("copia");
    }
    let guardadas: i64 = conn
        .query_row(
            "SELECT count(*) FROM import_duplicates WHERE run_id = ?1",
            [&run],
            |f| f.get(0),
        )
        .expect("recuento");
    assert_eq!(guardadas, imports::COPIAS_POR_EJECUCION as i64);
}
