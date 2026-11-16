//! Superficie IPC: la frontera de seguridad entre el webview y el runtime.
//!
//! **El webview es una zona no confiable.** Todo dato que cruza hacia Rust se
//! valida en Rust; la validación en TypeScript es experiencia de usuario, nunca
//! seguridad. Las reglas completas están en
//! `docs/architecture/ipc-contracts.md`.
//!
//! Cada comando añadido aquí amplía la superficie de ataque y requiere
//! justificación escrita en el pull request.

use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::annotations::{self, Annotation, Edit, NewAnnotation, OutlineEntry};
use crate::db::catalog::AnnotationKind;
use crate::error::{NbError, NbResult};
use crate::events::DomainEvent;
use crate::home;
use crate::library;
use crate::monitor::ComponentStatus;
use crate::notes;
use crate::reader::{Locator, PageSize, Rendered, Resolution, SessionInfo, TextLayer};
use crate::settings::{self, Setting, SettingKey, SettingValue};
use crate::AppState;

/// Cuántas filas puede traer de una vez una respuesta del puente.
///
/// Es un **techo**, no un tamaño de página: quien pida más recibe esto. Existe
/// porque una lista sin techo la acaba fijando quien llama, y quien llama es el
/// webview, que es zona no confiable. Ver la regla 1 del contrato de carga en
/// `docs/architecture/optimization-catalog.md`, sección 1.3.
const MAXIMO_FILAS: u32 = 500;

/// Recorta a [`MAXIMO_FILAS`] lo que pida el webview, y nunca devuelve cero:
/// pedir cero filas es siempre un error de quien llama, y servirlo dejaría una
/// lista vacía indistinguible de "no hay nada".
const fn acotar(limite: u32) -> u32 {
    if limite == 0 || limite > MAXIMO_FILAS {
        MAXIMO_FILAS
    } else {
        limite
    }
}

/// Informe de estado del runtime.
///
/// Es el contrato mínimo que permite al frontend comprobar que el backend está
/// vivo y que el esquema de base de datos es el que espera.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct HealthReport {
    /// Versión de la aplicación, tomada del manifiesto en tiempo de compilación.
    pub version: &'static str,
    /// Versión del esquema de base de datos aplicado.
    pub schema_version: u32,
}

/// Si el runtime terminó de prepararse.
///
/// La ventana se abre antes de abrir la base, para que una migración larga no
/// deje minutos sin nada en pantalla. Hasta que esto diga que sí, ningún comando
/// tiene estado con el que contestar, y la interfaz espera aquí en lugar de
/// recibir un error por cada llamada. Es el único comando que no toma estado.
#[tauri::command(async)]
pub fn runtime_ready() -> bool {
    crate::runtime_listo()
}

/// Comprobación de vida del runtime.
///
/// # Errors
///
/// Devuelve [`crate::error::NbError::StorageFailure`] si la base de datos no
/// responde, lo que significa que el runtime está vivo pero degradado.
#[tauri::command(async)]
pub fn health_check(state: State<'_, AppState>) -> NbResult<HealthReport> {
    Ok(HealthReport {
        version: env!("CARGO_PKG_VERSION"),
        schema_version: state.db.schema_version()?,
    })
}

/// Si las actualizaciones de esta copia las hace NB Launcher.
///
/// La interfaz lo pregunta antes de ofrecer buscar: en una copia del Launcher no
/// hay nada que buscar aquí, y se dice quién lo hace.
#[tauri::command(async)]
#[must_use]
pub fn update_managed_by_launcher() -> bool {
    crate::actualizacion::gestionada_por_launcher()
}

/// La última versión publicada, si es más nueva que la instalada.
///
/// Lo pide la persona desde Ajustes, y ese gesto es el consentimiento para el
/// propósito `update` en los anfitriones de las publicaciones del proyecto.
///
/// # Errors
///
/// Los de [`crate::actualizacion::buscar`].
#[tauri::command(async)]
pub fn update_check(
    state: State<'_, AppState>,
) -> NbResult<Option<crate::actualizacion::Actualizacion>> {
    crate::actualizacion::buscar(&state.net, &state.db)
}

/// Descarga, comprueba e instala la versión más nueva, y reinicia la aplicación.
///
/// # Errors
///
/// Los de [`crate::actualizacion::instalar`].
#[tauri::command(async)]
pub fn update_install(app: tauri::AppHandle, state: State<'_, AppState>) -> NbResult<()> {
    let temporal = state.paths.cache.join("actualizacion");
    std::fs::create_dir_all(&temporal).map_err(|_| NbError::StorageFailure)?;
    if crate::actualizacion::instalar(&state.net, &state.db, &temporal)? {
        app.restart();
    }
    Ok(())
}

/// Estado de salud de todos los componentes del runtime.
///
/// Es lo que permite a la interfaz decidir qué funciones ofrece: una función
/// cuyo componente no está disponible se **oculta**, no se muestra rota. Ver
/// `docs/architecture/component-model.md`.
///
/// # Errors
///
/// No falla: la ausencia de información sobre un componente es en sí misma
/// información, y devolver un error impediría a la interfaz degradar.
///
/// Cota: los componentes del runtime, que son una lista cerrada en `monitor::components`.
#[tauri::command(async)]
pub fn component_states(state: State<'_, AppState>) -> NbResult<Vec<ComponentStatus>> {
    Ok(state.monitor.snapshot())
}

/// Todas las preferencias con su valor efectivo.
///
/// Es la primera llamada que hace la interfaz tras montar: la caché de arranque
/// del webview pinta antes, y esto la corrige si difiere. Ver
/// `docs/adr/010-settings-persistence.md`.
///
/// # Errors
///
/// Devuelve [`crate::error::NbError::StorageFailure`] si la base no responde.
///
/// Cota: las preferencias declaradas en `SettingKey`, que es un enum cerrado.
#[tauri::command(async)]
pub fn settings_snapshot(state: State<'_, AppState>) -> NbResult<Vec<Setting>> {
    let conn = state.db.connection()?;
    settings::snapshot(&conn)
}

/// Escribe una preferencia y devuelve cómo quedó almacenada.
///
/// Devuelve el valor guardado, y no un acuse vacío, porque la regla 2 de ADR-010
/// dice que ante discrepancia gana SQLite: el frontend reescribe su caché con lo
/// que responde esta llamada, no con lo que creía estar escribiendo.
///
/// # Errors
///
/// - [`crate::error::NbError::Internal`] si la clave no es una preferencia
///   conocida o el valor no tiene el tipo que esa preferencia declara. El
///   webview es una zona no confiable: la clave llega como cadena y se valida
///   aquí contra la enumeración cerrada, que es lo que impide que un secreto o
///   una clave inventada lleguen a la base.
/// - [`crate::error::NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn update_setting(
    state: State<'_, AppState>,
    key: String,
    value: SettingValue,
) -> NbResult<Setting> {
    let clave = SettingKey::from_name(&key).ok_or_else(|| {
        // No se registra la clave recibida: podría ser justamente el nombre de
        // un secreto que alguien intentó colar.
        tracing::warn!("se rechazó una preferencia desconocida");
        NbError::Internal
    })?;

    let conn = state.db.connection()?;
    settings::set(&conn, clave, &value)?;

    // Tres preferencias cambian **qué proveedores de consulta existen**: poner
    // un servidor de traducción tiene que ofrecerla sin reiniciar, y quitarlo
    // tiene que retirarla. Sin esto, la preferencia se guardaría y no serviría
    // de nada hasta el siguiente arranque, que es la clase de ajuste que la
    // gente cambia dos veces creyendo que no funciona.
    if matches!(
        clave,
        SettingKey::TranslationHost | SettingKey::TranslationTarget | SettingKey::Language
    ) {
        rehacer_proveedores_de_consulta(&state);
    }

    // Subir o bajar el tope de lo derivado se nota en el momento: un indexador
    // parado por falta de sitio no tiene por qué esperar a su siguiente vuelta
    // para enterarse de que ya lo hay.
    if clave == SettingKey::DerivedBudget {
        state.indexer.revisar_presupuesto();
    }

    // Y el de la inferencia, por lo mismo: un tope de memoria bajado tiene
    // que soltar el modelo ahora, no en el siguiente arranque.
    // Y el proveedor externo: borrar la dirección tiene que dejar de
    // permitir enviar ahora, no en el siguiente arranque.
    if matches!(
        clave,
        SettingKey::AiProvider | SettingKey::AiHost | SettingKey::AiModel
    ) {
        state.ia.configurar_externo(&state.db, &state.net)?;
    }

    // Y el modelo generativo elegido: cambiarlo suelta el que estuviera
    // cargado, que son dos gigabytes, y la siguiente función la atiende el nuevo.
    if clave == SettingKey::AiModelLocal {
        state.ia.aplicar_modelo_generativo(&state.db)?;
    }

    if matches!(clave, SettingKey::AiThreads | SettingKey::AiMemoryLimitMb) {
        let presupuesto = crate::ai::presupuesto::Presupuesto::guardado(&conn);
        state.ia.aplicar_presupuesto(presupuesto);
        // La voz neuronal comparte el tope, así que un tope bajado la suelta a ella
        // también: son 600 MB, y dejarlos fuera del tope sería tener dos topes.
        state.voz_neuronal.aplicar(presupuesto);
    }

    Ok(Setting {
        key: clave.as_str().to_owned(),
        value: settings::get(&conn, clave)?,
        affects_first_paint: clave.affects_first_paint(),
        plane: clave.plane(),
    })
}

/// Vuelve a construir el registro de consulta: diccionarios y proveedores en
/// línea.
///
/// Un fallo aquí no puede tumbar el cambio de preferencia: la preferencia ya
/// está guardada y lo peor que ocurre es que el proveedor nuevo no aparezca
/// hasta reiniciar. Se registra y se sigue.
fn rehacer_proveedores_de_consulta(state: &State<'_, AppState>) {
    let mut nuevo = crate::lookup::instalados(&state.paths.dictionaries());
    crate::registrar_consulta_en_linea(&mut nuevo, &state.db, &state.net);

    match state.lookup.write() {
        Ok(mut registro) => *registro = nuevo,
        Err(_) => tracing::error!("el registro de proveedores de consulta quedó envenenado"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{migrations, Database};

    /// Construye el informe sin pasar por Tauri, para poder probar la lógica
    /// aislada. La prueba del comando completo, con el estado inyectado, llega
    /// en la.
    fn informe(db: &Database) -> NbResult<HealthReport> {
        Ok(HealthReport {
            version: env!("CARGO_PKG_VERSION"),
            schema_version: db.schema_version()?,
        })
    }

    #[test]
    fn el_informe_reporta_la_version_del_manifiesto_y_del_esquema() {
        let db = Database::open_in_memory().expect("base en memoria");
        let report = informe(&db).expect("no falla");

        assert_eq!(report.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(report.schema_version, migrations::target_version());
    }

    /// El contrato con el frontend usa `camelCase`. Cambiarlo rompe la interfaz
    /// sin que ningún compilador lo detecte.
    #[test]
    fn el_informe_se_serializa_en_camel_case() {
        let db = Database::open_in_memory().expect("base en memoria");
        let json = serde_json::to_string(&informe(&db).expect("no falla")).expect("serializable");

        assert!(json.contains("schemaVersion"), "json inesperado: {json}");
        assert!(!json.contains("schema_version"));
    }

    /// La estabilidad de la API no se declara escribiéndola.
    ///
    /// Si `stable` fuera una constante aparte, alguien podría ponerla en cierto
    /// sin subir la mayor, y la aplicación prometería estabilidad sobre una
    /// API que no es estable. Sale del número, y esta prueba es lo que impide
    /// que deje de salir de ahí.
    #[test]
    fn la_api_solo_se_declara_estable_cuando_su_mayor_lo_es() {
        let api = extensions_api();

        assert_eq!(api.stable, api.current >= 1);
        // La anterior es la actual menos una, y nunca mayor: es el compromiso
        // de dos mayores, visto desde el lado de la interfaz.
        assert!(api.oldest <= api.current);
        assert!(api.current - api.oldest <= 1);
    }
}

/// Abre un documento para leerlo y devuelve lo que el formato sabe hacer.
///
/// Es el comando del que depende toda la interfaz del lector: lo que devuelve en
/// `capabilities` es lo que se pinta. La interfaz **no decide** qué controles
/// existen; si lo decidiera, el día que entre un formato nuevo habría que
/// acordarse de ir a la pantalla a enseñárselo.
///
/// El identificador de archivo es lo único que cruza el puente, y viene del
/// catálogo: ninguna ruta llega desde el webview, así que no hay ruta que
/// canonicalizar ni ámbito que comprobar aquí. La ruta la pone el runtime desde
/// su propia base de datos.
///
/// Abrir dos veces el mismo documento devuelve la misma sesión, con la posición
/// donde estaba. Es deliberado: identificadores de sesión obligarían a la
/// interfaz a llevar la cuenta, y el día que se le olvide uno el usuario tendría
/// el mismo libro abierto dos veces.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no hay tal archivo en el catálogo.
/// - [`NbError::FileNotFound`] si lo hay pero no está disponible: volumen
///   desconectado, permisos, soporte dañado.
/// - [`NbError::ComponentUnavailable`] si esta instalación no tiene proveedor
///   para ese formato. No es lo mismo que el documento esté roto.
/// - [`NbError::DocumentCorrupted`] si el documento no se deja abrir.
#[tauri::command(async)]
pub fn open_document(state: State<'_, AppState>, file_id: String) -> NbResult<SessionInfo> {
    let sesion = state.sessions.open(&state.db, &state.readers, &file_id)?;
    let info = sesion.info().clone();

    /*
     * Lo que se abre y todavía no se puede buscar pasa delante de todo, y no
     * espera al presupuesto de disco: la búsqueda cubre siempre lo que se
     * ha leído, que es lo que promete la página. Antes esa promesa
     * estaba escrita y no la cumplía nada: el indexador seguía el orden de
     * importación, y con el presupuesto alcanzado no indexaba ni lo abierto.
     */
    if let Ok(conn) = state.db.connection() {
        if matches!(
            crate::search::status_of(&conn, &info.edition_id),
            Ok(crate::search::SearchStatus::Pending)
        ) {
            state.indexer.enqueue_prioritario(&info.edition_id);
        }
    }

    Ok(info)
}

/// Cierra una sesión de lectura y suelta lo que retenía.
///
/// Cerrar lo que no está abierto no es un error: la interfaz puede llamar a esto
/// al desmontar un componente sin saber si llegó a abrirse, y convertir eso en
/// un fallo solo produciría manejo de errores que no arregla nada.
///
/// # Errors
///
/// [`NbError::Internal`] si el estado compartido quedó envenenado por un pánico.
#[tauri::command(async)]
pub fn close_document(state: State<'_, AppState>, file_id: String) -> NbResult<()> {
    state.sessions.close(&file_id)
}

/// Apunta dónde está leyendo el usuario ahora.
///
/// Devuelve con qué precisión se resolvió, no un acuse: si el locator ya no
/// resuelve exactamente, la interfaz **avisa**. Tragárselo sería colocar al
/// usuario en un sitio parecido sin decírselo, que es lo que ADR-004 impide.
///
/// La escritura la agrupa el registrador de progreso, así que llamar a esto en
/// cada línea que se desplaza no escribe en cada línea. Lo que sí hace falta es
/// llamar a `close_document` al terminar: es lo que vacía lo pendiente.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto, o si el
///   locator no se resuelve por ninguna vía de la cascada.
/// - [`NbError::SchemaMismatch`] si el locator viene de una versión posterior.
#[tauri::command(async)]
pub fn save_progress(
    state: State<'_, AppState>,
    file_id: String,
    locator: Locator,
) -> NbResult<Resolution> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.go_to(&locator)
}

/// Escribe ya la última posición apuntada del documento abierto, sin cerrarlo.
///
/// La llama el lector al dejar de verse con el libro todavía abierto: el
/// antirrebote de [`save_progress`] deja la última posición sin escribir hasta el
/// siguiente movimiento, y quien va a la biblioteca a mitad de capítulo vería allí
/// una barra distinta de la del lector. No mueve nada ni abre sesión.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn flush_progress(state: State<'_, AppState>, file_id: String) -> NbResult<()> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.write_pending()
}

/// Mira si algo cambió en las carpetas referenciadas y lo refleja.
///
/// Lo pide la interfaz al volver a la ventana y al entrar en la biblioteca o en
/// Inicio, que son los momentos en que alguien que acaba de copiar un libro con
/// el explorador de archivos espera verlo. Devuelve enseguida: el repaso corre en
/// su hilo, y lo que encuentre llega por el bus como cualquier otra importación.
/// Si hay una importación en marcha, otro repaso o uno de hace un momento, no hace
/// nada. Ver `repaso.rs`.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn rescan_roots(state: State<'_, AppState>) -> NbResult<()> {
    let repaso = std::sync::Arc::clone(&state.repaso);
    let db = state.db.clone();
    let bus = state.bus.clone();
    let biblioteca = state.paths.library();
    let importacion = std::sync::Arc::clone(&state.import);
    std::thread::spawn(move || {
        if let Err(error) = repaso.si_toca(&db, &bus, &biblioteca, &importacion) {
            tracing::warn!(code = error.code(), "el repaso de las carpetas falló");
        }
    });
    Ok(())
}

/// Pinta una posición del documento abierto.
///
/// Sin `locator`, la posición actual. Con él, cualquier otra: la superficie de
/// lectura pinta la página siguiente antes de que el usuario llegue, y pedirla
/// **no mueve el progreso**. Si pintar moviera, adelantar una página adelantaría
/// la lectura de algo que nadie ha leído.
///
/// Lo que devuelve es o marcado **ya saneado en Rust** o la **ruta** de una
/// imagen que el webview pide por el protocolo de assets. Nunca bytes por el
/// canal JSON: `docs/architecture/data-flows.md`.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - Los del formato al resolver o al pintar.
#[tauri::command(async)]
pub fn render_document(
    state: State<'_, AppState>,
    file_id: String,
    locator: Option<Locator>,
) -> NbResult<Rendered> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    match locator {
        Some(l) => sesion.render_at(&l),
        None => sesion.render(),
    }
}

/// A qué ritmo se lee una edición, sacado de sus sesiones de lectura.
///
/// Nulo si no hay bastante lectura para decirlo: una estimación inventada es peor
/// que ninguna. Lo que falta lo calcula la interfaz con la posición de ahora.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn reading_pace(
    state: State<'_, AppState>,
    edition_id: String,
) -> NbResult<Option<crate::reader::progress::ReadingPace>> {
    let conn = state.db.connection()?;
    crate::reader::progress::ritmo(&conn, &edition_id)
}

/// Los nombres del libro abierto, con dónde aparece cada uno.
///
/// Local y sin IA: los nombres y sus primeros sitios salen del recorrido del texto,
/// y [`entity_places`] le pregunta al índice de búsqueda por los de uno. No se
/// guarda nada; se calcula la primera vez que se pide y vive lo que vive el
/// documento abierto. Ver `entidades.rs`.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::FormatUnsupported`] si no tiene texto.
///
/// Cota: `entidades::MAXIMO_DE_NOMBRES` nombres, y `MAXIMO_DE_SITIOS` por nombre.
#[tauri::command(async)]
pub fn book_entities(
    state: State<'_, AppState>,
    file_id: String,
) -> NbResult<crate::entidades::EntityIndex> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.entities()
}

/// Dónde aparece un nombre del libro abierto.
///
/// Se le pregunta al índice de búsqueda con una consulta de frase acotada a esta
/// edición; si el libro no está indexado todavía, responde el recorrido del texto,
/// y lo dice.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::StorageFailure`] si la base no responde.
///
/// Cota: `entidades::MAXIMO_DE_SITIOS`.
#[tauri::command(async)]
pub fn entity_places(
    state: State<'_, AppState>,
    file_id: String,
    name: String,
) -> NbResult<crate::entidades::EntityPlaces> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.entity_places(&name)
}

/// La narración que trae el propio libro para un capítulo, si la trae.
///
/// Es la superposición de EPUB 3 del capítulo -Media Overlays- con su audio ya
/// extraído a la caché, que la interfaz pide por el protocolo de assets, y un tramo
/// por `par` con su `Locator`. Nula en un capítulo o un libro que no la trae.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::DocumentCorrupted`] si la superposición no se deja leer o su
///   audio no se puede extraer.
#[tauri::command(async)]
pub fn synchronized_audio(
    state: State<'_, AppState>,
    file_id: String,
    href: String,
) -> NbResult<Option<crate::reader::SynchronizedAudio>> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.synchronized_audio(&href)
}

/// Tamaño intrínseco de la página que se pinta en una posición.
///
/// La interfaz lo necesita **antes** de tener la imagen para reservar el hueco:
/// pedirlo después es pedirlo cuando el salto de maquetación ya ha ocurrido.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::FormatUnsupported`] si el formato no se pinta como imagen. Un
///   EPUB no tiene tamaño de página: lo tiene la ventana.
#[tauri::command(async)]
pub fn page_size(
    state: State<'_, AppState>,
    file_id: String,
    locator: Option<Locator>,
) -> NbResult<PageSize> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    let locator = match locator {
        Some(l) => l,
        None => sesion.current()?,
    };
    sesion.page_size(&locator)
}

/// Capa de texto de la posición, para superponerla sobre la imagen.
///
/// Es lo que hace que un PDF no sea una imagen opaca para un lector de pantalla,
/// y lo que permite seleccionar de verdad en lugar de dibujar un rectángulo
/// encima. Una página sin texto se devuelve **declarada como ausente**, que es
/// distinto de una lista vacía: una lista vacía es indistinguible de una página
/// en blanco.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::FormatUnsupported`] si el formato no se pinta como imagen.
#[tauri::command(async)]
pub fn text_layer(
    state: State<'_, AppState>,
    file_id: String,
    locator: Option<Locator>,
) -> NbResult<TextLayer> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    let locator = match locator {
        Some(l) => l,
        None => sesion.current()?,
    };
    sesion.text_layer(&locator)
}

