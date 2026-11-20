//! Criterio de aceptación del índice vectorial.
//!
//! "Borrar el índice completo no pierde información; cambiar de motor solo
//! requiere reindexar."
//!
//! Las dos mitades se comprueban de verdad y no por construcción:
//!
//! * **Borrar no pierde nada**: con fragmentos, búsqueda léxica, anotaciones y
//!   colecciones en la base, vaciar el índice deja todo eso igual, y volver a
//!   generarlo da los mismos resultados que antes.
//! * **Cambiar de motor es reindexar**: las mismas pruebas de contrato pasan con el
//!   índice de ADR-007 y con otro escrito aquí, en memoria y sin nada en común con
//!   él; y regenerado en el otro, busca lo mismo.
//!
//! Contrato: `docs/adr/007-vector-store.md`.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use nb_books_lib::ai::indice::{self, Alcance, Cercano, EnLaBase, VectorIndex};
use nb_books_lib::ai::Embedding;
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbResult;
use nb_books_lib::reader::fake;
use nb_books_lib::reader::DocumentProvider;
use nb_books_lib::search;

const MODELO: &str = "modelo-de-prueba";
const OTRO_MODELO: &str = "otro-modelo";
const DIMENSION: usize = 384;

/// Otro motor, en memoria, que no comparte nada con el de la base. Existe para
/// comprobar que el contrato no depende de cómo se guarda un vector.
#[derive(Default)]
struct EnMemoria {
    vectores: Mutex<HashMap<String, (String, String, Embedding)>>,
}

impl VectorIndex for EnMemoria {
    fn guardar(
        &self,
        modelo: &str,
        edition_id: &str,
        vectores: &[(String, Embedding)],
    ) -> NbResult<()> {
        let mut guardados = self.vectores.lock().expect("vectores");
        for (chunk, vector) in vectores {
            guardados.insert(
                chunk.clone(),
                (edition_id.to_owned(), modelo.to_owned(), vector.clone()),
            );
        }
        Ok(())
    }

    fn con_vector(&self, modelo: &str, edition_id: &str) -> NbResult<HashSet<String>> {
        Ok(self
            .vectores
            .lock()
            .expect("vectores")
            .iter()
            .filter(|(_, (e, m, _))| e == edition_id && m == modelo)
            .map(|(c, _)| c.clone())
            .collect())
    }

    fn buscar(
        &self,
        modelo: &str,
        consulta: &[f32],
        alcance: &Alcance,
        limite: usize,
    ) -> NbResult<Vec<Cercano>> {
        let mut cercanos: Vec<Cercano> = self
            .vectores
            .lock()
            .expect("vectores")
            .iter()
            .filter(|(_, (e, m, _))| {
                m == modelo
                    && match alcance {
                        Alcance::Todo => true,
                        Alcance::Ediciones(ids) => ids.contains(e),
                    }
            })
            .map(|(c, (e, _, v))| Cercano {
                chunk_id: c.clone(),
                edition_id: e.clone(),
                similitud: v.iter().zip(consulta).map(|(a, b)| a * b).sum(),
            })
            .collect();
        cercanos.sort_by(|a, b| b.similitud.total_cmp(&a.similitud));
        cercanos.truncate(limite);
        Ok(cercanos)
    }

    fn hay(&self, modelo: &str, alcance: &Alcance) -> NbResult<bool> {
        Ok(!self.buscar(modelo, &[], alcance, 1)?.is_empty())
    }

    fn olvidar(&self, edition_id: &str) -> NbResult<usize> {
        let mut guardados = self.vectores.lock().expect("vectores");
        let antes = guardados.len();
        guardados.retain(|_, (e, _, _)| e != edition_id);
        Ok(antes - guardados.len())
    }

    fn vaciar(&self) -> NbResult<usize> {
        let mut guardados = self.vectores.lock().expect("vectores");
        let cuantos = guardados.len();
        guardados.clear();
        Ok(cuantos)
    }
}

