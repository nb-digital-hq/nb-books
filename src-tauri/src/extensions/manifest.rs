//! El manifiesto de una extensión.
//!
//! Existe para que la aplicación sepa **qué aporta una extensión sin ejecutar
//! ni una instrucción suya**: qué comandos pone en la interfaz, qué permisos
//! pide y contra qué versión de la API se compiló. De ahí sale la activación
//! perezosa, y de ahí sale que se le pueda enseñar al usuario lo que
//! va a instalar antes de instalarlo.
//!
//! # Una sola fuente para el validador y para el esquema
//!
//! El esquema publicado en `docs/architecture/api/nb-extension.schema.json` **no se escribe
//! a mano**: lo genera [`esquema`] desde la misma tabla [`CAMPOS`] que usa el
//! validador. Si fueran dos textos distintos, llegaría el día en que el esquema
//! promete una cosa y el anfitrión acepta otra, y quien lo sufriría es el autor
//! de una extensión que valida bien en su máquina y se rechaza aquí.
//!
//! Queda una costura: la expresión regular que viaja al esquema y la función
//! que valida en Rust son dos escrituras de la misma regla, porque el proyecto
//! no incorpora un motor de expresiones regulares —cabe en cien líneas, y ese
//! es el listón de `dependency-policy.md`—. Esa costura la vigila
//! `scripts/check-esquema.mjs`, que ejecuta la expresión del esquema en Node
//! sobre los mismos ejemplos que la prueba de Rust ejecuta contra la función, y
//! falla si no dicen lo mismo.
//!
//! # Por qué se informan todos los defectos y no el primero
//!
//! Un autor que arregla un campo y vuelve a fallar por el siguiente hace tantas
//! vueltas como defectos tenga. El validador recorre el manifiesto entero y
//! devuelve la lista, cada uno con **el campo concreto** en notación de puntos.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::wasm;

/// Cómo se llama el manifiesto dentro del paquete de una extensión.
pub const ARCHIVO: &str = "nb-extension.json";

// ---------------------------------------------------------------------------
// El tipo
// ---------------------------------------------------------------------------

/// A qué clase pertenece una extensión.
///
/// Son las tres de la sección 1 de `plugin-system.md`, y la lista es cerrada a
/// propósito: cada clase trae consigo qué se le permite, y una clase nueva es
/// una decisión de arquitectura, no un valor más.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum ExtensionKind {
    /// Lógica de fondo compilada a WebAssembly.
    Core,
    /// Contribuciones a la interfaz.
    Ui,
    /// Solo variables y recursos gráficos, sin código ejecutable.
    Theme,
}

/// Lo que una extensión puede pedir hacer.
///
/// **La lista tiene un solo elemento, y eso es deliberado.** Una capacidad que
/// figure aquí es una promesa: alguien la declarará en su manifiesto y esperará
/// que funcione. Solo entra lo que el anfitrión sabe conceder hoy. Ver la
/// sección 2.9 de `versioning.md`: añadir una es un cambio menor, quitarla es
/// mayor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum Capability {
    /// Salir a la red, por los anfitriones que se declaren y solo por ellos.
    #[serde(rename = "network:fetch")]
    NetworkFetch,
}

impl Capability {
    /// El nombre con el que viaja en el manifiesto.
    pub const fn nombre(self) -> &'static str {
        match self {
            Self::NetworkFetch => "network:fetch",
        }
    }

    /// Qué implica conceder esto, en lenguaje llano.
    ///
    /// Vive aquí y no en la interfaz por el mismo motivo por el que
    /// `net::Purpose::what_is_sent` vive junto al propósito: es lo que hay que
    /// enseñar antes de pedir el consentimiento, y tenerlo pegado a la
    /// capacidad impide que la explicación se quede vieja cuando la capacidad
    /// cambie.
    pub const fn en_lenguaje_llano(self) -> &'static str {
        match self {
            Self::NetworkFetch => {
                "Podrá enviar y recibir datos de los servidores que se listan. \
                 Su código no es nuestro, así que no podemos acotar qué envía."
            }
        }
    }
}

/// Todas las capacidades que existen, en orden estable.
pub const CAPACIDADES: &[Capability] = &[Capability::NetworkFetch];

/// El detalle de un permiso de red.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct NetworkPermission {
    /// Los anfitriones a los que se puede salir. El comodín solo vale en la
    /// ruta: ver [`es_url_de_permiso`].
    pub urls: Vec<String>,
}

/// Lo que la extensión pide, con su detalle.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Permissions {
    #[serde(rename = "network:fetch", skip_serializing_if = "Option::is_none")]
    pub network_fetch: Option<NetworkPermission>,
}

/// Un comando que la extensión pone en la paleta o en un menú.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Command {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
}

/// Los ambientes que el núcleo sabe sintetizar.
///
/// Cerrada a propósito, y es lo que hace que un fondo sonoro **no pese**: lo que
/// una extensión aporta es el nombre y qué ambiente, no el sonido. Con un
/// archivo, cada paquete traería megabytes y una licencia que revisar.
///
/// Los tres primeros son ruido puro. Los demás son **recetas**: ruido de base,
/// capas de filtro con su vaivén y granos horneados —gotas, chispas,
/// traqueteo—. La receta de cada uno vive en `src/lib/ui/lector/fondoSonoro.ts`
/// y hay una prueba que exige que esta lista y aquel catálogo coincidan: un
/// ambiente que se admita aquí y no sepa sonar allí sería una opción que falla
/// al pulsarla.
pub const CLASES_DE_RUIDO: &[&str] = &[
    "blanco",
    "rosa",
    "marron",
    "lluvia",
    "tormenta",
    "oleaje",
    "viento",
    "fuego",
    "bosque",
    "cafeteria",
    "ventilador",
    "tren",
];

/// Un formato de cita que aporta una extensión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct CitationFormatContribution {
    pub id: String,
    /// El nombre que se ve. Lo pone su autor y **no se traduce**.
    pub name: String,
    /// La extensión de archivo, sin el punto: `bib`, `ris`.
    ///
    /// La pone la extensión porque es parte del formato, y se acota a letras y
    /// dígitos: es lo que va a acabar en el nombre de un archivo del usuario.
    pub file_extension: String,
}

