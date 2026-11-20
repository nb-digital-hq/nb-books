//! Servicio de IA: un solo contrato para la inferencia local, los proveedores
//! externos y **la ausencia total**.
//!
//! Contrato: `docs/architecture/ai/philosophy.md`.
//!
//! # Por qué la ausencia es un valor y no un error
//!
//! La regla 1 de [`philosophy.md`] dice que si el subsistema de IA no existe, no
//! arranca o falla, la aplicación abre, lee, busca y anota con normalidad. Eso
//! solo se cumple si "no hay IA" es algo que el contrato **devuelve**, no algo
//! que lanza: un `Err` obliga a quien llama a tratarlo, y el día que alguien no
//! lo trate la ausencia de una función opcional se habrá convertido en el fallo
//! de una pantalla. Por eso [`Availability`] tiene una rama [`Absence`] con su
//! motivo, y por eso [`AiService`] se puede construir **vacío** sin que nada se
//! entere.
//!
//! [`philosophy.md`]: ../../../docs/architecture/ai/philosophy.md
//!
//! # Local primero, y no como preferencia
//!
//! La regla 2 dice que el proveedor cloud es una elección consciente del
//! usuario, **nunca un fallback silencioso**. Escrito como preferencia, esa
//! regla dura hasta el primer `unwrap_or_else` que busque "el siguiente
//! proveedor que haya". Escrito como aquí -[`Choice::Auto`] solo mira a los
//! proveedores de [`Placement::Local`], y salir del dispositivo exige nombrar a
//! quién- la regla es la firma de la función y no hay dónde saltársela. La
//! prueba `un_proveedor_externo_no_se_usa_sin_que_lo_elijan` la fija.
//!
//! # Qué **no** hace este módulo
//!
//! No hay ningún motor aquí: este módulo es la frontera, no la implementación. No
//! entra ninguna dependencia, no cruza nada el puente IPC y no hay pesos que
//! descargar. Los proveedores reales llegan (inferencia local) y
//! `ai::externo` (externos), y los tipos se exportan a TypeScript donde hay un
//! comando que los lleve: hasta entonces, generar enlaces de algo que nadie
//! invoca sería superficie pública sin usuario.

pub mod busqueda;
pub mod catalogo;
pub mod contexto;
pub mod cuaderno;
pub mod envios;
pub mod estudio;
pub mod externo;
pub mod generacion;
pub mod generativo;
pub mod hardware;
pub mod indice;
pub mod instalacion;
pub mod local;
pub mod modelo;
pub mod plantillas;
pub mod presupuesto;
pub use nb_books_lector::repaso_espaciado;
pub mod reportes;
pub mod rubrica;
pub mod salidas;
pub mod subsistema;

use crate::error::{NbError, NbResult};
use crate::monitor::ComponentState;
use crate::reader::Locator;

/// Dónde se procesa el texto que se le entrega a un proveedor.
///
/// No es una etiqueta descriptiva: es lo que decide si hace falta consentimiento
/// y lo que la interfaz tiene que enseñar de forma permanente, según la sección
/// 1 de [`ai/privacy.md`](../../../docs/architecture/ai/privacy.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Placement {
    /// En el dispositivo. No sale nada y no hace falta permiso de nadie.
    Local,
    /// En los servidores del proveedor que el usuario eligió, con su clave.
    External,
}

impl Placement {
    /// Si entregarle texto a un proveedor de esta clase saca contenido del
    /// dispositivo.
    ///
    /// Lo deduce el contrato y no cada proveedor, porque es la pregunta de la
    /// que cuelga el consentimiento: dejarla en manos de quien se registra
    /// sería permitir que un proveedor externo se declarase local.
    #[must_use]
    pub const fn sale_del_dispositivo(self) -> bool {
        !matches!(self, Self::Local)
    }
}

/// Qué sabe hacer un proveedor.
///
/// La lista sale de la tabla de casos de uso de `philosophy.md` sección 2, que
/// es también la que dice que cualquier función fuera de ella necesita un ADR
/// antes de implementarse. Añadir una variante aquí es, por tanto, una decisión
/// y no un detalle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// Resumen de un capítulo o de una selección.
    Summary,
    /// Tarjetas de estudio.
    Flashcards,
    /// Preguntas de autoevaluación.
    Questions,
    /// Entidades y conceptos con su procedencia.
    Concepts,
    /// Vectores para la recuperación semántica. Ver
    /// [ADR-007](../../../docs/adr/decisions/ADR-007-vector-store.md).
    Embedding,
}

impl Capability {
    /// Nombre estable, el que se guarda en el registro de envíos.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Flashcards => "flashcards",
            Self::Questions => "questions",
            Self::Concepts => "concepts",
            Self::Embedding => "embedding",
        }
    }
}