/// Ancla una cita que el usuario acaba de seleccionar.
///
/// **La cita se busca en el documento, no se cree.** La selección la hace el
/// webview, que es zona no confiable, y la cita es lo que sostiene el segundo
/// escalón de la cascada de ADR-004: una cita que no esté en el documento
/// produciría una anotación imposible de reanclar, y el usuario no se enteraría
/// hasta perderla.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si el documento no está abierto o la cita no
///   aparece en él.
/// - [`NbError::PathNotAllowed`] si la cita está vacía.
/// - [`NbError::FormatUnsupported`] si el formato no tiene texto.
#[tauri::command(async)]
pub fn locator_for_selection(
    state: State<'_, AppState>,
    file_id: String,
    quote: String,
) -> NbResult<Locator> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.locator_for_quote(&quote)
}

/// Crea una anotación anclada a un Locator.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si es un resaltado sin cita: sin ella no hay
///   segundo escalón de la cascada de ADR-004, así que se perdería en cuanto el
///   documento cambiase.
/// - [`NbError::FormatUnsupported`] si es un trazo libre sin geometría: una marca
///   invisible que ocupa sitio y no se puede pintar. La base también lo rechaza,
///   pero decirlo aquí da un código que la interfaz sabe traducir en lugar de un
///   fallo de almacenamiento que no explica nada.
/// - [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
#[tauri::command(async)]
pub fn create_annotation(
    state: State<'_, AppState>,
    edition_id: String,
    kind: AnnotationKind,
    body: Option<String>,
    target: Locator,
    payload: Option<String>,
) -> NbResult<Annotation> {
    if kind == AnnotationKind::Drawing && payload.is_none() {
        return Err(NbError::FormatUnsupported);
    }
    let conn = state.db.connection()?;
    annotations::create(
        &conn,
        &NewAnnotation {
            edition_id,
            kind,
            body,
            target,
            payload,
        },
    )
}

/// Edita una anotación: da de baja la anterior y devuelve la nueva.
///
/// Devuelve una anotación con **identificador nuevo**. La interfaz tiene que
/// quedarse con ese, no con el que envió: el anterior sigue existiendo y ya no
/// es el vigente.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no existe o si ya estaba dada de baja.
/// - [`NbError::PathNotAllowed`] si el ancla nueva deja un resaltado sin cita.
#[tauri::command(async)]
pub fn edit_annotation(
    state: State<'_, AppState>,
    id: String,
    body: Option<Option<String>>,
    target: Option<Locator>,
) -> NbResult<Annotation> {
    let mut conn = state.db.connection()?;
    annotations::edit(&mut conn, &id, &Edit { body, target })
}

/// Paquete de diagnóstico, para verlo **antes** de guardarlo.
///
/// Devuelve el paquete estructurado y no el texto: la interfaz lo enseña por
/// partes —versión, componentes, registro— para que se pueda leer, que es lo que
/// convierte "exportar diagnósticos" en algo que alguien puede consentir.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede consultar el esquema.
#[tauri::command(async)]
pub fn diagnostics_preview(
    state: State<'_, AppState>,
) -> NbResult<crate::diagnostics::DiagnosticsPackage> {
    // La medida de disco no puede impedir un diagnóstico: si la base no
    // responde, lo demás del paquete sigue sirviendo para saber por qué.
    let almacenamiento = state
        .db
        .connection()
        .ok()
        .and_then(|conn| crate::diagnostics::medir_almacenamiento(&conn, &state.paths));

    crate::diagnostics::collect(
        &state.paths.logs,
        state.db.schema_version()?,
        state.monitor.snapshot(),
        almacenamiento,
    )
}

/// Qué puede hacer la voz del runtime en esta máquina.
///
/// Existe porque el webview de Linux **no tiene voz**: WebKitGTK declara la
/// característica activada y `window.speechSynthesis` llega indefinido de todos
/// modos. La interfaz pregunta primero al webview y, si no la tiene, aquí.
///
/// # Errors
///
/// Ninguno. Devuelve `NbResult` por uniformidad del puente.
#[tauri::command(async)]
pub fn voice_status(_state: State<'_, AppState>) -> NbResult<crate::voice::VoiceStatus> {
    Ok(crate::voice::status())
}

/// Manda decir un tramo. **Vuelve enseguida**; el fin llega por el bus.
///
/// Hablar tarda lo que dura el párrafo: esperar aquí dejaría la llamada abierta
/// minutos y ocuparía un hilo del runtime asíncrono.
///
/// # Errors
///
/// Ninguno: no poder hablar no es un error de la operación, se avisa por el bus.
#[tauri::command(async)]
pub fn voice_speak(
    state: State<'_, AppState>,
    block: u32,
    text: String,
    rate: f32,
    voice: Option<String>,
) -> NbResult<()> {
    state.voice.speak(block, text, rate, voice);
    Ok(())
}

/// Sintetiza un tramo con el servicio remoto y devuelve la ruta del audio.
///
/// # Por qué devuelve una ruta y no el audio
///
/// Porque son cientos de kilobytes por párrafo. Cruzarlos por el puente
/// obligaría a codificarlos en base64 y a moverlos por el canal de mensajes, que
/// es el mismo por el que va todo lo demás; el webview ya sabe leer de la caché
/// por el protocolo de assets, que es para lo que está.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay servicio configurado.
/// - Los de la puerta de red, incluido [`NbError::ConsentRequired`] cuando el
///   anfitrión no está autorizado **para este propósito**: autorizar un catálogo
///   no autoriza a mandarle el libro entero.
/// - [`NbError::StorageFailure`] si no se puede escribir el audio.
#[tauri::command(async)]
pub fn synthesize_speech(state: State<'_, AppState>, text: String, rate: f32) -> NbResult<String> {
    let conn = state.db.connection()?;
    let destino = destino_de_voz(&conn)?;
    drop(conn);

    let carpeta = state.paths.cache.join("voz");
    let archivo =
        crate::speech::sintetizar(&state.db, &state.net, &carpeta, &destino, &text, rate)?;
    Ok(archivo.to_string_lossy().into_owned())
}

/// La voz neuronal local: qué es, si está instalada y si esta máquina la mueve.
///
/// Cota: una, la del catálogo compilado en el binario.
///
/// # Medir o no medir
///
/// Con `measure`, si está instalada y cabe, **se carga y se sintetiza una frase
/// corta** para saber su factor de tiempo real: tarda unos segundos y ocupa 600 MB
/// hasta que se suelta. Es lo que pide Ajustes, donde se ve el factor.
///
/// Sin `measure` **no carga nada**: dice lo que se sabe -instalada, si cabe, y el
/// factor si ya se midió-. Es lo que pide el narrador al arrancar. Hasta el
/// 2026-09-22 siempre medía, así que con la voz neuronal elegida **cada arranque
/// cargaba el modelo y lo calibraba** aunque nadie fuera a escuchar nada. Desde ese
/// día lento no quita la voz -recomendar no es obligar-, así que no hace falta
/// medir para ofrecerla: la medida solo decide el aviso, y llega con la primera
/// síntesis.
///
/// # Errors
///
/// No falla: lo que no se puede hacer viaja como motivo dentro de la vista.
#[tauri::command(async)]
pub fn neural_voice(
    state: State<'_, AppState>,
    measure: bool,
) -> NbResult<crate::voice::neuronal::VistaDeLaVoz> {
    Ok(if measure {
        state.voz_neuronal.vista()
    } else {
        state.voz_neuronal.vista_sin_cargar()
    })
}

/// Sintetiza un tramo con la voz neuronal local y devuelve la ruta del audio.
///
/// Devuelve una ruta y no el audio por lo mismo que `synthesize_speech`: son
/// cientos de kilobytes por párrafo y el webview ya sabe leer de la caché por el
/// protocolo de assets. Con un archivo detrás, además, **pausar es pausar**.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si el modelo no está instalado o la voz
///   pedida no existe.
/// - [`NbError::StorageFailure`] si no se puede escribir el audio.
/// - [`NbError::Internal`] si la síntesis falla.
#[tauri::command(async)]
pub fn synthesize_neural_speech(
    state: State<'_, AppState>,
    text: String,
    voice: Option<String>,
    rate: f32,
) -> NbResult<String> {
    let carpeta = state.paths.cache.join("voz-neuronal");
    let archivo =
        state
            .voz_neuronal
            .sintetizar_a_archivo(&carpeta, &text, voice.as_deref(), rate)?;
    Ok(archivo.to_string_lossy().into_owned())
}

/// Lo que costaría preparar el audio de un capítulo o de un libro.
///
/// Se pide **antes** de ofrecer empezar, como el coste de vectorizar: quien va a
/// esperar cuarenta minutos tiene que saberlo antes de empezar a esperar.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la edición no existe o no tiene texto.
/// - [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn estimate_audio(
    state: State<'_, AppState>,
    scope: crate::voice::preparacion::Alcance,
    voice: Option<String>,
) -> NbResult<crate::voice::preparacion::Estimacion> {
    state.audio.estimar(&scope, voice.as_deref())
}

/// Sintetiza una muestra corta **del propio libro**, para decidir antes de esperar.
///
/// Devuelve la ruta del audio. Se pidió el 2026-09-18: quien va a esperar
/// una hora merece oír antes cómo suena su libro, no una frase de ejemplo.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la voz no está instalada.
/// - [`NbError::DocumentNotFound`] si el alcance no tiene texto.
#[tauri::command(async)]
pub fn sample_audio(
    state: State<'_, AppState>,
    scope: crate::voice::preparacion::Alcance,
    voice: Option<String>,
) -> NbResult<String> {
    state.audio.muestra(&scope, voice.as_deref())
}

/// Empieza a preparar el audio, en segundo plano. Encola si ya hay algo en marcha.
///
/// El avance llega por `AudioJobProgress` y el final por `AudioJobFinished`, por lo
/// mismo que la importación: cuarenta minutos no caben en una llamada abierta.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la voz no está instalada.
/// - [`NbError::DocumentNotFound`] si el alcance no tiene texto.
#[tauri::command(async)]
pub fn start_audio(
    state: State<'_, AppState>,
    scope: crate::voice::preparacion::Alcance,
    voice: Option<String>,
) -> NbResult<()> {
    state.audio.empezar(scope, voice)
}

/// Encola el libro entero de cada obra de una colección.
///
/// Devuelve cuántas entraron, que no tiene por qué ser cuántas hay: lo que no
/// cabe en el presupuesto de lo derivado no se encola, y la interfaz enseña la
/// diferencia en lugar de prometer treinta y preparar doce.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la voz no está instalada.
/// - [`NbError::StorageFailure`] si la base no responde.
/// - [`NbError::StorageBudgetReached`] si no cupo ninguna.
#[tauri::command(async)]
pub fn start_audio_collection(
    state: State<'_, AppState>,
    collection_id: String,
    voice: Option<String>,
) -> NbResult<u32> {
    state.audio.encolar_coleccion(&collection_id, voice)
}

/// Los que esperan turno detrás del que se está preparando.
///
/// Cota: quinientos, que es lo que la preparación admite tener encolado.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn audio_queue(state: State<'_, AppState>) -> NbResult<Vec<crate::voice::preparacion::EnCola>> {
    Ok(state.audio.cola())
}

/// Corta la preparación en curso y vacía la cola. **Lo hecho se queda.**
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn cancel_audio(state: State<'_, AppState>) -> NbResult<()> {
    state.audio.cancelar();
    Ok(())
}

/// Lo que se está preparando ahora, si hay algo.
///
/// Cota: uno. Se prepara de uno en uno.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn audio_job(
    state: State<'_, AppState>,
) -> NbResult<Option<crate::voice::preparacion::EnCurso>> {
    Ok(state.audio.en_curso())
}

/// El audio preparado de un alcance, con sus marcas para seguir el texto.
///
/// Cota: las marcas son una por fragmento del alcance, y prepararlo las acota en
/// veinte mil, que es más de lo que tiene un libro largo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn prepared_audio(
    state: State<'_, AppState>,
    scope: crate::voice::preparacion::Alcance,
    voice: Option<String>,
) -> NbResult<Option<crate::voice::preparacion::Preparado>> {
    state.audio.preparado(&scope, voice.as_deref())
}

/// Quita el audio preparado de un alcance -un capítulo o el libro entero-: sus
/// filas, sus marcas y sus archivos. Lo demás de ese libro se queda.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn forget_audio(
    state: State<'_, AppState>,
    scope: crate::voice::preparacion::Alcance,
) -> NbResult<()> {
    state.audio.olvidar(&scope)
}

/// La dirección exacta a la que se mandaría el texto, tal y como la entiende el
/// runtime, o nada si no hay servicio escrito.
///
/// Existe para que Ajustes **enseñe** lo que se va a usar en lugar de dejar que
/// se descubra al fallar: el servidor se puede escribir de muchas formas, y la
/// única que importa es la que resulta. La interpreta el runtime y no la interfaz
/// para que no haya dos formas de leer lo mismo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se pueden leer las preferencias.
#[tauri::command(async)]
pub fn speech_endpoint(state: State<'_, AppState>) -> NbResult<Option<String>> {
    let conn = state.db.connection()?;
    let destino = destino_de_voz(&conn)?;
    Ok(destino.configurado().then(|| destino.punto_de_sintesis()))
}

/// Autoriza al servicio de síntesis configurado a recibir el texto del libro.
///
/// **No acepta un anfitrión cualquiera**, y esa es toda la diferencia: se
/// autoriza el que está en las preferencias, que es el que el usuario escribió.
/// Un comando que aceptara cualquiera convertiría al webview en el sitio donde
/// se decide a dónde sale el texto de los libros, y el webview es zona no
/// confiable.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay servicio configurado.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn allow_speech_host(state: State<'_, AppState>) -> NbResult<()> {
    let conn = state.db.connection()?;
    let destino = destino_de_voz(&conn)?;
    if !destino.configurado() {
        tracing::info!("se pidió autorizar una voz remota sin servicio configurado");
        return Err(NbError::PathNotAllowed);
    }
    let anfitrion = crate::net::host_de(&destino.direccion())?;
    crate::net::grant(&conn, &anfitrion, crate::net::Purpose::Speech)?;
    tracing::info!("el usuario autorizó el servicio de síntesis");
    Ok(())
}

/// Lo que la interfaz puede saber de la credencial: si cabe guardarla y si la
/// hay. **Nunca el secreto.**
///
/// Que este tipo no tenga un campo con la clave no es un olvido: el webview es
/// zona no confiable, y una credencial que llega hasta él está a una extensión
/// de distancia de salir. La interfaz necesita pintar dos estados —"aquí no se
/// puede guardar" y "ya hay una"— y para eso bastan dos booleanos.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct CredentialStatus {
    /// Si el sistema tiene un almacén de credenciales utilizable.
    ///
    /// En falso, la interfaz dice por qué y no ofrece guardar nada: no hay
    /// repliegue a texto plano, que es lo que prohíbe `authentication.md`.
    pub keyring_available: bool,
    /// Si el servicio configurado ya tiene credencial guardada.
    pub stored: bool,
}

/// Si se puede guardar una credencial aquí, y si el servicio ya tiene una.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si el llavero falla al consultarse.
#[tauri::command(async)]
pub fn speech_credential_status(state: State<'_, AppState>) -> NbResult<CredentialStatus> {
    let disponible = crate::secrets::disponible();
    let conn = state.db.connection()?;
    let destino = destino_de_voz(&conn)?;
    drop(conn);

    if !disponible || !destino.configurado() {
        return Ok(CredentialStatus {
            keyring_available: disponible,
            stored: false,
        });
    }

    let host = crate::net::host_de(&destino.direccion())?;
    let stored = crate::secrets::leer(&crate::secrets::cuenta_de_voz(&host))?.is_some();
    Ok(CredentialStatus {
        keyring_available: disponible,
        stored,
    })
}

/// Guarda en el llavero del sistema la credencial del servicio configurado.
///
/// **No acepta a qué servicio pertenece**, igual que `allow_speech_host`: la
/// clave se guarda para el anfitrión que está en las preferencias. Si el webview
/// pudiera decir de quién es la credencial, podría guardar la de un servicio
/// bajo el nombre de otro y conseguir que se la mandaran a él.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay servicio configurado o el secreto
///   está vacío o pasa del límite.
/// - [`NbError::ComponentUnavailable`] si el sistema no tiene llavero.
/// - [`NbError::StorageFailure`] si el llavero rechaza la escritura.
#[tauri::command(async)]
pub fn set_speech_credential(state: State<'_, AppState>, secret: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    let destino = destino_de_voz(&conn)?;
    drop(conn);
    if !destino.configurado() {
        tracing::info!("se pidió guardar una credencial sin servicio configurado");
        return Err(NbError::PathNotAllowed);
    }

    let host = crate::net::host_de(&destino.direccion())?;
    crate::secrets::guardar(&crate::secrets::cuenta_de_voz(&host), secret.trim())?;
    // El anfitrión sí; el secreto no, ni su largo, ni truncado.
    tracing::info!("se guardó una credencial de síntesis");
    Ok(())
}

/// Borra del llavero la credencial del servicio configurado.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay servicio configurado.
/// - [`NbError::ComponentUnavailable`] si el sistema no tiene llavero.
/// - [`NbError::StorageFailure`] si el llavero falla al borrar.
#[tauri::command(async)]
pub fn clear_speech_credential(state: State<'_, AppState>) -> NbResult<()> {
    let conn = state.db.connection()?;
    let destino = destino_de_voz(&conn)?;
    drop(conn);
    if !destino.configurado() {
        return Err(NbError::PathNotAllowed);
    }

    let host = crate::net::host_de(&destino.direccion())?;
    crate::secrets::borrar(&crate::secrets::cuenta_de_voz(&host))?;
    tracing::info!("se borró una credencial de síntesis");
    Ok(())
}

/// Lee de las preferencias a dónde se manda a sintetizar.
fn destino_de_voz(conn: &rusqlite::Connection) -> NbResult<crate::speech::Destino> {
    let texto = |clave| -> NbResult<String> {
        Ok(match crate::settings::get(conn, clave)? {
            crate::settings::SettingValue::Text(v) => v,
            _ => String::new(),
        })
    };
    Ok(crate::speech::Destino {
        host: texto(crate::settings::SettingKey::SpeechHost)?,
        model: texto(crate::settings::SettingKey::SpeechModel)?,
        voice: texto(crate::settings::SettingKey::SpeechVoice)?,
    })
}

/// Corta lo que la voz esté diciendo.
///
/// # Errors
///
/// Ninguno.
#[tauri::command(async)]
pub fn voice_stop(state: State<'_, AppState>) -> NbResult<()> {
    state.voice.stop();
    Ok(())
}

/// Qué pasó al abrir la biblioteca, cuando hubo que recuperarla.
///
/// Devuelve `null` en un arranque normal. Cuando devuelve algo, la interfaz lo
/// enseña: la biblioteca está vacía y el usuario no lo pidió, y una biblioteca
/// vacía sin explicación es exactamente la pérdida silenciosa que el producto existe
/// para impedir.
///
/// # Errors
///
/// Ninguno. Devuelve `NbResult` por uniformidad del puente.
#[tauri::command(async)]
pub fn recovery_report(
    state: State<'_, AppState>,
) -> NbResult<Option<crate::recovery::RecoveryReport>> {
    Ok(state.recovery.clone())
}

/// Guarda el paquete de diagnóstico donde el usuario haya elegido.
///
/// # Por qué escribe el runtime y no el webview
///
/// Porque la alternativa era darle al webview la capacidad de escribir cualquier
/// archivo. Un comando que solo sabe escribir el diagnóstico es una superficie
/// mucho más pequeña por exactamente el mismo resultado.
///
/// La ruta viene del **diálogo del sistema**, que es una elección explícita del
/// usuario, igual que al importar. Aun así se comprueba lo que se puede
/// comprobar: que el directorio destino existe, y que no se está escribiendo
/// encima de la base de datos ni de nada del directorio de datos, que sería una
/// forma muy tonta de perder una biblioteca.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si el destino no vale, [`NbError::StorageFailure`]
/// si la escritura falla, y los de montar el paquete.
#[tauri::command(async)]
pub fn save_diagnostics(state: State<'_, AppState>, path: String) -> NbResult<()> {
    let destino = destino_admisible(&state, &path)?;

    let almacenamiento = state
        .db
        .connection()
        .ok()
        .and_then(|conn| crate::diagnostics::medir_almacenamiento(&conn, &state.paths));

    let paquete = crate::diagnostics::collect(
        &state.paths.logs,
        state.db.schema_version()?,
        state.monitor.snapshot(),
        almacenamiento,
    )?;
    let texto = crate::diagnostics::render(&paquete)?;

    std::fs::write(&destino, texto).map_err(|error| {
        tracing::error!(error = %error, "no se pudo guardar el diagnóstico");
        NbError::StorageFailure
    })
}

/// Comprueba que una ruta de destino elegida por el usuario se pueda escribir.
///
/// La ruta llega del webview aunque la haya producido el diálogo del sistema, y
/// el webview es zona no confiable: lo que se comprueba aquí es que su carpeta
/// existe y que **no está dentro del directorio de datos**, donde vive la
/// biblioteca. Guardar ahí no es un error del usuario que se pueda deshacer.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si la carpeta no existe o es la de datos.
fn destino_admisible(state: &State<'_, AppState>, path: &str) -> NbResult<std::path::PathBuf> {
    let destino = std::path::Path::new(path);

    let Some(carpeta) = destino.parent() else {
        return Err(NbError::PathNotAllowed);
    };
    let carpeta = carpeta
        .canonicalize()
        .map_err(|_| NbError::PathNotAllowed)?;

    if let Ok(datos) = state.paths.data.canonicalize() {
        if carpeta.starts_with(&datos) {
            tracing::warn!("se intentó guardar dentro del directorio de datos");
            return Err(NbError::PathNotAllowed);
        }
    }

    Ok(destino.to_path_buf())
}

/// Guarda el vocabulario consultado donde el usuario haya dicho, en CSV.
///
/// CSV y no otra cosa porque lo abre cualquier hoja de cálculo y Anki lo importa
/// sin configurar nada. Devuelve cuántas palabras salieron.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el destino no vale.
/// - [`NbError::StorageFailure`] si la consulta o la escritura fallan.
#[tauri::command(async)]
pub fn save_vocabulary(state: State<'_, AppState>, path: String) -> NbResult<u32> {
    let destino = destino_admisible(&state, &path)?;
    let conn = state.db.connection()?;
    let (texto, cuantas) = crate::vocabulary::exportar(&conn)?;
    drop(conn);
    std::fs::write(&destino, texto).map_err(|error| {
        tracing::error!(error = %error, "no se pudo guardar el vocabulario");
        NbError::StorageFailure
    })?;
    Ok(u32::try_from(cuantas).unwrap_or(u32::MAX))
}