/// Un fondo sonoro que aporta una extensión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct SoundscapeContribution {
    pub id: String,
    /// El nombre que se ve. Lo pone su autor y **no se traduce**.
    pub name: String,
    /// Cuál de los ambientes. Ver [`CLASES_DE_RUIDO`].
    pub kind: String,
}

/// Un modo de lectura que aporta una extensión.
///
/// **Es un perfil completo, no una paleta.** Fija fondo, texto, brillo y
/// temperatura a la vez, igual que los seis del núcleo: un modo con dos valores
/// sueltos obligaría a componer el resto a mano, que es la fricción disfrazada
/// de libertad que ADR-012 declara como modo de fallo de esta sección.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ReadingModeContribution {
    /// Identificador dentro de la extensión. Lo que se guarda en las
    /// preferencias es `<extensión>/<id>`, para que dos paquetes puedan llamar
    /// igual a su modo sin pisarse.
    pub id: String,
    /// El nombre que se ve. Lo pone su autor y **no se traduce**.
    pub name: String,
    /// Color de fondo de la superficie de lectura.
    pub background: String,
    /// Color del texto.
    pub text: String,
    /// Brillo emulado, en porcentaje de 50 a 100.
    ///
    /// **Emulado**: es una capa de composición encima, no el brillo del panel,
    /// y la interfaz lo nombra así. En porcentaje entero y no en fracción
    /// porque un manifiesto se escribe a mano: `95` no admite las dos lecturas
    /// que admite `0.95`, y además no puede traer un `NaN`.
    #[serde(default = "cien")]
    pub brightness: u8,
    /// Temperatura emulada, de 0 (neutra) a 100 (muy cálida). Filtra el azul
    /// componiendo encima, que es lo único que un webview puede hacer.
    #[serde(default)]
    pub warmth: u8,
    /// Si el modo quita el color del contenido, no solo del fondo.
    #[serde(default)]
    pub monochrome: bool,
}

const fn cien() -> u8 {
    100
}

/// Una vista que una extensión aporta a Estudio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ViewContribution {
    pub id: String,
    /// El título de la pestaña. Lo pone su autor y **no se traduce**: es suyo,
    /// igual que el nombre de la extensión.
    pub title: String,
}

/// Lo que la extensión aporta a la aplicación, de forma declarativa.
///
/// Es lo que permite poblar la interfaz **antes** de cargar el código, que es
/// de lo que vive la activación perezosa. Los puntos de contribución
/// se implementan; aquí se fija su forma, que es lo que el
/// esquema publica.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Contributes {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,
    /// De cada sitio de la interfaz, qué comandos aparecen ahí.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context_menus: BTreeMap<String, Vec<String>>,
    /// De cada clase de proveedor, qué módulo la implementa.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, String>,
    /// Los colores que aporta, si es un tema.
    ///
    /// Solo colores, y solo de una lista cerrada: ver [`TOKENS_DE_TEMA`]. Un
    /// tema no puede apagar el foco porque **no existe un token para
    /// apagarlo**; lo más que puede hacer es darle un color, y ese color pasa
    /// por la comprobación de contraste antes de aplicarse.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub theme: BTreeMap<String, String>,
    /// Modos de lectura que aporta.
    ///
    /// Un modo es un **perfil completo** y no una paleta: fija fondo, texto,
    /// brillo y temperatura a la vez, igual que los seis del núcleo. Uno con
    /// dos valores sueltos obligaría a componer el resto a mano, que es la
    /// fricción disfrazada de libertad que ADR-012 declara como modo de fallo.
    ///
    /// Como los temas, **no traen código**: van en un paquete de clase `theme`,
    /// que no puede declarar permisos ni proveedores.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reading_modes: Vec<ReadingModeContribution>,
    /// Formatos de cita que aporta.
    ///
    /// El módulo que los implementa se declara en `providers` con la clave
    /// `citation`, igual que los demás proveedores: aquí solo va **qué**
    /// formatos ofrece, que es lo que la interfaz necesita para ofrecerlos sin
    /// compilar nada.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citation_formats: Vec<CitationFormatContribution>,
    /// Fondos sonoros que aporta.
    ///
    /// **Ni un byte de audio**: se declara qué clase de ruido es y el núcleo lo
    /// sintetiza. Es lo que permite que la extensión no añada peso al
    /// instalador de quien no la quiere, que es el criterio.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub soundscapes: Vec<SoundscapeContribution>,
    /// Vistas que aporta a Estudio.
    ///
    /// Estudio y no cualquier sitio: es la ranura que `ui-components.md`
    /// reserva, con el mismo aspecto que las nativas. Las otras dos
    /// —panel lateral y vista del área principal— llegarán cuando haya una
    /// razón concreta, no por simetría.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<ViewContribution>,
}

/// El manifiesto, ya validado.
///
/// Que exista un valor de este tipo significa que pasó [`parse`]: no hay forma
/// de construir uno saltándose la validación desde fuera del módulo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub kind: ExtensionKind,
    pub api_version: String,
    pub license: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default)]
    pub contributes: Contributes,
}

impl Manifest {
    /// La mayor de la versión de API declarada en el manifiesto.
    pub fn mayor_de_api(&self) -> Option<u64> {
        self.api_version.split('.').next()?.parse().ok()
    }
}

// ---------------------------------------------------------------------------
// Los defectos
// ---------------------------------------------------------------------------

/// Qué le pasa a un campo. Código canónico y estable, como los de `error.rs`.
///
/// Nunca se enseña tal cual: la interfaz lo traduce, igual que hace con los
/// códigos de error. Aquí no viaja ningún mensaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum DefectCode {
    /// El archivo no es JSON.
    NotJson,
    /// El documento no es un objeto.
    NotAnObject,
    /// Falta un campo obligatorio.
    MissingField,
    /// El campo existe pero es de otro tipo.
    WrongType,
    /// El valor no está entre los admitidos.
    ValueNotAllowed,
    /// El valor no tiene la forma que exige su campo.
    MalformedValue,
    /// Se declara una capacidad y no se detalla su permiso.
    CapabilityWithoutPermission,
    /// Se detalla un permiso que no se declaró como capacidad.
    PermissionWithoutCapability,
    /// Un anfitrión de permiso con comodín, que sería "toda la red".
    WildcardHost,
    /// El manifiesto y el módulo declaran versiones de API distintas.
    ApiVersionMismatch,
    /// Un tema declara código o permisos. Un tema es declarativo y nada más.
    ThemeWithCode,
    /// Un tema cambia un color que no está en la lista cerrada.
    UnknownThemeToken,
}

