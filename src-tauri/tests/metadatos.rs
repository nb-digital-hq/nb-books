//! Criterio de aceptación de la.
//!
//! "La importación termina sin esperar a la portada; un fallo al generarla no
//! impide que el documento exista."
//!
//! Las dos mitades dicen lo mismo desde ángulos distintos: **el enriquecimiento
//! no puede estar en el camino crítico de la importación**. Si lo estuviera, un
//! EPUB con una portada de veinte megabytes retrasaría la entrada del documento,
//! y uno con la portada rota impediría que entrara.
//!
//! Contrato: `docs/architecture/data-flows.md`, sección 1.

use std::path::{Path, PathBuf};

use nb_books_lib::db::catalog::{Contributor, Edition, StorageMode, Subject, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::metadata::{self, Field, Source};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    covers: PathBuf,
    db: Database,
    bus: EventBus,
}

impl Escenario {
    /// Escenario **sin** el servicio de metadatos conectado, para probar el
    /// enriquecimiento directamente.
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-meta-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        let covers = dir.join("cache/portadas");
        std::fs::create_dir_all(&scope).expect("ámbito");
        std::fs::create_dir_all(&library).expect("biblioteca");

        let db = Database::open(&dir.join("nb-books.sqlite")).expect("base");

        Self {
            dir,
            scope,
            library,
            covers,
            db,
            bus: EventBus::new(),
        }
    }

    /// El mismo escenario, con el servicio conectado al bus como en el arranque
    /// real.
    fn con_servicio(nombre: &str) -> Self {
        let esc = Self::nuevo(nombre);
        metadata::subscribe(&esc.bus, esc.db.clone(), esc.covers.clone());
        esc
    }

    fn escribir(&self, nombre: &str, contenido: &[u8]) -> PathBuf {
        let ruta = self.scope.join(nombre);
        std::fs::write(&ruta, contenido).expect("archivo");
        ruta
    }

    fn traer(&self, fixture: &str, como: &str) -> PathBuf {
        let destino = self.scope.join(como);
        std::fs::copy(fixtures().join(fixture), &destino).expect("copia");
        destino
    }

    fn importar(&self, ruta: &Path) -> nb_books_lib::library::Imported {
        let mut conn = self.db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: ruta.to_path_buf(),
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importado");

        let ImportOutcome::Imported(importado) = resultado else {
            panic!("debía importarse");
        };
        *importado
    }

    fn titulo(&self, edicion: &str) -> String {
        self.db
            .connection()
            .expect("conexión")
            .query_row("SELECT title FROM editions WHERE id = ?1", [edicion], |r| {
                r.get(0)
            })
            .expect("consulta")
    }

    /// Espera a que el servicio del bus haga su trabajo, sin dormir un tiempo
    /// fijo: una espera fija sería lenta o falsa según la máquina.
    /// Espera a que el servicio de fondo haga su trabajo.
    ///
    /// Diez segundos y no dos: la suite entera corre veinticinco binarios a la
    /// vez, y en una máquina cargada el hilo del bus puede tardar más que eso en
    /// llegar a ejecutarse. Un margen corto no detecta ningún defecto que uno
    /// largo no detecte también; lo único que añade es un fallo que no se
    /// reproduce y que hace desconfiar de una prueba que estaba bien.
    fn esperar<T>(&self, mut condicion: impl FnMut() -> Option<T>) -> Option<T> {
        for _ in 0..2_000 {
            if let Some(valor) = condicion() {
                return Some(valor);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        None
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Construye un EPUB con los metadatos y la portada que se le pidan.
fn epub_con(opf: &str, portada: Option<(&str, &[u8])>) -> Vec<u8> {
    use std::io::Write;

    let mut entradas: Vec<(String, Vec<u8>, bool)> = vec![
        (
            "mimetype".to_owned(),
            b"application/epub+zip".to_vec(),
            false,
        ),
        (
            "META-INF/container.xml".to_owned(),
            br#"<?xml version="1.0"?><container version="1.0"
                 xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
                 <rootfile full-path="OEBPS/content.opf"
                 media-type="application/oebps-package+xml"/></rootfiles></container>"#
                .to_vec(),
            true,
        ),
        (
            "OEBPS/content.opf".to_owned(),
            opf.as_bytes().to_vec(),
            true,
        ),
    ];
    if let Some((nombre, bytes)) = portada {
        entradas.push((format!("OEBPS/{nombre}"), bytes.to_vec(), true));
    }

    let mut salida = Vec::new();
    let mut central = Vec::new();
    let mut cuantas = 0_u16;

    for (nombre, contenido, comprimir) in &entradas {
        let offset = u32::try_from(salida.len()).expect("cabe");
        let datos = if *comprimir {
            let mut cod =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            cod.write_all(contenido).expect("comprime");
            cod.finish().expect("comprime")
        } else {
            contenido.clone()
        };
        let metodo: u16 = if *comprimir { 8 } else { 0 };
        let comp = u32::try_from(datos.len()).expect("cabe");
        let desc = u32::try_from(contenido.len()).expect("cabe");
        let nb = nombre.as_bytes();

        salida.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        salida.extend_from_slice(&[20, 0, 0, 0]);
        salida.extend_from_slice(&metodo.to_le_bytes());
        salida.extend_from_slice(&[0; 8]);
        salida.extend_from_slice(&comp.to_le_bytes());
        salida.extend_from_slice(&desc.to_le_bytes());
        salida.extend_from_slice(&u16::try_from(nb.len()).expect("cabe").to_le_bytes());
        salida.extend_from_slice(&[0, 0]);
        salida.extend_from_slice(nb);
        salida.extend_from_slice(&datos);

        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
        central.extend_from_slice(&metodo.to_le_bytes());
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&comp.to_le_bytes());
        central.extend_from_slice(&desc.to_le_bytes());
        central.extend_from_slice(&u16::try_from(nb.len()).expect("cabe").to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(nb);
        cuantas += 1;
    }

    let offset_central = u32::try_from(salida.len()).expect("cabe");
    let tamano_central = u32::try_from(central.len()).expect("cabe");
    salida.extend_from_slice(&central);
    salida.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    salida.extend_from_slice(&[0; 4]);
    salida.extend_from_slice(&cuantas.to_le_bytes());
    salida.extend_from_slice(&cuantas.to_le_bytes());
    salida.extend_from_slice(&tamano_central.to_le_bytes());
    salida.extend_from_slice(&offset_central.to_le_bytes());
    salida.extend_from_slice(&[0, 0]);
    salida
}

const OPF_COMPLETO: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Cien años de soledad</dc:title>
    <dc:creator>Gabriel García Márquez</dc:creator>
    <dc:language>es</dc:language>
    <dc:publisher>Editorial Sudamericana</dc:publisher>
    <dc:date>1967-05-30T00:00:00Z</dc:date>
    <dc:identifier opf:scheme="ISBN">9789500720021</dc:identifier>
    <dc:subject>Realismo mágico</dc:subject>
  </metadata>
  <manifest>
    <item id="cub" href="cover.png" media-type="image/png" properties="cover-image"/>
  </manifest>
</package>"#;

// --- El criterio de aceptación ---------------------------------------------

/// **Primera mitad.** La importación termina sin esperar a la portada.
///
/// Se comprueba de la forma que de verdad lo demuestra: en el instante en que
/// `import_document` devuelve, el documento **ya existe** y la portada **todavía
/// no**. Si el enriquecimiento estuviera en el camino crítico, las dos cosas
/// serían ciertas a la vez.
#[test]
fn la_importacion_termina_sin_esperar_a_la_portada() {
    let esc = Escenario::con_servicio("no-espera");
    // Una portada grande y **que no se comprime**, como cualquier PNG o JPEG
    // real: una imagen de dos megabytes de un solo byte repetido no existe, y
    // además el lector la rechazaría por ratio, con razón.
    let portada: Vec<u8> = (0..2 * 1024 * 1024_u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let ruta = esc.escribir(
        "libro.epub",
        &epub_con(OPF_COMPLETO, Some(("cover.png", &portada))),
    );

    let importado = esc.importar(&ruta);

    // El documento existe ya.
    let conn = esc.db.connection().expect("conexión");
    assert!(Work::find(&conn, &importado.work.id)
        .expect("consulta")
        .is_some());

    // Y el trabajo de fondo llega después, sin que nadie lo haya esperado.
    let portada_final = esc
        .esperar(|| {
            let ruta = esc.covers.join(format!("{}.png", importado.edition.id));
            ruta.exists().then_some(ruta)
        })
        .expect("la portada debe acabar apareciendo");

    assert_eq!(
        std::fs::metadata(&portada_final).expect("metadatos").len(),
        portada.len() as u64
    );
}

/// **Segunda mitad.** Un fallo al generar la portada no impide que el documento
/// exista, ni se lleva por delante el resto de los metadatos.
#[test]
fn un_fallo_al_generar_la_portada_no_impide_que_el_documento_exista() {
    let esc = Escenario::con_servicio("portada-rota");

    // El OPF declara una portada que no está en el contenedor.
    let opf = OPF_COMPLETO.replace("cover.png", "no-existe.png");
    let ruta = esc.escribir("libro.epub", &epub_con(&opf, None));

    let importado = esc.importar(&ruta);

    // El resto de los metadatos sí entra: el fallo de la portada es local.
    let titulo = esc
        .esperar(|| {
            let t = esc.titulo(&importado.edition.id);
            (t == "Cien años de soledad").then_some(t)
        })
        .expect("el título interno debe aplicarse aunque la portada falle");
    assert_eq!(titulo, "Cien años de soledad");

    // El contribuidor se espera, no se lee de seguido: `enrich` escribe el
    // título y los autores en operaciones distintas, así que ver el título
    // puesto no significa que los autores estén ya. Leerlo sin esperar hacía
    // fallar esta prueba de vez en cuando, y solo de vez en cuando.
    let conn = esc.db.connection().expect("conexión");
    let autores = esc
        .esperar(|| {
            let cuantos = Contributor::list_for_edition(&conn, &importado.edition.id)
                .expect("contribuidores")
                .len();
            (cuantos > 0).then_some(cuantos)
        })
        .expect("el autor interno debe aplicarse aunque la portada falle");
    assert_eq!(autores, 1);
    assert!(!esc
        .covers
        .join(format!("{}.png", importado.edition.id))
        .exists());
}

/// Y lo mismo con un documento cuyos metadatos no se dejan leer en absoluto: el
/// documento entró y ahí se queda, con el nombre de su archivo.
#[test]
fn un_documento_ilegible_para_el_extractor_sigue_estando_en_la_biblioteca() {
    let esc = Escenario::con_servicio("ilegible");
    // Un paquete mal formado. No se usa `xml-malformado.epub` del corpus porque
    // lo que está roto ahí es el capítulo, no el paquete: sus metadatos se leen
    // perfectamente, que es justo lo que su ficha describe.
    let ruta = esc.escribir(
        "roto.epub",
        &epub_con(
            "<package><metadata><dc:title>Sin cerrar</metadata></package>",
            None,
        ),
    );

    let importado = esc.importar(&ruta);

    // Se le da tiempo al servicio a fallar.
    std::thread::sleep(std::time::Duration::from_millis(80));

    let conn = esc.db.connection().expect("conexión");
    assert!(Work::find(&conn, &importado.work.id)
        .expect("consulta")
        .is_some());
    assert_eq!(esc.titulo(&importado.edition.id), "roto");
}

// --- Qué se extrae ----------------------------------------------------------

#[test]
fn se_incorporan_titulo_idioma_editorial_fecha_autores_materias_e_identificadores() {
    let esc = Escenario::nuevo("completo");
    let ruta = esc.escribir(
        "libro.epub",
        &epub_con(OPF_COMPLETO, Some(("cover.png", b"PNG falso"))),
    );
    let importado = esc.importar(&ruta);
    let conn = esc.db.connection().expect("conexión");

    let informe = metadata::enrich(
        &conn,
        &importado.edition.id,
        &nb_books_lib::library::resolve_path(&conn, &importado.file)
            .expect("ruta")
            .expect("raíz localizada"),
        "application/epub+zip",
        &esc.covers,
    )
    .expect("enriquecido");

    assert!(informe.applied.contains(&"title".to_owned()));
    assert!(informe.applied.contains(&"language".to_owned()));
    assert!(informe.applied.contains(&"publisher".to_owned()));
    assert!(informe.applied.contains(&"published_date".to_owned()));
    assert_eq!(informe.contributors, 1);
    assert_eq!(informe.subjects, 1);
    assert_eq!(informe.identifiers, 1);
    assert!(informe.cover.is_some());

    let ediciones = Edition::list_for_work(&conn, &importado.work.id).expect("ediciones");
    assert_eq!(ediciones[0].title, "Cien años de soledad");
    assert_eq!(ediciones[0].language.as_deref(), Some("es"));
    assert_eq!(
        ediciones[0].publisher.as_deref(),
        Some("Editorial Sudamericana")
    );
    // La fecha del EPUB traía hora y zona; el esquema solo admite tres formas.
    assert_eq!(ediciones[0].published_date.as_deref(), Some("1967-05-30"));

    // Y el título canónico de la obra sigue al de la edición: una obra que se
    // llama como su archivo y una edición que se llama bien se busca mal.
    let obra = Work::find(&conn, &importado.work.id)
        .expect("consulta")
        .expect("existe");
    assert_eq!(obra.canonical_title, "Cien años de soledad");

    assert_eq!(
        Edition::identifiers(&conn, &importado.edition.id).expect("identificadores"),
        vec![("isbn".to_owned(), "9789500720021".to_owned())]
    );
    assert!(Subject::list_for_edition(&conn, &importado.edition.id)
        .expect("materias")
        .iter()
        .any(|s| s.label == "Realismo mágico"));
}

/// **Un autor, una fila.** Dos libros del mismo autor, escrito de dos
/// maneras, dejan una sola fila en `contributors`, y las dos obras la comparten.
/// Antes cada obra daba de alta a su autor otra vez.
#[test]
fn dos_libros_del_mismo_autor_comparten_su_fila() {
    let esc = Escenario::nuevo("mismo-autor");
    let conn = esc.db.connection().expect("conexión");
    let mut ediciones = Vec::new();
    for (archivo, titulo, autor) in [
        ("uno.epub", "Cien años de soledad", "Gabriel García Márquez"),
        (
            "dos.epub",
            "El otoño del patriarca",
            "GABRIEL GARCIA MARQUEZ",
        ),
    ] {
        let opf = OPF_COMPLETO
            .replace("Cien años de soledad", titulo)
            .replace("Gabriel García Márquez", autor)
            .replace("9789500720021", &format!("97895007200{}", titulo.len()));
        let ruta = esc.escribir(archivo, &epub_con(&opf, None));
        let importado = esc.importar(&ruta);
        metadata::enrich(
            &conn,
            &importado.edition.id,
            &nb_books_lib::library::resolve_path(&conn, &importado.file)
                .expect("ruta")
                .expect("raíz localizada"),
            "application/epub+zip",
            &esc.covers,
        )
        .expect("enriquecido");
        ediciones.push(importado.edition.id);
    }

    let filas: i64 = conn
        .query_row("SELECT count(*) FROM contributors", [], |f| f.get(0))
        .expect("cuenta");
    assert_eq!(
        filas, 1,
        "el mismo autor, escrito de otra forma, es una fila"
    );
    let de = |edicion: &str| -> String {
        Contributor::list_for_edition(&conn, edicion).expect("autores")[0]
            .0
            .id
            .clone()
    };
    assert_eq!(de(&ediciones[0]), de(&ediciones[1]));
}

/// **Lo que hace que todo esto sea seguro de ejecutar.** El documento no puede
/// pisar lo que el usuario decidió a mano; discrepa, y su valor queda propuesto.
#[test]
fn los_metadatos_internos_no_pisan_una_correccion_del_usuario() {
    let esc = Escenario::nuevo("precedencia");
    let ruta = esc.escribir("libro.epub", &epub_con(OPF_COMPLETO, None));
    let importado = esc.importar(&ruta);
    let conn = esc.db.connection().expect("conexión");

    // El usuario corrige el título antes de que llegue la extracción.
    metadata::set_manual(
        &conn,
        &importado.edition.id,
        Field::Title,
        Some("Como yo lo llamo"),
    )
    .expect("corrección");

    let informe = metadata::enrich(
        &conn,
        &importado.edition.id,
        &nb_books_lib::library::resolve_path(&conn, &importado.file)
            .expect("ruta")
            .expect("raíz localizada"),
        "application/epub+zip",
        &esc.covers,
    )
    .expect("enriquecido");

    assert!(informe.suggested.contains(&"title".to_owned()));
    assert!(!informe.applied.contains(&"title".to_owned()));
    assert_eq!(esc.titulo(&importado.edition.id), "Como yo lo llamo");

    // Y lo que el documento decía queda disponible como propuesta.
    let propuestas = metadata::pending(&conn, metadata::EntityType::Edition, &importado.edition.id)
        .expect("propuestas");
    assert!(propuestas
        .iter()
        .any(|p| p.proposed_value.as_deref() == Some("Cien años de soledad")));

    // El idioma, que nadie tocó, sí se aplica.
    assert!(informe.applied.contains(&"language".to_owned()));
}

#[test]
fn la_procedencia_del_titulo_queda_registrada_como_del_archivo() {
    let esc = Escenario::nuevo("procedencia");
    let ruta = esc.escribir("libro.epub", &epub_con(OPF_COMPLETO, None));
    let importado = esc.importar(&ruta);
    let conn = esc.db.connection().expect("conexión");

    metadata::enrich(
        &conn,
        &importado.edition.id,
        &nb_books_lib::library::resolve_path(&conn, &importado.file)
            .expect("ruta")
            .expect("raíz localizada"),
        "application/epub+zip",
        &esc.covers,
    )
    .expect("enriquecido");

    let historial =
        metadata::history(&conn, &importado.edition.id, Field::Title).expect("historial");
    assert_eq!(historial.len(), 1);
    assert_eq!(historial[0].source, Source::File);
    assert_eq!(historial[0].value.as_deref(), Some("Cien años de soledad"));
}

/// La portada va a la caché, no a los datos: es regenerable, y la regla 5 del
/// esquema manda que lo regenerable viva fuera de lo que se respalda.
#[test]
fn la_portada_se_guarda_en_la_cache_y_no_en_los_datos() {
    let esc = Escenario::nuevo("cache");
    let ruta = esc.escribir(
        "libro.epub",
        &epub_con(OPF_COMPLETO, Some(("cover.png", b"bytes de la portada"))),
    );
    let importado = esc.importar(&ruta);
    let conn = esc.db.connection().expect("conexión");

    let informe = metadata::enrich(
        &conn,
        &importado.edition.id,
        &nb_books_lib::library::resolve_path(&conn, &importado.file)
            .expect("ruta")
            .expect("raíz localizada"),
        "application/epub+zip",
        &esc.covers,
    )
    .expect("enriquecido");

    let portada = informe.cover.expect("portada");
    assert!(portada.starts_with(&esc.covers));
    assert!(!portada.starts_with(&esc.library));
    // Se guarda tal cual viene, sin recodificar.
    assert_eq!(
        std::fs::read(&portada).expect("lectura"),
        b"bytes de la portada"
    );
}

#[test]
fn un_formato_sin_extractor_todavia_lo_dice_en_lugar_de_fingir() {
    let esc = Escenario::nuevo("sin-extractor");
    let ruta = esc.traer("minimo-valido.pdf", "documento.pdf");
    let importado = esc.importar(&ruta);
    let conn = esc.db.connection().expect("conexión");

    assert_eq!(
        metadata::enrich(
            &conn,
            &importado.edition.id,
            &nb_books_lib::library::resolve_path(&conn, &importado.file)
                .expect("ruta")
                .expect("raíz localizada"),
            "application/x-formato-que-no-existe",
            &esc.covers,
        ),
        Err(NbError::FormatUnsupported)
    );
}

/// Un PDF deja su primera página como portada, y un cómic su primera imagen.
#[test]
fn un_pdf_y_un_comic_dejan_portada_desde_su_primera_pagina() {
    let esc = Escenario::nuevo("portada-pdf-cbz");
    let conn = esc.db.connection().expect("conexión");

    let comic = esc.importar(&esc.traer("minimo-valido.cbz", "comic.cbz"));
    let informe = metadata::enrich(
        &conn,
        &comic.edition.id,
        &nb_books_lib::library::resolve_path(&conn, &comic.file)
            .expect("ruta")
            .expect("raíz localizada"),
        "application/vnd.comicbook+zip",
        &esc.covers,
    )
    .expect("el cómic se lee");
    assert!(informe.cover.is_some(), "el cómic tiene que dejar portada");

    // El PDF necesita el motor; sin él la prueba no tiene nada que comprobar.
    if nb_books_lib::reader::pdf::engine_state() != nb_books_lib::reader::pdf::Engine::Available {
        return;
    }
    let pdf = esc.importar(&esc.traer("minimo-valido.pdf", "documento.pdf"));
    let informe = metadata::enrich(
        &conn,
        &pdf.edition.id,
        &nb_books_lib::library::resolve_path(&conn, &pdf.file)
            .expect("ruta")
            .expect("raíz localizada"),
        "application/pdf",
        &esc.covers,
    )
    .expect("el PDF se lee");
    assert!(informe.cover.is_some(), "el PDF tiene que dejar portada");
}

/// Las portadas ya extraídas antes de que hubiera dónde apuntarlas se registran.
///
/// Es el caso de cualquier biblioteca importada antes de la V14: sus portadas
/// están en la caché y ninguna en el catálogo, así que la interfaz pintaría la
/// generada teniendo la buena al lado.
/// **Con la base ocupada, la pasada se para y lo dice una vez.**
///
/// El 2026-09-12 la primera versión seguía con la fila siguiente después de cada
/// bloqueo. Cada intento espera el `busy_timeout` y escribe su aviso: con otro
/// escritor delante salieron **dieciocho avisos idénticos en noventa segundos**, y
/// con las novecientas filas de esa pasada habrían sido más de una hora de lo
/// mismo. Es ruido que tapa el registro justo cuando hay algo que leer en él.
#[test]
fn con_la_base_ocupada_la_pasada_se_aplaza_en_vez_de_insistir() {
    let esc = Escenario::nuevo("titulos-ocupada");
    let conn = esc.db.connection().expect("conexión");
    let obra = nb_books_lib::db::catalog::Work::create(
        &conn,
        &nb_books_lib::db::catalog::NewWork {
            canonical_title: "Uno_dos".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = nb_books_lib::db::catalog::Edition::create(
        &conn,
        &nb_books_lib::db::catalog::NewEdition {
            work_id: obra.id,
            title: "Uno_dos".to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición");
    nb_books_lib::db::catalog::File::create(
        &conn,
        &nb_books_lib::db::catalog::NewFile {
            edition_id: edicion.id.clone(),
            root_id: nb_books_lib::db::catalog::MANAGED_ROOT_ID.to_owned(),
            relative_path: "Uno_dos.epub".to_owned(),
            content_hash: nb_books_lib::db::catalog::ContentHash::from_sha256(&[7; 32]),
            byte_size: 10,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: nb_books_lib::db::catalog::StorageMode::Managed,
        },
    )
    .expect("archivo");

    // Otro escritor con el bloqueo puesto, que es lo que hace el indexador.
    let mut bloqueador = esc.db.connection().expect("segunda conexión");
    let tx = nb_books_lib::db::write_transaction(&mut bloqueador).expect("transacción");

    assert_eq!(
        nb_books_lib::metadata::normalizar_titulos_de_archivo(&conn),
        0,
        "con la base ocupada no presenta ninguno"
    );
    let marcas: i64 = conn
        .query_row(
            "SELECT count(*) FROM app_metadata WHERE key = 'filename_titles_normalized_v1'",
            [],
            |f| f.get(0),
        )
        .expect("marca");
    assert_eq!(marcas, 0, "sin marca: el próximo arranque lo reintenta");

    // Y cuando el otro suelta, el reintento sí entra.
    tx.rollback().expect("liberar");
    assert_eq!(
        nb_books_lib::metadata::normalizar_titulos_de_archivo(&conn),
        1
    );
    let titulo: String = conn
        .query_row(
            "SELECT title FROM editions WHERE id = ?1",
            [&edicion.id],
            |f| f.get(0),
        )
        .expect("título");
    assert_eq!(titulo, "Uno dos");
}

/// **Un nombre de archivo no es un título, y con el corpus real eran 973.**
///
/// Lo importado antes del 2026-09-12 conserva como título el nombre de su archivo
/// con los guiones bajos puestos, y eso es lo que se ve en la rejilla. La pasada
/// lo presenta como título.
///
/// Lo que de verdad hay que fijar es lo que **no** toca: un título que viene de
/// los metadatos del documento y que además lleva un guion bajo. La procedencia no
/// sirve para distinguirlos -las dos se registran como `file`-, así que la pasada
/// compara el título con el nombre del archivo, y esta prueba comprueba que esa
/// comparación hace su trabajo.
#[test]
fn un_titulo_que_es_el_nombre_del_archivo_se_presenta_como_titulo() {
    let esc = Escenario::nuevo("titulos-de-archivo");
    let conn = esc.db.connection().expect("conexión");

    let crear = |titulo: &str, archivo: &str| {
        let obra = nb_books_lib::db::catalog::Work::create(
            &conn,
            &nb_books_lib::db::catalog::NewWork {
                canonical_title: titulo.to_owned(),
                original_language: None,
            },
        )
        .expect("obra");
        let edicion = nb_books_lib::db::catalog::Edition::create(
            &conn,
            &nb_books_lib::db::catalog::NewEdition {
                work_id: obra.id,
                title: titulo.to_owned(),
                subtitle: None,
                publisher: None,
                published_date: None,
                edition_statement: None,
                language: None,
                series_title: None,
                series_index: None,
            },
        )
        .expect("edición");
        nb_books_lib::db::catalog::File::create(
            &conn,
            &nb_books_lib::db::catalog::NewFile {
                edition_id: edicion.id.clone(),
                root_id: nb_books_lib::db::catalog::MANAGED_ROOT_ID.to_owned(),
                relative_path: archivo.to_owned(),
                content_hash: nb_books_lib::db::catalog::ContentHash::from_sha256(
                    &[titulo.len() as u8; 32],
                ),
                byte_size: 1_024,
                media_type: "application/epub+zip".to_owned(),
                storage_mode: nb_books_lib::db::catalog::StorageMode::Managed,
            },
        )
        .expect("archivo");
        edicion.id
    };

    let del_archivo = crear(
        "Como_tomar_las_mejores_decisiones_en_la_amistad",
        "Como_tomar_las_mejores_decisiones_en_la_amistad.epub",
    );
    // Este viene de los metadatos: su título no es el nombre de su archivo.
    let de_metadatos = crear("Sobre_la_libertad", "9788491045_ed2.epub");

    let cambiados = nb_books_lib::metadata::normalizar_titulos_de_archivo(&conn);
    assert_eq!(cambiados, 1, "solo el que era el nombre de su archivo");

    let titulo = |id: &str| -> String {
        conn.query_row("SELECT title FROM editions WHERE id = ?1", [id], |f| {
            f.get(0)
        })
        .expect("título")
    };
    assert_eq!(
        titulo(&del_archivo),
        "Como tomar las mejores decisiones en la amistad"
    );
    assert_eq!(
        titulo(&de_metadatos),
        "Sobre_la_libertad",
        "un título de metadatos con guion bajo no se toca"
    );

    // Y no se repite: recorrer ciento cincuenta mil ediciones en cada arranque
    // costaría lo que cuesta, y no hay nada que presentar dos veces.
    assert_eq!(
        nb_books_lib::metadata::normalizar_titulos_de_archivo(&conn),
        0
    );
    let marcas: i64 = conn
        .query_row(
            "SELECT count(*) FROM app_metadata WHERE key = 'filename_titles_normalized_v1'",
            [],
            |f| f.get(0),
        )
        .expect("marca");
    assert_eq!(marcas, 1);
}

#[test]
fn las_portadas_de_la_cache_se_registran_una_sola_vez() {
    let esc = Escenario::nuevo("reconciliar");
    let conn = esc.db.connection().expect("conexión");

    let obra = nb_books_lib::db::catalog::Work::create(
        &conn,
        &nb_books_lib::db::catalog::NewWork {
            canonical_title: "Cien Años de Soledad".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = nb_books_lib::db::catalog::Edition::create(
        &conn,
        &nb_books_lib::db::catalog::NewEdition {
            work_id: obra.id,
            title: "Cien Años de Soledad".to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición")
    .id;

    // Una portada en la caché, como la habría dejado una versión anterior.
    let portadas = esc.covers.clone();
    std::fs::create_dir_all(&portadas).expect("directorio");
    std::fs::write(portadas.join(format!("{edicion}.jpg")), b"no importa").expect("portada");

    let antes: Option<String> = conn
        .query_row(
            "SELECT cover_path FROM editions WHERE id = ?1",
            [&edicion],
            |f| f.get(0),
        )
        .expect("lectura");
    assert_eq!(antes, None, "la migración deja la columna vacía");

    nb_books_lib::metadata::reconciliar_portadas(&conn, &portadas);

    let despues: Option<String> = conn
        .query_row(
            "SELECT cover_path FROM editions WHERE id = ?1",
            [&edicion],
            |f| f.get(0),
        )
        .expect("lectura");
    assert_eq!(despues.as_deref(), Some(format!("{edicion}.jpg").as_str()));

    // Y no vuelve a recorrer el directorio: recorrerlo cuesta lo que cuesta con
    // ciento cincuenta mil documentos, y no hay nada que reconciliar dos veces.
    std::fs::write(portadas.join("otra-edicion.jpg"), b"x").expect("otra");
    nb_books_lib::metadata::reconciliar_portadas(&conn, &portadas);

    let marcas: i64 = conn
        .query_row(
            "SELECT count(*) FROM app_metadata WHERE key = 'covers_reconciled_v14'",
            [],
            |f| f.get(0),
        )
        .expect("marca");
    assert_eq!(marcas, 1, "la marca se pone una vez y no se duplica");
}

/// **El criterio.** Lo que se quedó sin autor vuelve a intentarse.
///
/// La extracción de metadatos ocurre fuera de la ruta de importación: escucha el
/// bus y trabaja después. Si falla -por contención, por un archivo que en ese
/// momento no se dejaba abrir, o porque la aplicación se cerró antes de que le
/// tocara-, el documento se queda con el nombre de su archivo por título, sin
/// nadie a quien atribuirlo, y **nadie lo vuelve a intentar**: el evento ya pasó.
/// En la instalación real eso fue el 20 % del catálogo.
///
/// Aquí se reproduce importando **sin** el servicio conectado, que es
/// exactamente el estado en el que quedaron esas treinta mil ediciones: en el
/// catálogo, con su archivo disponible y sin una sola autoría.
#[test]
fn una_edicion_que_se_quedo_sin_autor_lo_recupera_en_la_pasada_de_reconciliacion() {
    let escenario = Escenario::nuevo("reconciliacion");
    let ruta = escenario.traer("minimo-valido.epub", "libro.epub");
    let importado = escenario.importar(&ruta);

    let conn = escenario.db.connection().expect("conexión");
    let autorias: i64 = conn
        .query_row(
            "SELECT count(*) FROM edition_contributors WHERE edition_id = ?1",
            [&importado.edition.id],
            |fila| fila.get(0),
        )
        .expect("recuento");
    assert_eq!(
        autorias, 0,
        "sin el servicio conectado no debería haber ninguna autoría todavía"
    );

    let recuperadas = metadata::reconciliar_metadatos(&conn, &escenario.covers);

    assert_eq!(recuperadas, 1, "la pasada no recuperó la edición sin autor");
    let despues: i64 = conn
        .query_row(
            "SELECT count(*) FROM edition_contributors WHERE edition_id = ?1 AND role = 'author'",
            [&importado.edition.id],
            |fila| fila.get(0),
        )
        .expect("recuento");
    assert!(despues > 0, "la edición sigue sin autor tras la pasada");

    // Y la segunda pasada no la vuelve a mirar: ya tiene autoría, así que sale
    // del conjunto pendiente. Sin esto, cada arranque reabriría los mismos
    // archivos para no cambiar nada.
    assert_eq!(
        metadata::reconciliar_metadatos(&conn, &escenario.covers),
        0,
        "la pasada volvió a mirar una edición que ya tenía autor"
    );
}

/// **El criterio.** Toda edición tiene fila de estado desde que entra.
///
/// Antes la escribía el indexador cuando le tocaba el turno, así que hasta
/// entonces la edición no tenía fila. Y una edición sin fila **no está
/// pendiente**: no existe para esa maquinaria, y la interfaz no puede decir "aún
/// no" porque no hay nada que decir. En la instalación real eran 130.724 de
/// 151.411, el 86 % del catálogo.
///
/// La garantía es de la base -un disparador, V25- y no de que alguien se acuerde
/// de escribirla en cada camino que crea una edición. Por eso esta prueba cuenta
/// las dos tablas en vez de comprobar una llamada.
#[test]
fn toda_edicion_tiene_fila_de_estado_de_busqueda_desde_que_entra() {
    let escenario = Escenario::nuevo("estado-de-busqueda");
    let conn = escenario.db.connection().expect("conexión");

    let contar = |tabla: &str| -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {tabla}"), [], |f| f.get(0))
            .expect("recuento")
    };

    for (fixture, como) in [
        ("minimo-valido.epub", "uno.epub"),
        ("minimo-valido.pdf", "dos.pdf"),
        ("minimo-valido.cbz", "tres.cbz"),
    ] {
        let ruta = escenario.traer(fixture, como);
        escenario.importar(&ruta);
    }

    assert_eq!(contar("editions"), 3);
    assert_eq!(
        contar("search_status"),
        contar("editions"),
        "hay ediciones sin fila de estado: la interfaz no podría decir ni «aún no»"
    );

    // Y una edición que se crea por otro camino -no la importación- tampoco se
    // queda fuera: es lo que el disparador garantiza y una línea en la
    // importación no podría.
    let obra = nb_books_lib::db::catalog::Work::create(
        &conn,
        &nb_books_lib::db::catalog::NewWork {
            canonical_title: "Por otro camino".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    nb_books_lib::db::catalog::Edition::create(
        &conn,
        &nb_books_lib::db::catalog::NewEdition {
            work_id: obra.id,
            title: "Por otro camino".to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición");

    assert_eq!(contar("editions"), 4);
    assert_eq!(contar("search_status"), contar("editions"));

    // Y la fila dice la verdad: nadie la ha mirado todavía.
    let pendientes: i64 = conn
        .query_row(
            "SELECT count(*) FROM search_status WHERE state = 'pending'",
            [],
            |f| f.get(0),
        )
        .expect("recuento");
    assert!(pendientes > 0);
}

/// **Regresión del 2026-09-18: vaciar la caché no deja la biblioteca rota.**
///
/// Se reportó tras borrar la caché del sistema: el catálogo seguía
/// apuntando el nombre de cada portada, la caché ya no las tenía, y la rejilla se
/// llenó de imágenes rotas —una por obra— sin que nada volviera a generarlas. Lo
/// derivado que no se puede rehacer no es derivado: es un dato perdido.
///
/// Aquí se reproduce entero: se importa con portada, se borra la caché como la
/// borraría un limpiador del sistema, y se comprueba que la pasada la vuelve a
/// sacar del documento.
#[test]
fn si_se_vacia_la_cache_las_portadas_se_recuperan_del_documento() {
    let esc = Escenario::con_servicio("cache-vaciada");
    let portada: Vec<u8> = (0..64 * 1024_u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let ruta = esc.escribir(
        "libro.epub",
        &epub_con(OPF_COMPLETO, Some(("cover.png", &portada))),
    );
    let importado = esc.importar(&ruta);

    // La portada aparece sola al importar, que es lo de siempre.
    esc.esperar(|| {
        let ruta = esc.covers.join(format!("{}.png", importado.edition.id));
        ruta.exists().then_some(ruta)
    })
    .expect("la portada debe aparecer al importar");

    // Y ahora se vacía la caché, como hace un limpiador del sistema.
    std::fs::remove_dir_all(&esc.covers).expect("vaciar la caché");
    assert!(!esc.covers.exists());

    let conn = esc.db.connection().expect("conexión");
    // Lo que el catálogo sigue apuntando no está en el disco: es el estado en el
    // que la rejilla enseñaba imágenes rotas.
    let apuntada: Option<String> = conn
        .query_row(
            "SELECT cover_path FROM editions WHERE id = ?1",
            [&importado.edition.id],
            |fila| fila.get(0),
        )
        .expect("consulta");
    assert!(
        apuntada.is_some(),
        "el catálogo tiene que seguir apuntándola"
    );

    let recuperadas = metadata::recuperar_portadas(
        &conn,
        &esc.covers,
        std::slice::from_ref(&importado.edition.id),
    );

    assert_eq!(recuperadas, 1, "la pasada no recuperó la portada");
    let servida = nb_books_lib::covers::para_rejilla(
        &esc.covers,
        &conn
            .query_row(
                "SELECT cover_path FROM editions WHERE id = ?1",
                [&importado.edition.id],
                |fila| fila.get::<_, Option<String>>(0),
            )
            .expect("consulta")
            .expect("apuntada"),
    );
    assert!(
        servida.is_some_and(|r| r.is_file()),
        "tras recuperarla, la rejilla tiene algo que servir"
    );
}

/// Y lo que no se puede recuperar no se queda intentándolo: un documento que no
/// está disponible no tiene de dónde sacar su portada, y eso no es un fallo.
#[test]
fn una_portada_que_no_se_puede_recuperar_no_cuenta_como_recuperada() {
    let esc = Escenario::nuevo("sin-documento");
    let conn = esc.db.connection().expect("conexión");

    assert_eq!(
        metadata::recuperar_portadas(&conn, &esc.covers, &["no-existe".to_owned()]),
        0
    );
}
