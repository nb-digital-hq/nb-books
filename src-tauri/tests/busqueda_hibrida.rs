//! Criterio de aceptación de la búsqueda híbrida.
//!
//! "Con el subsistema de IA caído, la búsqueda léxica sigue funcionando sin
//! degradación."
//!
//! La búsqueda por significado es un camino aparte de la léxica, y estas pruebas
//! comprueban las dos cosas que eso tiene que garantizar: que la léxica devuelve
//! **exactamente lo mismo** con la IA sana, sin modelo, fallando o entrando en
//! pánico; y que la de significado encuentra por cercanía, no carga nada cuando no
//! hay nada preparado y dice que no está cuando no hay modelo.
//!
//! La fusión de las dos listas y la distinción visual son de la interfaz, y las
//! comprueban `fusion.test.ts` y un recorrido.
//!
//! Contrato: `docs/architecture/ai/embeddings.md`, sección 4.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use nb_books_lib::ai::generacion::{AlcanceDeVectores, Documentos, Generador, Vectorizador};
use nb_books_lib::ai::indice::{EnLaBase, VectorIndex};
use nb_books_lib::ai::presupuesto::Suspension;
use nb_books_lib::ai::Embedding;
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::events::EventBus;
use nb_books_lib::reader::secciones::{DocumentoDeSecciones, Seccion};
use nb_books_lib::reader::{fake, DocumentProvider};
use nb_books_lib::search::{self, Results, Scope};

const DIMENSION: usize = 64;

/// Cómo está la IA en cada prueba.
#[derive(Default)]
struct Ia {
    sin_modelo: AtomicBool,
    falla: AtomicBool,
    en_panico: AtomicBool,
    llamadas: AtomicUsize,
}

/// Un vector por palabras: dos textos que comparten palabras se parecen.
fn vector_de(texto: &str) -> Embedding {
    let mut v = vec![0.0_f32; DIMENSION];
    for palabra in texto
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| p.chars().count() > 3)
    {
        let mut h: u64 = 1_469_598_103_934_665_603;
        for b in palabra.to_lowercase().bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(1_099_511_628_211);
        }
        v[usize::try_from(h % DIMENSION as u64).expect("cabe")] += 1.0;
    }
    let norma = v
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt()
        .max(f32::EPSILON);
    v.into_iter().map(|x| x / norma).collect()
}

impl Vectorizador for Ia {
    fn modelo(&self) -> Option<&'static str> {
        (!self.sin_modelo.load(Ordering::SeqCst)).then_some("modelo-de-prueba")
    }

    fn vectores(&self, textos: &[String]) -> NbResult<Vec<Embedding>> {
        self.llamadas.fetch_add(1, Ordering::SeqCst);
        assert!(
            !self.en_panico.load(Ordering::SeqCst),
            "el motor de IA entró en pánico"
        );
        if self.falla.load(Ordering::SeqCst) {
            return Err(NbError::ComponentUnavailable);
        }
        Ok(textos.iter().map(|t| vector_de(t)).collect())
    }

    fn pausa(&self) -> Option<Suspension> {
        None
    }
}

fn documento() -> DocumentoDeSecciones {
    let textos = [
        "El faro de la costa guiaba a los barcos pesqueros durante las tormentas del invierno.",
        "La cosecha de trigo llenó los graneros del pueblo antes de que llegaran las lluvias.",
        "Los astrónomos observaron el cometa desde el observatorio de la montaña toda la noche.",
    ];
    DocumentoDeSecciones::new(
        fake::MEDIA_TYPE,
        textos
            .iter()
            .enumerate()
            .map(|(i, t)| Seccion {
                href: format!("c{i}.xhtml"),
                title: format!("Capítulo {i}"),
                text: (*t).to_owned(),
            })
            .collect(),
        fake::con_tres_capitulos().capabilities(),
    )
}

struct DePrueba;

impl Documentos for DePrueba {
    fn abrir(&self, _edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        Ok(Box::new(documento()))
    }
}

struct Montaje {
    db: Database,
    ia: Arc<Ia>,
    generador: Arc<Generador>,
    edicion: String,
}

fn montar() -> Montaje {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
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
            title: "Obra".to_owned(),
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
    drop(conn);
    search::index_edition(
        &mut db.connection().expect("conexión"),
        &edicion,
        &documento(),
    )
    .expect("indexar");

    let ia = Arc::new(Ia::default());
    let generador = Arc::new(Generador::new(
        db.clone(),
        Arc::new(EnLaBase::new(db.clone())) as Arc<dyn VectorIndex>,
        Arc::clone(&ia) as Arc<dyn Vectorizador>,
        Arc::new(DePrueba),
        EventBus::new(),
        None,
    ));
    Montaje {
        db,
        ia,
        generador,
        edicion,
    }
}

