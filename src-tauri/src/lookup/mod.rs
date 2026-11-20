//! Consulta sobre la selección: diccionario, enciclopedia y traducción.
//!
//! Contrato: `docs/architecture/reader-contracts.md`.//! //!
//! # Por qué un contrato y no tres funciones
//!
//! Las tres operaciones son la misma vista desde la interfaz: se recibe una
//! selección y se devuelve algo legible junto a ella, sin perder la posición de
//! lectura. Escribirlas por separado obligaría a que el panel supiera a quién
//! preguntar, y añadir después un traductor en línea o un modelo local
//! significaría reescribirlo. Es el mismo patrón que `DocumentProvider` para los
//! formatos, `VoiceProvider` para la voz y el servicio de IA.
//!
//! # Lo que un proveedor declara de sí mismo
//!
//! Su clase, sus idiomas y **si necesita red**. Lo tercero no lo adivina nadie
//! desde fuera: es lo que permite a la interfaz cumplir la regla 3 de
//! [ADR-013](../../../docs/adr/decisions/ADR-013-information-architecture.md)
//! —lo que sale del dispositivo se anuncia— sin tener que mantener una lista de
//! cuáles son.
//!
//! # Qué pasa cuando alguien no puede responder
//!
//! Se **omite**, y se dice por qué. Un proveedor que no cubre el idioma del
//! documento no puede aparecer como una pestaña vacía, y uno que falla no puede
//! llevarse por delante a los demás: es la regla de resiliencia de la
//! Constitución aplicada aquí.

pub mod online;
pub mod stardict;
pub mod store;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{NbError, NbResult};

/// Qué clase de respuesta da un proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum LookupKind {
    /// Definición de una palabra.
    Dictionary,
    /// Artículo o resumen enciclopédico.
    Encyclopedia,
    /// El mismo texto en otro idioma.
    Translation,
}

/// Lo que se consulta.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "LookupQuery")]
pub struct Query {
    /// El texto seleccionado.
    pub term: String,
    /// La frase alrededor. Sirve para desambiguar, y algún proveedor la usa.
    pub context: Option<String>,
    /// Idioma del documento, en etiqueta BCP-47. `None` cuando no se sabe.
    pub language: Option<String>,
}

/// Una entrada de resultado, **con su procedencia**.
///
/// La procedencia no es cortesía: el usuario tiene que poder distinguir lo que
/// dice un diccionario de lo que dice una máquina, y una traducción automática
/// de una definición.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/", rename = "LookupEntry")]
pub struct Entry {
    pub provider_id: String,
    pub kind: LookupKind,
    /// La palabra o expresión que se encontró, que puede no ser la seleccionada:
    /// un diccionario responde por el lema, no por la forma flexionada.
    pub headword: String,
    /// Texto plano. Nunca marcado sin sanear: lo que responde un proveedor es
    /// contenido de terceros.
    pub body: String,
    /// Atribución legible, cuando la obra la exige.
    pub source: Option<String>,
}

/// Por qué un proveedor no participó en una consulta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub enum SkipReason {
    /// No cubre el idioma del documento.
    LanguageUnsupported,
    /// Necesita red y el **interruptor general** está puesto.
    ///
    /// Se arregla apagando el modo sin red, y por eso está separado del
    /// siguiente: había un solo motivo de red, y se partió en
    /// dos al comprobar que las acciones que pide la interfaz no son la misma.
    /// Es la misma distinción que ya hacían `NETWORK_DISABLED` y
    /// `CONSENT_REQUIRED` en el contrato de errores.
    NetworkDisabled,
    /// Necesita salir a un destino que **todavía no está autorizado**.
    ///
    /// Se arregla concediendo el permiso, y por eso el omitido trae consigo el
    /// anfitrión: sin él, la interfaz podría decir que falta un permiso y no
    /// para qué.
    NetworkNotAllowed,
    /// Lo intentó y falló. No se lleva por delante a los demás.
    Failed,
}

