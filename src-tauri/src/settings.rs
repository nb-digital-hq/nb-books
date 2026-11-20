//! Preferencias del usuario, con SQLite como fuente de verdad.
//!
//! Vivían en el almacenamiento del webview, que se pierde al limpiar los datos
//! del navegador, no entra en el respaldo ni en la exportación y no se puede
//! sincronizar. Las preferencias que más importa preservar son las de
//! accesibilidad —alto contraste, movimiento reducido, escala tipográfica— y son
//! justamente las más expuestas a perderse así.
//!
//! Contrato: `docs/adr/010-settings-persistence.md`.
//!
//! # La caché del webview
//!
//! Cinco de estas preferencias afectan al primer pintado, y leerlas por el
//! puente antes de pintar obliga a elegir entre bloquear el montaje o pintar con
//! valores por defecto y corregir después. Lo segundo produce un destello de
//! tema claro para quien usa tema oscuro, o de tipografía sin escalar para quien
//! depende de la escala: no es cosmético.
//!
//! Por eso esas cinco se copian en el almacenamiento del webview **como caché de
//! solo lectura**. Aquí se declara cuáles son ([`SettingKey::affects_first_paint`]);
//! la caché en sí es del frontend (`src/lib/core/settings.ts`).
//!
//! # Los secretos no están aquí
//!
//! Ni pueden estar: las claves son una enumeración cerrada, así que no hay forma
//! de escribir una clave arbitraria, y la propia base rechaza por disparador
//! cualquier nombre que parezca un secreto. Los tokens y las claves de API van
//! al almacén de credenciales del sistema, que llega.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// Preferencia conocida por esta versión de la aplicación.
///
/// Enumeración cerrada a propósito: es lo que impide que una clave arbitraria
/// —un secreto, por ejemplo— llegue a la base a través de la superficie pública.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingKey {
    /// Nombre del tema, o `system`. Ver `ui-components.md`, sección 5.1: un tema
    /// no es una paleta, es un perfil de sensación.
    Theme,
    /// Acento elegido por el usuario, en hexadecimal, o `theme` para el del tema.
    AccentColor,
    HighContrast,
    ReducedMotion,
    /// Factor multiplicador de la tipografía.
    FontScale,
    /// Etiqueta BCP-47.
    Language,
    /// Palabras por minuto de la lectura en voz alta.
    ReadingSpeed,
    /// Qué proveedor de IA **externo** hay, si hay alguno: `none`, `openai` —que
    /// es cualquier servicio compatible con su API— o `anthropic`. **Su
    /// credencial no vive aquí.**
    ///
    /// No decide nada sobre lo local: los embeddings y todo lo que el motor local
    /// sepa hacer siguen en el equipo sea cual sea el valor, y un proveedor externo
    /// solo se usa cuando una función lo nombra, nunca como repliegue.
    AiProvider,
    /// Cuánta memoria puede ocupar un modelo de IA cargado, en MiB: 512, 1024, 2048
    /// o 4096. Un modelo que no quepa no se carga, y lo que cabe decide el tamaño
    /// de los lotes.
    ///
    /// Existía con 2048 de fábrica y sin nadie que lo leyera. **Bajó a
    /// 1024 el 2026-09-16**, cuando por fin hubo un modelo con el que medirlo: el de
    /// embeddings del catálogo ocupa 700 MB cargado y calcula en lotes dentro de ese
    /// tope, y un modelo de generación de varios gigas no se carga sin que alguien
    /// suba el tope a sabiendas. Ver `ai::presupuesto`.
    AiMemoryLimitMb,
    /// Interruptor general de red. Con él puesto **nada sale del dispositivo**,
    /// ni siquiera lo que ya tenía consentimiento.
    OfflineMode,
    /// Anfitrión del servidor de traducción, o vacío para no tener ninguno.
    ///
    /// **Vacío de fábrica y a propósito.** Traducir manda el texto que estás
    /// leyendo a un tercero, y elegir por el usuario a qué empresa se lo manda
    /// no le corresponde al programa. Sin anfitrión, el traductor no se
    /// registra: es la misma regla que los catálogos.
    TranslationHost,
    /// Idioma al que se traduce. Vacío significa el de la interfaz.
    TranslationTarget,
    /// El tema que aporta una extensión y está puesto, o vacío para el del
    /// núcleo.
    ///
    /// Se guarda el identificador de la extensión, no sus colores: los colores
    /// son suyos y cambian cuando se actualiza. Al desinstalarla, esta
    /// preferencia se vacía sola —ver `extensions::personalization`—, porque
    /// dejarla apuntando a algo que ya no está sería exactamente el rastro que
    /// la política de secretos prohíbe.
    ExtensionTheme,
    /// Anfitrión del registro de extensiones, o vacío para no tener ninguno.
    ///
    /// **Vacío de fábrica, y no es un hueco por rellenar:** el registro oficial
    /// no existe todavía, y poner una dirección que no responde sería peor que
    /// no poner ninguna. Cuando exista, aquí irá la suya; mientras tanto, es
    /// donde se apunta un espejo o el registro de una institución.
    RegistryHost,
    /// Cuántas obras se enseñan por página de la biblioteca.
    ///
    /// Es lo que decide cuánto se pinta de una vez, así que es también lo que
    /// mantiene la biblioteca dentro del presupuesto de nodos. La interfaz solo
    /// ofrece los tamaños de `paginacion.ts`; aquí se guarda el número que se
    /// eligió.
    LibraryPageSize,
    /// Forma de la vista de la biblioteca: `cuadricula`, `mosaico` o `lista`.
    LibraryView,
    /// Por qué se ordena la biblioteca y en qué sentido: `criterio:asc` o
    /// `criterio:desc`, con el criterio de `browse::BrowseSort`.
    ///
    /// Una sola preferencia y no dos porque el sentido no significa nada sin el
    /// criterio: guardadas por separado, un fallo al escribir la segunda dejaría
    /// "por título, de la más reciente a la más antigua". Antes no
    /// se guardaba, y cada arranque volvía a "por fecha".
    LibrarySort,
    /// Qué tarjetas de la barra lateral están plegadas, separadas por comas.
    ///
    /// Las tarjetas son dinámicas —aparecen cuando dicen algo—, pero algunas
    /// dicen lo mismo durante horas, como la del índice, y ocupan un sitio que
    /// quien las ha leído ya no necesita. Plegada, la tarjeta sigue ahí en una
    /// línea: se pliega lo que se sabe, no se esconde lo que pasa.
    SidebarFoldedCards,
    /// Si Inicio enseña filas que sugieren: las de materias, autores, colecciones y
    /// al azar. Las novedades y lo empezado se enseñan siempre, porque no sugieren
    /// nada.
    ///
    /// ADR-013 mitigaba el riesgo de que Inicio fuera un escaparate con dos cosas:
    /// que cada sugerencia dijera por qué aparece, y que se pudiera **desactivar
    /// entera**. La segunda figuraba como y no tenía interruptor:
    /// la vista aceptaba la propiedad y nadie se la pasaba. Salió al ampliar Inicio
    ///.
    HomeSuggestions,
    /// Si el servidor para NB Books Mobile escucha en la red local (ADR-016).
    ///
    /// Apagado de fábrica: la aplicación no abre ningún puerto hasta que quien
    /// la usa empareja un teléfono o lo enciende en Ajustes.
    MobileServer,
    /// Cuánto disco puede ocupar lo derivado, en tanto por ciento de lo que
    /// ocupan los documentos: 20, 30 o 40.
    ///
    /// Es el tope que frena al indexador. Veinte de fábrica, que es el
    /// presupuesto medido; más, si quien tiene el disco decide gastarlo en poder
    /// buscar dentro de toda su biblioteca. Se decidió el 2026-09-12 al
    /// fijar el tope. El innegociable -lo derivado nunca pesa más que la
    /// biblioteca- no depende de este valor: ninguna opción pasa del 40 %.
    DerivedBudget,
    /// Cuántos hilos puede usar la inferencia local. `0` es automático: la mitad
    /// de los núcleos.
    ///
    /// Nunca todos: el runtime rechaza un valor que deje sin núcleo libre al hilo
    /// que pinta, porque el criterio es que la inferencia nunca degrade
    /// el presupuesto de fotograma de la lectura. El tope se hace cumplir en el
    /// runtime y no en la interfaz, que es zona no confiable.
    AiThreads,
    /// Dónde está el proveedor de IA externo: un anfitrión, con o sin esquema y con
    /// o sin `/v1`. Vacío es que no hay.
    AiHost,
    /// Qué modelo se le pide al proveedor de IA externo, tal como lo nombre él.
    AiModel,
    /// Qué modelo del catálogo genera en local, por su identificador.
    ///
    /// Vacío es **el que se recomienda para este equipo**, que es lo que hace una
    /// instalación nueva: el catálogo ofrece varios porque la máquina es lo único que
    /// no se puede igualar entre usuarios, la aplicación lee la que hay y señala uno,
    /// y quien la tiene elige. Se usa **uno a la vez**.
    AiModelLocal,
    /// Quién genera los resúmenes, las tarjetas y los conceptos: `local` o
    /// `external`.
    ///
    /// Local de fábrica, como la voz: el que no sale del dispositivo es el que no hay
    /// que autorizar. Elegir `external` sin proveedor configurado **no cae al
    /// local ni al revés**: la función dice que no hay quien la atienda, porque la
    /// regla 2 de `ai/philosophy.md` prohíbe el repliegue silencioso en las dos
    /// direcciones —el texto no sale sin decidirlo, y quien eligió un servicio no
    /// recibe sin saberlo lo que da un modelo pequeño—.
    AiGenerator,
    /// De dónde sale la voz: `local` o `remote`.
    ///
    /// Local es el sintetizador del sistema, que no sale del dispositivo y suena
    /// a máquina. Remoto es un servicio de síntesis que el usuario configura y
    /// autoriza, al que **se le manda el texto del libro**. La elección es suya
    /// y por eso está aquí y no decidida por nosotros.
    VoiceProvider,
    /// Anfitrión del servicio de síntesis, o vacío para no tener ninguno.
    ///
    /// **Vacío de fábrica y a propósito**, igual que el del traductor: elegir a
    /// quién le manda el usuario el texto de sus libros no nos corresponde. Sin
    /// anfitrión, la voz remota no se ofrece.
    SpeechHost,
    /// Modelo o motor de síntesis, tal y como lo nombre el servicio.
    SpeechModel,
    /// Voz remota, tal y como la nombre el servicio.
    SpeechVoice,
    /// Voz del sistema con la que se lee en alto, o vacío para elegirla por el
    /// idioma del documento.
    ///
    /// Vacío de fábrica **y ese es el comportamiento bueno**: la primera voz del
    /// listado del sintetizador es la del primer idioma por orden alfabético, y
    /// leer una novela en español con una voz afrikáans no se oye como una voz
    /// mal elegida, se oye como que la función no sirve.
    VoiceName,

    // --- Plano del libro -------------------------------------------------
    //
    // Los siete de ADR-012, compromiso 4. Van aquí y no en otra tabla porque
    // son preferencias como las demás: entran en la exportación y sobreviven a
    // reinstalar. Lo que las separa del plano de la aplicación no es dónde se
    // guardan sino [`SettingKey::plane`], y esa marca existe para que la
    // separación se pueda **comprobar** en lugar de confiarla a que nadie
    // mezcle los prefijos.
    /// Familia tipográfica con la que se compone el libro.
    BookFontFamily,
    /// Cuerpo del texto del libro, en píxeles.
    BookFontSize,
    /// Interlineado, como múltiplo del cuerpo.
    BookLineHeight,
    /// Margen alrededor de la columna de lectura, en píxeles.
    BookMargin,
    /// Ancho máximo de la columna, en caracteres por línea.
    BookLineWidth,
    /// Si el texto se justifica a ambos lados.
    BookJustify,
    /// Alineación del texto (izquierda, centro, derecha, justificado).
    BookAlignment,
    /// `scroll` o `paged`.
    BookPagination,
    /// Modo de lectura con nombre: día, sepia, nocturno y los demás.
    BookReadingMode,
    /// Familia del kit accesible: `ninguna`, `dislexia` o `bajaVision`.
    BookAccessibleFont,
    /// Lectura biónica.
    BookBionic,
    /// Regla que sigue a la lectura.
    BookRuler,
    /// Atenuar todo menos la línea en la que se está.
    BookLineFocus,
    /// Animación de pase de página. Desaparece con movimiento reducido.
    BookPageAnimation,
}

