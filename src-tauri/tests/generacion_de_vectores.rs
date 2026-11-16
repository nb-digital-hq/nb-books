//! Criterio de aceptación de la generación de vectores.
//!
//! "El usuario conoce el coste en disco y tiempo antes de aceptar."
//!
//! Y lo que la ficha exige para llegar ahí: la generación es incremental, se
//! puede pausar, se detiene con batería baja o calor, se aparta mientras el lector
//! pinta —decidido por el equipo el 2026-09-16— y respeta lo que el usuario excluya.
//! Todo con un vectorizador y unos documentos de prueba, que es lo que deja
//! comprobar cada condición sin un modelo de doscientos megabytes ni un minuto por
//! libro.
//!
//! Contrato: `docs/architecture/ai/embeddings.md`, sección 3.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nb_books_lib::ai::generacion::{
    self, AlcanceDeVectores, Documentos, Exclusion, Generador, Vectorizador, LOTE_DE_FONDO,
};
use nb_books_lib::ai::indice::{self, EnLaBase, VectorIndex};
use nb_books_lib::ai::presupuesto::Suspension;
use nb_books_lib::ai::Embedding;
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::reader::secciones::{DocumentoDeSecciones, Seccion};
use nb_books_lib::reader::{fake, DocumentProvider};
use nb_books_lib::search;

const MODELO: &str = "modelo-falso";

/// Un vectorizador que cuenta lo que calcula y se deja detener.
#[derive(Default)]
struct Falso {
    textos: AtomicUsize,
    llamadas: AtomicUsize,
    sin_modelo: AtomicBool,
    con_bateria_baja: AtomicBool,
    lento: AtomicBool,
}

impl Vectorizador for Falso {
    fn modelo(&self) -> Option<&'static str> {
        (!self.sin_modelo.load(Ordering::SeqCst)).then_some(MODELO)
    }

    fn vectores(&self, textos: &[String]) -> NbResult<Vec<Embedding>> {
        if self.lento.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(60));
        }
        self.llamadas.fetch_add(1, Ordering::SeqCst);
        self.textos.fetch_add(textos.len(), Ordering::SeqCst);
        Ok(textos
            .iter()
            .map(|t| {
                let mut v = vec![0.0_f32; 8];
                for (i, b) in t.bytes().enumerate() {
                    v[i % 8] += f32::from(b);
                }
                let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1.0);
                v.into_iter().map(|x| x / n).collect()
            })
            .collect())
    }

    fn pausa(&self) -> Option<Suspension> {
        self.con_bateria_baja
            .load(Ordering::SeqCst)
            .then_some(Suspension::Bateria)
    }
}

/// Un documento largo: cuatro secciones de unos quince mil caracteres en párrafos.
fn documento_largo() -> DocumentoDeSecciones {
    let secciones = (0..4)
        .map(|s| Seccion {
            href: format!("c{s}.xhtml"),
            title: format!("Capítulo {s}"),
            text: (0..180)
                .map(|p| format!("Párrafo {p} de la sección {s}, con algo de texto que decir sobre el asunto.\n\n"))
                .collect(),
        })
        .collect();
    DocumentoDeSecciones::new(
        fake::MEDIA_TYPE,
        secciones,
        fake::con_tres_capitulos().capabilities(),
    )
}

struct DePrueba;

impl Documentos for DePrueba {
    fn abrir(&self, _edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        Ok(Box::new(documento_largo()))
    }
}

struct Montaje {
    db: Database,
    generador: Arc<Generador>,
    vectorizador: Arc<Falso>,
    indice: Arc<EnLaBase>,
    eventos: Arc<Mutex<Vec<DomainEvent>>>,
}