/// Un proveedor que no participó, y por qué.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "LookupSkipped"
)]
pub struct Skipped {
    pub provider_id: String,
    pub label: String,
    pub reason: SkipReason,
    /// A dónde habría salido, cuando el motivo es de red.
    ///
    /// Nulo para un proveedor local. Es lo que permite a la interfaz pedir el
    /// permiso **de este destino** en lugar de un permiso genérico, y lo que
    /// impide que el webview se invente uno: al conceder, el runtime comprueba
    /// que el anfitrión es de ese proveedor.
    pub host: Option<String>,
}

/// Resultado de una consulta.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "LookupOutcome"
)]
pub struct Outcome {
    pub entries: Vec<Entry>,
    /// Quién se quedó fuera y por qué. Es lo que permite a la interfaz decir
    /// "el diccionario de este idioma no está instalado" en lugar de enseñar un
    /// panel vacío.
    pub skipped: Vec<Skipped>,
}

impl Outcome {
    /// No hubo ni una respuesta.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Lo que sabe hacer una fuente de consulta.
pub trait LookupProvider: Send + Sync {
    /// Identificador estable. Es lo que se guarda al anotar la procedencia.
    fn id(&self) -> &str;

    /// Nombre legible, para la interfaz y para la atribución.
    fn label(&self) -> &str;

    fn kind(&self) -> LookupKind;

    /// Idiomas que sabe responder, en etiquetas BCP-47.
    ///
    /// Una lista **vacía** significa "cualquiera", que es el caso de un
    /// traductor con detección automática.
    fn languages(&self) -> &[String];

    /// Si necesita salir del dispositivo. Lo declara él; nadie lo deduce.
    fn requires_network(&self) -> bool;

    /// A dónde saldría **esta** consulta.
    ///
    /// Por consulta y no fijo porque hay proveedores cuyo destino depende del
    /// idioma: la enciclopedia de un documento en español y la de uno en
    /// portugués no son el mismo anfitrión, y decir uno de los dos siempre sería
    /// mentir la mitad de las veces.
    fn destination(&self, _query: &Query) -> Option<String> {
        None
    }

    /// Si este anfitrión es de los suyos.
    ///
    /// Lo usa el runtime al conceder un permiso desde la interfaz: sin esta
    /// comprobación, el webview podría pedir permiso para cualquier destino y
    /// convertirse en el sitio donde se decide a dónde sale la aplicación.
    fn owns_host(&self, _host: &str) -> bool {
        false
    }

    /// Responde a una consulta.
    ///
    /// # Errors
    ///
    /// Cualquiera. El registro lo convierte en [`SkipReason::Failed`] y sigue
    /// con los demás: un proveedor caído no puede tumbar la consulta entera.
    fn lookup(&self, query: &Query) -> NbResult<Vec<Entry>>;
}

/// Registro de proveedores de consulta.
///
/// El orden es estable —por clase y luego por identificador— para que la
/// interfaz no baile entre consultas: primero las definiciones, después lo
/// enciclopédico, y la traducción al final, que es el orden en que se necesita.
#[derive(Default)]
pub struct LookupRegistry {
    proveedores: BTreeMap<(LookupKind, String), Box<dyn LookupProvider>>,
}

impl LookupRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra un proveedor. Si ya había uno con la misma clase e
    /// identificador, lo sustituye.
    pub fn register(&mut self, provider: Box<dyn LookupProvider>) {
        let clave = (provider.kind(), provider.id().to_owned());
        if self.proveedores.insert(clave, provider).is_some() {
            tracing::warn!("se sustituyó un proveedor de consulta ya registrado");
        }
    }

    /// Cuántos proveedores hay instalados, sea cual sea el idioma.
    #[must_use]
    pub fn len(&self) -> usize {
        self.proveedores.len()
    }