/// Por qué no hay servicio de IA.
///
/// Las tres piden acciones distintas a la interfaz, y confundirlas produce el
/// defecto que `salud.ts` documenta: ofrecer instalar lo que no se puede
/// instalar, o esconder lo que solo hacía falta encender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absence {
    /// Nadie lo ha encendido. Es el estado de fábrica y **no es una avería**.
    Disabled,
    /// Falta el motor o los pesos, y se pueden traer. Invita a instalar.
    NotInstalled,
    /// El equipo no da para ello. Se oculta sin invitar a nada, porque invitar
    /// sería frustrar.
    NotSupported,
    /// Hay modelo y el equipo da para él, pero **no cabe en el tope de memoria que
    /// eligió el usuario**.
    ///
    /// No es una avería ni pide atención: es una decisión de quien tiene el equipo,
    /// y se ve en Ajustes junto al tope que la produce. Tampoco es `NotSupported`,
    /// que dice que no hay nada que hacer: aquí basta con subir el tope.
    OverBudget,
}

impl Absence {
    /// Cómo se le cuenta al monitor de salud.
    ///
    /// `Disabled` es [`ComponentState::Stopped`] y no `NotInstalled`, y la
    /// diferencia se ve en la interfaz: `salud.ts` trata `NOT_INSTALLED` como un
    /// aviso accionable -"falta algo, instálalo"- y excluye `STOPPED` de los
    /// avisos justamente porque es "un componente que nadie ha pedido todavía".
    #[must_use]
    pub const fn component_state(self) -> ComponentState {
        match self {
            Self::Disabled => ComponentState::Stopped,
            Self::NotInstalled => ComponentState::NotInstalled,
            Self::NotSupported => ComponentState::NotSupported,
            Self::OverBudget => ComponentState::Stopped,
        }
    }

    /// Código canónico del motivo, nunca un mensaje libre: lo traduce la
    /// interfaz. Ver `docs/architecture/i18n.md`.
    #[must_use]
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::Disabled => "AI_DISABLED",
            Self::NotInstalled => "AI_NOT_INSTALLED",
            Self::NotSupported => "AI_NOT_SUPPORTED",
            Self::OverBudget => "AI_MEMORY_LIMIT",
        }
    }
}

/// Lo que el servicio puede ofrecer para una capacidad concreta.
///
/// Por capacidad y no en general porque no son la misma pregunta: un proveedor
/// puede generar vectores y no saber resumir, y decir "la IA está lista" cuando
/// solo lo está la mitad haría que la interfaz ofreciera lo que no hay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// Hay quien la atienda, y es este.
    Ready(ProviderInfo),
    /// No la hay, y este es el motivo.
    Absent(Absence),
}

impl Availability {
    /// Si se puede pedir.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready(_))
    }
}

/// La cara pública de un proveedor: lo que la interfaz necesita para presentarlo
/// sin conocer su implementación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    /// Identificador estable. Es lo que se guarda al anotar la procedencia de
    /// una salida generada.
    pub id: String,
    /// Nombre legible.
    pub label: String,
    pub placement: Placement,
    /// A dónde saldría el texto. Nulo para un proveedor local.
    ///
    /// Lo necesita la interfaz para pedir el permiso **de este destino** en
    /// lugar de uno genérico, que es la misma razón por la que existe en
    /// `lookup::Skipped`.
    pub host: Option<String>,
}

/// Un pasaje del documento, con su posición.
///
/// Lleva [`Locator`] y no un número porque es la unidad universal de posición
/// del producto ([ADR-004]) y porque de aquí salen las citas verificables de
/// una afirmación generada tiene que poder abrir el sitio del que salió,
/// y un entero no abre nada.
///
/// [ADR-004]: ../../../docs/adr/decisions/ADR-004-reader-locator-model.md
#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    pub locator: Locator,
    /// Texto plano del documento. **Es entrada hostil**: ver `ai/prompts.md`.
    pub text: String,
}

/// Lo que se le pide al servicio.
///
/// # La separación entre instrucción y contenido es del tipo, no del prompt
///
/// `instruction` la escribe el producto; `passages` sale del documento, que es
/// de un tercero y puede estar escrito para engañar. Tenerlos en dos campos
/// obliga a que cualquier proveedor los reciba separados y no pueda recibir una
/// cadena ya mezclada. Las plantillas construyen la defensa encima; esto es el sitio
/// donde apoyarla, y ponerlo ahora cuesta un campo, mientras que ponerlo después
/// costaría reescribir a todos los proveedores.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub capability: Capability,
    /// La instrucción del producto. Nunca sale del documento.
    pub instruction: String,
    /// El contenido sobre el que se trabaja. Entrada no confiable.
    pub passages: Vec<Passage>,
    /// Lo que se pide sobre ese contenido, escrito también por el producto.
    ///
    /// Va **después** de los pasajes, y no dentro de la instrucción, porque un
    /// modelo pequeño atiende mejor a lo último que lee; pero es del producto igual
    /// que la instrucción, y por eso tiene su campo y no se mezcla con los pasajes.
    pub task: String,
    /// Idioma del documento, en etiqueta BCP-47. `None` cuando no se sabe.
    pub language: Option<String>,
}

