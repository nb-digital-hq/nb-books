//! Criterio de aceptación de la.
//!
//! "Un proveedor simulado permite ejercitar todo el lector sin ningún formato
//! real."
//!
//! El criterio es el propio contrato: si todo lo que el lector necesita se puede
//! hacer contra un documento que no existe en el disco, entonces el lector no
//! conoce formatos. Y si no los conoce, añadir uno es añadir un archivo.
//!
//! Contrato: `docs/architecture/reader-contracts.md`, sección 4.

use std::path::Path;

use nb_books_lib::error::NbError;
use nb_books_lib::reader::fake::{self, FakeFactory, MEDIA_TYPE};
use nb_books_lib::reader::secciones::{DocumentoDeSecciones, Seccion};
use nb_books_lib::reader::{
    DocumentProvider, NativePosition, Precision, ProviderRegistry, ReaderCapabilities,
};

fn documento() -> DocumentoDeSecciones {
    fake::con_tres_capitulos()
}

fn en(href: &str, offset: u32) -> NativePosition {
    NativePosition::Fragment {
        href: href.to_owned(),
        offset,
    }
}

// --- El criterio: todo el lector, sin ningún formato real -------------------

/// **El criterio de aceptación**, recorrido entero.
///
/// Las cinco cosas que el contrato promete, sobre un documento que no existe en
/// ningún disco: navegar, situar, resolver, extraer texto y declarar qué sabe
/// hacer.
#[test]
fn el_lector_completo_funciona_sobre_un_documento_que_no_existe() {
    let doc = documento();

    // 1. Navegar.
    let navegacion = doc.navigation().expect("navegación");
    assert_eq!(navegacion.entries.len(), 3);
    assert_eq!(navegacion.entries[0].title, "Primer capítulo");
    assert_eq!(navegacion.entries[2].locator.href, "c3.xhtml");

    // 2. Situar: una posición del formato se convierte en un locator que se
    //    puede guardar.
    let locator = doc.locator_at(&en("c2.xhtml", 10)).expect("locator");
    assert_eq!(locator.href, "c2.xhtml");
    assert!(locator.locations.progression.is_some());
    assert!(locator.text.is_some(), "sin cita no hay reanclaje posible");

    // 3. Resolver: y vuelve al mismo sitio.
    let resolucion = doc.resolve(&locator).expect("resolución");
    assert_eq!(resolucion.precision, Precision::Exact);
    assert_eq!(resolucion.position, en("c2.xhtml", 10));

    // 4. Extraer texto.
    let texto = doc.text_range(&locator, 7).expect("texto");
    assert_eq!(texto.chars().count(), 7);

    // 5. Declarar capacidades.
    let capacidades = doc.capabilities();
    assert!(capacidades.reflow);
    assert!(capacidades.text_extraction);
    assert!(!capacidades.synchronized_audio);
}

/// Guardar y restaurar tiene que devolver al mismo sitio, o el progreso de
/// lectura no vale nada.
#[test]
fn situar_y_resolver_es_una_ida_y_vuelta() {
    let doc = documento();

    for (href, offset) in [("c1.xhtml", 0), ("c1.xhtml", 25), ("c3.xhtml", 40)] {
        let locator = doc.locator_at(&en(href, offset)).expect("locator");
        let resolucion = doc.resolve(&locator).expect("resolución");

        assert_eq!(resolucion.position, en(href, offset), "en {href}:{offset}");
        assert_eq!(resolucion.precision, Precision::Exact);
    }
}

// --- La cascada de ADR-004 --------------------------------------------------

