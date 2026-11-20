//! Colecciones: una colección por regla se actualiza al importar un documento
//! que la cumple, y una manual guarda exactamente lo que el usuario puso.

use std::path::{Path, PathBuf};

use nb_books_lib::collections::{
    self, Collection, CollectionKind, CollectionOrigin, Condition, Rule,
};
use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, StorageMode, Work};
use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::EventBus;
use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Escenario {
    dir: PathBuf,
    scope: PathBuf,
    library: PathBuf,
    db: Database,
    bus: EventBus,
}

impl Escenario {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-colec-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = dir.join("origen");
        let library = dir.join("biblioteca");
        std::fs::create_dir_all(&scope).expect("ámbito");
        std::fs::create_dir_all(&library).expect("biblioteca");
        Self {
            dir,
            scope,
            library,
            db: Database::open_in_memory().expect("base"),
            bus: EventBus::new(),
        }
    }

    /// Importa un documento del corpus con el nombre indicado.
    fn importar(&self, fixture: &str, como: &str) -> String {
        let ruta = self.scope.join(como);
        std::fs::copy(fixtures().join(fixture), &ruta).expect("copia");

        let mut conn = self.db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: ruta,
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importado");

        let ImportOutcome::Imported(importado) = resultado else {
            panic!("debía importarse");
        };
        importado.edition.id
    }

    /// Como [`Self::importar`], pero con contenido propio: añade un comentario
    /// después del `%%EOF`, de modo que el archivo sigue siendo un PDF completo
    /// y su identidad por contenido es distinta.
    fn importar_distinto(&self, fixture: &str, como: &str) -> String {
        let ruta = self.scope.join(como);
        let mut contenido = std::fs::read(fixtures().join(fixture)).expect("plantilla");
        contenido.extend_from_slice(format!("\n% {como}\n%%EOF\n").as_bytes());
        std::fs::write(&ruta, contenido).expect("documento propio");

        let mut conn = self.db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &self.scope,
                library: &self.library,
                bus: &self.bus,
            },
            &ImportRequest {
                source: ruta,
                storage_mode: StorageMode::Referenced,
                root: None,
            },
        )
        .expect("importado");

        let ImportOutcome::Imported(importado) = resultado else {
            panic!("debía importarse");
        };
        importado.edition.id
    }

    /// Crea una edición suelta, sin archivo, para probar reglas de metadatos.
    fn edicion(&self, titulo: &str, idioma: Option<&str>, editorial: Option<&str>) -> String {
        let conn = self.db.connection().expect("conexión");
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
                language: idioma.map(str::to_owned),
                publisher: editorial.map(str::to_owned),
                ..NewEdition::default()
            },
        )
        .expect("edición")
        .id
    }
}

impl Drop for Escenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// **El criterio de aceptación.**
///
/// La colección no se "actualiza" en el sentido de que algo la recalcule: sus
/// miembros son los que cumplen la regla en el momento de preguntar, así que un
/// documento que entra y la cumple ya está dentro. Sin trabajo en segundo plano
/// y sin posibilidad de quedarse obsoleta.
#[test]
fn una_coleccion_por_regla_se_actualiza_al_importar_un_documento_que_la_cumple() {
    let esc = Escenario::nuevo("por-regla");
    let conn = esc.db.connection().expect("conexión");

    let coleccion = Collection::create_rule(
        &conn,
        "Todos mis PDF",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::MediaType("application/pdf".to_owned())]).expect("regla"),
    )
    .expect("colección");

    // Recién creada, está vacía.
    assert!(Collection::members(&conn, &coleccion.id)
        .expect("miembros")
        .is_empty());

    // Entra un EPUB: no la cumple.
    esc.importar("minimo-valido.epub", "un-epub.epub");
    assert!(
        Collection::members(&conn, &coleccion.id)
            .expect("miembros")
            .is_empty(),
        "un EPUB no pertenece a la colección de los PDF"
    );

    // Entra un PDF: la cumple, y ya está dentro.
    let pdf = esc.importar("minimo-valido.pdf", "un-pdf.pdf");
    assert_eq!(
        Collection::members(&conn, &coleccion.id).expect("miembros"),
        vec![pdf.clone()],
        "el PDF recién importado debe pertenecer a la colección"
    );

    // Y otro más, sin tocar nada. Un PDF disfrazado de EPUB, con contenido
    // propio: `extension-falsificada.epub` del corpus es byte a byte el mismo
    // archivo que `minimo-valido.pdf`, así que usarlo tal cual daría un
    // duplicado y no un segundo documento.
    let otro = esc.importar_distinto("minimo-valido.pdf", "otro-pdf-disfrazado.epub");
    let miembros = Collection::members(&conn, &coleccion.id).expect("miembros");
    assert_eq!(miembros.len(), 2);
    assert!(miembros.contains(&pdf));
    assert!(
        miembros.contains(&otro),
        "el formato lo decide el contenido, también aquí"
    );
}

