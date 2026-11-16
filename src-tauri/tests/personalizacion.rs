//! Paquetes de personalización: temas y modos de lectura.
//!
//! # El criterio, en dos mitades
//!
//! *"Un paquete no puede pedir permisos"* —lo mismo que ya exigía la política para
//! los temas, extendido a los modos— y *"instalarlo y quitarlo no deja rastro
//! en las preferencias del usuario"*, que es lo que se comprueba abajo con las
//! preferencias delante.
//!
//! # Por qué el rastro necesita código y no solo cuidado
//!
//! Porque elegir un tema o un modo **se guarda**: si no, se olvidaría al cerrar
//! la aplicación. Lo que se guarda es el identificador de quien lo aporta, así
//! que al desinstalarla esa preferencia señalaría a algo que ya no existe, y
//! eso es exactamente el rastro que el criterio prohíbe.

use std::path::{Path, PathBuf};

use nb_books_lib::db::Database;
use nb_books_lib::extensions::manifest::{self, DefectCode};
use nb_books_lib::extensions::store::Store;
use nb_books_lib::extensions::{permissions, personalization};
use nb_books_lib::settings::{self, SettingKey, SettingValue};

/// El paquete de verdad, el que se publica.
const PAQUETE: &str = "personalizacion";
const QUIEN: &str = "lat.nbdigitallabs.books.personalizacion.papel";

fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri tiene padre")
        .to_path_buf()
}

fn temporal(nombre: &str) -> PathBuf {
    let dir = nb_books_lib::paths::temporal_de_pruebas()
        .join(format!("nb-person-{nombre}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("directorio temporal");
    dir
}

fn manifiesto_del_paquete() -> String {
    std::fs::read_to_string(
        raiz_repo()
            .join("extensiones")
            .join("propias")
            .join(PAQUETE)
            .join("nb-extension.json"),
    )
    .expect("el paquete existe")
}

/// Copia el paquete real a un directorio instalable.
///
/// Se instala **el que se publica**, no uno escrito para la prueba: si el que
/// se distribuye dejara de valer, esta prueba tiene que enterarse.
fn paquete(en: &Path) -> PathBuf {
    let origen = raiz_repo()
        .join("extensiones")
        .join("propias")
        .join(PAQUETE);
    let dir = en.join("paquete");
    std::fs::create_dir_all(&dir).expect("paquete");
    for archivo in ["nb-extension.json", "LICENSE"] {
        std::fs::copy(origen.join(archivo), dir.join(archivo)).expect("copia");
    }
    dir
}

fn codigos(defectos: &[manifest::ManifestDefect]) -> Vec<(DefectCode, &str)> {
    defectos
        .iter()
        .map(|d| (d.code, d.field.as_str()))
        .collect()
}

/// Un manifiesto de tema con los modos que se le pasen.
fn con_modos(modos: &str) -> String {
    format!(
        r#"{{
          "id": "com.ejemplo.modos",
          "name": "Paquete de prueba",
          "version": "1.0.0",
          "type": "theme",
          "apiVersion": "0.1",
          "license": "Apache-2.0",
          "contributes": {{ "readingModes": {modos} }}
        }}"#
    )
}

// ---------------------------------------------------------------------------
// El paquete que se publica
// ---------------------------------------------------------------------------

#[test]
fn el_paquete_que_se_publica_es_valido_y_no_trae_nada_ejecutable() {
    let manifiesto = manifest::parse(&manifiesto_del_paquete()).expect("valida");

    assert_eq!(manifiesto.id, QUIEN);
    assert_eq!(manifiesto.contributes.reading_modes.len(), 3);
    assert!(!manifiesto.contributes.theme.is_empty());
    // Lo que hace que sea un paquete de personalización y no otra cosa.
    assert!(manifiesto.capabilities.is_empty());
    assert!(manifiesto.contributes.providers.is_empty());
    assert!(manifiesto.contributes.commands.is_empty());
    assert!(manifiesto.contributes.views.is_empty());
}

#[test]
fn un_paquete_de_personalizacion_que_pide_permisos_no_es_un_paquete_de_personalizacion() {
    let manifiesto = manifiesto_del_paquete().replace(
        r#""license": "Apache-2.0","#,
        r#""license": "Apache-2.0", "capabilities": ["network:fetch"],
           "permissions": { "network:fetch": { "urls": ["https://ejemplo.invalid/*"] } },"#,
    );

    let defectos = manifest::parse(&manifiesto).expect_err("no puede valer");
    assert!(
        codigos(&defectos).contains(&(DefectCode::ThemeWithCode, "capabilities")),
        "{:?}",
        codigos(&defectos)
    );
}

// ---------------------------------------------------------------------------
// La forma de un modo
// ---------------------------------------------------------------------------

#[test]
fn un_modo_sin_nombre_no_se_puede_ofrecer() {
    let defectos = manifest::parse(&con_modos(
        r##"[{ "id": "x", "background": "#ffffff", "text": "#000000" }]"##,
    ))
    .expect_err("no puede valer");

    assert_eq!(
        codigos(&defectos),
        vec![(DefectCode::MissingField, "contributes.readingModes[0].name")]
    );
}

