//! Criterio de aceptación de las notas enlazadas.
//!
//! "Enlazar A con B hace aparecer A en los retroenlaces de B sin ninguna
//! escritura adicional; borrar A no deja un retroenlace colgando; la exportación
//! y la reimportación conservan el grafo entero."
//!
//! Las notas existen porque la investigación de
//! `docs/design/reading-experience/README.md` dice que resaltar por sí solo
//! no mejora el aprendizaje y que quien lee en digital casi no subraya. Lo que
//! funciona es escribir algo propio, y hasta ahora no había dónde.

use std::path::PathBuf;

use nb_books_lib::annotations;
use nb_books_lib::db::catalog::{
    Annotation, AnnotationKind, Edition, NewAnnotation, NewEdition, NewWork, Work,
};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::notes;
use nb_books_lib::portability;

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-notas-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }
    fn base(&self, nombre: &str) -> PathBuf {
        self.0.join(format!("{nombre}.sqlite"))
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Una edición con la que anclar anotaciones de prueba.
fn edicion_de_prueba(conn: &rusqlite::Connection, titulo: &str) -> Edition {
    let obra = Work::create(
        conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            ..NewWork::default()
        },
    )
    .expect("obra");

    Edition::create(
        conn,
        &NewEdition {
            work_id: obra.id.clone(),
            title: titulo.to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición")
}

/// Una anotación anclada a esa edición.
fn anotacion_de_prueba(conn: &rusqlite::Connection, edition_id: &str) -> Annotation {
    Annotation::create(
        conn,
        &NewAnnotation {
            payload: None,
            edition_id: edition_id.to_owned(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: r#"{"version":1,"href":"c1.xhtml","mediaType":"application/xhtml+xml",
                        "locations":{"progression":0.1},"text":{"highlight":"algo"}}"#
                .to_owned(),
        },
    )
    .expect("anotación")
}

fn titulos(notas: &[notes::Note]) -> Vec<&str> {
    notas.iter().map(|n| n.title.as_str()).collect()
}

/// **El criterio, primera mitad:** el retroenlace no se escribe, se consulta.
#[test]
fn enlazar_produce_el_retroenlace_sin_ninguna_escritura_adicional() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let a = notes::create(&conn, "Sobre la costumbre", "").expect("nota A");
    let b = notes::create(&conn, "Hume y la causalidad", "").expect("nota B");

    assert!(notes::backlinks(&conn, &b.id)
        .expect("retroenlaces")
        .is_empty());

    // Una sola escritura.
    notes::link(&conn, &a.id, &b.id).expect("enlace");

    assert_eq!(
        titulos(&notes::backlinks(&conn, &b.id).expect("retroenlaces")),
        vec!["Sobre la costumbre"],
        "el retroenlace debe aparecer sin haberlo escrito"
    );
    assert_eq!(
        titulos(&notes::outgoing(&conn, &a.id).expect("salientes")),
        vec!["Hume y la causalidad"]
    );

    // Y no aparece al revés: un enlace tiene dirección.
    assert!(notes::backlinks(&conn, &a.id)
        .expect("retroenlaces")
        .is_empty());
}

/// **El criterio, segunda mitad:** borrar no deja nada colgando.
#[test]
fn borrar_una_nota_no_deja_ningun_retroenlace_colgando() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let a = notes::create(&conn, "A", "").expect("A");
    let b = notes::create(&conn, "B", "").expect("B");
    let c = notes::create(&conn, "C", "").expect("C");

    notes::link(&conn, &a.id, &b.id).expect("A -> B");
    notes::link(&conn, &c.id, &b.id).expect("C -> B");
    notes::link(&conn, &b.id, &c.id).expect("B -> C");

    notes::delete(&conn, &a.id).expect("borrada");

    // B pierde el retroenlace de A y conserva el de C.
    assert_eq!(
        titulos(&notes::backlinks(&conn, &b.id).expect("retroenlaces")),
        vec!["C"]
    );

    // Y no queda ninguna fila apuntando a una nota que ya no existe.
    let huerfanos: i64 = conn
        .query_row(
            "SELECT count(*) FROM note_links
              WHERE source_id NOT IN (SELECT id FROM notes)
                 OR target_id NOT IN (SELECT id FROM notes)",
            [],
            |r| r.get(0),
        )
        .expect("consulta");
    assert_eq!(huerfanos, 0, "quedaron enlaces apuntando al vacío");
}

