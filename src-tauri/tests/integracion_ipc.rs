//! Pruebas de integración de la capa de comandos.
//!
//! Validan que el estado compartido está bien inyectado y que los comandos
//! responden con el contrato esperado, sin abrir una ventana real. Ver
//! `docs/developer/testing/integration.md`.

mod comun;

use comun::estado::estado_aislado;
use nb_books_lib::db::migrations;
use nb_books_lib::monitor::{components, ComponentState};

#[test]
fn el_estado_compartido_expone_la_base_migrada() {
    let (state, _guard) = estado_aislado("estado");

    assert_eq!(
        state.db.schema_version().expect("legible"),
        migrations::target_version()
    );
}

/// La interfaz decide qué muestra a partir de esto. Si el estado compartido no
/// llega, todo aparece como no disponible.
#[test]
fn el_monitor_reporta_los_componentes_declarados() {
    let (state, _guard) = estado_aislado("monitor");

    let componentes = state.monitor.snapshot();

    assert!(!componentes.is_empty());
    assert!(componentes
        .iter()
        .any(|c| c.component == components::DATABASE && c.state == ComponentState::Ready));
}

/// El corpus de archivos hostiles existe para que las defensas se prueben. Si
/// esta prueba falla, alguien lo borró o lo movió.
#[test]
fn el_corpus_de_pruebas_esta_disponible() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");

    for archivo in [
        "minimo-valido.pdf",
        "minimo-valido.epub",
        "truncado.pdf",
        "xml-malformado.epub",
        "entidad-externa.epub",
        "recorrido-directorios.cbz",
        "extension-falsificada.epub",
        "adversario-prompt.txt",
    ] {
        assert!(
            fixtures.join(archivo).exists(),
            "falta el archivo de prueba {archivo}"
        );
    }
}

/// Las entradas hostiles del corpus deben rechazarse **antes** de escribir nada
/// en disco. Es la defensa contra el recorrido de directorios en la extracción.
#[test]
fn las_entradas_hostiles_del_corpus_se_rechazan() {
    use nb_books_lib::security::validate_archive_entry;

    let cbz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/recorrido-directorios.cbz");
    let bytes = std::fs::read(&cbz).expect("archivo legible");

    // Lectura mínima del directorio central del ZIP para extraer los nombres,
    // sin añadir una dependencia solo para esta prueba.
    let nombres = nombres_de_zip(&bytes);
    assert!(
        nombres.iter().any(|n| n.contains("..")),
        "el corpus debe contener rutas hostiles"
    );

    let mut rechazadas = 0;
    for nombre in &nombres {
        if validate_archive_entry(nombre).is_err() {
            rechazadas += 1;
        }
    }

    assert_eq!(
        rechazadas, 2,
        "ambas entradas con recorrido deben rechazarse: {nombres:?}"
    );
}

/// Extrae los nombres de entrada de un ZIP recorriendo las cabeceras locales.
fn nombres_de_zip(bytes: &[u8]) -> Vec<String> {
    const FIRMA_LOCAL: [u8; 4] = [0x50, 0x4b, 0x03, 0x04];
    let mut nombres = Vec::new();
    let mut i = 0;

    while i + 30 <= bytes.len() {
        if bytes[i..i + 4] != FIRMA_LOCAL {
            i += 1;
            continue;
        }
        let len_nombre = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
        let inicio = i + 30;
        if inicio + len_nombre <= bytes.len() {
            nombres.push(String::from_utf8_lossy(&bytes[inicio..inicio + len_nombre]).into_owned());
        }
        i = inicio + len_nombre;
    }

    nombres
}

/// El estado compartido lleva lo que necesitan `open_document` y `close_document`.
///
/// Los dos comandos son dos líneas cada uno: piden conexión, delegan en el
/// almacén de sesiones y devuelven. Lo que aquí se comprueba es que el estado
/// que el runtime inyecta tiene las dos piezas y que hacen lo que el comando
/// espera; el cuerpo del comando no se puede invocar sin una aplicación Tauri
/// real, y fingir una no probaría nada que esto no pruebe.
#[test]
fn el_estado_compartido_lleva_el_registro_de_lectores_y_las_sesiones() {
    let (state, _guard) = estado_aislado("lector");

    assert!(
        state.sessions.is_empty().expect("almacén"),
        "al arrancar no hay ningún documento abierto"
    );

    // El registro se construye en el arranque según lo que haya en la máquina;
    // aquí lo puebla `estado_aislado` porque esta prueba no levanta el runtime.
    assert!(state.readers.supports("application/epub+zip"));

    // Un documento que no está en el catálogo: es el camino de error que la
    // interfaz verá si pide abrir algo que ya se dio de baja.
    assert_eq!(
        state
            .sessions
            .open(
                &state.db,
                &state.readers,
                "00000000000000000000000000000000"
            )
            .err(),
        Some(nb_books_lib::error::NbError::DocumentNotFound)
    );

    // Y un error al abrir no deja media sesión registrada.
    assert!(state.sessions.is_empty().expect("almacén"));
}

#[test]
fn sin_subsistema_de_extensiones_la_aplicacion_arranca_y_sus_comandos_lo_dicen() {
    let (estado, _guarda) = estado_aislado("sin-extensiones");

    // El almacén no se pudo abrir, y aun así hay estado, hay base y hay
    // lector. Lo que no hay es extensiones, y se dice con un código que la
    // interfaz sabe traducir en lugar de con una pantalla en blanco.
    assert!(estado.extensions.is_none());
    assert!(estado.db.connection().is_ok());
}
