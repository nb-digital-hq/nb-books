//! Criterio de aceptación del vocabulario.
//!
//! "La lista sobrevive a reiniciar, entra en la exportación y desde
//! cada entrada se salta al punto exacto donde apareció la palabra."
//!
//! Las tres mitades se comprueban aquí sobre una base real: la primera abriendo
//! el archivo dos veces, la segunda con una exportación y una importación
//! completas, y la tercera releyendo el locator y exigiendo que sea **el mismo
//! objeto**, no uno parecido.

use std::path::PathBuf;

use nb_books_lib::db::catalog::{Edition, NewEdition, NewWork, Work};
use nb_books_lib::db::Database;
use nb_books_lib::portability;
use nb_books_lib::reader::{Locations, Locator, TextContext};
use nb_books_lib::vocabulary;

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas().join(format!(
            "nb-vocabulario-{nombre}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
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

/// Una edición donde colgar el vocabulario. Devuelve su identificador.
fn edicion(conn: &rusqlite::Connection, titulo: &str) -> String {
    let obra = Work::create(
        conn,
        &NewWork {
            canonical_title: titulo.to_owned(),
            original_language: Some("es".to_owned()),
        },
    )
    .expect("obra");
    Edition::create(
        conn,
        &NewEdition {
            work_id: obra.id,
            title: titulo.to_owned(),
            ..NewEdition::default()
        },
    )
    .expect("edición")
    .id
}

/// Un locator con contexto de texto, que es como los produce el lector.
fn locator(href: &str, progresion: f64, cita: &str) -> Locator {
    Locator::new(
        href,
        "application/xhtml+xml",
        Locations {
            fragments: vec![format!("epubcfi(/6/14!/4/10/3:{})", cita.len())],
            progression: Some(progresion),
            total_progression: Some(progresion / 2.0),
            position: None,
        },
        Some(TextContext::new(None, cita, None).expect("contexto")),
    )
    .expect("locator")
}

/// **El criterio, tercera mitad:** la entrada guarda el punto exacto, y el punto
/// exacto vuelve entero.
///
/// Se afirma sobre el `Locator` completo y no sobre su `href`: un salto que
/// acierta el capítulo y pierde el párrafo cumpliría una comprobación floja y no
/// el criterio, que dice **exacto**.
#[test]
fn cada_palabra_guarda_el_punto_exacto_donde_aparecio() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let edicion = edicion(&conn, "Rayuela");
    let donde = locator("OEBPS/cap07.xhtml", 0.42, "aletea");

    let apuntada = vocabulary::record(&conn, &edicion, "es-basico", "aletear", "aletea", &donde)
        .expect("se apunta")
        .expect("no era un rebote");

    assert_eq!(apuntada.headword, "aletear");
    assert_eq!(apuntada.term, "aletea");
    assert_eq!(
        apuntada.locator, donde,
        "el locator guardado no es el mismo que se apuntó"
    );

    let lista = vocabulary::list(&conn, 100, 0).expect("lista");
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].title, "Rayuela");
    assert_eq!(
        lista[0].entry.locator, donde,
        "el locator no vuelve entero al listarlo"
    );
}

/// **El criterio, primera mitad:** la lista sobrevive a reiniciar.
///
/// Se comprueba cerrando la base y volviéndola a abrir desde el archivo, no
/// releyendo la misma conexión: una lista que solo vive en memoria pasaría lo
/// segundo y fallaría lo primero, que es lo que le ocurre al usuario.
#[test]
fn la_lista_sobrevive_a_cerrar_y_volver_a_abrir() {
    let tmp = Temporal::nuevo("reinicio");
    let ruta = tmp.base("biblioteca");

    let esperado = {
        let db = Database::open(&ruta).expect("base");
        let conn = db.connection().expect("conexión");
        let edicion = edicion(&conn, "Pedro Páramo");
        let donde = locator("OEBPS/cap01.xhtml", 0.1, "murmullos");
        vocabulary::record(
            &conn,
            &edicion,
            "es-basico",
            "murmullo",
            "murmullos",
            &donde,
        )
        .expect("se apunta")
        .expect("no era un rebote")
    };

    let db = Database::open(&ruta).expect("reabrir");
    let conn = db.connection().expect("conexión");
    let lista = vocabulary::list(&conn, 100, 0).expect("lista");

    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].entry.id, esperado.id);
    assert_eq!(lista[0].entry.locator, esperado.locator);
}