/// Una nota se edita; una anotación no. Son dos cosas distintas y por eso la
/// respuesta es distinta: una anotación ancla a un texto que puede moverse, una
/// nota es un escrito propio.
#[test]
fn una_nota_se_reescribe_y_conserva_sus_enlaces() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let a = notes::create(&conn, "Borrador", "primera versión con [[Destino]]").expect("A");
    notes::create(&conn, "Destino", "").expect("B");
    // El destino se escribió después que la mención, así que el enlace lo tuvo
    // que resolver la creación de la nota destino, no la de la nota origen.
    let b = notes::find_by_title(&conn, "Destino")
        .expect("consulta")
        .expect("existe");

    let editada = notes::update(
        &conn,
        &a.id,
        "Título corregido",
        "segunda versión, con [[Destino]] todavía",
    )
    .expect("reescritura");

    assert_eq!(editada.id, a.id, "reescribir no crea una nota nueva");
    assert_eq!(editada.body, "segunda versión, con [[Destino]] todavía");
    assert_eq!(
        titulos(&notes::backlinks(&conn, &b.id).expect("retroenlaces")),
        vec!["Título corregido"],
        "el enlace sobrevive a la reescritura y refleja el título nuevo"
    );
}

/// El cuerpo es la única declaración de un enlace: se escribe `[[Título]]` y el
/// enlace existe, se borra la mención y deja de existir. Sin esta simetría el
/// retroenlace seguiría enseñando una relación que el usuario ya quitó del
/// texto, y no habría forma de deshacerla desde la interfaz.
#[test]
fn quitar_la_mencion_del_cuerpo_deshace_el_enlace() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let destino = notes::create(&conn, "Destino", "").expect("destino");
    let origen = notes::create(&conn, "Origen", "hablo de [[Destino]]").expect("origen");

    assert_eq!(
        titulos(&notes::backlinks(&conn, &destino.id).expect("retroenlaces")),
        vec!["Origen"],
        "escribir la mención crea el enlace"
    );

    notes::update(&conn, &origen.id, "Origen", "ya no hablo de nada").expect("reescritura");

    assert!(
        notes::backlinks(&conn, &destino.id)
            .expect("retroenlaces")
            .is_empty(),
        "quitar la mención deshace el enlace"
    );
    assert!(
        notes::outgoing(&conn, &origen.id)
            .expect("salientes")
            .is_empty(),
        "y no queda la fila del otro lado"
    );
}

/// Se enlaza escribiendo el nombre de una nota que quizá aún no se ha escrito.
/// Si esa mención no se resolviera al crearla, el enlace se perdería en silencio
/// y no volvería nunca: exactamente la pérdida que esta fase existe para evitar.
#[test]
fn una_mencion_escrita_antes_que_su_destino_se_resuelve_al_crearlo() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let origen =
        notes::create(&conn, "Origen", "esto irá a [[Todavía no existe]]").expect("origen");
    assert!(
        notes::outgoing(&conn, &origen.id)
            .expect("salientes")
            .is_empty(),
        "no se enlaza a lo que no existe"
    );

    let destino = notes::create(&conn, "Todavía no existe", "ya existo").expect("destino");

    assert_eq!(
        titulos(&notes::backlinks(&conn, &destino.id).expect("retroenlaces")),
        vec!["Origen"],
        "al aparecer el destino, la mención que lo esperaba se enlaza"
    );
}

/// Renombrar una nota reengancha a quien la nombra por el nombre nuevo y suelta
/// a quien la nombraba por el viejo. Lo contrario dejaría un enlace apuntando a
/// una nota que ya no se llama así, y el retroenlace enseñaría una relación que
/// el texto no dice.
#[test]
fn renombrar_una_nota_rehace_los_enlaces_que_la_nombraban() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let destino = notes::create(&conn, "Nombre viejo", "").expect("destino");
    notes::create(&conn, "Por el viejo", "cito a [[Nombre viejo]]").expect("origen viejo");
    notes::create(&conn, "Por el nuevo", "cito a [[Nombre nuevo]]").expect("origen nuevo");

    assert_eq!(
        titulos(&notes::backlinks(&conn, &destino.id).expect("retroenlaces")),
        vec!["Por el viejo"],
    );

    notes::update(&conn, &destino.id, "Nombre nuevo", "").expect("renombrado");

    assert_eq!(
        titulos(&notes::backlinks(&conn, &destino.id).expect("retroenlaces")),
        vec!["Por el nuevo"],
        "el enlace lo tiene quien la nombra ahora, no quien la nombraba antes"
    );
}

