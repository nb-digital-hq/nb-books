//! Criterio de aceptación de los diccionarios.
//!
//! "Un diccionario corrupto o truncado se rechaza al instalarlo y no deja la
//! consulta rota; desinstalar uno no deja rastro en las preferencias."
//!
//! Los diccionarios de prueba se **generan aquí**, no se versionan: así se puede
//! construir la variante rota de cada caso, que es justo lo que hay que
//! comprobar, y no entra ni un binario al repositorio.

use std::io::Write;
use std::path::{Path, PathBuf};

use nb_books_lib::error::NbError;
use nb_books_lib::lookup::store::{self, Dictionary};
use nb_books_lib::lookup::{LookupProvider, LookupRegistry, Query, SkipReason};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-dicc-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("origen")).expect("origen");
        std::fs::create_dir_all(dir.join("instalados")).expect("instalados");
        Self(dir)
    }
    fn origen(&self) -> PathBuf {
        self.0.join("origen")
    }
    fn instalados(&self) -> PathBuf {
        self.0.join("instalados")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Cómo se estropea un diccionario, para poder comprobar que se rechaza.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Averia {
    Ninguna,
    /// El `.ifo` dice más palabras de las que trae el índice.
    CuentaQueNoCuadra,
    /// El índice apunta más allá del final de las definiciones.
    IndiceQueApuntaFuera,
    /// Falta el archivo de definiciones.
    SinDefiniciones,
    /// El `.ifo` no lleva su firma.
    SinFirma,
    /// Declara un tipo de contenido que no sabemos convertir.
    TipoDesconocido,
    /// El índice está cortado a la mitad de una entrada.
    IndiceTruncado,
}

/// Escribe un diccionario StarDict en disco y devuelve la ruta de su `.ifo`.
fn escribir_diccionario(
    carpeta: &Path,
    nombre: &str,
    entradas: &[(&str, &str)],
    averia: Averia,
    comprimir: bool,
) -> PathBuf {
    let base = carpeta.join(nombre);

    let mut dict = Vec::new();
    let mut idx = Vec::new();
    for (palabra, definicion) in entradas {
        let offset = u32::try_from(dict.len()).expect("cabe");
        dict.extend_from_slice(definicion.as_bytes());
        let size = u32::try_from(definicion.len()).expect("cabe");

        idx.extend_from_slice(palabra.as_bytes());
        idx.push(0);
        let desplazamiento = if averia == Averia::IndiceQueApuntaFuera {
            offset + 10_000
        } else {
            offset
        };
        idx.extend_from_slice(&desplazamiento.to_be_bytes());
        idx.extend_from_slice(&size.to_be_bytes());
    }

    if averia == Averia::IndiceTruncado {
        idx.truncate(idx.len() - 3);
    }

    let firma = if averia == Averia::SinFirma {
        "No es un ifo"
    } else {
        "StarDict's dict ifo file"
    };
    let tipo = if averia == Averia::TipoDesconocido {
        "z"
    } else {
        "m"
    };
    let cuenta = if averia == Averia::CuentaQueNoCuadra {
        entradas.len() + 3
    } else {
        entradas.len()
    };

    let ifo = format!(
        "{firma}\nversion=3.0.0\nbookname={nombre}\nwordcount={cuenta}\n\
         idxfilesize={}\nsametypesequence={tipo}\nlang=es\n",
        idx.len()
    );
    std::fs::write(base.with_extension("ifo"), ifo).expect("ifo");
    std::fs::write(base.with_extension("idx"), &idx).expect("idx");

    if averia != Averia::SinDefiniciones {
        if comprimir {
            let archivo = std::fs::File::create(base.with_extension("dict.dz")).expect("dz");
            let mut codificador =
                flate2::write::GzEncoder::new(archivo, flate2::Compression::default());
            codificador.write_all(&dict).expect("comprimido");
            codificador.finish().expect("cerrado");
        } else {
            std::fs::write(base.with_extension("dict"), &dict).expect("dict");
        }
    }

    base.with_extension("ifo")
}

fn diccionario_bueno(carpeta: &Path) -> PathBuf {
    escribir_diccionario(
        carpeta,
        "Ejemplo",
        &[
            ("agua", "Sustancia líquida."),
            ("Casa", "Edificio para habitar."),
            ("casa", "Segunda acepción de casa."),
            ("zorro", "Mamífero carnívoro."),
        ],
        Averia::Ninguna,
        false,
    )
}

#[test]
fn un_diccionario_valido_se_instala_y_responde() {
    let tmp = Temporal::nuevo("valido");
    let ifo = diccionario_bueno(&tmp.origen());

    let manifest = store::install(&tmp.instalados(), &ifo, None).expect("instalado");
    assert_eq!(manifest.word_count, 4);
    assert_eq!(manifest.language, "es");

    let diccionario = Dictionary::open(&tmp.instalados(), &manifest.id).expect("abierto");
    let entradas = diccionario
        .lookup(&Query {
            term: "agua".to_owned(),
            ..Query::default()
        })
        .expect("consulta");

    assert_eq!(entradas.len(), 1);
    assert_eq!(entradas[0].body, "Sustancia líquida.");
}

/// Buscar no puede depender de cómo escribió el usuario, ni de cómo escribió el
/// lexicógrafo: un diccionario que solo responde a la forma exacta no sirve
/// dentro de un lector, donde la palabra viene del texto tal cual estaba.
#[test]
fn la_busqueda_no_distingue_mayusculas_y_devuelve_todas_las_acepciones() {
    let tmp = Temporal::nuevo("mayusculas");
    let ifo = diccionario_bueno(&tmp.origen());
    let manifest = store::install(&tmp.instalados(), &ifo, None).expect("instalado");
    let diccionario = Dictionary::open(&tmp.instalados(), &manifest.id).expect("abierto");

    for forma in ["casa", "Casa", "CASA", "  casa  "] {
        let entradas = diccionario
            .lookup(&Query {
                term: forma.to_owned(),
                ..Query::default()
            })
            .expect("consulta");
        assert_eq!(entradas.len(), 2, "fallo con la forma {forma:?}");
    }
}

/// El `.dict.dz` es gzip válido, así que se lee con lo que ya hay en el árbol.
#[test]
fn un_diccionario_comprimido_se_instala_igual() {
    let tmp = Temporal::nuevo("comprimido");
    let ifo = escribir_diccionario(
        &tmp.origen(),
        "Comprimido",
        &[("zorro", "Mamífero carnívoro.")],
        Averia::Ninguna,
        true,
    );

    let manifest = store::install(&tmp.instalados(), &ifo, None).expect("instalado");
    let diccionario = Dictionary::open(&tmp.instalados(), &manifest.id).expect("abierto");
    let entradas = diccionario
        .lookup(&Query {
            term: "zorro".to_owned(),
            ..Query::default()
        })
        .expect("consulta");

    assert_eq!(entradas[0].body, "Mamífero carnívoro.");
}

/// **El criterio, primera mitad.** Cada avería se rechaza **al instalar**, y no
/// deja media instalación detrás.
#[test]
fn un_diccionario_roto_se_rechaza_al_instalarlo_y_no_deja_nada() {
    let averias = [
        Averia::CuentaQueNoCuadra,
        Averia::IndiceQueApuntaFuera,
        Averia::SinDefiniciones,
        Averia::SinFirma,
        Averia::TipoDesconocido,
        Averia::IndiceTruncado,
    ];

    for (i, averia) in averias.into_iter().enumerate() {
        let tmp = Temporal::nuevo(&format!("roto-{i}"));
        let ifo = escribir_diccionario(
            &tmp.origen(),
            "Roto",
            &[("agua", "Sustancia líquida.")],
            averia,
            false,
        );

        let resultado = store::install(&tmp.instalados(), &ifo, None);
        assert!(
            resultado.is_err(),
            "la avería {i} debía rechazarse al instalar"
        );

        // Y sobre todo: no queda nada instalado ni a medias.
        assert!(
            store::list(&tmp.instalados()).expect("listado").is_empty(),
            "la avería {i} dejó algo instalado"
        );
        let restos: Vec<_> = std::fs::read_dir(tmp.instalados())
            .expect("carpeta")
            .filter_map(Result::ok)
            .collect();
        assert!(restos.is_empty(), "la avería {i} dejó restos en disco");
    }
}

/// **El criterio, segunda mitad.** La lista de instalados **es** la carpeta, así
/// que desinstalar no puede dejar rastro en ningún otro sitio.
#[test]
fn desinstalar_no_deja_rastro() {
    let tmp = Temporal::nuevo("desinstalar");
    let ifo = diccionario_bueno(&tmp.origen());
    let manifest = store::install(&tmp.instalados(), &ifo, None).expect("instalado");

    assert_eq!(store::list(&tmp.instalados()).expect("listado").len(), 1);

    store::uninstall(&tmp.instalados(), &manifest.id).expect("desinstalado");

    assert!(store::list(&tmp.instalados()).expect("listado").is_empty());
    assert!(
        std::fs::read_dir(tmp.instalados())
            .expect("carpeta")
            .next()
            .is_none(),
        "quedó algo en la carpeta de diccionarios"
    );
    // Y desinstalar dos veces tampoco es un error.
    store::uninstall(&tmp.instalados(), &manifest.id).expect("idempotente");

    assert!(matches!(
        Dictionary::open(&tmp.instalados(), &manifest.id),
        Err(NbError::ComponentUnavailable)
    ));
}

/// Una carpeta con una instalación interrumpida no puede inutilizar el listado:
/// se ignora lo que no tiene manifiesto legible.
#[test]
fn una_instalacion_a_medias_se_ignora_en_lugar_de_romper_el_listado() {
    let tmp = Temporal::nuevo("a-medias");
    let ifo = diccionario_bueno(&tmp.origen());
    store::install(&tmp.instalados(), &ifo, None).expect("instalado");

    std::fs::create_dir_all(tmp.instalados().join("basura")).expect("basura");
    std::fs::write(tmp.instalados().join("basura/manifest.json"), "{ roto").expect("manifiesto");

    assert_eq!(
        store::list(&tmp.instalados()).expect("listado").len(),
        1,
        "el listado debe ignorar lo que no sabe leer"
    );
}

/// El diccionario instalado es un proveedor del contrato: el registro
/// lo trata como a cualquier otro, y lo omite si el documento está en otro
/// idioma.
#[test]
fn un_diccionario_instalado_es_un_proveedor_mas_del_registro() {
    let tmp = Temporal::nuevo("registro");
    let ifo = diccionario_bueno(&tmp.origen());
    let manifest = store::install(&tmp.instalados(), &ifo, None).expect("instalado");

    let mut registro = LookupRegistry::new();
    registro.register(Box::new(
        Dictionary::open(&tmp.instalados(), &manifest.id).expect("abierto"),
    ));

    let en_espanol = registro
        .lookup(
            &Query {
                term: "agua".to_owned(),
                context: None,
                language: Some("es-CO".to_owned()),
            },
            false,
        )
        .expect("consulta");
    assert_eq!(en_espanol.entries.len(), 1);
    assert!(en_espanol.skipped.is_empty());

    let en_aleman = registro
        .lookup(
            &Query {
                term: "agua".to_owned(),
                context: None,
                language: Some("de".to_owned()),
            },
            false,
        )
        .expect("consulta");
    assert!(en_aleman.entries.is_empty());
    assert_eq!(en_aleman.skipped[0].reason, SkipReason::LanguageUnsupported);
}

/// Un diccionario sin idioma declarado y sin que nadie lo indique no se instala:
/// sin idioma no hay forma de decidir cuándo ofrecerlo, y acabaría respondiendo
/// a documentos de cualquier lengua.
#[test]
fn sin_idioma_no_se_instala() {
    let tmp = Temporal::nuevo("sin-idioma");
    let base = tmp.origen().join("Anonimo");
    let mut idx = Vec::new();
    idx.extend_from_slice(b"agua\0");
    idx.extend_from_slice(&0u32.to_be_bytes());
    idx.extend_from_slice(&5u32.to_be_bytes());

    std::fs::write(
        base.with_extension("ifo"),
        format!(
            "StarDict's dict ifo file\nversion=3.0.0\nbookname=Anonimo\nwordcount=1\n\
             idxfilesize={}\nsametypesequence=m\n",
            idx.len()
        ),
    )
    .expect("ifo");
    std::fs::write(base.with_extension("idx"), &idx).expect("idx");
    std::fs::write(base.with_extension("dict"), b"agua!").expect("dict");

    assert!(store::install(&tmp.instalados(), &base.with_extension("ifo"), None).is_err());

    // Pero si el usuario lo indica al instalar, entra.
    let manifest = store::install(&tmp.instalados(), &base.with_extension("ifo"), Some("en"))
        .expect("instalado con idioma indicado");
    assert_eq!(manifest.language, "en");
}
