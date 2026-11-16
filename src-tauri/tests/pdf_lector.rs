//! Criterio de aceptación de la.
//!
//! "Un PDF de gran tamaño abre dentro del presupuesto de tiempo hasta la primera
//! página, y el consumo de memoria se mantiene estable durante desplazamiento
//! continuo."
//!
//! Y la propiedad que hace que todo lo demás sea posible: **la aplicación
//! funciona sin la biblioteca nativa**. Esa mitad se prueba siempre; la del
//! renderizado, solo cuando la biblioteca está.
//!
//! Contratos: `docs/audit/threat-model.md`, sección 3.1, y
//! `docs/adr/002-critical-bottlenecks.md`.

use std::io::Write;
use std::path::{Path, PathBuf};

use nb_books_lib::error::NbError;
use nb_books_lib::reader::pdf::{engine_state, Engine, PdfDocument, PdfFactory, MEDIA_TYPE};
use nb_books_lib::reader::{
    DocumentProvider, NativePosition, PageSize, Precision, ProviderRegistry, Rendered,
    TextLayerStatus,
};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-pdf-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        Self(dir)
    }
    fn cache(&self) -> PathBuf {
        self.0.join("cache")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Construye un PDF de `paginas` páginas, cada una con su texto.
///
/// A mano y no con una biblioteca: fabricar el caso de prueba con el mismo motor
/// que se está probando no demostraría nada, y un PDF mínimo son cuarenta
/// líneas de formato bien documentado.
fn pdf(tmp: &Temporal, nombre: &str, paginas: usize) -> PathBuf {
    let mut objetos: Vec<String> = Vec::new();

    // 1: catálogo. 2: árbol de páginas. Después, por cada página: la página, su
    // contenido y nada más.
    let ids_paginas: Vec<usize> = (0..paginas).map(|i| 3 + i * 2).collect();
    let kids = ids_paginas
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");

    objetos.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
    objetos.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {paginas} >>"
    ));

    for i in 0..paginas {
        let contenido_id = 4 + i * 2;
        objetos.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] \
             /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> \
             /Contents {contenido_id} 0 R >>"
        ));
        let texto = format!("Pagina numero {} de este documento de prueba.", i + 1);
        let flujo = format!("BT /F1 18 Tf 60 760 Td ({texto}) Tj ET");
        objetos.push(format!(
            "<< /Length {} >>\nstream\n{flujo}\nendstream",
            flujo.len()
        ));
    }

    let mut salida = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objetos.len());
    for (i, cuerpo) in objetos.iter().enumerate() {
        offsets.push(salida.len());
        salida.push_str(&format!("{} 0 obj\n{cuerpo}\nendobj\n", i + 1));
    }

    let inicio_xref = salida.len();
    salida.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objetos.len() + 1
    ));
    for offset in &offsets {
        salida.push_str(&format!("{offset:010} 00000 n \n"));
    }
    salida.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio_xref}\n%%EOF\n",
        objetos.len() + 1
    ));

    let ruta = tmp.0.join(nombre);
    let mut archivo = std::fs::File::create(&ruta).expect("pdf");
    archivo.write_all(salida.as_bytes()).expect("escritura");
    ruta
}

/// Memoria residente del proceso, en kibibytes. Solo en Linux.
#[cfg(target_os = "linux")]
fn memoria_kib() -> u64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).map(ToOwned::to_owned))
        .and_then(|p| p.parse::<u64>().ok())
        .map(|paginas| paginas * 4)
        .unwrap_or(0)
}

fn hay_motor() -> bool {
    engine_state() == Engine::Available
}

// --- Lo que se prueba siempre ----------------------------------------------

/// **La propiedad que hace que todo lo demás sea posible.**
///
/// Sin biblioteca nativa la aplicación no se rompe: el motor se declara no
/// instalado y quien intenta abrir un PDF recibe `COMPONENT_UNAVAILABLE`, que es
/// lo que la interfaz necesita para **ocultar** la función en lugar de decirle al
/// usuario que su documento está roto.
///
/// Es el escenario de caos que faltaba, y con el
/// enlazado en tiempo de ejecución sale gratis: para probar la ausencia, basta
/// con no tenerla.
#[test]
fn sin_biblioteca_nativa_el_componente_se_declara_no_instalado() {
    if hay_motor() {
        // Con la biblioteca presente no se puede simular su ausencia dentro del
        // mismo proceso: el enlace se hace una vez. La otra mitad la cubre
        // `caos_sin_biblioteca_de_pdf`, que lanza un proceso aparte.
        assert_eq!(engine_state(), Engine::Available);
        return;
    }

    assert_eq!(engine_state(), Engine::NotInstalled);

    let tmp = Temporal::nuevo("ausente");
    let ruta = pdf(&tmp, "documento.pdf", 1);

    assert_eq!(
        PdfDocument::open(&ruta, &tmp.cache()).err(),
        Some(NbError::ComponentUnavailable),
        "la ausencia del motor no es que el documento esté roto"
    );

    // Y el registro lo dice igual, sin romperse.
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(PdfFactory::new(tmp.cache())));
    assert_eq!(
        registro.open(MEDIA_TYPE, &ruta).err(),
        Some(NbError::ComponentUnavailable)
    );
}