#[test]
fn un_color_de_modo_que_no_es_hexadecimal_de_seis_digitos_no_vale() {
    // Ocho dígitos llevan transparencia, y un color traslúcido rompe el
    // contraste sin que se vea en el manifiesto: el declarado cumpliría el
    // umbral y el que se pinta, compuesto contra lo que haya detrás, no.
    let defectos = manifest::parse(&con_modos(
        r##"[{ "id": "x", "name": "X", "background": "#ffffffcc", "text": "#000000" }]"##,
    ))
    .expect_err("no puede valer");

    assert_eq!(
        codigos(&defectos),
        vec![(
            DefectCode::MalformedValue,
            "contributes.readingModes[0].background"
        )]
    );
}

#[test]
fn un_brillo_fuera_de_rango_no_vale() {
    // Por debajo del mínimo el texto deja de leerse por mucho que su contraste
    // declarado sea correcto: el brillo emulado se compone **encima**.
    let defectos = manifest::parse(&con_modos(
        r##"[{ "id": "x", "name": "X", "background": "#ffffff", "text": "#000000",
              "brightness": 10 }]"##,
    ))
    .expect_err("no puede valer");

    assert_eq!(
        codigos(&defectos),
        vec![(
            DefectCode::ValueNotAllowed,
            "contributes.readingModes[0].brightness"
        )]
    );
}

#[test]
fn los_modos_de_lectura_solo_van_en_un_paquete_de_personalizacion() {
    // Un modo es declarativo y va donde no puede haber código. Permitirlo en una
    // extensión de clase `core` dejaría que algo con permisos y proveedores
    // decidiera también con qué colores se lee.
    let manifiesto =
        con_modos(r##"[{ "id": "x", "name": "X", "background": "#ffffff", "text": "#000000" }]"##)
            .replace(r#""type": "theme""#, r#""type": "core""#);

    let defectos = manifest::parse(&manifiesto).expect_err("no puede valer");
    assert!(
        codigos(&defectos).contains(&(DefectCode::ValueNotAllowed, "type")),
        "{:?}",
        codigos(&defectos)
    );
}

// ---------------------------------------------------------------------------
// El criterio: quitarlo no deja rastro
// ---------------------------------------------------------------------------

#[test]
fn desinstalar_un_paquete_no_deja_su_tema_ni_su_modo_en_las_preferencias() {
    let raiz = temporal("rastro");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen.instalar(&paquete(&raiz)).expect("instala");

    // El usuario elige su tema y su modo, que es lo que se guarda.
    settings::set(
        &conn,
        SettingKey::ExtensionTheme,
        &SettingValue::Text(QUIEN.to_owned()),
    )
    .expect("tema");
    settings::set(
        &conn,
        SettingKey::BookReadingMode,
        &SettingValue::Text(format!("{QUIEN}/pergamino")),
    )
    .expect("modo");

    assert!(permissions::desinstalar(&conn, &almacen, QUIEN).expect("desinstala"));

    assert_eq!(
        settings::get(&conn, SettingKey::ExtensionTheme).expect("tema"),
        SettingValue::Text(String::new()),
        "el tema elegido no puede seguir apuntando a algo que ya no está"
    );
    assert_eq!(
        settings::get(&conn, SettingKey::BookReadingMode).expect("modo"),
        SettingKey::BookReadingMode.default_value(),
        "y el modo vuelve al del núcleo, no a un nombre que nadie sabe pintar"
    );
}

#[test]
fn desinstalar_un_paquete_no_toca_lo_que_eligio_de_otro() {
    // Se compara con la barra incluida: sin ella, desinstalar
    // `org.ejemplo.tema` se llevaría por delante el modo de
    // `org.ejemplo.temas-oscuros`.
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");

    settings::set(
        &conn,
        SettingKey::ExtensionTheme,
        &SettingValue::Text("org.ejemplo.otro".to_owned()),
    )
    .expect("tema");
    settings::set(
        &conn,
        SettingKey::BookReadingMode,
        &SettingValue::Text("org.ejemplo.temas-oscuros/noche".to_owned()),
    )
    .expect("modo");

    let limpiadas = personalization::olvidar(&conn, "org.ejemplo.temas").expect("olvida");

    assert_eq!(limpiadas, 0);
    assert_eq!(
        settings::get(&conn, SettingKey::BookReadingMode).expect("modo"),
        SettingValue::Text("org.ejemplo.temas-oscuros/noche".to_owned())
    );
}

#[test]
fn reinstalar_no_devuelve_lo_que_estaba_elegido() {
    // Volver a instalar algo no es volver a elegirlo. Reponer la preferencia
    // sola sería decidir por el usuario que quiere otra vez lo que tenía, y eso
    // no se sabe.
    let raiz = temporal("reinstalar");
    let db = Database::open_in_memory().expect("base");
    let conn = db.connection().expect("conexión");
    let almacen = Store::abrir(raiz.join("extensiones")).expect("almacén");
    let paquete = paquete(&raiz);

    almacen.instalar(&paquete).expect("instala");
    settings::set(
        &conn,
        SettingKey::ExtensionTheme,
        &SettingValue::Text(QUIEN.to_owned()),
    )
    .expect("tema");
    permissions::desinstalar(&conn, &almacen, QUIEN).expect("desinstala");
    almacen.instalar(&paquete).expect("reinstala");

    assert_eq!(
        settings::get(&conn, SettingKey::ExtensionTheme).expect("tema"),
        SettingValue::Text(String::new())
    );
}

#[test]
fn el_paquete_pasa_la_puerta_de_revision_del_registro() {
    let raiz = temporal("revision");
    let informe = nb_books_lib::extensions::review::revisar(
        &paquete(&raiz),
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
    assert_eq!(
        informe.modulos, 0,
        "un paquete de personalización no trae ni un artefacto"
    );
}
