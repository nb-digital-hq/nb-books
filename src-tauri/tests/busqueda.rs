//! Criterio de aceptación de la.
//!
//! "Buscar sin tildes encuentra resultados con tildes; un resultado abre la
//! posición exacta."
//!
//! Las dos mitades importan por separado. La primera es la que decide si la
//! búsqueda sirve en español: quien escribe deprisa, o desde un teclado sin
//! acentos, es la mayoría. La segunda es la que la distingue de un buscador de
//! títulos: encontrar el libro no es encontrar el párrafo.
//!
//! Contrato: `docs/architecture/search.md`.

use std::io::Write;
use std::path::PathBuf;

use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::reader::epub::EpubDocument;
use nb_books_lib::reader::{DocumentProvider, NativePosition, Precision};
use nb_books_lib::search::{self, MARCA_FIN, MARCA_INICIO};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-busqueda-{nombre}-{}", std::process::id()));
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

const CAP1: &str = "<p>El análisis crítico de la señal comenzó una mañana de niebla. \
                    La señal era débil y el equipo llevaba semanas sin dormir.</p>";
const CAP2: &str = "<p>Macondo era entonces una aldea de veinte casas de barro y cañabrava \
                    construidas a la orilla de un río de aguas diáfanas.</p>";

fn edicion(db: &Database) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
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
    .id
}