/// Una salida generada.
///
/// **El tipo es la marca.** La regla 4 de `philosophy.md` exige que toda salida
/// generada sea identificable como tal; si esto fuera un `String`, la marca
/// dependería de que quien lo recibe se acuerde de ponerla, y lo que depende de
/// acordarse no es una garantía.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    pub text: String,
    /// De qué pasajes salió.
    ///
    /// **Vacío significa no verificable**, y la interfaz tiene que decirlo así
    /// (regla 5). No es lo mismo que una salida sin citas por descuido: el
    /// contrato no distingue las dos, y por eso lo no verificable se marca en
    /// lugar de suponerse correcto.
    pub grounding: Vec<Locator>,
    /// Quién la generó. Es la procedencia que la interfaz enseña.
    pub provider_id: String,
}

/// Un vector de embeddings.
///
/// Coma flotante aquí y cuantizado a ocho bits al persistir, que es lo que
/// decidió [ADR-007](../../../docs/adr/decisions/ADR-007-vector-store.md) el
/// 2026-09-12. La cuantización es y no de este contrato: al proveedor
/// se le piden vectores, no un formato de almacenamiento.
pub type Embedding = Vec<f32>;

/// Lo que sabe hacer un motor de IA, sea cual sea y esté donde esté.
///
/// Ningún componente del producto instancia un motor concreto: el antipatrón que
/// prohíbe la sección 1 de
/// [`component-model.md`](../../../docs/architecture/component-model.md)
/// es exactamente `Reader -> instancia concreta -> fallo si no existe`.
pub trait AiProvider: Send + Sync {
    /// Identificador estable. Se guarda con cada salida generada.
    fn id(&self) -> &str;

    /// Nombre legible.
    fn label(&self) -> &str;

    /// Dónde procesa el texto. De aquí cuelga el consentimiento.
    fn placement(&self) -> Placement;

    /// Qué capacidades atiende. Las declara él; nadie las deduce.
    fn capabilities(&self) -> &[Capability];

    /// A dónde saldría el texto. Nulo para un proveedor local.
    fn host(&self) -> Option<&str> {
        None
    }

    /// Si puede atender ahora, y si no, por qué.
    ///
    /// Se pregunta en cada consulta y no una vez al arrancar: un modelo se
    /// libera por inactividad y un presupuesto de recursos se agota
    ///, así que "estaba listo hace un rato" no es una respuesta.
    fn readiness(&self) -> Availability;

    /// Genera texto.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si no atiende esa capacidad. Un
    /// proveedor que no genera texto no implementa esto.
    fn generate(&self, _request: &Request) -> NbResult<Completion> {
        Err(NbError::ComponentUnavailable)
    }

    /// Genera vectores.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] si no atiende esa capacidad.
    fn embed(&self, _texts: &[String]) -> NbResult<Vec<Embedding>> {
        Err(NbError::ComponentUnavailable)
    }

    /// Caracteres de documento que admite en una petición de generación.
    ///
    /// Lo declara cada proveedor porque no es lo mismo la ventana de un modelo
    /// local pequeño, donde además cada token de entrada cuesta tiempo de
    /// procesador, que un servicio que admite cien mil tokens.
    fn input_chars(&self) -> usize {
        CARACTERES_DE_ENTRADA
    }
}

/// Lo que admite un proveedor que no dice otra cosa, en caracteres.
pub const CARACTERES_DE_ENTRADA: usize = 8_000;

/// Un proveedor compartido es un proveedor.
///
/// Existe para que el subsistema pueda **registrar** un proveedor en el servicio y
/// **quedarse con él** a la vez: el externo se registra para que las
/// funciones lo nombren, y Ajustes lo prueba directamente para enseñar qué se
/// enviaría antes de enviarlo.
impl<T: AiProvider + ?Sized> AiProvider for std::sync::Arc<T> {
    fn id(&self) -> &str {
        (**self).id()
    }

    fn label(&self) -> &str {
        (**self).label()
    }

    fn placement(&self) -> Placement {
        (**self).placement()
    }

    fn capabilities(&self) -> &[Capability] {
        (**self).capabilities()
    }

    fn host(&self) -> Option<&str> {
        (**self).host()
    }

    fn readiness(&self) -> Availability {
        (**self).readiness()
    }

    fn generate(&self, request: &Request) -> NbResult<Completion> {
        (**self).generate(request)
    }

    fn embed(&self, texts: &[String]) -> NbResult<Vec<Embedding>> {
        (**self).embed(texts)
    }

    fn input_chars(&self) -> usize {
        (**self).input_chars()
    }
}