/// Hace tarjetas de estudio con el vocabulario consultado, con la definición del
/// diccionario de aquí dentro en el reverso y sin salir a la red.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn vocabulary_to_cards(
    state: State<'_, AppState>,
) -> NbResult<crate::vocabulary::VocabularyCards> {
    let mut conn = state.db.connection()?;
    let registro = state.lookup.read().map_err(|_| {
        tracing::error!("el registro de proveedores de consulta quedó envenenado");
        NbError::Internal
    })?;
    crate::vocabulary::a_tarjetas(&mut conn, &registro)
}

/// Guarda las anotaciones de una edición donde el usuario haya dicho: en el JSON de
/// W3C, o en Markdown o texto plano con su cita y su localizador.
///
/// El comando de exportar devuelve el JSON y no escribe: así se decidió, y
/// elegir destino es del diálogo nativo. Esto es la otra mitad, la que faltaba:
/// antes `export_annotations` existía y **no lo llamaba nadie**,
/// así que el criterio "el archivo exportado es legible sin NB Books" se cumplía
/// en una prueba y no en el producto, porque el archivo no se podía producir.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el destino no vale.
/// - [`NbError::StorageFailure`] si la consulta o la escritura fallan.
#[tauri::command(async)]
pub fn save_annotations(
    state: State<'_, AppState>,
    edition_id: String,
    path: String,
    format: Option<String>,
    language: Option<String>,
) -> NbResult<()> {
    let destino = destino_admisible(&state, &path)?;

    let conn = state.db.connection()?;
    // En Markdown o en texto plano se lee sin NB Books; sin formato, el JSON
    // de W3C que entienden otras herramientas, que es lo que había.
    let texto = match format.as_deref().and_then(annotations::texto::Formato::de) {
        Some(formato) => annotations::texto::exportar(
            &conn,
            &edition_id,
            formato,
            language.as_deref().unwrap_or("es"),
        )?,
        None => serde_json::to_string_pretty(&annotations::web::export(&conn, &edition_id)?)
            .map_err(|_| NbError::Internal)?,
    };
    drop(conn);

    std::fs::write(&destino, texto).map_err(|error| {
        tracing::error!(error = %error, "no se pudieron guardar las anotaciones");
        NbError::StorageFailure
    })
}

/// Exporta la biblioteca entera donde el usuario haya dicho.
///
/// Es la garantía de posesión puesta detrás de un botón. `portability.rs` existe
/// y se probó con ciento cincuenta mil documentos, y **no
/// tenía comando**: "el usuario es dueño de sus datos" solo se cumplía dentro de
/// las pruebas. Es lo que se diseñó.
///
/// Lee dentro de [`crate::db::read_snapshot`], porque la indexación puede estar
/// escribiendo a la vez y una exportación que no casa consigo misma no se puede
/// restaurar. Devuelve cuántos documentos salieron, para que la interfaz lo diga.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el destino no vale.
/// - [`NbError::StorageFailure`] si la lectura o la escritura fallan.
#[tauri::command(async)]
pub fn save_library_export(state: State<'_, AppState>, path: String) -> NbResult<u32> {
    let destino = destino_admisible(&state, &path)?;

    let mut conn = state.db.connection()?;
    let lectura = crate::db::read_snapshot(&mut conn)?;
    let exportacion = crate::portability::export(&lectura)?;
    drop(lectura);
    drop(conn);

    crate::portability::write_json(&exportacion, &destino)?;
    tracing::info!(documentos = exportacion.files.len(), "biblioteca exportada");
    Ok(u32::try_from(exportacion.files.len()).unwrap_or(u32::MAX))
}

/// Borra el historial de lectura y por dónde iba cada documento.
///
/// Descarta antes lo que `ProgressTracker` tuviera pendiente en memoria para que
/// un vaciado posterior no vuelva a escribir el progreso recién borrado, y avisa
/// por el bus de eventos para que Inicio y Biblioteca se refresquen al instante.
/// Nunca toca los archivos del usuario en el disco.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si la transacción falla.
#[tauri::command(async)]
pub fn clear_reading_history(state: State<'_, AppState>) -> NbResult<()> {
    state.sessions.tracker().clear()?;
    let mut conn = state.db.connection()?;
    crate::portability::clear_reading_history(&mut conn)?;
    drop(conn);
    state
        .bus
        .emit(crate::events::DomainEvent::CoversRecovered { count: 0 });
    Ok(())
}

/// Borra todas las anotaciones, las notas, sus enlaces y los tableros.
///
/// Conserva el catálogo, el historial de lectura, el vocabulario, las preferencias
/// y los archivos del usuario en el disco.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si la transacción falla.
#[tauri::command(async)]
pub fn clear_annotations_and_notes(state: State<'_, AppState>) -> NbResult<()> {
    let mut conn = state.db.connection()?;
    crate::portability::clear_annotations_and_notes(&mut conn)?;
    drop(conn);
    state
        .bus
        .emit(crate::events::DomainEvent::CoversRecovered { count: 0 });
    Ok(())
}

/// Vacía la biblioteca entera en la base de datos.
///
/// Cierra antes las sesiones en memoria sin volcar su progreso, ejecuta el borrado
/// en una sola transacción inmediata y avisa por el bus para que las vistas de
/// obras y colecciones se vacíen. Nunca borra los archivos del usuario en el disco.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si la transacción falla.
#[tauri::command(async)]
pub fn clear_library(state: State<'_, AppState>) -> NbResult<()> {
    state.sessions.clear()?;
    let mut conn = state.db.connection()?;
    crate::portability::clear_library(&mut conn)?;
    drop(conn);
    state
        .bus
        .emit(crate::events::DomainEvent::CoversRecovered { count: 0 });
    state
        .bus
        .emit(crate::events::DomainEvent::CollectionsChanged {
            collection_id: None,
        });
    Ok(())
}

/// Anotaciones de **toda** la biblioteca, de la más reciente a la más antigua.
///
/// Es el otro alcance del mismo dato: `annotation_outline` da las del documento
/// abierto en orden de lectura, y esto da las de todos los documentos por fecha.
/// La frontera es la misma que separa la búsqueda de catálogo de la de corpus.
///
/// **No dice dónde ha quedado cada una.** Saberlo exigiría abrir los sesenta
/// documentos, y desde Estudio no se está leyendo ninguno: el reanclaje se
/// resuelve al abrir, que es cuando significa algo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla o si alguna anotación tiene
/// el ancla ilegible.
#[tauri::command(async)]
pub fn library_annotations(
    state: State<'_, AppState>,
    kind: Option<AnnotationKind>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<annotations::LibraryAnnotation>> {
    let conn = state.db.connection()?;
    annotations::library(&conn, kind, acotar(limit), offset)
}

/// Cuántas anotaciones hay de cada clase, para no ofrecer un filtro vacío.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn annotation_counts(state: State<'_, AppState>) -> NbResult<annotations::AnnotationCounts> {
    let conn = state.db.connection()?;
    annotations::counts(&conn)
}

/// Cuántas tarjetas y conceptos hay guardados.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn study_counts(state: State<'_, AppState>) -> NbResult<crate::ai::cuaderno::StudyCounts> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::cuentas(&conn)
}

/// Todas las versiones de una anotación, de la más antigua a la más reciente.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si el identificador no existe.
///
/// Cota: las versiones de una anotación, que las escribe quien la edita a mano.
#[tauri::command(async)]
pub fn annotation_history(state: State<'_, AppState>, id: String) -> NbResult<Vec<Annotation>> {
    let conn = state.db.connection()?;
    annotations::history(&conn, &id)
}

/// Da de baja una anotación. La fila se conserva.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si no existe. Darla de baja dos veces no lo es:
/// la interfaz puede pedirlo sin saber si ya ocurrió.
#[tauri::command(async)]
pub fn delete_annotation(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    annotations::delete(&conn, &id)
}

/// Panel de anotaciones del documento abierto, en orden de lectura.
///
/// En orden de lectura y no por fecha: el panel acompaña a quien está leyendo, y
/// una lista por fecha obliga a buscar en ella al pasar de capítulo. Las que no
/// se pudieron colocar van al final, marcadas, en la **misma** lista: esconderlas
/// en otra pestaña las convertiría en algo que se descubre por casualidad.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si ese documento no está abierto.
/// - [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: las anotaciones de un documento, no las de la biblioteca.
#[tauri::command(async)]
pub fn annotation_outline(
    state: State<'_, AppState>,
    file_id: String,
) -> NbResult<Vec<OutlineEntry>> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    sesion.outline()
}

/// Pone un marcador en la posición actual del documento abierto.
///
/// La posición la pone el runtime, no el webview: el marcador tiene que caer
/// donde está la lectura, y dejar que el webview mande un locator abriría la
/// puerta a marcar un sitio que no es donde el usuario está.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si ese documento no está abierto.
#[tauri::command(async)]
pub fn create_bookmark(state: State<'_, AppState>, file_id: String) -> NbResult<Annotation> {
    let sesion = state
        .sessions
        .get(&file_id)?
        .ok_or(NbError::DocumentNotFound)?;
    let conn = state.db.connection()?;
    annotations::bookmark(&conn, &sesion.info().edition_id, &sesion.current()?)
}

/// Informe de una importación.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ImportReport {
    pub processed: u32,
    pub imported: u32,
    /// Ya estaban. En una reanudación son la mayoría, y no son un problema: son
    /// la prueba de que reanudar no duplica.
    pub already_in_library: u32,
    /// Copias exactas, en otra ruta, de algo que ya estaba. No entran, y
    /// se dicen aparte de lo que ya estaba.
    pub duplicates: u32,
    /// Cuántos se rechazaron, y por qué. La ruta que viaja es **relativa**.
    pub rejected: Vec<library::Rejected>,
    pub cancelled: bool,
    /// La importación sigue en marcha y el avance llega por eventos.
    ///
    /// Cierto para una carpeta, falso para un archivo suelto. Sin esta marca, la
    /// interfaz no sabría si el informe que tiene delante es el final o el
    /// principio.
    pub running: bool,
}
/// Libera la reserva de importación al salir del ámbito, salga como salga.
///
/// Un `Drop` y no una llamada al final: la función tiene varios caminos de
/// salida, y olvidarse en uno solo dejaría la aplicación sin poder volver a
/// importar hasta reiniciarla.
struct SueltaAlSalir<'a>(&'a library::CancelToken);

impl Drop for SueltaAlSalir<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

/// Reanuda la indexación al terminar la importación, salga como salga.
struct ReanudaAlSalir<'a>(&'a crate::search::Indexer);

impl Drop for ReanudaAlSalir<'_> {
    fn drop(&mut self) {
        self.0.resume_after_import();
    }
}

/// Importa un archivo o una carpeta entera. **No espera a terminar.**
///
/// # Por qué no espera
///
/// La biblioteca de referencia tiene 152.080 obras y recorrerla son horas. Un
/// comando que espere deja la interfaz sin responder todo ese tiempo, y el
/// usuario sin forma de cancelar salvo cerrando la aplicación, que es justo lo
/// que el runtime ya evita: cancelable entre archivo y archivo. Lo que
/// devuelve es cuántos archivos hay por delante; el avance llega por
/// `BulkImportProgress` y el final, por `BulkImportFinished`.
///
/// Un **archivo suelto** sí se importa en el acto: uno solo no tarda, y esperar
/// un evento para saber que entró sería ceremonia.
///
/// # De dónde puede venir esta ruta
///
/// **Solo de un gesto del usuario**: soltar sobre la ventana, o escribirla él.
/// El webview es zona no confiable y esta es la única frontera del puente que
/// recibe una ruta, así que se canonicaliza antes de tocar nada.
///
/// Mientras el diálogo nativo no exista, la elección sigue siendo del usuario
/// porque la ruta la escribe o la suelta él; cuando exista, se sustituye la vía
/// y esta comprobación se queda igual.
///
/// # Errors
///
/// - [`NbError::FileNotFound`] si la ruta no existe o no se deja canonicalizar.
/// - [`NbError::PathNotAllowed`] si un archivo suelto no tiene carpeta padre.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn import_path(state: State<'_, AppState>, path: String, copy: bool) -> NbResult<ImportReport> {
    let canonica = std::path::PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| {
            // No se registra la ruta: es dato del usuario.
            tracing::info!("se intentó importar una ruta que no se deja canonicalizar");
            NbError::FileNotFound
        })?;

    let storage_mode = if copy {
        crate::db::catalog::StorageMode::Managed
    } else {
        crate::db::catalog::StorageMode::Referenced
    };
    let library = state.paths.library();

    if canonica.is_dir() {
        let es_calibre = crate::calibre::is_calibre_library(&canonica);
        if es_calibre {
            // Validación previa: si metadata.db no se puede leer, falla de inmediato
            // para que la interfaz lo diga y no importe nada a medias.
            crate::calibre::validar_biblioteca(&canonica)?;
        }

        /*
         * Una sola importación a la vez. Sin esto, pulsar el botón dos veces
         * arrancaba dos hilos recorriendo la misma carpeta contra la misma base:
         * el doble de contención, y un avance que salta hacia atrás porque hay
         * dos contadores escribiendo en el mismo sitio.
         *
         * Se responde con `running: true` en lugar de con un error: desde fuera,
         * pedir importar cuando ya se está importando **no ha fallado**, y lo que
         * la interfaz tiene que hacer es lo mismo en los dos casos: enseñar el
         * avance de la que hay.
         */
        if !state.import.try_begin() {
            tracing::info!("se pidió importar con una importación ya en marcha");
            return Ok(ImportReport {
                processed: 0,
                imported: 0,
                already_in_library: 0,
                duplicates: 0,
                rejected: Vec::new(),
                cancelled: false,
                running: true,
            });
        }

        // Una importación nueva empieza con el testigo limpio: si no, una
        // cancelación anterior pararía esta antes de mirar el primer archivo.
        state.import.reset();

        let db = state.db.clone();
        let bus = state.bus.clone();
        let cancel = std::sync::Arc::clone(&state.import);
        let carpeta = canonica.clone();
        let covers = state.paths.cache.join("portadas");

        let indexador = std::sync::Arc::clone(&state.indexer);

        std::thread::spawn(move || {
            // La reserva se libera pase lo que pase: si se escapara por un
            // camino de error, no se podría volver a importar hasta reiniciar.
            let _reserva = SueltaAlSalir(&cancel);

            /*
             * La indexación cede el paso. Indexar un documento escribe cerca de
             * novecientas filas en el índice de texto completo; importar escribe
             * unas pocas. Con las dos a la vez, lo que el usuario está esperando
             * pasa el rato haciendo cola detrás de lo que puede esperar.
             *
             * Al terminar se reanuda y se encola lo que falte, así que no se
             * pierde nada: solo se hace después.
             */
            indexador.pause_for_import();
            let _indexacion = ReanudaAlSalir(&indexador);

            let Ok(mut conn) = db.connection() else {
                tracing::error!("la importación no pudo abrir una conexión");
                return;
            };

            if es_calibre {
                match crate::calibre::import_calibre_library(
                    &mut conn, &carpeta, &covers, &bus, &cancel,
                ) {
                    Ok(informe) => tracing::info!(
                        procesados = informe.processed,
                        importados = informe.imported,
                        rechazados = informe.rejected.len(),
                        cancelada = informe.cancelled,
                        "importación de Calibre terminada"
                    ),
                    Err(error) => {
                        tracing::error!(code = error.code(), "la importación de Calibre falló")
                    }
                }
            } else {
                let ctx = library::ImportContext {
                    scope: &carpeta,
                    library: &library,
                    bus: &bus,
                };
                match library::import_directory(&mut conn, &ctx, &carpeta, storage_mode, &cancel) {
                    Ok(informe) => tracing::info!(
                        procesados = informe.processed,
                        importados = informe.imported,
                        rechazados = informe.rejected.len(),
                        cancelada = informe.cancelled,
                        "importación terminada"
                    ),
                    Err(error) => tracing::error!(code = error.code(), "la importación falló"),
                }
            }
        });

        return Ok(ImportReport {
            processed: 0,
            imported: 0,
            already_in_library: 0,
            duplicates: 0,
            rejected: Vec::new(),
            cancelled: false,
            running: true,
        });
    }

    // Un archivo suelto: su ámbito es la carpeta que lo contiene, no el disco.
    let carpeta = canonica
        .parent()
        .ok_or(NbError::PathNotAllowed)?
        .to_path_buf();
    let mut conn = state.db.connection()?;
    let ctx = library::ImportContext {
        scope: &carpeta,
        library: &library,
        bus: &state.bus,
    };
    let resultado = library::import_document(
        &mut conn,
        &ctx,
        &library::ImportRequest {
            source: canonica,
            storage_mode,
            // Un archivo suelto no viene de una carpeta elegida.
            root: None,
        },
    );

    let (imported, already, duplicates) = match resultado {
        Ok(library::ImportOutcome::Imported(_)) => (1, 0, 0),
        Ok(library::ImportOutcome::AlreadyInLibrary(_)) => (0, 1, 0),
        Ok(library::ImportOutcome::Duplicate(_)) => (0, 0, 1),
        // Un archivo que falla se informa como rechazo y no como error del
        // comando: la interfaz muestra lo mismo que en una importación masiva.
        Err(error) => {
            return Ok(ImportReport {
                processed: 1,
                imported: 0,
                already_in_library: 0,
                duplicates: 0,
                rejected: vec![library::Rejected {
                    path: String::new(),
                    code: error.code(),
                }],
                cancelled: false,
                running: false,
            })
        }
    };

    Ok(ImportReport {
        processed: 1,
        imported,
        already_in_library: already,
        duplicates,
        rejected: Vec::new(),
        cancelled: false,
        running: false,
    })
}

/// Cancela la importación en curso.
///
/// Cancelar lo que no está corriendo no es un error: la interfaz puede pulsarlo
/// justo cuando acaba de terminar, y convertir eso en un fallo solo produciría
/// manejo de errores que no arregla nada.
///
/// **Nunca corta a mitad de un archivo**: interrumpir una copia dejaría
/// exactamente la copia parcial que el producto existe para evitar.
#[tauri::command(async)]
pub fn cancel_import(state: State<'_, AppState>) {
    state.import.cancel();
    tracing::info!("se pidió cancelar la importación");
}

/// La importación que quedó a medias, si la hay.
///
/// Se consulta al arrancar. Reanudar es gratis por la identidad de contenido
/// —lo ya importado se reconoce y se salta— pero solo servía si el
/// usuario **se acordaba** de volver a lanzarla sobre la misma carpeta, y nadie
/// se acuerda de una carpeta que eligió hace tres días.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn pending_import(state: State<'_, AppState>) -> NbResult<Option<crate::imports::ImportRun>> {
    let conn = state.db.connection()?;
    crate::imports::pendiente(&conn)
}

/// La última importación, terminada o no, para poder enseñar su informe.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn last_import(state: State<'_, AppState>) -> NbResult<Option<crate::imports::ImportRun>> {
    let conn = state.db.connection()?;
    crate::imports::ultima(&conn)
}

/// Lo que una importación dejó fuera, y por qué.
///
/// Puede ser menos de lo que dice el recuento de la ejecución: por encima del
/// tope se dejan de guardar filas y la cifra sigue siendo la de verdad. La
/// interfaz enseña las dos cosas, porque "mil rechazados, se listan los primeros
/// mil" es información y "mil rechazados" a secas con mil filas debajo es una
/// coincidencia que engaña.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn import_rejections(
    state: State<'_, AppState>,
    run_id: String,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<crate::imports::ImportRejection>> {
    let conn = state.db.connection()?;
    crate::imports::rechazos(&conn, &run_id, acotar(limit), offset)
}

/// Las copias exactas que encontró una importación, con el título y la ruta de lo
/// que ya estaba. Como los rechazos: puede ser menos que el recuento de la
/// ejecución, que es el de verdad.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn import_duplicates(
    state: State<'_, AppState>,
    run_id: String,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<crate::imports::ImportDuplicate>> {
    let conn = state.db.connection()?;
    crate::imports::copias(&conn, &run_id, acotar(limit), offset)
}

/// Da por zanjada una importación a medias sin continuarla.
///
/// Sin esto, la aplicación la ofrecería en cada arranque, que es exactamente el
/// aviso que la gente aprende a ignorar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn discard_pending_import(state: State<'_, AppState>, run_id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::imports::descartar(&conn, &run_id)
}

/// Lo que la pantalla de Inicio muestra sobre la lectura.
///
/// Sale de datos que ya existen —sesiones y progreso—, sin red y sin IA. La
/// racha se calcula y se muestra, pero no se presiona con ella: ADR-013 deja
/// fuera la culpa administrada por el software.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
#[tauri::command(async)]
pub fn reading_stats(state: State<'_, AppState>) -> NbResult<home::Lectura> {
    let conn = state.db.connection()?;
    home::lectura(&conn)
}

/// Las filas de Inicio para una visita.
///
/// Cada fila dice por qué aparece -es la condición de ADR-013 para que Inicio no
/// sea un escaparate-, y con la `seed` de la visita cambian las materias, los
/// autores y las obras que se enseñan. La misma semilla da la misma pantalla. Ver
/// `home::filas`.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
///
/// Cota: `home::OBRAS_POR_FILA` por fila, y las filas son un número fijo.
#[tauri::command(async)]
pub fn home_rows(state: State<'_, AppState>, seed: u32) -> NbResult<Vec<home::HomeRow>> {
    let conn = state.db.connection()?;
    let mut filas = home::filas(&conn, u64::from(seed))?;
    drop(conn);

    // La miniatura, por lo mismo que en `browse_library`: Inicio pinta portadas del
    // mismo tamaño que la rejilla, y todas las de la pantalla de una vez.
    let portadas = state.paths.cache.join("portadas");
    let nombres: Vec<Option<String>> = filas
        .iter()
        .flat_map(|f| f.items.iter().map(|i| i.cover_path.clone()))
        .collect();
    let mut rutas = crate::covers::para_rejilla_en_lote(&portadas, &nombres).into_iter();
    // Y lo que falte en la caché se recupera en segundo plano, como en la
    // biblioteca: Inicio es la primera pantalla, así que suele ser la primera en
    // encontrarse una caché vacía.
    let mut perdidas = Vec::new();
    for item in filas.iter_mut().flat_map(|f| f.items.iter_mut()) {
        let ruta = rutas.next().flatten();
        if ruta.is_none() && item.cover_path.is_some() {
            perdidas.push(item.edition_id.clone());
        }
        item.cover_path = ruta.map(|r| r.to_string_lossy().into_owned());
    }
    crate::metadata::recuperar_portadas_en_segundo_plano(
        state.db.clone(),
        portadas,
        state.bus.clone(),
        perdidas,
    );

    Ok(filas)
}