/// Las condiciones se cumplen todas, no cualquiera.
#[test]
fn una_regla_con_varias_condiciones_las_exige_todas() {
    let esc = Escenario::nuevo("varias");
    let conn = esc.db.connection().expect("conexión");

    let en_espanol_de_anagrama = esc.edicion("Uno", Some("es"), Some("Anagrama"));
    esc.edicion("Dos", Some("es"), Some("Alfaguara"));
    esc.edicion("Tres", Some("en"), Some("Anagrama"));

    let coleccion = Collection::create_rule(
        &conn,
        "Anagrama en español",
        CollectionOrigin::Personal,
        &Rule::new(vec![
            Condition::Language("es".to_owned()),
            Condition::Publisher("Anagrama".to_owned()),
        ])
        .expect("regla"),
    )
    .expect("colección");

    assert_eq!(
        Collection::members(&conn, &coleccion.id).expect("miembros"),
        vec![en_espanol_de_anagrama]
    );
}

/// El título se busca sin distinguir mayúsculas, y sin que los comodines de
/// `LIKE` se cuelen: un título con un guion bajo tiene que encontrarse.
#[test]
fn la_condicion_de_titulo_no_distingue_mayusculas_ni_interpreta_comodines() {
    let esc = Escenario::nuevo("titulo");
    let conn = esc.db.connection().expect("conexión");

    let con_guion = esc.edicion("Manual_de_estilo", None, None);
    esc.edicion("Manual de estilo", None, None);
    esc.edicion("MANUAL DE HISTORIA", None, None);

    let por_mayusculas = Collection::create_rule(
        &conn,
        "Manuales",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::TitleContains("manual".to_owned())]).expect("regla"),
    )
    .expect("colección");
    assert_eq!(
        Collection::members(&conn, &por_mayusculas.id)
            .expect("miembros")
            .len(),
        3,
        "la búsqueda de título no puede distinguir mayúsculas"
    );

    let con_comodin = Collection::create_rule(
        &conn,
        "Con guion bajo",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::TitleContains("Manual_de".to_owned())]).expect("regla"),
    )
    .expect("colección");
    assert_eq!(
        Collection::members(&conn, &con_comodin.id).expect("miembros"),
        vec![con_guion],
        "el guion bajo es un carácter, no un comodín"
    );
}

/// Una regla vacía se cumpliría siempre. Una colección que contiene la
/// biblioteca entera sin que nadie lo haya pedido es peor que una vacía, porque
/// parece que funciona.
#[test]
fn una_regla_sin_condiciones_se_rechaza() {
    assert_eq!(Rule::new(Vec::new()), Err(NbError::Internal));
}

// --- Colecciones manuales --------------------------------------------------

#[test]
fn una_coleccion_manual_conserva_lo_que_se_le_pone_y_en_su_orden() {
    let esc = Escenario::nuevo("manual");
    let conn = esc.db.connection().expect("conexión");

    let uno = esc.edicion("Primero", None, None);
    let dos = esc.edicion("Segundo", None, None);
    let tres = esc.edicion("Tercero", None, None);

    let coleccion = Collection::create_manual(&conn, "Para el examen", CollectionOrigin::Personal)
        .expect("colección");

    // El orden de una bibliografía importa, así que se conserva el de inserción
    // y no el alfabético ni el de creación.
    Collection::add(&conn, &coleccion.id, &tres).expect("añadido");
    Collection::add(&conn, &coleccion.id, &uno).expect("añadido");
    Collection::add(&conn, &coleccion.id, &dos).expect("añadido");
    // Repetir no duplica.
    Collection::add(&conn, &coleccion.id, &uno).expect("repetido");

    assert_eq!(
        Collection::members(&conn, &coleccion.id).expect("miembros"),
        vec![tres.clone(), uno.clone(), dos]
    );

    Collection::remove(&conn, &coleccion.id, &uno).expect("quitado");
    assert!(!Collection::members(&conn, &coleccion.id)
        .expect("miembros")
        .contains(&uno));
}