/// Un "modelo" determinista: cada palabra suma una dirección fija, así que dos
/// textos que comparten palabras se parecen. No es semántica, pero sí es un
/// vector que depende del texto, que es lo que hace falta para que ordenar
/// signifique algo.
fn vector_de(texto: &str) -> Embedding {
    let mut v = vec![0.0_f32; DIMENSION];
    for palabra in texto
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| p.len() > 2)
    {
        let mut h: u64 = 1_469_598_103_934_665_603;
        for b in palabra.to_lowercase().bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(1_099_511_628_211);
        }
        for k in 0..4 {
            let i = usize::try_from((h >> (k * 8)) % DIMENSION as u64).expect("cabe");
            v[i] += if (h >> (32 + k)) & 1 == 0 { 1.0 } else { -1.0 };
        }
    }
    let norma = v
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt()
        .max(f32::EPSILON);
    v.into_iter().map(|x| x / norma).collect()
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

/// Una base con dos ediciones indexadas con el documento de prueba, troceado en
/// fragmentos pequeños para que haya varios.
fn base_con_dos_ediciones() -> (Database, Vec<String>) {
    let db = Database::open_in_memory().expect("base");
    let documento = fake::con_tres_capitulos();
    let ediciones: Vec<String> = ["Una", "Otra"].iter().map(|t| edicion(&db, t)).collect();
    for id in &ediciones {
        search::index_edition_con(&mut db.connection().expect("conexión"), id, &documento, 60)
            .expect("indexar");
    }
    (db, ediciones)
}

/// **Reindexar el índice vectorial**: volver a trocear, emparejar con los
/// fragmentos guardados y guardar el vector de cada uno. Es lo único que hace falta
/// para pasar a otro motor, y usa solo el contrato.
fn regenerar(indice: &dyn VectorIndex, db: &Database, ediciones: &[String], modelo: &str) -> usize {
    let documento = fake::con_tres_capitulos();
    let producidos = documento.text_chunks(60).expect("fragmentos");
    let mut total = 0;
    for id in ediciones {
        let guardados =
            indice::fragmentos_de(&db.connection().expect("conexión"), id).expect("guardados");
        let emparejados = indice::alinear(&producidos, &guardados).expect("coinciden");
        let vectores: Vec<(String, Embedding)> = emparejados
            .into_iter()
            .map(|(chunk, texto)| (chunk, vector_de(&texto)))
            .collect();
        total += vectores.len();
        indice.guardar(modelo, id, &vectores).expect("guardar");
    }
    total
}