/// Construye un EPUB con los capítulos que se le den.
fn epub(tmp: &Temporal, nombre: &str, capitulos: &[(&str, &str)]) -> PathBuf {
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
    ];

    let items: String = capitulos
        .iter()
        .enumerate()
        .map(|(i, (archivo, _))| {
            format!(r#"<item id="c{i}" href="{archivo}" media-type="application/xhtml+xml"/>"#)
        })
        .collect();
    let refs: String = (0..capitulos.len())
        .map(|i| format!(r#"<itemref idref="c{i}"/>"#))
        .collect();

    entradas.push((
        "OEBPS/content.opf".to_owned(),
        format!(
            r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
               <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Un libro</dc:title></metadata>
               <manifest>{items}</manifest><spine>{refs}</spine></package>"#
        )
        .into_bytes(),
        true,
    ));

    for (archivo, cuerpo) in capitulos {
        entradas.push((
            format!("OEBPS/{archivo}"),
            format!(
                r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>{cuerpo}</body></html>"#
            )
            .into_bytes(),
            true,
        ));
    }

    let mut salida = Vec::new();
    let mut central = Vec::new();
    let mut cuantas = 0_u16;

    for (nombre_entrada, contenido, comprimir) in &entradas {
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
        let nb = nombre_entrada.as_bytes();

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

    let ruta = tmp.0.join(nombre);
    std::fs::write(&ruta, salida).expect("epub");
    ruta
}

fn documento(tmp: &Temporal, nombre: &str) -> EpubDocument {
    let ruta = epub(tmp, nombre, &[("c1.xhtml", CAP1), ("c2.xhtml", CAP2)]);
    EpubDocument::open(&ruta).expect("epub")
}

/// Una edición **con su archivo dado de alta**, para que el pasaje se pueda
/// releer del documento.
///
/// Ahora el texto no vive en la base, así que una edición sin archivo
/// disponible no tiene pasaje que enseñar. Eso es correcto -es un volumen
/// desconectado- pero no es lo que estas pruebas quieren medir.
fn edicion_con_archivo(db: &Database, tmp: &Temporal, nombre: &str) -> (String, EpubDocument) {
    let ruta = epub(tmp, nombre, &[("c1.xhtml", CAP1), ("c2.xhtml", CAP2)]);
    let edicion = edicion(db);

    let conn = db.connection().expect("conexión");
    let carpeta = ruta
        .parent()
        .expect("carpeta")
        .to_string_lossy()
        .into_owned();
    let archivo = ruta
        .file_name()
        .expect("nombre")
        .to_string_lossy()
        .into_owned();
    conn.execute(
        "INSERT INTO library_roots (id, name, path, kind)
              VALUES ('raiz-busqueda', 'Pruebas', ?1, 'referenced')
         ON CONFLICT (id) DO UPDATE SET path = excluded.path",
        [&carpeta],
    )
    .expect("raíz");
    conn.execute(
        "INSERT INTO files (id, edition_id, root_id, relative_path, content_hash,
                            byte_size, media_type, storage_mode)
              VALUES (?1, ?2, 'raiz-busqueda', ?3, ?1, 1024,
                      'application/epub+zip', 'referenced')",
        rusqlite::params![format!("f-{edicion}"), &edicion, &archivo],
    )
    .expect("archivo");

    (edicion, EpubDocument::open(&ruta).expect("epub"))
}

/// Los lectores con los que se relee un pasaje. Solo EPUB, que es lo que estas
/// pruebas indexan.
fn registro_de_lectura() -> nb_books_lib::reader::ProviderRegistry {
    let mut registro = nb_books_lib::reader::ProviderRegistry::new();
    registro.register(Box::new(
        nb_books_lib::reader::epub::EpubFactory::solo_texto(),
    ));
    registro
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn buscar_sin_tildes_encuentra_resultados_con_tildes() {
    let tmp = Temporal::nuevo("tildes");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");

    // Quien escribe deprisa, o desde un teclado sin acentos, es la mayoría.
    for consulta in ["analisis", "senal", "manana", "canabrava", "diafanas"] {
        let resultados = search::search_corpus(&conn, consulta, 10, 0).expect("búsqueda");
        assert!(
            !resultados.is_empty(),
            "buscar {consulta:?} sin tildes no encontró nada"
        );
    }

    // Y con tildes también, claro: el índice no las quita del texto, solo de la
    // comparación.
    assert!(
        !search::search_corpus(&db.connection().expect("conexión"), "análisis", 10, 0)
            .expect("búsqueda")
            .is_empty()
    );
}

#[test]
fn un_resultado_abre_la_posicion_exacta_y_no_el_principio_del_capitulo() {
    let tmp = Temporal::nuevo("posicion");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");

    let resultados = search::search_corpus(&conn, "cañabrava", 10, 0).expect("búsqueda");
    let hit = resultados.first().expect("hay resultado");

    // El locator del resultado tiene que resolver sobre el documento, y hacerlo
    // de forma exacta: si resolviera por progresión, "abrir la posición exacta"
    // sería una forma de hablar.
    let resolucion = doc.resolve(&hit.locator).expect("resolución");
    assert_eq!(resolucion.precision, Precision::Exact);

    match resolucion.position {
        NativePosition::Fragment { href, .. } => {
            assert_eq!(
                href, "OEBPS/c2.xhtml",
                "la palabra está en el segundo capítulo, no en el primero"
            );
        }
        otra => panic!("un EPUB ancla por fragmento, no por {otra:?}"),
    }
}

// --- El fragmento con la coincidencia ---------------------------------------

/// **Lo marcado lo sigue poniendo la base, aunque el pasaje ya no salga de ella.**
///
/// Ahora el texto no se guarda, así que el pasaje se relee del documento
/// con el locator del fragmento. Lo que **no** cambia es quién decide qué se
/// marca: se le da el pasaje a una tabla con el mismo tokenizador que el índice
/// y se le pide el fragmento. Por eso esta prueba sigue buscando "senal" y
/// exigiendo que lo marcado sea "señal": un resaltado que comparase cadenas
/// dejaría el resultado encontrado y sin resaltar.
#[test]
fn el_fragmento_resaltado_lo_genera_la_base_y_no_el_frontend() {
    let tmp = Temporal::nuevo("resaltado");
    let db = Database::open_in_memory().expect("base");
    let (edicion, doc) = edicion_con_archivo(&db, &tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");

    let resultados = search::search_corpus(&conn, "senal", 10, 0).expect("búsqueda");
    let resultados = search::con_pasajes(&conn, &registro_de_lectura(), "senal", resultados);
    let hit = resultados.first().expect("hay resultado");

    // Lo decisivo: se buscó "senal" y lo marcado es "señal". Un resaltado hecho
    // en JavaScript buscaría la cadena literal y no marcaría nada, porque no
    // sabe cómo tokenizó el índice.
    assert!(hit.snippet.contains(MARCA_INICIO));
    assert!(hit.snippet.contains(MARCA_FIN));
    assert!(
        hit.snippet
            .contains(&format!("{MARCA_INICIO}señal{MARCA_FIN}"))
            || hit
                .snippet
                .contains(&format!("{MARCA_INICIO}Señal{MARCA_FIN}")),
        "lo marcado debería ser la palabra con tilde: {}",
        hit.snippet
    );
}

#[test]
fn las_marcas_del_fragmento_no_son_marcado_html() {
    // El texto viene de un documento que escribió un desconocido. Devolver
    // `<mark>` obligaría a la interfaz a interpretarlo como marcado, y por ahí
    // es por donde entra una inyección.
    assert!(!MARCA_INICIO.contains('<'));
    assert!(!MARCA_FIN.contains('<'));
}

// --- La consulta del usuario no es una consulta de FTS5 ---------------------

#[test]
fn lo_que_el_usuario_teclea_no_se_interpreta_como_operadores() {
    // Una comilla suelta haría fallar la consulta con un error de sintaxis, y
    // escribir OR cambiaría lo que se busca sin que nadie lo haya pedido.
    let consulta = search::to_fts_query("documento OR \"otra cosa\"").expect("consulta");
    assert!(
        !consulta.contains(" OR "),
        "OR tiene que viajar como término, no como operador: {consulta}"
    );
    assert!(consulta.starts_with('"'));

    // Y sobre todo, no revienta.
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    for entrada in [
        "\"",
        "* *",
        "NEAR(a b)",
        "a AND",
        "(((",
        "analisis OR NOT senal",
        "--",
    ] {
        assert!(
            search::search_corpus(&conn, entrada, 10, 0).is_ok(),
            "la entrada {entrada:?} hizo fallar la búsqueda"
        );
    }
}

#[test]
fn una_consulta_sin_terminos_no_busca_nada_en_lugar_de_buscarlo_todo() {
    assert_eq!(search::to_fts_query("   "), None);
    assert_eq!(search::to_fts_query("!!! ¿¿¿"), None);

    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    assert!(search::search_corpus(&conn, "  ", 10, 0)
        .expect("búsqueda")
        .is_empty());
}

#[test]
fn el_ultimo_termino_busca_por_prefijo_solo_si_es_lo_bastante_largo() {
    // Mientras se escribe, "analisi" tiene que encontrar "análisis".
    assert_eq!(
        search::to_fts_query("analisi"),
        Some("\"analisi\"*".to_owned())
    );

    // Pero con dos letras el prefijo recorre casi el índice entero y no dice
    // nada: "an*" encontraría media biblioteca.
    assert_eq!(search::to_fts_query("an"), Some("\"an\"".to_owned()));

    // Los términos anteriores al último son palabras completas: quien ya escribió
    // un espacio detrás de una palabra ha terminado de escribirla.
    assert_eq!(
        search::to_fts_query("la senal"),
        Some("\"la\" \"senal\"*".to_owned())
    );
}

// --- El ciclo de vida del índice --------------------------------------------

#[test]
fn reindexar_sustituye_en_lugar_de_acumular() {
    let tmp = Temporal::nuevo("reindexar");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    let primera = search::index_edition(&mut conn, &edicion, &doc).expect("indexación");
    let segunda = search::index_edition(&mut conn, &edicion, &doc).expect("reindexación");
    assert_eq!(primera, segunda);

    let fragmentos: i64 = conn
        .query_row("SELECT count(*) FROM text_chunks", [], |r| r.get(0))
        .expect("conteo");
    assert_eq!(fragmentos as usize, primera, "no se han duplicado");

    // Y el índice invertido tampoco: si los disparadores no borraran, cada
    // resultado saldría dos veces sin que la tabla de fragmentos lo delatara.
    let indexados: i64 = conn
        .query_row("SELECT count(*) FROM documents_fts", [], |r| r.get(0))
        .expect("conteo");
    assert_eq!(indexados, fragmentos);
}

#[test]
fn borrar_un_documento_lo_saca_del_indice_en_la_misma_transaccion() {
    let tmp = Temporal::nuevo("borrado");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");
    assert!(search::is_indexed(&conn, &edicion).expect("consulta"));

    // Por la cascada de la clave foránea, no por una llamada aparte: un índice
    // que sobrevive al documento devuelve resultados que no se pueden abrir.
    conn.execute("DELETE FROM editions WHERE id = ?1", [&edicion])
        .expect("baja");

    assert!(!search::is_indexed(&conn, &edicion).expect("consulta"));
    assert!(search::search_corpus(&conn, "cañabrava", 10, 0)
        .expect("búsqueda")
        .is_empty());
}

#[test]
fn un_formato_sin_texto_lo_dice_en_lugar_de_indexar_vacio() {
    let tmp = Temporal::nuevo("sin-texto");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let mut conn = db.connection().expect("conexión");

    let comic = nb_books_lib::reader::comic::ComicDocument::open(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minimo-valido.cbz"),
        &tmp.0,
    )
    .expect("cómic");

    // Un índice vacío es indistinguible de uno que nadie ha hecho todavía.
    // Decirlo es lo que permite marcar el documento como no buscable.
    assert_eq!(
        search::index_edition(&mut conn, &edicion, &comic).err(),
        Some(NbError::FormatUnsupported)
    );
}

#[test]
fn buscar_dentro_de_un_documento_no_devuelve_resultados_de_otro() {
    let tmp = Temporal::nuevo("alcance");
    let db = Database::open_in_memory().expect("base");
    let uno = edicion(&db);
    let otro = edicion(&db);

    let doc = documento(&tmp, "libro.epub");
    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &uno, &doc).expect("indexación");
    search::index_edition(&mut conn, &otro, &doc).expect("indexación");

    let en_corpus = search::search_corpus(&conn, "señal", 50, 0).expect("búsqueda");
    let en_uno = search::search_in_edition(&conn, &uno, "señal", 50).expect("búsqueda");

    assert!(en_corpus.len() > en_uno.len());
    assert!(en_uno.iter().all(|h| h.edition_id == uno));
}

#[test]
fn los_resultados_se_pueden_paginar_sin_repetirse() {
    let tmp = Temporal::nuevo("paginado");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");

    let todos = search::search_corpus(&conn, "de", 50, 0).expect("búsqueda");
    assert!(todos.len() >= 2, "hacen falta varios para paginar");

    let primera = search::search_corpus(&conn, "de", 1, 0).expect("búsqueda");
    let segunda = search::search_corpus(&conn, "de", 1, 1).expect("búsqueda");

    assert_eq!(primera.len(), 1);
    assert_eq!(segunda.len(), 1);
    assert_ne!(primera[0].locator, segunda[0].locator);
}

// --- Búsqueda por alcance ------------------------------------------

use nb_books_lib::collections::{Collection, CollectionOrigin};
use nb_books_lib::db::catalog::{
    Contributor, ContributorRole, NewContributor, NewEdition as NuevaEdicion, Subject,
};
use nb_books_lib::search::{Results, Scope};

/// Da de alta una edición con título, autor y materia.
fn catalogar(db: &Database, titulo: &str, autor: &str, materia: &str) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NuevaEdicion {
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

    let contribuidor = Contributor::ensure(&conn, autor).expect("autor");
    Contributor::link(
        &conn,
        &edicion.id,
        &contribuidor.id,
        ContributorRole::Author,
        0,
    )
    .expect("vínculo");

    let sujeto = Subject::ensure(&conn, "tag", materia).expect("materia");
    Subject::link(&conn, &edicion.id, &sujeto.id).expect("vínculo de materia");

    edicion.id
}

#[test]
fn el_catalogo_se_busca_por_titulo_por_autor_y_por_materia() {
    let db = Database::open_in_memory().expect("base");
    let uno = catalogar(
        &db,
        "Cien años de soledad",
        "Gabriel García Márquez",
        "novela",
    );
    let otro = catalogar(
        &db,
        "El otoño del patriarca",
        "Gabriel García Márquez",
        "dictadura",
    );
    catalogar(&db, "Rayuela", "Julio Cortázar", "experimental");

    let conn = db.connection().expect("conexión");
    let buscar = |q: &str| search::search(&conn, q, &Scope::Catalog, 10, 0).expect("búsqueda");

    // Por título, sin tildes, mientras se escribe.
    let Results::Catalog(por_titulo) = buscar("soledad") else {
        panic!("el catálogo devuelve documentos, no párrafos");
    };
    assert_eq!(por_titulo.len(), 1);
    assert_eq!(por_titulo[0].edition_id, uno);

    // Por autor.
    let Results::Catalog(por_autor) = buscar("garcia marquez") else {
        panic!("catálogo");
    };
    assert_eq!(por_autor.len(), 2);
    assert!(por_autor.iter().any(|h| h.edition_id == otro));

    // Por materia.
    let Results::Catalog(por_materia) = buscar("experimental") else {
        panic!("catálogo");
    };
    assert_eq!(por_materia.len(), 1);
}

/// **Buscar un autor trae lo que firma antes que lo que lo nombra, y una errata no
/// deja la lista vacía**. "cervantes" traía libros titulados *Cervantes*, y
/// "quijte" no encontraba nada en una biblioteca con diez Quijotes.
#[test]
fn el_catalogo_prefiere_al_autor_y_perdona_una_errata() {
    let db = Database::open_in_memory().expect("base");
    let sobre = catalogar(&db, "Vida de Cervantes", "Jean Canavaggio", "biografía");
    let suyo = catalogar(&db, "Novelas ejemplares", "Miguel de Cervantes", "novela");
    let quijote = catalogar(
        &db,
        "Don Quijote de la Mancha",
        "Miguel de Cervantes",
        "novela",
    );

    let conn = db.connection().expect("conexión");
    let buscar = |q: &str| match search::search(&conn, q, &Scope::Catalog, 10, 0).expect("búsqueda")
    {
        Results::Catalog(v) => v,
        Results::Text(_) => panic!("catálogo"),
    };

    let autor = buscar("cervantes");
    let posicion = |id: &str| autor.iter().position(|h| h.edition_id == id).expect("está");
    assert!(posicion(&suyo) < posicion(&sobre), "lo que firma va antes");
    assert!(autor[0].authors.contains("Cervantes"), "y dice quién firma");

    let errata = buscar("quijte");
    assert!(errata.iter().any(|h| h.edition_id == quijote));
}

#[test]
fn el_indice_de_catalogo_lo_mantiene_la_base_y_no_hay_que_acordarse() {
    let db = Database::open_in_memory().expect("base");
    let edicion = catalogar(&db, "Un título cualquiera", "Alguien", "prueba");
    let conn = db.connection().expect("conexión");

    let cuantos = |q: &str| {
        let Results::Catalog(r) =
            search::search(&conn, q, &Scope::Catalog, 10, 0).expect("búsqueda")
        else {
            panic!("catálogo");
        };
        r.len()
    };

    assert_eq!(cuantos("cualquiera"), 1);

    // Cambiar el título actualiza el índice sin que nadie lo pida. Un índice que
    // el código olvida actualizar produce un libro que existe y no aparece al
    // buscarlo, y ese olvido no lo detecta ninguna prueba de la ruta feliz.
    conn.execute(
        "UPDATE editions SET title = 'Un título distinto' WHERE id = ?1",
        [&edicion],
    )
    .expect("cambio de título");
    assert_eq!(cuantos("cualquiera"), 0);
    assert_eq!(cuantos("distinto"), 1);

    // Y darla de baja lo retira.
    conn.execute("DELETE FROM editions WHERE id = ?1", [&edicion])
        .expect("baja");
    assert_eq!(cuantos("distinto"), 0);
}

#[test]
fn anadir_un_autor_a_posteriori_lo_hace_buscable() {
    let db = Database::open_in_memory().expect("base");
    let edicion = catalogar(&db, "Sin autor todavía", "Provisional", "prueba");
    let conn = db.connection().expect("conexión");

    let contribuidor = Contributor::create(
        &conn,
        &NewContributor {
            name: "Ursula K. Le Guin".to_owned(),
            sort_name: None,
            orcid: None,
        },
    )
    .expect("autor");
    Contributor::link(
        &conn,
        &edicion,
        &contribuidor.id,
        ContributorRole::Author,
        1,
    )
    .expect("vínculo");

    // Es el caso real: el autor llega del servicio de metadatos, en segundo
    // plano, después de importar. Si el índice no se rehiciera, el libro no
    // aparecería nunca al buscar por su autor.
    let Results::Catalog(resultados) =
        search::search(&conn, "guin", &Scope::Catalog, 10, 0).expect("búsqueda")
    else {
        panic!("catálogo");
    };
    assert_eq!(resultados.len(), 1);
}

#[test]
fn acotar_por_coleccion_se_hace_en_sql_y_no_trayendose_la_coleccion_entera() {
    let tmp = Temporal::nuevo("coleccion");
    let db = Database::open_in_memory().expect("base");
    let dentro = catalogar(&db, "Libro de la colección", "Alguien", "prueba");
    let fuera = catalogar(&db, "Libro de fuera", "Alguien", "prueba");

    let doc = documento(&tmp, "libro.epub");
    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &dentro, &doc).expect("indexación");
    search::index_edition(&mut conn, &fuera, &doc).expect("indexación");

    let coleccion = Collection::create_manual(&conn, "Del trabajo", CollectionOrigin::Personal)
        .expect("colección");
    Collection::add(&conn, &coleccion.id, &dentro).expect("miembro");

    let acotada = search::search(
        &conn,
        "señal",
        &Scope::Corpus {
            collection_id: Some(coleccion.id.clone()),
        },
        50,
        0,
    )
    .expect("búsqueda");
    let Results::Text(acotada) = acotada else {
        panic!("el corpus devuelve párrafos");
    };

    assert!(!acotada.is_empty());
    // Traerse la colección entera al frontend para cruzarla allí es lo que los
    // presupuestos prohíben: una colección de veinte mil libros viajaría por el
    // puente en cada pulsación.
    assert!(acotada.iter().all(|h| h.edition_id == dentro));

    let todo = search::search(
        &conn,
        "señal",
        &Scope::Corpus {
            collection_id: None,
        },
        50,
        0,
    )
    .expect("búsqueda");
    assert!(todo.len() > acotada.len());
}

#[test]
fn buscar_en_el_catalogo_de_mil_libros_cabe_en_el_presupuesto() {
    let db = Database::open_in_memory().expect("base");
    for n in 0..1_000 {
        catalogar(
            &db,
            &format!("Título número {n} sobre análisis crítico"),
            &format!("Autora Número {n}"),
            "ensayo",
        );
    }

    let conn = db.connection().expect("conexión");
    // Se calienta: la primera consulta prepara la sentencia y toca el índice por
    // primera vez, y medir eso mediría el arranque, no la búsqueda.
    let _ = search::search(&conn, "analisis", &Scope::Catalog, 20, 0).expect("búsqueda");

    let comienzo = std::time::Instant::now();
    for _ in 0..20 {
        let resultados =
            search::search(&conn, "analisis", &Scope::Catalog, 20, 0).expect("búsqueda");
        assert!(!resultados.is_empty());
    }
    let por_consulta = comienzo.elapsed() / 20;

    println!("catálogo de 1000 libros: {por_consulta:?} por consulta");
    // El presupuesto de `performance-budgets.md` es 50 ms sobre mil libros
    // mientras se escribe. Se compara con el doble porque esto corre en
    // compilación de depuración; el margen real se mide en release.
    assert!(
        por_consulta < std::time::Duration::from_millis(100),
        "una consulta de catálogo tardó {por_consulta:?}"
    );
}

#[test]
fn los_tres_alcances_devuelven_cosas_distintas_a_proposito() {
    let tmp = Temporal::nuevo("alcances");
    let db = Database::open_in_memory().expect("base");
    let edicion = catalogar(&db, "La señal", "Alguien", "prueba");

    let doc = documento(&tmp, "libro.epub");
    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");

    // El catálogo devuelve **el libro**; el corpus, los párrafos. Meter el
    // título en el mismo índice que el texto haría que buscar un libro
    // devolviera doscientos párrafos antes que el libro.
    let catalogo = search::search(&conn, "señal", &Scope::Catalog, 10, 0).expect("búsqueda");
    assert!(matches!(catalogo, Results::Catalog(_)));
    assert_eq!(catalogo.len(), 1);

    let corpus = search::search(
        &conn,
        "señal",
        &Scope::Corpus {
            collection_id: None,
        },
        10,
        0,
    )
    .expect("búsqueda");
    assert!(matches!(corpus, Results::Text(_)));
    assert!(!corpus.is_empty());

    let documento_abierto = search::search(
        &conn,
        "señal",
        &Scope::Document {
            edition_id: edicion.clone(),
        },
        10,
        0,
    )
    .expect("búsqueda");
    assert!(matches!(documento_abierto, Results::Text(_)));
}

// --- Reconstrucción del índice -------------------------------------

use nb_books_lib::search::{IndexHealth, Repair};

/// Llena el índice con bastante texto como para que tenga varios segmentos.
///
/// Hace falta: sobre un índice de dos o tres bloques, `integrity-check` no llega
/// a visitar los que se corrompen y responde que todo está bien. Con un índice
/// del tamaño de uno real, lo detecta. Es una limitación de la comprobación, no
/// de la prueba, y por eso la prueba se asegura de que la corrupción se detecta
/// antes de dar por buena la reparación.
fn indexar_bastante(db: &Database, doc: &EpubDocument) -> Vec<String> {
    let mut ediciones = Vec::new();
    for _ in 0..25 {
        let id = edicion(db);
        let mut conn = db.connection().expect("conexión");
        search::index_edition(&mut conn, &id, doc).expect("indexación");
        ediciones.push(id);
    }
    ediciones
}

/// Estropea las estructuras internas del índice invertido.
///
/// Se escribe directamente en la tabla sombra de FTS5, que es donde vive el
/// índice. Es lo más parecido a lo que produce un corte de corriente a mitad de
/// una escritura o un sistema de archivos que devuelve bloques de otro archivo.
fn corromper(conn: &rusqlite::Connection) {
    let filas = conn
        .execute(
            "UPDATE documents_fts_data SET block = zeroblob(64) WHERE id > 1",
            [],
        )
        .expect("corrupción");
    assert!(filas > 0, "no había nada que corromper");
}

/// **El criterio de aceptación.**
#[test]
fn corromper_el_indice_y_arrancar_produce_una_reconstruccion_y_no_un_fallo() {
    let tmp = Temporal::nuevo("corrupcion");
    let db = Database::open_in_memory().expect("base");
    let doc = documento(&tmp, "libro.epub");
    indexar_bastante(&db, &doc);

    let conn = db.connection().expect("conexión");
    assert!(!search::search_corpus(&conn, "cañabrava", 10, 0)
        .expect("búsqueda")
        .is_empty());

    corromper(&conn);

    // Esto es lo que hace el arranque.
    let reparacion = search::repair_index(&conn).expect("reparación");

    // No falla: repara. Un índice corrupto no puede impedir abrir la biblioteca,
    // porque es caché y se rehace.
    assert_ne!(reparacion, Repair::NotNeeded);
    assert_eq!(
        search::verify_index(&conn).expect("comprobación"),
        IndexHealth::Healthy
    );

    // Y lo que cuesta ahora, que es la otra mitad de la verdad: reparar deja el
    // índice vacío, porque no hay texto guardado del que rehacerlo.
    // Lo que se pierde es tiempo de indexación, no documentos.
    assert!(
        search::search_corpus(&conn, "cañabrava", 10, 0)
            .expect("búsqueda")
            .is_empty(),
        "tras reparar sin poder rehacer, el índice tiene que quedar vacío"
    );

    // Volver a indexar lo devuelve, que es lo que hace la indexación de fondo.
    let mut conn = db.connection().expect("conexión");
    let id = edicion(&db);
    search::index_edition(&mut conn, &id, &doc).expect("reindexación");
    assert!(
        !search::search_corpus(&conn, "cañabrava", 10, 0)
            .expect("búsqueda")
            .is_empty(),
        "reindexar tiene que devolver lo que encontraba antes"
    );
}

#[test]
fn reparar_un_indice_sano_no_toca_nada() {
    let tmp = Temporal::nuevo("sano");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");

    // Rehacer por si acaso en cada arranque tiraría minutos de CPU a diario por
    // un problema que casi nunca ocurre.
    assert_eq!(
        search::repair_index(&conn).expect("reparación"),
        Repair::NotNeeded
    );
}

/// **Por qué la comprobación no puede ser solo la que trae FTS5.**
///
/// Esta prueba no comprueba una función del proyecto: comprueba el motivo por el
/// que existe la sonda. Sobre un índice sin contenido, la comprobación de FTS5
/// pasa en verde mientras la búsqueda ha dejado de encontrar nada, y sin dar
/// error. Si algún día dejara de ser cierto -otra versión de SQLite, otro
/// formato de índice- esta prueba fallaría, y entonces habría que volver a
/// discutir si la sonda sigue haciendo falta. Por eso afirma lo que afirma, en
/// lugar de darlo por sabido en un comentario.
#[test]
fn sin_las_sondas_la_comprobacion_de_fts5_daria_por_bueno_un_indice_vaciado() {
    let tmp = Temporal::nuevo("sondas-hacen-falta");
    let db = Database::open_in_memory().expect("base");
    let doc = documento(&tmp, "libro.epub");
    indexar_bastante(&db, &doc);

    let conn = db.connection().expect("conexión");
    corromper(&conn);

    let fts5_lo_da_por_bueno = conn
        .execute_batch("INSERT INTO documents_fts(documents_fts) VALUES('integrity-check')")
        .is_ok();
    assert!(
        fts5_lo_da_por_bueno,
        "si FTS5 detectara esto por su cuenta, la sonda sobraría"
    );

    assert!(
        search::search_corpus(&conn, "cañabrava", 10, 0)
            .expect("la búsqueda no da error: ese es el problema")
            .is_empty(),
        "el índice ha perdido lo que tenía, en silencio"
    );

    assert_eq!(
        search::verify_index(&conn).expect("comprobación"),
        IndexHealth::Corrupt,
        "la sonda es lo que lo ve"
    );
}

/// **Regresión.** Un índice sano de texto en español no se declara corrupto.
///
/// La primera versión de la sonda recorría el texto por caracteres ASCII, así
/// que partía "cañabrava" por la eñe y guardaba "abrava", un trozo que el
/// tokenizador no produce nunca. La sonda no respondía, la comprobación decía
/// corrupto y la reparación habría borrado un índice intacto para releer ciento
/// cincuenta mil libros. Un falso positivo aquí cuesta más que no comprobar.
#[test]
fn un_texto_con_enes_y_tildes_no_hace_que_el_indice_parezca_roto() {
    let tmp = Temporal::nuevo("acentos");
    let db = Database::open_in_memory().expect("base");
    let id = edicion(&db);
    let ruta = epub(
        &tmp,
        "acentos.epub",
        &[(
            "c1.xhtml",
            "<p>Cañabrava, señal, análisis, mañana, niebla; ñandú.</p>",
        )],
    );
    let doc = EpubDocument::open(&ruta).expect("epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &id, &doc).expect("indexación");

    assert_eq!(
        search::verify_index(&conn).expect("comprobación"),
        IndexHealth::Healthy
    );
    assert_eq!(
        search::repair_index(&conn).expect("reparación"),
        Repair::NotNeeded,
        "nada que reparar: el índice está entero"
    );
}

/// Retirar una edición se lleva su sonda.
///
/// Una sonda que apunta a un fragmento que ya no existe señalaría una corrupción
/// que no ha ocurrido, que es la otra forma de romper esto.
#[test]
fn retirar_una_edicion_no_deja_una_sonda_apuntando_al_vacio() {
    let tmp = Temporal::nuevo("olvido");
    let db = Database::open_in_memory().expect("base");
    let id = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &id, &doc).expect("indexación");
    let sondas: i64 = conn
        .query_row("SELECT count(*) FROM index_probes", [], |f| f.get(0))
        .expect("recuento");
    assert_eq!(sondas, 1, "indexar deja sonda");

    search::forget_edition(&conn, &id).expect("olvido");
    let sondas: i64 = conn
        .query_row("SELECT count(*) FROM index_probes", [], |f| f.get(0))
        .expect("recuento");
    assert_eq!(sondas, 0, "y olvidar se la lleva");

    assert_eq!(
        search::verify_index(&conn).expect("comprobación"),
        IndexHealth::Healthy
    );
}

/// **La reparación barata ya no existe, y esta prueba lo fija.**
///
/// Antes el índice invertido se rehacía desde los fragmentos guardados,
/// en segundos y sin abrir un documento. Eso era posible porque los fragmentos
/// llevaban el texto dentro, y ese texto pesaba el 52 % de lo que ocupan los
/// documentos: es justo lo que hubo que quitar para caber en el presupuesto de
/// disco.
///
/// Se comprueba **que no se puede**, y no se borra la prueba, porque es una
/// capacidad que el proyecto tuvo y perdió a cambio de algo. Si algún día vuelve
/// -guardando el texto comprimido, por ejemplo- esto fallará y hará mirar aquí.
#[test]
fn rehacer_el_indice_sin_abrir_los_documentos_ya_no_se_puede() {
    let tmp = Temporal::nuevo("sin-texto");
    let db = Database::open_in_memory().expect("base");
    let doc = documento(&tmp, "libro.epub");
    indexar_bastante(&db, &doc);

    let conn = db.connection().expect("conexión");

    assert!(
        search::rebuild_text_index(&conn).is_err(),
        "un índice sin contenido no se puede rehacer desde la base: si esto pasa, \
         es que el texto volvió a guardarse y hay que volver a medir el disco"
    );

    // Y los fragmentos siguen ahí con su locator, que es lo que abre la posición
    // exacta. Lo que se perdió es el texto, no el sitio.
    let fragmentos: i64 = conn
        .query_row("SELECT count(*) FROM text_chunks", [], |r| r.get(0))
        .expect("conteo");
    assert!(fragmentos > 0);
}

#[test]
fn vaciar_el_indice_no_toca_nada_del_usuario() {
    let tmp = Temporal::nuevo("vaciado");
    let db = Database::open_in_memory().expect("base");
    let edicion = edicion(&db);
    let doc = documento(&tmp, "libro.epub");

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &edicion, &doc).expect("indexación");
    search::set_status(&conn, &edicion, &search::SearchStatus::Searchable).expect("estado");

    search::reset(&conn).expect("vaciado");

    // El índice se va entero...
    assert!(!search::is_indexed(&conn, &edicion).expect("consulta"));
    assert_eq!(
        search::status_of(&conn, &edicion).expect("estado"),
        search::SearchStatus::Pending,
        "sin índice, el estado vuelve a ser pendiente, no no-buscable"
    );

    // ...y la biblioteca no se entera. Por eso rehacerlo no pide confirmación
    // destructiva: no hay nada que perder.
    let ediciones: i64 = conn
        .query_row("SELECT count(*) FROM editions", [], |r| r.get(0))
        .expect("conteo");
    assert_eq!(ediciones, 1);
}

#[test]
fn el_indice_de_catalogo_tambien_se_rehace() {
    let db = Database::open_in_memory().expect("base");
    catalogar(
        &db,
        "Cien años de soledad",
        "Gabriel García Márquez",
        "novela",
    );
    let conn = db.connection().expect("conexión");

    // Se vacía a mano, como si se hubiera perdido.
    conn.execute("DELETE FROM catalog_fts", [])
        .expect("vaciado");
    let Results::Catalog(vacio) =
        search::search(&conn, "soledad", &Scope::Catalog, 10, 0).expect("búsqueda")
    else {
        panic!("catálogo");
    };
    assert!(vacio.is_empty());

    search::rebuild_catalog(&conn).expect("reconstrucción");

    // `catalog_fts` no es de contenido externo: lo escriben los disparadores, así
    // que FTS5 no sabe de dónde sacarlo y hay que repetir su consulta.
    let Results::Catalog(rehecho) =
        search::search(&conn, "soledad", &Scope::Catalog, 10, 0).expect("búsqueda")
    else {
        panic!("catálogo");
    };
    assert_eq!(rehecho.len(), 1);
}

/// **Regresión.** Los tres alcances cruzan el puente.
///
/// `Scope::Document` era una variante de un solo valor dentro de una enumeración
/// etiquetada por dentro, y eso **no se puede serializar**: buscar dentro del
/// documento abierto habría fallado siempre, y el fallo no aparecía al compilar
/// sino al usarlo. No se notó porque no había interfaz que lo pidiera.
#[test]
fn los_tres_alcances_cruzan_el_puente() {
    for alcance in [
        Scope::Catalog,
        Scope::Corpus {
            collection_id: None,
        },
        Scope::Document {
            edition_id: "abc".to_owned(),
        },
    ] {
        let json = serde_json::to_string(&alcance)
            .unwrap_or_else(|e| panic!("{alcance:?} no serializa: {e}"));

        // Y vuelve igual: un alcance que va y no vuelve deja la búsqueda del
        // documento abierto respondiendo sobre el corpus entero.
        let vuelta: Scope = serde_json::from_str(&json).expect("deserializa");
        assert_eq!(vuelta, alcance);
    }
}

/// **El criterio**: comprobar el índice deja de costar lo que ocupa.
///
/// El `integrity-check` de FTS5 recorre el índice entero, y sobre la biblioteca
/// real eso eran **5 minutos y 24 segundos** con el 13,7 % indexado, en cada
/// sesión, para comprobar casi siempre que no había pasado nada. Un índice
/// invertido no se corrompe solo: se corrompe cuando el proceso muere a media
/// escritura. Eso deja rastro, y el rastro es lo que se mira.
#[test]
fn el_indice_solo_se_comprueba_si_la_sesion_anterior_no_cerro_bien() {
    use nb_books_lib::search::CierreAnterior;

    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    // Una base recién creada no tiene marca ni índice: no hay nada que comprobar,
    // y decir que "la sesión anterior no cerró bien" sería falso.
    assert_eq!(search::cierre_anterior(&conn), CierreAnterior::PrimeraVez);
    assert!(
        search::repair_index_si_hace_falta(&conn, CierreAnterior::PrimeraVez)
            .expect("comprobación")
            .is_none(),
        "una base sin índice no tiene nada que comprobar"
    );

    // Una base con índice y sin marca viene de una versión que no la escribía:
    // ante la duda, se comprueba.
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
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
    conn.execute(
        "INSERT INTO text_chunks (edition_id, locator) VALUES (?1, '{\"version\":1,\"href\":\"c.xhtml\",\"mediaType\":\"application/xhtml+xml\",\"locations\":{}}')",
        [&edicion.id],
    )
    .expect("fragmento");
    assert_eq!(search::cierre_anterior(&conn), CierreAnterior::SinMarca);
    assert!(
        search::repair_index_si_hace_falta(&conn, CierreAnterior::SinMarca)
            .expect("comprobación")
            .is_some(),
        "sin saber cómo cerró la sesión anterior hay que comprobar"
    );

    search::marcar_cierre(&conn, true);
    assert_eq!(search::cierre_anterior(&conn), CierreAnterior::Limpio);
    assert!(
        search::repair_index_si_hace_falta(&conn, CierreAnterior::Limpio)
            .expect("comprobación")
            .is_none(),
        "tras un cierre limpio no hay nada que comprobar"
    );

    // Y una sesión que empieza vuelve a marcarla sucia, así que si muere sin
    // firmar la salida, la siguiente comprueba.
    search::marcar_cierre(&conn, false);
    assert_eq!(search::cierre_anterior(&conn), CierreAnterior::Sucio);
    assert!(
        search::repair_index_si_hace_falta(&conn, CierreAnterior::Sucio)
            .expect("comprobación")
            .is_some(),
        "una sesión que no firmó su salida obliga a comprobar"
    );
}

/// **El criterio**: el presupuesto de disco frena al indexador.
///
/// Hasta aquí el presupuesto era un número escrito en un documento, y el
/// innegociable "lo derivado nunca pesa más que la biblioteca que describe" no
/// tenía nada que lo hiciera cumplir. Un número sin mecanismo se incumple sin que
/// nadie se entere, que es exactamente lo que pasó: 24 GB con el 13,7 % indexado.
///
/// Se comprueba con la medida, que es lo que el indexador consulta: con una
/// biblioteca cuyos documentos pesan poco y una caché que ya pesa más que el
/// presupuesto, la medida tiene que decir que se pasó.
#[test]
fn el_presupuesto_de_disco_se_nota_en_la_medida_que_consulta_el_indexador() {
    let temporal = Temporal::nuevo("presupuesto");
    // SAFETY: esta prueba usa su propio directorio y lo borra al terminar.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, temporal.0.clone()) };
    let rutas = nb_books_lib::paths::AppPaths::resolve().expect("rutas");

    let db = Database::open(&rutas.database()).expect("base");
    let conn = db.connection().expect("conexión");

    // Sin documentos no hay presupuesto que gastar: medir contra cero no
    // significa nada, y el indexador no debe frenarse por eso.
    let vacia = nb_books_lib::diagnostics::medir_almacenamiento(&conn, &rutas).expect("medida");
    assert_eq!(vacia.documents_bytes, 0);
    assert_eq!(vacia.percent_of_documents, 0.0);

    // Un documento que pesa poco y una caché que pesa mucho: lo derivado se
    // pasa del presupuesto, que es la situación que el freno existe para ver.
    conn.execute_batch(
        "INSERT INTO works (id, canonical_title) VALUES ('w1', 'Obra');
         INSERT INTO editions (id, work_id, title) VALUES ('e1', 'w1', 'Obra');
         INSERT INTO library_roots (id, name, path, kind)
              VALUES ('r1', 'Disco', '/tmp/nb-presupuesto', 'referenced');
         INSERT INTO files (id, edition_id, root_id, relative_path, content_hash,
                            byte_size, media_type, storage_mode)
              VALUES ('f1', 'e1', 'r1', 'obra.epub', 'abc', 1024,
                      'application/epub+zip', 'referenced');",
    )
    .expect("catálogo");

    std::fs::create_dir_all(&rutas.cache).expect("caché");
    std::fs::write(rutas.cache.join("relleno.bin"), vec![0_u8; 64 * 1024]).expect("relleno");

    let pasada = nb_books_lib::diagnostics::medir_almacenamiento(&conn, &rutas).expect("medida");
    assert_eq!(pasada.documents_bytes, 1024);
    assert!(
        pasada.percent_of_documents > pasada.budget_percent,
        "lo derivado ocupa {:.0} % de los documentos y el presupuesto es {:.0} %: \
         el indexador tendría que frenarse",
        pasada.percent_of_documents,
        pasada.budget_percent
    );
}

/// La forma del índice está escrita en dos sitios, y tienen que decir lo mismo.
///
/// La migración lo crea la primera vez; `search::reset` lo recrea cuando está
/// corrupto, porque un índice roto no se deja vaciar. Si las dos definiciones se
/// separaran, un índice reparado tendría otra forma que uno recién migrado -otro
/// tokenizador, o con contenido- y la diferencia solo se notaría en la máquina de
/// alguien, buscando algo que no aparece.
#[test]
fn la_migracion_y_la_reparacion_crean_el_mismo_indice() {
    let migracion = include_str!("../migrations/V27__indice_sin_texto.sql");

    assert!(
        migracion.contains(search::CREAR_INDICE),
        "la migración V27 ya no crea el índice como lo recrea `reset`"
    );
}

/// **Regresión, y la que más le importa a quien usa la aplicación.**
///
/// Antes comprobar el índice lanzaba `integrity-check` también
/// sobre `documents_fts`. La documentación decía que se conservaba "porque lo
/// que ve lo ve gratis", y medirlo desmintió la frase entera:
///
/// * **No es gratis.** Sobre la base real -9,9 GB y 6.542.712 fragmentos- cuesta
///   **4 minutos y 12 segundos**, frente a los 2 segundos del índice de
///   catálogo.
/// * **No es inofensivo.** `integrity-check` es una escritura sobre la tabla
///   virtual, así que retiene el bloqueo de escritura los cuatro minutos. En el
///   registro del usuario, cada escritura de la aplicación fallaba con
///   `database is locked` exactamente cada 5 segundos -su `busy_timeout`- y el
///   pool de conexiones se agotaba detrás: no se podía guardar el progreso de
///   lectura, ni una preferencia, ni unos metadatos.
/// * **Y lo que veía no compensaba**, que es lo que demuestra
///   `sin_las_sondas_la_comprobacion_de_fts5_daria_por_bueno_un_indice_vaciado`:
///   sobre una tabla sin contenido da por bueno un índice puesto a ceros. Lo que
///   detecta la pérdida son las sondas, y cuestan milisegundos.
///
/// El registro del 2026-09-12 tenía veinte arranques, nueve comprobaciones y
/// **cero cierres limpios**: cada arranque sucio lanzaba los cuatro minutos, la
/// aplicación no admitía una sola escritura mientras tanto, se la mataba antes
/// de que terminara -ninguna de las nueve registró su resultado- y el arranque
/// siguiente volvía a estar sucio.
///
/// Se comprueba con el rastro de SQLite y no leyendo el código, porque una
/// prueba de tiempo mediría la máquina y sobre una base de pruebas los cuatro
/// minutos son microsegundos: el defecto no se distingue por el reloj, se
/// distingue por **qué sentencia se ejecuta**.
#[test]
fn comprobar_el_indice_no_lanza_la_revision_estructural_del_texto() {
    // `Connection::trace` recibe un puntero a función y no un cierre, así que el
    // recolector es propio del hilo. Sirve igual: SQLite llama al rastro en el
    // mismo hilo que ejecuta la sentencia, y así dos pruebas en paralelo no se
    // mezclan las suyas.
    thread_local! {
        static EJECUTADAS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    fn apuntar(evento: rusqlite::trace::TraceEvent<'_>) {
        if let rusqlite::trace::TraceEvent::Stmt(_, sql) = evento {
            EJECUTADAS.with(|v| v.borrow_mut().push(sql.to_owned()));
        }
    }

    let tmp = Temporal::nuevo("sin-revision-cara");
    let db = Database::open_in_memory().expect("base");
    let doc = documento(&tmp, "libro.epub");
    indexar_bastante(&db, &doc);

    let mut conn = db.connection().expect("conexión");
    EJECUTADAS.with(|v| v.borrow_mut().clear());
    conn.trace_v2(
        rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
        Some(apuntar),
    )
    .expect("traza");

    assert_eq!(
        search::verify_index(&conn).expect("comprobación"),
        IndexHealth::Healthy
    );

    conn.trace_v2(
        rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
        None::<fn(rusqlite::trace::TraceEvent<'_>)>,
    )
    .expect("sin traza");
    let ejecutadas = EJECUTADAS.with(|v| v.borrow().clone());

    assert!(
        !ejecutadas
            .iter()
            .any(|sql| sql.contains("documents_fts(documents_fts)")),
        "volvió el integrity-check de cuatro minutos sobre el índice del texto"
    );

    // Y la otra mitad: lo que sí tiene que seguir haciendo. Sin esto, la prueba
    // pasaría igual con un `verify_index` que no comprobara nada en absoluto,
    // que es un defecto peor que el que vigila.
    assert!(
        ejecutadas
            .iter()
            .any(|sql| sql.contains("catalog_fts(catalog_fts)")),
        "dejó de comprobar el índice del catálogo, que es el que sí puede compararse"
    );
    assert!(
        ejecutadas.iter().any(|sql| sql.contains("index_probes")),
        "dejó de leer las sondas, que son lo único que detecta la pérdida silenciosa"
    );
}

/// Da de alta un archivo disponible para una edición y devuelve su identificador.
///
/// Sin archivo, un resultado de búsqueda no tiene con qué abrirse, que es
/// justamente lo que comprueban las dos pruebas de abajo.
fn archivo_disponible(db: &Database, edition_id: &str, ruta: &std::path::Path) -> String {
    use nb_books_lib::db::catalog::{ContentHash, File, NewFile, StorageMode};

    let conn = db.connection().expect("conexión");
    let (root_id, relative_path) =
        nb_books_lib::library::ensure_root(&conn, ruta, StorageMode::Referenced, None)
            .expect("raíz");

    File::create(
        &conn,
        &NewFile {
            edition_id: edition_id.to_owned(),
            root_id,
            relative_path,
            content_hash: ContentHash::parse(&format!("sha256:{:064x}", 1_u128)).expect("hash"),
            byte_size: 1,
            media_type: "application/epub+zip".to_owned(),
            storage_mode: StorageMode::Referenced,
        },
    )
    .expect("archivo")
    .id
}

/// **Regresión.** Un resultado de búsqueda trae con qué abrirlo.
///
/// Sin esto, pulsar un resultado no podía hacer nada, y no lo hacía: el
/// manejador de la interfaz recibía la edición, la descartaba con un `void` y
/// se limitaba a cambiar de pantalla. Ir a la biblioteca sin llevar el libro es,
/// para quien pulsa, lo mismo que no hacer nada.
///
/// Es el mismo dato que ya viajaba con una anotación, con una palabra del
/// vocabulario y con una recomendación, y por el mismo motivo: quien enseña algo
/// que se puede abrir tiene que traer con qué abrirlo.
#[test]
fn un_resultado_de_busqueda_trae_el_archivo_con_el_que_se_abre() {
    let tmp = Temporal::nuevo("resultado-abrible");
    let db = Database::open_in_memory().expect("base");
    let id = edicion(&db);
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CAP2)]);
    let doc = EpubDocument::open(&ruta).expect("epub");

    let archivo = archivo_disponible(&db, &id, &ruta);

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &id, &doc).expect("indexación");

    let texto = search::search_corpus(&conn, "cañabrava", 10, 0).expect("búsqueda");
    assert_eq!(texto.len(), 1, "el fragmento tiene que encontrarse");
    assert_eq!(
        texto[0].file_id.as_deref(),
        Some(archivo.as_str()),
        "el resultado de texto llegó sin archivo, así que no se puede abrir"
    );

    let catalogo = search::search(&conn, "Edición", &search::Scope::Catalog, 10, 0)
        .expect("búsqueda de catálogo");
    let search::Results::Catalog(hits) = catalogo else {
        panic!("el alcance de catálogo tiene que devolver documentos");
    };
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0].file_id.as_deref(),
        Some(archivo.as_str()),
        "el resultado de catálogo llegó sin archivo, así que no se puede abrir"
    );
}