/// A cuál de los dos planos de personalización pertenece una preferencia.
///
/// ADR-012, compromiso 4: cambiar el tema de la aplicación no debe alterar cómo
/// se ve un libro, ni al revés. Son decisiones distintas del usuario y se
/// pierden juntas si se mezclan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum SettingPlane {
    /// Tema, densidad, acento, tipografía de la interfaz, accesibilidad.
    Application,
    /// Tipografía, cuerpo, interlineado, márgenes, ancho, justificación y
    /// paginación **del libro**.
    Book,
}

/// Todas las preferencias conocidas, en orden estable.
pub const TODAS: &[SettingKey] = &[
    SettingKey::Theme,
    SettingKey::AccentColor,
    SettingKey::HighContrast,
    SettingKey::ReducedMotion,
    SettingKey::FontScale,
    SettingKey::Language,
    SettingKey::ReadingSpeed,
    SettingKey::AiProvider,
    SettingKey::AiMemoryLimitMb,
    SettingKey::OfflineMode,
    SettingKey::TranslationHost,
    SettingKey::RegistryHost,
    SettingKey::ExtensionTheme,
    SettingKey::TranslationTarget,
    SettingKey::LibraryPageSize,
    SettingKey::LibraryView,
    SettingKey::LibrarySort,
    SettingKey::SidebarFoldedCards,
    SettingKey::HomeSuggestions,
    SettingKey::MobileServer,
    SettingKey::DerivedBudget,
    SettingKey::AiThreads,
    SettingKey::AiHost,
    SettingKey::AiModel,
    SettingKey::AiGenerator,
    SettingKey::AiModelLocal,
    SettingKey::VoiceName,
    SettingKey::VoiceProvider,
    SettingKey::SpeechHost,
    SettingKey::SpeechModel,
    SettingKey::SpeechVoice,
    SettingKey::BookFontFamily,
    SettingKey::BookFontSize,
    SettingKey::BookLineHeight,
    SettingKey::BookMargin,
    SettingKey::BookLineWidth,
    SettingKey::BookJustify,
    SettingKey::BookAlignment,
    SettingKey::BookPagination,
    SettingKey::BookReadingMode,
    SettingKey::BookAccessibleFont,
    SettingKey::BookBionic,
    SettingKey::BookRuler,
    SettingKey::BookLineFocus,
    SettingKey::BookPageAnimation,
];