/// Un defecto concreto, en un campo concreto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct ManifestDefect {
    pub code: DefectCode,
    /// El campo, en notación de puntos: `permissions.network:fetch.urls[0]`.
    /// Vacío cuando el defecto es del documento entero.
    pub field: String,
    /// Qué se esperaba, cuando decirlo ayuda a arreglarlo. Es un **valor**, no
    /// un mensaje: el patrón admitido, la lista de valores, la versión que
    /// esperaba el anfitrión. La frase la compone la interfaz.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

impl ManifestDefect {
    fn nuevo(code: DefectCode, field: impl Into<String>) -> Self {
        Self {
            code,
            field: field.into(),
            expected: None,
        }
    }

    fn con_esperado(
        code: DefectCode,
        field: impl Into<String>,
        esperado: impl Into<String>,
    ) -> Self {
        Self {
            code,
            field: field.into(),
            expected: Some(esperado.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// La tabla que gobierna el validador y el esquema
// ---------------------------------------------------------------------------

/// Qué forma tiene que tener un campo.
pub enum Forma {
    /// Texto no vacío, sin más reglas.
    Texto,
    /// Texto que casa con un patrón. `patron` es lo que viaja al esquema;
    /// `comprueba` es lo que ejecuta el anfitrión. Que digan lo mismo lo
    /// vigila `scripts/check-esquema.mjs`.
    TextoConPatron {
        patron: &'static str,
        comprueba: fn(&str) -> bool,
    },
    /// Uno de un conjunto cerrado.
    Enumerado(&'static [&'static str]),
    /// Lista de valores de un conjunto cerrado.
    ListaEnumerada(&'static [&'static str]),
    /// Objeto con reglas propias, comprobadas aparte.
    Objeto,
}

/// Un campo del manifiesto.
pub struct Campo {
    pub nombre: &'static str,
    pub obligatorio: bool,
    pub descripcion: &'static str,
    pub forma: Forma,
    /// Ejemplos que **tienen** que aceptarse.
    pub aceptados: &'static [&'static str],
    /// Ejemplos que **tienen** que rechazarse.
    pub rechazados: &'static [&'static str],
}

/// Los colores que un tema puede cambiar. Lista cerrada.
///
/// Cerrada por dos motivos, y el segundo es el que importa. El primero es que
/// solo se puede prometer estabilidad sobre una superficie acotada, igual que
/// con los puntos de contribución. El segundo es que **aquí no hay ningún token
/// que apague el contorno del foco ni que quite una sombra de fondo**: lo que
/// no está en esta lista no se puede tocar, así que las garantías de
/// accesibilidad de `app.css` no dependen de que el autor de un tema se acuerde.
pub const TOKENS_DE_TEMA: &[&str] = &[
    "--nb-accent",
    "--nb-accent-soft",
    "--nb-border",
    "--nb-border-strong",
    "--nb-danger",
    "--nb-focus",
    "--nb-on-accent",
    "--nb-on-danger",
    "--nb-surface",
    "--nb-surface-raised",
    "--nb-surface-sunken",
    "--nb-text",
    "--nb-text-muted",
    "--nb-text-subtle",
];

/// Un color en hexadecimal de seis dígitos, con almohadilla.
///
/// Seis y no tres ni ocho: tres es ambiguo al mezclar y ocho trae
/// transparencia, y un token de tema translúcido rompe el contraste sin que se
/// vea en el valor.
#[must_use]
pub fn es_color(valor: &str) -> bool {
    let Some(digitos) = valor.strip_prefix('#') else {
        return false;
    };
    digitos.len() == 6 && digitos.chars().all(|c| c.is_ascii_hexdigit())
}

/// Los valores admitidos en `type`.
const CLASES: &[&str] = &["core", "ui", "theme"];

/// Los nombres de las capacidades admitidas.
const NOMBRES_DE_CAPACIDAD: &[&str] = &["network:fetch"];

/// El manifiesto entero, campo por campo.
///
/// Añadir un campo aquí lo añade a la vez al validador, al esquema publicado y
/// a los ejemplos. Es el único sitio donde se declara.
pub const CAMPOS: &[Campo] = &[
    Campo {
        nombre: "id",
        obligatorio: true,
        descripcion: "Identificador único, en forma de dominio invertido.",
        forma: Forma::TextoConPatron {
            patron: r"^[a-z][a-z0-9-]*(\.[a-z][a-z0-9-]*){2,}$",
            comprueba: es_identificador,
        },
        aceptados: &[
            "lat.nbdigitallabs.books.metadata.openlibrary",
            "com.ejemplo.mi-tema",
        ],
        rechazados: &[
            "",
            "openlibrary",
            "lat.nbdigitallabs",
            "Lat.NbDigitalLabs.X",
            "org..x",
            "1org.x.y",
        ],
    },
    Campo {
        nombre: "name",
        obligatorio: true,
        descripcion: "Nombre legible, el que ve quien la instala.",
        forma: Forma::Texto,
        aceptados: &["Open Library Metadata Provider"],
        rechazados: &[""],
    },
    Campo {
        nombre: "version",
        obligatorio: true,
        descripcion: "Versión de la extensión, en SemVer completo.",
        forma: Forma::TextoConPatron {
            patron: r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$",
            comprueba: es_version,
        },
        aceptados: &["1.0.0", "0.1.0", "10.20.30"],
        rechazados: &["1.0", "1", "01.0.0", "1.0.0-beta", "v1.0.0", ""],
    },
    Campo {
        nombre: "type",
        obligatorio: true,
        descripcion: "Clase de extensión.",
        forma: Forma::Enumerado(CLASES),
        aceptados: &["core", "ui", "theme"],
        rechazados: &["nucleo", "Core", "plugin", ""],
    },
    Campo {
        nombre: "apiVersion",
        obligatorio: true,
        descripcion: "Versión de la API de extensiones contra la que se compiló.",
        forma: Forma::TextoConPatron {
            patron: r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$",
            comprueba: es_version_de_api,
        },
        aceptados: &["0.1", "1.0", "99.0"],
        rechazados: &["1", "1.0.0", "01.0", "", "uno.cero"],
    },
    // Política del registro, **R6**: la licencia no es opcional y el campo
    // vacío no vale. Que el texto viaje además dentro del paquete lo comprueba
    // la puerta de revisión, no el manifiesto: aquí solo hay JSON.
    Campo {
        nombre: "license",
        obligatorio: true,
        descripcion: "Licencia declarada, con su identificador SPDX.",
        forma: Forma::Texto,
        aceptados: &["Apache-2.0", "MIT"],
        rechazados: &[""],
    },
    Campo {
        nombre: "capabilities",
        obligatorio: false,
        descripcion: "Lo que la extensión pide hacer.",
        forma: Forma::ListaEnumerada(NOMBRES_DE_CAPACIDAD),
        aceptados: &["network:fetch"],
        rechazados: &["network", "filesystem:read", "network:*"],
    },
    Campo {
        nombre: "permissions",
        obligatorio: false,
        descripcion: "El detalle de cada capacidad declarada.",
        forma: Forma::Objeto,
        aceptados: &[],
        rechazados: &[],
    },
    Campo {
        nombre: "contributes",
        obligatorio: false,
        descripcion: "Lo que aporta a la interfaz, de forma declarativa.",
        forma: Forma::Objeto,
        aceptados: &[],
        rechazados: &[],
    },
];