/// **Segundo escalón.** El fragmento deja de resolver y la cita salva la
/// posición. Es el caso que ocurre cuando el documento cambia de edición.
#[test]
fn cuando_el_fragmento_deja_de_resolver_la_cita_salva_la_anotacion() {
    let mut doc = documento();
    let locator = doc.locator_at(&en("c2.xhtml", 10)).expect("locator");
    let cita = locator.text.as_ref().expect("cita").highlight.clone();

    // El capítulo se reescribe: el mismo texto, precedido de un prólogo. El
    // desplazamiento antiguo ya no señala lo mismo.
    let original = "Macondo era entonces una aldea de veinte casas de barro y cañabrava \
                    construidas a la orilla de un río de aguas diáfanas.";
    doc.reescribir(
        "c2.xhtml",
        &format!("Nota del editor a esta edición. {original}"),
    );

    let resolucion = doc.resolve(&locator).expect("resolución");

    assert_eq!(
        resolucion.precision,
        Precision::Quote,
        "debía resolverse por la cita"
    );
    // Y apunta donde está la cita ahora, no donde estaba antes.
    let NativePosition::Fragment { href, offset } = resolucion.position else {
        panic!("posición inesperada");
    };
    assert_eq!(href, "c2.xhtml");
    let texto = doc
        .text_range(&locator, cita.chars().count())
        .expect("texto");
    assert_eq!(texto, cita, "la cita tiene que quedar bajo la anotación");
    assert!(offset > 10);
}

/// La cita se busca en **todos** los recursos: un capítulo puede haberse
/// dividido o renombrado entre ediciones, y el texto sigue estando.
#[test]
fn la_cita_se_encuentra_aunque_haya_cambiado_de_capitulo() {
    let mut doc = documento();
    let locator = doc.locator_at(&en("c2.xhtml", 0)).expect("locator");
    let cita = locator.text.as_ref().expect("cita").highlight.clone();

    // El capítulo desaparece y su texto acaba en otro.
    doc.eliminar("c2.xhtml");
    doc.reescribir("c3.xhtml", &format!("{cita} y lo que sigue."));

    let resolucion = doc.resolve(&locator).expect("resolución");

    assert_eq!(resolucion.precision, Precision::Quote);
    assert_eq!(resolucion.position, en("c3.xhtml", 0));
}

/// **Tercer escalón.** Ni fragmento ni cita: se usa la progresión y **se marca**.
///
/// Es la promesa de ADR-004: es preferible una anotación señalada como incierta
/// que una anotación silenciosamente mal situada.
#[test]
fn sin_fragmento_ni_cita_se_aproxima_y_se_avisa() {
    let mut doc = documento();
    let locator = doc.locator_at(&en("c1.xhtml", 20)).expect("locator");

    // El capítulo se reescribe entero: ni el desplazamiento vale ni la cita
    // existe ya.
    doc.reescribir("c1.xhtml", "Un texto completamente distinto del anterior.");

    let resolucion = doc.resolve(&locator).expect("resolución");

    assert_eq!(resolucion.precision, Precision::Approximate);
    assert!(
        resolucion.precision.requires_warning(),
        "una posición aproximada tiene que avisar al usuario"
    );
}

/// Lo que no se puede situar de ninguna forma no se inventa.
#[test]
fn lo_que_no_se_puede_situar_de_ninguna_forma_no_se_inventa() {
    let mut doc = documento();
    let locator = doc.locator_at(&en("c1.xhtml", 20)).expect("locator");
    doc.eliminar("c1.xhtml");
    doc.reescribir("c2.xhtml", "Nada que ver.");
    doc.reescribir("c3.xhtml", "Tampoco.");

    assert_eq!(doc.resolve(&locator), Err(NbError::DocumentNotFound));
}

/// Los tres escalones, en orden y sin saltarse ninguno: el exacto gana al de
/// cita, y el de cita al aproximado.
#[test]
fn la_cascada_prefiere_siempre_el_escalon_mas_preciso() {
    let doc = documento();
    let locator = doc.locator_at(&en("c1.xhtml", 5)).expect("locator");

    // Con todo intacto, gana el fragmento aunque la cita también resolvería.
    assert_eq!(
        doc.resolve(&locator).expect("resolución").precision,
        Precision::Exact
    );

    // Sin fragmento pero con cita, gana la cita aunque haya progresión.
    let mut sin_fragmento = locator.clone();
    sin_fragmento.locations.fragments.clear();
    assert_eq!(
        doc.resolve(&sin_fragmento).expect("resolución").precision,
        Precision::Quote
    );

    // Solo con progresión, se aproxima.
    let mut solo_progresion = sin_fragmento.clone();
    solo_progresion.text = None;
    assert_eq!(
        doc.resolve(&solo_progresion).expect("resolución").precision,
        Precision::Approximate
    );
}

// --- Capacidades ------------------------------------------------------------

