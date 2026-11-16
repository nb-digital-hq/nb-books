//! Sesión de lectura: lo que la interfaz recibe al abrir un documento.
//!
//! # Por qué existe
//!
//! Sin esta capa, la interfaz tendría que preguntar el formato, deducir qué
//! controles corresponden y acertar. Eso pone la regla en la pantalla, y una
//! regla en la pantalla se olvida cada vez que se añade un formato: el día que
//! entre el audiolibro habría que acordarse de ir a tocar el lector para que no
//! ofrezca cambiar la tipografía de algo que no tiene letras.
//!
//! Aquí la relación es la contraria. **El proveedor declara lo que sabe hacer y
//! la sesión lo transporta tal cual.** La interfaz no decide qué se puede: pinta
//! lo que se le dice que existe. Un formato nuevo llega con sus capacidades
//! puestas y la interfaz se adapta sin tocarla.
//!
//! # Lo que no se muestra deshabilitado
//!
//! Un control deshabilitado es una promesa a medias: el usuario lo ve, lo
//! intenta y no pasa nada. Un cómic no tiene tipografía que ajustar, y decirlo
//! con un botón gris es peor que no decir nada. Por eso las capacidades son
//! booleanos que la interfaz usa para **omitir**, no para atenuar.
//!
//! Contrato: `docs/architecture/reader-contracts.md`, sección 4.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::locator::Locator;
use super::progress::{self, ProgressTracker};
use super::provider::{
    DocumentProvider, Navigation, Precision, ProviderRegistry, ReaderCapabilities, Rendered,
    Resolution,
};
use crate::db::catalog::{Availability, File};
use crate::db::Database;
use crate::error::{NbError, NbResult};

/// Lo que la interfaz recibe al abrir un documento.
///
/// Va todo junto en una sola respuesta a propósito: la alternativa es que la
/// interfaz pida capacidades, después el índice y después la posición, y pinte
/// tres veces mientras llegan. Es un objeto pequeño; el contenido no viaja aquí.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct SessionInfo {
    /// Archivo abierto.
    pub file_id: String,
    /// Edición a la que pertenece: es lo que llevan las anotaciones.
    pub edition_id: String,
    /// Título de la obra abierta.
    ///
    /// Va aquí y no lo trae la interfaz de la fila de biblioteca porque un
    /// documento se puede abrir sin pasar por ella —desde Inicio, desde una
    /// recomendación, desde una anotación— y en esos casos la interfaz no tenía
    /// forma de nombrar lo que estaba leyendo.
    pub title: String,
    /// Idioma de la edición, como etiqueta BCP-47, si se conoce.
    ///
    /// Está aquí por la voz. Sin él, leer en alto usaba la primera voz del
    /// listado del sintetizador —que por orden alfabético es afrikáans— y una
    /// novela en español sonaba a ruido. No es un defecto de la síntesis: es
    /// preguntarle en el idioma equivocado.
    pub language: Option<String>,
    pub media_type: String,
    /// Lo que este formato sabe hacer. La interfaz se construye desde aquí.
    pub capabilities: ReaderCapabilities,
    /// Índice navegable. Vacío si el formato no tiene ninguno.
    pub navigation: Navigation,
    /// Dónde empieza la lectura.
    pub locator: Locator,
    /// Si la posición inicial se reancló de forma aproximada.
    ///
    /// Cierto cuando el progreso guardado ya no resuelve exactamente: el
    /// documento cambió de edición, se actualizó, o se abrió con otro motor. La
    /// interfaz **avisa**; devolver al usuario a un sitio parecido sin decírselo
    /// es lo que ADR-004 existe para impedir.
    pub approximate: bool,
    /// Si el documento se abre por primera vez, sin progreso guardado.
    ///
    /// Distinto de "el progreso apunta al principio": una cosa es no haber
    /// empezado y otra haber vuelto a la primera página a propósito.
    pub fresh: bool,
}