    /// No hay ninguno instalado.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.proveedores.is_empty()
    }

    /// Si algún proveedor registrado reconoce este anfitrión como suyo.
    ///
    /// Es la comprobación que separa "el usuario autoriza un destino que la
    /// aplicación le ha pedido" de "el webview autoriza el destino que quiera".
    #[must_use]
    pub fn algun_proveedor_usa(&self, host: &str) -> bool {
        self.proveedores.values().any(|p| p.owns_host(host))
    }

    /// Consulta a todos los que pueden responder.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si no hay **ningún** proveedor
    /// instalado. Es distinto de que ninguno cubra el idioma: lo primero
    /// significa que esta instalación no sabe consultar y la interfaz debe
    /// ocultar la función; lo segundo, que falta instalar un idioma y la
    /// interfaz debe decirlo y ofrecerlo.
    pub fn lookup(&self, query: &Query, allow_network: bool) -> NbResult<Outcome> {
        if self.proveedores.is_empty() {
            tracing::info!("no hay ningún proveedor de consulta en esta instalación");
            return Err(NbError::ComponentUnavailable);
        }

        let mut resultado = Outcome::default();

        for proveedor in self.proveedores.values() {
            let omitir = |reason| Skipped {
                provider_id: proveedor.id().to_owned(),
                label: proveedor.label().to_owned(),
                reason,
                host: proveedor.destination(query),
            };

            if proveedor.requires_network() && !allow_network {
                resultado.skipped.push(omitir(SkipReason::NetworkDisabled));
                continue;
            }

            if !cubre_el_idioma(proveedor.languages(), query.language.as_deref()) {
                resultado
                    .skipped
                    .push(omitir(SkipReason::LanguageUnsupported));
                continue;
            }

            match proveedor.lookup(query) {
                Ok(entradas) => resultado.entries.extend(entradas),
                Err(error) => {
                    // Un fallo local produce consecuencias locales: se anota y
                    // se sigue. Ver la regla 1 de la Constitución.
                    //
                    // Falta de permiso **no es un fallo**: es una decisión que
                    // el usuario todavía no ha tomado, y llamarla fallo dejaría
                    // a la interfaz diciendo "no respondió" cuando lo que hay
                    // que ofrecer es autorizar el destino.
                    let motivo = match error {
                        NbError::ConsentRequired => SkipReason::NetworkNotAllowed,
                        NbError::NetworkDisabled => SkipReason::NetworkDisabled,
                        _ => SkipReason::Failed,
                    };
                    tracing::warn!(
                        provider = proveedor.id(),
                        error = %error,
                        "un proveedor de consulta no participó; se omite y se sigue con los demás"
                    );
                    resultado.skipped.push(omitir(motivo));
                }
            }
        }

        Ok(resultado)
    }
}

/// Cuántos caracteres puede tener lo que se consulta.
///
/// El webview es zona no confiable, así que el límite se pone en el runtime y no
/// en la interfaz. No es un número redondo por gusto: una selección de un
/// párrafo entero cabe —la traducción la necesita—, y un documento entero
/// mandado por error, no.
pub const MAXIMO_TERMINO: usize = 2_000;

/// Comprueba lo que llega del webview y devuelve el término ya recortado.
///
/// Existe como función y no como dos líneas dentro del comando para poder
/// romperla en una prueba: un límite sin prueba es una intención, no una
/// defensa.
///
/// # Errors
///
/// [`NbError::PathNotAllowed`] si está vacío o pasa de [`MAXIMO_TERMINO`]
/// caracteres. Se cuentan **caracteres** y no bytes: con bytes, un término en
/// japonés valdría la tercera parte que uno en inglés.
pub fn termino_valido(bruto: &str) -> NbResult<&str> {
    let termino = bruto.trim();
    if termino.is_empty() || termino.chars().count() > MAXIMO_TERMINO {
        // No se registra el término: es texto del documento del usuario.
        tracing::info!("se rechazó una consulta con un término fuera de límites");
        return Err(NbError::PathNotAllowed);
    }
    Ok(termino)
}