fn edicion(db: &Database, titulo: &str) -> String {
    let conn = db.connection().expect("conexión");
    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    Edition::create(
        &conn,
        &NewEdition {
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
    .expect("edición")
    .id
}

fn montar() -> Montaje {
    let db = Database::open_in_memory().expect("base");
    let bus = EventBus::new();
    let eventos = Arc::new(Mutex::new(Vec::new()));
    for nombre in ["VectorJobProgress", "VectorJobFinished"] {
        let copia = Arc::clone(&eventos);
        bus.subscribe(nombre, move |e| {
            copia.lock().expect("eventos").push(e.clone())
        });
    }
    let vectorizador = Arc::new(Falso::default());
    let indice = Arc::new(EnLaBase::new(db.clone()));
    let generador = Arc::new(Generador::new(
        db.clone(),
        Arc::clone(&indice) as Arc<dyn VectorIndex>,
        Arc::clone(&vectorizador) as Arc<dyn Vectorizador>,
        Arc::new(DePrueba),
        bus,
        None,
    ));
    Montaje {
        db,
        generador,
        vectorizador,
        indice,
        eventos,
    }
}

fn documento(id: &str) -> AlcanceDeVectores {
    AlcanceDeVectores::Document {
        edition_id: id.to_owned(),
    }
}

fn esperar_a_que_termine(m: &Montaje) {
    let inicio = Instant::now();
    while m.generador.estado().running {
        assert!(
            inicio.elapsed() < Duration::from_secs(20),
            "el trabajo no terminó"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn esperar(tope: Duration, condicion: impl Fn() -> bool) -> bool {
    let inicio = Instant::now();
    while inicio.elapsed() < tope {
        if condicion() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    condicion()
}

/// **El criterio.** Antes de aceptar se sabe cuántos fragmentos, cuánto disco y
/// cuánto tiempo, y **estimar no calcula nada**. Con el documento sin indexar, los
/// fragmentos se suponen y se dice que se suponen; indexado, son los suyos.
#[test]
fn el_coste_se_conoce_antes_de_aceptar_y_estimar_no_calcula_nada() {
    let m = montar();
    let id = edicion(&m.db, "Un libro");

    let sin_indexar = m.generador.estimar(&documento(&id)).expect("estimar");
    assert_eq!(sin_indexar.documents, 1);
    assert_eq!(sin_indexar.not_indexed, 1);
    assert_eq!(sin_indexar.fragments, generacion::FRAGMENTOS_SUPUESTOS);
    assert!(sin_indexar.bytes > 0 && sin_indexar.seconds > 0 && sin_indexar.model_ready);
    assert!(!sin_indexar.measured_rate, "no se ha medido nada todavía");

    search::index_edition(
        &mut m.db.connection().expect("conexión"),
        &id,
        &documento_largo(),
    )
    .expect("indexar");
    let indexado = m.generador.estimar(&documento(&id)).expect("estimar");
    let fragmentos = documento_largo()
        .text_chunks(search::FRAGMENTO)
        .expect("fragmentos")
        .len() as u64;
    assert_eq!(indexado.not_indexed, 0);
    assert_eq!(indexado.fragments, fragmentos);
    assert_eq!(indexado.pending, fragmentos);
    assert_eq!(indexado.bytes, fragmentos * generacion::BYTES_POR_VECTOR);

    assert_eq!(
        m.vectorizador.textos.load(Ordering::SeqCst),
        0,
        "estimar calculó vectores"
    );
    assert!(m
        .indice
        .con_vector(MODELO, &id)
        .expect("vectores")
        .is_empty());
}

/// Aceptado, se calculan todos por lotes, y **la siguiente vez solo lo que falte**:
/// es incremental. Y lo medido reemplaza a lo supuesto en la estimación.
#[test]
fn se_generan_todos_y_la_segunda_vez_solo_lo_que_falta() {
    let m = montar();
    let id = edicion(&m.db, "Un libro");

    m.generador.empezar(&documento(&id)).expect("empezar");
    esperar_a_que_termine(&m);

    let fragmentos =
        indice::fragmentos_de(&m.db.connection().expect("conexión"), &id).expect("fragmentos");
    assert!(
        fragmentos.len() > LOTE_DE_FONDO,
        "el documento de prueba es demasiado corto"
    );
    assert_eq!(
        m.indice.con_vector(MODELO, &id).expect("vectores").len(),
        fragmentos.len()
    );
    assert_eq!(
        m.vectorizador.textos.load(Ordering::SeqCst),
        fragmentos.len()
    );
    assert_eq!(
        m.vectorizador.llamadas.load(Ordering::SeqCst),
        fragmentos.len().div_ceil(LOTE_DE_FONDO),
        "no se calculó por lotes de fondo"
    );

    let despues = m.generador.estimar(&documento(&id)).expect("estimar");
    assert_eq!(despues.pending, 0);
    assert_eq!(despues.bytes, 0);
    assert!(despues.measured_rate);

    m.generador.empezar(&documento(&id)).expect("otra vez");
    esperar_a_que_termine(&m);
    assert_eq!(
        m.vectorizador.textos.load(Ordering::SeqCst),
        fragmentos.len(),
        "la segunda vez recalculó lo que ya estaba"
    );
    assert!(m.eventos.lock().expect("eventos").iter().any(|e| matches!(
        e,
        DomainEvent::VectorJobFinished {
            outcome: "done",
            ..
        }
    )));
}

/// Un documento indexado con otro troceado —de— se reindexa antes
/// de calcular, y los vectores quedan atados a los fragmentos nuevos.
#[test]
fn un_documento_indexado_con_otro_troceado_se_reindexa_y_se_vectoriza() {
    let m = montar();
    let id = edicion(&m.db, "Un libro viejo");
    search::index_edition_con(
        &mut m.db.connection().expect("conexión"),
        &id,
        &documento_largo(),
        300,
    )
    .expect("indexar con otro tamaño");

    m.generador.empezar(&documento(&id)).expect("empezar");
    esperar_a_que_termine(&m);

    let fragmentos =
        indice::fragmentos_de(&m.db.connection().expect("conexión"), &id).expect("fragmentos");
    let esperados = documento_largo()
        .text_chunks(search::FRAGMENTO)
        .expect("fragmentos")
        .len();
    assert_eq!(
        fragmentos.len(),
        esperados,
        "no se reindexó con el troceado de verdad"
    );
    assert_eq!(
        m.indice.con_vector(MODELO, &id).expect("vectores").len(),
        esperados
    );
}

/// **Pausable**: pausado no avanza, y al reanudar termina. Lo mismo con batería
/// baja, que se dice con su motivo.
#[test]
fn pausado_o_con_bateria_baja_no_avanza_y_al_volver_termina() {
    let m = montar();
    let id = edicion(&m.db, "Un libro");

    m.generador.pausar(true);
    m.generador.empezar(&documento(&id)).expect("empezar");
    assert!(esperar(Duration::from_secs(5), || m
        .generador
        .estado()
        .pause_reason
        == Some("USER_PAUSED")));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        m.vectorizador.textos.load(Ordering::SeqCst),
        0,
        "pausado siguió calculando"
    );

    m.vectorizador
        .con_bateria_baja
        .store(true, Ordering::SeqCst);
    m.generador.pausar(false);
    assert!(esperar(Duration::from_secs(5), || m
        .generador
        .estado()
        .pause_reason
        == Some("AI_SUSPENDED_BATTERY")));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        m.vectorizador.textos.load(Ordering::SeqCst),
        0,
        "con batería baja siguió calculando"
    );

    m.vectorizador
        .con_bateria_baja
        .store(false, Ordering::SeqCst);
    esperar_a_que_termine(&m);
    assert!(m.vectorizador.textos.load(Ordering::SeqCst) > 0);
    assert!(m.eventos.lock().expect("eventos").iter().any(|e| matches!(
        e,
        DomainEvent::VectorJobProgress {
            pause_reason: Some("AI_SUSPENDED_BATTERY"),
            ..
        }
    )));
}

/// **Se aparta mientras el lector pinta**: con el lector avisando, no se empieza
/// ningún lote; en cuanto lleva un momento quieto, sigue.
#[test]
fn mientras_el_lector_pinta_no_empieza_ningun_lote() {
    let m = montar();
    let id = edicion(&m.db, "Un libro");
    m.vectorizador.lento.store(true, Ordering::SeqCst);

    m.generador.empezar(&documento(&id)).expect("empezar");
    assert!(esperar(Duration::from_secs(5), || m
        .vectorizador
        .llamadas
        .load(Ordering::SeqCst)
        >= 1));

    // Un lector que desplaza, avisando cada 200 ms durante un segundo y medio.
    let inicio = Instant::now();
    let mut llamadas_al_parar = None;
    while inicio.elapsed() < Duration::from_millis(1_500) {
        m.generador.apuntar_actividad_del_lector();
        std::thread::sleep(Duration::from_millis(200));
        if inicio.elapsed() > Duration::from_millis(300) && llamadas_al_parar.is_none() {
            // El lote que estaba en marcha termina; después, ninguno más.
            llamadas_al_parar = Some(m.vectorizador.llamadas.load(Ordering::SeqCst));
        }
    }
    let durante = m.vectorizador.llamadas.load(Ordering::SeqCst);
    assert_eq!(
        Some(durante),
        llamadas_al_parar,
        "se calcularon lotes mientras el lector pintaba"
    );
    assert_eq!(m.generador.estado().pause_reason, Some("READER_ACTIVE"));

    esperar_a_que_termine(&m);
    assert!(m.vectorizador.llamadas.load(Ordering::SeqCst) > durante);
}

#[test]
fn el_lector_deja_de_contar_como_activo_al_rato() {
    let ahora = Instant::now();
    assert!(!generacion::lector_activo(None, ahora));
    assert!(generacion::lector_activo(
        Some(ahora),
        ahora + Duration::from_millis(500)
    ));
    assert!(!generacion::lector_activo(
        Some(ahora),
        ahora + generacion::LECTOR_QUIETO
    ));
}

/// **Cancelar** corta, y lo ya calculado se queda: la próxima vez sigue desde ahí.
#[test]
fn cancelar_corta_y_lo_calculado_se_queda() {
    let m = montar();
    let id = edicion(&m.db, "Un libro");
    m.vectorizador.lento.store(true, Ordering::SeqCst);

    m.generador.empezar(&documento(&id)).expect("empezar");
    assert!(esperar(Duration::from_secs(5), || m
        .vectorizador
        .llamadas
        .load(Ordering::SeqCst)
        >= 2));
    m.generador.cancelar();
    esperar_a_que_termine(&m);

    let hechos = m.indice.con_vector(MODELO, &id).expect("vectores").len();
    let total = indice::fragmentos_de(&m.db.connection().expect("conexión"), &id)
        .expect("fragmentos")
        .len();
    assert!(hechos > 0 && hechos < total, "{hechos} de {total}");
    assert!(m.eventos.lock().expect("eventos").iter().any(|e| matches!(
        e,
        DomainEvent::VectorJobFinished {
            outcome: "cancelled",
            ..
        }
    )));

    m.vectorizador.lento.store(false, Ordering::SeqCst);
    let antes = m.vectorizador.textos.load(Ordering::SeqCst);
    m.generador.empezar(&documento(&id)).expect("seguir");
    esperar_a_que_termine(&m);
    assert_eq!(
        m.indice.con_vector(MODELO, &id).expect("vectores").len(),
        total
    );
    assert_eq!(
        m.vectorizador.textos.load(Ordering::SeqCst) - antes,
        total - hechos
    );
}

/// **Excluir**: un documento excluido no se estima, no se vectoriza, y excluirlo
/// borra lo que ya tuviera. Excluir una colección excluye sus documentos.
#[test]
fn lo_excluido_no_se_vectoriza_y_excluirlo_borra_sus_vectores() {
    let m = montar();
    let uno = edicion(&m.db, "Uno");
    let otro = edicion(&m.db, "Otro");
    m.db.connection()
        .expect("conexión")
        .execute_batch(&format!(
            "INSERT INTO collections (id, name, kind) VALUES ('col', 'Tesis', 'manual');
             INSERT INTO collection_items (collection_id, edition_id) VALUES ('col', '{uno}'), ('col', '{otro}');"
        ))
        .expect("colección");
    let coleccion = AlcanceDeVectores::Collection {
        collection_id: "col".to_owned(),
    };

    m.generador.empezar(&documento(&uno)).expect("empezar");
    esperar_a_que_termine(&m);
    assert!(!m
        .indice
        .con_vector(MODELO, &uno)
        .expect("vectores")
        .is_empty());

    generacion::excluir(&m.db, m.indice.as_ref(), Exclusion::Edition, &uno, true).expect("excluir");
    assert!(
        m.indice
            .con_vector(MODELO, &uno)
            .expect("borrados")
            .is_empty(),
        "excluir dejó sus vectores"
    );
    let estimacion = m.generador.estimar(&coleccion).expect("estimar");
    assert_eq!((estimacion.documents, estimacion.excluded), (1, 1));

    m.generador.empezar(&coleccion).expect("colección");
    esperar_a_que_termine(&m);
    assert!(m
        .indice
        .con_vector(MODELO, &uno)
        .expect("excluido")
        .is_empty());
    assert!(!m
        .indice
        .con_vector(MODELO, &otro)
        .expect("incluido")
        .is_empty());

    generacion::excluir(&m.db, m.indice.as_ref(), Exclusion::Collection, "col", true)
        .expect("excluir colección");
    let conn = m.db.connection().expect("conexión");
    assert!(generacion::esta_excluido(&conn, Exclusion::Collection, "col").expect("excluida"));
    assert!(m
        .indice
        .con_vector(MODELO, &otro)
        .expect("borrados")
        .is_empty());
    assert_eq!(
        m.generador.estimar(&coleccion).expect("estimar").documents,
        0
    );

    generacion::excluir(
        &m.db,
        m.indice.as_ref(),
        Exclusion::Collection,
        "col",
        false,
    )
    .expect("incluir");
    generacion::excluir(&m.db, m.indice.as_ref(), Exclusion::Edition, &uno, false)
        .expect("incluir");
    assert_eq!(
        m.generador.estimar(&coleccion).expect("estimar").documents,
        2
    );
}

/// Sin modelo listo no se empieza nada, y la estimación lo dice.
#[test]
fn sin_modelo_no_se_empieza_y_la_estimacion_lo_dice() {
    let m = montar();
    let id = edicion(&m.db, "Un libro");
    m.vectorizador.sin_modelo.store(true, Ordering::SeqCst);

    assert!(
        !m.generador
            .estimar(&documento(&id))
            .expect("estimar")
            .model_ready
    );
    assert_eq!(
        m.generador.empezar(&documento(&id)),
        Err(NbError::ComponentUnavailable)
    );
    assert!(!m.generador.estado().running);
}