/// A quién se le pide.
///
/// Que exista este tipo es lo que hace que la regla "local primero" sea
/// estructural: no hay ninguna forma de salir del dispositivo sin escribir el
/// identificador de a quién, y escribirlo es lo que la interfaz solo hace cuando
/// el usuario lo ha elegido.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Choice {
    /// Lo que haya en el dispositivo, y **nada más**.
    #[default]
    Auto,
    /// Este proveedor y ningún otro, lo elija el usuario o la institución.
    Provider(String),
}

/// El servicio de IA del runtime.
///
/// Se construye vacío o apagado sin que nada falle, que es la mitad del criterio
/// de aceptación.
#[derive(Default)]
pub struct AiService {
    proveedores: Vec<Box<dyn AiProvider>>,
    /// Apagado por decisión, no por falta de motor. Separar los dos casos es lo
    /// que permite a la interfaz decir "enciéndelo" en vez de "instálalo".
    apagado: bool,
    /// A quién contarle que un proveedor se cayó. Opcional porque el servicio
    /// tiene que poder construirse en una prueba sin montar medio runtime.
    monitor: Option<crate::monitor::Monitor>,
    /// De dónde se sabe si el trabajo de fondo tiene que esperar. `None` es la
    /// sonda real, la batería y las zonas térmicas; otra cosa, la de una prueba.
    sonda: Option<fn() -> Option<presupuesto::Suspension>>,
}

impl AiService {
    /// Un servicio sin ningún proveedor. No es un error: es el estado de fábrica
    /// hasta que alguien instale un motor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// El mismo servicio, informando al monitor de salud.
    #[must_use]
    pub fn con_monitor(mut self, monitor: crate::monitor::Monitor) -> Self {
        self.monitor = Some(monitor);
        self
    }

    /// El mismo servicio, preguntando por la batería y la temperatura a otra
    /// sonda. Existe para comprobar la suspensión sin descargar una batería.
    #[must_use]
    pub fn con_sonda_de_suspension(
        mut self,
        sonda: fn() -> Option<presupuesto::Suspension>,
    ) -> Self {
        self.sonda = Some(sonda);
        self
    }

    /// Si el trabajo **de fondo** de esta capacidad tiene que esperar ahora, y por
    /// qué.
    ///
    /// # Por qué solo el de fondo
    ///
    /// Porque lo que vacía una batería o calienta un equipo es generar los vectores
    /// de un libro —cientos de fragmentos, minutos de procesador—, no calcular el de
    /// una consulta, que son unas décimas de segundo que alguien acaba de pedir.
    /// Negarle una búsqueda a quien tiene el 19 % de batería le quitaría una función
    /// a cambio de nada que se note, y es lo mismo que hace el indexador léxico: se
    /// detiene él, no la búsqueda. Lo interactivo queda acotado por el grupo de
    /// hilos y la prioridad, que se aplican siempre.
    ///
    /// # Por qué solo con el proveedor local
    ///
    /// Porque un proveedor externo no calienta este equipo ni gasta su batería: lo
    /// que gasta es la red, y eso lo gobierna su consentimiento.
    ///
    /// Quien genera en segundo plano lo pregunta **antes de cada lote**, igual que
    /// el indexador pregunta por la batería antes de cada documento.
    #[must_use]
    pub fn pausa_de_fondo(
        &self,
        capability: Capability,
        choice: &Choice,
    ) -> Option<presupuesto::Suspension> {
        let local = self
            .elegir(capability, choice)
            .is_ok_and(|p| p.placement() == Placement::Local);
        if !local {
            return None;
        }
        self.sonda
            .map_or_else(presupuesto::suspension, |sonda| sonda())
    }

    /// Un servicio **desactivado por completo**.
    ///
    /// Es el que exige el criterio de aceptación, y el que usa
    /// el escenario 8 de caos: con esto puesto, el resto del producto tiene que
    /// comportarse exactamente igual.
    #[must_use]
    pub fn apagado() -> Self {
        Self {
            proveedores: Vec::new(),
            apagado: true,
            monitor: None,
            sonda: None,
        }
    }

    /// Registra un proveedor. El último con un identificador dado sustituye al
    /// anterior, igual que en `LookupRegistry`.
    pub fn register(&mut self, provider: Box<dyn AiProvider>) {
        if let Some(sitio) = self
            .proveedores
            .iter()
            .position(|p| p.id() == provider.id())
        {
            tracing::warn!("se sustituyó un proveedor de IA ya registrado");
            self.proveedores[sitio] = provider;
        } else {
            self.proveedores.push(provider);
        }
    }

    /// Quita el proveedor con ese identificador, si lo hay. Devuelve si lo quitó.
    ///
    /// Lo necesita el proveedor externo: quien borra la dirección en
    /// Ajustes tiene que dejar de poder enviar **en ese momento**, no en el
    /// siguiente arranque.
    pub fn quitar(&mut self, id: &str) -> bool {
        let antes = self.proveedores.len();
        self.proveedores.retain(|p| p.id() != id);
        self.proveedores.len() != antes
    }