/// El estado del motor es estable: no puede decir una cosa y luego otra.
#[test]
fn el_estado_del_motor_es_consistente() {
    let primero = engine_state();
    for _ in 0..5 {
        assert_eq!(engine_state(), primero);
    }
}

// --- Lo que solo se prueba con la biblioteca presente ------------------------

macro_rules! requiere_motor {
    () => {
        if !hay_motor() {
            eprintln!("omitida: sin biblioteca de PDF (pnpm pdfium)");
            return;
        }
    };
}

#[test]
fn se_abre_un_pdf_y_se_lee_su_estructura() {
    requiere_motor!();
    let tmp = Temporal::nuevo("estructura");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 5), &tmp.cache()).expect("abierto");

    assert_eq!(doc.page_count(), 5);
    assert!(doc.has_text_layer());
    assert!(doc.page_text(1).expect("texto").contains("Pagina numero 1"));
    assert!(doc.page_text(5).expect("texto").contains("Pagina numero 5"));
    assert_eq!(doc.page_text(6).err(), Some(NbError::DocumentNotFound));

    let navegacion = doc.navigation().expect("navegación");
    assert_eq!(navegacion.entries.len(), 5);
}

/// Un PDF tiene páginas fijas: ni se re-pagina ni admite cambiar la tipografía.
/// Ofrecer esos controles sería prometer algo imposible.
#[test]
fn un_pdf_declara_que_no_se_repagina() {
    requiere_motor!();
    let tmp = Temporal::nuevo("capacidades");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 2), &tmp.cache()).expect("abierto");

    let capacidades = doc.capabilities();
    assert!(!capacidades.reflow);
    assert!(!capacidades.typography);
    assert!(capacidades.text_extraction, "este sí tiene capa de texto");

    // Y sus posiciones son páginas, no fragmentos.
    assert_eq!(
        doc.locator_at(&NativePosition::Fragment {
            href: "c1.xhtml".to_owned(),
            offset: 0
        })
        .err(),
        Some(NbError::PathNotAllowed)
    );
}

#[test]
fn situar_y_resolver_una_pagina_es_una_ida_y_vuelta() {
    requiere_motor!();
    let tmp = Temporal::nuevo("locators");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 8), &tmp.cache()).expect("abierto");

    for pagina in [1_u32, 4, 8] {
        let locator = doc
            .locator_at(&NativePosition::Page(pagina))
            .expect("locator");
        assert_eq!(locator.locations.position, Some(pagina));

        let resolucion = doc.resolve(&locator).expect("resolución");
        assert_eq!(resolucion.position, NativePosition::Page(pagina));
        assert_eq!(resolucion.precision, Precision::Exact);
    }

    assert_eq!(
        doc.locator_at(&NativePosition::Page(9)).err(),
        Some(NbError::PathNotAllowed)
    );
}

/// La cascada de ADR-004 también aquí: si al documento le añaden páginas al
/// principio, la cita encuentra el texto en su página nueva.
#[test]
fn si_las_paginas_se_desplazan_la_cita_encuentra_el_texto() {
    requiere_motor!();
    let tmp = Temporal::nuevo("cascada");

    let original = PdfDocument::open(&pdf(&tmp, "original.pdf", 3), &tmp.cache()).expect("abierto");
    let locator = original
        .locator_at(&NativePosition::Page(1))
        .expect("locator");

    // Una edición con dos páginas más al principio: el texto de la antigua
    // página 1 está ahora en la 3.
    let ampliado = {
        let mut objetos = Vec::new();
        for i in 0..5 {
            objetos.push(if i < 2 {
                format!("Prologo {}", i + 1)
            } else {
                format!("Pagina numero {} de este documento de prueba.", i - 1)
            });
        }
        pdf_con_textos(&tmp, "ampliado.pdf", &objetos)
    };
    let doc = PdfDocument::open(&ampliado, &tmp.cache()).expect("abierto");

    let resolucion = doc.resolve(&locator).expect("resolución");

    assert_eq!(resolucion.precision, Precision::Quote);
    assert_eq!(
        resolucion.position,
        NativePosition::Page(3),
        "la cita tiene que encontrar el texto en su página nueva"
    );
}