// ---------------------------------------------------------------------------
// Las reglas de forma
// ---------------------------------------------------------------------------

/// Dominio invertido de al menos tres segmentos, en minúsculas.
///
/// Tres y no dos porque el identificador es la clave con la que la extensión
/// vive en el almacén y aparece en rutas: `com.ejemplo` es de quien
/// tenga el dominio, y no dice de qué extensión suya se habla.
fn es_identificador(valor: &str) -> bool {
    let mut segmentos = 0;
    for segmento in valor.split('.') {
        segmentos += 1;
        let mut caracteres = segmento.chars();
        if !matches!(caracteres.next(), Some(c) if c.is_ascii_lowercase()) {
            return false;
        }
        if !caracteres.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return false;
        }
    }
    segmentos >= 3
}

/// Tres números sin ceros a la izquierda.
fn es_version(valor: &str) -> bool {
    numeros(valor, 3)
}

/// Dos números: la mayor y la menor. El parche no aparece porque no cambia lo
/// que un módulo puede pedir, por la tabla de la sección 2.5 de `versioning.md`.
fn es_version_de_api(valor: &str) -> bool {
    numeros(valor, 2)
}

fn numeros(valor: &str, cuantos: usize) -> bool {
    let partes: Vec<&str> = valor.split('.').collect();
    partes.len() == cuantos
        && partes.iter().all(|parte| {
            !parte.is_empty()
                && parte.chars().all(|c| c.is_ascii_digit())
                && (parte.len() == 1 || !parte.starts_with('0'))
        })
}

/// Una dirección de permiso de red.
///
/// **El comodín no puede estar en el anfitrión.** El permiso se concede por
/// dominio, así que `https://*/*` sería "toda la red" disfrazada de permiso
/// concreto, y quien lo aprobara no estaría aprobando lo que cree.
pub fn es_url_de_permiso(valor: &str) -> bool {
    let Some(resto) = valor.strip_prefix("https://") else {
        return false;
    };
    let anfitrion = resto.split('/').next().unwrap_or_default();
    !anfitrion.is_empty() && !anfitrion.contains('*') && anfitrion.contains('.')
}

// ---------------------------------------------------------------------------
// El validador
// ---------------------------------------------------------------------------

/// Valida un manifiesto y devuelve **todos** sus defectos, no el primero.
///
/// Es el mecanismo del requisito **R1** de la política del registro, y en su
/// tabla de campos vive también el de **R6**, la licencia declarada. Ver
/// `docs/legal/governance/extension-policy.md`.
pub fn parse(json: &str) -> Result<Manifest, Vec<ManifestDefect>> {
    let documento: serde_json::Value = match serde_json::from_str(json) {
        Ok(valor) => valor,
        Err(_) => return Err(vec![ManifestDefect::nuevo(DefectCode::NotJson, "")]),
    };
    let Some(objeto) = documento.as_object() else {
        return Err(vec![ManifestDefect::nuevo(DefectCode::NotAnObject, "")]);
    };

    let mut defectos = Vec::new();
    for campo in CAMPOS {
        revisar_campo(campo, objeto, &mut defectos);
    }
    revisar_permisos(objeto, &mut defectos);
    revisar_tema(objeto, &mut defectos);
    revisar_sonidos(objeto, &mut defectos);
    revisar_citas(objeto, &mut defectos);

    if !defectos.is_empty() {
        return Err(defectos);
    }

    // Con la forma comprobada, esto no puede fallar; si fallara sería un
    // defecto del propio validador y no del manifiesto, así que se dice como
    // tal en lugar de callarlo.
    serde_json::from_value(documento)
        .map_err(|_| vec![ManifestDefect::nuevo(DefectCode::WrongType, "")])
}

fn revisar_campo(
    campo: &Campo,
    objeto: &serde_json::Map<String, serde_json::Value>,
    defectos: &mut Vec<ManifestDefect>,
) {
    let Some(valor) = objeto.get(campo.nombre) else {
        if campo.obligatorio {
            defectos.push(ManifestDefect::nuevo(
                DefectCode::MissingField,
                campo.nombre,
            ));
        }
        return;
    };

    match &campo.forma {
        Forma::Texto => match valor.as_str() {
            Some(texto) if !texto.is_empty() => {}
            Some(_) => defectos.push(ManifestDefect::nuevo(
                DefectCode::MalformedValue,
                campo.nombre,
            )),
            None => defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                campo.nombre,
                "string",
            )),
        },
        Forma::TextoConPatron { patron, comprueba } => match valor.as_str() {
            Some(texto) if comprueba(texto) => {}
            Some(_) => defectos.push(ManifestDefect::con_esperado(
                DefectCode::MalformedValue,
                campo.nombre,
                *patron,
            )),
            None => defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                campo.nombre,
                "string",
            )),
        },
        Forma::Enumerado(admitidos) => match valor.as_str() {
            Some(texto) if admitidos.contains(&texto) => {}
            Some(_) => defectos.push(ManifestDefect::con_esperado(
                DefectCode::ValueNotAllowed,
                campo.nombre,
                admitidos.join(", "),
            )),
            None => defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                campo.nombre,
                "string",
            )),
        },
        Forma::ListaEnumerada(admitidos) => {
            let Some(lista) = valor.as_array() else {
                defectos.push(ManifestDefect::con_esperado(
                    DefectCode::WrongType,
                    campo.nombre,
                    "array",
                ));
                return;
            };
            for (indice, elemento) in lista.iter().enumerate() {
                let sitio = format!("{}[{indice}]", campo.nombre);
                match elemento.as_str() {
                    Some(texto) if admitidos.contains(&texto) => {}
                    Some(_) => defectos.push(ManifestDefect::con_esperado(
                        DefectCode::ValueNotAllowed,
                        sitio,
                        admitidos.join(", "),
                    )),
                    None => defectos.push(ManifestDefect::con_esperado(
                        DefectCode::WrongType,
                        sitio,
                        "string",
                    )),
                }
            }
        }
        Forma::Objeto => {
            if !valor.is_object() {
                defectos.push(ManifestDefect::con_esperado(
                    DefectCode::WrongType,
                    campo.nombre,
                    "object",
                ));
            }
        }
    }
}