/// Un corchete sin cerrar no es un enlace a medias: no es un enlace. Y una
/// mención repetida es un enlace, no dos, porque el enlace es una relación entre
/// dos notas y no una cuenta de cuántas veces se nombran.
#[test]
fn un_corchete_sin_cerrar_no_enlaza_y_una_mencion_repetida_no_duplica() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let destino = notes::create(&conn, "Destino", "").expect("destino");
    let origen = notes::create(
        &conn,
        "Origen",
        "abro [[Destino sin cerrar y luego [[Destino]] y otra vez [[ Destino ]]",
    )
    .expect("origen");

    assert_eq!(
        titulos(&notes::outgoing(&conn, &origen.id).expect("salientes")),
        vec!["Destino"],
        "un solo enlace, y ninguno al título trufado del corchete abierto"
    );
    assert_eq!(
        notes::backlinks(&conn, &destino.id)
            .expect("retroenlaces")
            .len(),
        1,
    );
}

/// El título es el nombre con el que se enlaza una nota, así que dos notas no
/// pueden compartirlo: haría ambiguo cualquier enlace escrito a mano.
#[test]
fn dos_notas_no_pueden_llamarse_igual() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    notes::create(&conn, "Repetida", "").expect("primera");
    let segunda = notes::create(&conn, "Repetida", "");

    assert!(matches!(segunda, Err(NbError::StorageFailure)));
}

/// Una nota enlazada a sí misma no aporta nada y ensucia todo retroenlace.
#[test]
fn una_nota_no_puede_enlazarse_consigo_misma() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let a = notes::create(&conn, "Sola", "").expect("A");
    assert!(matches!(
        notes::link(&conn, &a.id, &a.id),
        Err(NbError::StorageFailure)
    ));
}

/// El puente entre lo que se lee y lo que se piensa: una nota puede apuntar a
/// una anotación anclada, y desde la anotación se ve qué notas la citan.
#[test]
fn una_nota_enlaza_una_anotacion_y_la_anotacion_ve_quien_la_cita() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Tratado".to_owned(),
            ..NewWork::default()
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id.clone(),
            title: "Tratado".to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición");
    let anotacion = Annotation::create(
        &conn,
        &NewAnnotation {
            payload: None,
            edition_id: edicion.id.clone(),
            kind: AnnotationKind::Highlight,
            body: None,
            target: r#"{"version":1,"href":"c1.xhtml","mediaType":"application/xhtml+xml",
                        "locations":{"progression":0.1},"text":{"highlight":"algo"}}"#
                .to_owned(),
        },
    )
    .expect("anotación");

    let nota = notes::create(&conn, "Idea a partir de la cita", "").expect("nota");
    notes::link_annotation(&conn, &nota.id, &anotacion.id).expect("enlace");

    assert_eq!(
        titulos(&notes::notes_for_annotation(&conn, &anotacion.id).expect("notas")),
        vec!["Idea a partir de la cita"]
    );
    assert_eq!(
        notes::linked_annotations(&conn, &nota.id).expect("anotaciones"),
        vec![anotacion.id.clone()]
    );

    // Y la cita se enseña con **de qué pasaje habla**, no con su identificador:
    // una lista de identificadores no se puede leer, y es lo único que había.
    let citadas = annotations::for_note(&conn, &nota.id).expect("citadas");
    assert_eq!(citadas.len(), 1);
    assert_eq!(citadas[0].annotation.id, anotacion.id);
    assert_eq!(
        citadas[0].title, "Tratado",
        "dice de qué libro es el pasaje"
    );

    // Deshacerla la quita de los dos lados a la vez, porque solo hay una fila.
    notes::unlink_annotation(&conn, &nota.id, &anotacion.id).expect("deshacer");
    assert!(notes::notes_for_annotation(&conn, &anotacion.id)
        .expect("notas")
        .is_empty());
    assert!(annotations::for_note(&conn, &nota.id)
        .expect("citadas")
        .is_empty());
}