/// **El criterio de aceptación.**
#[test]
fn un_pdf_grande_abre_rapido_y_la_memoria_no_crece_al_desplazarse() {
    requiere_motor!();
    let tmp = Temporal::nuevo("grande");
    // Trescientas páginas: un manual técnico o una tesis de verdad.
    let ruta = pdf(&tmp, "grande.pdf", 300);
    let tamano = std::fs::metadata(&ruta).expect("metadatos").len();

    // --- Tiempo hasta la primera página ---
    let inicio = std::time::Instant::now();
    let doc = PdfDocument::open(&ruta, &tmp.cache()).expect("abierto");
    let primera = doc.render_page(1).expect("primera página");
    let hasta_la_primera = inicio.elapsed();

    assert!(primera.exists());
    eprintln!(
        "pdf: {} páginas, {:.0} KiB, primera página en {:.0} ms",
        doc.page_count(),
        tamano as f64 / 1024.0,
        hasta_la_primera.as_millis()
    );
    assert!(
        hasta_la_primera < std::time::Duration::from_secs(3),
        "la primera página tardó {hasta_la_primera:?}"
    );

    // --- Desplazamiento continuo ---
    #[cfg(target_os = "linux")]
    {
        // Se calienta primero, para no medir la reserva inicial del motor.
        for pagina in 2..=20 {
            doc.render_page(pagina).expect("página");
        }
        let antes = memoria_kib();

        for pagina in 21..=140 {
            doc.render_page(pagina).expect("página");
        }
        let despues = memoria_kib();

        let crecimiento = despues.saturating_sub(antes);
        eprintln!("pdf: 120 páginas más, memoria {antes} -> {despues} KiB (+{crecimiento} KiB)");
        assert!(
            crecimiento < 128 * 1024,
            "la memoria creció {crecimiento} KiB al desplazarse: algo no se libera"
        );
    }
}

/// El desalojo por uso menos reciente: sin tope, desplazarse por un documento
/// largo llena el disco de páginas que nadie va a volver a mirar.
#[test]
fn la_cache_de_paginas_no_crece_sin_limite() {
    requiere_motor!();
    let tmp = Temporal::nuevo("cache");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 30), &tmp.cache()).expect("abierto");

    for pagina in 1..=30 {
        doc.render_page(pagina).expect("página");
    }

    let total: u64 = std::fs::read_dir(tmp.cache())
        .expect("caché")
        .filter_map(Result::ok)
        .filter_map(|e| e.metadata().ok().map(|m| m.len()))
        .sum();

    assert!(
        total <= 256 * 1024 * 1024,
        "la caché ocupa {total} bytes, por encima del presupuesto"
    );
    // Y volver a pedir una página ya rasterizada no la vuelve a rasterizar.
    let inicio = std::time::Instant::now();
    doc.render_page(30).expect("página cacheada");
    assert!(inicio.elapsed() < std::time::Duration::from_millis(200));
}

#[test]
fn lo_que_se_pinta_es_una_ruta_de_imagen_y_no_bytes_por_el_canal() {
    requiere_motor!();
    let tmp = Temporal::nuevo("render");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 1), &tmp.cache()).expect("abierto");

    let pintado = doc.render(&NativePosition::Page(1)).expect("render");

    let Rendered::Image { path, media_type } = pintado else {
        panic!("un PDF se pinta como imagen, no como marcado");
    };
    assert_eq!(media_type, "image/png");
    assert!(Path::new(&path).exists());
    // Es un PNG de verdad, no un archivo con ese nombre.
    let cabecera = std::fs::read(&path).expect("lectura");
    assert_eq!(
        &cabecera[..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
    );
    // Una página A4 a 2.000 de ancho excedía el presupuesto real del lector.
    let ancho = u32::from_be_bytes(cabecera[16..20].try_into().expect("ancho PNG"));
    let alto = u32::from_be_bytes(cabecera[20..24].try_into().expect("alto PNG"));
    assert!(ancho > 1240 && ancho <= 2000, "resolución útil: {ancho}");
    assert!(
        u64::from(ancho) * u64::from(alto) <= 5_000_000,
        "el raster de {ancho}x{alto} excede el presupuesto del lector"
    );
}

