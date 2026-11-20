//! La narración que trae el propio libro.
//!
//! Un EPUB 3 con superposiciones -Media Overlays- asocia cada trozo del texto con
//! el tramo de un audio que lo lee. El criterio pide tres cosas: que el audio se
//! reproduzca **anclado por Locator**, que el bloque que suena se resalte con el
//! mismo mecanismo que la voz, y que la posición sobreviva a cerrar y volver a
//! abrir; y que sin superposiciones el documento se comporte exactamente como
//! antes. Aquí se comprueba la mitad del runtime: qué se lee de la superposición,
//! dónde cae cada tramo y que la posición a la que lleva sobrevive. El resaltado
//! lo comprueba el recorrido de `e2e/flujo.spec.ts`.
//!
//! El libro se construye aquí: no hay en el corpus real ninguno con narración que
//! se pueda meter en el repositorio, y un audio de verdad pesaría lo que no hace
//! falta. Lo que importa es la forma, que es la de la especificación.

mod comun;

use std::path::PathBuf;
use std::sync::Arc;

use nb_books_lib::db::catalog::StorageMode;
use nb_books_lib::db::Database;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
use nb_books_lib::reader::epub::{EpubFactory, MEDIA_TYPE};
use nb_books_lib::reader::progress::ProgressTracker;
use nb_books_lib::reader::{ProviderFactory, ProviderRegistry, SessionStore};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-narrado-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("carpeta");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CONTENEDOR: &str = r#"<?xml version="1.0"?><container version="1.0"
  xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles>
  <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles></container>"#;

fn paquete(con_narracion: bool) -> String {
    let superposicion = |id: &str| {
        if con_narracion {
            format!(r#" media-overlay="{id}""#)
        } else {
            String::new()
        }
    };
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0">
        <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Un libro narrado</dc:title>
        <meta property="media:duration">0:00:05.000</meta></metadata>
        <manifest>
          <item id="c1" href="c1.xhtml" media-type="application/xhtml+xml"{}/>
          <item id="c2" href="c2.xhtml" media-type="application/xhtml+xml"{}/>
          <item id="c3" href="c3.xhtml" media-type="application/xhtml+xml"/>
          <item id="m1" href="smil/c1.smil" media-type="application/smil+xml"/>
          <item id="m2" href="smil/c2.smil" media-type="application/smil+xml"/>
          <item id="a1" href="audio/c1.wav" media-type="audio/wav"/>
          <item id="a2" href="audio/c2.wav" media-type="audio/wav"/>
        </manifest>
        <spine><itemref idref="c1"/><itemref idref="c2"/><itemref idref="c3"/></spine></package>"#,
        superposicion("m1"),
        superposicion("m2")
    )
}

const CAPITULO_1: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>
  <h1 id="t1">Capítulo primero</h1>
  <p id="f1">Muchos años después, frente al pelotón de fusilamiento.</p>
  <p id="f2">Macondo era entonces una aldea de veinte casas.</p>
  <p id="f3">El mundo era tan reciente que muchas cosas carecían de nombre.</p>
</body></html>"#;

const CAPITULO_2: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>
  <p id="g1">Todos los años, por el mes de marzo, llegaba una familia de gitanos.</p>
</body></html>"#;

const CAPITULO_3: &str = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><body>
  <p>Un capítulo sin narración.</p>
</body></html>"#;