impl SettingKey {
    /// Nombre tal y como se almacena.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::AccentColor => "accent_color",
            Self::HighContrast => "high_contrast",
            Self::ReducedMotion => "reduced_motion",
            Self::FontScale => "font_scale",
            Self::Language => "language",
            Self::ReadingSpeed => "reading_speed",
            Self::AiProvider => "ai_provider",
            Self::AiMemoryLimitMb => "ai_memory_limit_mb",
            Self::OfflineMode => "offline_mode",
            Self::TranslationHost => "translation_host",
            Self::RegistryHost => "registry_host",
            Self::ExtensionTheme => "extension_theme",
            Self::TranslationTarget => "translation_target",
            Self::LibraryPageSize => "library_page_size",
            Self::LibraryView => "library_view",
            Self::LibrarySort => "library_sort",
            Self::SidebarFoldedCards => "sidebar_folded_cards",
            Self::HomeSuggestions => "home_suggestions",
            Self::MobileServer => "mobile_server",
            Self::DerivedBudget => "derived_budget",
            Self::AiThreads => "ai_threads",
            Self::AiHost => "ai_host",
            Self::AiModel => "ai_model",
            Self::AiGenerator => "ai_generator",
            Self::AiModelLocal => "ai_model_local",
            Self::VoiceName => "voice_name",
            Self::VoiceProvider => "voice_provider",
            Self::SpeechHost => "speech_host",
            Self::SpeechModel => "speech_model",
            Self::SpeechVoice => "speech_voice",
            Self::BookFontFamily => "book_font_family",
            Self::BookFontSize => "book_font_size",
            Self::BookLineHeight => "book_line_height",
            Self::BookMargin => "book_margin",
            Self::BookLineWidth => "book_line_width",
            Self::BookJustify => "book_justify",
            Self::BookAlignment => "book_alignment",
            Self::BookPagination => "book_pagination",
            Self::BookReadingMode => "book_reading_mode",
            Self::BookAccessibleFont => "book_accessible_font",
            Self::BookBionic => "book_bionic",
            Self::BookRuler => "book_ruler",
            Self::BookLineFocus => "book_line_focus",
            Self::BookPageAnimation => "book_page_animation",
        }
    }

    /// Plano de personalización al que pertenece.
    #[must_use]
    pub const fn plane(self) -> SettingPlane {
        match self {
            Self::BookFontFamily
            | Self::BookFontSize
            | Self::BookLineHeight
            | Self::BookMargin
            | Self::BookLineWidth
            | Self::BookJustify
            | Self::BookAlignment
            | Self::BookPagination
            | Self::BookReadingMode
            | Self::BookAccessibleFont
            | Self::BookBionic
            | Self::BookRuler
            | Self::BookLineFocus
            | Self::BookPageAnimation => SettingPlane::Book,
            _ => SettingPlane::Application,
        }
    }

    /// Lee un nombre almacenado.
    #[must_use]
    pub fn from_name(nombre: &str) -> Option<Self> {
        TODAS.iter().copied().find(|k| k.as_str() == nombre)
    }

    /// Indica si la preferencia afecta al primer pintado y, por tanto, entra en
    /// la caché del webview.
    ///
    /// La lista es la de ADR-010 y es **deliberadamente estrecha**: acotarla a
    /// cinco valores reduce la superficie de divergencia entre la caché y la
    /// fuente de verdad a lo que de verdad la justifica.
    #[must_use]
    pub const fn affects_first_paint(self) -> bool {
        matches!(
            self,
            Self::Theme
                | Self::HighContrast
                | Self::ReducedMotion
                | Self::FontScale
                | Self::Language
        )
    }

    /// Valor con el que se comporta la aplicación mientras nadie haya decidido.
    #[must_use]
    pub fn default_value(self) -> SettingValue {
        match self {
            Self::Theme => SettingValue::Text("system".to_owned()),
            // `theme` y no un color: el acento del tema es parte del tema, y
            // copiarlo aquí lo congelaría al cambiar de uno a otro.
            Self::AccentColor => SettingValue::Text("theme".to_owned()),
            // Los valores por defecto de accesibilidad son los del sistema, no
            // los nuestros: activarlos por nuestra cuenta sería tan invasivo
            // como ignorarlos.
            Self::HighContrast | Self::ReducedMotion => SettingValue::Flag(false),
            Self::FontScale => SettingValue::Number(1.0),
            Self::Language => SettingValue::Text("es".to_owned()),
            Self::ReadingSpeed => SettingValue::Number(180.0),
            Self::AiProvider => SettingValue::Text("none".to_owned()),
            Self::AiMemoryLimitMb => SettingValue::Number(1024.0),
            // Falso, y no verdadero, porque la garantía real no es este
            // interruptor sino el consentimiento por destino: sin él no sale
            // nada aunque el modo sin red esté apagado. Empezar en modo sin red
            // añadiría una segunda puerta que abrir sin proteger de nada más.
            Self::OfflineMode => SettingValue::Flag(false),

            // Sin servidor de traducción. Ver la nota de la variante: no es un
            // hueco por rellenar, es la decisión.
            Self::TranslationHost => SettingValue::Text(String::new()),
            Self::RegistryHost => SettingValue::Text(String::new()),
            Self::ExtensionTheme => SettingValue::Text(String::new()),
            // Vacío significa "el idioma de la interfaz", que es el que la
            // persona entiende. Fijar uno aquí sería suponer cuál es.
            Self::TranslationTarget => SettingValue::Text(String::new()),

            // Cincuenta: lo que llena una pantalla ancha sin obligar a
            // desplazarse mucho, y muy por debajo del presupuesto de nodos.
            Self::LibraryPageSize => SettingValue::Number(50.0),
            Self::LibraryView => SettingValue::Text("cuadricula".to_owned()),
            // Lo último que entró, primero: es lo que se busca al volver de
            // importar, y era el orden de siempre antes de guardarse.
            Self::LibrarySort => SettingValue::Text("addedAt:desc".to_owned()),
            Self::SidebarFoldedCards => SettingValue::Text(String::new()),
            Self::HomeSuggestions => SettingValue::Flag(true),
            Self::MobileServer => SettingValue::Flag(false),
            Self::DerivedBudget => SettingValue::Number(20.0),
            Self::AiThreads => SettingValue::Number(0.0),
            // Vacíos de fábrica, por lo mismo que la voz y el traductor: elegir por
            // el usuario a quién le manda el texto de sus libros no nos corresponde.
            Self::AiHost | Self::AiModel => SettingValue::Text(String::new()),
            Self::AiGenerator => SettingValue::Text("local".to_owned()),
            Self::AiModelLocal => SettingValue::Text(String::new()),
            // Ver la nota de la variante: vacío significa "la del idioma del
            // documento", no "la primera que haya".
            Self::VoiceName => SettingValue::Text(String::new()),
            // Local de fábrica: la voz que no sale del dispositivo es la que no
            // hay que autorizar, y empezar por la otra sería decidir por el
            // usuario algo que solo él puede decidir.
            Self::VoiceProvider => SettingValue::Text("local".to_owned()),
            // Los tres vacíos. Ver la nota de `SpeechHost`: no son huecos por
            // rellenar, son la decisión.
            Self::SpeechHost | Self::SpeechModel | Self::SpeechVoice => {
                SettingValue::Text(String::new())
            }

            // Ninguna familia concreta: la del sistema, que es la que ya está
            // instalada y la que el usuario lee sin darse cuenta. Nombrar una
            // aquí obligaría a distribuirla o a fallar en silencio donde no
            // esté.
            Self::BookFontFamily => SettingValue::Text("system".to_owned()),
            Self::BookFontSize => SettingValue::Number(18.0),
            Self::BookLineHeight => SettingValue::Number(1.6),
            Self::BookMargin => SettingValue::Number(32.0),
            // Sesenta y cinco caracteres por línea: el rango que la composición
            // tipográfica da por cómodo. No es una cifra nuestra.
            Self::BookLineWidth => SettingValue::Number(65.0),
            // Sin justificar. Justificar sin partición de palabras abre ríos de
            // espacio, y eso estorba más a quien tiene dislexia que la bandera
            // derecha que evita.
            Self::BookJustify => SettingValue::Flag(false),
            Self::BookAlignment => SettingValue::Text("izquierda".to_owned()),
            Self::BookPagination => SettingValue::Text("scroll".to_owned()),
            Self::BookReadingMode => SettingValue::Text("dia".to_owned()),
            Self::BookAccessibleFont => SettingValue::Text("ninguna".to_owned()),
            // El kit no toca nada mientras no se active: activarlo por nuestra
            // cuenta sería tan invasivo como ignorarlo, igual que con las
            // preferencias de accesibilidad de la aplicación.
            Self::BookBionic | Self::BookRuler | Self::BookLineFocus => SettingValue::Flag(false),
            // La animación sí viene puesta: transporta significado y cabe en el
            // presupuesto. Con movimiento reducido desaparece sola.
            Self::BookPageAnimation => SettingValue::Flag(true),
        }
    }

    /// Comprueba que el valor tiene el tipo que la preferencia declara.
    fn acepta(self, valor: &SettingValue) -> bool {
        std::mem::discriminant(valor) == std::mem::discriminant(&self.default_value())
    }
}