/// **El criterio, segunda mitad:** entra en la exportación y vuelve al importar.
#[test]
fn el_vocabulario_viaja_en_la_exportacion_completa() {
    let tmp = Temporal::nuevo("exportacion");
    let origen = Database::open(&tmp.base("origen")).expect("origen");

    let (esperado, cuantas) = {
        let conn = origen.connection().expect("conexión");
        let uno = edicion(&conn, "La ciudad y los perros");
        let otro = edicion(&conn, "Cien años de soledad");

        vocabulary::record(
            &conn,
            &uno,
            "es-basico",
            "arrecife",
            "arrecifes",
            &locator("OEBPS/cap02.xhtml", 0.2, "arrecifes"),
        )
        .expect("apunta")
        .expect("no rebote");
        let segunda = vocabulary::record(
            &conn,
            &otro,
            "es-basico",
            "alquimia",
            "alquimia",
            &locator("OEBPS/cap01.xhtml", 0.05, "alquimia"),
        )
        .expect("apunta")
        .expect("no rebote");

        (
            segunda,
            vocabulary::list(&conn, 100, 0).expect("lista").len(),
        )
    };
    assert_eq!(cuantas, 2);

    let exportado = {
        let conn = origen.connection().expect("conexión");
        portability::export(&conn).expect("exportación")
    };
    assert_eq!(
        exportado.vocabulary.len(),
        2,
        "el vocabulario no viajó en la exportación"
    );
    drop(origen);

    let destino = Database::open(&tmp.base("destino")).expect("destino");
    {
        let mut conn = destino.connection().expect("conexión");
        portability::import(&mut conn, &exportado).expect("importación");
    }

    let conn = destino.connection().expect("conexión");
    let lista = vocabulary::list(&conn, 100, 0).expect("lista");
    assert_eq!(lista.len(), 2);

    let recuperada = lista
        .iter()
        .find(|i| i.entry.id == esperado.id)
        .expect("la palabra restaurada");
    assert_eq!(recuperada.entry.locator, esperado.locator);
    assert_eq!(recuperada.title, "Cien años de soledad");
}

/// Consultar dos veces la misma palabra sin moverse no es un dato: es haber
/// cerrado y vuelto a abrir el panel.
#[test]
fn consultar_dos_veces_en_el_mismo_punto_no_duplica_la_palabra() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let edicion = edicion(&conn, "El Aleph");
    let donde = locator("OEBPS/cap03.xhtml", 0.3, "inconcebible");

    let primera = vocabulary::record(
        &conn,
        &edicion,
        "es-basico",
        "inconcebible",
        "inconcebible",
        &donde,
    )
    .expect("apunta");
    let segunda = vocabulary::record(
        &conn,
        &edicion,
        "es-basico",
        "inconcebible",
        "inconcebible",
        &donde,
    )
    .expect("apunta");

    assert!(primera.is_some());
    assert!(segunda.is_none(), "el rebote no se descarta");
    assert_eq!(vocabulary::list(&conn, 100, 0).expect("lista").len(), 1);
}