    /// Si el subsistema está apagado del todo.
    #[must_use]
    pub const fn esta_apagado(&self) -> bool {
        self.apagado
    }

    /// Los proveedores registrados, para que la interfaz pueda enseñarlos.
    ///
    /// Los devuelve **estén listos o no**: un proveedor que hoy no puede atender
    /// tiene que poder verse con su motivo, y esconderlo dejaría al usuario sin
    /// saber que existe.
    #[must_use]
    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.proveedores.iter().map(|p| info(p.as_ref())).collect()
    }

    /// Qué hay para esta capacidad, con esta elección.
    #[must_use]
    pub fn availability(&self, capability: Capability, choice: &Choice) -> Availability {
        if self.apagado {
            return Availability::Absent(Absence::Disabled);
        }

        match self.elegir(capability, choice) {
            Ok(proveedor) => proveedor.readiness(),
            Err(ausencia) => Availability::Absent(ausencia),
        }
    }

    /// Genera texto con quien corresponda.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] cuando no hay quien atienda. Es un
    /// error aquí y un valor en [`Self::availability`] a propósito: preguntar si
    /// se puede es una consulta, y pedirlo cuando no se puede es un fallo de
    /// quien no preguntó.
    pub fn generate(&self, request: &Request, choice: &Choice) -> NbResult<Completion> {
        let proveedor = self.listo(request.capability, choice)?;
        self.sin_tumbar("generate", || proveedor.generate(request))
    }

    /// Caracteres de documento que admite quien atendería una capacidad, si hay
    /// quien la atienda.
    #[must_use]
    pub fn input_chars(&self, capability: Capability, choice: &Choice) -> Option<usize> {
        self.listo(capability, choice).ok().map(|p| p.input_chars())
    }

    /// Genera vectores con quien corresponda.
    ///
    /// # Errors
    ///
    /// [`NbError::ComponentUnavailable`] cuando no hay quien atienda.
    pub fn embed(&self, texts: &[String], choice: &Choice) -> NbResult<Vec<Embedding>> {
        let proveedor = self.listo(Capability::Embedding, choice)?;
        self.sin_tumbar("embed", || proveedor.embed(texts))
    }

    /// Ejecuta una llamada a un proveedor **sin que su pánico salga de aquí**.
    ///
    /// Es el mismo patrón que `reader::session::sin_tumbar`, y existe por la
    /// misma razón: el perfil de publicación evita `panic = "abort"`, así que un
    /// pánico desenrolla en lugar de matar el proceso, pero desenrollar por sí
    /// solo no basta. Sin nadie que lo recoja, sube por el comando del puente y
    /// quien lo pidió se queda esperando una respuesta que no llega.
    ///
    /// Aquí importa más que en el lector, porque un motor de inferencia es
    /// código nativo de terceros cargado por FFI y porque el
    /// innegociable dice que el fallo de un componente no puede tumbar a otro.
    /// Un resumen que no sale no puede llevarse por delante la lectura.
    ///
    /// **No se propaga el mensaje del pánico**: puede llevar rutas o fragmentos
    /// del documento, y el contrato de errores y `telemetry::redact_path`
    /// prohíben que salgan.
    fn sin_tumbar<T>(
        &self,
        que: &'static str,
        operacion: impl FnOnce() -> NbResult<T>,
    ) -> NbResult<T> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operacion)) {
            Ok(resultado) => resultado,
            Err(_) => {
                tracing::error!(operacion = que, "un proveedor de IA entró en pánico");
                if let Some(monitor) = &self.monitor {
                    // `Failed` y no una ausencia: un motor que se ha caído no
                    // está "no soportado" ni "sin instalar". Está roto, y son
                    // tres cosas distintas para quien las lee.
                    monitor.set(
                        crate::monitor::components::AI,
                        ComponentState::Failed,
                        Some("AI_PROVIDER_CRASH"),
                    );
                }
                Err(NbError::ComponentUnavailable)
            }
        }
    }

    /// Cómo se le cuenta al monitor de salud, con su motivo canónico.
    ///
    /// Lo calcula el servicio y no lo fija el arranque a mano. Hasta el
    /// 2026-09-12 `lib.rs` declaraba `ai` como `NotInstalled` con motivo
    /// `NOT_IMPLEMENTED` en cada arranque, y la interfaz lo creía: `salud.ts`
    /// convierte `NOT_INSTALLED` en un aviso **accionable**, así que la
    /// aplicación llevaba una tarjeta permanente invitando a instalar un motor
    /// que no existía y que nadie podía traer.
    #[must_use]
    pub fn component_state(&self) -> (ComponentState, Option<&'static str>) {
        if self.apagado {
            let ausencia = Absence::Disabled;
            return (ausencia.component_state(), Some(ausencia.reason_code()));
        }

        // Con proveedores registrados, el estado lo decide si alguno puede
        // atender algo: uno que se declara no soportado no hace al subsistema
        // utilizable.
        let alguno_listo = self.proveedores.iter().any(|p| p.readiness().is_ready());

        if alguno_listo {
            (ComponentState::Ready, None)
        } else if self.proveedores.is_empty() {
            // Sin ningún proveedor no hay nada que arreglar todavía, así que no
            // se anuncia: es la regla de la clase "Ajeno" de
            // `component-model.md`.
            let ausencia = Absence::Disabled;
            (ausencia.component_state(), Some(ausencia.reason_code()))
        } else {
            // Los hay y ninguno puede. El motivo del primero es el que se
            // publica, que es más útil que un "no disponible" sin causa.
            let ausencia = self
                .proveedores
                .iter()
                .find_map(|p| match p.readiness() {
                    Availability::Absent(motivo) => Some(motivo),
                    Availability::Ready(_) => None,
                })
                .unwrap_or(Absence::NotInstalled);
            (ausencia.component_state(), Some(ausencia.reason_code()))
        }
    }

    /// El proveedor elegido, ya comprobado que puede atender.
    fn listo(&self, capability: Capability, choice: &Choice) -> NbResult<&dyn AiProvider> {
        if self.apagado {
            return Err(NbError::ComponentUnavailable);
        }

        let proveedor = self
            .elegir(capability, choice)
            .map_err(|_| NbError::ComponentUnavailable)?;

        if proveedor.readiness().is_ready() {
            Ok(proveedor)
        } else {
            Err(NbError::ComponentUnavailable)
        }
    }

    /// A quién le toca, sin mirar si puede.
    ///
    /// **Aquí vive "local primero".** Con [`Choice::Auto`] solo se consideran los
    /// proveedores locales, así que ningún camino automático saca texto del
    /// dispositivo. Salir exige [`Choice::Provider`], que es lo que la interfaz
    /// escribe únicamente cuando el usuario ha elegido.
    fn elegir(&self, capability: Capability, choice: &Choice) -> Result<&dyn AiProvider, Absence> {
        let mut candidatos = self
            .proveedores
            .iter()
            .map(std::convert::AsRef::as_ref)
            .filter(|p| p.capabilities().contains(&capability));

        match choice {
            Choice::Auto => candidatos
                .filter(|p| p.placement() == Placement::Local)
                .find(|p| p.readiness().is_ready())
                .ok_or(Absence::Disabled),
            Choice::Provider(id) => candidatos
                .find(|p| p.id() == id)
                .ok_or(Absence::NotInstalled),
        }
    }
}