/// Registro con los diccionarios que hay instalados en esta máquina.
///
/// # Por qué se construye leyendo la carpeta y no una preferencia
///
/// Porque la lista de instalados **es** la carpeta, que es la decisión de
/// con una segunda lista en las preferencias existiría la posibilidad
/// de que las dos discreparan, y desinstalar dejaría rastro justo en el sitio
/// del que nadie se acuerda.
///
/// Un diccionario que no se deja abrir se omite con un aviso en lugar de impedir
/// que se registren los demás: una instalación a medias no puede dejar la
/// consulta entera inservible.
#[must_use]
pub fn instalados(carpeta: &std::path::Path) -> LookupRegistry {
    let mut registro = LookupRegistry::new();
    let Ok(manifiestos) = store::list(carpeta) else {
        tracing::warn!("la carpeta de diccionarios no se deja leer");
        return registro;
    };

    for manifiesto in manifiestos {
        match store::Dictionary::open(carpeta, &manifiesto.id) {
            Ok(diccionario) => registro.register(Box::new(diccionario)),
            Err(error) => tracing::warn!(
                code = error.code(),
                "un diccionario instalado no se deja abrir; se omite"
            ),
        }
    }

    tracing::info!(
        diccionarios = registro.len(),
        "proveedores de consulta registrados"
    );
    registro
}