/// Las tres formas de reloj de la especificación, a propósito; un `par` con una
/// ruta que se sale del contenedor, que tiene que caer; y uno que lee otro
/// capítulo, que no es de este.
const SMIL_1: &str = r#"<?xml version="1.0"?>
<smil xmlns="http://www.w3.org/ns/SMIL" xmlns:epub="http://www.idpf.org/2007/ops" version="3.0">
 <body>
  <seq epub:textref="../c1.xhtml" epub:type="chapter">
   <par id="p0"><text src="../c1.xhtml#t1"/><audio src="../audio/c1.wav" clipBegin="0:00:00.000" clipEnd="0:00:00.800"/></par>
   <par id="p1"><text src="../c1.xhtml#f1"/><audio src="../audio/c1.wav" clipBegin="0:00:00.800" clipEnd="00:02.000"/></par>
   <par id="p2"><text src="../c1.xhtml#f2"/><audio src="../audio/c1.wav" clipBegin="2s" clipEnd="3500ms"/></par>
   <par id="p3"><text src="../c1.xhtml#f3"/><audio src="../audio/c1.wav" clipBegin="3.5" clipEnd="0.075min"/></par>
   <par id="malo"><text src="../../../fuera.xhtml#x"/><audio src="../../../../etc/passwd" clipBegin="0s" clipEnd="1s"/></par>
   <par id="ajeno"><text src="../c2.xhtml#g1"/><audio src="../audio/c1.wav" clipBegin="4.5s" clipEnd="5s"/></par>
  </seq>
 </body>
</smil>"#;

const SMIL_2: &str = r#"<?xml version="1.0"?>
<smil xmlns="http://www.w3.org/ns/SMIL" version="3.0"><body><seq>
   <par><text src="../c2.xhtml#g1"/><audio src="../audio/c2.wav" clipBegin="0s" clipEnd="2s"/></par>
</seq></body></smil>"#;

fn libro(tmp: &Temporal, con_narracion: bool) -> PathBuf {
    let opf = paquete(con_narracion);
    let audio = comun::silencio_wav(5);
    let bytes = comun::zip_sin_comprimir(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", CONTENEDOR.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        ("OEBPS/c1.xhtml", CAPITULO_1.as_bytes()),
        ("OEBPS/c2.xhtml", CAPITULO_2.as_bytes()),
        ("OEBPS/c3.xhtml", CAPITULO_3.as_bytes()),
        ("OEBPS/smil/c1.smil", SMIL_1.as_bytes()),
        ("OEBPS/smil/c2.smil", SMIL_2.as_bytes()),
        ("OEBPS/audio/c1.wav", &audio),
        ("OEBPS/audio/c2.wav", &audio),
    ]);
    let nombre = if con_narracion {
        "narrado.epub"
    } else {
        "mudo.epub"
    };
    let ruta = tmp.0.join(nombre);
    std::fs::write(&ruta, bytes).expect("libro");
    ruta
}

fn fabrica(tmp: &Temporal) -> EpubFactory {
    EpubFactory::new(tmp.0.join("cache/recursos"))
}

#[test]
fn un_libro_con_su_narracion_la_declara_y_uno_sin_ella_no() {
    let tmp = Temporal::nuevo("capacidad");
    let narrado = fabrica(&tmp).open(&libro(&tmp, true)).expect("abierto");
    assert!(narrado.capabilities().synchronized_audio);

    // Sin superposiciones, exactamente como antes: ni la capacidad ni el audio.
    let mudo = fabrica(&tmp).open(&libro(&tmp, false)).expect("abierto");
    assert!(!mudo.capabilities().synchronized_audio);
    assert_eq!(
        mudo.synchronized_audio("OEBPS/c1.xhtml").expect("responde"),
        None
    );
}

#[test]
fn cada_tramo_del_audio_cae_en_el_parrafo_que_lee() {
    let tmp = Temporal::nuevo("tramos");
    let doc = fabrica(&tmp).open(&libro(&tmp, true)).expect("abierto");
    let narracion = doc
        .synchronized_audio("OEBPS/c1.xhtml")
        .expect("se lee")
        .expect("el capítulo trae narración");

    // Las tres formas de reloj, y el `par` que se salía del contenedor, fuera.
    let tramos: Vec<(u64, u64)> = narracion
        .marks
        .iter()
        .map(|m| (m.start_ms, m.end_ms))
        .collect();
    assert_eq!(
        tramos,
        vec![(0, 800), (800, 2000), (2000, 3500), (3500, 4500)]
    );
    assert_eq!(narracion.duration_ms, 4500);

    // Cada tramo lleva al principio de su párrafo, **por Locator**: el texto que
    // hay en su posición es el del párrafo que el audio está leyendo.
    let esperados = [
        "Capítulo primero",
        "Muchos años después",
        "Macondo era entonces",
        "El mundo era tan reciente",
    ];
    for (marca, esperado) in narracion.marks.iter().zip(esperados) {
        let texto = doc
            .text_range(&marca.locator, esperado.chars().count())
            .expect("texto");
        assert_eq!(texto, esperado);
        assert_eq!(marca.locator.href, "OEBPS/c1.xhtml");
    }

    // El audio está extraído a la caché, entero, para pedirlo por el protocolo de
    // assets; y el de fuera del contenedor no se leyó.
    let audio = std::fs::read(&narracion.path).expect("el audio está en la caché");
    assert_eq!(audio, comun::silencio_wav(5));
    assert!(narracion
        .path
        .starts_with(&tmp.0.to_string_lossy().into_owned()));
}