/// Los miembros de una colección por regla los decide el criterio. Poder
/// añadirlos a mano daría dos fuentes de verdad sobre quién pertenece.
#[test]
fn a_una_coleccion_por_regla_no_se_le_puede_anadir_a_mano() {
    let esc = Escenario::nuevo("mixta");
    let conn = esc.db.connection().expect("conexión");
    let edicion = esc.edicion("Cualquiera", Some("es"), None);

    let coleccion = Collection::create_rule(
        &conn,
        "En español",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::Language("es".to_owned())]).expect("regla"),
    )
    .expect("colección");

    assert_eq!(
        Collection::add(&conn, &coleccion.id, &edicion),
        Err(NbError::PathNotAllowed)
    );
}

// --- Etiquetas --------------------------------------------------------------

#[test]
fn las_etiquetas_se_ponen_se_quitan_y_se_comparten_entre_documentos() {
    let esc = Escenario::nuevo("etiquetas");
    let conn = esc.db.connection().expect("conexión");
    let uno = esc.edicion("Uno", None, None);
    let dos = esc.edicion("Dos", None, None);

    collections::tag(&conn, &uno, "para el examen").expect("etiqueta");
    collections::tag(&conn, &uno, "releer").expect("etiqueta");
    collections::tag(&conn, &dos, "para el examen").expect("etiqueta");

    assert_eq!(
        collections::tags_of(&conn, &uno).expect("etiquetas"),
        vec!["para el examen".to_owned(), "releer".to_owned()]
    );

    collections::untag(&conn, &uno, "releer").expect("quitada");
    assert_eq!(
        collections::tags_of(&conn, &uno).expect("etiquetas"),
        vec!["para el examen".to_owned()]
    );

    // Quitarla de uno no la quita del otro, ni la borra del vocabulario.
    assert_eq!(
        collections::tags_of(&conn, &dos).expect("etiquetas"),
        vec!["para el examen".to_owned()]
    );
}

/// Y una colección por regla puede basarse en una etiqueta, que es lo que hace
/// útiles a las dos cosas juntas.
#[test]
fn una_coleccion_por_regla_puede_seguir_una_etiqueta() {
    let esc = Escenario::nuevo("etiqueta-regla");
    let conn = esc.db.connection().expect("conexión");

    let coleccion = Collection::create_rule(
        &conn,
        "Pendientes de leer",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::Subject {
            scheme: collections::ESQUEMA_ETIQUETA.to_owned(),
            label: "pendiente".to_owned(),
        }])
        .expect("regla"),
    )
    .expect("colección");

    let edicion = esc.edicion("Un libro", None, None);
    assert!(Collection::members(&conn, &coleccion.id)
        .expect("miembros")
        .is_empty());

    collections::tag(&conn, &edicion, "pendiente").expect("etiqueta");

    assert_eq!(
        Collection::members(&conn, &coleccion.id).expect("miembros"),
        vec![edicion.clone()],
        "etiquetar debe meterlo en la colección sin tocarla"
    );

    collections::untag(&conn, &edicion, "pendiente").expect("quitada");
    assert!(
        Collection::members(&conn, &coleccion.id)
            .expect("miembros")
            .is_empty(),
        "y desetiquetar debe sacarlo"
    );
}

/// Una materia y una etiqueta se guardan igual y significan cosas distintas: la
/// primera dice de qué trata la obra, la segunda es el nombre que le pone su
/// dueño. No pueden mezclarse en la interfaz.
#[test]
fn las_etiquetas_no_se_mezclan_con_las_materias() {
    use nb_books_lib::db::catalog::Subject;

    let esc = Escenario::nuevo("materias");
    let conn = esc.db.connection().expect("conexión");
    let edicion = esc.edicion("Un libro", None, None);

    let materia = Subject::ensure(&conn, "local", "Realismo mágico").expect("materia");
    Subject::link(&conn, &edicion, &materia.id).expect("enlazada");
    collections::tag(&conn, &edicion, "releer").expect("etiqueta");

    assert_eq!(
        collections::tags_of(&conn, &edicion).expect("etiquetas"),
        vec!["releer".to_owned()]
    );
    assert_eq!(
        Subject::list_for_edition(&conn, &edicion)
            .expect("materias")
            .len(),
        2,
        "las dos se guardan en el mismo sitio, con esquemas distintos"
    );
}