/// La otra mitad: sin copia disponible, el resultado **sigue apareciendo** y
/// dice que no hay con qué abrirlo.
///
/// Las dos mitades hacen falta. Con solo la de arriba, un `file_id` que
/// devolviera siempre algo pasaría igual, y la interfaz ofrecería abrir un
/// archivo que no está: el volumen desconectado es el caso cotidiano, no el
/// raro.
#[test]
fn un_resultado_sin_copia_disponible_aparece_y_dice_que_no_hay_con_que_abrirlo() {
    let tmp = Temporal::nuevo("resultado-sin-copia");
    let db = Database::open_in_memory().expect("base");
    let id = edicion(&db);
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", CAP2)]);
    let doc = EpubDocument::open(&ruta).expect("epub");

    let archivo = archivo_disponible(&db, &id, &ruta);

    let mut conn = db.connection().expect("conexión");
    search::index_edition(&mut conn, &id, &doc).expect("indexación");
    conn.execute(
        "UPDATE files SET availability = 'missing' WHERE id = ?1",
        [&archivo],
    )
    .expect("desconectar el volumen");

    let texto = search::search_corpus(&conn, "cañabrava", 10, 0).expect("búsqueda");
    assert_eq!(
        texto.len(),
        1,
        "el resultado tiene que seguir apareciendo: la obra coincide"
    );
    assert!(
        texto[0].file_id.is_none(),
        "sin copia disponible no puede ofrecerse un salto que va a fallar"
    );
}