/// Notas propias del usuario, de la más reciente a la más antigua.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn list_notes(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<notes::Note>> {
    let conn = state.db.connection()?;
    notes::list(&conn, acotar(limit), offset)
}

/// Una nota por su identificador, o nada si ya no existe.
///
/// Existe por el tablero: una tarjeta abre su nota, y la nota puede no
/// estar entre las que la lista de Estudio tiene cargadas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn find_note(state: State<'_, AppState>, id: String) -> NbResult<Option<notes::Note>> {
    let conn = state.db.connection()?;
    notes::find(&conn, &id)
}

/// Crea una nota propia.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si el título está vacío o repetido, o
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn create_note(
    state: State<'_, AppState>,
    title: String,
    body: String,
) -> NbResult<notes::Note> {
    let conn = state.db.connection()?;
    notes::create(&conn, &title, &body)
}

/// Edita una nota propia.
///
/// A diferencia de una anotación, una nota **sí se edita**: es del usuario y no
/// ancla a nada de nadie, así que no hay ancla que romper.
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si no existe, o [`NbError::StorageFailure`].
#[tauri::command(async)]
pub fn update_note(
    state: State<'_, AppState>,
    id: String,
    title: String,
    body: String,
) -> NbResult<notes::Note> {
    let conn = state.db.connection()?;
    notes::update(&conn, &id, &title, &body)
}

/// Borra una nota propia y sus enlaces.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn delete_note(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    notes::delete(&conn, &id)
}

/// Notas que apuntan a esta.
///
/// El retroenlace no se guarda: es el mismo enlace leído del otro lado, y por
/// eso no puede desincronizarse.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: las notas del usuario que nombran a esta, no el catálogo.
#[tauri::command(async)]
pub fn note_backlinks(state: State<'_, AppState>, id: String) -> NbResult<Vec<notes::Note>> {
    let conn = state.db.connection()?;
    notes::backlinks(&conn, &id)
}

/// Cita una anotación desde una nota.
///
/// # Por qué el gesto va aquí y no en el cuerpo de la nota
///
/// Una nota cita a otra escribiendo `[[Título]]`, porque el título es lo que una
/// persona escribe y recuerda. Una anotación **no tiene nombre**: es un trozo de
/// un libro, y lo único que la identifica es dónde está. Así que se cita
/// señalándola, no nombrándola, y el enlace vive en su propia tabla en lugar de
/// derivarse del texto.
///
/// La asimetría no es un descuido: los dos referentes son distintos, y forzar la
/// misma sintaxis obligaría a escribir un identificador que nadie teclea.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la nota o la anotación no existen.
#[tauri::command(async)]
pub fn cite_annotation(
    state: State<'_, AppState>,
    note_id: String,
    annotation_id: String,
) -> NbResult<()> {
    let conn = state.db.connection()?;
    notes::link_annotation(&conn, &note_id, &annotation_id)
}

/// Deshace la cita. Que no existiera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la operación falla.
#[tauri::command(async)]
pub fn uncite_annotation(
    state: State<'_, AppState>,
    note_id: String,
    annotation_id: String,
) -> NbResult<()> {
    let conn = state.db.connection()?;
    notes::unlink_annotation(&conn, &note_id, &annotation_id)
}

/// Los pasajes que una nota cita, con el libro del que salió cada uno.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: los pasajes que cita una nota.
#[tauri::command(async)]
pub fn cited_annotations(
    state: State<'_, AppState>,
    note_id: String,
) -> NbResult<Vec<annotations::LibraryAnnotation>> {
    let conn = state.db.connection()?;
    annotations::for_note(&conn, &note_id)
}

/// Notas que citan una anotación.
///
/// Es el retroenlace visto desde el documento, y como todos los retroenlaces de
/// este proyecto **no se guarda: se consulta**. Guardarlo aparte permitiría que
/// las dos direcciones dejaran de coincidir.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: las notas que citan una anotación.
#[tauri::command(async)]
pub fn notes_citing(
    state: State<'_, AppState>,
    annotation_id: String,
) -> NbResult<Vec<notes::Note>> {
    let conn = state.db.connection()?;
    notes::notes_for_annotation(&conn, &annotation_id)
}

/// Avance de la indexación de texto completo.
///
/// # Errors
///
/// No falla: la ausencia de avance también es información.
#[tauri::command(async)]
pub fn indexing_progress(state: State<'_, AppState>) -> NbResult<IndexingReport> {
    let avance = state.indexer.progress();

    // Cuánto de la biblioteca **se puede buscar**, contado en la base y no en
    // esta sesión. La tarjeta usaba lo indexado desde que se abrió la aplicación
    // más lo que quedaba en la cola, y sobre una biblioteca indexada a medias en
    // sesiones anteriores eso no era ningún porcentaje de nada.
    let (searchable, total) = state
        .db
        .connection()
        .ok()
        .and_then(|conn| {
            conn.query_row(
                "SELECT (SELECT count(*) FROM search_status WHERE state = 'searchable'),
                        (SELECT count(*) FROM editions)",
                [],
                |fila| Ok((fila.get::<_, i64>(0)?, fila.get::<_, i64>(1)?)),
            )
            .ok()
        })
        .unwrap_or((0, 0));

    Ok(IndexingReport {
        pending: avance.pending,
        indexed: avance.indexed,
        skipped: avance.skipped,
        failed: avance.failed,
        paused: avance.paused,
        pause_reason: avance.pause_reason.map(str::to_owned),
        searchable: u32::try_from(searchable).unwrap_or(u32::MAX),
        total: u32::try_from(total).unwrap_or(u32::MAX),
        storage: state.indexer.medida(),
    })
}

/// Detiene o reanuda la indexación.
///
/// Existe porque indexar una biblioteca grande es la tarea más cara que hace
/// esta aplicación, y quien esté haciendo otra cosa con el ordenador tiene que
/// poder decirle que espere. Lo que el runtime decide solo es la parada por
/// batería; esta es la del usuario, y son distintas a propósito: reanudar a mano
/// no debe hacer que se ignore una batería al 8 %.
///
/// # Errors
///
/// No falla.
#[tauri::command(async)]
pub fn set_indexing_paused(state: State<'_, AppState>, paused: bool) -> NbResult<()> {
    if paused {
        state.indexer.pause();
    } else {
        state.indexer.resume();
    }
    Ok(())
}

/// Avance de la indexación, tal y como cruza el puente.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct IndexingReport {
    pub pending: u32,
    pub indexed: u32,
    pub skipped: u32,
    pub failed: u32,
    pub paused: bool,
    /// Código canónico, nunca un mensaje: el texto lo pone la interfaz.
    pub pause_reason: Option<String>,
    /// Cuántas obras se pueden buscar por dentro, de todas las que hay.
    pub searchable: u32,
    pub total: u32,
    /// Lo que ocupa lo derivado y el tope vigente, de la última vez que el
    /// indexador lo midió. Es lo que explica por qué se paró.
    pub storage: Option<crate::diagnostics::DerivedStorage>,
}

/// Las iniciales de la biblioteca ordenada por título o autor, con dónde empieza
/// cada una, para saltar con anclas alfabéticas.
///
/// Cota: [`crate::browse::MAXIMO_DE_ANCLAS`].
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn browse_letter_anchors(
    state: State<'_, AppState>,
    query: crate::browse::BrowseQuery,
) -> NbResult<Vec<crate::browse::LetterAnchor>> {
    let conn = state.db.connection()?;
    crate::browse::letter_anchors(&conn, &query)
}

/// Devuelve una ventana de la biblioteca, ya ordenada y filtrada.
///
/// **Ordenar y filtrar no se hacen aquí ni en la interfaz: se hacen en SQL.** La
/// biblioteca de referencia tiene 152.080 obras y el presupuesto de nodos del
/// documento es de 1.500. Traer la lista entera al webview para ordenarla allí
/// no cabe, y el límite por página está acotado por
/// [`browse::MAXIMO_POR_PAGINA`] porque lo que pide el webview es entrada no
/// confiable.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn browse_library(
    state: State<'_, AppState>,
    query: crate::browse::BrowseQuery,
) -> NbResult<crate::browse::BrowsePage> {
    let conn = state.db.connection()?;
    let mut pagina = crate::browse::browse(&conn, &query)?;

    // La portada se guarda por su **nombre** y cruza el puente como **ruta**.
    //
    // En el catálogo va el nombre porque la caché cambia de sitio entre equipos
    // y versiones del sistema: una ruta guardada sería una ruta rota. Al webview
    // tiene que llegar la ruta, porque es lo que el protocolo de assets sabe
    // servir. Quien sabe dónde está la caché de **esta** instalación es el
    // runtime, así que resuelve aquí y nadie más tiene que saberlo.
    //
    // Y lo que se sirve es la **miniatura**. Las portadas reales son de 800x1200
    // y la rejilla las pinta a 200x300: descodificar y reescalar el original
    // cincuenta veces por fotograma es lo que hacía que la biblioteca se
    // recorriera a veinticinco fotogramas por segundo en lugar de a sesenta.
    // Medido en WebKitGTK, que es el motor de verdad; ver `covers.rs`.
    let portadas = state.paths.cache.join("portadas");
    let nombres: Vec<Option<String>> = pagina.items.iter().map(|i| i.cover_path.clone()).collect();
    let rutas = crate::covers::para_rejilla_en_lote(&portadas, &nombres);

    // Lo que el catálogo apunta y la caché ya no tiene se vuelve a sacar del
    // documento, en segundo plano. La caché es caché: vaciarla no puede dejar la
    // biblioteca llena de imágenes rotas para siempre, que es lo que pasaba hasta
    // el 2026-09-18. Ver `metadata::recuperar_portadas`.
    let mut perdidas = Vec::new();
    for ((item, ruta), nombre) in pagina.items.iter_mut().zip(rutas).zip(&nombres) {
        if ruta.is_none() && nombre.is_some() {
            perdidas.push(item.edition_id.clone());
        }
        item.cover_path = ruta.map(|r| r.to_string_lossy().into_owned());
    }
    crate::metadata::recuperar_portadas_en_segundo_plano(
        state.db.clone(),
        portadas,
        state.bus.clone(),
        perdidas,
    );

    Ok(pagina)
}

/// Cuántas obras hay detrás de cada filtro fijo de la biblioteca.
///
/// La barra de la biblioteca solo ofrece los filtros que llevan a algo: con la
/// biblioteca sana, "Duplicados" y "No disponibles" son cero y no se pintan.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn library_facets(state: State<'_, AppState>) -> NbResult<crate::browse::LibraryFacets> {
    let conn = state.db.connection()?;
    crate::browse::facets(&conn)
}

/// Estanterías de una clase: autores, series o materias, con su tamaño.
///
/// No construye nada nuevo. Agrupar por autor o por serie es lo que ya hacía
/// una colección por regla, evaluada al consultar y por eso nunca
/// obsoleta. Aquí se presenta lo que ya existía.
///
/// **Se busca, no se lista**, y el filtrado va en SQL. Con 152.080 obras hay
/// decenas de miles de autores: traerlos para elegir uno es el mismo error que
/// traer la biblioteca entera para enseñar una página.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn library_shelves(
    state: State<'_, AppState>,
    query: crate::browse::ShelfQuery,
) -> NbResult<crate::browse::ShelfPage> {
    let conn = state.db.connection()?;
    crate::browse::shelves(&conn, &query)
}

/// Si un documento se puede buscar, y por qué no.
///
/// Lo que devuelve no es un booleano: "todavía no se ha indexado" y "no tiene
/// texto" son cosas distintas, y presentarlas igual haría que la interfaz
/// dijera "no" mientras la respuesta correcta es "aún no".
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn search_status(
    state: State<'_, AppState>,
    edition_id: String,
) -> NbResult<crate::search::SearchStatus> {
    let conn = state.db.connection()?;
    crate::search::status_of(&conn, &edition_id)
}

/// Busca en el alcance que se le diga.
///
/// Los tres alcances no son un filtro sobre un mismo resultado: son consultas
/// distintas contra índices distintos, y devuelven cosas distintas. El catálogo
/// devuelve **documentos**; el corpus y el documento abierto, **párrafos**.
/// Meter el título en el mismo índice que el texto haría que buscar un libro
/// devolviera doscientos párrafos que lo mencionan antes que el libro.
///
/// El acotado por colección viaja aquí y se resuelve en SQL. Traerse la
/// colección entera al webview para cruzarla allí es lo que
/// `docs/architecture/performance-budgets.md` prohíbe: una colección
/// de veinte mil libros cruzaría el puente en cada pulsación de tecla.
///
/// Una búsqueda nueva **corta** la anterior si sigue corriendo: la anterior
/// responde [`NbError::Cancelled`], que la interfaz ya descarta por obsoleta.
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si la consulta falla.
/// - [`NbError::Cancelled`] si otra búsqueda la sustituyó mientras corría.
#[tauri::command(async)]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    scope: crate::search::Scope,
    limit: usize,
    offset: usize,
) -> NbResult<crate::search::Results> {
    let conn = state.db.connection()?;
    // El turno se declara **después** de la conexión: se suelta antes que ella,
    // y así la interrupción nunca puede alcanzar la conexión cuando ya atiende a
    // otro comando. Ver `search::BusquedaEnCurso`.
    let turno = state.busqueda.empezar(&conn);
    // El límite se acota aquí y no se confía al webview: una petición con un
    // límite de un millón traería la biblioteca entera por el puente, y el
    // webview es zona no confiable.
    let limite = limit.clamp(1, 200);
    let resultados = crate::search::search(&conn, &query, &scope, limite, offset)?;

    /*
     * El pasaje de cada resultado se compone aquí y no dentro de la consulta,
     * porque no sale del índice: se relee del documento con el
     * locator del fragmento, y para eso hacen falta los lectores, que viven en
     * el estado y no en la base. Un resultado cuyo volumen está desconectado se
     * queda sin pasaje, y eso es la respuesta y no un hueco.
     */
    Ok(match resultados {
        crate::search::Results::Text(hits) => crate::search::Results::Text(
            crate::search::con_pasajes_mientras(&conn, &state.readers, &query, hits, || {
                turno.vigente()
            }),
        ),
        otros => otros,
    })
}

/// Rehace el índice de búsqueda desde cero.
///
/// Es la salida para cuando algo no cuadra y no se sabe por qué. No pide
/// confirmación destructiva porque no hay nada que perder: el índice es caché y
/// se rehace leyendo los documentos, que siguen donde estaban.
///
/// Devuelve cuántas ediciones han quedado encoladas. El trabajo ocurre en
/// segundo plano; este comando no espera a que termine, porque sobre una
/// biblioteca grande son horas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn reindex(state: State<'_, AppState>) -> NbResult<usize> {
    let conn = state.db.connection()?;
    crate::search::reset(&conn)?;
    let encoladas = state.indexer.enqueue_missing(&conn)?;
    tracing::info!(
        encoladas,
        "índice de búsqueda rehecho a petición del usuario"
    );
    Ok(encoladas)
}

// ---------------------------------------------------------------------------
// Consulta sobre la selección y vocabulario.
// ---------------------------------------------------------------------------

/// Consulta la selección en los proveedores instalados.
///
/// # Qué hace además de consultar
///
/// Dos cosas que la interfaz no puede hacer por sí sola:
///
/// 1. **Decide si se puede salir a la red**, leyendo el interruptor general. Un
///    proveedor que necesita red se omite con su motivo en lugar de fallar, que
///    es lo que permite al panel decir *"esto se quedó fuera porque estás sin
///    red"* en lugar de enseñar un hueco.
/// 2. **Apunta la palabra en el vocabulario** cuando un diccionario respondió y
///    la consulta traía dónde estaba leyendo. Ver [`crate::vocabulary`].
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el término está vacío o pasa de
///   [`MAXIMO_TERMINO`].
/// - [`NbError::ComponentUnavailable`] si esta instalación no tiene **ningún**
///   proveedor de consulta. Es distinto de que ninguno cubra el idioma: lo
///   primero hace que la interfaz oculte la función; lo segundo es una respuesta
///   vacía explicada.
/// - [`NbError::StorageFailure`] si no se puede leer el interruptor de red.
#[tauri::command(async)]
pub fn lookup_selection(
    state: State<'_, AppState>,
    term: String,
    context: Option<String>,
    language: Option<String>,
    edition_id: Option<String>,
    locator: Option<Locator>,
) -> NbResult<crate::lookup::Outcome> {
    let termino = crate::lookup::termino_valido(&term)?;

    let conn = state.db.connection()?;
    let permitir_red = !crate::net::is_offline(&conn)?;

    // El idioma lo sabe la obra, no la interfaz. Si no viene declarado se
    // resuelve aquí desde la edición: pedírselo al webview obligaría a que la
    // sesión lo transportara solo para esto, y a que dos sitios pudieran
    // discrepar sobre en qué idioma está un libro.
    let idioma = match (language, edition_id.as_deref()) {
        (Some(declarado), _) => Some(declarado),
        (None, Some(edicion)) => idioma_de_la_edicion(&conn, edicion),
        (None, None) => None,
    };

    let consulta = crate::lookup::Query {
        term: termino.to_owned(),
        context: context.map(|c| c.chars().take(crate::lookup::MAXIMO_TERMINO).collect()),
        language: idioma,
    };

    let registro = state.lookup.read().map_err(|_| {
        tracing::error!("el registro de proveedores de consulta quedó envenenado");
        NbError::Internal
    })?;
    let resultado = registro.lookup(&consulta, permitir_red)?;
    drop(registro);

    if let (Some(edition_id), Some(locator)) = (edition_id, locator) {
        apuntar_vocabulario(&conn, &edition_id, &locator, termino, &resultado);
    }

    Ok(resultado)
}

/// Idioma de una edición, o el de su obra si la edición no lo declara.
///
/// Devuelve `None` cuando no se sabe, que **no** es lo mismo que un idioma que
/// nadie cubre: sin idioma declarado el registro ofrece la consulta a todos los
/// proveedores, porque es mejor una consulta de más que ninguna.
fn idioma_de_la_edicion(conn: &rusqlite::Connection, edition_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT coalesce(e.language, w.original_language)
           FROM editions e JOIN works w ON w.id = e.work_id
          WHERE e.id = ?1",
        [edition_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
}

/// Apunta en el vocabulario la primera definición que haya respondido.
///
/// # Por qué la primera y no todas
///
/// Porque lo que se aprende es la palabra, no cada acepción: guardar una fila
/// por diccionario instalado multiplicaría la lista sin añadir nada que
/// estudiar. Y **solo diccionario**: una traducción de una frase entera no es
/// vocabulario, y dejaría la lista con párrafos dentro.
///
/// Un fallo al apuntar no rompe la consulta: la palabra ya está en pantalla, y
/// perder el apunte es peor que nada pero mucho menos que perder la respuesta.
fn apuntar_vocabulario(
    conn: &rusqlite::Connection,
    edition_id: &str,
    locator: &Locator,
    termino: &str,
    resultado: &crate::lookup::Outcome,
) {
    let Some(entrada) = resultado
        .entries
        .iter()
        .find(|e| e.kind == crate::lookup::LookupKind::Dictionary)
    else {
        return;
    };

    if let Err(error) = crate::vocabulary::record(
        conn,
        edition_id,
        &entrada.provider_id,
        &entrada.headword,
        termino,
        locator,
    ) {
        tracing::warn!(
            code = error.code(),
            "no se pudo apuntar una palabra en el vocabulario"
        );
    }
}

/// Diccionarios instalados en esta máquina.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la carpeta existe y no se deja leer.
///
/// Cota: los diccionarios instalados, que son las carpetas de un directorio.
#[tauri::command(async)]
pub fn list_dictionaries(
    state: State<'_, AppState>,
) -> NbResult<Vec<crate::lookup::store::Manifest>> {
    crate::lookup::store::list(&state.paths.dictionaries())
}

/// Instala un diccionario a partir de su archivo `.ifo`.
///
/// # De dónde puede venir esta ruta
///
/// Del diálogo nativo o de un gesto del usuario, igual que en [`import_path`].
/// Se canonicaliza antes de tocar nada porque el webview es zona no confiable.
///
/// # Errors
///
/// - [`NbError::FileNotFound`] si la ruta no existe o falta alguno de los tres
///   archivos del diccionario.
/// - [`NbError::DocumentCorrupted`] si está truncado o el índice no cuadra.
/// - [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn install_dictionary(
    state: State<'_, AppState>,
    path: String,
    language: Option<String>,
) -> NbResult<crate::lookup::store::Manifest> {
    let canonica = std::path::PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| {
            tracing::info!("se intentó instalar un diccionario de una ruta que no se canonicaliza");
            NbError::FileNotFound
        })?;

    let carpeta = state.paths.dictionaries();
    let manifiesto = crate::lookup::store::install(&carpeta, &canonica, language.as_deref())?;
    refrescar_diccionarios(&state, &carpeta)?;
    Ok(manifiesto)
}

/// Desinstala un diccionario. Que no estuviera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede borrar.
#[tauri::command(async)]
pub fn uninstall_dictionary(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let carpeta = state.paths.dictionaries();
    crate::lookup::store::uninstall(&carpeta, &id)?;
    refrescar_diccionarios(&state, &carpeta)
}