#[test]
fn borrar_una_coleccion_no_borra_los_documentos() {
    let esc = Escenario::nuevo("borrado");
    let conn = esc.db.connection().expect("conexión");
    let edicion = esc.edicion("Un libro", None, None);
    let coleccion = Collection::create_manual(&conn, "Temporal", CollectionOrigin::Personal)
        .expect("colección");
    Collection::add(&conn, &coleccion.id, &edicion).expect("añadido");

    conn.execute("DELETE FROM collections WHERE id = ?1", [&coleccion.id])
        .expect("borrado");

    assert!(Edition::list_for_work(
        &conn,
        &conn
            .query_row(
                "SELECT work_id FROM editions WHERE id = ?1",
                [&edicion],
                |r| r.get::<_, String>(0)
            )
            .expect("obra")
    )
    .expect("ediciones")
    .iter()
    .any(|e| e.id == edicion));

    let huerfanos: i64 = conn
        .query_row("SELECT count(*) FROM collection_items", [], |r| r.get(0))
        .expect("consulta");
    assert_eq!(huerfanos, 0);
}

#[test]
fn una_coleccion_por_regla_declara_su_criterio_al_releerla() {
    let esc = Escenario::nuevo("relectura");
    let conn = esc.db.connection().expect("conexión");
    let regla = Rule::new(vec![
        Condition::Language("pt".to_owned()),
        Condition::MediaType("application/epub+zip".to_owned()),
    ])
    .expect("regla");

    let creada = Collection::create_rule(
        &conn,
        "EPUB en portugués",
        CollectionOrigin::Personal,
        &regla,
    )
    .expect("colección");
    let releida = Collection::find(&conn, &creada.id)
        .expect("consulta")
        .expect("existe");

    assert_eq!(releida.kind, CollectionKind::Rule);
    assert_eq!(releida.rule, Some(regla));
    assert_eq!(releida, creada);
}

// --- Lo que usa la interfaz ----------------------------------------

#[test]
fn renombrar_una_coleccion_personal_conserva_sus_miembros() {
    let esc = Escenario::nuevo("renombrar");
    let conn = esc.db.connection().expect("conexión");
    let edicion = esc.edicion("Un libro", None, None);
    let coleccion =
        Collection::create_manual(&conn, "Tesis", CollectionOrigin::Personal).expect("colección");
    Collection::add(&conn, &coleccion.id, &edicion).expect("añadido");

    let renombrada =
        Collection::rename(&conn, &coleccion.id, "  Para la Tesis  ").expect("renombrada");

    // Sin los espacios de los lados: "Tesis" y "Tesis " serían dos colecciones
    // que en la lista no se distinguen.
    assert_eq!(renombrada.name, "Para la Tesis");
    assert_eq!(renombrada.id, coleccion.id);
    assert_eq!(
        Collection::members(&conn, &coleccion.id).expect("miembros"),
        vec![edicion]
    );
}

#[test]
fn renombrar_con_un_nombre_vacio_o_repetido_no_cambia_nada() {
    let esc = Escenario::nuevo("renombrar-mal");
    let conn = esc.db.connection().expect("conexión");
    Collection::create_manual(&conn, "Relecturas", CollectionOrigin::Personal).expect("una");
    let otra =
        Collection::create_manual(&conn, "Poesía", CollectionOrigin::Personal).expect("otra");

    assert!(Collection::rename(&conn, &otra.id, "   ").is_err());
    assert!(Collection::rename(&conn, &otra.id, "Relecturas").is_err());
    assert_eq!(
        Collection::find(&conn, &otra.id)
            .expect("consulta")
            .expect("existe")
            .name,
        "Poesía"
    );
}

#[test]
fn borrar_una_coleccion_dice_si_existia_y_deja_los_documentos() {
    let esc = Escenario::nuevo("borrar");
    let conn = esc.db.connection().expect("conexión");
    let edicion = esc.edicion("Un libro", None, None);
    let coleccion = Collection::create_manual(&conn, "Temporal", CollectionOrigin::Personal)
        .expect("colección");
    Collection::add(&conn, &coleccion.id, &edicion).expect("añadido");

    assert_eq!(Collection::delete(&conn, &coleccion.id), Ok(true));
    assert_eq!(Collection::delete(&conn, &coleccion.id), Ok(false));

    let ediciones: i64 = conn
        .query_row(
            "SELECT count(*) FROM editions WHERE id = ?1",
            [&edicion],
            |r| r.get(0),
        )
        .expect("consulta");
    assert_eq!(
        ediciones, 1,
        "borrar la agrupación no puede llevarse el libro"
    );
}

