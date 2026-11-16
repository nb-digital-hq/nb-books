//! **El criterio de aceptación:** el paquete de diagnóstico no
//! contiene rutas de usuario, títulos ni secretos.
//!
//! # Por qué la prueba parte de un registro sucio a propósito
//!
//! Porque comprobarlo sobre un registro limpio no comprueba nada. El paquete se
//! monta aquí a partir de un archivo que lleva dentro exactamente lo que no puede
//! salir —la ruta del usuario, el nombre de su cuenta, títulos de sus libros, una
//! clave de API y su correo—, y lo que se afirma es que **nada de eso aparece en
//! lo que se entrega**.
//!
//! Es la segunda red. La primera son `redact_path` y `redact_text` en el punto
//! donde se escribe cada línea, y el propio contrato del proyecto la declara
//! **parcial**: nada obliga a llamarlas. La consecuencia de que falle una vez es
//! que alguien mande su historial de lectura a un desconocido creyendo que manda
//! un informe técnico, y eso no se deshace.
//!
//! Contrato: `docs/audit/privacy.md`.

use std::path::PathBuf;

use nb_books_lib::diagnostics;
use nb_books_lib::monitor::{ComponentState, ComponentStatus};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-diag-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un registro con todo lo que no puede salir del equipo.
const REGISTRO_SUCIO: &str = r#"2026-08-28T09:00:00Z  INFO nb_books_lib: arrancando
2026-08-28T09:00:01Z  INFO nb_books_lib::library: importando /home/ana/Documentos/Biblioteca/Ulises.epub
2026-08-28T09:00:02Z  WARN nb_books_lib::epub: metadatos incompletos en C:\Users\Ana\Libros\Rayuela.epub
2026-08-28T09:00:03Z  INFO nb_books_lib::net: api_key=sk-1234567890abcdef
2026-08-28T09:00:04Z  INFO nb_books_lib::library: usuario ana.perez@universidad.edu
2026-08-28T09:00:05Z  INFO nb_books_lib::db: esquema al día; schema=13
2026-08-28T09:00:06Z  WARN nb_books_lib::reader: no se pudo abrir; code=DOCUMENT_CORRUPTED
2026-08-28T09:00:07Z  INFO nb_books_lib::library: importado <ruta ext=epub len=42>
"#;

/// Lo que **nunca** puede aparecer en el paquete.
const PROHIBIDO: &[&str] = &[
    "ana",
    "Ana",
    "Ulises",
    "Rayuela",
    "sk-1234567890abcdef",
    "universidad.edu",
    "Documentos/Biblioteca",
];

fn componentes() -> Vec<ComponentStatus> {
    vec![
        ComponentStatus {
            component: "database",
            state: ComponentState::Ready,
            reason_code: None,
        },
        ComponentStatus {
            component: "reader",
            state: ComponentState::Failed,
            reason_code: Some("ENGINE_CRASH"),
        },
    ]
}

/// **El criterio.**
#[test]
fn el_paquete_no_lleva_rutas_ni_titulos_ni_secretos() {
    let temporal = Temporal::nuevo("sucio");
    std::fs::write(temporal.0.join("nb-books.log.2026-08-28"), REGISTRO_SUCIO).expect("registro");

    let paquete = diagnostics::collect(&temporal.0, 13, componentes(), None).expect("paquete");
    let texto = diagnostics::render(&paquete).expect("se entrega");

    for prohibido in PROHIBIDO {
        assert!(
            !texto.contains(prohibido),
            "el paquete lleva {prohibido:?}, que identifica al usuario o a lo que lee.\n{texto}"
        );
    }
}

/// Y lo que sí sirve para diagnosticar **se conserva**.
///
/// Un paquete que lo redacta todo es tan inútil como uno que no redacta nada: la
/// diferencia es que el segundo hace daño y el primero solo hace perder el
/// tiempo. Las dos formas de estar mal.
#[test]
fn el_paquete_conserva_lo_que_sirve_para_entender_el_fallo() {
    let temporal = Temporal::nuevo("util");
    std::fs::write(temporal.0.join("nb-books.log.2026-08-28"), REGISTRO_SUCIO).expect("registro");

    let paquete = diagnostics::collect(&temporal.0, 13, componentes(), None).expect("paquete");
    let texto = diagnostics::render(&paquete).expect("se entrega");

    assert!(texto.contains("DOCUMENT_CORRUPTED"), "el código del error");
    assert!(texto.contains("ENGINE_CRASH"), "el motivo del componente");
    assert!(
        texto.contains("\"schemaVersion\": 13"),
        "la versión de esquema"
    );
    assert!(texto.contains(std::env::consts::OS), "la plataforma");
    assert!(
        texto.contains("<ruta ext=epub len=42>"),
        "una ruta ya redactada se conserva tal cual: dice la extensión, que sirve"
    );
    assert!(
        texto.contains("línea retirada"),
        "y las que se retiraron lo dicen, en lugar de dejar un hueco silencioso"
    );
}