/// Las pruebas del contrato, sobre cualquier motor.
fn contrato(indice: &dyn VectorIndex, db: &Database, ediciones: &[String]) {
    let generados = regenerar(indice, db, ediciones, MODELO);
    assert!(
        generados > 4,
        "el documento de prueba dio {generados} fragmentos"
    );

    // Todos los fragmentos tienen vector, y solo del modelo con que se guardaron.
    let fragmentos = indice::fragmentos_de(&db.connection().expect("conexión"), &ediciones[0])
        .expect("fragmentos");
    let con = indice
        .con_vector(MODELO, &ediciones[0])
        .expect("con vector");
    assert_eq!(con.len(), fragmentos.len());
    assert!(indice
        .con_vector(OTRO_MODELO, &ediciones[0])
        .expect("otro")
        .is_empty());

    // Lo más cercano a un texto es el fragmento que lo contiene.
    let consulta = vector_de("pelotón de fusilamiento coronel Aureliano");
    let cercanos = indice
        .buscar(
            MODELO,
            &consulta,
            &Alcance::Ediciones(vec![ediciones[0].clone()]),
            3,
        )
        .expect("buscar");
    assert_eq!(cercanos.len(), 3);
    assert!(
        cercanos
            .windows(2)
            .all(|p| p[0].similitud >= p[1].similitud),
        "no vienen en orden"
    );
    assert!(
        cercanos.iter().all(|c| c.edition_id == ediciones[0]),
        "se salió del alcance"
    );
    let (_, locator) =
        indice::fragmento(&db.connection().expect("conexión"), &cercanos[0].chunk_id)
            .expect("fragmento")
            .expect("existe");
    let documento = fake::con_tres_capitulos();
    let texto = documento
        .text_range(
            &nb_books_lib::reader::Locator::from_json(&locator).expect("locator"),
            60,
        )
        .expect("texto");
    assert!(
        texto.contains("fusilamiento") || texto.contains("pelotón") || texto.contains("Aureliano")
    );

    // Con otro modelo no se encuentra nada: no se comparan vectores de modelos distintos.
    assert!(indice
        .buscar(OTRO_MODELO, &consulta, &Alcance::Todo, 5)
        .expect("otro")
        .is_empty());
    // En todo, aparecen las dos ediciones.
    let todo = indice
        .buscar(MODELO, &consulta, &Alcance::Todo, 100)
        .expect("todo");
    let de: HashSet<&String> = todo.iter().map(|c| &c.edition_id).collect();
    assert_eq!(de.len(), 2);

    // Hay vectores de este modelo, en todo y en cada edición, y de otro no.
    assert!(indice.hay(MODELO, &Alcance::Todo).expect("hay"));
    assert!(indice
        .hay(MODELO, &Alcance::Ediciones(vec![ediciones[1].clone()]))
        .expect("hay en la otra"));
    assert!(!indice
        .hay(OTRO_MODELO, &Alcance::Todo)
        .expect("otro modelo"));
    assert!(!indice
        .hay(MODELO, &Alcance::Ediciones(vec!["no-existe".to_owned()]))
        .expect("edición sin vectores"));

    // Olvidar una edición no toca la otra.
    assert_eq!(
        indice.olvidar(&ediciones[1]).expect("olvidar"),
        fragmentos.len()
    );
    assert!(indice
        .con_vector(MODELO, &ediciones[1])
        .expect("olvidada")
        .is_empty());
    assert_eq!(
        indice
            .con_vector(MODELO, &ediciones[0])
            .expect("intacta")
            .len(),
        fragmentos.len()
    );

    // Y vaciar lo vacía todo.
    assert_eq!(indice.vaciar().expect("vaciar"), fragmentos.len());
    assert!(indice
        .buscar(MODELO, &consulta, &Alcance::Todo, 5)
        .expect("vacío")
        .is_empty());
    assert!(!indice.hay(MODELO, &Alcance::Todo).expect("vacío"));
}

/// **Cambiar de motor solo requiere cumplir el contrato**: las mismas pruebas, con
/// el índice de la base y con uno en memoria que no comparte nada con él.
#[test]
fn el_contrato_se_cumple_con_cualquier_motor() {
    let (db, ediciones) = base_con_dos_ediciones();
    contrato(&EnLaBase::new(db.clone()), &db, &ediciones);

    let (db, ediciones) = base_con_dos_ediciones();
    contrato(&EnMemoria::default(), &db, &ediciones);
}

/// **Cambiar de motor es reindexar**: generado en el de la base y regenerado en el
/// de memoria desde los fragmentos, los dos encuentran lo mismo en el mismo orden.
/// La cuantización a ocho bits puede mover una similitud en la tercera cifra, así
/// que lo que se compara es el orden.
#[test]
fn regenerado_en_otro_motor_encuentra_lo_mismo() {
    let (db, ediciones) = base_con_dos_ediciones();
    let en_la_base = EnLaBase::new(db.clone());
    let en_memoria = EnMemoria::default();
    regenerar(&en_la_base, &db, &ediciones, MODELO);
    regenerar(&en_memoria, &db, &ediciones, MODELO);

    for consulta in [
        "aldea de barro y cañabrava",
        "cosas sin nombre señalar con el dedo",
        "tarde remota",
    ] {
        let v = vector_de(consulta);
        // En una sola edición: las dos tienen el mismo texto, así que en las dos a
        // la vez cada fragmento empata con su gemelo y el orden entre ellos es de
        // cada motor, no de la consulta.
        let alcance = Alcance::Ediciones(vec![ediciones[0].clone()]);
        let base = en_la_base.buscar(MODELO, &v, &alcance, 100).expect("base");
        let memoria = en_memoria
            .buscar(MODELO, &v, &alcance, 100)
            .expect("memoria");
        assert_eq!(base.len(), memoria.len());

        // La similitud de cada fragmento es la misma salvo la cuantización.
        for cercano in &base {
            let exacto = memoria
                .iter()
                .find(|m| m.chunk_id == cercano.chunk_id)
                .expect("el mismo fragmento");
            assert!((cercano.similitud - exacto.similitud).abs() < 0.01);
        }
        // Y el orden es el mismo allí donde la consulta distingue: dos fragmentos
        // que empatan dentro del error de la cuantización pueden cambiar de sitio
        // sin que eso diga nada del motor.
        let mut comprobadas = 0;
        for i in 0..memoria.len() {
            let separado_de = |j: Option<usize>| {
                j.and_then(|j| memoria.get(j))
                    .is_none_or(|otro| (otro.similitud - memoria[i].similitud).abs() > 0.02)
            };
            if separado_de(i.checked_sub(1)) && separado_de(Some(i + 1)) {
                assert_eq!(
                    base[i].chunk_id, memoria[i].chunk_id,
                    "los dos motores no coinciden en la posición {i} para «{consulta}»"
                );
                comprobadas += 1;
            }
        }
        assert!(
            comprobadas > 0,
            "la consulta «{consulta}» no distinguía ningún fragmento"
        );
    }
}