/// Vuelve a construir el registro desde la carpeta.
///
/// Se hace aquí y no dentro de `store` porque el registro vive en el estado de
/// la aplicación: instalar sin esto obligaría a reiniciar para poder consultar
/// lo que se acaba de instalar, y desinstalar dejaría respondiendo a un
/// diccionario que ya no está en disco.
fn refrescar_diccionarios(state: &State<'_, AppState>, carpeta: &std::path::Path) -> NbResult<()> {
    let mut nuevo = crate::lookup::instalados(carpeta);
    // Los de red vuelven a entrar: el registro se reemplaza entero, así que
    // reconstruir solo los diccionarios daría de baja la enciclopedia y el
    // traductor al instalar un diccionario, que es justo lo contrario de lo que
    // el usuario acaba de pedir.
    crate::registrar_consulta_en_linea(&mut nuevo, &state.db, &state.net);

    let mut registro = state.lookup.write().map_err(|_| {
        tracing::error!("el registro de proveedores de consulta quedó envenenado");
        NbError::Internal
    })?;
    *registro = nuevo;
    Ok(())
}

/// El vocabulario consultado, de lo más reciente a lo más antiguo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn list_vocabulary(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<crate::vocabulary::VocabularyItem>> {
    let conn = state.db.connection()?;
    crate::vocabulary::list(&conn, acotar(limit), offset)
}

/// Borra una palabra del vocabulario.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn delete_vocabulary(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::vocabulary::delete(&conn, &id)
}

/// Borra el vocabulario entero. Devuelve cuántas palabras se borraron.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn clear_vocabulary(state: State<'_, AppState>) -> NbResult<usize> {
    let conn = state.db.connection()?;
    crate::vocabulary::clear(&conn)
}

// ---------------------------------------------------------------------------
// Red: lo que se ha autorizado y lo que ha salido.
// ---------------------------------------------------------------------------

/// Cuántas líneas del registro de salidas se devuelven.
///
/// El registro está acotado en la base, pero traerlo entero por el puente en
/// cada apertura de Ajustes sería mover doscientas filas para enseñar veinte.
const LINEAS_DEL_REGISTRO: usize = 60;

/// Una raíz de biblioteca, con lo que hay que saber para decidir sobre ella.
///
/// Lleva la cuenta de archivos porque una raíz vacía y una con veinte mil
/// documentos ausentes no piden lo mismo, y sin el número la interfaz no puede
/// decir cuál es cuál.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct LibraryRootView {
    pub id: String,
    pub name: String,
    /// `managed` o `referenced`. Solo una referenciada se reapunta.
    pub kind: String,
    /// Dónde está. Vacía en una referenciada significa **no localizada**.
    pub path: Option<String>,
    pub files: u32,
    /// Cuántos de sus archivos no están donde dice el catálogo.
    pub missing: u32,
}

/// Las raíces de la biblioteca y el estado de sus archivos.
///
/// # Por qué esto existe
///
/// Porque sin ello, mudar los libros de disco es perderlos. `LibraryRoot` y
/// `refresh_all_availability` llevaban resolviendo exactamente ese
/// caso, probados en `tests/mudanza.rs`, y **no los llamaba nadie desde la
/// interfaz**: la capacidad existía y no se podía usar. La regla es
/// "ninguna pérdida", y una biblioteca que no sabe seguir a sus archivos cuando
/// cambian de sitio es una pérdida.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
///
/// Cota: las raíces de biblioteca dadas de alta, que se añaden de una en una.
#[tauri::command(async)]
pub fn library_roots(state: State<'_, AppState>) -> NbResult<Vec<LibraryRootView>> {
    let conn = state.db.connection()?;
    let raices = crate::db::catalog::LibraryRoot::list(&conn)?;

    let mut salida = Vec::with_capacity(raices.len());
    for raiz in raices {
        let (files, missing) = library::cuentas_de_raiz(&conn, &raiz.id)?;
        salida.push(LibraryRootView {
            id: raiz.id,
            name: raiz.name,
            kind: raiz.kind.as_str().to_owned(),
            path: raiz.path,
            files,
            missing,
        });
    }
    Ok(salida)
}

/// Vuelve a apuntar una raíz a donde estén ahora sus archivos.
///
/// Es la mudanza entera: una llamada por raíz devuelve todos sus documentos, sin
/// reimportar y sin tocar una sola anotación. Después se revisa la
/// disponibilidad, porque reapuntar sin comprobar dejaría la biblioteca diciendo
/// que falta lo que ya está.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la ruta no existe o no es una carpeta.
/// - [`NbError::StorageFailure`] si la raíz no existe, es administrada, o la
///   escritura falla.
#[tauri::command(async)]
pub fn relocate_library_root(
    state: State<'_, AppState>,
    root_id: String,
    path: String,
) -> NbResult<library::AvailabilityReport> {
    // La ruta viene del webview, que es zona no confiable. Que exista y sea una
    // carpeta se comprueba aquí: reapuntar a algo que no está dejaría la
    // biblioteca peor que antes y sin decirlo.
    //
    // Y se guarda **canonicalizada**, como la guarda la importación. Antes     // se guardaba tal como llegaba: una raíz escrita con `..`, con una barra de
    // más o a través de un enlace simbólico no casaba con las rutas canónicas de
    // sus archivos, y la biblioteca se veía como no disponible recién reapuntada.
    let destino = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|_| NbError::PathNotAllowed)?;
    if !destino.is_dir() {
        tracing::info!("se rechazó reapuntar una raíz a algo que no es una carpeta");
        return Err(NbError::PathNotAllowed);
    }
    let Some(destino) = destino.to_str() else {
        tracing::info!("se rechazó reapuntar una raíz a una ruta que no es UTF-8");
        return Err(NbError::PathNotAllowed);
    };

    let mut conn = state.db.connection()?;
    crate::db::catalog::LibraryRoot::relocate(&conn, &root_id, destino)?;
    state.repaso.olvidar_foto(&root_id);
    tracing::info!("el usuario reapuntó una raíz de biblioteca");
    library::refresh_all_availability(&mut conn)
}

/// Revisa dónde está cada archivo de la biblioteca.
///
/// Consulta metadatos del sistema de archivos, no contenido: comprobar si un
/// archivo está no puede costar lo que costaría leerlo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
#[tauri::command(async)]
pub fn refresh_availability(state: State<'_, AppState>) -> NbResult<library::AvailabilityReport> {
    let mut conn = state.db.connection()?;
    library::refresh_all_availability(&mut conn)
}

/// Lo que la interfaz enseña sobre la red.
///
/// Va todo junto porque son tres respuestas a una sola pregunta —¿qué sale de
/// este equipo?— y pedirlas por separado haría que la pantalla se pintara tres
/// veces con verdades parciales.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct NetworkReport {
    /// El interruptor general. Con él puesto, nada sale.
    pub offline: bool,
    pub consents: Vec<crate::net::Consent>,
    /// Lo que salió y lo que se intentó, de lo más reciente a lo más antiguo.
    pub log: Vec<crate::net::LogEntry>,
    /// Destinos que la aplicación conoce y que **todavía no pueden salir**.
    ///
    /// Son los catálogos añadidos cuyo anfitrión no tiene permiso. Existen
    /// porque anotar un catálogo no autoriza a salir a él —y no debe: sería
    /// convertir un formulario en un permiso—, pero sin enseñarlos aquí el único
    /// sitio donde se podía conceder era el aviso de error de Descubrir, es
    /// decir, había que tropezar con el fallo para poder arreglarlo.
    pub pending: Vec<PendingDestination>,
}

/// Un destino conocido que aún no tiene permiso de salida.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct PendingDestination {
    pub host: String,
    /// Cómo lo reconoce el usuario: el nombre que le puso al catálogo.
    pub label: String,
    /// Qué se enviaría, en lenguaje llano. Sale del propio propósito.
    pub what_is_sent: String,
    /// Si lo que falta es el permiso de hablarle **sin cifrar**.
    pub cleartext: bool,
}

/// Estado de la salida de red: interruptor, permisos y registro.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn network_report(state: State<'_, AppState>) -> NbResult<NetworkReport> {
    let conn = state.db.connection()?;

    // Los pendientes se calculan aquí y no en la interfaz: cruzar catálogos con
    // permisos desde la vista de Ajustes la obligaría a cargar los catálogos,
    // que no son asunto suyo, y a repetir la regla de qué cuenta como permiso.
    let mut pending = Vec::new();
    for catalogo in crate::opds::list(&conn)? {
        let Ok((host, cleartext)) = crate::net::anfitrion_de_catalogo(&catalogo.url) else {
            continue;
        };
        let proposito = if cleartext {
            crate::net::Purpose::CatalogCleartext
        } else {
            crate::net::Purpose::Catalog
        };
        if crate::net::has_consent(&conn, &host, proposito)? {
            continue;
        }
        if pending.iter().any(|p: &PendingDestination| p.host == host) {
            continue;
        }
        pending.push(PendingDestination {
            host,
            label: catalogo.title,
            what_is_sent: proposito.what_is_sent().to_owned(),
            cleartext,
        });
    }

    Ok(NetworkReport {
        offline: crate::net::is_offline(&conn)?,
        consents: crate::net::consents(&conn)?,
        log: crate::net::log(&conn, LINEAS_DEL_REGISTRO)?,
        pending,
    })
}

/// Revoca un permiso de salida. Que no existiera no es un error.
///
/// **Solo revoca.** Conceder no está aquí a propósito: un permiso se da
/// respondiendo a una petición concreta, con el anfitrión y el propósito
/// delante, y un comando genérico de conceder permitiría que la interfaz los
/// diera por su cuenta. Quitar, en cambio, tiene que poder hacerse siempre y sin
/// que nadie lo pida.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el propósito no es uno de los cuatro.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn revoke_network_consent(
    state: State<'_, AppState>,
    host: String,
    purpose: String,
) -> NbResult<()> {
    let conn = state.db.connection()?;
    let purpose = crate::net::purpose_de(&purpose)?;
    crate::net::revoke(&conn, &host, purpose)?;
    tracing::info!(
        purpose = purpose.as_str(),
        "el usuario revocó un permiso de salida"
    );
    Ok(())
}

/// Los modelos de IA del catálogo, como los enseña Ajustes.
///
/// Cota: el catálogo, compilado en el binario.
///
/// # Errors
///
/// No falla: lo que no se pudo medir —el espacio libre, la memoria— viaja como
/// `None` en lugar de impedir enseñar el resto.
#[tauri::command(async)]
pub fn ai_models(state: State<'_, AppState>) -> NbResult<crate::ai::subsistema::VistaDeModelos> {
    Ok(state.ia.vista())
}

/// Empieza a instalar un modelo. **No espera a terminar.**
///
/// Devuelve `true` si ya había una instalación en marcha, que no es un error. El
/// avance llega por `AiModelProgress` y el final, por `AiModelFinished`, igual
/// que la importación y por lo mismo: doscientos megabytes no pueden dejar una
/// llamada abierta.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si el identificador no es del catálogo.
/// - [`NbError::ComponentUnavailable`] si el equipo no da para ejecutarlo.
#[tauri::command(async)]
pub fn install_ai_model(state: State<'_, AppState>, id: String) -> NbResult<bool> {
    state.ia.instalar(
        &id,
        state.db.clone(),
        std::sync::Arc::clone(&state.net),
        state.bus.clone(),
    )
}

/// Pide cortar la instalación en curso. Sin ninguna, no hace nada.
///
/// # Errors
///
/// No falla.
#[tauri::command(async)]
pub fn cancel_ai_model_install(state: State<'_, AppState>) -> NbResult<()> {
    state.ia.cancelar();
    Ok(())
}

/// Quita un modelo instalado.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si el identificador no es del catálogo.
/// - [`NbError::StorageFailure`] si no se puede borrar, o si se está instalando.
#[tauri::command(async)]
pub fn uninstall_ai_model(state: State<'_, AppState>, id: String) -> NbResult<()> {
    state.ia.desinstalar(&id)
}

/// Autoriza la descarga de modelos desde un anfitrión.
///
/// Solo el del catálogo o el que la última instalación pidió autorizar, que es la
/// red de distribución a la que redirigió: el webview no decide a dónde se sale.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el anfitrión no lo propuso el runtime.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn allow_model_host(state: State<'_, AppState>, host: String) -> NbResult<()> {
    state.ia.autorizar(&state.db, &host)
}

/// Lo que costaría vectorizar un alcance: documentos, fragmentos, disco y tiempo.
/// Se pide **antes** de ofrecer empezar.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la colección no existe.
/// - [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn estimate_vectors(
    state: State<'_, AppState>,
    scope: crate::ai::generacion::AlcanceDeVectores,
) -> NbResult<crate::ai::generacion::Estimacion> {
    state.vectores.estimar(&scope)
}

/// Empieza a vectorizar un alcance en segundo plano. El avance llega por
/// `VectorJobProgress` y el final por `VectorJobFinished`.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si no hay modelo de embeddings listo.
/// - [`NbError::DocumentNotFound`] si la colección no existe.
/// - [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn start_vectors(
    state: State<'_, AppState>,
    scope: crate::ai::generacion::AlcanceDeVectores,
) -> NbResult<()> {
    state.vectores.empezar(&scope)
}

/// Pausa o reanuda la generación de vectores.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn pause_vectors(state: State<'_, AppState>, paused: bool) -> NbResult<()> {
    state.vectores.pausar(paused);
    Ok(())
}

/// Corta la generación de vectores. Lo ya calculado se queda.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn cancel_vectors(state: State<'_, AppState>) -> NbResult<()> {
    state.vectores.cancelar();
    Ok(())
}

/// Cómo va la generación de vectores.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn vector_job(state: State<'_, AppState>) -> NbResult<crate::ai::generacion::EstadoDelTrabajo> {
    Ok(state.vectores.estado())
}

/// El lector está pintando: la generación de fondo se aparta un momento.
///
/// Lo llama el lector al desplazar o pasar página, y solo con un trabajo en
/// marcha.
///
/// # Errors
///
/// Ninguno: se declara por el contrato del puente.
#[tauri::command(async)]
pub fn reader_activity(state: State<'_, AppState>) -> NbResult<()> {
    state.vectores.apuntar_actividad_del_lector();
    Ok(())
}

/// Si una edición o una colección está excluida de la búsqueda por significado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn vector_exclusion(
    state: State<'_, AppState>,
    kind: crate::ai::generacion::Exclusion,
    id: String,
) -> NbResult<bool> {
    let conn = state.db.connection()?;
    crate::ai::generacion::esta_excluido(&conn, kind, &id)
}

/// Excluye o vuelve a incluir una edición o una colección. Excluir borra sus
/// vectores.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la colección no existe.
/// - [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn set_vector_exclusion(
    state: State<'_, AppState>,
    kind: crate::ai::generacion::Exclusion,
    id: String,
    excluded: bool,
) -> NbResult<()> {
    crate::ai::generacion::excluir(&state.db, state.vectores.indice(), kind, &id, excluded)
}

/// Busca por significado en lo preparado de un alcance.
///
/// **Aparte de `search`, y a propósito**: la interfaz pide primero la búsqueda
/// léxica, que no cambia, y esta después. Si esta falla o no existe, lo léxico ya
/// está en pantalla. Sin nada preparado en el alcance contesta vacío sin calcular
/// nada.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si no hay modelo de embeddings listo.
/// - [`NbError::DocumentNotFound`] si la colección no existe.
/// - [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    scope: crate::search::Scope,
    limit: usize,
) -> NbResult<Vec<crate::ai::busqueda::Cercania>> {
    // Acotado aquí: el webview es zona no confiable.
    state.vectores.buscar(&query, &scope, limit.clamp(1, 20))
}

/// Quién generaría las funciones de estudio ahora, y si puede.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si las preferencias no se pueden leer.
#[tauri::command(async)]
pub fn study_availability(
    state: State<'_, AppState>,
) -> NbResult<crate::ai::subsistema::DisponibilidadDeEstudio> {
    state.ia.disponibilidad_de_estudio(&state.db, &state.net)
}

/// Hace una función de estudio sobre una parte de un documento, con quien eligió
/// la persona en Ajustes.
///
/// Tarda: con un modelo local, un par de minutos. Corre fuera del hilo de la
/// ventana, y [`cancel_study`] la corta.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si no hay quien genere.
/// - [`NbError::ConsentRequired`] si quien genera es externo y no se ha autorizado
///   salir a él: no sale nada, y la interfaz enseña a dónde y qué saldría.
/// - [`NbError::Cancelled`] si se canceló.
/// - [`NbError::DocumentNotFound`] si el alcance no tiene texto, y los de abrir el
///   documento o generar.
#[tauri::command(async)]
pub fn generate_study(
    state: State<'_, AppState>,
    function: crate::ai::estudio::Funcion,
    scope: crate::ai::estudio::AlcanceDeEstudio,
) -> NbResult<crate::ai::estudio::Generacion> {
    let documentos = crate::ai::generacion::DeLaBiblioteca {
        db: state.db.clone(),
        registry: std::sync::Arc::clone(&state.readers),
    };
    state.ia.estudiar(&state.db, &documentos, function, &scope)
}

/// Estima el texto que saldría para una parte de un documento, en caracteres y
/// tokens aproximados.
///
/// # Errors
///
/// Los de abrir el documento.
#[tauri::command(async)]
pub fn estimate_study_scope(
    state: State<'_, AppState>,
    scope: crate::ai::estudio::AlcanceDeEstudio,
) -> NbResult<crate::ai::estudio::EstimacionDeAlcance> {
    let documentos = crate::ai::generacion::DeLaBiblioteca {
        db: state.db.clone(),
        registry: std::sync::Arc::clone(&state.readers),
    };
    state.ia.estimar_alcance(&state.db, &documentos, &scope)
}

/// Corta la generación en curso, si la hay.
#[tauri::command(async)]
pub fn cancel_study(state: State<'_, AppState>) {
    state.ia.cancelar_estudio();
}

/// Guarda tarjetas y preguntas aceptadas. Todas o ninguna.
///
/// Cota: `cuaderno::MAXIMO_POR_GUARDADO` identificadores, los de las que se mandan,
/// que el runtime rechaza por encima.
///
/// # Errors
///
/// - [`NbError::Internal`] si alguna no cumple.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn save_study_cards(
    state: State<'_, AppState>,
    cards: Vec<crate::ai::cuaderno::NuevaTarjeta>,
) -> NbResult<Vec<String>> {
    let mut conn = state.db.connection()?;
    crate::ai::cuaderno::guardar_tarjetas(&mut conn, &cards)
}

/// Las tarjetas y preguntas guardadas, de un libro o de todos.
///
/// Si `due_only` es `Some(true)`, filtra por las que tocan hoy.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn study_cards(
    state: State<'_, AppState>,
    edition_id: Option<String>,
    due_only: Option<bool>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<crate::ai::cuaderno::Tarjeta>> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::tarjetas(
        &conn,
        edition_id.as_deref(),
        due_only.unwrap_or(false),
        acotar(limit),
        offset,
    )
}

/// Registra la respuesta de repaso de una tarjeta o pregunta, recalculando
/// su intervalo, facilidad y próxima fecha según SM-2 determinista.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn review_study_card(
    state: State<'_, AppState>,
    id: String,
    rating: crate::ai::repaso_espaciado::CalificacionRepaso,
) -> NbResult<crate::ai::cuaderno::Tarjeta> {
    let mut conn = state.db.connection()?;
    crate::ai::cuaderno::responder_tarjeta(&mut conn, &id, rating)
}

/// Cantidad total y pendientes de repasar hoy para las tarjetas y preguntas de estudio.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn study_card_counts(
    state: State<'_, AppState>,
    edition_id: Option<String>,
) -> NbResult<crate::ai::cuaderno::CuentasDeTarjetas> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::cuentas_de_tarjetas(&conn, edition_id.as_deref())
}

/// Edita una tarjeta o una pregunta. Su procedencia no cambia.
///
/// # Errors
///
/// - [`NbError::Internal`] si no cumple.
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn edit_study_card(
    state: State<'_, AppState>,
    id: String,
    edit: crate::ai::cuaderno::EdicionDeTarjeta,
) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::editar_tarjeta(&conn, &id, &edit)
}

/// Descarta una tarjeta o una pregunta guardada.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn delete_study_card(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::borrar_tarjeta(&conn, &id)
}

/// Guarda conceptos aceptados. Uno que ya estaba en el libro junta sus fuentes.
///
/// Cota: `cuaderno::MAXIMO_POR_GUARDADO` identificadores, como `save_study_cards`.
///
/// # Errors
///
/// - [`NbError::Internal`] si alguno no cumple.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn save_concepts(
    state: State<'_, AppState>,
    concepts: Vec<crate::ai::cuaderno::NuevoConcepto>,
) -> NbResult<Vec<String>> {
    let mut conn = state.db.connection()?;
    crate::ai::cuaderno::guardar_conceptos(&mut conn, &concepts)
}

/// Los conceptos guardados, de un libro o de todos.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn concepts(
    state: State<'_, AppState>,
    edition_id: Option<String>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<crate::ai::cuaderno::Concepto>> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::conceptos(&conn, edition_id.as_deref(), acotar(limit), offset)
}

/// Edita un concepto. Su procedencia no cambia.
///
/// # Errors
///
/// - [`NbError::Internal`] si no cumple.
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn edit_concept(
    state: State<'_, AppState>,
    id: String,
    name: String,
    kind: crate::ai::salidas::TipoDeConcepto,
    description: String,
) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::editar_concepto(&conn, &id, &name, kind, &description)
}

/// Borra un concepto guardado.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn delete_concept(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::ai::cuaderno::borrar_concepto(&conn, &id)
}

/// Cuántos envíos a un proveedor externo puede pedir la interfaz de una vez.
const MAXIMO_DE_ENVIOS_DE_IA: usize = 200;

/// El proveedor de IA externo: cuál es, a dónde sale, si está autorizado y si
/// tiene credencial. **Nunca la credencial.**
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn ai_provider_status(
    state: State<'_, AppState>,
) -> NbResult<crate::ai::subsistema::VistaDelProveedor> {
    state.ia.vista_del_proveedor(&state.db)
}

/// Prueba el proveedor externo.
///
/// **La primera vez no envía nada**: devuelve el mensaje entero que enviaría, y
/// hace falta autorizar con `allow_ai_provider` para que salga.
///
/// # Errors
///
/// - [`NbError::NetworkDisabled`] con el modo sin red puesto.
/// - [`NbError::NetworkFailure`] si no se pudo hablar con el servicio.
/// - [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn probe_ai_provider(state: State<'_, AppState>) -> NbResult<crate::ai::subsistema::Prueba> {
    state.ia.probar(&state.db)
}