/// Una nota citada no se lleva por delante lo que la nota escribe.
///
/// Las dos clases de enlace de una nota viven en tablas distintas y con
/// mecanismos distintos: la que va a otra nota la declara el cuerpo con
/// `[[Título]]` y se rehace en cada guardado; la que va a una anotación la
/// declara un gesto y no depende del texto. Reescribir el cuerpo tiene que
/// rehacer la primera y **no tocar** la segunda, o citar un pasaje duraría hasta
/// la siguiente corrección de una errata.
#[test]
fn reescribir_una_nota_no_deshace_los_pasajes_que_cita() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    let edicion = edicion_de_prueba(&conn, "Tratado");
    let anotacion = anotacion_de_prueba(&conn, &edicion.id);

    notes::create(&conn, "Destino", "").expect("destino");
    let nota = notes::create(&conn, "Origen", "cito a [[Destino]]").expect("nota");
    notes::link_annotation(&conn, &nota.id, &anotacion.id).expect("enlace");

    notes::update(&conn, &nota.id, "Origen", "ya no cito a ninguna nota").expect("reescritura");

    assert!(
        notes::outgoing(&conn, &nota.id)
            .expect("salientes")
            .is_empty(),
        "quitar la mención sí deshace el enlace entre notas"
    );
    assert_eq!(
        annotations::for_note(&conn, &nota.id)
            .expect("citadas")
            .len(),
        1,
        "y el pasaje citado sigue ahí, porque no lo declaraba el cuerpo"
    );
}

/// El grafo entero sobrevive a la ida y vuelta. Es lo que convierte "el usuario
/// es dueño de sus datos" en algo comprobable también para lo que escribe él.
#[test]
fn la_exportacion_y_la_reimportacion_conservan_el_grafo_entero() {
    let tmp = Temporal::nuevo("ida-y-vuelta");
    let origen = Database::open(&tmp.base("origen")).expect("origen");

    let (esperado_salientes, esperado_retro) = {
        let conn = origen.connection().expect("conexión");
        let a = notes::create(&conn, "Punto de partida", "cuerpo A").expect("A");
        let b = notes::create(&conn, "Consecuencia", "cuerpo B").expect("B");
        let c = notes::create(&conn, "Objeción", "cuerpo C").expect("C");
        notes::link(&conn, &a.id, &b.id).expect("A -> B");
        notes::link(&conn, &a.id, &c.id).expect("A -> C");
        notes::link(&conn, &c.id, &b.id).expect("C -> B");

        (
            titulos(&notes::outgoing(&conn, &a.id).expect("salientes"))
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
            titulos(&notes::backlinks(&conn, &b.id).expect("retroenlaces"))
                .iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>(),
        )
    };

    let exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };
    assert_eq!(exportado.notes.len(), 3);
    assert_eq!(exportado.note_links.len(), 3);
    drop(origen);

    let destino = Database::open(&tmp.base("destino")).expect("destino");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &exportado).expect("importación");
    }

    let conn = destino.connection().expect("conexión");
    let a = notes::find_by_title(&conn, "Punto de partida")
        .expect("consulta")
        .expect("existe");
    let b = notes::find_by_title(&conn, "Consecuencia")
        .expect("consulta")
        .expect("existe");

    assert_eq!(
        titulos(&notes::outgoing(&conn, &a.id).expect("salientes")),
        esperado_salientes
    );
    assert_eq!(
        titulos(&notes::backlinks(&conn, &b.id).expect("retroenlaces")),
        esperado_retro
    );
}

/// Una base que solo tiene notas tampoco está vacía. Dejarla pasar acabaría en
/// un choque de títulos a mitad de la restauración en lugar de en un rechazo.
#[test]
fn importar_sobre_una_base_que_solo_tiene_notas_se_rechaza() {
    let db = Database::open_in_memory().expect("base");
    {
        let conn = db.connection().expect("conexión");
        notes::create(&conn, "Ya escrita", "").expect("nota");
    }

    let vacia = Database::open_in_memory().expect("vacía");
    let exportado = {
        let conn = vacia.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };

    let mut conn = db.connection().expect("conexión");
    assert!(matches!(
        portability::import(&mut conn, &exportado),
        Err(NbError::SchemaMismatch)
    ));
}