/// Las dos mitades de un permiso tienen que estar las dos.
///
/// Declarar una capacidad sin detallarla deja al usuario aprobando algo sin
/// alcance; detallar un permiso sin declararlo lo deja fuera de la lista que se
/// le enseña antes de instalar. Las dos son la misma clase de agujero.
fn revisar_permisos(
    objeto: &serde_json::Map<String, serde_json::Value>,
    defectos: &mut Vec<ManifestDefect>,
) {
    let declaradas: Vec<&str> = objeto
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .map(|lista| lista.iter().filter_map(serde_json::Value::as_str).collect())
        .unwrap_or_default();
    let permisos = objeto
        .get("permissions")
        .and_then(serde_json::Value::as_object);

    for nombre in NOMBRES_DE_CAPACIDAD {
        let se_declara = declaradas.contains(nombre);
        let se_detalla = permisos.is_some_and(|p| p.contains_key(*nombre));
        if se_declara && !se_detalla {
            defectos.push(ManifestDefect::nuevo(
                DefectCode::CapabilityWithoutPermission,
                format!("permissions.{nombre}"),
            ));
        }
        if se_detalla && !se_declara {
            defectos.push(ManifestDefect::nuevo(
                DefectCode::PermissionWithoutCapability,
                "capabilities",
            ));
        }
    }

    let Some(red) = permisos
        .and_then(|p| p.get("network:fetch"))
        .and_then(serde_json::Value::as_object)
        .and_then(|p| p.get("urls"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for (indice, entrada) in red.iter().enumerate() {
        let sitio = format!("permissions.network:fetch.urls[{indice}]");
        match entrada.as_str() {
            Some(texto) if es_url_de_permiso(texto) => {}
            Some(texto) if texto.starts_with("https://") => {
                defectos.push(ManifestDefect::nuevo(DefectCode::WildcardHost, sitio))
            }
            Some(_) => defectos.push(ManifestDefect::con_esperado(
                DefectCode::MalformedValue,
                sitio,
                "https://anfitrion/ruta",
            )),
            None => defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                sitio,
                "string",
            )),
        }
    }
}

/// Brillo mínimo que admite un modo: por debajo, el texto deja de leerse por
/// mucho que su contraste declarado sea correcto.
const BRILLO_MINIMO: u64 = 50;

/// Los modos de lectura de un paquete de personalización.
///
/// Se comprueba la **forma**: que los valores estén en su rango y que los
/// colores sean hexadecimales de seis dígitos. El **contraste** entre fondo y
/// texto lo comprueba el frontend antes de aplicarlo, con la misma aritmética y
/// los mismos umbrales que usa el sistema de diseño para sí mismo. Es la misma
/// decisión que con los temas, y por el mismo motivo: dos fuentes para un
/// número que tiene que ser uno acaban separándose.
fn revisar_modos(
    objeto: &serde_json::Map<String, serde_json::Value>,
    defectos: &mut Vec<ManifestDefect>,
) {
    let Some(modos) = objeto
        .get("contributes")
        .and_then(serde_json::Value::as_object)
        .and_then(|c| c.get("readingModes"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };

    for (indice, modo) in modos.iter().enumerate() {
        let sitio = |campo: &str| format!("contributes.readingModes[{indice}].{campo}");
        let Some(modo) = modo.as_object() else {
            defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                format!("contributes.readingModes[{indice}]"),
                "object",
            ));
            continue;
        };

        for campo in ["id", "name"] {
            match modo.get(campo).and_then(serde_json::Value::as_str) {
                Some(valor) if !valor.trim().is_empty() => {}
                _ => defectos.push(ManifestDefect::con_esperado(
                    DefectCode::MissingField,
                    sitio(campo),
                    "string",
                )),
            }
        }
        for campo in ["background", "text"] {
            match modo.get(campo).and_then(serde_json::Value::as_str) {
                Some(valor) if es_color(valor) => {}
                _ => defectos.push(ManifestDefect::con_esperado(
                    DefectCode::MalformedValue,
                    sitio(campo),
                    "#rrggbb",
                )),
            }
        }
        for (campo, minimo) in [("brightness", BRILLO_MINIMO), ("warmth", 0)] {
            let Some(valor) = modo.get(campo) else {
                continue;
            };
            match valor.as_u64() {
                Some(numero) if (minimo..=100).contains(&numero) => {}
                _ => defectos.push(ManifestDefect::con_esperado(
                    DefectCode::ValueNotAllowed,
                    sitio(campo),
                    format!("{minimo}..100"),
                )),
            }
        }
        if let Some(valor) = modo.get("monochrome") {
            if !valor.is_boolean() {
                defectos.push(ManifestDefect::con_esperado(
                    DefectCode::WrongType,
                    sitio("monochrome"),
                    "boolean",
                ));
            }
        }
    }
}