/// Valor de una preferencia.
///
/// Se serializa sin etiqueta, de modo que en el puente es `boolean | number |
/// string`: el frontend recibe el valor, no un envoltorio que tenga que
/// desenvolver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum SettingValue {
    /// Se llama `Flag` y no `Bool` porque `untagged` prueba las variantes en
    /// orden y el nombre no viaja; lo que importa es que sea la primera, para
    /// que `true` no acabe interpretado como número.
    Flag(bool),
    Number(f64),
    Text(String),
}

/// Preferencia con su valor efectivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Setting {
    pub key: String,
    pub value: SettingValue,
    /// Si entra en la caché de arranque del webview.
    pub affects_first_paint: bool,
    /// A cuál de los dos planos de personalización pertenece.
    pub plane: SettingPlane,
}

/// Los topes de lo derivado que se pueden elegir, en tanto por ciento.
///
/// Ninguno llega a 100: el innegociable no es una de las opciones.
pub const PRESUPUESTOS_ADMITIDOS: &[f64] = &[20.0, 30.0, 40.0];

/// Los proveedores de IA externos que se admiten.
pub const PROVEEDORES_DE_IA_ADMITIDOS: &[&str] = &["none", "openai", "anthropic"];

/// Quién puede generar.
pub const GENERADORES_DE_IA_ADMITIDOS: &[&str] = &["local", "external"];