/// Pero la misma palabra en **otro** punto sí es un dato: es la que cuesta.
#[test]
fn la_misma_palabra_en_otro_punto_si_se_apunta() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let edicion = edicion(&conn, "Ficciones");

    vocabulary::record(
        &conn,
        &edicion,
        "es-basico",
        "hexagonal",
        "hexagonales",
        &locator("OEBPS/cap01.xhtml", 0.1, "hexagonales"),
    )
    .expect("apunta")
    .expect("no rebote");
    vocabulary::record(
        &conn,
        &edicion,
        "es-basico",
        "hexagonal",
        "hexagonal",
        &locator("OEBPS/cap04.xhtml", 0.8, "hexagonal"),
    )
    .expect("apunta")
    .expect("no rebote");

    assert_eq!(vocabulary::list(&conn, 100, 0).expect("lista").len(), 2);
}

/// La lista se borra entera, que es lo que el criterio pide: es un registro de
/// lo que alguien ha leído y tiene que poder deshacerse de él de una vez.
#[test]
fn el_vocabulario_se_borra_entero_y_de_una_vez() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let edicion = edicion(&conn, "Los detectives salvajes");

    for (lema, href) in [("visceral", "c1"), ("realismo", "c2"), ("desierto", "c3")] {
        vocabulary::record(
            &conn,
            &edicion,
            "es-basico",
            lema,
            lema,
            &locator(href, 0.5, lema),
        )
        .expect("apunta")
        .expect("no rebote");
    }

    assert_eq!(vocabulary::clear(&conn).expect("borrado"), 3);
    assert!(vocabulary::list(&conn, 100, 0).expect("lista").is_empty());
}

/// Dar de baja la obra se lleva su vocabulario: una palabra que apunta a un
/// libro que ya no está no lleva a ninguna parte, y el criterio del salto
/// dejaría de poder cumplirse.
#[test]
fn borrar_la_edicion_se_lleva_su_vocabulario() {
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let edicion = edicion(&conn, "Efímera");

    vocabulary::record(
        &conn,
        &edicion,
        "es-basico",
        "efímero",
        "efímera",
        &locator("OEBPS/cap01.xhtml", 0.5, "efímera"),
    )
    .expect("apunta")
    .expect("no rebote");

    conn.execute("DELETE FROM editions WHERE id = ?1", [&edicion])
        .expect("baja");

    assert!(
        vocabulary::list(&conn, 100, 0).expect("lista").is_empty(),
        "quedó vocabulario apuntando a una edición que ya no existe"
    );
}

// --- Lo que se consultó queda: se exporta y se repasa --------------

/// Un diccionario de aquí dentro que sabe una palabra.
struct Diccionario;

impl nb_books_lib::lookup::LookupProvider for Diccionario {
    fn id(&self) -> &str {
        "es-prueba"
    }
    fn label(&self) -> &str {
        "Diccionario de prueba"
    }
    fn kind(&self) -> nb_books_lib::lookup::LookupKind {
        nb_books_lib::lookup::LookupKind::Dictionary
    }
    fn languages(&self) -> &[String] {
        &[]
    }
    fn requires_network(&self) -> bool {
        false
    }
    fn lookup(
        &self,
        query: &nb_books_lib::lookup::Query,
    ) -> nb_books_lib::error::NbResult<Vec<nb_books_lib::lookup::Entry>> {
        Ok(if query.term == "candela" {
            vec![nb_books_lib::lookup::Entry {
                provider_id: "es-prueba".to_owned(),
                kind: nb_books_lib::lookup::LookupKind::Dictionary,
                headword: "candela".to_owned(),
                body: "Vela de cera o de sebo.".to_owned(),
                source: Some("Diccionario de prueba".to_owned()),
            }]
        } else {
            Vec::new()
        })
    }
}

/// Uno en línea que **no puede** llamarse: hacer tarjetas no sale a la red.
struct EnLinea;

impl nb_books_lib::lookup::LookupProvider for EnLinea {
    fn id(&self) -> &str {
        "en-linea"
    }
    fn label(&self) -> &str {
        "En línea"
    }
    fn kind(&self) -> nb_books_lib::lookup::LookupKind {
        nb_books_lib::lookup::LookupKind::Dictionary
    }
    fn languages(&self) -> &[String] {
        &[]
    }
    fn requires_network(&self) -> bool {
        true
    }
    fn lookup(
        &self,
        _query: &nb_books_lib::lookup::Query,
    ) -> nb_books_lib::error::NbResult<Vec<nb_books_lib::lookup::Entry>> {
        panic!("hacer tarjetas con el vocabulario no puede salir a la red");
    }
}