/// **La ficha enseña lo que la base sabe**: editorial, idioma y materias
/// viajan con la edición.
#[test]
fn la_ficha_trae_lo_que_la_base_sabe_de_la_obra() {
    let db = Database::open_in_memory().expect("base");
    let id = catalogar(&db, "Rayuela", "Julio Cortázar", "experimental");
    let conn = db.connection().expect("conexión");
    conn.execute(
        "UPDATE editions SET publisher = 'Sudamericana', language = 'es' WHERE id = ?1",
        [&id],
    )
    .expect("datos");
    let ficha = nb_books_lib::browse::details(&conn, &id).expect("ficha");
    assert_eq!(ficha.publisher.as_deref(), Some("Sudamericana"));
    assert_eq!(ficha.language.as_deref(), Some("es"));
    assert_eq!(ficha.subjects, vec!["experimental".to_owned()]);
    assert_eq!(
        nb_books_lib::browse::details(&conn, "no-existe"),
        Err(nb_books_lib::error::NbError::DocumentNotFound)
    );
}

/// Con la biblioteca grande, la búsqueda en todos los libros da **una
/// coincidencia por obra** y se puede paginar sin repetir ninguna.
///
/// Es el camino que toma la aplicación por encima de
/// `search::UMBRAL_SIN_PUNTUAR` fragmentos, donde puntuar con `bm25` no termina:
/// con el índice real, "amor" no había contestado a los dos minutos. Aquí se
/// llama directamente porque una prueba no puede indexar trescientos mil
/// fragmentos; lo que se comprueba es lo que promete, no el umbral.
#[test]
fn con_la_biblioteca_grande_cada_resultado_es_una_obra_distinta() {
    let tmp = Temporal::nuevo("por-obra");
    let db = Database::open_in_memory().expect("base");
    let doc = documento(&tmp, "libro.epub");
    let mut conn = db.connection().expect("conexión");
    let obras: Vec<String> = (0..5).map(|_| edicion(&db)).collect();
    for obra in &obras {
        search::index_edition(&mut conn, obra, &doc).expect("indexación");
    }

    // El mismo documento cinco veces: sin saltar de obra, las primeras
    // coincidencias serían todas de la primera.
    let todas = search::search_corpus(&conn, "de", 50, 0).expect("búsqueda");
    assert!(
        todas.len() > obras.len(),
        "hay varias coincidencias por obra"
    );

    let primeras = search::buscar_por_obra(&conn, "\"de\"", 3, 0).expect("por obra");
    let siguientes = search::buscar_por_obra(&conn, "\"de\"", 3, 3).expect("por obra");
    let vistas: Vec<&String> = primeras.iter().chain(&siguientes).map(|f| &f.0).collect();
    assert_eq!(primeras.len(), 3);
    assert_eq!(
        siguientes.len(),
        2,
        "solo quedan dos obras tras las tres primeras"
    );
    let mut unicas = vistas.clone();
    unicas.sort();
    unicas.dedup();
    assert_eq!(unicas.len(), 5, "ninguna obra se repite entre páginas");
}

#[test]
fn la_busqueda_por_obra_no_salta_una_edicion_intercalada() {
    let tmp = Temporal::nuevo("intercaladas");
    let db = Database::open_in_memory().unwrap();
    let doc = documento(&tmp, "intercalado.epub");
    let primera = edicion(&db);
    let segunda = edicion(&db);
    let conn = db.connection().unwrap();
    let locator = serde_json::to_string(&doc.text_chunks(512).unwrap()[0].0).unwrap();
    for id in [&primera, &segunda, &primera] {
        conn.execute(
            "INSERT INTO text_chunks (edition_id,locator) VALUES(?1,?2)",
            rusqlite::params![id, locator],
        )
        .unwrap();
        let rowid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO documents_fts(rowid,text) VALUES(?1,'aguja')",
            [rowid],
        )
        .unwrap();
    }
    let filas = search::buscar_por_obra(&conn, "\"aguja\"", 10, 0).unwrap();
    assert_eq!(
        filas.iter().map(|f| &f.0).collect::<Vec<_>>(),
        vec![&primera, &segunda]
    );
    let pagina = search::buscar_por_obra(&conn, "\"aguja\"", 1, 1).unwrap();
    assert_eq!(pagina[0].0, segunda);
}