/// De dónde puede salir la voz que lee en alto.
///
/// Tres: la del sistema, la neuronal **de aquí dentro** y la de un
/// servicio. Es lista cerrada por lo mismo que el proveedor de IA: el valor decide
/// a quién se le pide el audio, y uno inventado desde el webview no puede llegar a
/// significar nada.
pub const ORIGENES_DE_VOZ_ADMITIDOS: &[&str] = &["local", "neural", "remote"];

/// Los topes de memoria para un modelo cargado que se admiten, en MiB.
pub const MEMORIAS_DE_IA_ADMITIDAS: &[f64] = &[512.0, 1024.0, 2048.0, 4096.0, 8192.0];

/// Versión del esquema del valor que escribe esta compilación.
const VALUE_VERSION: i64 = 1;

fn fallo(error: &rusqlite::Error) -> NbError {
    // El mensaje puede citar el valor, y un valor de preferencia es un dato del
    // usuario. No se registra.
    tracing::error!(clase = %error, "consulta de preferencias fallida");
    NbError::StorageFailure
}

/// Valor efectivo de una preferencia: el guardado, o el de por defecto.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si la consulta falla o si el valor almacenado no
/// se puede interpretar.
pub fn get(conn: &Connection, key: SettingKey) -> NbResult<SettingValue> {
    let guardado: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [key.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| fallo(&e))?;

    let Some(json) = guardado else {
        return Ok(key.default_value());
    };

    match serde_json::from_str::<SettingValue>(&json) {
        Ok(valor) if key.acepta(&valor) => Ok(valor),
        _ => {
            // Un valor con otro tipo solo puede venir de una escritura que se
            // saltó esta capa. Se cae al valor por defecto en lugar de fallar:
            // una preferencia corrupta no puede impedir que alguien lea.
            tracing::warn!(
                clave = key.as_str(),
                "preferencia con un valor que no corresponde a su tipo; se usa el de por defecto"
            );
            Ok(key.default_value())
        }
    }
}

/// Escribe una preferencia.
///
/// # Errors
///
/// - [`NbError::Internal`] si el valor no tiene el tipo que la preferencia
///   declara.
/// - [`NbError::StorageFailure`] si la escritura falla.
pub fn set(conn: &Connection, key: SettingKey, value: &SettingValue) -> NbResult<()> {
    if !key.acepta(value) {
        tracing::warn!(
            clave = key.as_str(),
            "se rechazó una preferencia con un tipo que no le corresponde"
        );
        return Err(NbError::Internal);
    }

    // El tope de lo derivado es una lista cerrada y la cierra el runtime: el
    // webview es zona no confiable, y un 400 % escrito desde ahí convertiría el
    // innegociable en una sugerencia.
    if key == SettingKey::DerivedBudget
        && !matches!(value, SettingValue::Number(n) if PRESUPUESTOS_ADMITIDOS.contains(n))
    {
        tracing::warn!("se rechazó un tope de lo derivado fuera de los admitidos");
        return Err(NbError::Internal);
    }

    // Lo mismo con el presupuesto de la inferencia: los hilos son un
    // entero entre automático y todos los núcleos **menos uno**, y la memoria, una
    // de la lista. Un valor que dejara sin núcleo al hilo que pinta rompería el
    // criterio de el módulo desde la interfaz.
    if key == SettingKey::AiThreads {
        let nucleos = crate::ai::hardware::nucleos();
        let admitido = matches!(value, SettingValue::Number(n)
            if n.fract() == 0.0 && *n >= 0.0 && (*n == 0.0 || *n <= (nucleos.saturating_sub(1)).max(1) as f64));
        if !admitido {
            tracing::warn!("se rechazó un número de hilos para la IA fuera de lo admitido");
            return Err(NbError::Internal);
        }
    }
    // El proveedor externo es una lista cerrada: el valor decide a qué protocolo se
    // habla, y uno inventado desde el webview no puede llegar a significar nada.
    if key == SettingKey::AiProvider
        && !matches!(value, SettingValue::Text(t) if PROVEEDORES_DE_IA_ADMITIDOS.contains(&t.as_str()))
    {
        tracing::warn!("se rechazó un proveedor de IA fuera de los admitidos");
        return Err(NbError::Internal);
    }

    // El modelo local es una lista cerrada: la del catálogo, compilada en el binario.
    // Un identificador inventado desde el webview no puede llegar a significar nada.
    if key == SettingKey::AiModelLocal
        && !matches!(value, SettingValue::Text(t)
            if t.is_empty() || crate::ai::catalogo::generativos().any(|m| m.id == t))
    {
        tracing::warn!("se rechazó un modelo generativo que no está en el catálogo");
        return Err(NbError::Internal);
    }

    if key == SettingKey::AiGenerator
        && !matches!(value, SettingValue::Text(t) if GENERADORES_DE_IA_ADMITIDOS.contains(&t.as_str()))
    {
        tracing::warn!("se rechazó un generador de IA fuera de los admitidos");
        return Err(NbError::Internal);
    }

    if key == SettingKey::VoiceProvider
        && !matches!(value, SettingValue::Text(t) if ORIGENES_DE_VOZ_ADMITIDOS.contains(&t.as_str()))
    {
        tracing::warn!("se rechazó un origen de voz fuera de los admitidos");
        return Err(NbError::Internal);
    }

    if key == SettingKey::AiMemoryLimitMb
        && !matches!(value, SettingValue::Number(n) if MEMORIAS_DE_IA_ADMITIDAS.contains(n))
    {
        tracing::warn!("se rechazó un tope de memoria para la IA fuera de los admitidos");
        return Err(NbError::Internal);
    }

    let json = serde_json::to_string(value).map_err(|_| NbError::Internal)?;

    conn.execute(
        "INSERT INTO settings (key, value, value_version) VALUES (?1, ?2, ?3)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value,
                                         value_version = excluded.value_version,
                                         updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
        rusqlite::params![key.as_str(), json, VALUE_VERSION],
    )
    .map(|_| ())
    .map_err(|e| fallo(&e))
}