#[test]
fn un_archivo_que_no_es_pdf_se_rechaza_sin_confundirlo_con_un_motor_ausente() {
    requiere_motor!();
    let tmp = Temporal::nuevo("roto");
    let ruta = tmp.0.join("no-es.pdf");
    std::fs::write(&ruta, b"esto no es un pdf en absoluto").expect("archivo");

    assert_eq!(
        PdfDocument::open(&ruta, &tmp.cache()).err(),
        Some(NbError::DocumentCorrupted)
    );
    assert_eq!(
        PdfDocument::open(&tmp.0.join("no-existe.pdf"), &tmp.cache()).err(),
        Some(NbError::FileNotFound)
    );
}

/// Variante del constructor con textos a medida.
fn pdf_con_textos(tmp: &Temporal, nombre: &str, textos: &[String]) -> PathBuf {
    let mut objetos: Vec<String> = Vec::new();
    let ids: Vec<usize> = (0..textos.len()).map(|i| 3 + i * 2).collect();
    let kids = ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");

    objetos.push("<< /Type /Catalog /Pages 2 0 R >>".to_owned());
    objetos.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {} >>",
        textos.len()
    ));

    for (i, texto) in textos.iter().enumerate() {
        let contenido_id = 4 + i * 2;
        objetos.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] \
             /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> \
             /Contents {contenido_id} 0 R >>"
        ));
        let flujo = format!("BT /F1 18 Tf 60 760 Td ({texto}) Tj ET");
        objetos.push(format!(
            "<< /Length {} >>\nstream\n{flujo}\nendstream",
            flujo.len()
        ));
    }

    let mut salida = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objetos.len());
    for (i, cuerpo) in objetos.iter().enumerate() {
        offsets.push(salida.len());
        salida.push_str(&format!("{} 0 obj\n{cuerpo}\nendobj\n", i + 1));
    }
    let inicio_xref = salida.len();
    salida.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objetos.len() + 1
    ));
    for offset in &offsets {
        salida.push_str(&format!("{offset:010} 00000 n \n"));
    }
    salida.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{inicio_xref}\n%%EOF\n",
        objetos.len() + 1
    ));

    let ruta = tmp.0.join(nombre);
    std::fs::write(&ruta, salida).expect("pdf");
    ruta
}

// --- Capa de texto accesible ---------------------------------------

/// **Criterio de aceptación**, primera mitad: hay contenido que un
/// lector de pantalla pueda leer, y está donde está en la página.
///
/// La lectura con un lector de pantalla real exige a una persona, y la superposición
/// visual la pinta la interfaz. Lo que se demuestra aquí es lo que las hace posibles: que
/// el texto y sus coordenadas existen y son correctos.
#[test]
fn una_pagina_con_texto_expone_su_capa_con_coordenadas() {
    requiere_motor!();
    let tmp = Temporal::nuevo("capa");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 3), &tmp.cache()).expect("abierto");

    let capa = doc.page_text_layer(2).expect("capa de texto");

    assert_eq!(capa.page, 2);
    assert_eq!(capa.status, TextLayerStatus::Available);
    assert!(
        !capa.spans.is_empty(),
        "una página con texto tiene fragmentos"
    );

    let todo: String = capa.spans.iter().map(|s| s.text.as_str()).collect();
    assert!(
        todo.contains("Pagina numero 2"),
        "la capa tiene que llevar el texto de su página: {todo}"
    );

    // Las coordenadas van normalizadas de 0 a 1 sobre el tamaño de la página:
    // en píxeles, cambiar el zoom descolocaría el texto respecto de la imagen.
    for span in &capa.spans {
        assert!(
            (0.0..=1.0).contains(&span.x) && (0.0..=1.0).contains(&span.y),
            "coordenadas fuera de la página: {span:?}"
        );
        assert!(span.width > 0.0 && span.height > 0.0);
        assert!(
            span.x + span.width <= 1.001,
            "se sale por la derecha: {span:?}"
        );
    }

    // El origen es la esquina superior izquierda, como en el marcado: el texto
    // está arriba de la página, así que su `y` es pequeña.
    let primero = &capa.spans[0];
    assert!(
        primero.y < 0.3,
        "el texto está arriba de la página, así que su y debe ser pequeña: {primero:?}"
    );
}