#[test]
fn un_capitulo_sin_narracion_en_un_libro_narrado_no_trae_audio() {
    let tmp = Temporal::nuevo("sin-capitulo");
    let doc = fabrica(&tmp).open(&libro(&tmp, true)).expect("abierto");
    assert_eq!(
        doc.synchronized_audio("OEBPS/c3.xhtml").expect("responde"),
        None
    );
    assert!(doc
        .synchronized_audio("OEBPS/c2.xhtml")
        .expect("responde")
        .is_some());
}

/// **La tercera mitad del criterio.** Seguir la narración mueve la lectura a cada
/// tramo, y lo que se guarda es su `Locator`: cerrar y volver a abrir devuelve al
/// párrafo que sonaba, no al principio del capítulo.
#[test]
fn la_posicion_de_la_narracion_sobrevive_a_cerrar_y_volver_a_abrir() {
    let tmp = Temporal::nuevo("reapertura");
    let origen = tmp.0.join("origen");
    std::fs::create_dir_all(&origen).expect("origen");
    let fuente = libro(&tmp, true);
    let destino = origen.join("narrado.epub");
    std::fs::copy(&fuente, &destino).expect("copia");

    let db = Database::open(&tmp.0.join("nb.sqlite")).expect("base");
    let archivo = {
        let mut conn = db.connection().expect("conexión");
        match import_document(
            &mut conn,
            &ImportContext {
                scope: &origen,
                library: &tmp.0.join("biblioteca"),
                bus: &EventBus::new(),
            },
            &ImportRequest {
                source: destino,
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importación")
        {
            ImportOutcome::Imported(datos) => datos.file.id.clone(),
            ImportOutcome::AlreadyInLibrary(f) | ImportOutcome::Duplicate(f) => f.id.clone(),
        }
    };
    let registro = {
        let mut r = ProviderRegistry::new();
        r.register(Box::new(fabrica(&tmp)));
        r
    };
    assert!(registro.supports(MEDIA_TYPE));

    let sonaba = {
        let almacen = SessionStore::new(Arc::new(ProgressTracker::new(db.clone())));
        let sesion = almacen.open(&db, &registro, &archivo).expect("abierta");
        let narracion = sesion
            .synchronized_audio("OEBPS/c1.xhtml")
            .expect("se lee")
            .expect("narración");
        // Suena el tercer tramo: el lector va ahí, como lo hace la interfaz.
        let tercero = narracion.marks[2].locator.clone();
        sesion.go_to(&tercero).expect("se sigue");
        almacen.close(&archivo).expect("cierre");
        tercero
    };

    let almacen = SessionStore::new(Arc::new(ProgressTracker::new(db.clone())));
    let sesion = almacen.open(&db, &registro, &archivo).expect("reabierta");
    assert_eq!(
        sesion.info().locator,
        sonaba,
        "vuelve al párrafo que sonaba"
    );
    let narracion = sesion
        .synchronized_audio("OEBPS/c1.xhtml")
        .expect("se lee")
        .expect("narración");
    assert_eq!(
        narracion.marks[2].locator, sonaba,
        "y el tramo que lo lee es el mismo Locator: se puede seguir escuchando desde ahí"
    );
}