/// Un documento abierto para leer.
/// Cuánto texto tiene que traer una sección para considerarla el principio.
///
/// Doscientos caracteres: una cubierta, una página de créditos o una dedicatoria
/// se quedan muy por debajo, y un primer capítulo de verdad los pasa de largo
/// sin acercarse. No es un umbral fino porque no hace falta que lo sea.
const TEXTO_MINIMO: usize = 200;

/// Cuántas secciones se miran antes de rendirse.
///
/// El preliminar de un libro son unas pocas páginas. Mirar más sería pagar
/// aperturas de archivo por un caso que no existe, y abrir un documento es
/// justo donde no sobra tiempo.
const PRELIMINARES: usize = 12;

/// Dónde se abre un documento que nunca se ha leído.
///
/// **No en la primera entrada del índice.** Esa entrada es, casi siempre, la
/// cubierta: una imagen y tres etiquetas. Medido sobre una biblioteca real, la
/// primera sección de un EPUB cualquiera devolvía 186 caracteres de marcado y la
/// séptima 68.658. Abrir por la primera dejaba al lector con una página en
/// blanco y a quien la miraba con la impresión de que el documento no había
/// cargado.
///
/// Se busca la primera sección con texto de verdad, y si no la hay —un documento
/// que es todo imágenes, por ejemplo— se vuelve a la primera entrada, que es lo
/// que había. Nunca falla por no encontrarla: un documento raro se abre igual.
fn primera_con_texto(
    provider: &dyn DocumentProvider,
    navigation: &crate::reader::Navigation,
) -> NbResult<Locator> {
    let primera = navigation
        .entries
        .first()
        .map(|e| e.locator.clone())
        .ok_or(NbError::DocumentCorrupted)?;

    // Solo aplica a documentos de flujo: un PDF se pinta como imagen, así que
    // `render` nunca devuelve marcado y llamarlo en las doce primeras entradas
    // solo rasterizaría doce páginas para descartarlas.
    if !provider.capabilities().reflow {
        return Ok(primera);
    }

    for entrada in navigation.entries.iter().take(PRELIMINARES) {
        let Ok(resuelta) = provider.resolve(&entrada.locator) else {
            continue;
        };
        let Ok(crate::reader::Rendered::Markup { html, .. }) = provider.render(&resuelta.position)
        else {
            continue;
        };
        if texto_visible(&html) >= TEXTO_MINIMO {
            return Ok(entrada.locator.clone());
        }
    }

    Ok(primera)
}

/// Cuenta los caracteres que quedan al quitar las etiquetas.
///
/// Contar el marcado entero mediría el peso del envoltorio: una cubierta con una
/// imagen y sus atributos supera los doscientos caracteres sin una sola palabra.
fn texto_visible(html: &str) -> usize {
    let mut dentro = false;
    let mut cuenta = 0;
    for c in html.chars() {
        match c {
            '<' => dentro = true,
            '>' => dentro = false,
            c if !dentro && !c.is_whitespace() => cuenta += 1,
            _ => {}
        }
    }
    cuenta
}

pub struct ReadingSession {
    info: SessionInfo,
    provider: Box<dyn DocumentProvider>,
    /// Posición actual. Detrás de un cerrojo porque la interfaz la mueve desde
    /// un comando y el resto de la sesión se comparte por referencia.
    actual: Mutex<Locator>,
    db: Database,
    tracker: std::sync::Arc<ProgressTracker>,
    /// Fila de `reading_sessions` que esta sesión abrió, si se pudo abrir.
    ///
    /// Opcional porque no poder registrar la sesión **no** puede impedir leer:
    /// la analítica local es un extra, y hacerla obligatoria convertiría un
    /// problema de la base en un libro que no se abre.
    registro: Option<String>,
    /// Dónde se apunta que el motor se ha caído, si hay dónde.
    ///
    /// Opcional para que una sesión se pueda construir en una prueba sin montar
    /// medio runtime. Sin monitor, el pánico sigue conteniéndose y devolviendo
    /// error: lo único que se pierde es el aviso a la interfaz.
    monitor: Option<crate::monitor::Monitor>,
    /// Los nombres del libro, calculados la primera vez que se piden.
    ///
    /// Viven lo que vive el documento abierto y no se guardan: el disco que cuestan
    /// es cero. Recalcularlos cada vez que se abre el panel sería volver a recorrer
    /// el libro entero por algo que no ha cambiado.
    entidades: std::sync::OnceLock<crate::entidades::EntityIndex>,
}