/// Los formatos de cita que declara un manifiesto.
///
/// Se comprueba además que declare el **módulo** que los implementa: unos
/// formatos sin proveedor aparecerían en la interfaz y fallarían al pulsarlos,
/// que es la peor forma de fallar.
fn revisar_citas(
    objeto: &serde_json::Map<String, serde_json::Value>,
    defectos: &mut Vec<ManifestDefect>,
) {
    let contribuciones = objeto
        .get("contributes")
        .and_then(serde_json::Value::as_object);
    let Some(formatos) = contribuciones
        .and_then(|c| c.get("citationFormats"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    if formatos.is_empty() {
        return;
    }

    let hay_modulo = contribuciones
        .and_then(|c| c.get("providers"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|p| p.contains_key(super::citation::CLAVE));
    if !hay_modulo {
        defectos.push(ManifestDefect::con_esperado(
            DefectCode::MissingField,
            format!("contributes.providers.{}", super::citation::CLAVE),
            "string",
        ));
    }

    for (indice, formato) in formatos.iter().enumerate() {
        let sitio = |campo: &str| format!("contributes.citationFormats[{indice}].{campo}");
        let Some(formato) = formato.as_object() else {
            defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                format!("contributes.citationFormats[{indice}]"),
                "object",
            ));
            continue;
        };
        for campo in ["id", "name"] {
            match formato.get(campo).and_then(serde_json::Value::as_str) {
                Some(valor) if !valor.trim().is_empty() => {}
                _ => defectos.push(ManifestDefect::con_esperado(
                    DefectCode::MissingField,
                    sitio(campo),
                    "string",
                )),
            }
        }
        // La extensión de archivo acaba en el nombre de un archivo del usuario,
        // así que se acota a letras y dígitos: un punto o una barra ahí serían
        // una ruta, no una extensión.
        match formato
            .get("fileExtension")
            .and_then(serde_json::Value::as_str)
        {
            Some(valor)
                if !valor.is_empty()
                    && valor.len() <= 8
                    && valor.chars().all(|c| c.is_ascii_alphanumeric()) => {}
            _ => defectos.push(ManifestDefect::con_esperado(
                DefectCode::MalformedValue,
                sitio("fileExtension"),
                "letras y dígitos, hasta ocho",
            )),
        }
    }
}

/// Los fondos sonoros que declara un manifiesto.
///
/// Se comprueban **fuera** de `revisar_tema`, al revés que los modos de
/// lectura, y la diferencia tiene motivo: un modo decide con qué colores se lee
/// —si lo aportara una extensión con permisos, algo con red decidiría la
/// legibilidad del texto—, y un fondo sonoro no decide nada. Lo único que puede
/// hacer es sonar, y solo cuando alguien lo pone.
fn revisar_sonidos(
    objeto: &serde_json::Map<String, serde_json::Value>,
    defectos: &mut Vec<ManifestDefect>,
) {
    let Some(sonidos) = objeto
        .get("contributes")
        .and_then(serde_json::Value::as_object)
        .and_then(|c| c.get("soundscapes"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };

    for (indice, sonido) in sonidos.iter().enumerate() {
        let sitio = |campo: &str| format!("contributes.soundscapes[{indice}].{campo}");
        let Some(sonido) = sonido.as_object() else {
            defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                format!("contributes.soundscapes[{indice}]"),
                "object",
            ));
            continue;
        };
        for campo in ["id", "name"] {
            match sonido.get(campo).and_then(serde_json::Value::as_str) {
                Some(valor) if !valor.trim().is_empty() => {}
                _ => defectos.push(ManifestDefect::con_esperado(
                    DefectCode::MissingField,
                    sitio(campo),
                    "string",
                )),
            }
        }
        match sonido.get("kind").and_then(serde_json::Value::as_str) {
            Some(valor) if CLASES_DE_RUIDO.contains(&valor) => {}
            _ => defectos.push(ManifestDefect::con_esperado(
                DefectCode::ValueNotAllowed,
                sitio("kind"),
                CLASES_DE_RUIDO.join(", "),
            )),
        }
    }
}

/// Un tema es declarativo, y nada más.
///
/// Mecanismo del requisito **R7** de la política del registro.
///
/// El criterio dice que un tema no puede pedir permisos de red ni de
/// disco. Aquí se va más lejos y tampoco puede traer **nada ejecutable**:
/// proveedores, órdenes ni vistas. Un tema que trae código no es un tema, y
/// dejarlo pasar obligaría a preguntarse en cada revisión si esta vez sí.
fn revisar_tema(
    objeto: &serde_json::Map<String, serde_json::Value>,
    defectos: &mut Vec<ManifestDefect>,
) {
    let es_tema = objeto.get("type").and_then(serde_json::Value::as_str) == Some("theme");
    let contribuciones = objeto
        .get("contributes")
        .and_then(serde_json::Value::as_object);
    let colores = contribuciones
        .and_then(|c| c.get("theme"))
        .and_then(serde_json::Value::as_object);

    if !es_tema {
        // Modos de lectura en algo que no se declara tema: un modo es
        // declarativo y va en un paquete que no puede traer código. Permitirlo
        // en una extensión de clase `core` dejaría que algo con permisos y
        // proveedores decidiera también con qué colores se lee.
        let trae_modos = contribuciones
            .and_then(|c| c.get("readingModes"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|m| !m.is_empty());
        if trae_modos {
            defectos.push(ManifestDefect::con_esperado(
                DefectCode::ValueNotAllowed,
                "type",
                "theme",
            ));
        }
        // Colores en algo que no se declara tema: o miente sobre lo que es, o
        // se equivocó de clase. Las dos se arreglan igual.
        if colores.is_some_and(|c| !c.is_empty()) {
            defectos.push(ManifestDefect::con_esperado(
                DefectCode::ValueNotAllowed,
                "type",
                "theme",
            ));
        }
        return;
    }

    for (clave, sitio) in [
        ("providers", "contributes.providers"),
        ("commands", "contributes.commands"),
        ("views", "contributes.views"),
    ] {
        let trae_algo = contribuciones
            .and_then(|c| c.get(clave))
            .is_some_and(|v| match v {
                serde_json::Value::Object(o) => !o.is_empty(),
                serde_json::Value::Array(a) => !a.is_empty(),
                _ => false,
            });
        if trae_algo {
            defectos.push(ManifestDefect::nuevo(DefectCode::ThemeWithCode, sitio));
        }
    }
    if objeto
        .get("capabilities")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|c| !c.is_empty())
    {
        defectos.push(ManifestDefect::nuevo(
            DefectCode::ThemeWithCode,
            "capabilities",
        ));
    }

    revisar_modos(objeto, defectos);

    let Some(colores) = colores else {
        return;
    };
    for (token, valor) in colores {
        let sitio = format!("contributes.theme.{token}");
        if !TOKENS_DE_TEMA.contains(&token.as_str()) {
            defectos.push(ManifestDefect::con_esperado(
                DefectCode::UnknownThemeToken,
                sitio,
                TOKENS_DE_TEMA.join(", "),
            ));
            continue;
        }
        match valor.as_str() {
            Some(color) if es_color(color) => {}
            Some(_) => defectos.push(ManifestDefect::con_esperado(
                DefectCode::MalformedValue,
                sitio,
                "#rrggbb",
            )),
            None => defectos.push(ManifestDefect::con_esperado(
                DefectCode::WrongType,
                sitio,
                "string",
            )),
        }
    }
}