/// Si un proveedor cubre el idioma pedido.
///
/// Una lista vacía cubre cualquiera. La comparación es por **subetiqueta
/// primaria**: un diccionario que declara `es` responde a un documento en
/// `es-CO`, porque exigir la coincidencia exacta dejaría sin diccionario a casi
/// todo el mundo. Y un documento sin idioma declarado se acepta: es mejor
/// ofrecer una consulta de más que ninguna, y quien no encuentre nada devolverá
/// una lista vacía.
fn cubre_el_idioma(soportados: &[String], pedido: Option<&str>) -> bool {
    if soportados.is_empty() {
        return true;
    }
    let Some(pedido) = pedido else {
        return true;
    };

    let primaria = |etiqueta: &str| {
        etiqueta
            .split(['-', '_'])
            .next()
            .unwrap_or(etiqueta)
            .to_ascii_lowercase()
    };

    let objetivo = primaria(pedido);
    soportados.iter().any(|s| primaria(s) == objetivo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_termino_vacio_o_solo_de_espacios_se_rechaza() {
        assert!(matches!(termino_valido(""), Err(NbError::PathNotAllowed)));
        assert!(matches!(
            termino_valido("   \n "),
            Err(NbError::PathNotAllowed)
        ));
    }

    #[test]
    fn un_termino_mas_largo_que_el_limite_se_rechaza() {
        let cabe = "a".repeat(MAXIMO_TERMINO);
        assert_eq!(termino_valido(&cabe).expect("cabe"), cabe);

        let pasa = "a".repeat(MAXIMO_TERMINO + 1);
        assert!(matches!(
            termino_valido(&pasa),
            Err(NbError::PathNotAllowed)
        ));
    }

    /// El límite se cuenta en caracteres. Con bytes, un término en japonés
    /// valdría la tercera parte que uno en inglés, y el mismo párrafo pasaría o
    /// no según el idioma del libro.
    #[test]
    fn el_limite_se_cuenta_en_caracteres_y_no_en_bytes() {
        let japones = "本".repeat(MAXIMO_TERMINO);
        assert!(japones.len() > MAXIMO_TERMINO);
        assert!(termino_valido(&japones).is_ok());
    }

    #[test]
    fn el_termino_llega_recortado() {
        assert_eq!(termino_valido("  aletear \n").expect("válido"), "aletear");
    }

    /// Proveedor de prueba, configurable. Existe para poder comprobar el
    /// contrato sin ningún diccionario real: lo que se prueba aquí es el
    /// registro, no una obra lexicográfica.
    struct Falso {
        id: &'static str,
        kind: LookupKind,
        languages: Vec<String>,
        network: bool,
        falla: bool,
    }

    impl Falso {
        fn nuevo(id: &'static str, kind: LookupKind, idiomas: &[&str]) -> Self {
            Self {
                id,
                kind,
                languages: idiomas.iter().map(|s| (*s).to_owned()).collect(),
                network: false,
                falla: false,
            }
        }
        fn con_red(mut self) -> Self {
            self.network = true;
            self
        }
        fn que_falla(mut self) -> Self {
            self.falla = true;
            self
        }
        fn caja(self) -> Box<dyn LookupProvider> {
            Box::new(self)
        }
    }

    impl LookupProvider for Falso {
        fn id(&self) -> &str {
            self.id
        }
        fn label(&self) -> &str {
            self.id
        }
        fn kind(&self) -> LookupKind {
            self.kind
        }
        fn languages(&self) -> &[String] {
            &self.languages
        }
        fn requires_network(&self) -> bool {
            self.network
        }
        fn lookup(&self, query: &Query) -> NbResult<Vec<Entry>> {
            if self.falla {
                return Err(NbError::ComponentUnavailable);
            }
            Ok(vec![Entry {
                provider_id: self.id.to_owned(),
                kind: self.kind,
                headword: query.term.clone(),
                body: format!("respuesta de {}", self.id),
                source: None,
            }])
        }
    }

    fn consulta(idioma: Option<&str>) -> Query {
        Query {
            term: "costumbre".to_owned(),
            context: None,
            language: idioma.map(str::to_owned),
        }
    }

    /// **Criterio, primera mitad.** El que no cubre el idioma se omite y se
    /// dice por qué, en lugar de aparecer como una respuesta vacía.
    #[test]
    fn el_proveedor_que_no_cubre_el_idioma_se_omite_y_se_declara() {
        let mut registro = LookupRegistry::new();
        registro.register(Falso::nuevo("es", LookupKind::Dictionary, &["es"]).caja());
        registro.register(Falso::nuevo("de", LookupKind::Dictionary, &["de"]).caja());

        let resultado = registro
            .lookup(&consulta(Some("es-CO")), false)
            .expect("consulta");

        assert_eq!(resultado.entries.len(), 1);
        assert_eq!(resultado.entries[0].provider_id, "es");
        assert_eq!(resultado.skipped.len(), 1);
        assert_eq!(resultado.skipped[0].provider_id, "de");
        assert_eq!(resultado.skipped[0].reason, SkipReason::LanguageUnsupported);
    }

    /// **Criterio, segunda mitad.** Sin ningún proveedor, la consulta lo declara
    /// con un error que la interfaz sabe interpretar como "ocultar la función".
    #[test]
    fn sin_ningun_proveedor_la_consulta_lo_declara() {
        let registro = LookupRegistry::new();
        assert!(registro.is_empty());
        assert!(matches!(
            registro.lookup(&consulta(Some("es")), false),
            Err(NbError::ComponentUnavailable)
        ));
    }

    /// Distinto de lo anterior: hay proveedores, pero ninguno para este idioma.
    /// No es un error, es una respuesta vacía **explicada**.
    #[test]
    fn con_proveedores_pero_ninguno_del_idioma_no_es_un_error() {
        let mut registro = LookupRegistry::new();
        registro.register(Falso::nuevo("de", LookupKind::Dictionary, &["de"]).caja());

        let resultado = registro
            .lookup(&consulta(Some("es")), false)
            .expect("no es un error");

        assert!(resultado.is_empty());
        assert_eq!(resultado.skipped.len(), 1, "tiene que decir por qué");
    }

    /// La subetiqueta primaria basta: exigir coincidencia exacta dejaría sin
    /// diccionario a casi todo el mundo.
    #[test]
    fn un_diccionario_de_espanol_sirve_para_una_variante_regional() {
        assert!(cubre_el_idioma(&["es".to_owned()], Some("es-CO")));
        assert!(cubre_el_idioma(&["pt-BR".to_owned()], Some("pt")));
        assert!(cubre_el_idioma(&["EN".to_owned()], Some("en-GB")));
        assert!(!cubre_el_idioma(&["es".to_owned()], Some("de")));
    }

    /// Una lista vacía de idiomas es "cualquiera", y un documento sin idioma
    /// declarado no puede quedarse sin consulta.
    #[test]
    fn sin_idioma_declarado_se_consulta_igual() {
        assert!(cubre_el_idioma(&[], Some("cualquiera")));
        assert!(cubre_el_idioma(&["es".to_owned()], None));
    }

    /// La red no se usa por defecto. Mientras no exista la capa de salida
    /// consentida, un proveedor que la necesita se omite siempre, y
    /// se dice.
    #[test]
    fn un_proveedor_que_necesita_red_se_omite_si_no_se_autoriza() {
        let mut registro = LookupRegistry::new();
        registro.register(
            Falso::nuevo("wikipedia", LookupKind::Encyclopedia, &[])
                .con_red()
                .caja(),
        );
        registro.register(Falso::nuevo("local", LookupKind::Dictionary, &["es"]).caja());

        let sin_red = registro
            .lookup(&consulta(Some("es")), false)
            .expect("consulta");
        assert_eq!(sin_red.entries.len(), 1);
        assert_eq!(sin_red.entries[0].provider_id, "local");
        // Con el interruptor general puesto el motivo es ese y no la falta de
        // permiso: lo primero se arregla apagando el modo sin red y lo segundo
        // concediendo una autorización, y confundirlos haría que la interfaz
        // ofreciera la acción que no toca.
        assert_eq!(sin_red.skipped[0].reason, SkipReason::NetworkDisabled);

        let con_red = registro
            .lookup(&consulta(Some("es")), true)
            .expect("consulta");
        assert_eq!(con_red.entries.len(), 2);
        assert!(con_red.skipped.is_empty());
    }

    /// La regla 1 de la Constitución, aquí: un fallo local produce
    /// consecuencias locales.
    #[test]
    fn un_proveedor_que_falla_no_se_lleva_por_delante_a_los_demas() {
        let mut registro = LookupRegistry::new();
        registro.register(
            Falso::nuevo("roto", LookupKind::Dictionary, &["es"])
                .que_falla()
                .caja(),
        );
        registro.register(Falso::nuevo("sano", LookupKind::Dictionary, &["es"]).caja());

        let resultado = registro
            .lookup(&consulta(Some("es")), false)
            .expect("la consulta no puede fallar entera");

        assert_eq!(resultado.entries.len(), 1);
        assert_eq!(resultado.entries[0].provider_id, "sano");
        assert_eq!(resultado.skipped[0].provider_id, "roto");
        assert_eq!(resultado.skipped[0].reason, SkipReason::Failed);
    }

    /// El orden no puede bailar entre consultas: primero las definiciones,
    /// después lo enciclopédico y la traducción al final, que es el orden en que
    /// se necesitan.
    #[test]
    fn el_orden_de_los_resultados_es_estable_y_por_clase() {
        let mut registro = LookupRegistry::new();
        registro.register(Falso::nuevo("t", LookupKind::Translation, &[]).caja());
        registro.register(Falso::nuevo("e", LookupKind::Encyclopedia, &[]).caja());
        registro.register(Falso::nuevo("d", LookupKind::Dictionary, &[]).caja());

        let resultado = registro.lookup(&consulta(None), false).expect("consulta");
        let clases: Vec<LookupKind> = resultado.entries.iter().map(|e| e.kind).collect();

        assert_eq!(
            clases,
            vec![
                LookupKind::Dictionary,
                LookupKind::Encyclopedia,
                LookupKind::Translation
            ]
        );
    }
}