impl ReadingSession {
    /// Abre un archivo de la biblioteca con el proveedor que le corresponda.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si el archivo no está registrado.
    /// - [`NbError::FileNotFound`] si está registrado pero no se puede leer
    ///   ahora mismo: volumen desconectado, permisos, soporte dañado. Se
    ///   distingue del anterior porque la interfaz reacciona distinto: uno es
    ///   "esto no existe" y el otro, "conecta el disco".
    /// - [`NbError::ComponentUnavailable`] si esta instalación no tiene
    ///   proveedor para ese tipo de medio. Deliberadamente distinto de
    ///   [`NbError::FormatUnsupported`]: falta un motor, no está roto el libro.
    /// - Los del formato al abrirlo.
    pub fn open(
        db: &Database,
        registry: &ProviderRegistry,
        file_id: &str,
        tracker: std::sync::Arc<ProgressTracker>,
        monitor: Option<crate::monitor::Monitor>,
    ) -> NbResult<Self> {
        let conn = db.connection()?;
        let archivo = File::find(&conn, file_id)?.ok_or(NbError::DocumentNotFound)?;

        // Se comprueba lo que dice el registro, no el disco: `refresh_availability`
        // es quien mira el sistema de archivos, y hacerlo otra vez aquí pondría
        // una llamada al disco en el camino de abrir cada documento.
        if archivo.availability != Availability::Available {
            tracing::info!(
                // no es del usuario: es el estado de disponibilidad, una lista cerrada.
                availability = archivo.availability.as_str(),
                "se intentó abrir un archivo que no está disponible"
            );
            return Err(NbError::FileNotFound);
        }

        // La dirección del archivo son dos datos y no uno: su raíz y su ruta
        // dentro de ella. Una raíz sin localizar deja el documento sin
        // dirección, y eso es lo mismo que no encontrarlo.
        let ruta = crate::library::resolve_path(&conn, &archivo)?.ok_or_else(|| {
            tracing::info!("se intentó abrir un archivo cuya raíz no está localizada");
            NbError::FileNotFound
        })?;

        let provider = registry.open(&archivo.media_type, &ruta)?;
        let navigation = provider.navigation()?;

        let principio = primera_con_texto(provider.as_ref(), &navigation)?;

        // El progreso guardado manda sobre el principio del documento. Si ya no
        // resuelve exactamente, se coloca donde la cascada de ADR-004 alcance y
        // se marca: la alternativa, devolver al principio en silencio, pierde
        // trabajo del usuario sin decírselo.
        let guardado = progress::load(&conn, &archivo.edition_id, progress::PERFIL)?;
        let fresh = guardado.is_none();
        let (locator, approximate) = match guardado {
            Some(anterior) => {
                let precision = provider
                    .resolve(&anterior.locator)
                    .map(|r| r.precision)
                    .unwrap_or(Precision::Approximate);
                (anterior.locator, precision.requires_warning())
            }
            None => (principio, false),
        };

        if approximate {
            tracing::info!("el progreso guardado se restauró de forma aproximada");
        }

        let registro = match progress::open_session(&conn, &archivo.edition_id, &locator) {
            Ok(id) => Some(id),
            Err(error) => {
                // No poder registrar la sesión no puede impedir leer.
                tracing::warn!(code = error.code(), "no se registró la sesión de lectura");
                None
            }
        };

        // El título y el idioma, con una consulta de dos columnas: traer la
        // edición entera para leer dos cadenas sería traer quince campos que
        // nadie mira. Si falla no se deja de abrir: se abre sin nombre y sin
        // idioma, que es peor pero no es perder nada.
        let (titulo, idioma): (String, Option<String>) = conn
            .query_row(
                "SELECT title, language FROM editions WHERE id = ?1",
                [&archivo.edition_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or_default();

        let info = SessionInfo {
            file_id: archivo.id.clone(),
            edition_id: archivo.edition_id.clone(),
            title: titulo,
            language: idioma.filter(|l| !l.trim().is_empty()),
            media_type: archivo.media_type.clone(),
            capabilities: provider.capabilities(),
            navigation,
            locator: locator.clone(),
            approximate,
            fresh,
        };

        tracing::info!(
            media_type = %info.media_type,
            typography = info.capabilities.typography,
            fresh,
            "sesión de lectura abierta"
        );

        Ok(Self {
            info,
            provider,
            actual: Mutex::new(locator),
            db: db.clone(),
            tracker,
            registro,
            monitor,
            entidades: std::sync::OnceLock::new(),
        })
    }

    /// Lo que se le devuelve a la interfaz al abrir.
    #[must_use]
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    /// Capacidades del formato abierto.
    #[must_use]
    pub fn capabilities(&self) -> ReaderCapabilities {
        self.info.capabilities
    }

    /// Posición actual.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo quedó envenenado por un pánico de
    /// otro hilo. No se recupera en silencio: significaría devolver una posición
    /// de la que no se sabe si es la buena.
    pub fn current(&self) -> NbResult<Locator> {
        self.actual
            .lock()
            .map(|l| l.clone())
            .map_err(|_| NbError::Internal)
    }

    /// Ejecuta algo del proveedor **conteniendo su pánico**.
    ///
    /// # Por qué hace falta, si el perfil no aborta
    ///
    /// El perfil de publicación evita `panic = "abort"` a propósito, de modo que
    /// un pánico desenrolla en lugar de matar el proceso. Pero desenrollar por
    /// sí solo no basta: sin nadie que lo recoja, el pánico sube por el comando
    /// del puente y quien lo pidió se queda esperando una respuesta que no
    /// llega. Para el usuario eso es exactamente la pantalla en blanco que el
    /// escenario 1 de `chaos.md` prohíbe.
    ///
    /// Aquí se recoge y se convierte en un error de dominio, que es lo que la
    /// interfaz sabe pintar. El componente pasa además a `Failed` con su motivo,
    /// porque un motor que se ha caído no está "no soportado": está roto, y son
    /// dos cosas distintas para quien lo lee.
    ///
    /// **No se propaga el mensaje del pánico**: puede llevar rutas del usuario o
    /// detalles internos, y el contrato de errores prohíbe que salgan.
    fn sin_tumbar<T>(
        &self,
        que: &'static str,
        operacion: impl FnOnce() -> NbResult<T>,
    ) -> NbResult<T> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operacion)) {
            Ok(resultado) => resultado,
            Err(_) => {
                tracing::error!(operacion = que, "el motor de formato entró en pánico");
                if let Some(monitor) = &self.monitor {
                    monitor.set(
                        crate::monitor::components::READER,
                        crate::monitor::ComponentState::Failed,
                        Some("ENGINE_CRASH"),
                    );
                }
                Err(NbError::ComponentUnavailable)
            }
        }
    }

    /// Mueve la lectura a un locator y dice con qué precisión se consiguió.
    ///
    /// Devuelve la resolución en lugar de tragársela porque una posición
    /// reanclada de forma aproximada **se avisa** (ADR-004): que el usuario
    /// aparezca en un sitio parecido sin saberlo es exactamente lo que el modelo
    /// existe para impedir.
    ///
    /// # Errors
    ///
    /// [`NbError::DocumentNotFound`] si el locator no se resuelve de ninguna de
    /// las formas de la cascada, o [`NbError::Internal`] si el cerrojo está
    /// envenenado.
    pub fn go_to(&self, locator: &Locator) -> NbResult<Resolution> {
        let resolucion = self.provider.resolve(locator)?;

        if resolucion.precision.requires_warning() {
            tracing::info!("una posición se reancló de forma aproximada");
        }

        // Se guarda el locator que pidió quien llama, no uno reconstruido desde
        // la posición resuelta: el original lleva la cita, y perderla dejaría a
        // la siguiente resolución sin el segundo escalón de la cascada.
        *self.actual.lock().map_err(|_| NbError::Internal)? = locator.clone();

        // Apuntar no es escribir: el registrador agrupa. Un fallo aquí no puede
        // impedir moverse por el documento, así que se registra y se sigue.
        if let Err(error) = self
            .tracker
            .note(&self.info.edition_id, locator, resolucion.precision)
        {
            tracing::warn!(code = error.code(), "no se pudo apuntar el progreso");
        }

        Ok(resolucion)
    }

    /// Escribe la última posición apuntada sin cerrar el documento.
    ///
    /// Lo pide la interfaz al salir del lector con el libro abierto, para que la
    /// biblioteca y el lector digan la misma cifra. Ver
    /// [`super::progress::ProgressTracker::write_pending`].
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la escritura falla.
    pub fn write_pending(&self) -> NbResult<()> {
        self.tracker.write_pending(&self.info.edition_id)
    }

    /// Cierra la sesión: escribe lo pendiente y cierra su registro.
    ///
    /// Se llama al cerrar el documento y **no** es opcional: el antirrebote deja
    /// la última posición apuntada sin escribir, y es justo la que le importa al
    /// usuario cuando vuelva.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la escritura falla.
    pub fn finish(&self) -> NbResult<()> {
        let actual = self.current()?;
        self.tracker.flush(&self.info.edition_id)?;

        if let Some(registro) = &self.registro {
            let conn = self.db.connection()?;
            progress::close_session(&conn, registro, &actual)?;
        }

        Ok(())
    }

    /// Pinta la posición actual.
    ///
    /// # Errors
    ///
    /// Los del formato, o [`NbError::Internal`] si el cerrojo está envenenado.
    pub fn render(&self) -> NbResult<Rendered> {
        let actual = self.current()?;
        self.sin_tumbar("render", || {
            let resolucion = self.provider.resolve(&actual)?;
            self.provider.render(&resolucion.position)
        })
    }

    /// Pinta una posición cualquiera del documento.
    ///
    /// Existe además de [`Self::render`] porque la superficie pinta la página
    /// siguiente antes de que el usuario llegue a ella, y pedirla no puede
    /// significar moverse: si pintar moviera, adelantar una página adelantaría
    /// el progreso del usuario sin que lo hubiera leído.
    ///
    /// # Errors
    ///
    /// Los del formato al resolver o al pintar.
    pub fn render_at(&self, locator: &Locator) -> NbResult<Rendered> {
        self.sin_tumbar("render_at", || {
            let resolucion = self.provider.resolve(locator)?;
            self.provider.render(&resolucion.position)
        })
    }

    /// Los nombres del libro con dónde aparecen, calculados una vez.
    ///
    /// # Errors
    ///
    /// [`NbError::FormatUnsupported`] si el documento no tiene texto.
    pub fn entities(&self) -> NbResult<crate::entidades::EntityIndex> {
        if let Some(hecho) = self.entidades.get() {
            return Ok(hecho.clone());
        }
        let indice =
            self.sin_tumbar("entities", || crate::entidades::construir(&*self.provider))?;
        let _ = self.entidades.set(indice.clone());
        Ok(indice)
    }

    /// Dónde aparece un nombre del libro: según el índice de búsqueda si está, y si
    /// no, según el recorrido del texto.
    ///
    /// # Errors
    ///
    /// [`NbError::FormatUnsupported`] si el documento no tiene texto, o
    /// [`NbError::StorageFailure`] si la base no responde.
    pub fn entity_places(&self, nombre: &str) -> NbResult<crate::entidades::EntityPlaces> {
        let conn = self.db.connection()?;
        if let Some(places) =
            crate::entidades::sitios_del_indice(&conn, &self.info.edition_id, nombre)?
        {
            return Ok(crate::entidades::EntityPlaces {
                places,
                from_index: true,
            });
        }
        drop(conn);
        let places = self
            .entities()?
            .entities
            .into_iter()
            .find(|e| e.name == nombre)
            .map(|e| e.places)
            .unwrap_or_default();
        Ok(crate::entidades::EntityPlaces {
            places,
            from_index: false,
        })
    }

    /// La narración que trae el propio libro para un capítulo, si la trae.
    ///
    /// # Errors
    ///
    /// Los del formato al leer la superposición o su audio.
    pub fn synchronized_audio(
        &self,
        href: &str,
    ) -> NbResult<Option<crate::reader::provider::SynchronizedAudio>> {
        self.sin_tumbar("synchronized_audio", || {
            self.provider.synchronized_audio(href)
        })
    }

    /// Tamaño de la página que se pinta en una posición.
    ///
    /// # Errors
    ///
    /// [`NbError::FormatUnsupported`] si el formato no se pinta como imagen, y
    /// los del formato al resolver.
    pub fn page_size(&self, locator: &Locator) -> NbResult<super::provider::PageSize> {
        self.sin_tumbar("page_size", || {
            let resolucion = self.provider.resolve(locator)?;
            self.provider.page_size(&resolucion.position)
        })
    }

    /// Capa de texto de la posición, para superponerla al pintado.
    ///
    /// # Errors
    ///
    /// [`NbError::FormatUnsupported`] si el formato no se pinta como imagen, y
    /// los del formato al resolver.
    pub fn text_layer(&self, locator: &Locator) -> NbResult<super::provider::TextLayer> {
        self.sin_tumbar("text_layer", || {
            let resolucion = self.provider.resolve(locator)?;
            self.provider.text_layer(&resolucion.position)
        })
    }

    /// Locator de una cita seleccionada, anclado donde de verdad está.
    ///
    /// La pista de progresión es la posición actual de lectura, y no un dato que
    /// tenga que aportar quien llama: quien selecciona está leyendo aquí.
    ///
    /// # Errors
    ///
    /// - [`NbError::PathNotAllowed`] si la cita está vacía.
    /// - [`NbError::DocumentNotFound`] si la cita no aparece en el documento.
    /// - [`NbError::FormatUnsupported`] si el formato no tiene texto.
    pub fn locator_for_quote(&self, quote: &str) -> NbResult<Locator> {
        let pista = self.current()?.locations.total_progression;
        self.provider.locator_for_quote(quote, pista)
    }

    /// Texto plano alrededor de la posición actual.
    ///
    /// # Errors
    ///
    /// [`NbError::FormatUnsupported`] si el formato no tiene texto que dar, que
    /// es lo que `capabilities().text_extraction` anticipa.
    pub fn text(&self, len: usize) -> NbResult<String> {
        let actual = self.current()?;
        self.provider.text_range(&actual, len)
    }

    /// Índice navegable.
    #[must_use]
    pub fn navigation(&self) -> &Navigation {
        &self.info.navigation
    }

    /// Reancla las anotaciones vivas del documento abierto.
    ///
    /// Es lo que la interfaz pide al pintar una página: hay que saber cuáles
    /// llevan aviso **antes** de dibujarlas.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn annotations(&self) -> NbResult<crate::annotations::AnchorReport> {
        let conn = self.db.connection()?;
        crate::annotations::anchor(&conn, self.provider.as_ref(), &self.info.edition_id)
    }

    /// Panel de anotaciones del documento abierto, en orden de lectura.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si la consulta falla.
    pub fn outline(&self) -> NbResult<Vec<crate::annotations::OutlineEntry>> {
        let conn = self.db.connection()?;
        crate::annotations::outline(&conn, self.provider.as_ref(), &self.info.edition_id)
    }

    /// Salta a una anotación y dice con qué precisión se llegó.
    ///
    /// Es [`Self::go_to`] con el ancla de la anotación, y existe con nombre
    /// propio porque saltar a una anotación **mueve el progreso**: quien salta
    /// se queda ahí leyendo, y guardar el progreso solo cuando se pasa de página
    /// perdería ese salto.
    ///
    /// # Errors
    ///
    /// - [`NbError::DocumentNotFound`] si la anotación no existe, está dada de
    ///   baja, o su ancla no se resuelve por ninguna vía de la cascada.
    /// - [`NbError::StorageFailure`] si la consulta falla.
    pub fn go_to_annotation(&self, annotation_id: &str) -> NbResult<Resolution> {
        let conn = self.db.connection()?;
        let anotacion =
            crate::annotations::find(&conn, annotation_id)?.ok_or(NbError::DocumentNotFound)?;

        if !anotacion.is_live() {
            tracing::info!("se intentó saltar a una anotación dada de baja");
            return Err(NbError::DocumentNotFound);
        }
        if anotacion.edition_id != self.info.edition_id {
            tracing::warn!("se intentó saltar a una anotación de otro documento");
            return Err(NbError::DocumentNotFound);
        }

        drop(conn);
        self.go_to(&anotacion.target)
    }

    /*
     * Aquí vivía `precision_of`, que decía con qué precisión se resolvería un
     * locator sin moverse a él, "para pintar las anotaciones de una página".
     *
     * Se retira el 2026-09-01, y no por no usarse: por **no ser la forma que
     * haría falta**. Pintar los resaltados dentro del texto es una cuestión abierta, y necesita la precisión de
     * todas las anotaciones del tramo visible, no la de un locator suelto; eso
     * ya lo da `annotations::anchor`, que resuelve el conjunto de una pasada y
     * devuelve `warn` y `relocated` por anotación.
     *
     * Andamio para una decisión que no se ha tomado, con la forma equivocada y
     * sin nada que lo ejercitara: afirmaba servir para algo y nadie lo había
     * comprobado nunca. Si G1 se responde y hace falta, es una línea, y entonces
     * vuelve con quien la llame y con su prueba.
     */
}