fn locator_con_contexto(antes: &str, cita: &str, despues: &str) -> Locator {
    Locator::new(
        "OEBPS/c1.xhtml",
        "application/xhtml+xml",
        Locations {
            fragments: vec!["#char=40".to_owned()],
            progression: Some(0.1),
            total_progression: Some(0.05),
            position: None,
        },
        Some(TextContext::new(Some(antes), cita, Some(despues)).expect("contexto")),
    )
    .expect("locator")
}

#[test]
fn el_vocabulario_se_exporta_con_su_libro_y_su_contexto() {
    let tmp = Temporal::nuevo("exporta");
    let db = Database::open(&tmp.base("vocabulario")).expect("base");
    let conn = db.connection().expect("conexión");
    let libro = edicion(&conn, "Cien años de \"soledad\"");
    vocabulary::record(
        &conn,
        &libro,
        "es-prueba",
        "candela",
        "candelas",
        &locator_con_contexto("encendió las ", "candelas", " del altar"),
    )
    .expect("apuntada");

    let (csv, cuantas) = vocabulary::exportar(&conn).expect("exportado");
    assert_eq!(cuantas, 1);
    let lineas: Vec<&str> = csv.lines().collect();
    assert_eq!(lineas[0], "palabra,lema,libro,contexto,fecha,diccionario");
    assert!(lineas[1].starts_with("\"candelas\",\"candela\",\"Cien años de \"\"soledad\"\"\","));
    assert!(
        lineas[1].contains("…encendió las «candelas» del altar…"),
        "{}",
        lineas[1]
    );
}

#[test]
fn el_vocabulario_se_convierte_en_tarjetas_sin_copiar_nada_ni_salir_a_la_red() {
    let tmp = Temporal::nuevo("tarjetas");
    let db = Database::open(&tmp.base("vocabulario")).expect("base");
    let mut conn = db.connection().expect("conexión");
    let libro = edicion(&conn, "Un libro");
    let sitio = locator_con_contexto("encendió las ", "candelas", " del altar");
    vocabulary::record(&conn, &libro, "es-prueba", "candela", "candelas", &sitio).expect("una");
    vocabulary::record(
        &conn,
        &libro,
        "es-prueba",
        "almendro",
        "almendros",
        &locator_con_contexto("bajo los ", "almendros", " del patio"),
    )
    .expect("otra");

    let mut registro = nb_books_lib::lookup::LookupRegistry::new();
    registro.register(Box::new(Diccionario));
    registro.register(Box::new(EnLinea));

    let informe = vocabulary::a_tarjetas(&mut conn, &registro).expect("tarjetas");
    assert_eq!(informe.created, 1, "{informe:?}");
    assert_eq!(
        informe.without_definition, 1,
        "la que el diccionario no define no hace una tarjeta vacía"
    );

    // La tarjeta es de estudio como cualquier otra, con su fuente en el párrafo
    // donde se consultó la palabra.
    let tarjetas =
        nb_books_lib::ai::cuaderno::tarjetas(&conn, Some(&libro), false, 50, 0).expect("lista");
    assert_eq!(tarjetas.len(), 1);
    let tarjeta = &tarjetas[0];
    assert_eq!(tarjeta.front, "…encendió las «candelas» del altar…");
    assert_eq!(tarjeta.back, "candela: Vela de cera o de sebo.");
    assert_eq!(tarjeta.sources, vec![sitio]);
    assert_eq!(tarjeta.template, vocabulary::PLANTILLA);

    // Otra vez: nada se repite.
    let otra = vocabulary::a_tarjetas(&mut conn, &registro).expect("tarjetas");
    assert_eq!((otra.created, otra.already), (0, 1));
}