/// Autoriza el envío al proveedor externo **configurado**. No recibe a cuál.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay proveedor externo configurado.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn allow_ai_provider(state: State<'_, AppState>) -> NbResult<()> {
    state.ia.autorizar_externo(&state.db)
}

/// Guarda en el llavero la credencial del proveedor externo que hay escrito.
///
/// No acepta de quién es, por lo mismo que `set_speech_credential`.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay dirección válida, o el secreto está
///   vacío o pasa del límite.
/// - [`NbError::ComponentUnavailable`] si el sistema no tiene llavero.
/// - [`NbError::StorageFailure`] si el llavero rechaza la escritura.
#[tauri::command(async)]
pub fn set_ai_credential(state: State<'_, AppState>, secret: String) -> NbResult<()> {
    let host = crate::ai::subsistema::Subsistema::anfitrion_escrito(&state.db)?;
    crate::secrets::guardar(&crate::secrets::cuenta_de_ia(&host), secret.trim())?;
    tracing::info!("se guardó la credencial de un proveedor de IA externo");
    Ok(())
}

/// Borra la credencial del proveedor externo que hay escrito.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no hay dirección válida.
/// - [`NbError::StorageFailure`] si el llavero rechaza el borrado.
#[tauri::command(async)]
pub fn clear_ai_credential(state: State<'_, AppState>) -> NbResult<()> {
    let host = crate::ai::subsistema::Subsistema::anfitrion_escrito(&state.db)?;
    crate::secrets::borrar(&crate::secrets::cuenta_de_ia(&host))
}

/// Lo que se ha enviado a proveedores externos, de lo más reciente a lo más
/// antiguo: el registro que `ai/privacy.md` pide poder auditar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn ai_sends(
    state: State<'_, AppState>,
    limit: usize,
) -> NbResult<Vec<crate::ai::envios::EnvioRegistrado>> {
    let conn = state.db.connection()?;
    crate::ai::envios::listar(&conn, limit.min(MAXIMO_DE_ENVIOS_DE_IA))
}

/// Borra el registro de envíos. Devuelve cuántos había.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn clear_ai_sends(state: State<'_, AppState>) -> NbResult<usize> {
    let conn = state.db.connection()?;
    crate::ai::envios::borrar(&conn)
}

/// Marca una salida generada como incorrecta.
///
/// **Local, y sin el texto del documento**: lo que se guarda es la salida del
/// modelo, con qué plantilla y qué proveedor se hizo, y qué dijo el usuario que
/// estaba mal. El esquema de V36 no tiene dónde poner el texto del libro, que es lo
/// que impide que entre por descuido.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn report_ai_output(
    state: State<'_, AppState>,
    report: crate::ai::reportes::Reporte,
) -> NbResult<i64> {
    let conn = state.db.connection()?;
    crate::ai::reportes::anotar(&conn, &report)
}

/// Lo marcado como incorrecto, de lo más reciente a lo más antiguo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn ai_reports(
    state: State<'_, AppState>,
    limit: usize,
) -> NbResult<Vec<crate::ai::reportes::Registrado>> {
    let conn = state.db.connection()?;
    crate::ai::reportes::listar(&conn, limit.min(MAXIMO_DE_ENVIOS_DE_IA))
}

/// Quita un reporte. Devuelve si existía.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn forget_ai_report(state: State<'_, AppState>, id: i64) -> NbResult<bool> {
    let conn = state.db.connection()?;
    crate::ai::reportes::olvidar(&conn, id)
}

/// Borra los reportes. Devuelve cuántos había.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn clear_ai_reports(state: State<'_, AppState>) -> NbResult<usize> {
    let conn = state.db.connection()?;
    crate::ai::reportes::borrar(&conn)
}

/// Guarda los reportes donde el usuario diga, para poder mirarlos antes de decidir
/// si los manda a alguna parte.
///
/// **No los manda**: no hay a dónde -no existe un servicio del proyecto que los
/// reciba-, y ofrecer un envío que no va a ningún sitio sería peor que no
/// ofrecerlo. Cuando exista, el consentimiento se pedirá como el de cualquier otra
/// salida, por `net.rs`.
///
/// Escribe el runtime y no el webview, y la ruta sale del diálogo del sistema:
/// misma forma que `save_diagnostics`, y por la misma razón.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si el destino no vale y [`NbError::StorageFailure`]
/// si la base no responde o la escritura falla.
#[tauri::command(async)]
pub fn save_ai_reports(state: State<'_, AppState>, path: String) -> NbResult<()> {
    let destino = destino_admisible(&state, &path)?;
    let conn = state.db.connection()?;
    let texto = crate::ai::reportes::exportar(&conn, MAXIMO_DE_ENVIOS_DE_IA)?;
    std::fs::write(&destino, texto).map_err(|error| {
        tracing::error!(%error, "no se pudieron guardar los reportes de IA");
        NbError::StorageFailure
    })
}

/// Autoriza a un catálogo a salir a un destino.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no es un anfitrión `https` válido.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn allow_catalog_host(
    state: State<'_, AppState>,
    host: String,
    cleartext: bool,
) -> NbResult<()> {
    // Se normaliza por el mismo camino que la puerta. El claro es otro permiso,
    // con otro propósito: autorizar un anfitrión por `https` no autoriza a
    // hablarle sin cifrar, y al revés tampoco.
    let anfitrion = crate::net::host_de(&format!("https://{host}/"))?;
    let proposito = if cleartext {
        crate::net::Purpose::CatalogCleartext
    } else {
        crate::net::Purpose::Catalog
    };
    let conn = state.db.connection()?;
    crate::net::grant(&conn, &anfitrion, proposito)?;
    tracing::info!(
        purpose = proposito.as_str(),
        "el usuario autorizó un destino de catálogo"
    );
    Ok(())
}

/// Autoriza a un proveedor de consulta a salir a un destino.
///
/// # Por qué se autoriza un proveedor y no un anfitrión suelto
///
/// Porque un comando que aceptara cualquier anfitrión convertiría al webview en
/// el sitio donde se decide a dónde sale la aplicación. Aquí el runtime pregunta
/// al registro si **algún proveedor instalado reconoce ese anfitrión como
/// suyo**, y solo entonces concede. El destino lo propuso el propio runtime al
/// omitir la consulta, así que la interfaz devuelve lo que recibió.
///
/// El propósito es siempre `lookup`, y no un parámetro: un permiso para
/// consultar no puede convertirse en un permiso para descargar.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si no es un anfitrión `https` válido o si
///   ningún proveedor lo reconoce.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn allow_lookup_host(state: State<'_, AppState>, host: String) -> NbResult<()> {
    // Se normaliza por el mismo camino que la puerta: si aquí se guardara
    // `ES.Wikipedia.org` y allí se buscara `es.wikipedia.org`, el permiso
    // existiría y no serviría.
    let anfitrion = crate::net::host_de(&format!("https://{host}/"))?;

    let reconocido = state
        .lookup
        .read()
        .map_err(|_| {
            tracing::error!("el registro de proveedores de consulta quedó envenenado");
            NbError::Internal
        })?
        .algun_proveedor_usa(&anfitrion);

    if !reconocido {
        tracing::warn!("se pidió autorizar un destino que ningún proveedor usa");
        return Err(NbError::PathNotAllowed);
    }

    let conn = state.db.connection()?;
    crate::net::grant(&conn, &anfitrion, crate::net::Purpose::Lookup)?;
    tracing::info!("el usuario autorizó un destino de consulta");
    Ok(())
}

/// Autoriza el catálogo abierto de metadatos desde la propia ficha.
///
/// Sin esto, "Completar desde un catálogo" se quedaba en "Falta autorizar. Se hace
/// en Ajustes", cuando Descubrir ofrece "Autorizar" en el sitio. El anfitrión es
/// fijo y no un parámetro: la interfaz no puede pedir permiso para otro.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn allow_metadata_host(state: State<'_, AppState>) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::net::grant(
        &conn,
        crate::metadata_online::HOST_CATALOGO,
        crate::net::Purpose::Metadata,
    )?;
    tracing::info!("el usuario autorizó el catálogo de metadatos");
    Ok(())
}

/// Qué se envía al salir con cada propósito, en lenguaje llano.
///
/// Sale del runtime y no de la interfaz porque vive junto al propósito: una
/// explicación en otro archivo se queda vieja en cuanto el propósito cambia, y
/// el criterio dice que el primer uso **dice qué texto se va a
/// enviar**.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si el propósito no es uno de los cuatro.
#[tauri::command(async)]
pub fn what_is_sent(purpose: String) -> NbResult<String> {
    Ok(crate::net::purpose_de(&purpose)?.what_is_sent().to_owned())
}

// ---------------------------------------------------------------------------
// Portadas y metadatos en línea.
// ---------------------------------------------------------------------------

/// Qué pasó al aplicar un candidato del catálogo.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct EnrichReport {
    /// Campos que se escribieron: el usuario no había decidido ninguno.
    pub applied: u32,
    /// Campos que **no** se escribieron porque había una edición manual, y
    /// quedaron como propuesta para que la interfaz pueda ofrecerlos.
    pub suggested: u32,
    /// Campos en los que no había nada que cambiar, o cuyo valor ya se rechazó.
    pub unchanged: u32,
}

/// Busca en el catálogo abierto candidatos para completar una edición.
///
/// **Explícita, nunca automática al importar.** Es el criterio: una
/// biblioteca de ciento cincuenta mil obras que consultara sola al importar
/// haría ciento cincuenta mil peticiones que nadie pidió.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la edición no existe.
/// - [`NbError::NetworkDisabled`], [`NbError::ConsentRequired`] o
///   [`NbError::NetworkFailure`] según lo que diga la puerta.
///
/// Cota: los candidatos que devuelve el proveedor, que los acota él.
#[tauri::command(async)]
pub fn search_online_metadata(
    state: State<'_, AppState>,
    edition_id: String,
) -> NbResult<Vec<crate::metadata_online::Candidate>> {
    let conn = state.db.connection()?;

    // Se busca por lo que ya se sabe del libro, que sale del catálogo local y no
    // de la interfaz: lo que se manda fuera tiene que ser lo que hay, no lo que
    // el webview diga que hay.
    let (titulo, autor): (String, Option<String>) = conn
        .query_row(
            "SELECT e.title,
                    (SELECT c.name FROM edition_contributors ec
                       JOIN contributors c ON c.id = ec.contributor_id
                      WHERE ec.edition_id = e.id AND ec.role = 'author'
                      ORDER BY ec.position LIMIT 1)
               FROM editions e WHERE e.id = ?1",
            [&edition_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map(Some)
        .or_else(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            otro => Err(otro),
        })
        .map_err(|error| {
            tracing::error!(error = %error, "no se pudo leer la edición");
            NbError::StorageFailure
        })?
        .ok_or(NbError::DocumentNotFound)?;

    // Y por sus identificadores, que son lo único con lo que una fuente
    // académica puede responder de forma exacta. Salen del catálogo, igual que
    // el título: lo que se manda fuera es lo que hay, no lo que diga el webview.
    let identificadores: Vec<String> =
        crate::db::catalog::Edition::identifiers(&conn, &edition_id)?
            .into_iter()
            .map(|(esquema, valor)| format!("{esquema}:{valor}"))
            .collect();
    drop(conn);

    let mut candidatos =
        crate::metadata_online::buscar(&state.db, &state.net, &titulo, autor.as_deref())?;

    // Y lo que aporten las extensiones, después. Van detrás y no delante a
    // propósito: el catálogo del núcleo es el que el usuario no eligió instalar,
    // así que es el que menos le sorprende encontrar arriba.
    //
    // Que una fuente falle no puede llevarse a las demás ni al catálogo: se
    // omite y se anota. Es el mismo innegociable de siempre.
    if let Some(subsistema) = &state.extensions {
        match crate::extensions::metadata::fuentes(
            subsistema,
            crate::extensions::Limites::default(),
        ) {
            Ok(fuentes) => {
                for fuente in fuentes {
                    match fuente.buscar(
                        &state.db,
                        &state.net,
                        &titulo,
                        autor.as_deref(),
                        &identificadores,
                    ) {
                        Ok(mas) => candidatos.extend(mas),
                        Err(error) => tracing::info!(
                            extension = %fuente.extension,
                            code = error.code(),
                            "una fuente de metadatos de extensión no respondió"
                        ),
                    }
                }
            }
            Err(_) => tracing::warn!("no se pudieron leer las fuentes de metadatos instaladas"),
        }
    }

    Ok(candidatos)
}

/// Aplica un candidato a una edición, campo por campo.
///
/// **Nada de esto sobrescribe una edición manual**: cada campo pasa por la regla
/// de procedencia, que lo deja como propuesta en lugar de escribirlo.
/// Y lo que el usuario rechazó no se vuelve a ofrecer.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn apply_online_metadata(
    state: State<'_, AppState>,
    edition_id: String,
    candidate: crate::metadata_online::Candidate,
) -> NbResult<EnrichReport> {
    let conn = state.db.connection()?;
    let resultados = crate::metadata_online::aplicar(&conn, &edition_id, &candidate)?;
    drop(conn);

    let mut informe = EnrichReport {
        applied: 0,
        suggested: 0,
        unchanged: 0,
    };
    for (_, resultado) in resultados {
        match resultado {
            crate::metadata::Outcome::Applied => informe.applied += 1,
            crate::metadata::Outcome::Suggested(_) => informe.suggested += 1,
            crate::metadata::Outcome::Unchanged => informe.unchanged += 1,
        }
    }
    if informe.applied > 0 {
        state.bus.emit(DomainEvent::MetadataUpdated { edition_id });
    }
    Ok(informe)
}

/// Cuántas preparaciones de audio se borraron por usar una voz que ya no existe,
/// para decirlo una vez. Devuelve nada si no hay nada que decir.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la base no responde.
#[tauri::command(async)]
pub fn retired_audio_notice(state: State<'_, AppState>) -> NbResult<Option<u32>> {
    state.audio.aviso_de_audio_retirado()
}

/// Los campos de una edición que el usuario puede corregir a mano desde la ficha.
///
/// Lista cerrada y en el runtime: el nombre viaja como texto desde el webview, y
/// lo que no esté aquí no se escribe. El autor no está porque no es un campo de
/// la edición sino un contribuidor compartido con otras obras; corregirlo aquí
/// renombraría a esa persona en toda la biblioteca.
fn campo_editable(nombre: &str) -> Option<crate::metadata::Field> {
    use crate::metadata::Field;
    Some(match nombre {
        "title" => Field::Title,
        "subtitle" => Field::Subtitle,
        "publisher" => Field::Publisher,
        "published_date" => Field::PublishedDate,
        "language" => Field::Language,
        "series_title" => Field::SeriesTitle,
        "series_index" => Field::SeriesIndex,
        "description" => Field::Description,
        _ => return None,
    })
}

/// Corrige a mano un campo de una edición.
///
/// Lo escrito por el usuario gana siempre, sobre lo extraído del archivo, lo de
/// Calibre y lo que llegue en línea (`metadata::set_manual`), y retira las
/// propuestas vivas de ese campo. Un valor vacío deja el campo vacío, salvo el
/// título, que una obra no puede perder.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si el campo no es de los editables o se intenta
///   dejar el título vacío.
/// - [`NbError::StorageFailure`] si la edición no existe o la escritura falla.
#[tauri::command(async)]
pub fn edit_edition_field(
    state: State<'_, AppState>,
    edition_id: String,
    field: String,
    value: Option<String>,
) -> NbResult<()> {
    let campo = campo_editable(&field).ok_or(NbError::PathNotAllowed)?;
    let valor = value.as_deref().map(str::trim).filter(|v| !v.is_empty());
    if valor.is_none() && campo == crate::metadata::Field::Title {
        return Err(NbError::PathNotAllowed);
    }
    let conn = state.db.connection()?;
    crate::metadata::set_manual(&conn, &edition_id, campo, valor)?;
    drop(conn);
    state.bus.emit(DomainEvent::MetadataUpdated { edition_id });
    Ok(())
}

/// Lo que se llevó una baja, para decirlo.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct DocumentRemoval {
    /// Se borró la copia administrada del disco.
    pub file_deleted: bool,
    /// La obra se fue con su última edición.
    pub work_removed: bool,
}

/// Quita un documento de la biblioteca, con todo lo que colgaba de él.
///
/// El archivo **referenciado no se toca nunca**: sale del catálogo, no del disco.
/// Una copia administrada solo se borra si se pide de forma explícita
/// (`delete_managed_copy`), porque es irreversible. Ver
/// `docs/architecture/storage.md`, sección 2.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si la edición no existe.
/// - [`NbError::StorageFailure`] si la transacción falla; entonces no se borra
///   nada.
#[tauri::command(async)]
pub fn delete_document(
    state: State<'_, AppState>,
    edition_id: String,
    delete_managed_copy: bool,
) -> NbResult<DocumentRemoval> {
    let politica = if delete_managed_copy {
        crate::library::FilePolicy::DeleteManagedCopy
    } else {
        crate::library::FilePolicy::Keep
    };
    let portadas = state.paths.cache.join("portadas");
    let mut conn = state.db.connection()?;
    let baja =
        crate::library::delete_document(&mut conn, &state.bus, &edition_id, &portadas, politica)?;
    Ok(DocumentRemoval {
        file_deleted: baja.file_deleted.is_some(),
        work_removed: baja.work_removed,
    })
}

/// Descarga la portada de un candidato y la deja registrada en la edición.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la dirección no es del anfitrión de
///   portadas. Se comprueba en el runtime: la dirección viene del webview.
/// - Los de la puerta de red, o [`NbError::StorageFailure`] al escribir.
#[tauri::command(async)]
pub fn fetch_online_cover(
    state: State<'_, AppState>,
    edition_id: String,
    url: String,
) -> NbResult<String> {
    let portada = crate::metadata_online::descargar_portada(
        &state.db,
        &state.net,
        &crate::metadata_online::carpeta_de_portadas(&state.paths.cache),
        &edition_id,
        &url,
    )?;
    state.bus.emit(DomainEvent::MetadataUpdated { edition_id });
    Ok(portada)
}

/// La ficha de una edición: editorial, fecha, idioma, materias y copias.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla, y
/// [`NbError::DocumentNotFound`] si la edición no existe.
///
/// Cota: `browse::MAXIMO_DE_MATERIAS` materias y `browse::MAXIMO_DE_COPIAS` copias.
#[tauri::command(async)]
pub fn edition_details(
    state: State<'_, AppState>,
    edition_id: String,
) -> NbResult<crate::browse::EditionDetails> {
    let conn = state.db.connection()?;
    crate::browse::details(&conn, &edition_id)
}

/// Propuestas vivas de una edición.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: las sugerencias pendientes de una edición.
#[tauri::command(async)]
pub fn pending_suggestions(
    state: State<'_, AppState>,
    edition_id: String,
) -> NbResult<Vec<crate::metadata::Suggestion>> {
    let conn = state.db.connection()?;
    crate::metadata::pending(&conn, crate::metadata::EntityType::Edition, &edition_id)
}

/// Acepta una propuesta: la decisión pasa a ser del usuario.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no existe o ya estaba resuelta.
#[tauri::command(async)]
pub fn accept_suggestion(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    let edition_id = crate::metadata::accept(&conn, &id)?;
    drop(conn);
    state.bus.emit(DomainEvent::MetadataUpdated { edition_id });
    Ok(())
}

/// Descarta una propuesta. **No se vuelve a ofrecer.**
///
/// # Errors
///
/// [`NbError::DocumentNotFound`] si no existe o ya estaba resuelta.
#[tauri::command(async)]
pub fn reject_suggestion(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::metadata::reject(&conn, &id)
}

// ---------------------------------------------------------------------------
// Catálogos OPDS.
// ---------------------------------------------------------------------------

/// Catálogos que el usuario ha añadido.
///
/// **Ninguno viene puesto.** Un catálogo preconfigurado es una recomendación
/// comercial disfrazada de función.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: los catálogos dados de alta, que se añaden de uno en uno.
#[tauri::command(async)]
pub fn list_catalogs(state: State<'_, AppState>) -> NbResult<Vec<crate::opds::Catalog>> {
    let conn = state.db.connection()?;
    crate::opds::list(&conn)
}

/// Añade un catálogo por su dirección.
///
/// # Errors
///
/// - [`NbError::PathNotAllowed`] si la dirección no es `https` con anfitrión, o
///   si el nombre está vacío.
/// - [`NbError::StorageFailure`] si ya estaba.
#[tauri::command(async)]
pub fn add_catalog(
    state: State<'_, AppState>,
    title: String,
    url: String,
) -> NbResult<crate::opds::Catalog> {
    let conn = state.db.connection()?;
    crate::opds::add(&conn, &title, &url)
}

/// Quita un catálogo. Que no estuviera no es un error.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn remove_catalog(state: State<'_, AppState>, id: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::opds::remove(&conn, &id)
}

/// Navega un catálogo: pide un feed y lo devuelve analizado.
///
/// La dirección la propone el propio catálogo —la raíz que el usuario añadió, o
/// un enlace de un feed que él ya ha visto—, y la puerta de red la comprueba
/// igual: si lleva a otro anfitrión, hará falta autorizarlo aparte.
///
/// # Errors
///
/// - Los de la puerta de red.
/// - [`NbError::DocumentCorrupted`] si lo que responde no es un feed. Es
///   deliberadamente distinto de un fallo de red: uno inalcanzable y uno que
///   devuelve basura piden cosas distintas al usuario.
#[tauri::command(async)]
pub fn browse_catalog(
    state: State<'_, AppState>,
    url: String,
) -> NbResult<crate::opds::CatalogPage> {
    crate::opds::navegar(&state.db, &state.net, &url)
}

/// Cómo terminó descargar de un catálogo.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "estado", content = "detalle")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum CatalogDownload {
    /// Se descargó e intentó importar: el informe dice cómo fue.
    Imported(ImportReport),
    /// Falta el permiso de este anfitrión, que suele ser el del servidor donde
    /// viven las descargas y no el del feed.
    #[serde(rename_all = "camelCase")]
    NeedsConsent { host: String, cleartext: bool },
}