/// **Borrar el índice completo no pierde información.** Lo que hay en la base
/// además de los vectores —los fragmentos, el índice léxico, las anotaciones y
/// las colecciones— queda igual, y regenerar da los mismos resultados.
#[test]
fn vaciar_el_indice_no_pierde_nada_y_regenerarlo_da_lo_mismo() {
    let (db, ediciones) = base_con_dos_ediciones();
    let conn = db.connection().expect("conexión");
    conn.execute_batch(&format!(
        "INSERT INTO collections (id, name, kind) VALUES ('col', 'Lecturas', 'manual');
         INSERT INTO collection_items (collection_id, edition_id) VALUES ('col', '{}');",
        ediciones[0]
    ))
    .expect("colección");
    drop(conn);

    let foto = |db: &Database| {
        let conn = db.connection().expect("conexión");
        let cuenta = |tabla: &str| -> i64 {
            conn.query_row(&format!("SELECT count(*) FROM {tabla}"), [], |f| f.get(0))
                .expect("cuenta")
        };
        let lexico: Vec<String> = search::search_corpus(&conn, "Macondo", 10, 0)
            .expect("léxica")
            .into_iter()
            .map(|h| format!("{}|{}", h.edition_id, h.locator.href))
            .collect();
        (
            cuenta("text_chunks"),
            cuenta("collection_items"),
            cuenta("editions"),
            lexico,
        )
    };

    let indice = EnLaBase::new(db.clone());
    regenerar(&indice, &db, &ediciones, MODELO);
    let v = vector_de("aguas diáfanas del río");
    let antes = indice.buscar(MODELO, &v, &Alcance::Todo, 5).expect("antes");
    let base_antes = foto(&db);
    assert!(
        !base_antes.3.is_empty(),
        "la búsqueda léxica no encontraba nada ya antes"
    );

    assert!(indice.vaciar().expect("vaciar") > 0);

    assert_eq!(
        foto(&db),
        base_antes,
        "vaciar el índice vectorial tocó otra cosa"
    );
    regenerar(&indice, &db, &ediciones, MODELO);
    assert_eq!(
        indice
            .buscar(MODELO, &v, &Alcance::Todo, 5)
            .expect("después"),
        antes
    );
}

/// **Un vector no sobrevive a su fragmento.** Reindexar una edición borra y rehace
/// sus fragmentos, y con ellos se van sus vectores, en la misma transacción. Sin
/// esto, una búsqueda devolvería fragmentos que ya no existen.
#[test]
fn reindexar_una_edicion_se_lleva_sus_vectores() {
    let (db, ediciones) = base_con_dos_ediciones();
    let indice = EnLaBase::new(db.clone());
    regenerar(&indice, &db, &ediciones, MODELO);

    let documento = fake::con_tres_capitulos();
    search::index_edition_con(
        &mut db.connection().expect("conexión"),
        &ediciones[0],
        &documento,
        60,
    )
    .expect("reindexar");

    assert!(indice
        .con_vector(MODELO, &ediciones[0])
        .expect("reindexada")
        .is_empty());
    assert!(!indice
        .con_vector(MODELO, &ediciones[1])
        .expect("la otra")
        .is_empty());

    // Y guardar un vector de un fragmento que ya no existe no guarda nada.
    indice
        .guardar(
            MODELO,
            &ediciones[0],
            &[("no-existe".to_owned(), vector_de("hola"))],
        )
        .expect("guardar");
    assert!(indice
        .con_vector(MODELO, &ediciones[0])
        .expect("sin huérfanos")
        .is_empty());
}