/// Las sesiones abiertas ahora mismo.
///
/// # Por qué el archivo es el identificador
///
/// No hay identificadores de sesión. Abrir dos veces el mismo documento devuelve
/// la misma sesión, con su posición donde el usuario la dejó. La alternativa
/// (una sesión nueva por apertura, con su propio identificador) obliga a la
/// interfaz a llevar la cuenta y a cerrarlas bien, y el día que se le olvide una
/// el usuario tendrá el mismo libro abierto dos veces en dos sitios distintos.
pub struct SessionStore {
    abiertas: Mutex<Vec<(String, std::sync::Arc<ReadingSession>)>>,
    tracker: std::sync::Arc<ProgressTracker>,
    /// Dónde apuntar que un motor de formato se ha caído. Ver `chaos.md`, 1.
    monitor: Option<crate::monitor::Monitor>,
}

/// Cuántos documentos pueden estar abiertos a la vez.
///
/// Hay tope porque cada sesión retiene el contenedor abierto y, en el PDF, la
/// caché de páginas rendidas. Sin tope, una tarde de biblioteca deja doscientos
/// documentos abiertos que nadie cerró. Ocho es más de lo que nadie lee a la
/// vez y sigue siendo poco que retener.
const MAXIMO_ABIERTAS: usize = 8;

