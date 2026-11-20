//! El fondo sonoro para concentrarse.
//!
//! # El criterio, y dónde se comprueba cada parte
//!
//! *"No añade ni un byte de audio al instalador base"*: aquí, contando lo que
//! trae el paquete. *"Desinstalarla no deja rastro"*: aquí, comprobando que no
//! hay nada que guardar. *"Reproducir no degrada el presupuesto de fotograma"*:
//! en `e2e/rendimiento.spec.ts`, que es donde se puede medir un fotograma.

use std::path::{Path, PathBuf};

use nb_books_lib::extensions::manifest::{self, DefectCode, CLASES_DE_RUIDO};

const PAQUETE: &str = "fondo-sonoro";
const QUIEN: &str = "lat.nbdigitallabs.books.sonido.concentracion";

fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri tiene padre")
        .to_path_buf()
}

fn directorio() -> PathBuf {
    raiz_repo()
        .join("extensiones")
        .join("propias")
        .join(PAQUETE)
}

fn manifiesto() -> String {
    std::fs::read_to_string(directorio().join("nb-extension.json")).expect("el paquete existe")
}

fn con_sonidos(sonidos: &str) -> String {
    format!(
        r#"{{
          "id": "com.ejemplo.sonido",
          "name": "Paquete de prueba",
          "version": "1.0.0",
          "type": "ui",
          "apiVersion": "0.1",
          "license": "Apache-2.0",
          "contributes": {{ "soundscapes": {sonidos} }}
        }}"#
    )
}

fn codigos(defectos: &[manifest::ManifestDefect]) -> Vec<(DefectCode, &str)> {
    defectos
        .iter()
        .map(|d| (d.code, d.field.as_str()))
        .collect()
}

/// **La primera mitad del criterio, contada en bytes.**
#[test]
fn el_paquete_no_trae_ni_un_byte_de_audio() {
    let mut pesado = Vec::new();
    let mut total = 0_u64;

    for entrada in std::fs::read_dir(directorio()).expect("el paquete existe") {
        let entrada = entrada.expect("entrada");
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        let tamano = entrada.metadata().expect("metadatos").len();
        total += tamano;

        // Ni audio, ni módulos: lo que aporta es el nombre y qué clase de ruido.
        assert!(
            !nombre.ends_with(".wav")
                && !nombre.ends_with(".mp3")
                && !nombre.ends_with(".ogg")
                && !nombre.ends_with(".flac")
                && !nombre.ends_with(".opus")
                && !nombre.ends_with(".wasm"),
            "{nombre} no debería estar en un paquete que no trae sonido"
        );
        if tamano > 8 * 1024 {
            pesado.push((nombre, tamano));
        }
    }

    assert!(
        pesado.iter().all(|(n, _)| n == "LICENSE"),
        "lo único que puede pesar es la licencia: {pesado:?}"
    );
    // El manifiesto y la licencia. Una grabación de diez minutos pesa más que
    // la aplicación entera.
    assert!(total < 64 * 1024, "el paquete entero pesa {total} bytes");
}

#[test]
fn el_paquete_que_se_publica_ofrece_todos_los_ambientes_que_el_nucleo_sabe_hacer() {
    // Uno por ambiente: si el núcleo aprende a sintetizar algo y el paquete no
    // lo ofrece, nadie puede oírlo. Y al revés lo impide el validador.
    let manifiesto = manifest::parse(&manifiesto()).expect("valida");

    assert_eq!(manifiesto.id, QUIEN);
    let mut clases: Vec<&str> = manifiesto
        .contributes
        .soundscapes
        .iter()
        .map(|s| s.kind.as_str())
        .collect();
    clases.sort_unstable();
    let mut esperadas = CLASES_DE_RUIDO.to_vec();
    esperadas.sort_unstable();
    assert_eq!(clases, esperadas);
    // Y no pide nada: un fondo sonoro no necesita red ni disco.
    assert!(manifiesto.capabilities.is_empty());
    assert!(manifiesto.contributes.providers.is_empty());
}

#[test]
fn una_clase_de_ruido_que_el_nucleo_no_sabe_sintetizar_no_vale() {
    // La lista es cerrada, y es lo que hace que un fondo sonoro no pese: si se
    // admitiera cualquier nombre, el paquete acabaría teniendo que traer el
    // sonido.
    let defectos = manifest::parse(&con_sonidos(
        r#"[{ "id": "x", "name": "X", "kind": "maremoto" }]"#,
    ))
    .expect_err("no puede valer");

    assert_eq!(
        codigos(&defectos),
        vec![(
            DefectCode::ValueNotAllowed,
            "contributes.soundscapes[0].kind"
        )]
    );
}

#[test]
fn un_fondo_sin_nombre_no_se_puede_ofrecer() {
    let defectos = manifest::parse(&con_sonidos(r#"[{ "id": "x", "kind": "rosa" }]"#))
        .expect_err("no puede valer");

    assert_eq!(
        codigos(&defectos),
        vec![(DefectCode::MissingField, "contributes.soundscapes[0].name")]
    );
}

#[test]
fn un_fondo_sonoro_puede_ir_en_cualquier_clase_de_extension() {
    // Al revés que los modos de lectura, y la diferencia tiene motivo: un modo
    // decide con qué colores se lee, y un fondo sonoro no decide nada. Lo único
    // que puede hacer es sonar, y solo cuando alguien lo pone.
    let manifiesto = con_sonidos(r#"[{ "id": "x", "name": "X", "kind": "rosa" }]"#)
        .replace(r#""type": "ui""#, r#""type": "core""#);

    assert!(manifest::parse(&manifiesto).is_ok());
}

#[test]
fn el_paquete_pasa_la_puerta_de_revision_del_registro() {
    let informe = nb_books_lib::extensions::review::revisar(
        &directorio(),
        nb_books_lib::extensions::review::Opciones {
            de_primera_parte: true,
            exige_firma: false,
        },
    )
    .expect("se puede revisar");

    assert!(
        informe.pasa(),
        "el paquete propio no pasa su propia revisión: {:?}",
        informe.hallazgos
    );
    assert_eq!(informe.modulos, 0);
}