/// **Segunda mitad del criterio**, la que de verdad importa: un documento sin
/// capa de texto **se declara como tal** en lugar de aparentar ser accesible.
#[test]
fn un_documento_sin_capa_de_texto_lo_declara_en_lugar_de_aparentarlo() {
    requiere_motor!();
    let tmp = Temporal::nuevo("escaneado");
    // Un PDF con páginas sin ningún operador de texto: el equivalente a un
    // escaneado sin OCR.
    let ruta = pdf_con_textos(&tmp, "escaneado.pdf", &[String::new(), String::new()]);
    let doc = PdfDocument::open(&ruta, &tmp.cache()).expect("abierto");

    assert_eq!(doc.accessibility(), TextLayerStatus::Missing);
    assert!(!doc.has_text_layer());

    let capa = doc.page_text_layer(1).expect("capa");
    assert_eq!(
        capa.status,
        TextLayerStatus::Missing,
        "no basta con devolver una lista vacía: hay que decir que no hay texto"
    );
    assert!(capa.spans.is_empty());

    // Y las capacidades lo dicen también, para que la interfaz no ofrezca
    // buscar ni seleccionar en algo donde no se puede.
    let capacidades = doc.capabilities();
    assert!(!capacidades.text_extraction);
    assert!(!capacidades.text_selection);
    assert!(!capacidades.internal_search);

    // Extraer texto responde que el formato no lo soporta, que es distinto de
    // devolver una cadena vacía: una vacía es indistinguible de una página en
    // blanco, y esta distinción es la que permite avisar al usuario.
    let locator = doc.locator_at(&NativePosition::Page(1)).expect("locator");
    assert_eq!(
        doc.text_range(&locator, 10),
        Err(NbError::FormatUnsupported)
    );
}

#[test]
fn pedir_la_capa_de_una_pagina_que_no_existe_es_un_error() {
    requiere_motor!();
    let tmp = Temporal::nuevo("capa-inexistente");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 2), &tmp.cache()).expect("abierto");

    assert_eq!(
        doc.page_text_layer(0).err(),
        Some(NbError::DocumentNotFound)
    );
    assert_eq!(
        doc.page_text_layer(3).err(),
        Some(NbError::DocumentNotFound)
    );
}

/// La capa cruza el puente hacia la interfaz, así que su forma es contrato.
#[test]
fn la_capa_de_texto_viaja_al_puente_en_camel_case() {
    requiere_motor!();
    let tmp = Temporal::nuevo("serializacion");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 1), &tmp.cache()).expect("abierto");

    let json = serde_json::to_string(&doc.page_text_layer(1).expect("capa")).expect("serializable");

    assert!(json.contains(r#""status":"available""#), "json: {json}");
    assert!(json.contains(r#""spans""#));
    assert!(!json.contains("text_layer"));
}

// --- Lo que la superficie de lectura necesita ----------------------

/// El tamaño de la página se sabe **antes** de tener la imagen.
///
/// Es lo que permite reservar el hueco y no dar el salto de maquetación. Un
/// tamaño deducido de la imagen rendida llegaría cuando el salto ya ha ocurrido,
/// y además sería el del rendido y no el del documento.
#[test]
fn el_tamano_de_la_pagina_se_conoce_sin_pintarla() {
    requiere_motor!();
    let tmp = Temporal::nuevo("tamano");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 2), &tmp.cache()).expect("abierto");

    let medida = doc
        .page_size(&NativePosition::Page(1))
        .expect("tamaño de la página");

    assert!(medida.width > 0 && medida.height > 0);
    // El fixture declara páginas A4: 595 por 842 puntos. El tamaño sale del
    // documento y no del rendido, que se compone a un ancho fijo distinto.
    assert_eq!(
        medida,
        PageSize {
            width: 595,
            height: 842
        }
    );

    // Y no hay ningún archivo en la caché: preguntar el tamaño no pinta nada.
    let pintadas = std::fs::read_dir(tmp.cache())
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(pintadas, 0, "preguntar el tamaño no debe rendir la página");
}

/// Una página que no existe no tiene tamaño, y se dice.
#[test]
fn una_pagina_inexistente_no_tiene_tamano_inventado() {
    requiere_motor!();
    let tmp = Temporal::nuevo("tamano-inexistente");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 2), &tmp.cache()).expect("abierto");

    assert_eq!(
        doc.page_size(&NativePosition::Page(9)).err(),
        Some(NbError::DocumentNotFound)
    );
}