fn preparar(m: &Montaje) {
    m.generador
        .empezar(&AlcanceDeVectores::Document {
            edition_id: m.edicion.clone(),
        })
        .expect("preparar");
    let inicio = std::time::Instant::now();
    while m.generador.estado().running {
        assert!(inicio.elapsed().as_secs() < 10, "no terminó de preparar");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn documento_abierto(m: &Montaje) -> Scope {
    Scope::Document {
        edition_id: m.edicion.clone(),
    }
}

/// Lo que devuelve la búsqueda léxica, sin el pasaje, que se compone fuera.
fn lexica(m: &Montaje, consulta: &str) -> Vec<String> {
    let conn = m.db.connection().expect("conexión");
    let Results::Text(hits) =
        search::search(&conn, consulta, &documento_abierto(m), 20, 0).expect("léxica")
    else {
        panic!("se esperaban párrafos");
    };
    hits.into_iter()
        .map(|h| format!("{}|{}", h.locator.href, h.rank))
        .collect()
}

/// **Por significado se encuentra lo cercano**, aunque no comparta la forma exacta
/// de lo buscado, con el pasaje para enseñarlo.
#[test]
fn por_significado_se_encuentra_lo_cercano() {
    let m = montar();
    preparar(&m);

    let cercanos = m
        .generador
        .buscar("tormentas barcos faro", &documento_abierto(&m), 3)
        .expect("buscar");
    assert!(!cercanos.is_empty());
    assert_eq!(cercanos[0].locator.href, "c0.xhtml");
    assert!(cercanos[0].snippet.contains("faro"));
    assert!(cercanos
        .windows(2)
        .all(|p| p[0].similarity >= p[1].similarity));
}

/// Sin nada preparado en el alcance **no se calcula el vector de la consulta**: no
/// se carga el modelo para compararlo con nada.
#[test]
fn sin_nada_preparado_no_se_calcula_nada() {
    let m = montar();

    let cercanos = m
        .generador
        .buscar("tormentas barcos faro", &documento_abierto(&m), 5)
        .expect("buscar");
    assert!(cercanos.is_empty());
    assert_eq!(
        m.ia.llamadas.load(Ordering::SeqCst),
        0,
        "se calculó sin nada preparado"
    );

    // Tampoco en el catálogo, que no es de párrafos.
    assert!(m
        .generador
        .buscar("faro", &Scope::Catalog, 5)
        .expect("catálogo")
        .is_empty());
}

/// **El criterio.** Con la IA sana, sin modelo, fallando o entrando en pánico, la
/// búsqueda léxica devuelve exactamente lo mismo; la de significado dice que no
/// está, y un pánico no sale de su hilo.
#[test]
fn con_la_ia_caida_la_busqueda_lexica_no_cambia() {
    let m = montar();
    preparar(&m);
    let consulta = "cometa";
    let sana = lexica(&m, consulta);
    assert!(
        !sana.is_empty(),
        "la búsqueda léxica no encontraba nada ya con la IA sana"
    );

    // Sin modelo.
    m.ia.sin_modelo.store(true, Ordering::SeqCst);
    assert_eq!(lexica(&m, consulta), sana);
    assert_eq!(
        m.generador.buscar(consulta, &documento_abierto(&m), 5),
        Err(NbError::ComponentUnavailable)
    );
    m.ia.sin_modelo.store(false, Ordering::SeqCst);

    // Fallando al calcular.
    m.ia.falla.store(true, Ordering::SeqCst);
    assert_eq!(lexica(&m, consulta), sana);
    assert!(m
        .generador
        .buscar(consulta, &documento_abierto(&m), 5)
        .is_err());
    m.ia.falla.store(false, Ordering::SeqCst);

    // En pánico: el de la búsqueda por significado no llega a la léxica.
    m.ia.en_panico.store(true, Ordering::SeqCst);
    let generador = Arc::clone(&m.generador);
    let alcance = documento_abierto(&m);
    let hilo = std::thread::spawn(move || generador.buscar("cometa", &alcance, 5));
    assert!(hilo.join().is_err(), "el pánico simulado no ocurrió");
    assert_eq!(lexica(&m, consulta), sana);
}