/// Sin registros que leer, el paquete sigue sirviendo.
///
/// Negarse a generar nada porque falta una parte deja sin poder contar el
/// problema justo a quien lo tiene. La versión, el esquema y los componentes ya
/// explican bastante.
#[test]
fn sin_registro_el_paquete_se_genera_igual() {
    let temporal = Temporal::nuevo("vacio");

    let paquete = diagnostics::collect(&temporal.0, 13, componentes(), None).expect("paquete");
    assert!(paquete.log.is_empty());
    assert_eq!(paquete.schema_version, 13);
    assert_eq!(paquete.components.len(), 2);

    let texto = diagnostics::render(&paquete).expect("se entrega igual");
    assert!(texto.contains("ENGINE_CRASH"));
}

/// Un registro enorme se recorta, y **se dice cuánto**.
///
/// Un recorte silencioso hace que quien lee el paquete saque conclusiones de un
/// principio que no es el principio.
#[test]
fn un_registro_enorme_se_recorta_diciendo_cuanto() {
    let temporal = Temporal::nuevo("enorme");
    let lineas: String = (0..2_000)
        .map(|i| format!("2026-08-28T09:00:00Z  INFO nb_books_lib: linea {i}\n"))
        .collect();
    std::fs::write(temporal.0.join("nb-books.log.2026-08-28"), lineas).expect("registro");

    let paquete = diagnostics::collect(&temporal.0, 13, componentes(), None).expect("paquete");

    assert_eq!(paquete.log.len(), 500, "se queda con las últimas");
    assert_eq!(paquete.omitted_lines, 1_500, "y dice cuántas dejó fuera");
    assert!(
        paquete
            .log
            .last()
            .expect("hay líneas")
            .contains("linea 1999"),
        "las últimas son las que explican el fallo, no las primeras"
    );
}

/// Si algo se colara, **no se entrega el paquete**.
///
/// Es la diferencia entre una comprobación y un aviso. Entregar un paquete
/// marcado como sospechoso dejaría la decisión en manos de quien menos
/// información tiene para tomarla.
#[test]
fn si_algo_se_cuela_no_se_entrega_nada() {
    let paquete = diagnostics::DiagnosticsPackage {
        version: "1.0.0".to_owned(),
        schema_version: 13,
        platform: "linux x86_64".to_owned(),
        components: componentes(),
        // Una línea que la redacción de arriba no vio: es lo que esta segunda
        // red existe para atrapar.
        log: vec!["abriendo /home/ana/Libros/Ulises.epub".to_owned()],
        omitted_lines: 0,
        storage: None,
    };

    assert!(
        diagnostics::render(&paquete).is_err(),
        "un paquete con una ruta de usuario dentro no puede entregarse"
    );
}

/// **El criterio**: el presupuesto se puede medir sobre cualquier
/// instalación.
///
/// Un límite escrito que solo se puede comprobar en la máquina de quien lo
/// escribió no es un límite: es una intención. Por eso la medida va dentro del
/// diagnóstico, que es lo que ya sale del equipo cuando su dueño lo pide.
#[test]
fn el_diagnostico_dice_cuanto_ocupa_lo_derivado_y_contra_que_se_compara() {
    let temporal = Temporal::nuevo("almacenamiento");
    let base = temporal.0.join("data");
    std::fs::create_dir_all(&base).expect("directorio de datos");
    // SAFETY: este binario de prueba usa su propio directorio y lo borra.
    unsafe { std::env::set_var(nb_books_lib::paths::TEST_DIR_ENV, &temporal.0) };
    let rutas = nb_books_lib::paths::AppPaths::resolve().expect("rutas");

    let db = nb_books_lib::db::Database::open(&rutas.database()).expect("base");
    let conn = db.connection().expect("conexión");

    let medida = nb_books_lib::diagnostics::medir_almacenamiento(&conn, &rutas)
        .expect("la medida se puede tomar");

    // Sin documentos, lo derivado no se compara con nada: cero y no una
    // división entre cero disfrazada de porcentaje enorme.
    assert_eq!(medida.documents_bytes, 0);
    assert_eq!(medida.percent_of_documents, 0.0);
    assert!(
        medida.derived_bytes > 0,
        "la base recién creada ya ocupa algo: una medida de cero sería falsa"
    );
    assert!(
        (medida.budget_percent - nb_books_lib::diagnostics::PRESUPUESTO_DERIVADO).abs()
            < f64::EPSILON,
        "el paquete tiene que llevar el presupuesto para que la cifra se lea sola"
    );

    // Y el diagnóstico entero sigue pudiendo salir del equipo con la medida
    // dentro: son cuatro números, pero la comprobación se hace sobre el texto
    // final, que es lo que de verdad sale.
    let paquete = nb_books_lib::diagnostics::collect(
        &rutas.logs,
        nb_books_lib::db::migrations::target_version(),
        Vec::new(),
        Some(medida),
    )
    .expect("paquete");
    let texto = nb_books_lib::diagnostics::render(&paquete).expect("se entrega");
    assert!(texto.contains("budgetPercent"));
}