/// Devuelve una preferencia a su valor por defecto.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si el borrado falla.
pub fn reset(conn: &Connection, key: SettingKey) -> NbResult<()> {
    conn.execute("DELETE FROM settings WHERE key = ?1", [key.as_str()])
        .map(|_| ())
        .map_err(|e| fallo(&e))
}

/// Todas las preferencias con su valor efectivo.
///
/// Incluye las que nadie ha tocado, con su valor por defecto: quien lo consume
/// no debería tener que saber cuáles se han escrito alguna vez.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn snapshot(conn: &Connection) -> NbResult<Vec<Setting>> {
    TODAS
        .iter()
        .map(|&key| {
            Ok(Setting {
                key: key.as_str().to_owned(),
                value: get(conn, key)?,
                affects_first_paint: key.affects_first_paint(),
                plane: key.plane(),
            })
        })
        .collect()
}

/// Solo las preferencias que afectan al primer pintado.
///
/// Es lo que el frontend copia en su caché de arranque.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si alguna consulta falla.
pub fn first_paint(conn: &Connection) -> NbResult<Vec<Setting>> {
    Ok(snapshot(conn)?
        .into_iter()
        .filter(|s| s.affects_first_paint)
        .collect())
}

/// Escribe los valores por defecto del primer pintado como módulo de TypeScript.
///
/// Es una prueba y no un script porque así entra en el mismo mecanismo que ya
/// regenera los tipos del puente: `cargo test` la ejecuta, `pnpm check:bindings`
/// comprueba que el árbol no queda sucio, y el gancho de pre-commit lo hace
/// cumplir. El nombre empieza por `export_bindings_` justamente para que ese
/// filtro la recoja.
///
/// Existe porque el frontend necesita esos valores **antes** de poder preguntar
/// por el puente: son lo que se pinta mientras nadie ha respondido todavía.
/// Escribirlos a mano en TypeScript los condenaría a divergir de Rust en
/// silencio, y la consecuencia sería un destello de tema claro para quien usa
/// tema oscuro.
#[cfg(test)]
#[test]
#[allow(non_snake_case)]
fn export_bindings_firstpaintdefaults() {
    use std::fmt::Write as _;

    let cabecera = [
        "// This file was generated by NB Books from src-tauri/src/settings.rs.",
        "// Do not edit this file manually.",
        "",
        "import type { SettingValue } from './SettingValue';",
        "",
        "/**",
        " * Valores por defecto de las preferencias que afectan al primer pintado.",
        " *",
        " * Son lo que se pinta mientras el runtime todavía no ha respondido, así que no",
        " * pueden depender de una llamada al puente. Se generan desde Rust para que no",
        " * puedan divergir de él en silencio.",
        " */",
        "export const FIRST_PAINT_DEFAULTS: Record<string, SettingValue> = {",
        "",
    ];
    let mut ts = cabecera.join("\n");

    let del_arranque: Vec<SettingKey> = TODAS
        .iter()
        .copied()
        .filter(|k| k.affects_first_paint())
        .collect();

    for (i, clave) in del_arranque.iter().enumerate() {
        let valor = serde_json::to_string(&clave.default_value()).expect("serializable");
        let coma = if i + 1 == del_arranque.len() { "" } else { "," };
        let _ = writeln!(ts, "  {}: {valor}{coma}", clave.as_str());
    }
    ts.push_str("};\n");

    let destino = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/lib/bindings/firstPaintDefaults.ts");
    std::fs::write(destino, ts).expect("no se pudieron escribir los valores por defecto");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn base() -> Database {
        Database::open_in_memory().expect("base en memoria")
    }

    #[test]
    fn una_preferencia_no_escrita_devuelve_su_valor_por_defecto() {
        let db = base();
        let conn = db.connection().expect("conexión");

        assert_eq!(
            get(&conn, SettingKey::Theme).expect("lectura"),
            SettingValue::Text("system".to_owned())
        );
        assert_eq!(
            get(&conn, SettingKey::HighContrast).expect("lectura"),
            SettingValue::Flag(false)
        );
    }

    #[test]
    fn una_preferencia_escrita_se_relee_igual() {
        let db = base();
        let conn = db.connection().expect("conexión");

        set(&conn, SettingKey::FontScale, &SettingValue::Number(1.35)).expect("escritura");
        set(
            &conn,
            SettingKey::Theme,
            &SettingValue::Text("dark".to_owned()),
        )
        .expect("escritura");
        set(&conn, SettingKey::ReducedMotion, &SettingValue::Flag(true)).expect("escritura");

        assert_eq!(
            get(&conn, SettingKey::FontScale).expect("lectura"),
            SettingValue::Number(1.35)
        );
        assert_eq!(
            get(&conn, SettingKey::Theme).expect("lectura"),
            SettingValue::Text("dark".to_owned())
        );
        assert_eq!(
            get(&conn, SettingKey::ReducedMotion).expect("lectura"),
            SettingValue::Flag(true)
        );
    }

    /// El tipo es parte del contrato de cada preferencia: guardar la cadena
    /// "true" donde se espera un booleano produce una interfaz que se comporta
    /// al revés de lo que el usuario pidió.
    #[test]
    fn una_preferencia_con_el_tipo_equivocado_se_rechaza() {
        let db = base();
        let conn = db.connection().expect("conexión");

        for (clave, valor) in [
            (
                SettingKey::HighContrast,
                SettingValue::Text("sí".to_owned()),
            ),
            (SettingKey::FontScale, SettingValue::Flag(true)),
            (SettingKey::Language, SettingValue::Number(2.0)),
        ] {
            assert!(
                matches!(set(&conn, clave, &valor), Err(NbError::Internal)),
                "{} no debería admitir {valor:?}",
                clave.as_str()
            );
        }
    }

    /// El presupuesto de la inferencia no admite dejar sin núcleo al hilo que
    /// pinta, ni un tope de memoria que no esté en la lista.
    #[test]
    fn el_presupuesto_de_la_ia_rechaza_lo_que_romperia_el_fotograma() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let nucleos = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);

        set(&conn, SettingKey::AiThreads, &SettingValue::Number(0.0)).expect("automático");
        set(&conn, SettingKey::AiThreads, &SettingValue::Number(1.0)).expect("uno");
        if nucleos > 1 {
            assert!(
                set(
                    &conn,
                    SettingKey::AiThreads,
                    &SettingValue::Number(nucleos as f64)
                )
                .is_err(),
                "todos los núcleos dejarían sin sitio al hilo que pinta"
            );
        }
        for fuera in [-1.0, 1.5, f64::NAN, 10_000.0] {
            assert!(set(&conn, SettingKey::AiThreads, &SettingValue::Number(fuera)).is_err());
        }

        for admitido in MEMORIAS_DE_IA_ADMITIDAS {
            set(
                &conn,
                SettingKey::AiMemoryLimitMb,
                &SettingValue::Number(*admitido),
            )
            .expect("un tope de la lista se guarda");
        }
        for fuera in [0.0, 700.0, 1_000_000.0] {
            assert!(set(
                &conn,
                SettingKey::AiMemoryLimitMb,
                &SettingValue::Number(fuera)
            )
            .is_err());
        }
    }

    /// El tope de lo derivado solo admite los tres que se ofrecen: un número
    /// cualquiera escrito desde el webview no puede abrir el disco entero.
    #[test]
    fn el_tope_de_lo_derivado_solo_admite_los_que_se_ofrecen() {
        let db = base();
        let conn = db.connection().expect("conexión");

        for admitido in PRESUPUESTOS_ADMITIDOS {
            set(
                &conn,
                SettingKey::DerivedBudget,
                &SettingValue::Number(*admitido),
            )
            .expect("un tope de la lista se guarda");
        }
        for fuera in [0.0, 25.0, 100.0, 400.0, f64::NAN] {
            assert!(
                matches!(
                    set(
                        &conn,
                        SettingKey::DerivedBudget,
                        &SettingValue::Number(fuera)
                    ),
                    Err(NbError::Internal)
                ),
                "{fuera} no es un tope admitido"
            );
        }
    }

    /// Escribir el mismo valor dos veces no falla ni duplica: es lo que ocurre
    /// cuando la interfaz reenvía el estado al montar.
    #[test]
    fn escribir_dos_veces_la_misma_preferencia_es_idempotente() {
        let db = base();
        let conn = db.connection().expect("conexión");

        set(
            &conn,
            SettingKey::Theme,
            &SettingValue::Text("dark".to_owned()),
        )
        .expect("primera");
        set(
            &conn,
            SettingKey::Theme,
            &SettingValue::Text("dark".to_owned()),
        )
        .expect("segunda");

        let filas: i64 = conn
            .query_row("SELECT count(*) FROM settings", [], |r| r.get(0))
            .expect("consulta");
        assert_eq!(filas, 1);
    }

    #[test]
    fn devolver_una_preferencia_a_su_valor_por_defecto_la_borra() {
        let db = base();
        let conn = db.connection().expect("conexión");
        set(&conn, SettingKey::FontScale, &SettingValue::Number(2.0)).expect("escritura");

        reset(&conn, SettingKey::FontScale).expect("reset");

        assert_eq!(
            get(&conn, SettingKey::FontScale).expect("lectura"),
            SettingValue::Number(1.0)
        );
    }

    /// Una preferencia corrupta no puede impedir que alguien lea sus libros. Se
    /// cae al valor por defecto y se avisa, en vez de fallar el arranque.
    #[test]
    fn una_preferencia_corrupta_degrada_al_valor_por_defecto() {
        let db = base();
        let conn = db.connection().expect("conexión");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('font_scale', '\"grande\"')",
            [],
        )
        .expect("escritura cruda");

        assert_eq!(
            get(&conn, SettingKey::FontScale).expect("lectura"),
            SettingValue::Number(1.0)
        );
    }

    /// La caché del webview es deliberadamente estrecha: exactamente los cinco
    /// valores de ADR-010, ni uno más. Cada valor añadido es superficie de
    /// divergencia entre la caché y la fuente de verdad.
    #[test]
    fn solo_cinco_preferencias_afectan_al_primer_pintado() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let del_arranque = first_paint(&conn).expect("primer pintado");
        let nombres: Vec<&str> = del_arranque.iter().map(|s| s.key.as_str()).collect();

        assert_eq!(
            del_arranque.len(),
            5,
            "la caché es de cinco valores: {nombres:?}"
        );
        for esperado in [
            "theme",
            "high_contrast",
            "reduced_motion",
            "font_scale",
            "language",
        ] {
            assert!(
                nombres.contains(&esperado),
                "falta {esperado} en {nombres:?}"
            );
        }
        assert!(!nombres.contains(&"ai_provider"));
    }

    #[test]
    fn la_instantanea_incluye_todas_las_preferencias_conocidas() {
        let db = base();
        let conn = db.connection().expect("conexión");

        let todas = snapshot(&conn).expect("instantánea");

        assert_eq!(todas.len(), TODAS.len());
        // Veintiuna del plano de la aplicación —el interruptor de red de
        // el tema, el acento, los dos del traductor, los
        // tres (la forma de la biblioteca, su tamaño de página y la
        // voz del sistema), los cuatro (de dónde sale la voz, y el
        // servidor, el modelo y la voz del servicio), el anfitrión del registro
        // de extensiones y el tema de extensión elegido—;
        // siete del plano del libro; seis más del modo de lectura y
        // el kit accesible; y tres de F7: el orden de la biblioteca,
        // las tarjetas plegadas de la barra lateral y el tope de lo derivado; y
        // uno: los hilos de la inferencia local. La memoria ya existía.
        // Y dos: dónde está el proveedor de IA externo y qué modelo. Y una: si
        // el servidor de NB Books Mobile escucha.
        assert_eq!(todas.len(), 45);
    }

    // --- Secretos -----------------------------------------------------------

    /// **Parte del criterio de aceptación:** ningún secreto llega a la base.
    ///
    /// La primera puerta es la enumeración cerrada: no existe forma de nombrar
    /// una clave arbitraria a través de la superficie pública.
    #[test]
    fn no_hay_forma_de_nombrar_una_preferencia_que_sea_un_secreto() {
        for intento in [
            "api_key",
            "openai_api_key",
            "access_token",
            "refresh_token",
            "client_secret",
            "password",
            "lms_credential",
            "passphrase",
        ] {
            assert!(
                SettingKey::from_name(intento).is_none(),
                "{intento} no puede ser una preferencia conocida"
            );
        }
    }

    /// La segunda puerta, la del SQL escrito a mano: la propia base lo rechaza.
    /// Sin esto, la regla dependería de que nadie escriba nunca un `INSERT`
    /// directo, que es exactamente la clase de garantía que este proyecto no
    /// acepta.
    #[test]
    fn la_base_rechaza_por_si_misma_cualquier_clave_que_parezca_un_secreto() {
        let db = base();
        let conn = db.connection().expect("conexión");

        for intento in [
            "api_key",
            "openai_api_key",
            "access_token",
            "client_secret",
            "user_password",
            "lms_credential",
            "passphrase",
        ] {
            let resultado = conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, '\"lo que sea\"')",
                [intento],
            );
            assert!(
                resultado.is_err(),
                "la base aceptó una clave con pinta de secreto: {intento}"
            );
        }

        let filas: i64 = conn
            .query_row("SELECT count(*) FROM settings", [], |r| r.get(0))
            .expect("consulta");
        assert_eq!(filas, 0, "algún secreto llegó a la base");
    }

    /// El proveedor de IA sí es una preferencia; su credencial no. La distinción
    /// tiene que poder expresarse, o la regla se vuelve inútil en la práctica.
    #[test]
    fn el_proveedor_de_ia_es_preferencia_pero_su_credencial_no() {
        let db = base();
        let conn = db.connection().expect("conexión");

        set(
            &conn,
            SettingKey::AiProvider,
            &SettingValue::Text("anthropic".to_owned()),
        )
        .expect("el proveedor es una preferencia legítima");
        // Y una lista cerrada: un valor inventado no llega a decidir a qué se habla.
        assert!(set(
            &conn,
            SettingKey::AiProvider,
            &SettingValue::Text("cualquiera".to_owned()),
        )
        .is_err());

        assert!(SettingKey::from_name("ai_provider_api_key").is_none());
    }

    // --- Serialización hacia el puente --------------------------------------

    /// El valor viaja sin envoltorio: el frontend recibe `true`, `1.35` o
    /// `"dark"`, no `{"Flag": true}`.
    #[test]
    fn el_valor_viaja_al_puente_sin_envoltorio() {
        let ejemplos = [
            (SettingValue::Flag(true), "true"),
            (SettingValue::Number(1.35), "1.35"),
            (SettingValue::Text("dark".to_owned()), r#""dark""#),
        ];

        for (valor, esperado) in ejemplos {
            let json = serde_json::to_string(&valor).expect("serializable");
            assert_eq!(json, esperado);
            assert_eq!(
                serde_json::from_str::<SettingValue>(&json).expect("legible"),
                valor
            );
        }
    }

    /// `untagged` prueba las variantes en orden, así que `true` no puede acabar
    /// interpretado como número. Si alguien reordena la enumeración, esto cae.
    #[test]
    fn un_booleano_no_se_lee_como_numero() {
        assert_eq!(
            serde_json::from_str::<SettingValue>("true").expect("legible"),
            SettingValue::Flag(true)
        );
        assert_eq!(
            serde_json::from_str::<SettingValue>("0").expect("legible"),
            SettingValue::Number(0.0)
        );
    }

    /// **El modelo generativo es una lista cerrada: la del catálogo.** Vacío vale, y
    /// significa "el que se recomienda para este equipo"; un identificador inventado
    /// desde el webview, o el del modelo de embeddings, no.
    #[test]
    fn solo_se_admite_un_modelo_generativo_del_catalogo() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let poner = |valor: &str| {
            set(
                &conn,
                SettingKey::AiModelLocal,
                &SettingValue::Text(valor.to_owned()),
            )
        };
        let del_catalogo = crate::ai::catalogo::generativos()
            .next()
            .expect("hay modelos generativos")
            .id;

        assert!(poner("").is_ok(), "vacío es el recomendado");
        assert!(poner(del_catalogo).is_ok());
        assert!(poner("modelo-inventado").is_err());
        assert!(
            poner(crate::ai::catalogo::GRANITE_107M_MULTILINGUAL.id).is_err(),
            "el de embeddings no genera"
        );
    }

    /// **De dónde sale la voz es una lista cerrada de tres.** Escribir otra cosa
    /// desde el webview dejaría la lectura en alto sin proveedor y sin motivo.
    #[test]
    fn solo_se_admiten_los_tres_origenes_de_voz() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let poner = |valor: &str| {
            set(
                &conn,
                SettingKey::VoiceProvider,
                &SettingValue::Text(valor.to_owned()),
            )
        };

        for admitido in ORIGENES_DE_VOZ_ADMITIDOS {
            assert!(poner(admitido).is_ok(), "{admitido} se admite");
        }
        assert!(poner("").is_err(), "vacío no es un origen");
        assert!(
            poner("kokoro").is_err(),
            "el nombre del modelo no es un origen"
        );
    }

    #[test]
    fn la_instantanea_se_serializa_en_camel_case() {
        let db = base();
        let conn = db.connection().expect("conexión");
        let json = serde_json::to_string(&first_paint(&conn).expect("primer pintado"))
            .expect("serializable");

        assert!(json.contains("affectsFirstPaint"), "json: {json}");
        assert!(!json.contains("affects_first_paint"));
    }
}
