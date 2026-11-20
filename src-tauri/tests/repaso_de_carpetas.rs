//! Lo que cambia en el disco se ve sin pedirlo.
//!
//! El criterio pide comprobar lo elegido "con una prueba que añade un archivo
//! mientras la aplicación está abierta". Aquí la aplicación es el runtime con su
//! base y su bus, y lo que se añade es un archivo de verdad en una carpeta
//! referenciada de verdad. Por qué es un repaso y no un vigilante está medido en
//! `docs/architecture/storage.md`, sección 2.2.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::library::{import_directory, CancelToken, ImportContext};
use nb_books_lib::repaso::Repaso;

struct Escenario {
    dir: PathBuf,
    raiz: PathBuf,
    biblioteca: PathBuf,
    db: Database,
    bus: EventBus,
    repaso: Repaso,
    eventos: mpsc::Receiver<DomainEvent>,
}

/// Un PDF distinto por número: el mínimo del corpus de pruebas con un
/// comentario al final, que cambia su huella sin dejar de ser un PDF. La
/// biblioteca identifica por contenido, así que dos copias iguales serían un
/// solo documento y la prueba no distinguiría "entró" de "ya estaba".
fn pdf(n: u32) -> Vec<u8> {
    let mut bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimo-valido.pdf"),
    )
    .expect("el PDF mínimo del corpus");
    bytes.extend_from_slice(format!("\n% ejemplar {n}\n").as_bytes());
    bytes
}

fn poner(ruta: &Path, n: u32) {
    std::fs::create_dir_all(ruta.parent().expect("carpeta")).expect("carpeta");
    std::fs::write(ruta, pdf(n)).expect("archivo");
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-repaso-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let raiz = dir.join("Mis Libros");
        let biblioteca = dir.join("biblioteca");
        std::fs::create_dir_all(&biblioteca).expect("biblioteca");
        poner(&raiz.join("Autora Uno").join("primero.pdf"), 1);
        poner(&raiz.join("Autor Dos").join("segundo.pdf"), 2);

        let db = Database::open(&dir.join("nb.sqlite")).expect("base");
        let bus = EventBus::new();
        let (tx, eventos) = mpsc::channel();
        let tx = Mutex::new(tx);
        bus.subscribe("prueba-repaso", move |e| {
            let _ = tx.lock().expect("canal").send(e.clone());
        });

        // La carpeta entra como la importa el usuario: una raíz referenciada.
        let mut conn = db.connection().expect("conexión");
        let informe = import_directory(
            &mut conn,
            &ImportContext {
                scope: &raiz,
                library: &biblioteca,
                bus: &bus,
            },
            &raiz,
            StorageMode::Referenced,
            &CancelToken::default(),
        )
        .expect("importación");
        assert_eq!(informe.imported, 2);
        drop(conn);

        let repaso = Repaso::nuevo(dir.join("cache/repaso"));
        Self {
            dir,
            raiz,
            biblioteca,
            db,
            bus,
            repaso,
            eventos,
        }
    }

    fn repasar(&self) -> nb_books_lib::repaso::Informe {
        self.repaso
            .repasar(&self.db, &self.bus, &self.biblioteca)
            .expect("repaso")
    }

    fn archivos(&self) -> i64 {
        let conn = self.db.connection().expect("conexión");
        conn.query_row("SELECT count(*) FROM files", [], |f| f.get(0))
            .expect("cuenta")
    }

    fn disponibilidad(&self, relativa: &str) -> String {
        let conn = self.db.connection().expect("conexión");
        conn.query_row(
            "SELECT availability FROM files WHERE relative_path = ?1",
            [relativa],
            |f| f.get(0),
        )
        .expect("disponibilidad")
    }

    /// Los nombres de los eventos que llegaron desde la última vez.
    fn anunciados(&self) -> Vec<&'static str> {
        std::thread::sleep(Duration::from_millis(100));
        self.eventos.try_iter().map(|e| e.name()).collect()
    }

    /// Espera a que la fecha de modificación de una carpeta pueda cambiar. En
    /// ext4 son nanosegundos, pero no todo sistema de archivos los tiene, y una
    /// prueba que depende de la resolución del reloj del disco falla de vez en
    /// cuando en la máquina de otro.
    fn dejar_pasar_el_reloj(&self) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn un_archivo_que_aparece_mientras_la_aplicacion_esta_abierta_entra_sin_pedirlo() {
    let e = Escenario::nuevo("aparece");
    let primero = e.repasar();
    assert_eq!(
        primero.fotos_nuevas, 1,
        "la primera vez solo se toma la foto"
    );
    let _ = e.anunciados();

    e.dejar_pasar_el_reloj();
    // Uno en una carpeta que ya estaba y otro en una carpeta nueva.
    poner(&e.raiz.join("Autor Dos").join("tercero.pdf"), 3);
    poner(&e.raiz.join("Autora Nueva").join("cuarto.pdf"), 4);

    let informe = e.repasar();
    assert_eq!(informe.importados, 2, "{informe:?}");
    assert_eq!(e.archivos(), 4);
    // Entran por la misma puerta que una importación: con su anuncio, que es lo
    // que hace que la biblioteca los pinte sin recargar.
    let anunciados = e.anunciados();
    assert_eq!(
        anunciados
            .iter()
            .filter(|n| **n == "DocumentImported")
            .count(),
        2,
        "{anunciados:?}"
    );
}

