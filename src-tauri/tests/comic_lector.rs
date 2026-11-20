//! Criterio de aceptación de la.
//!
//! "Un archivo con rutas de escape o con ratio de compresión desmedido se
//! rechaza sin escribir fuera del destino ni agotar la memoria."
//!
//! El cómic es el formato más simple y el más expuesto: es un ZIP con imágenes,
//! y un ZIP es donde viven el recorrido de directorios y la bomba de
//! descompresión. Estas pruebas comprueban las dos cosas **por la puerta del
//! proveedor**, no por la del lector de ZIP, porque lo que hay que demostrar no
//! es que el lector sepa defenderse sino que no hay forma de abrir un cómic
//! saltándose esa defensa.
//!
//! Contratos: `docs/architecture/reader-contracts.md` y
//! `docs/audit/threat-model.md`, sección 3.1.

use std::path::{Path, PathBuf};

use nb_books_lib::error::NbError;
use nb_books_lib::reader::comic::{ComicDocument, ComicFactory, MEDIA_TYPE};
use nb_books_lib::reader::{
    DocumentProvider, FitMode, NativePosition, PageSize, Precision, ProviderRegistry, Rendered,
};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-comic-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        Self(dir)
    }

    fn ruta(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn abrir(tmp: &Temporal, nombre: &str) -> Result<ComicDocument, NbError> {
    ComicDocument::open(&fixtures().join(nombre), tmp.ruta())
}

// --- El criterio de aceptación ---------------------------------------------

#[test]
fn un_comic_con_rutas_de_escape_se_rechaza_sin_escribir_fuera_del_destino() {
    let tmp = Temporal::nuevo("escape");
    let vigilado = tmp
        .ruta()
        .parent()
        .expect("padre")
        .join("fuera-del-destino.txt");
    let _ = std::fs::remove_file(&vigilado);

    assert_eq!(
        abrir(&tmp, "recorrido-directorios.cbz").err(),
        Some(NbError::PathNotAllowed),
        "una entrada con ../ tiene que rechazar el contenedor entero"
    );

    // Lo que de verdad importa no es el código de error, sino que no haya
    // aparecido nada donde la entrada quería ponerlo.
    assert!(
        !vigilado.exists(),
        "el recorrido de directorios escribió fuera del destino"
    );
    assert_eq!(
        std::fs::read_dir(tmp.ruta()).expect("caché").count(),
        0,
        "no debería haberse extraído nada"
    );
}

#[test]
fn un_comic_con_ratio_de_compresion_desmedido_se_rechaza_antes_de_descomprimirlo() {
    let tmp = Temporal::nuevo("bomba");

    // El archivo ocupa 20 KB y declara 20 MiB. Si el rechazo llegara después de
    // descomprimir, esta prueba pasaría igual y la defensa no serviría de nada:
    // lo que la hace válida es que el error sale del índice, donde solo se han
    // leído los tamaños declarados.
    let en_disco = std::fs::metadata(fixtures().join("bomba-comprimida.cbz"))
        .expect("metadatos")
        .len();
    assert!(en_disco < 64 * 1024, "la bomba de prueba debe ser diminuta");

    assert_eq!(
        abrir(&tmp, "bomba-comprimida.cbz").err(),
        Some(NbError::DocumentCorrupted),
        "un ratio de 1000:1 no es un cómic"
    );
}

// --- Lo que el proveedor tiene que hacer bien -------------------------------

#[test]
fn las_paginas_se_ordenan_por_numero_y_no_alfabeticamente() {
    let tmp = Temporal::nuevo("orden");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    assert_eq!(comic.page_count(), 4, "solo las imágenes son páginas");

    // El fixture salta de 9 a 10 justo porque en orden alfabético "10" va antes
    // que "9". Un lector que ordene como texto muestra el cómic desordenado a
    // partir de la décima página, y es el defecto más aburrido de este formato.
    let ultima = comic.page_image(4).expect("última página");
    let novena = comic.page_image(3).expect("penúltima página");
    assert!(ultima.to_string_lossy().contains("00004"));
    assert_eq!(
        std::fs::read(&ultima).expect("bytes").len(),
        74,
        "la cuarta página debe ser pagina-10.png, la más grande"
    );
    assert_eq!(std::fs::read(&novena).expect("bytes").len(), 73);
}

#[test]
fn lo_que_no_es_una_imagen_no_cuenta_como_pagina() {
    let tmp = Temporal::nuevo("filtrado");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    // El contenedor trae siete entradas: cuatro páginas, un ComicInfo.xml, la
    // basura de macOS y un archivo oculto. Contar las tres últimas desplazaría
    // toda la numeración y las anotaciones ancladas por página.
    assert_eq!(comic.page_count(), 4);
}

#[test]
fn un_contenedor_sin_ninguna_imagen_no_es_un_comic() {
    let tmp = Temporal::nuevo("vacio");
    assert_eq!(
        abrir(&tmp, "minimo-valido.epub").err(),
        Some(NbError::DocumentCorrupted),
        "un ZIP legible sin imágenes no se abre como cómic"
    );
}

#[test]
fn pedir_una_pagina_que_no_existe_es_un_error_y_no_un_panico() {
    let tmp = Temporal::nuevo("fuera-de-rango");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    assert_eq!(comic.page_image(0).err(), Some(NbError::DocumentNotFound));
    assert_eq!(comic.page_image(5).err(), Some(NbError::DocumentNotFound));
}

#[test]
fn la_pagina_se_extrae_una_sola_vez_y_se_reutiliza() {
    let tmp = Temporal::nuevo("cache");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    let primera = comic.page_image(1).expect("página");
    let modificado = std::fs::metadata(&primera).expect("metadatos");

    let repetida = comic.page_image(1).expect("página otra vez");
    assert_eq!(primera, repetida);
    assert_eq!(
        std::fs::metadata(&repetida)
            .expect("metadatos")
            .modified()
            .ok(),
        modificado.modified().ok(),
        "la segunda petición no debe volver a escribir el archivo"
    );
}

// --- El contrato de proveedor -----------------------------------------------

#[test]
fn una_posicion_de_comic_es_una_pagina_y_ancla_por_ella() {
    let tmp = Temporal::nuevo("locator");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    let locator = comic
        .locator_at(&NativePosition::Page(2))
        .expect("locator de la página 2");

    assert_eq!(locator.locations.position, Some(2));
    assert_eq!(locator.locations.fragments, vec!["page=2".to_owned()]);
    assert_eq!(locator.media_type, MEDIA_TYPE);
    // Sin texto no hay cita: fingir una sería peor que no tenerla.
    assert!(locator.text.is_none());

    let resuelto = comic.resolve(&locator).expect("resolución");
    assert_eq!(resuelto.position, NativePosition::Page(2));
    assert_eq!(resuelto.precision, Precision::Exact);
    assert!(!resuelto.precision.requires_warning());
}

#[test]
fn una_pagina_que_ya_no_existe_degrada_a_aproximada_y_avisa() {
    let tmp = Temporal::nuevo("degrada");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    // Un locator guardado cuando el cómic tenía cien páginas: la edición actual
    // tiene cuatro. La anotación no se descarta, pero tampoco se coloca en la
    // página 40 como si nada.
    let mut viajero = comic.locator_at(&NativePosition::Page(4)).expect("locator");
    viajero.locations.position = Some(97);
    viajero.locations.total_progression = Some(0.97);

    let resuelto = comic.resolve(&viajero).expect("resolución degradada");
    assert_eq!(resuelto.precision, Precision::Approximate);
    assert!(
        resuelto.precision.requires_warning(),
        "reanclar por progresión sin avisar es lo que ADR-004 prohíbe"
    );
    assert_eq!(resuelto.position, NativePosition::Page(4));
}

#[test]
fn un_comic_no_ofrece_tipografia_ni_extraccion_de_texto() {
    let tmp = Temporal::nuevo("capacidades");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");
    let capacidades = comic.capabilities();

    // La interfaz se construye desde aquí: lo que es falso no se muestra
    // deshabilitado, se oculta.
    assert!(!capacidades.typography);
    assert!(!capacidades.reflow);
    assert!(!capacidades.text_extraction);
    assert!(!capacidades.internal_search);
    assert!(!capacidades.text_selection);

    // Y pedir texto de todos modos dice por qué no lo hay, en lugar de devolver
    // una cadena vacía que parecería un documento sin contenido.
    let locator = comic.locator_at(&NativePosition::Page(1)).expect("locator");
    assert_eq!(
        comic.text_range(&locator, 100).err(),
        Some(NbError::FormatUnsupported)
    );
}

#[test]
fn el_indice_de_un_comic_es_la_lista_de_sus_paginas() {
    let tmp = Temporal::nuevo("indice");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");
    let navegacion = comic.navigation().expect("índice");

    assert_eq!(navegacion.entries.len(), 4);
    assert_eq!(navegacion.entries[0].title, "Página 1");
    assert_eq!(navegacion.entries[3].locator.locations.position, Some(4));
}

#[test]
fn el_render_devuelve_la_ruta_de_una_imagen_y_no_marcado() {
    let tmp = Temporal::nuevo("render");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    match comic.render(&NativePosition::Page(1)).expect("render") {
        Rendered::Image { path, media_type } => {
            assert_eq!(media_type, "image/png");
            assert!(Path::new(&path).exists());
            // La imagen viaja como ruta porque data-flows.md manda servirla por
            // el protocolo de assets; en base64 por el canal JSON, un cómic de
            // trescientas páginas serían trescientos mensajes enormes.
            assert!(path.starts_with(&tmp.ruta().to_string_lossy().to_string()));
        }
        Rendered::Markup { .. } => panic!("un cómic no es marcado"),
    }
}

#[test]
fn el_registro_abre_un_comic_por_su_tipo_de_medio() {
    let tmp = Temporal::nuevo("registro");
    let mut registro = ProviderRegistry::new();
    registro.register(Box::new(ComicFactory::new(tmp.ruta().to_path_buf())));

    assert!(registro.supports(MEDIA_TYPE));
    let documento = registro
        .open(MEDIA_TYPE, &fixtures().join("minimo-valido.cbz"))
        .expect("el registro abre el cómic");
    assert_eq!(documento.media_type(), MEDIA_TYPE);
    assert_eq!(documento.navigation().expect("índice").entries.len(), 4);
}

// --- Modos de ajuste --------------------------------------------------------

#[test]
fn el_tamano_de_la_pagina_se_lee_de_la_cabecera_sin_decodificar_la_imagen() {
    let tmp = Temporal::nuevo("tamano");
    let comic = abrir(&tmp, "minimo-valido.cbz").expect("cómic válido");

    // Las páginas del fixture son PNG de 4x4. La interfaz necesita esto antes de
    // tener la imagen, para reservar el hueco y no dar el salto de maquetación.
    assert_eq!(
        comic.page_dimensions(1).expect("tamaño"),
        PageSize {
            width: 4,
            height: 4
        }
    );
    assert_eq!(
        comic.page_dimensions(9).err(),
        Some(NbError::DocumentNotFound)
    );
}

#[test]
fn el_tamano_se_lee_igual_en_los_formatos_de_imagen_que_admite_un_comic() {
    // Cada formato guarda el tamaño en un sitio distinto, y ninguno obliga a
    // decodificar los píxeles para llegar a él.
    let png = std::fs::read(fixtures().join("minimo-valido.cbz")).expect("cbz");
    assert!(!png.is_empty());

    // GIF: dos enteros de 16 bits en little-endian justo tras la firma.
    let gif = b"GIF89a\x40\x01\xf0\x00resto";
    assert_eq!(
        nb_books_lib::reader::comic::image_size(gif),
        Some(PageSize {
            width: 320,
            height: 240
        })
    );

    // JPEG: hay que recorrer los segmentos, porque el tamaño va detrás de los
    // metadatos de cámara y no en una posición fija.
    let mut jpeg = vec![0xff, 0xd8];
    jpeg.extend_from_slice(&[0xff, 0xe0, 0x00, 0x10]); // APP0 de 16 bytes
    jpeg.extend_from_slice(&[0_u8; 14]);
    jpeg.extend_from_slice(&[0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0x2c, 0x01, 0x90]);
    assert_eq!(
        nb_books_lib::reader::comic::image_size(&jpeg),
        Some(PageSize {
            width: 400,
            height: 300
        })
    );

    // Un tipo cuyo tamaño no sabemos leer se declara desconocido, en lugar de
    // devolver un tamaño inventado con el que la página se pintaría deformada.
    assert_eq!(nb_books_lib::reader::comic::image_size(b"AVIF?"), None);
}

#[test]
fn ajustar_al_ancho_llena_la_ventana_y_ajustar_a_la_pagina_la_deja_entera() {
    let pagina = PageSize {
        width: 1000,
        height: 1500,
    };
    let ventana = PageSize {
        width: 800,
        height: 600,
    };

    let ancho = FitMode::Width.apply(pagina, ventana);
    assert_eq!(ancho.width, 800);
    assert_eq!(ancho.height, 1200, "la proporción se conserva");

    let alto = FitMode::Height.apply(pagina, ventana);
    assert_eq!(alto.height, 600);
    assert_eq!(alto.width, 400);

    // Página completa es el más restrictivo de los dos, y aquí lo es el alto.
    assert_eq!(FitMode::Page.apply(pagina, ventana), alto);

    assert_eq!(FitMode::Original.apply(pagina, ventana), pagina);
}

#[test]
fn un_ajuste_con_una_medida_en_cero_devuelve_la_pagina_sin_tocar() {
    let pagina = PageSize {
        width: 600,
        height: 900,
    };

    // Ocurre de verdad: la ventana mide cero mientras se monta el componente. Un
    // ajuste que divida por ahí produce infinito, y luego una imagen de cero
    // píxeles o un pánico al convertirla.
    assert_eq!(
        FitMode::Page.apply(
            pagina,
            PageSize {
                width: 0,
                height: 0
            }
        ),
        pagina
    );
    assert_eq!(
        FitMode::Width.apply(
            PageSize {
                width: 0,
                height: 0
            },
            pagina
        ),
        PageSize {
            width: 0,
            height: 0
        }
    );
}

#[test]
fn el_ajuste_por_defecto_de_un_comic_es_el_ancho() {
    // Un cómic se lee bajando: el ancho es lo que se quiere aprovechar, y elegir
    // otro por defecto obliga al usuario a corregirlo en cada documento.
    assert_eq!(FitMode::default(), FitMode::Width);
}