/// Un formato sin capa de texto lo **dice**, en lugar de devolver una cadena
/// vacía que parece un documento sin contenido.
#[test]
fn un_formato_sin_extraccion_de_texto_lo_declara_en_lugar_de_fingir() {
    let doc = DocumentoDeSecciones::new(
        MEDIA_TYPE,
        vec![Seccion {
            href: "pagina1".to_owned(),
            title: "Escaneado".to_owned(),
            text: "lo que hubiera".to_owned(),
        }],
        ReaderCapabilities {
            text_selection: false,
            internal_search: false,
            reflow: false,
            typography: false,
            synchronized_audio: false,
            text_extraction: false,
        },
    );

    let locator = doc.locator_at(&en("pagina1", 0)).expect("locator");

    assert!(!doc.capabilities().text_extraction);
    assert_eq!(
        doc.text_range(&locator, 10),
        Err(NbError::FormatUnsupported),
        "una cadena vacía sería indistinguible de un documento sin texto"
    );
}

/// Una posición que no pertenece a este formato se rechaza en lugar de
/// interpretarse a la fuerza.
#[test]
fn una_posicion_ajena_al_formato_se_rechaza() {
    let doc = documento();

    assert_eq!(
        doc.locator_at(&NativePosition::Page(3)),
        Err(NbError::PathNotAllowed),
        "un formato reflowable no tiene páginas"
    );
    assert_eq!(
        doc.locator_at(&NativePosition::Time(12.5)),
        Err(NbError::PathNotAllowed)
    );
    assert_eq!(
        doc.locator_at(&en("no-existe.xhtml", 0)),
        Err(NbError::PathNotAllowed)
    );
}

// --- El registro ------------------------------------------------------------

/// Añadir un formato es registrar una fábrica, no tocar el lector.
#[test]
fn el_registro_abre_por_tipo_de_medio() {
    let mut registro = ProviderRegistry::new();
    assert!(!registro.supports(MEDIA_TYPE));
    assert!(registro.media_types().is_empty());

    registro.register(Box::new(FakeFactory));

    assert!(registro.supports(MEDIA_TYPE));
    assert_eq!(registro.media_types(), vec![MEDIA_TYPE]);

    let doc = registro
        .open(MEDIA_TYPE, Path::new("da-igual"))
        .expect("abierto");
    assert_eq!(doc.media_type(), MEDIA_TYPE);
    assert_eq!(doc.navigation().expect("navegación").entries.len(), 3);
}

/// **Que falte un formato no puede romper nada.**
///
/// Y el error dice qué clase de ausencia es: `COMPONENT_UNAVAILABLE` significa
/// "esta instalación no puede", que es lo que la interfaz necesita para ocultar
/// la función. `FORMAT_UNSUPPORTED` significaría "esto no es un documento", que
/// es otra cosa y se le cuenta al usuario de otra manera.
#[test]
fn un_tipo_de_medio_sin_proveedor_lo_dice_sin_romperse() {
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(FakeFactory));

    assert_eq!(
        registro
            .open("application/pdf", Path::new("libro.pdf"))
            .err(),
        Some(NbError::ComponentUnavailable)
    );
    assert!(!registro.supports("application/pdf"));
    // Y el que sí está sigue funcionando: la ausencia de uno no afecta al otro.
    assert!(registro.open(MEDIA_TYPE, Path::new("x")).is_ok());
}

/// El lector se maneja a través del contrato, sin saber qué hay detrás. Si esto
/// compila y pasa, el desacoplamiento es real.
#[test]
fn el_lector_trabaja_contra_el_contrato_y_no_contra_el_formato() {
    fn resumir(proveedor: &dyn DocumentProvider) -> (usize, bool, String) {
        let navegacion = proveedor.navigation().expect("navegación");
        let primero = &navegacion.entries[0].locator;
        (
            navegacion.entries.len(),
            proveedor.capabilities().reflow,
            proveedor.text_range(primero, 6).unwrap_or_default(),
        )
    }

    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(FakeFactory));
    let proveedor = registro.open(MEDIA_TYPE, Path::new("x")).expect("abierto");

    let (capitulos, reflujo, principio) = resumir(proveedor.as_ref());

    assert_eq!(capitulos, 3);
    assert!(reflujo);
    assert_eq!(principio, "Muchos");
}