#[test]
fn sin_cambios_no_se_lista_ninguna_carpeta_ni_se_anuncia_nada() {
    let e = Escenario::nuevo("quieto");
    e.repasar();
    let _ = e.anunciados();

    let informe = e.repasar();
    assert_eq!(informe.carpetas, 3, "la raíz y sus dos carpetas se miran");
    assert_eq!(informe.cambiadas, 0, "y ninguna se lista: {informe:?}");
    assert!(e.anunciados().is_empty());
}

#[test]
fn quitar_un_archivo_lo_deja_ausente_y_devolverlo_lo_recupera() {
    let e = Escenario::nuevo("ausente");
    e.repasar();
    let _ = e.anunciados();

    e.dejar_pasar_el_reloj();
    let ruta = e.raiz.join("Autora Uno").join("primero.pdf");
    std::fs::remove_file(&ruta).expect("se quita");
    let informe = e.repasar();
    assert_eq!(informe.ausentes, 1, "{informe:?}");
    assert_eq!(e.disponibilidad("Autora Uno/primero.pdf"), "missing");
    // Sigue en la biblioteca: un archivo que falta no se lleva sus anotaciones.
    assert_eq!(e.archivos(), 2);
    assert!(e.anunciados().contains(&"AvailabilityChanged"));

    e.dejar_pasar_el_reloj();
    poner(&ruta, 1);
    let informe = e.repasar();
    assert_eq!(informe.vueltos, 1, "{informe:?}");
    assert_eq!(e.disponibilidad("Autora Uno/primero.pdf"), "available");
}

#[test]
fn lo_que_ya_estaba_al_tomar_la_primera_foto_no_se_importa_solo() {
    let e = Escenario::nuevo("primera");
    // Un archivo que el usuario no importó: estaba ahí antes de que existiera
    // el repaso, y no se sabe si faltaba por descuido o por decisión.
    poner(&e.raiz.join("Autora Uno").join("olvidado.pdf"), 9);

    let informe = e.repasar();
    assert_eq!(informe.importados, 0);
    assert_eq!(e.archivos(), 2);
}

#[test]
fn una_raiz_que_no_esta_no_deja_la_biblioteca_ausente() {
    let e = Escenario::nuevo("desconectada");
    e.repasar();

    // El disco se desconecta: la carpeta entera deja de estar.
    let movida = e.dir.join("en otro sitio");
    std::fs::rename(&e.raiz, &movida).expect("se mueve");
    let informe = e.repasar();
    assert_eq!(informe.raices, 0);
    assert_eq!(informe.ausentes, 0);
    assert_eq!(e.disponibilidad("Autora Uno/primero.pdf"), "available");
}

#[test]
fn una_carpeta_que_desaparece_deja_ausente_lo_que_tenia() {
    let e = Escenario::nuevo("carpeta");
    e.repasar();

    e.dejar_pasar_el_reloj();
    std::fs::remove_dir_all(e.raiz.join("Autor Dos")).expect("se borra");
    let informe = e.repasar();
    assert_eq!(informe.ausentes, 1, "{informe:?}");
    assert_eq!(e.disponibilidad("Autor Dos/segundo.pdf"), "missing");
}

#[test]
fn dos_repasos_seguidos_hacen_uno_y_una_importacion_en_marcha_ninguno() {
    let e = Escenario::nuevo("freno");
    let importacion = CancelToken::default();

    assert!(e
        .repaso
        .si_toca(&e.db, &e.bus, &e.biblioteca, &importacion)
        .expect("primero")
        .is_some());
    assert!(
        e.repaso
            .si_toca(&e.db, &e.bus, &e.biblioteca, &importacion)
            .expect("segundo")
            .is_none(),
        "volver a la ventana y entrar en la biblioteca son un solo gesto"
    );

    let otro = Repaso::nuevo(e.dir.join("cache/otro"));
    assert!(importacion.try_begin());
    assert!(
        otro.si_toca(&e.db, &e.bus, &e.biblioteca, &importacion)
            .expect("con importación")
            .is_none(),
        "una importación ya recorre lo que importa"
    );
    importacion.finish();
}