fn info(provider: &dyn AiProvider) -> ProviderInfo {
    ProviderInfo {
        id: provider.id().to_owned(),
        label: provider.label().to_owned(),
        placement: provider.placement(),
        host: provider.host().map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un proveedor de mentira con la conducta declarada, que es lo que hace
    /// falta para comprobar el contrato sin ningún motor detrás.
    struct Falso {
        id: &'static str,
        placement: Placement,
        capabilities: Vec<Capability>,
        readiness: Availability,
    }

    impl Falso {
        fn local(id: &'static str) -> Self {
            Self {
                id,
                placement: Placement::Local,
                capabilities: vec![Capability::Summary, Capability::Embedding],
                readiness: Availability::Ready(ProviderInfo {
                    id: id.to_owned(),
                    label: id.to_owned(),
                    placement: Placement::Local,
                    host: None,
                }),
            }
        }

        fn externo(id: &'static str) -> Self {
            Self {
                id,
                placement: Placement::External,
                capabilities: vec![Capability::Summary],
                readiness: Availability::Ready(ProviderInfo {
                    id: id.to_owned(),
                    label: id.to_owned(),
                    placement: Placement::External,
                    host: Some("ejemplo.invalid".to_owned()),
                }),
            }
        }

        fn ausente(id: &'static str, motivo: Absence) -> Self {
            Self {
                id,
                placement: Placement::Local,
                capabilities: vec![Capability::Summary],
                readiness: Availability::Absent(motivo),
            }
        }
    }

    impl AiProvider for Falso {
        fn id(&self) -> &str {
            self.id
        }

        fn label(&self) -> &str {
            self.id
        }

        fn placement(&self) -> Placement {
            self.placement
        }

        fn capabilities(&self) -> &[Capability] {
            &self.capabilities
        }

        fn host(&self) -> Option<&str> {
            match self.placement {
                Placement::Local => None,
                _ => Some("ejemplo.invalid"),
            }
        }

        fn readiness(&self) -> Availability {
            self.readiness.clone()
        }

        fn generate(&self, request: &Request) -> NbResult<Completion> {
            Ok(Completion {
                text: format!("resumen de {} pasajes", request.passages.len()),
                grounding: request.passages.iter().map(|p| p.locator.clone()).collect(),
                provider_id: self.id.to_owned(),
            })
        }

        fn embed(&self, texts: &[String]) -> NbResult<Vec<Embedding>> {
            Ok(texts.iter().map(|_| vec![0.0_f32; 4]).collect())
        }
    }

    fn peticion(capability: Capability) -> Request {
        Request {
            capability,
            instruction: "resume".to_owned(),
            passages: Vec::new(),
            task: String::new(),
            language: None,
        }
    }

    /// La mitad del criterio de aceptación: un servicio sin nada detrás se
    /// construye, se consulta y responde que no hay, sin entrar en pánico y sin
    /// devolver un error que alguien tenga que tratar.
    #[test]
    fn un_servicio_sin_proveedores_responde_que_no_hay_en_vez_de_fallar() {
        let servicio = AiService::new();

        assert_eq!(
            servicio.availability(Capability::Summary, &Choice::Auto),
            Availability::Absent(Absence::Disabled)
        );
        assert!(servicio.providers().is_empty());
    }

    /// Apagar el subsistema entero no puede ser distinguible de fuera de tenerlo
    /// vacío: las dos son ausencia y ninguna es una avería.
    #[test]
    fn un_servicio_apagado_no_ofrece_ninguna_capacidad() {
        let mut servicio = AiService::apagado();
        servicio.register(Box::new(Falso::local("local")));

        for capacidad in [
            Capability::Summary,
            Capability::Flashcards,
            Capability::Questions,
            Capability::Concepts,
            Capability::Embedding,
        ] {
            assert_eq!(
                servicio.availability(capacidad, &Choice::Auto),
                Availability::Absent(Absence::Disabled),
                "una capacidad se ofrecía con el subsistema apagado"
            );
        }

        assert!(servicio
            .generate(&peticion(Capability::Summary), &Choice::Auto)
            .is_err());
        assert!(servicio.embed(&["hola".to_owned()], &Choice::Auto).is_err());
    }

    /// La regla 2 de `philosophy.md`, escrita como prueba: con un proveedor
    /// externo perfectamente listo y ninguno local, el camino automático **no lo
    /// usa**. Si esta prueba deja de pasar, el producto habrá empezado a sacar
    /// texto del dispositivo sin que nadie lo decida.
    #[test]
    fn un_proveedor_externo_no_se_usa_sin_que_lo_elijan() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::externo("nube")));

        assert_eq!(
            servicio.availability(Capability::Summary, &Choice::Auto),
            Availability::Absent(Absence::Disabled),
            "el camino automático salió del dispositivo"
        );
        assert!(servicio
            .generate(&peticion(Capability::Summary), &Choice::Auto)
            .is_err());

        // Y nombrándolo sí, que es lo que hace la interfaz cuando el usuario lo
        // ha elegido.
        let elegido = Choice::Provider("nube".to_owned());
        assert!(servicio
            .availability(Capability::Summary, &elegido)
            .is_ready());
        assert!(servicio
            .generate(&peticion(Capability::Summary), &elegido)
            .is_ok());
    }

    /// Un proveedor local sí se usa solo: "local primero" no significa "local
    /// nunca".
    #[test]
    fn un_proveedor_local_atiende_el_camino_automatico() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::local("motor")));

        assert!(servicio
            .availability(Capability::Summary, &Choice::Auto)
            .is_ready());
        let salida = servicio
            .generate(&peticion(Capability::Summary), &Choice::Auto)
            .expect("el proveedor local atiende");
        assert_eq!(salida.provider_id, "motor");
    }

    /// Pedir algo que el proveedor no declara no puede acabar en el proveedor:
    /// el servicio no enruta una capacidad que nadie ofrece.
    #[test]
    fn una_capacidad_que_nadie_declara_no_llega_a_ningun_proveedor() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::externo("nube")));

        let elegido = Choice::Provider("nube".to_owned());
        assert_eq!(
            servicio.availability(Capability::Embedding, &elegido),
            Availability::Absent(Absence::NotInstalled)
        );
        assert!(servicio.embed(&["hola".to_owned()], &elegido).is_err());
    }

    /// Los tres motivos de ausencia piden tres acciones distintas a la interfaz,
    /// así que tienen que llegar distintos al monitor. Si dos colapsaran en el
    /// mismo estado, la interfaz no podría decidir entre ocultar e invitar.
    #[test]
    fn cada_motivo_de_ausencia_llega_al_monitor_con_su_propio_estado() {
        assert_eq!(
            Absence::Disabled.component_state(),
            ComponentState::Stopped,
            "un subsistema apagado se anunciaría como algo que falta"
        );
        assert_eq!(
            Absence::NotInstalled.component_state(),
            ComponentState::NotInstalled
        );
        assert_eq!(
            Absence::NotSupported.component_state(),
            ComponentState::NotSupported
        );
        // Un tope que eligió el usuario no pide atención: se ve en Ajustes.
        assert_eq!(
            Absence::OverBudget.component_state(),
            ComponentState::Stopped
        );

        let codigos = [
            Absence::Disabled.reason_code(),
            Absence::NotInstalled.reason_code(),
            Absence::NotSupported.reason_code(),
            Absence::OverBudget.reason_code(),
        ];
        let unicos: std::collections::BTreeSet<_> = codigos.iter().collect();
        assert_eq!(
            unicos.len(),
            codigos.len(),
            "dos motivos con el mismo código"
        );
        for codigo in codigos {
            assert!(
                codigo.chars().all(|c| c.is_ascii_uppercase() || c == '_'),
                "el motivo {codigo} no es un identificador canónico"
            );
        }
    }

    /// El trabajo de fondo del proveedor local espera con batería baja o calor, y
    /// el de uno externo no: no gasta la batería ni calienta este equipo. Y sin
    /// suspensión, no espera nadie.
    #[test]
    fn solo_el_trabajo_de_fondo_local_espera_a_la_bateria_y_la_temperatura() {
        let externo = Choice::Provider("nube".to_owned());

        let mut caliente =
            AiService::new().con_sonda_de_suspension(|| Some(presupuesto::Suspension::Temperatura));
        caliente.register(Box::new(Falso::local("motor")));
        caliente.register(Box::new(Falso::externo("nube")));
        assert_eq!(
            caliente.pausa_de_fondo(Capability::Summary, &Choice::Auto),
            Some(presupuesto::Suspension::Temperatura)
        );
        assert_eq!(caliente.pausa_de_fondo(Capability::Summary, &externo), None);
        // Y lo interactivo no se niega: la suspensión es del trabajo de fondo.
        assert!(caliente
            .generate(&peticion(Capability::Summary), &Choice::Auto)
            .is_ok());

        let mut enchufado = AiService::new().con_sonda_de_suspension(|| None);
        enchufado.register(Box::new(Falso::local("motor")));
        assert_eq!(
            enchufado.pausa_de_fondo(Capability::Summary, &Choice::Auto),
            None
        );
    }

    /// Un servicio vacío no puede publicarse como `NotInstalled`: eso es lo que
    /// hacía que la aplicación llevara una tarjeta permanente ofreciendo
    /// instalar un motor inexistente.
    #[test]
    fn un_servicio_sin_proveedores_no_se_anuncia_como_instalable() {
        let (estado, motivo) = AiService::new().component_state();

        assert_eq!(estado, ComponentState::Stopped);
        assert_eq!(motivo, Some("AI_DISABLED"));
        assert!(!estado.is_usable());
    }

    /// Con un proveedor que sí puede, el subsistema se declara listo. Sin esto,
    /// la interfaz seguiría ocultando funciones que ya existen, que es el
    /// defecto contrario y el que tuvo `search` antes.
    #[test]
    fn un_proveedor_listo_pone_el_subsistema_en_marcha() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::local("motor")));

        let (estado, motivo) = servicio.component_state();
        assert_eq!(estado, ComponentState::Ready);
        assert_eq!(motivo, None);
        assert!(estado.is_usable());
    }

    /// Un proveedor que no puede atender publica **su** motivo, no uno genérico:
    /// "el equipo no da para ello" y "falta descargarlo" piden cosas opuestas.
    #[test]
    fn un_proveedor_que_no_puede_publica_su_propio_motivo() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::ausente("motor", Absence::NotSupported)));

        let (estado, motivo) = servicio.component_state();
        assert_eq!(estado, ComponentState::NotSupported);
        assert_eq!(motivo, Some("AI_NOT_SUPPORTED"));
    }

    /// Registrar dos veces el mismo identificador sustituye en lugar de
    /// duplicar. Con duplicados, `providers()` enseñaría dos veces el mismo
    /// proveedor y la elección por identificador sería ambigua.
    #[test]
    fn registrar_el_mismo_identificador_sustituye_en_vez_de_duplicar() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::local("motor")));
        servicio.register(Box::new(Falso::ausente("motor", Absence::NotInstalled)));

        assert_eq!(servicio.providers().len(), 1);
        assert_eq!(
            servicio.availability(Capability::Summary, &Choice::Auto),
            Availability::Absent(Absence::Disabled)
        );
    }

    /// Quién sale del dispositivo lo decide el contrato y no cada proveedor: si
    /// lo decidiera él, uno externo podría declararse local y llevarse el texto
    /// sin pedir consentimiento.
    #[test]
    fn solo_lo_local_se_queda_en_el_dispositivo() {
        assert!(!Placement::Local.sale_del_dispositivo());
        assert!(Placement::External.sale_del_dispositivo());
    }

    /// La procedencia de una salida generada no es opcional: sin ella, la
    /// interfaz no puede cumplir la regla 4 y distinguir lo generado de lo
    /// escrito por una persona.
    #[test]
    fn toda_salida_generada_dice_quien_la_generó() {
        let mut servicio = AiService::new();
        servicio.register(Box::new(Falso::local("motor")));

        let salida = servicio
            .generate(&peticion(Capability::Summary), &Choice::Auto)
            .expect("atiende");

        assert!(!salida.provider_id.is_empty());
    }
}