impl SessionStore {
    /// Crea un almacén vacío que apunta el progreso en `tracker`.
    #[must_use]
    pub fn new(tracker: std::sync::Arc<ProgressTracker>) -> Self {
        Self {
            abiertas: Mutex::new(Vec::new()),
            tracker,
            monitor: None,
        }
    }

    /// Le da al almacén dónde apuntar que un motor se ha caído.
    ///
    /// Se pone después de construirlo y no en el constructor porque el monitor
    /// se crea junto al bus, y el bus se crea junto al almacén: pedirlo aquí
    /// obligaría a ordenar la construcción del runtime alrededor de esto.
    #[must_use]
    pub fn con_monitor(mut self, monitor: crate::monitor::Monitor) -> Self {
        self.monitor = Some(monitor);
        self
    }

    /// El registrador de progreso que usan sus sesiones.
    #[must_use]
    pub fn tracker(&self) -> &std::sync::Arc<ProgressTracker> {
        &self.tracker
    }

    /// Abre un documento, o devuelve la sesión que ya estaba abierta.
    ///
    /// # Errors
    ///
    /// Los de [`ReadingSession::open`], y [`NbError::Internal`] si el cerrojo
    /// está envenenado.
    pub fn open(
        &self,
        db: &Database,
        registry: &ProviderRegistry,
        file_id: &str,
    ) -> NbResult<std::sync::Arc<ReadingSession>> {
        let mut abiertas = self.abiertas.lock().map_err(|_| NbError::Internal)?;

        if let Some(indice) = abiertas.iter().position(|(id, _)| id == file_id) {
            // Se mueve al final: el orden del vector es el de uso reciente, y es
            // lo que decide a quién se desaloja.
            let entrada = abiertas.remove(indice);
            let sesion = std::sync::Arc::clone(&entrada.1);
            abiertas.push(entrada);
            return Ok(sesion);
        }

        let sesion = std::sync::Arc::new(ReadingSession::open(
            db,
            registry,
            file_id,
            std::sync::Arc::clone(&self.tracker),
            self.monitor.clone(),
        )?);
        abiertas.push((file_id.to_owned(), std::sync::Arc::clone(&sesion)));

        while abiertas.len() > MAXIMO_ABIERTAS {
            let (desalojada, vieja) = abiertas.remove(0);
            // Desalojar no puede perder el progreso: se cierra como si el usuario
            // la hubiera cerrado, porque desde su punto de vista eso es lo que ha
            // pasado.
            if let Err(error) = vieja.finish() {
                tracing::error!(
                    code = error.code(),
                    "no se pudo cerrar la sesión desalojada"
                );
            }
            tracing::info!(
                abiertas = abiertas.len(),
                "se cerró la sesión menos usada para no retener documentos indefinidamente"
            );
            debug_assert_ne!(desalojada, file_id);
        }

        Ok(sesion)
    }