#[test]
fn anadir_en_lote_cuenta_solo_las_que_entran_nuevas() {
    let esc = Escenario::nuevo("lote");
    let mut conn = esc.db.connection().expect("conexión");
    let uno = esc.edicion("Uno", None, None);
    let dos = esc.edicion("Dos", None, None);
    let tres = esc.edicion("Tres", None, None);
    let coleccion =
        Collection::create_manual(&conn, "Lote", CollectionOrigin::Personal).expect("colección");

    assert_eq!(
        Collection::add_many(&mut conn, &coleccion.id, &[uno.clone(), dos.clone()]),
        Ok(2)
    );
    assert_eq!(
        Collection::add_many(&mut conn, &coleccion.id, &[dos.clone(), tres.clone()]),
        Ok(1),
        "la que ya estaba no puede contarse como añadida"
    );
    assert_eq!(
        Collection::members(&conn, &coleccion.id).expect("miembros"),
        vec![uno, dos, tres]
    );
}

/// Una selección que entra a medias deja al usuario sin saber cuáles faltan.
#[test]
fn anadir_en_lote_es_todo_o_nada() {
    let esc = Escenario::nuevo("lote-atomico");
    let mut conn = esc.db.connection().expect("conexión");
    let uno = esc.edicion("Uno", None, None);
    let dos = esc.edicion("Dos", None, None);
    let coleccion =
        Collection::create_manual(&conn, "Atómica", CollectionOrigin::Personal).expect("colección");

    let resultado = Collection::add_many(
        &mut conn,
        &coleccion.id,
        &[uno, "no-existe".to_owned(), dos],
    );

    assert!(
        resultado.is_err(),
        "una edición que no existe tiene que hacer fallar el lote"
    );
    assert!(
        Collection::members(&conn, &coleccion.id)
            .expect("miembros")
            .is_empty(),
        "y no puede quedar ninguna de las que iban antes"
    );
}

#[test]
fn la_ficha_sabe_en_que_colecciones_manuales_esta_una_edicion() {
    let esc = Escenario::nuevo("pertenencia");
    let conn = esc.db.connection().expect("conexión");
    let edicion = esc.edicion("Cien años", Some("es"), None);
    let otra = esc.edicion("Otra", Some("es"), None);

    let zeta = Collection::create_manual(&conn, "zeta", CollectionOrigin::Personal).expect("zeta");
    let alfa = Collection::create_manual(&conn, "Alfa", CollectionOrigin::Personal).expect("alfa");
    let ajena =
        Collection::create_manual(&conn, "Ajena", CollectionOrigin::Personal).expect("ajena");
    Collection::add(&conn, &zeta.id, &edicion).expect("añadido");
    Collection::add(&conn, &alfa.id, &edicion).expect("añadido");
    Collection::add(&conn, &ajena.id, &otra).expect("añadido");
    // La cumple, pero la pertenencia a una por regla no se pone ni se quita.
    Collection::create_rule(
        &conn,
        "En español",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::Language("es".to_owned())]).expect("regla"),
    )
    .expect("por regla");

    assert_eq!(
        Collection::containing(&conn, &edicion).expect("pertenencia"),
        vec![alfa.id, zeta.id],
        "solo las manuales, y por nombre sin distinguir mayúsculas"
    );
}

#[test]
fn el_resumen_cuenta_miembros_y_ordena_por_nombre() {
    let esc = Escenario::nuevo("resumen");
    let conn = esc.db.connection().expect("conexión");
    let uno = esc.edicion("Uno", Some("es"), None);
    let dos = esc.edicion("Dos", Some("es"), None);
    esc.edicion("Tres", Some("en"), None);

    let tesis =
        Collection::create_manual(&conn, "tesis", CollectionOrigin::Personal).expect("tesis");
    Collection::add(&conn, &tesis.id, &uno).expect("añadido");
    Collection::add(&conn, &tesis.id, &dos).expect("añadido");
    Collection::create_rule(
        &conn,
        "En español",
        CollectionOrigin::Personal,
        &Rule::new(vec![Condition::Language("es".to_owned())]).expect("regla"),
    )
    .expect("por regla");

    let resumen = Collection::summaries(&conn).expect("resumen");
    let vistos: Vec<(&str, u32)> = resumen
        .iter()
        .map(|c| (c.name.as_str(), c.members))
        .collect();

    assert_eq!(
        vistos,
        vec![("En español", 2), ("tesis", 2)],
        "por nombre; la por regla cuenta las que la cumplen ahora"
    );
}