/// Comprueba que el manifiesto y el módulo declaran la misma versión de API.
///
/// Es un defecto real y fácil de cometer: la versión se escribe en dos sitios,
/// el manifiesto y la sección personalizada del módulo, y quien sube una sin la
/// otra deja al anfitrión creyendo una cosa y al módulo compilado contra otra.
pub fn coherente_con_el_modulo(manifiesto: &Manifest, wasm: &[u8]) -> Option<ManifestDefect> {
    let declarada = manifiesto.mayor_de_api()?;
    let en_el_modulo = wasm::seccion_personalizada(wasm, super::SECCION_DE_VERSION)
        .and_then(|bytes| super::mayor_declarada(&bytes))?;
    (declarada != en_el_modulo).then(|| {
        ManifestDefect::con_esperado(
            DefectCode::ApiVersionMismatch,
            "apiVersion",
            en_el_modulo.to_string(),
        )
    })
}

// ---------------------------------------------------------------------------
// El esquema publicado y sus ejemplos
// ---------------------------------------------------------------------------

/// La dirección con la que se identifica el esquema publicado.
const IDENTIFICADOR: &str = "https://nbdigitallabs.lat/books/schemas/nb-extension.schema.json";

/// El esquema JSON del manifiesto, generado desde [`CAMPOS`].
pub fn esquema() -> serde_json::Value {
    let mut propiedades = serde_json::Map::new();
    let mut obligatorios = Vec::new();

    for campo in CAMPOS {
        if campo.obligatorio {
            obligatorios.push(serde_json::Value::from(campo.nombre));
        }
        let mut propiedad = serde_json::Map::new();
        propiedad.insert(
            "description".into(),
            serde_json::Value::from(campo.descripcion),
        );
        match &campo.forma {
            Forma::Texto => {
                propiedad.insert("type".into(), "string".into());
                propiedad.insert("minLength".into(), 1.into());
            }
            Forma::TextoConPatron { patron, .. } => {
                propiedad.insert("type".into(), "string".into());
                propiedad.insert("pattern".into(), serde_json::Value::from(*patron));
            }
            Forma::Enumerado(admitidos) => {
                propiedad.insert("type".into(), "string".into());
                propiedad.insert("enum".into(), (*admitidos).into());
            }
            Forma::ListaEnumerada(admitidos) => {
                propiedad.insert("type".into(), "array".into());
                propiedad.insert(
                    "items".into(),
                    serde_json::json!({ "type": "string", "enum": *admitidos }),
                );
                propiedad.insert("uniqueItems".into(), true.into());
            }
            Forma::Objeto => {
                propiedad.insert("type".into(), "object".into());
            }
        }
        propiedades.insert(campo.nombre.into(), propiedad.into());
    }

    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": IDENTIFICADOR,
        "title": "Manifiesto de extensión de NB Books",
        "description": concat!(
            "Generado desde la tabla CAMPOS de src-tauri/src/extensions/manifest.rs. ",
            "No se edita a mano: lo comprueba scripts/check-esquema.mjs."
        ),
        "type": "object",
        "required": obligatorios,
        "additionalProperties": false,
        "properties": propiedades,
    })
}

/// Qué acepta y qué rechaza cada campo, en ejemplos.
///
/// Se publica junto al esquema porque le sirve a quien escribe una extensión, y
/// porque es el corpus con el que `scripts/check-esquema.mjs` comprueba que la
/// expresión del esquema y la función del anfitrión dicen lo mismo.
pub fn ejemplos() -> serde_json::Value {
    let mut salida = serde_json::Map::new();
    for campo in CAMPOS {
        if campo.aceptados.is_empty() && campo.rechazados.is_empty() {
            continue;
        }
        salida.insert(
            campo.nombre.into(),
            serde_json::json!({
                "aceptados": campo.aceptados,
                "rechazados": campo.rechazados,
            }),
        );
    }
    serde_json::json!({
        "description": concat!(
            "Generado desde la tabla CAMPOS de src-tauri/src/extensions/manifest.rs. ",
            "Cada ejemplo lo comprueban a la vez la prueba de Rust, contra la funcion ",
            "del anfitrion, y scripts/check-esquema.mjs, contra la expresion del esquema."
        ),
        "campos": salida,
    })
}

