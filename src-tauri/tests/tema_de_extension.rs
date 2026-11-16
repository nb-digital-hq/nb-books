//! Un tema aportado por una extensión.
//!
//! El criterio tiene dos mitades y esta prueba cubre la primera: **un tema no
//! puede pedir permisos de red ni de disco**. Aquí se va más lejos y tampoco
//! puede traer nada ejecutable, porque un tema que trae código no es un tema.
//!
//! La segunda mitad —que no pueda apagar el foco ni romper el contraste— vive
//! en el frontend, en `temaDeExtension.ts`, y por un motivo: la aritmética del
//! contraste y sus dos umbrales están en `contraste.ts`, que es de donde los
//! toma el sistema de diseño. Reimplementarlos aquí habría creado una segunda
//! fuente para un número que tiene que ser uno solo.
//!
//! Y hay una tercera garantía que no necesita prueba de contraste porque es
//! estructural: **no existe un token para apagar el foco**. Lo más que un tema
//! puede hacer es darle un color.

use nb_books_lib::extensions::manifest::{self, DefectCode, TOKENS_DE_TEMA};

/// El tema de ejemplo que vive en el repositorio.
fn tema_de_ejemplo() -> String {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("raíz")
        .join("extensiones")
        .join("tema-de-prueba")
        .join("nb-extension.json");
    std::fs::read_to_string(ruta).expect("el tema de ejemplo")
}

fn codigos(defectos: &[manifest::ManifestDefect]) -> Vec<(DefectCode, &str)> {
    defectos
        .iter()
        .map(|d| (d.code, d.field.as_str()))
        .collect()
}

#[test]
fn el_tema_de_ejemplo_es_valido_y_no_trae_nada_ejecutable() {
    let manifiesto = manifest::parse(&tema_de_ejemplo()).expect("es válido");

    assert_eq!(manifiesto.kind, manifest::ExtensionKind::Theme);
    assert!(manifiesto.capabilities.is_empty());
    assert!(manifiesto.contributes.providers.is_empty());
    assert!(manifiesto.contributes.commands.is_empty());
    assert!(manifiesto.contributes.views.is_empty());
    assert!(!manifiesto.contributes.theme.is_empty());
}

#[test]
fn un_tema_que_pide_permisos_de_red_no_es_un_tema() {
    let mut roto: serde_json::Value = serde_json::from_str(&tema_de_ejemplo()).expect("json");
    roto["capabilities"] = serde_json::json!(["network:fetch"]);
    roto["permissions"] =
        serde_json::json!({ "network:fetch": { "urls": ["https://ejemplo.org/api"] } });

    let defectos = manifest::parse(&roto.to_string()).expect_err("no puede pasar");

    // El criterio de aceptación, y en su forma fuerte: no se marca para
    // revisión manual, se rechaza. Un tema es declarativo o no es un tema.
    assert!(codigos(&defectos).contains(&(DefectCode::ThemeWithCode, "capabilities")));
}

#[test]
fn un_tema_que_trae_codigo_no_es_un_tema() {
    let mut roto: serde_json::Value = serde_json::from_str(&tema_de_ejemplo()).expect("json");
    roto["contributes"]["providers"] = serde_json::json!({ "view": "modulo.wasm" });
    roto["contributes"]["commands"] = serde_json::json!([{ "id": "x.hacer", "title": "Hacer" }]);

    let defectos = manifest::parse(&roto.to_string()).expect_err("no puede pasar");
    let senalados = codigos(&defectos);

    // Los dos sitios se señalan, no solo el primero: quien lo arregle tiene que
    // ver todo lo que sobra de una vez.
    assert!(senalados.contains(&(DefectCode::ThemeWithCode, "contributes.providers")));
    assert!(senalados.contains(&(DefectCode::ThemeWithCode, "contributes.commands")));
}

#[test]
fn un_tema_solo_puede_cambiar_los_colores_de_la_lista_cerrada() {
    let mut roto: serde_json::Value = serde_json::from_str(&tema_de_ejemplo()).expect("json");
    // Ni una sombra, ni un desenfoque, ni una escala de movimiento: la lista
    // cerrada es lo que impide que un tema toque lo que no es color.
    roto["contributes"]["theme"]["--nb-shadow-lg"] = serde_json::json!("#000000");

    let defectos = manifest::parse(&roto.to_string()).expect_err("no puede pasar");
    assert!(codigos(&defectos).contains(&(
        DefectCode::UnknownThemeToken,
        "contributes.theme.--nb-shadow-lg"
    )));
}

#[test]
fn ningun_token_de_tema_permite_apagar_el_foco_ni_una_sombra() {
    // La garantía es estructural y no de validación: lo que no está en la lista
    // no se puede tocar. Esta prueba la fija, de modo que añadir un token que
    // no sea un color obligue a pasar por aquí.
    for token in TOKENS_DE_TEMA {
        assert!(
            !token.contains("shadow")
                && !token.contains("blur")
                && !token.contains("escala")
                && !token.contains("font"),
            "{token} no es un color y no puede estar en la lista de un tema"
        );
    }
}

#[test]
fn un_color_que_no_es_hexadecimal_de_seis_digitos_no_vale() {
    let mut roto: serde_json::Value = serde_json::from_str(&tema_de_ejemplo()).expect("json");
    // Ocho dígitos son seis y transparencia, y un token translúcido rompe el
    // contraste sin que se vea en el valor.
    roto["contributes"]["theme"]["--nb-surface"] = serde_json::json!("#161a2280");

    let defectos = manifest::parse(&roto.to_string()).expect_err("no puede pasar");
    assert!(codigos(&defectos)
        .contains(&(DefectCode::MalformedValue, "contributes.theme.--nb-surface")));
}

#[test]
fn colores_en_algo_que_no_se_declara_tema_se_rechazan() {
    let mut roto: serde_json::Value = serde_json::from_str(&tema_de_ejemplo()).expect("json");
    roto["type"] = serde_json::json!("core");

    let defectos = manifest::parse(&roto.to_string()).expect_err("no puede pasar");
    // O miente sobre lo que es, o se equivocó de clase. Las dos se arreglan
    // igual, y por eso el defecto señala `type`.
    assert!(codigos(&defectos).contains(&(DefectCode::ValueNotAllowed, "type")));
}