/// Descarga un documento de un catálogo y lo importa.
///
/// # El orden importa, y es lo que hace cierto el criterio
///
/// 1. Se descarga a un archivo **temporal**, fuera de la biblioteca.
/// 2. Se importa por la canalización de siempre, que reconoce el formato **por
///    contenido**: un catálogo que anuncia un EPUB y sirve otra cosa se rechaza
///    aquí.
/// 3. El temporal se borra **pase lo que pase**.
///
/// Con ese orden, un fallo en cualquier paso no deja registro huérfano ni copia
/// a medias: lo primero lo garantiza la transacción de `import_document`, y lo
/// segundo, el borrado del temporal.
///
/// # Errors
///
/// - Los de la puerta de red.
/// - [`NbError::FormatUnsupported`] o [`NbError::DocumentCorrupted`] si lo
///   descargado no es un documento que sepamos abrir.
/// - [`NbError::StorageFailure`] si la escritura falla.
#[tauri::command(async)]
pub fn download_from_catalog(state: State<'_, AppState>, url: String) -> NbResult<CatalogDownload> {
    let descargas = state.paths.cache.join("descargas");
    let temporal = match crate::opds::descargar(&state.db, &state.net, &descargas, &url)? {
        crate::opds::Descarga::Guardada(temporal) => temporal,
        crate::opds::Descarga::FaltaPermiso { host, cleartext } => {
            return Ok(CatalogDownload::NeedsConsent { host, cleartext });
        }
    };

    // El borrado va en un guardián y no al final: entre medias hay tres caminos
    // de error, y en los tres el temporal tiene que irse.
    let _limpieza = BorraAlSalir(&temporal);

    let library = state.paths.library();
    let mut conn = state.db.connection()?;
    let ctx = library::ImportContext {
        // El ámbito es la carpeta de descargas y nada más: lo que se importa
        // aquí no puede venir de ningún otro sitio del disco.
        scope: &descargas,
        library: &library,
        bus: &state.bus,
    };

    let resultado = library::import_document(
        &mut conn,
        &ctx,
        &library::ImportRequest {
            source: temporal.clone(),
            // Siempre administrado: el temporal se borra, así que un documento
            // referenciado apuntaría a un archivo que no existe.
            storage_mode: crate::db::catalog::StorageMode::Managed,
            root: None,
        },
    );

    match resultado {
        Ok(library::ImportOutcome::Imported(_)) => Ok(CatalogDownload::Imported(ImportReport {
            processed: 1,
            imported: 1,
            already_in_library: 0,
            duplicates: 0,
            rejected: Vec::new(),
            cancelled: false,
            running: false,
        })),
        // Lo descargado es un temporal, así que el mismo libro ya presente sale
        // como copia exacta. Para quien descarga eso es "ya lo tienes", no "lo
        // tienes dos veces": la copia se borra con el temporal.
        Ok(library::ImportOutcome::AlreadyInLibrary(_) | library::ImportOutcome::Duplicate(_)) => {
            Ok(CatalogDownload::Imported(ImportReport {
                processed: 1,
                imported: 0,
                already_in_library: 1,
                duplicates: 0,
                rejected: Vec::new(),
                cancelled: false,
                running: false,
            }))
        }
        // Lo que se descargó y no era lo que decía se informa como rechazo, con
        // su código: la interfaz enseña lo mismo que en cualquier importación.
        Err(error) => Ok(CatalogDownload::Imported(ImportReport {
            processed: 1,
            imported: 0,
            already_in_library: 0,
            duplicates: 0,
            rejected: vec![library::Rejected {
                path: String::new(),
                code: error.code(),
            }],
            cancelled: false,
            running: false,
        })),
    }
}

/// Borra un archivo al salir del ámbito, pase lo que pase.
struct BorraAlSalir<'a>(&'a std::path::Path);

impl Drop for BorraAlSalir<'_> {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(self.0) {
            // Que no se pueda borrar un temporal no puede tumbar una
            // importación que ya salió bien: se deja constancia y se sigue.
            tracing::warn!(error = %error, "quedó un archivo temporal de descarga");
        }
    }
}

// ---------------------------------------------------------------------------
// Extensiones
// ---------------------------------------------------------------------------

/// Una extensión, tal y como se ve en Ajustes.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionView {
    pub id: String,
    /// El nombre de su manifiesto, o el identificador si no se puede leer: una
    /// fila sin nombre es una fila que nadie puede señalar.
    pub name: String,
    pub version: String,
    pub enabled: bool,
    /// Si su manifiesto se pudo leer. Cuando no, la fila **lo dice** en lugar
    /// de desaparecer: que una extensión se esfume de la lista sin explicación
    /// es justo lo que el producto llama pérdida silenciosa.
    pub readable: bool,
    /// Los módulos que declara y no están en su carpeta.
    ///
    /// Vacío en una instalación sana. Lleno en las que entraron antes de que la
    /// instalación lo comprobara: la fila lo dice, en lugar de que cada
    /// uso de la extensión falle con "no se encontró el archivo".
    pub missing_modules: Vec<String>,
    /// Cuántos comandos aporta a la interfaz.
    pub commands: u32,
    /// Lo que pide, con el estado de cada permiso.
    pub permissions: Vec<crate::extensions::permissions::PermissionAsk>,
    /// Qué clases de proveedor declara: `citation`, `bulk-metadata`,
    /// `format:...`.
    ///
    /// Sale del manifiesto, así que la interfaz sabe qué ofrece **sin compilar
    /// una sola instrucción suya**. Es de lo que vive la activación perezosa.
    pub providers: Vec<String>,
    /// La clave con la que venía firmada, si venía firmada.
    ///
    /// Se enseña porque es lo que distingue una actualización del mismo autor
    /// de una de cualquiera: sin verla, quien instala no tiene forma de saber
    /// si lo que tiene está atado a alguien o no lo está a nadie.
    pub key: Option<String>,
}

/// Cómo terminó pedir una instalación.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "estado", content = "detalle")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum ExtensionInstall {
    /// Entró.
    Installed(ExtensionView),
    /// Pide permisos que no tiene. **No se instaló nada.**
    NeedsConsent(ExtensionPending),
    /**
     * Le faltan los módulos que su manifiesto declara. **No se instaló nada.**
     *
     * Lleva sus nombres porque decir "falta un módulo" sin decir cuál obliga a
     * adivinar. Es el caso de instalar una extensión desde su carpeta de
     * fuentes, donde está el manifiesto y no el `.wasm` compilado.
     */
    Incomplete { modules: Vec<String> },
}

/// Lo que hay que aprobar antes de que una extensión entre.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionPending {
    pub id: String,
    pub name: String,
    pub version: String,
    pub permissions: Vec<crate::extensions::permissions::PermissionAsk>,
    /// En qué estado llega la firma de lo que se va a instalar.
    ///
    /// Va aquí y no solo en la lista de instaladas porque el momento de
    /// decidir es este: después de instalar, saber que no venía firmada ya no
    /// sirve para no instalarla.
    pub signature: crate::extensions::signature::SignatureState,
}

fn subsistema(
    state: &State<'_, AppState>,
) -> NbResult<std::sync::Arc<crate::extensions::Subsistema>> {
    state
        .extensions
        .clone()
        .ok_or(NbError::ComponentUnavailable)
}

/// Compone la vista de todo lo instalado.
fn vista_de_extensiones(state: &State<'_, AppState>) -> NbResult<Vec<ExtensionView>> {
    let sub = subsistema(state)?;
    let conn = state.db.connection()?;
    let registro = sub.registro()?;
    let comandos = registro.comandos();

    registro
        .instaladas()
        .iter()
        .map(|instalada| {
            let manifiesto = registro.manifiesto(&instalada.id);
            let permisos = match manifiesto {
                Some(m) => crate::extensions::permissions::lo_que_pide(&conn, m)?,
                None => Vec::new(),
            };
            Ok(ExtensionView {
                id: instalada.id.clone(),
                name: manifiesto.map_or_else(|| instalada.id.clone(), |m| m.name.clone()),
                version: instalada.version.clone(),
                enabled: instalada.enabled,
                readable: manifiesto.is_some(),
                missing_modules: manifiesto
                    .map(|m| sub.almacen.modulos_que_faltan(instalada, m))
                    .unwrap_or_default(),
                commands: comandos
                    .iter()
                    .filter(|c| c.extension == instalada.id)
                    .count()
                    .try_into()
                    .unwrap_or(u32::MAX),
                permissions: permisos,
                providers: manifiesto
                    .map(|m| m.contributes.providers.keys().cloned().collect())
                    .unwrap_or_default(),
                key: instalada.key.clone(),
            })
        })
        .collect()
}

/// Lo que hay instalado.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el subsistema no arrancó.
///
/// Cota: las extensiones instaladas.
#[tauri::command(async)]
pub fn extensions_list(state: State<'_, AppState>) -> NbResult<Vec<ExtensionView>> {
    vista_de_extensiones(&state)
}

/// Lo que el anfitrión dice de su propia API de extensiones.
///
/// Existe para que la interfaz **no repita el número**. La sección 2 de
/// `versioning.md` es el único sitio donde viven las cifras del compromiso, y
/// el único sitio donde vive la que se cumple es `extensions::MAYOR_ACTUAL`:
/// una constante escrita a mano en el frontend sería una segunda fuente para
/// un dato que tiene que ser uno.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionApi {
    /// La mayor que este anfitrión habla hoy.
    pub current: u32,
    /// La más antigua que sigue aceptando. Es `current - 1` mientras haya una.
    pub oldest: u32,
    /// Si el compromiso de estabilidad está en vigor. Desde la 1.0.0 lo está, y la
    /// sección 2.2 de `versioning.md` obliga a que la aplicación lo diga.
    pub stable: bool,
}

/// Qué versión de la API de extensiones habla esta aplicación.
///
/// No falla ni depende del subsistema: es una propiedad del anfitrión, y quien
/// abre Ajustes tiene que poder leerla aunque las extensiones estén rotas.
#[tauri::command(async)]
#[must_use]
pub fn extensions_api() -> ExtensionApi {
    let (oldest, current) = crate::extensions::admitidas();
    ExtensionApi {
        current: current as u32,
        oldest: oldest as u32,
        // La estabilidad no se declara escribiéndola: se deduce del número, que
        // es lo que impide que alguien prometa 1.0 sin subir la API.
        stable: current >= 1,
    }
}

/// Instala una extensión desde una carpeta del disco.
///
/// La ruta llega del diálogo nativo, que es la única vía por la que una ruta
/// entra al sistema (`ipc-contracts.md`, regla 1). Se canonicaliza antes de
/// tocarla.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si el subsistema no arrancó.
/// - [`NbError::PathNotAllowed`] si la ruta no se puede resolver.
/// - [`NbError::ExtensionManifestInvalid`] si el manifiesto no vale.
/// - [`NbError::ExtensionApiUnsupported`] si se hizo para otra versión.
#[tauri::command(async)]
pub fn extensions_install(state: State<'_, AppState>, path: String) -> NbResult<ExtensionInstall> {
    let sub = subsistema(&state)?;
    let conn = state.db.connection()?;
    let carpeta = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|_| NbError::PathNotAllowed)?;

    match crate::extensions::permissions::instalar(&conn, &sub.almacen, &carpeta)? {
        crate::extensions::permissions::Instalacion::Instalada(instalada) => {
            // Lo que aporta -un tema, un modo, una orden- tiene que aparecer sin
            // reiniciar: principio 1 de la filosofía de producto.
            state.bus.emit(DomainEvent::ExtensionsChanged);
            let vistas = vista_de_extensiones(&state)?;
            vistas
                .into_iter()
                .find(|v| v.id == instalada.id)
                .map(ExtensionInstall::Installed)
                .ok_or(NbError::Internal)
        }
        crate::extensions::permissions::Instalacion::Incompleta(modulos) => {
            Ok(ExtensionInstall::Incomplete { modules: modulos })
        }
        crate::extensions::permissions::Instalacion::FaltaConsentimiento(permisos) => {
            let texto = std::fs::read_to_string(carpeta.join(crate::extensions::manifest::ARCHIVO))
                .map_err(|_| NbError::DocumentNotFound)?;
            let manifiesto = crate::extensions::manifest::parse(&texto)
                .map_err(|_| NbError::ExtensionManifestInvalid)?;
            let firma = crate::extensions::signature::verificar(
                &carpeta,
                &manifiesto.id,
                &manifiesto.version,
            )?;
            Ok(ExtensionInstall::NeedsConsent(ExtensionPending {
                id: manifiesto.id,
                name: manifiesto.name,
                version: manifiesto.version,
                permissions: permisos,
                signature: firma,
            }))
        }
    }
}

/// Concede a una extensión salida a un anfitrión que su manifiesto declara.
///
/// Acepta la carpeta del paquete además del identificador porque el permiso se
/// concede **antes** de instalar: en ese momento la extensión todavía no está
/// en el almacén, y su manifiesto solo existe en el disco de quien la va a
/// instalar.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si ese anfitrión no está en el manifiesto.
#[tauri::command(async)]
pub fn extensions_grant(state: State<'_, AppState>, path: String, host: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    let carpeta = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|_| NbError::PathNotAllowed)?;
    let texto = std::fs::read_to_string(carpeta.join(crate::extensions::manifest::ARCHIVO))
        .map_err(|_| NbError::DocumentNotFound)?;
    let manifiesto = crate::extensions::manifest::parse(&texto)
        .map_err(|_| NbError::ExtensionManifestInvalid)?;
    crate::extensions::permissions::conceder(&conn, &manifiesto, &host)
}

/// Revoca un anfitrión concreto a una extensión instalada.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn extensions_revoke(state: State<'_, AppState>, id: String, host: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::extensions::permissions::revocar(&conn, &id, &host)?;
    drop(conn);
    // La fila de la extensión dice a quién puede hablar, y eso acaba de cambiar.
    state.bus.emit(DomainEvent::ExtensionsChanged);
    Ok(())
}

/// Enciende o apaga una extensión instalada.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el subsistema no arrancó.
///
/// Cota: las extensiones instaladas: devuelve la lista después del cambio.
#[tauri::command(async)]
pub fn extensions_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> NbResult<Vec<ExtensionView>> {
    let sub = subsistema(&state)?;
    sub.almacen.habilitar(&id, enabled)?;
    state.bus.emit(DomainEvent::ExtensionsChanged);
    vista_de_extensiones(&state)
}

/// Desinstala una extensión **y sus permisos**.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el subsistema no arrancó.
///
/// Cota: las extensiones instaladas: devuelve la lista después del cambio.
#[tauri::command(async)]
pub fn extensions_uninstall(
    state: State<'_, AppState>,
    id: String,
) -> NbResult<Vec<ExtensionView>> {
    let sub = subsistema(&state)?;
    let conn = state.db.connection()?;
    crate::extensions::permissions::desinstalar(&conn, &sub.almacen, &id)?;
    drop(conn);
    state.bus.emit(DomainEvent::ExtensionsChanged);
    vista_de_extensiones(&state)
}

/// Los avisos de retirada que ya se conocen, sin salir a la red.
///
/// Se leen de lo guardado y no del registro: el aviso tiene que verse aunque
/// no haya red, y aunque el registro no exista todavía.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer.
///
/// Cota: las retiradas del registro que afectan a lo instalado.
#[tauri::command(async)]
pub fn extensions_withdrawals(
    state: State<'_, AppState>,
) -> NbResult<Vec<crate::extensions::withdrawals::Withdrawal>> {
    let conn = state.db.connection()?;
    crate::extensions::withdrawals::guardados(&conn)
}

/// Le pregunta al registro qué ha retirado, y aplica lo que diga.
///
/// **Apaga lo que resulte malicioso**, y no lo desinstala: apagar no es
/// desinstalar, y decidir por el usuario que se borra sería perder algo suyo
/// sin preguntar.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si el subsistema no arrancó o **no hay
///   registro configurado**. Lo segundo es el estado de hoy y no un fallo.
/// - [`NbError::ConsentRequired`] si nadie ha autorizado la salida a ese
///   anfitrión con este propósito.
/// - [`NbError::DocumentCorrupted`] si lo que responde no es la lista.
///
/// Cota: las retiradas del registro que afectan a lo instalado.
#[tauri::command(async)]
pub fn extensions_check_withdrawals(
    state: State<'_, AppState>,
) -> NbResult<Vec<crate::extensions::withdrawals::Withdrawal>> {
    let sub = subsistema(&state)?;
    let conn = state.db.connection()?;
    let anfitrion = match crate::settings::get(&conn, crate::settings::SettingKey::RegistryHost)? {
        crate::settings::SettingValue::Text(texto) => texto,
        _ => String::new(),
    };
    let retiradas =
        crate::extensions::withdrawals::comprobar(&conn, &state.net, &sub.almacen, &anfitrion)?;
    // Lo que el registro dice se guarda y se pinta en la fila de cada extensión, y
    // una retirada maliciosa además la apaga: lo que aportaba deja de verse ya.
    if !retiradas.is_empty() {
        state.bus.emit(DomainEvent::ExtensionsChanged);
    }
    Ok(retiradas)
}

/// Las generaciones a las que se puede volver, de la más antigua a la más nueva.
///
/// # Errors
///
/// [`NbError::ComponentUnavailable`] si el subsistema no arrancó.
///
/// Cota: las generaciones que guarda el almacén de una extensión.
#[tauri::command(async)]
pub fn extensions_generations(state: State<'_, AppState>) -> NbResult<Vec<u32>> {
    subsistema(&state)?.almacen.generaciones()
}

/// Vuelve a la generación anterior.
///
/// # Errors
///
/// [`NbError::ExtensionRollbackUnavailable`] si no hay a dónde volver.
///
/// Cota: las extensiones instaladas: devuelve la lista después de volver atrás.
#[tauri::command(async)]
pub fn extensions_rollback(state: State<'_, AppState>) -> NbResult<Vec<ExtensionView>> {
    let sub = subsistema(&state)?;
    sub.almacen.volver_atras()?;
    state.bus.emit(DomainEvent::ExtensionsChanged);
    vista_de_extensiones(&state)
}

/// Una orden que una extensión pone en un sitio de la interfaz.
///
/// Sale del manifiesto, así que existe **antes** de que se cargue una sola
/// instrucción de la extensión. Es de lo que vive la activación perezosa.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionCommand {
    /// Quién la aporta. Viaja para poder decirlo y para poder invocarla.
    pub extension: String,
    pub id: String,
    /// El título que puso su autor. **No se traduce**: es suyo, igual que el
    /// nombre de la extensión.
    pub title: String,
    pub category: Option<String>,
}

/// Las órdenes que las extensiones ponen en un sitio de la interfaz.
///
/// Devuelve lista vacía si el subsistema no arrancó, en lugar de fallar: que no
/// haya extensiones no es un error de la pantalla que las mostraría.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil.
///
/// Cota: los comandos que aportan las extensiones instaladas para un sitio.
#[tauri::command(async)]
pub fn extensions_context_commands(
    state: State<'_, AppState>,
    site: String,
) -> NbResult<Vec<ExtensionCommand>> {
    let Some(sub) = &state.extensions else {
        return Ok(Vec::new());
    };
    Ok(sub
        .registro()?
        .comandos_en(&site)
        .into_iter()
        .map(|aportado| ExtensionCommand {
            extension: aportado.extension,
            id: aportado.comando.id,
            title: aportado.comando.title,
            category: aportado.comando.category,
        })
        .collect())
}

/// Ejecuta una orden de una extensión.
///
/// **Aquí se compila su código, y no antes.** Devuelve el texto corto que la
/// extensión quiera enseñar, o nada.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la extensión no está, está apagada o
///   no declara esa orden.
/// - [`NbError::ConsentRequired`], [`NbError::NetworkDisabled`] o
///   [`NbError::NetworkFailure`] según lo que le dijera la puerta de red.
/// - [`NbError::ExtensionLimitExceeded`] si se pasó de sus límites.
#[tauri::command(async)]
pub fn extensions_run_command(
    state: State<'_, AppState>,
    extension: String,
    command: String,
) -> NbResult<Option<String>> {
    let sub = subsistema(&state)?;
    let registro = sub.registro()?;
    let resultado = registro.ejecutar_orden(
        &crate::extensions::registry::Entorno {
            motor: &sub.motor,
            almacen: &sub.almacen,
            db: &state.db,
            red: &state.net,
        },
        &extension,
        &command,
        crate::extensions::Limites::default(),
    )?;

    use crate::extensions::world::Activacion;
    match resultado {
        Activacion::Hecha(texto) => Ok(texto),
        // Los motivos de la extensión se traducen a los códigos canónicos que
        // la interfaz ya sabe contar. Sin esto, "te falta un permiso" y "se
        // rompió" se verían igual, y piden acciones opuestas.
        Activacion::NoPudo(motivo) => {
            use crate::extensions::world::nb::extension::host::FetchError;
            use crate::extensions::world::CommandError;
            Err(match motivo {
                CommandError::Network(FetchError::Denied) => NbError::ConsentRequired,
                CommandError::Network(FetchError::Offline) => NbError::NetworkDisabled,
                CommandError::Network(FetchError::Failed) => NbError::NetworkFailure,
                CommandError::Unknown | CommandError::Failed => NbError::ComponentUnavailable,
            })
        }
        Activacion::Detenido(desenlace) => {
            Err(desenlace.error().unwrap_or(NbError::ComponentUnavailable))
        }
    }
}

/// Una vista que una extensión aporta a Estudio.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionStudyView {
    pub extension: String,
    pub id: String,
    /// El título de la pestaña. Lo pone su autor y **no se traduce**.
    pub title: String,
}

/// Un trozo de una vista aportada por una extensión.
///
/// Es una lista cerrada: solo entra lo que la interfaz sabe pintar con sus
/// propios componentes. Una extensión aporta contenido, no marcado.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase", tag = "clase", content = "valor")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum ExtensionBlock {
    Heading(String),
    Paragraph(String),
    Items(Vec<String>),
    Field { label: String, value: String },
}

/// Lo que una extensión quiere pintar en su vista.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionRenderedView {
    pub title: String,
    pub blocks: Vec<ExtensionBlock>,
}