/// Si el valor de un campo lo acepta el anfitrión.
///
/// La usa la prueba del corpus, y es la mitad de Rust de lo que
/// `scripts/check-esquema.mjs` compara con la mitad del esquema.
pub fn acepta(campo: &Campo, valor: &str) -> bool {
    match &campo.forma {
        Forma::Texto => !valor.is_empty(),
        Forma::TextoConPatron { comprueba, .. } => comprueba(valor),
        Forma::Enumerado(admitidos) | Forma::ListaEnumerada(admitidos) => {
            admitidos.contains(&valor)
        }
        Forma::Objeto => true,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un manifiesto correcto, del que parten las variantes de abajo.
    fn valido() -> serde_json::Value {
        serde_json::json!({
            "id": "lat.nbdigitallabs.books.metadata.openlibrary",
            "name": "Open Library Metadata Provider",
            "version": "1.0.0",
            "type": "core",
            "apiVersion": "0.1",
            "license": "Apache-2.0",
            "capabilities": ["network:fetch"],
            "permissions": { "network:fetch": { "urls": ["https://openlibrary.org/api/*"] } },
            "contributes": {}
        })
    }

    fn codigos(defectos: &[ManifestDefect]) -> Vec<(DefectCode, &str)> {
        defectos
            .iter()
            .map(|d| (d.code, d.field.as_str()))
            .collect()
    }

    #[test]
    fn un_manifiesto_correcto_se_acepta_entero() {
        let manifiesto = parse(&valido().to_string()).expect("es válido");
        assert_eq!(
            manifiesto.id,
            "lat.nbdigitallabs.books.metadata.openlibrary"
        );
        assert_eq!(manifiesto.kind, ExtensionKind::Core);
        assert_eq!(manifiesto.capabilities, vec![Capability::NetworkFetch]);
        assert_eq!(manifiesto.mayor_de_api(), Some(0));
    }

    #[test]
    fn se_informan_todos_los_defectos_y_no_solo_el_primero() {
        // Quien arregla un campo y vuelve a fallar por el siguiente hace tantas
        // vueltas como defectos tenga. Es el mismo manifiesto del banco.
        let roto = serde_json::json!({
            "name": "Le falta el identificador",
            "version": 1,
            "type": "nucleo",
            "apiVersion": "1.0",
            "license": "Apache-2.0",
            "capabilities": [],
            "permissions": {},
            "contributes": {}
        });
        let defectos = parse(&roto.to_string()).expect_err("tiene tres defectos");
        assert_eq!(
            codigos(&defectos),
            vec![
                (DefectCode::MissingField, "id"),
                (DefectCode::WrongType, "version"),
                (DefectCode::ValueNotAllowed, "type"),
            ]
        );
    }

    #[test]
    fn cada_defecto_señala_el_campo_concreto() {
        let mut roto = valido();
        roto["permissions"]["network:fetch"]["urls"][0] = serde_json::Value::from("https://*/todo");
        let defectos = parse(&roto.to_string()).expect_err("el comodín no vale");
        assert_eq!(
            codigos(&defectos),
            vec![(
                DefectCode::WildcardHost,
                "permissions.network:fetch.urls[0]"
            )]
        );
    }

    #[test]
    fn una_capacidad_sin_su_permiso_deja_al_usuario_aprobando_algo_sin_alcance() {
        let mut roto = valido();
        roto["permissions"] = serde_json::json!({});
        let defectos = parse(&roto.to_string()).expect_err("falta el detalle");
        assert_eq!(
            codigos(&defectos),
            vec![(
                DefectCode::CapabilityWithoutPermission,
                "permissions.network:fetch"
            )]
        );
    }

    #[test]
    fn un_permiso_sin_su_capacidad_no_aparece_en_lo_que_se_le_enseña_al_usuario() {
        let mut roto = valido();
        roto["capabilities"] = serde_json::json!([]);
        let defectos = parse(&roto.to_string()).expect_err("no se declaró");
        assert_eq!(
            codigos(&defectos),
            vec![(DefectCode::PermissionWithoutCapability, "capabilities")]
        );
    }

    #[test]
    fn el_manifiesto_y_el_modulo_no_pueden_declarar_versiones_distintas() {
        // Se escribe en dos sitios y es fácil subir uno sin el otro. El módulo
        // dice 7 y el manifiesto 0.
        let manifiesto = parse(&valido().to_string()).expect("válido");
        let modulo = modulo_que_declara(b"7.0.0");
        let defecto = coherente_con_el_modulo(&manifiesto, &modulo).expect("discrepan");
        assert_eq!(defecto.code, DefectCode::ApiVersionMismatch);
        assert_eq!(defecto.field, "apiVersion");
        assert_eq!(defecto.expected.as_deref(), Some("7"));

        let coincide = modulo_que_declara(b"0.9.0");
        assert!(coherente_con_el_modulo(&manifiesto, &coincide).is_none());
    }

    /// Un módulo mínimo con la sección de versión y nada más.
    fn modulo_que_declara(version: &[u8]) -> Vec<u8> {
        let nombre = super::super::SECCION_DE_VERSION.as_bytes();
        let mut contenido = vec![nombre.len() as u8];
        contenido.extend_from_slice(nombre);
        contenido.extend_from_slice(version);
        let mut modulo = b"\0asm\x01\0\0\0".to_vec();
        modulo.push(0); // sección personalizada
        modulo.push(contenido.len() as u8);
        modulo.extend_from_slice(&contenido);
        modulo
    }

    #[test]
    fn el_validador_cumple_los_ejemplos_que_se_publican() {
        // Es la mitad de Rust de la costura: la otra la comprueba
        // scripts/check-esquema.mjs ejecutando la expresión del esquema sobre
        // estos mismos ejemplos. Si las dos mitades dejaran de coincidir, un
        // autor validaría bien en su máquina y se le rechazaría aquí.
        for campo in CAMPOS {
            for valor in campo.aceptados {
                assert!(
                    acepta(campo, valor),
                    "{}: se publica {valor:?} como aceptado y el anfitrión lo rechaza",
                    campo.nombre
                );
            }
            for valor in campo.rechazados {
                assert!(
                    !acepta(campo, valor),
                    "{}: se publica {valor:?} como rechazado y el anfitrión lo acepta",
                    campo.nombre
                );
            }
        }
    }

    #[test]
    fn ningun_campo_con_forma_se_queda_sin_ejemplos() {
        // Un campo sin ejemplos no lo cubre ni la prueba de arriba ni el guion,
        // así que su regla dejaría de estar comprobada sin que se note.
        for campo in CAMPOS {
            if matches!(campo.forma, Forma::Objeto) {
                continue;
            }
            assert!(
                !campo.aceptados.is_empty() && !campo.rechazados.is_empty(),
                "{} no publica ejemplos, así que su regla no la comprueba nadie",
                campo.nombre
            );
        }
    }

    /// Publica el esquema y sus ejemplos.
    ///
    /// Se escribe desde una prueba por el mismo motivo que los tipos del puente:
    /// es la forma de que **se generen y no se escriban**, y de que
    /// `scripts/check-esquema.mjs` pueda detectar la deriva regenerando y
    /// comparando el árbol de trabajo.
    #[test]
    fn el_esquema_y_los_ejemplos_se_publican() {
        let destino = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri tiene padre")
            .join("docs")
            .join("architecture")
            .join("api");
        std::fs::create_dir_all(&destino).expect("docs/architecture/api existe");
        for (archivo, contenido) in [
            ("nb-extension.schema.json", esquema()),
            ("nb-extension.ejemplos.json", ejemplos()),
        ] {
            let texto = format!(
                "{}\n",
                serde_json::to_string_pretty(&contenido).expect("serializa")
            );
            std::fs::write(destino.join(archivo), texto).expect("se publica");
        }
    }
}