/// Si el documento se troceó de otra forma que la guardada, emparejar **no
/// adivina**: dice que no coinciden, y quien genera reindexa.
#[test]
fn con_otro_troceado_no_se_empareja_nada() {
    let (db, ediciones) = base_con_dos_ediciones();
    let guardados = indice::fragmentos_de(&db.connection().expect("conexión"), &ediciones[0])
        .expect("guardados");
    let documento = fake::con_tres_capitulos();

    assert!(indice::alinear(&documento.text_chunks(60).expect("60"), &guardados).is_some());
    assert!(indice::alinear(&documento.text_chunks(90).expect("90"), &guardados).is_none());

    // Y con los mismos fragmentos salvo uno que empieza en otro sitio: el mismo
    // número no basta, tiene que coincidir cada uno.
    let mut movido = documento.text_chunks(60).expect("60");
    let ultimo = movido.len() - 1;
    movido[ultimo].0 = movido[0].0.clone();
    assert!(indice::alinear(&movido, &guardados).is_none());
}

/// Lo que ocupa un vector en la base, **medido**: es la cifra con la que el presupuesto
/// dirá cuánto disco cuesta preparar un documento antes de hacerlo.
#[test]
fn lo_que_ocupa_un_vector_en_la_base_se_mide() {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-vectores-tamano-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio");
    let db = Database::open(&dir.join("medida.sqlite")).expect("base");
    let id = edicion(&db, "Medida");

    // Muchos fragmentos: un documento de secciones largas troceado fino.
    let mut conn = db.connection().expect("conexión");
    let secciones = (0..40)
        .map(|i| nb_books_lib::reader::secciones::Seccion {
            href: format!("c{i}.xhtml"),
            title: format!("Capítulo {i}"),
            text: "Una frase de relleno con palabras. ".repeat(400),
        })
        .collect();
    let documento = nb_books_lib::reader::secciones::DocumentoDeSecciones::new(
        fake::MEDIA_TYPE,
        secciones,
        fake::con_tres_capitulos().capabilities(),
    );
    search::index_edition_con(&mut conn, &id, &documento, 200).expect("indexar");
    let paginas = |conn: &rusqlite::Connection| -> i64 {
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .expect("checkpoint");
        conn.query_row("PRAGMA page_count", [], |f| f.get(0))
            .expect("páginas")
    };
    let tamano_pagina: i64 = conn
        .query_row("PRAGMA page_size", [], |f| f.get(0))
        .expect("página");
    let antes = paginas(&conn);
    drop(conn);

    let guardados =
        indice::fragmentos_de(&db.connection().expect("conexión"), &id).expect("guardados");
    let vectores: Vec<(String, Embedding)> = guardados
        .iter()
        .enumerate()
        .map(|(i, (chunk, _))| {
            (
                chunk.clone(),
                vector_de(&format!("texto {i} distinto {}", i * 7)),
            )
        })
        .collect();
    EnLaBase::new(db.clone())
        .guardar(MODELO, &id, &vectores)
        .expect("guardar");

    let despues = paginas(&db.connection().expect("conexión"));
    let bytes = (despues - antes) * tamano_pagina;
    #[allow(clippy::cast_precision_loss)]
    let por_vector = bytes as f64 / vectores.len() as f64;
    eprintln!(
        "{} vectores de {DIMENSION} dimensiones: {bytes} B, {por_vector:.0} B por vector",
        vectores.len()
    );
    // 384 del vector y el resto de la fila y los dos índices: si pasa de mil, algo
    // se está guardando que no debería.
    assert!(por_vector < 1_000.0, "{por_vector:.0} B por vector");
    let _ = std::fs::remove_dir_all(&dir);
}