/// Las vistas que las extensiones aportan a Estudio.
///
/// Devuelve lista vacía si el subsistema no arrancó: que no haya extensiones no
/// es un error de la pantalla que las mostraría.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil.
///
/// Cota: las vistas que aportan las extensiones instaladas.
#[tauri::command(async)]
pub fn extensions_study_views(state: State<'_, AppState>) -> NbResult<Vec<ExtensionStudyView>> {
    let Some(sub) = &state.extensions else {
        return Ok(Vec::new());
    };
    Ok(sub
        .registro()?
        .vistas()
        .into_iter()
        .map(|(extension, vista)| ExtensionStudyView {
            extension,
            id: vista.id,
            title: vista.title,
        })
        .collect())
}

/// Compone una vista de una extensión. **Aquí se compila su código.**
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si la extensión no está, está apagada o
///   no declara esa vista.
/// - [`NbError::ExtensionLimitExceeded`] si se pasó de sus límites, que es lo
///   que le ocurre a una que entra en bucle.
#[tauri::command(async)]
pub fn extensions_render_view(
    state: State<'_, AppState>,
    extension: String,
    view: String,
) -> NbResult<ExtensionRenderedView> {
    let sub = subsistema(&state)?;
    let registro = sub.registro()?;
    let compuesta = registro.componer_vista(
        &crate::extensions::registry::Entorno {
            motor: &sub.motor,
            almacen: &sub.almacen,
            db: &state.db,
            red: &state.net,
        },
        &extension,
        &view,
        crate::extensions::Limites::default(),
    )?;

    use crate::extensions::view::Block;
    Ok(ExtensionRenderedView {
        title: compuesta.title,
        blocks: compuesta
            .blocks
            .into_iter()
            .map(|bloque| match bloque {
                Block::Heading(texto) => ExtensionBlock::Heading(texto),
                Block::Paragraph(texto) => ExtensionBlock::Paragraph(texto),
                Block::Items(lineas) => ExtensionBlock::Items(lineas),
                Block::Field((label, value)) => ExtensionBlock::Field { label, value },
            })
            .collect(),
    })
}

/// Un formato de cita aportado por una extensión.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct CitationFormat {
    /// `<extensión>/<id>`, que es lo que la interfaz devuelve al pedir el
    /// archivo.
    pub id: String,
    pub extension: String,
    /// El nombre que puso su autor. **No se traduce.**
    pub name: String,
    /// La extensión de archivo, sin el punto.
    pub file_extension: String,
}

/// Un fondo sonoro aportado por una extensión.
///
/// Lleva el nombre y la clase de ruido, y nada más: el sonido lo sintetiza el
/// núcleo, así que la extensión no trae ni un byte de audio.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionSoundscape {
    /// `<extensión>/<id>`.
    pub id: String,
    pub extension: String,
    /// El nombre que puso su autor. **No se traduce.**
    pub name: String,
    /// `blanco`, `rosa` o `marron`.
    pub kind: String,
}

/// Un modo de lectura aportado por una extensión.
///
/// Un perfil completo, igual que los seis del núcleo: fondo, texto, brillo y
/// temperatura. Va en un paquete de clase `theme`, así que **no trae código**.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionReadingMode {
    /// `<extensión>/<id>`, que es lo que se guarda en las preferencias.
    pub id: String,
    pub extension: String,
    /// El nombre que puso su autor. **No se traduce.**
    pub name: String,
    pub background: String,
    pub text: String,
    /// Brillo emulado, en porcentaje.
    pub brightness: u8,
    /// Temperatura emulada, en porcentaje.
    pub warmth: u8,
    pub monochrome: bool,
}

/// Un tema aportado por una extensión.
///
/// Solo colores, y solo de la lista cerrada de `manifest::TOKENS_DE_TEMA`. No
/// trae código: un manifiesto de clase `theme` que declare proveedores,
/// órdenes, vistas o permisos se rechaza al validarlo.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ExtensionTheme {
    pub extension: String,
    /// El nombre que puso su autor. **No se traduce.**
    pub name: String,
    /// De cada token de color, su valor en hexadecimal.
    pub tokens: std::collections::BTreeMap<String, String>,
}

/// Pide a una extensión que revise los metadatos de un lote de ediciones.
///
/// Lo que propone queda como sugerencia, nunca escrito: ver
/// [`crate::extensions::bulk`]. El lote lo compone el anfitrión; la extensión no
/// recorre la biblioteca.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si el subsistema de extensiones no arrancó.
/// - [`NbError::StorageFailure`] si no se puede leer la base.
#[tauri::command(async)]
pub fn review_metadata_in_bulk(
    state: State<'_, AppState>,
    extension: String,
    edition_ids: Vec<String>,
) -> NbResult<crate::extensions::bulk::BulkReviewSummary> {
    let sub = subsistema(&state)?;
    let conn = state.db.connection()?;

    // El anfitrión compone la lista: la extensión no recorre la biblioteca ni
    // elige qué documentos entran.
    let mut documentos = Vec::with_capacity(edition_ids.len());
    for id in &edition_ids {
        let Some(documento) = crate::extensions::bulk::documento_de(&conn, id)? else {
            continue;
        };
        documentos.push(documento);
    }

    crate::extensions::bulk::revisar(
        &conn,
        &sub.motor,
        &sub.almacen,
        &extension,
        &documentos,
        crate::extensions::Limites::default(),
    )
}

/// Cuánto cabe en un tablero.
///
/// Lo decide el runtime y no la interfaz: es lo que ata el tablero al
/// presupuesto de nodos, y una interfaz que lo eligiera podría pintar un lienzo
/// infinito con otro nombre. La interfaz lo usa para dejar de ofrecer una columna
/// o una tarjeta más cuando ya no caben, en lugar de ofrecerla y fallar.
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct BoardLimits {
    pub columns: u32,
    pub cards: u32,
}

/// Cuánto cabe en un tablero.
#[tauri::command(async)]
#[must_use]
pub const fn board_limits() -> BoardLimits {
    BoardLimits {
        columns: crate::boards::MAXIMO_DE_COLUMNAS as u32,
        cards: crate::boards::MAXIMO_DE_TARJETAS as u32,
    }
}

/// Los tableros que hay.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
#[tauri::command(async)]
pub fn boards_list(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> NbResult<Vec<crate::boards::Board>> {
    let conn = state.db.connection()?;
    crate::boards::listar(&conn, acotar(limit), offset)
}

/// Crea un tablero con sus primeras columnas, que nombra la interfaz en su idioma.
///
/// # Errors
///
/// - [`NbError::Internal`] si el nombre viene vacío o sobran columnas.
/// - [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn boards_create(
    state: State<'_, AppState>,
    title: String,
    columns: Vec<String>,
) -> NbResult<crate::boards::Board> {
    let mut conn = state.db.connection()?;
    crate::boards::crear(&mut conn, &title, &columns)
}

/// Borra un tablero. **Las notas siguen donde estaban.**
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn boards_delete(state: State<'_, AppState>, id: String) -> NbResult<bool> {
    let conn = state.db.connection()?;
    crate::boards::borrar(&conn, &id)
}

/// Las colecciones, con cuántos miembros tiene cada una.
///
/// El esquema y la lógica existen; hasta aquí no había comando ni
/// botón detrás, así que agrupar libros era una capacidad que nadie podía usar.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: las colecciones, que el usuario crea de una en una.
#[tauri::command(async)]
pub fn collections_list(
    state: State<'_, AppState>,
) -> NbResult<Vec<crate::collections::CollectionSummary>> {
    let conn = state.db.connection()?;
    crate::collections::Collection::summaries(&conn)
}

/// Crea una colección manual del usuario.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si el nombre queda vacío o ya lo lleva otra
/// colección suya. La interfaz lo comprueba antes con la lista que ya tiene, así
/// que llegar aquí con uno repetido es una carrera, no el caso normal.
#[tauri::command(async)]
pub fn collections_create(
    state: State<'_, AppState>,
    name: String,
) -> NbResult<crate::collections::CollectionSummary> {
    let conn = state.db.connection()?;
    let creada = crate::collections::Collection::create_manual(
        &conn,
        name.trim(),
        crate::collections::CollectionOrigin::Personal,
    )?;
    drop(conn);
    state.bus.emit(DomainEvent::CollectionsChanged {
        collection_id: None,
    });
    Ok(crate::collections::CollectionSummary {
        id: creada.id,
        name: creada.name,
        kind: creada.kind,
        origin: creada.origin,
        members: 0,
    })
}

/// Cambia el nombre de una colección del usuario.
///
/// # Errors
///
/// - [`NbError::DocumentNotFound`] si no existe.
/// - [`NbError::StorageFailure`] si el nombre queda vacío o repetido.
#[tauri::command(async)]
pub fn collections_rename(state: State<'_, AppState>, id: String, name: String) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::collections::Collection::rename(&conn, &id, &name)?;
    drop(conn);
    state.bus.emit(DomainEvent::CollectionsChanged {
        collection_id: Some(id),
    });
    Ok(())
}

/// Borra una colección del usuario. **Los libros siguen en la biblioteca.**
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn collections_delete(state: State<'_, AppState>, id: String) -> NbResult<bool> {
    let conn = state.db.connection()?;
    let habia = crate::collections::Collection::delete(&conn, &id)?;
    drop(conn);
    if habia {
        state.bus.emit(DomainEvent::CollectionsChanged {
            collection_id: None,
        });
    }
    Ok(habia)
}

/// Añade ediciones a una colección manual, todas o ninguna, y dice cuántas
/// entraron nuevas.
///
/// # Errors
///
/// - [`NbError::Internal`] si llegan más de [`MAXIMO_FILAS`]: la lista la manda
///   el webview, que es zona no confiable, y una selección así no la produce la
///   interfaz.
/// - Los de `Collection::add_many`.
#[tauri::command(async)]
pub fn collections_add(
    state: State<'_, AppState>,
    id: String,
    edition_ids: Vec<String>,
) -> NbResult<u32> {
    if edition_ids.len() > MAXIMO_FILAS as usize {
        tracing::error!(
            cuantas = edition_ids.len(),
            "se pidió añadir demasiadas ediciones de una vez"
        );
        return Err(NbError::Internal);
    }
    let mut conn = state.db.connection()?;
    let nuevas = crate::collections::Collection::add_many(&mut conn, &id, &edition_ids)?;
    drop(conn);
    if nuevas > 0 {
        state.bus.emit(DomainEvent::CollectionsChanged {
            collection_id: Some(id),
        });
    }
    Ok(nuevas)
}

/// Quita una edición de una colección manual.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn collections_remove(
    state: State<'_, AppState>,
    id: String,
    edition_id: String,
) -> NbResult<()> {
    let conn = state.db.connection()?;
    crate::collections::Collection::remove(&conn, &id, &edition_id)?;
    drop(conn);
    state.bus.emit(DomainEvent::CollectionsChanged {
        collection_id: Some(id),
    });
    Ok(())
}

/// Las colecciones manuales en las que está una edición, para su ficha.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: las colecciones del usuario, que las crea de una en una.
#[tauri::command(async)]
pub fn collections_containing(
    state: State<'_, AppState>,
    edition_id: String,
) -> NbResult<Vec<String>> {
    let conn = state.db.connection()?;
    crate::collections::Collection::containing(&conn, &edition_id)
}

/// Las columnas de un tablero con sus tarjetas, en orden.
///
/// Es la única lectura del tablero: las columnas y la lista equivalente se pintan
/// de esta misma respuesta, así que la lista no es un resumen.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla.
///
/// Cota: `boards::MAXIMO_DE_COLUMNAS` columnas y `boards::MAXIMO_DE_TARJETAS`
/// tarjetas, que el runtime no deja pasar al escribir.
#[tauri::command(async)]
pub fn board_columns(
    state: State<'_, AppState>,
    board_id: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let conn = state.db.connection()?;
    crate::boards::contenido(&conn, &board_id)
}

/// Añade una columna al final del tablero.
///
/// # Errors
///
/// - [`NbError::Internal`] si el nombre viene vacío o no caben más columnas.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_add_column(
    state: State<'_, AppState>,
    board_id: String,
    title: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::anadir_columna(&mut conn, &board_id, &title)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Cambia el nombre de una columna.
///
/// # Errors
///
/// - [`NbError::Internal`] si el nombre viene vacío o la columna no es del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_rename_column(
    state: State<'_, AppState>,
    board_id: String,
    column_id: String,
    title: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let conn = state.db.connection()?;
    crate::boards::renombrar_columna(&conn, &board_id, &column_id, &title)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Borra una columna y sus tarjetas. **Las notas siguen existiendo.**
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_delete_column(
    state: State<'_, AppState>,
    board_id: String,
    column_id: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::borrar_columna(&mut conn, &board_id, &column_id)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Lleva una columna a otro lugar del tablero.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_move_column(
    state: State<'_, AppState>,
    board_id: String,
    column_id: String,
    position: u32,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::mover_columna(&mut conn, &board_id, &column_id, position as usize)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Pone una nota al final de una columna; si ya estaba en el tablero, la mueve.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero o no caben más tarjetas.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_add_note(
    state: State<'_, AppState>,
    board_id: String,
    column_id: String,
    note_id: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::poner_nota(&mut conn, &board_id, &column_id, &note_id)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Pone al final de una columna un enlace a otro tablero.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero, no caben más tarjetas
///   o el tablero se enlaza a sí mismo.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_add_link(
    state: State<'_, AppState>,
    board_id: String,
    column_id: String,
    links_to: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::poner_enlace(&mut conn, &board_id, &column_id, &links_to)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Crea una nota y la pone al final de una columna, en una sola escritura.
///
/// # Errors
///
/// - [`NbError::Internal`] si la columna no es del tablero o no caben más tarjetas.
/// - Lo que devuelva crear la nota.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_create_note(
    state: State<'_, AppState>,
    board_id: String,
    column_id: String,
    title: String,
    body: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::crear_nota_en(&mut conn, &board_id, &column_id, &title, &body)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Lleva una tarjeta a un lugar de una columna, la misma u otra.
///
/// # Errors
///
/// - [`NbError::Internal`] si la tarjeta o la columna no son del tablero.
/// - [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_move_card(
    state: State<'_, AppState>,
    board_id: String,
    card_id: String,
    column_id: String,
    position: u32,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::mover_tarjeta(
        &mut conn,
        &board_id,
        &card_id,
        &column_id,
        position as usize,
    )?;
    crate::boards::contenido(&conn, &board_id)
}

/// Quita una tarjeta del tablero. La nota sigue existiendo.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
///
/// Cota: la de `board_columns`.
#[tauri::command(async)]
pub fn board_remove_card(
    state: State<'_, AppState>,
    board_id: String,
    card_id: String,
) -> NbResult<Vec<crate::boards::BoardColumn>> {
    let mut conn = state.db.connection()?;
    crate::boards::quitar_tarjeta(&mut conn, &board_id, &card_id)?;
    crate::boards::contenido(&conn, &board_id)
}

/// Los formatos de cita que aportan las extensiones instaladas y activas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil.
///
/// Cota: los formatos que aportan las extensiones instaladas.
#[tauri::command(async)]
pub fn extensions_citation_formats(state: State<'_, AppState>) -> NbResult<Vec<CitationFormat>> {
    let Some(sub) = &state.extensions else {
        return Ok(Vec::new());
    };
    Ok(crate::extensions::citation::formatos(sub)?
        .into_iter()
        .map(|f| CitationFormat {
            id: format!("{}/{}", f.extension, f.id),
            extension: f.extension,
            name: f.name,
            file_extension: f.file_extension,
        })
        .collect())
}

/// Guarda las citas de una edición en el formato que aporte una extensión.
///
/// El anfitrión compone las referencias —salen de las anotaciones y del
/// catálogo— y la extensión solo las escribe. Es el mismo reparto que en los
/// formatos de documento: el dato es del usuario y la extensión recibe lo justo.
///
/// # Errors
///
/// - [`NbError::ComponentUnavailable`] si el subsistema no arrancó o la
///   extensión no está activa.
/// - [`NbError::PathNotAllowed`] si el destino no vale.
/// - [`NbError::DocumentNotFound`] si la edición no existe.
/// - [`NbError::StorageFailure`] si no se puede escribir.
#[tauri::command(async)]
pub fn save_citations(
    state: State<'_, AppState>,
    edition_id: String,
    format: String,
    path: String,
) -> NbResult<()> {
    let sub = subsistema(&state)?;
    let destino = destino_admisible(&state, &path)?;

    let formatos = crate::extensions::citation::formatos(&sub)?;
    let elegido = formatos
        .into_iter()
        .find(|f| format!("{}/{}", f.extension, f.id) == format)
        .ok_or(NbError::ComponentUnavailable)?;

    let conn = state.db.connection()?;
    let referencias = crate::extensions::citation::referencias(&conn, &edition_id)?;
    drop(conn);

    let texto = crate::extensions::citation::componer(
        &sub.motor,
        &sub.almacen,
        &elegido,
        &referencias,
        crate::extensions::Limites::default(),
    )?;

    std::fs::write(&destino, texto).map_err(|error| {
        tracing::error!(error = %error, "no se pudieron guardar las citas");
        NbError::StorageFailure
    })
}

/// Los fondos sonoros que aportan las extensiones instaladas y activas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil.
///
/// Cota: los ambientes que aportan las extensiones instaladas.
#[tauri::command(async)]
pub fn extensions_soundscapes(state: State<'_, AppState>) -> NbResult<Vec<ExtensionSoundscape>> {
    let Some(sub) = &state.extensions else {
        return Ok(Vec::new());
    };
    let registro = sub.registro()?;
    Ok(registro
        .instaladas()
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| Some((e, registro.manifiesto(&e.id)?)))
        .flat_map(|(e, m)| {
            m.contributes
                .soundscapes
                .iter()
                .map(|sonido| ExtensionSoundscape {
                    id: format!("{}/{}", e.id, sonido.id),
                    extension: e.id.clone(),
                    name: sonido.name.clone(),
                    kind: sonido.kind.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect())
}

/// Los modos de lectura que aportan las extensiones instaladas y activas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil.
///
/// Cota: los modos que aportan las extensiones instaladas.
#[tauri::command(async)]
pub fn extensions_reading_modes(state: State<'_, AppState>) -> NbResult<Vec<ExtensionReadingMode>> {
    let Some(sub) = &state.extensions else {
        return Ok(Vec::new());
    };
    let registro = sub.registro()?;
    Ok(registro
        .instaladas()
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| Some((e, registro.manifiesto(&e.id)?)))
        .flat_map(|(e, m)| {
            m.contributes
                .reading_modes
                .iter()
                .map(|modo| ExtensionReadingMode {
                    // El identificador que se guarda lleva delante quién lo
                    // aporta: dos paquetes pueden llamar igual a su modo, y sin
                    // el prefijo el segundo instalado se llevaría al primero.
                    id: format!("{}/{}", e.id, modo.id),
                    extension: e.id.clone(),
                    name: modo.name.clone(),
                    background: modo.background.clone(),
                    text: modo.text.clone(),
                    brightness: modo.brightness,
                    warmth: modo.warmth,
                    monochrome: modo.monochrome,
                })
                .collect::<Vec<_>>()
        })
        .collect())
}

/// Los temas que aportan las extensiones instaladas y activas.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede leer el perfil.
///
/// Cota: los temas que aportan las extensiones instaladas.
#[tauri::command(async)]
pub fn extensions_themes(state: State<'_, AppState>) -> NbResult<Vec<ExtensionTheme>> {
    let Some(sub) = &state.extensions else {
        return Ok(Vec::new());
    };
    let registro = sub.registro()?;
    Ok(registro
        .instaladas()
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| Some((e, registro.manifiesto(&e.id)?)))
        .filter(|(_, m)| !m.contributes.theme.is_empty())
        .map(|(e, m)| ExtensionTheme {
            extension: e.id.clone(),
            name: m.name.clone(),
            tokens: m.contributes.theme.clone(),
        })
        .collect())
}

/// Los textos legales que viajan con la aplicación (`LICENSE`, `NOTICE` y
/// `THIRD-PARTY-NOTICES.md`), servidos desde el binario para la pantalla
/// `Acerca de`.
///
/// # Por qué cruzan por comando y no entran en el paquete de arranque
///
/// `THIRD-PARTY-NOTICES.md` ronda los 67 KB. Meterlo en el bundle de JavaScript
/// se comería más de la cuarta parte del presupuesto de arranque de 250 KB
/// (`performance-budgets.md`, sección 4.11) por una pantalla que solo se abre
/// cuando alguien entra a `Acerca de`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct LegalNotices {
    /// Texto íntegro de `LICENSE` (Apache License 2.0).
    pub license: String,
    /// Texto íntegro de `NOTICE` (sección 4 de Apache 2.0).
    pub notice: String,
    /// Texto íntegro de `THIRD-PARTY-NOTICES.md` generado por `pnpm notices`.
    pub third_party: String,
}

/// Devuelve `LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md` para `Acerca de`.
///
/// # Errors
///
/// No falla: los tres textos se incrustan en compilación desde la raíz del
/// repositorio, que son los mismos archivos que `bundle.resources` empaqueta en
/// el instalador.
///
/// Cota: tres textos fijos compilados en el binario (~78 KB en total).
#[tauri::command(async)]
pub fn legal_notices() -> NbResult<LegalNotices> {
    Ok(LegalNotices {
        license: include_str!("../../LICENSE").to_owned(),
        notice: include_str!("../../NOTICE").to_owned(),
        third_party: include_str!("../../THIRD-PARTY-NOTICES.md").to_owned(),
    })
}

#[cfg(test)]
mod pruebas_de_la_ficha {
    use super::campo_editable;

    #[test]
    fn solo_se_editan_a_mano_los_campos_de_la_edicion_que_ofrece_la_ficha() {
        for campo in [
            "title",
            "series_title",
            "series_index",
            "publisher",
            "published_date",
            "language",
            "description",
        ] {
            assert!(campo_editable(campo).is_some(), "{campo}");
        }
        // El autor es un contribuidor compartido, y lo demás no es de la edición.
        for campo in [
            "name",
            "canonical_title",
            "cover_path",
            "title; DROP TABLE editions",
        ] {
            assert!(campo_editable(campo).is_none(), "{campo}");
        }
    }
}