/// **La guarda contra la desalineación entre lo que se ve y lo que se ancla.**
///
/// La interfaz superpone los fragmentos de la capa de texto y anota lo que el
/// usuario selecciona sobre ellos. Si el texto de un fragmento no se encontrara
/// en el texto de la página, esa selección no se podría anclar y la anotación se
/// perdería: son dos vistas del mismo texto y tienen que seguir siéndolo.
#[test]
fn seleccionar_sobre_la_capa_de_texto_produce_una_cita_que_ancla() {
    requiere_motor!();
    let tmp = Temporal::nuevo("seleccion");
    let doc = PdfDocument::open(&pdf(&tmp, "libro.pdf", 3), &tmp.cache()).expect("abierto");

    let capa = doc
        .text_layer(&NativePosition::Page(2))
        .expect("capa de texto");
    let seleccion: String = capa
        .spans
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!seleccion.trim().is_empty());

    let locator = doc
        .locator_for_quote(seleccion.trim(), None)
        .expect("lo que se ve se puede anclar");

    assert_eq!(locator.locations.position, Some(2));
    assert_eq!(
        doc.resolve(&locator).expect("resuelve").precision,
        Precision::Exact
    );
}

/// Una cita repetida en varias páginas ancla a la más cercana a la lectura.
#[test]
fn una_cita_repetida_ancla_a_la_pagina_mas_cercana_a_donde_se_lee() {
    requiere_motor!();
    let tmp = Temporal::nuevo("repetida-pdf");
    let comun = "frase que se repite".to_owned();
    let ruta = pdf_con_textos(
        &tmp,
        "libro.pdf",
        &[
            comun.clone(),
            "relleno intermedio".to_owned(),
            comun.clone(),
        ],
    );
    let doc = PdfDocument::open(&ruta, &tmp.cache()).expect("abierto");

    let al_principio = doc.locator_for_quote(&comun, Some(0.0)).expect("ancla");
    let al_final = doc.locator_for_quote(&comun, Some(1.0)).expect("ancla");

    assert_eq!(al_principio.locations.position, Some(1));
    assert_eq!(al_final.locations.position, Some(3));
}

/// Regresión: abrir una sesión sobre un PDF nuevo no debe llamar a
/// `render` en las doce primeras entradas del índice buscando marcado HTML, ni
/// construir el índice debe extraer el texto de todas las páginas.
#[test]
fn abrir_una_sesion_de_pdf_no_rasteriza_las_doce_primeras_paginas() {
    use nb_books_lib::db::catalog::StorageMode;
    use nb_books_lib::db::Database;
    use nb_books_lib::events::EventBus;
    use nb_books_lib::library::{import_document, ImportContext, ImportOutcome, ImportRequest};
    use nb_books_lib::reader::progress::ProgressTracker;
    use nb_books_lib::reader::ReadingSession;
    use std::sync::Arc;

    requiere_motor!();
    let tmp = Temporal::nuevo("sesion-pdf-f736");
    let ruta = pdf(&tmp, "manual.pdf", 2_000);

    let db = Database::open_in_memory().expect("base");
    let bus = EventBus::new();
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(PdfFactory::new(tmp.cache())));

    let mut conn = db.connection().expect("conexión");
    let ImportOutcome::Imported(datos) = import_document(
        &mut conn,
        &ImportContext {
            scope: &tmp.0,
            library: &tmp.0,
            bus: &bus,
        },
        &ImportRequest {
            source: ruta,
            storage_mode: StorageMode::Referenced,
            root: None,
        },
    )
    .expect("importación") else {
        panic!("debe importarse como nuevo");
    };
    drop(conn);

    let inicio = std::time::Instant::now();
    let sesion = ReadingSession::open(
        &db,
        &registro,
        &datos.file.id,
        Arc::new(ProgressTracker::new(db.clone())),
        None,
    )
    .expect("sesión abierta");
    let apertura = inicio.elapsed();

    let pintadas_al_abrir = std::fs::read_dir(tmp.cache())
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(
        pintadas_al_abrir, 0,
        "abrir la sesión no debe rasterizar páginas por adelantado"
    );
    assert_eq!(sesion.navigation().entries.len(), 2_000);
    // En modo depuración y con toda la suite ejecutando en paralelo, 500 ms puede
    // fluctuar ligeramente por contención de CPU; el presupuesto del contrato son 2 s.
    assert!(
        apertura < std::time::Duration::from_millis(1500),
        "abrir la sesión de un PDF de 2.000 páginas tardó {apertura:?}"
    );
}