    /// Cierra una sesión. Cerrar lo que no está abierto no es un error.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo está envenenado.
    pub fn close(&self, file_id: &str) -> NbResult<()> {
        let sesion = {
            let mut abiertas = self.abiertas.lock().map_err(|_| NbError::Internal)?;
            let indice = abiertas.iter().position(|(id, _)| id == file_id);
            indice.map(|i| abiertas.remove(i).1)
        };

        // El cerrojo se suelta antes de escribir: guardar el progreso puede
        // esperar a la base, y sostener el cerrojo mientras tanto bloquearía a
        // quien intente abrir otro documento.
        if let Some(sesion) = sesion {
            sesion.finish()?;
        }

        Ok(())
    }

    /// Sesión ya abierta para ese archivo, si la hay.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo está envenenado.
    pub fn get(&self, file_id: &str) -> NbResult<Option<std::sync::Arc<ReadingSession>>> {
        let abiertas = self.abiertas.lock().map_err(|_| NbError::Internal)?;
        Ok(abiertas
            .iter()
            .find(|(id, _)| id == file_id)
            .map(|(_, s)| std::sync::Arc::clone(s)))
    }

    /// Cuántas hay abiertas.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo está envenenado.
    pub fn len(&self) -> NbResult<usize> {
        Ok(self.abiertas.lock().map_err(|_| NbError::Internal)?.len())
    }

    /// Indica si no hay ninguna abierta.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo está envenenado.
    pub fn is_empty(&self) -> NbResult<bool> {
        Ok(self.len()? == 0)
    }

    /// Descarta todas las sesiones abiertas sin vaciar su progreso.
    ///
    /// Se usa al vaciar la biblioteca entera: `close_all` llama a `flush_all` y
    /// volvería a insertar progreso sobre archivos que se están borrando.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el cerrojo está envenenado.
    pub fn clear(&self) -> NbResult<()> {
        self.abiertas.lock().map_err(|_| NbError::Internal)?.clear();
        self.tracker.clear()
    }
}
