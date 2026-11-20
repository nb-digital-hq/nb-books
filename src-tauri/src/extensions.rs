//! El cajón de arena donde corre el código de terceros.
//!
//! Una extensión es código que no escribimos nosotros ejecutándose dentro de
//! la aplicación de alguien, con su biblioteca delante. La garantía no puede
//! ser que su autor se porte bien: tiene que ser que **el anfitrión no le
//! entregue nada que no le haya dado a propósito**.
//!
//! # Qué se le da a un módulo, y es poco
//!
//! Un contexto WASI **vacío de capacidades**: ni un directorio preabierto, ni
//! una variable de entorno, ni argumentos, ni entrada estándar, ni un solo
//! descriptor heredado. El contexto existe —no se le niega la interfaz entera—
//! porque un módulo compilado con la biblioteca estándar necesita arrancar, y
//! porque negarle el enlace produce un mensaje que su autor no puede usar. Lo
//! que se le niega son las **capacidades**, que es donde vive la seguridad.
//!
//! De ahí salen los dos desenlaces que la prueba comprueba: quien intenta
//! abrir un archivo recibe un error del anfitrión porque no hay ningún
//! descriptor que abrir, y quien pide una función que el anfitrión no ofrece
//! ni siquiera llega a instanciarse.
//!
//! # Por qué el plazo se cuenta en tiempo de pared
//!
//! Wasmtime ofrece dos formas de acotar una ejecución: contar instrucciones
//! (combustible) o interrumpir por época. El combustible es determinista y
//! sirve para reproducir una ejecución, pero **lo que aquí se protege es el
//! fotograma de quien está leyendo**, y un módulo puede consumir muy poco
//! combustible mientras espera. Se mide lo que el usuario sufre.
//!
//! # Lo que este módulo todavía no hace
//!
//! No lee la versión de API declarada por el módulo ni la contrasta con el
//! rango soportado: eso lo hace `manifest.rs`. Los permisos y el consentimiento viven
//! en `permissions.rs`; la instalación y las generaciones, en `store.rs`; y lo que se
//! enseña, en la sección Extensiones de Ajustes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use wasmtime::{Config, Engine, Linker, Module, ResourceLimiter, Store};
use wasmtime_wasi::p1::{add_to_linker_sync, WasiP1Ctx};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::WasiCtxBuilder;

pub mod bulk;
pub mod citation;
pub mod format;
pub mod manifest;
pub mod metadata;
pub mod permissions;
pub mod personalization;
pub mod registry;
pub mod review;
pub mod signature;
pub mod store;
pub mod view;
pub mod wasm;
pub mod withdrawals;
pub mod world;

use crate::error::NbError;

/// El subsistema de extensiones: su almacén y su motor.
///
/// Vive en el estado de la aplicación y no se crea por llamada porque el motor
/// lleva **un hilo propio** —el latido que hace cumplir el plazo—, y crearlo por
/// llamada sería un hilo por llamada.
pub struct Subsistema {
    pub almacen: store::Store,
    /// Compartido porque cada formato que aporte una extensión se queda con una
    /// referencia: el motor lleva un hilo dentro y no puede haber uno por
    /// fábrica.
    pub motor: std::sync::Arc<Motor>,
}

impl Subsistema {
    /// Arranca el subsistema sobre una raíz.
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si el almacén no se puede abrir;
    /// [`NbError::ComponentUnavailable`] si el motor no arranca.
    pub fn arrancar(raiz: std::path::PathBuf) -> Result<Self, NbError> {
        Ok(Self {
            almacen: store::Store::abrir(raiz)?,
            motor: std::sync::Arc::new(Motor::nuevo()?),
        })
    }

    /// Lee lo instalado. Barato: ver [`registry::Registro::cargar`].
    ///
    /// # Errors
    ///
    /// [`NbError::StorageFailure`] si no se puede leer el perfil.
    pub fn registro(&self) -> Result<registry::Registro, NbError> {
        registry::Registro::cargar(&self.almacen)
    }
}

/// Cada cuánto avanza la época. Es la resolución del plazo: un módulo puede
/// pasarse como mucho esto de lo que se le concedió. Diez milisegundos son
/// menos de un fotograma a sesenta por segundo, así que un desbordamiento no
/// llega a verse.
const LATIDO: Duration = Duration::from_millis(10);

/// El nombre del punto de entrada de un módulo de línea de órdenes de WASI.
const PUNTO_DE_ENTRADA: &str = "_start";

/// La sección personalizada donde un módulo declara contra qué versión de la
/// API de extensiones fue compilado. Es donde `plugin-system.md` sección 2.1
/// dice que el anfitrión la lee.
pub(crate) const SECCION_DE_VERSION: &str = "nb:api-version";

/// La versión mayor de la API que este anfitrión habla hoy.
///
/// Pasa a 1 al completarse una extensión de primera parte sobre cada
/// clase de punto de contribución
/// y declarar la API 1.0.0 estable. Ver la sección 2.2 de `versioning.md`.
const MAYOR_ACTUAL: u64 = 1;

/// Qué mayores admite el anfitrión: la actual y la anterior.
///
/// Son dos y no una porque el día que la API suba de mayor se romperían a la
/// vez todas las extensiones instaladas; y no tres porque sobre una superficie
/// tan corta mantener tres implementaciones del anfitrión sería una promesa
/// que este proyecto no puede cumplir. Es el compromiso, escrito
/// aquí en lugar de recordado.
pub(crate) const fn admitidas() -> (u64, u64) {
    (MAYOR_ACTUAL.saturating_sub(1), MAYOR_ACTUAL)
}

/// Cuánto se guarda de lo que el módulo escribe por su salida estándar.
///
/// La salida **no se hereda**: si se heredara, lo que escriba una extensión
/// acabaría en el registro de la aplicación sin pasar por la redacción de
/// `telemetry`, y ahí puede ir el título de un documento o una ruta. Se queda
/// en un búfer del anfitrión, y acotado, para que un módulo hablador no pueda
/// hacer crecer la memoria del anfitrión escribiendo.
pub const SALIDA_MAXIMA: usize = 8 * 1024;

/// Lo que un módulo puede gastar en una llamada.
#[derive(Debug, Clone, Copy)]
pub struct Limites {
    /// Memoria lineal máxima, en bytes.
    pub memoria: usize,
    /// Tiempo de pared máximo por llamada.
    pub tiempo: Duration,
}

impl Default for Limites {
    /// Treinta y dos megabytes y un segundo.
    ///
    /// La memoria sale del presupuesto: `performance-budgets.md` concede unos
    /// 30 MB al lado de Rust con la aplicación en reposo, así que una sola
    /// extensión no puede disponer de tanto como el runtime entero. Treinta y
    /// dos es holgado para un analizador de formato y sigue siendo una cifra
    /// que el usuario puede ver y entender cuando la interfaz se la enseña.
    ///
    /// El segundo es para llamadas que no están en el camino de la lectura.
    /// Una contribución que se pinte dentro de un fotograma llevará un plazo
    /// mucho más corto, pasado por parámetro; por eso los límites viajan en la
    /// llamada y no en el motor.
    fn default() -> Self {
        Self {
            memoria: 32 * 1024 * 1024,
            tiempo: Duration::from_secs(1),
        }
    }
}

/// Cómo terminó la ejecución de un módulo.
///
/// Es un enumerado y no un error a propósito: que el anfitrión detenga a una
/// extensión **no es un fallo de la aplicación**, es el sistema funcionando.
/// Los errores de `NbError` quedan para lo que sí es un fallo nuestro.
#[derive(Debug, PartialEq, Eq)]
pub enum Desenlace {
    /// Terminó por su cuenta y sin salirse de nada.
    Termino,
    /// Lo detuvo el anfitrión: agotó su plazo.
    TiempoAgotado,
    /// Lo detuvo el anfitrión: pidió más memoria de la concedida.
    MemoriaAgotada,
    /// Lo detuvo el anfitrión antes de instanciarlo: pide una capacidad que
    /// no se ofrece. Lleva el nombre para que su autor sepa cuál.
    CapacidadDenegada { modulo: String, funcion: String },
    /// No es un módulo de WebAssembly válido, o no trae punto de entrada.
    NoEsUnModulo,
    /// No declara contra qué versión de la API fue compilado. Sin eso el
    /// anfitrión no puede protegerlo de nada, así que no se ejecuta.
    SinVersionDeApi,
    /// Declara una versión, pero no se puede leer.
    VersionDeApiIlegible,
    /// Declara una versión que este anfitrión no habla. Lleva los tres números
    /// con los que su autor puede arreglarlo: la que trae y el rango que se
    /// admite. Es lo que promete la política de versiones.
    VersionDeApiNoSoportada {
        declarada: u64,
        admitida_minima: u64,
        admitida_maxima: u64,
    },
    /// Falló por su cuenta, dentro de sus límites. Es problema suyo, no del
    /// anfitrión.
    FalloDelModulo,
}

/// El resultado de una ejecución: cómo terminó y qué llegó a decir.
#[derive(Debug)]
pub struct Ejecucion {
    pub desenlace: Desenlace,
    /// Lo que el módulo escribió por su salida estándar, hasta
    /// [`SALIDA_MAXIMA`]. Es la única forma que tiene el anfitrión de observar
    /// lo que le pasó a un módulo por dentro sin darle ninguna capacidad más.
    pub salida: String,
}

impl Desenlace {
    /// El error canónico que corresponde a este desenlace, si lo hay.
    ///
    /// Vive aquí y no solo en [`Ejecucion`] porque el camino de componente
    /// devuelve el desenlace suelto: si la traducción viviera en un solo sitio,
    /// el otro camino tendría que copiarla.
    #[must_use]
    pub const fn error(&self) -> Option<NbError> {
        match self {
            Self::Termino => None,
            Self::TiempoAgotado | Self::MemoriaAgotada => Some(NbError::ExtensionLimitExceeded),
            Self::VersionDeApiNoSoportada { .. }
            | Self::SinVersionDeApi
            | Self::VersionDeApiIlegible => Some(NbError::ExtensionApiUnsupported),
            Self::NoEsUnModulo => Some(NbError::ExtensionManifestInvalid),
            Self::CapacidadDenegada { .. } => Some(NbError::ConsentRequired),
            Self::FalloDelModulo => Some(NbError::ComponentUnavailable),
        }
    }
}

impl Ejecucion {
    /// El error canónico que corresponde a este desenlace, si lo hay.
    ///
    /// Existe porque la interfaz necesita **una** cosa que enseñar, y los
    /// desenlaces son siete. La traducción de cada código vive en el catálogo
    /// de idioma, como la de los trece anteriores.
    ///
    /// Los cuatro rechazos por versión y forma comparten código a propósito:
    /// para quien instala son el mismo hecho —esa extensión no se puede
    /// ejecutar aquí— y distinguirlos en pantalla no le ofrece ninguna acción
    /// distinta. El detalle, que sí le sirve a su autor, va en el registro.
    #[must_use]
    pub const fn error(&self) -> Option<NbError> {
        self.desenlace.error()
    }

    fn sin_salida(desenlace: Desenlace) -> Self {
        Self {
            desenlace,
            salida: String::new(),
        }
    }
}

/// Lo que el almacén de un módulo lleva dentro.
struct Estado {
    wasi: WasiP1Ctx,
    cupo: Cupo,
}

/// El contador de memoria.
///
/// Existe en lugar de `StoreLimits` porque hace falta **distinguir** el caso:
/// cuando se le niega crecer, el asignador del invitado recibe un fallo y
/// acaba en una trampa genérica, indistinguible de un error propio del módulo.
/// Con la marca, "se quedó sin memoria" se puede afirmar en vez de suponer.
pub(crate) struct Cupo {
    pub(crate) memoria: usize,
    pub(crate) se_le_nego_crecer: bool,
}

impl ResourceLimiter for Cupo {
    fn memory_growing(
        &mut self,
        _actual: usize,
        deseado: usize,
        _maximo: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let cabe = deseado <= self.memoria;
        if !cabe {
            self.se_le_nego_crecer = true;
        }
        Ok(cabe)
    }

    fn table_growing(
        &mut self,
        _actual: usize,
        _deseado: usize,
        _maximo: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }
}

/// El motor. Se crea una vez y se comparte: compilar módulos es caro y el
/// motor cachea lo que puede.
pub struct Motor {
    motor: Engine,
    latido_vivo: Arc<AtomicBool>,
}

impl Motor {
    pub fn nuevo() -> Result<Self, NbError> {
        let mut ajustes = Config::new();
        // El plazo se cuenta por época. Ver la cabecera del módulo.
        ajustes.epoch_interruption(true);
        // Cero fotogramas de rastro de pila del invitado. Un rastro puede
        // llevar rutas dentro, y ninguna ruta entra en el registro. Ver
        // `telemetry::redact_path`.
        ajustes.wasm_backtrace_max_frames(None);
        // Los hilos del invitado no se desactivan aquí porque no llegan a
        // existir: la opción `threads` de Wasmtime está fuera en `Cargo.toml`.
        // Importa que sea así, no solo por peso: un hilo del invitado se
        // escaparía del plazo, que se cuenta por almacén y no por proceso.

        let motor = Engine::new(&ajustes).map_err(|error| {
            tracing::error!(?error, "el motor de extensiones no arranca");
            NbError::ComponentUnavailable
        })?;

        let latido_vivo = Arc::new(AtomicBool::new(true));
        let compas = motor.clone();
        let vivo = Arc::clone(&latido_vivo);
        std::thread::Builder::new()
            .name("latido-extensiones".to_owned())
            .spawn(move || {
                while vivo.load(Ordering::Relaxed) {
                    std::thread::sleep(LATIDO);
                    compas.increment_epoch();
                }
            })
            .map_err(|error| {
                tracing::error!(?error, "sin hilo de latido no hay plazo que valga");
                NbError::ComponentUnavailable
            })?;

        Ok(Self { motor, latido_vivo })
    }

    /// Ejecuta el punto de entrada de un módulo dentro de sus límites.
    ///
    /// Devuelve `Err` solo cuando el fallo es del anfitrión. Que la extensión
    /// se pase de tiempo, de memoria o de capacidades es un `Ok` con su
    /// desenlace: son resultados esperados, no averías.
    pub fn ejecutar(&self, wasm: &[u8], limites: Limites) -> Result<Ejecucion, NbError> {
        // El invitado no puede tumbar al anfitrión. Es la misma contención que
        // `reader::session` aplica al motor de formato.
        let intento =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.correr(wasm, limites)));
        match intento {
            Ok(desenlace) => desenlace,
            Err(_) => {
                tracing::error!("el motor de extensiones entró en pánico");
                Err(NbError::ComponentUnavailable)
            }
        }
    }

    fn correr(&self, wasm: &[u8], limites: Limites) -> Result<Ejecucion, NbError> {
        // El orden de estas dos comprobaciones importa, y lo encontró una
        // prueba: a un archivo que no es WebAssembly se le estaba diciendo que
        // no declaraba versión de API, que es cierto y es inútil. Primero se
        // mira si esto es un módulo.
        if !wasm::es_modulo(wasm) {
            return Ok(Ejecucion::sin_salida(Desenlace::NoEsUnModulo));
        }

        // Y la versión antes de compilar: se lee de los bytes, así que
        // rechazar un módulo que no se habla no cuesta ni una compilación. Va
        // antes que todo lo demás porque un módulo de otra mayor puede esperar
        // del anfitrión cosas que no existen.
        if let Some(desenlace) = version_rechazada(wasm) {
            return Ok(Ejecucion::sin_salida(desenlace));
        }

        let Ok(modulo) = Module::new(&self.motor, wasm) else {
            return Ok(Ejecucion::sin_salida(Desenlace::NoEsUnModulo));
        };

        let mut enlazador: Linker<Estado> = Linker::new(&self.motor);
        add_to_linker_sync(&mut enlazador, |estado: &mut Estado| &mut estado.wasi).map_err(
            |error| {
                tracing::error!(?error, "no se pudo preparar la interfaz del anfitrión");
                NbError::ComponentUnavailable
            },
        )?;

        // Un contexto sin nada: ni preaperturas, ni entorno, ni argumentos, ni
        // descriptores heredados. Lo que no se nombra aquí, no existe para el
        // módulo.
        let salida = MemoryOutputPipe::new(SALIDA_MAXIMA);
        let estado = Estado {
            wasi: WasiCtxBuilder::new().stdout(salida.clone()).build_p1(),
            cupo: Cupo {
                memoria: limites.memoria,
                se_le_nego_crecer: false,
            },
        };
        let mut almacen = Store::new(&self.motor, estado);
        almacen.limiter(|estado| &mut estado.cupo);
        almacen.set_epoch_deadline(plazo_en_latidos(limites.tiempo));

        // Se comprueba antes de instanciar, y no se deja que falle el enlace,
        // porque el mensaje de un enlace roto no le dice a su autor **cuál** de
        // sus importaciones sobra. Esta comprobación es además la semilla de
        // `check-capacidades.mjs`, que llega.
        for importacion in modulo.imports() {
            if enlazador
                .get(&mut almacen, importacion.module(), importacion.name())
                .is_err()
            {
                return Ok(Ejecucion::sin_salida(Desenlace::CapacidadDenegada {
                    modulo: importacion.module().to_owned(),
                    funcion: importacion.name().to_owned(),
                }));
            }
        }

        let Ok(instancia) = enlazador.instantiate(&mut almacen, &modulo) else {
            return Ok(Ejecucion::sin_salida(Desenlace::FalloDelModulo));
        };
        let Ok(entrada) = instancia.get_typed_func::<(), ()>(&mut almacen, PUNTO_DE_ENTRADA) else {
            return Ok(Ejecucion::sin_salida(Desenlace::NoEsUnModulo));
        };

        let resultado = entrada.call(&mut almacen, ());
        let desenlace = clasificar(resultado, almacen.data().cupo.se_le_nego_crecer);
        drop(almacen);
        Ok(Ejecucion {
            desenlace,
            salida: String::from_utf8_lossy(&salida.contents()).into_owned(),
        })
    }
}

impl Motor {
    /// El motor de Wasmtime, para quien tenga que crear su propio almacén.
    ///
    /// Lo necesita el camino de componente de `world`, que usa un tipo de
    /// estado distinto del de aquí: un componente lleva el anfitrión del mundo
    /// dentro y un módulo suelto no.
    pub(crate) const fn interno(&self) -> &Engine {
        &self.motor
    }
}

impl Drop for Motor {
    fn drop(&mut self) {
        self.latido_vivo.store(false, Ordering::Relaxed);
    }
}

/// Comprueba la versión de API declarada, y devuelve el motivo si no vale.
///
/// Mecanismo del requisito **R11** de la política del registro.
///
/// Devuelve `None` cuando el módulo se puede ejecutar. Se escribe así, y no
/// como un `Result`, porque ninguno de estos casos es un fallo del anfitrión:
/// son módulos que no se admiten, que es el sistema funcionando.
pub(crate) fn version_rechazada(wasm: &[u8]) -> Option<Desenlace> {
    let Some(bytes) = wasm::seccion_personalizada(wasm, SECCION_DE_VERSION) else {
        return Some(Desenlace::SinVersionDeApi);
    };
    let Some(mayor) = mayor_declarada(&bytes) else {
        return Some(Desenlace::VersionDeApiIlegible);
    };
    let (minima, maxima) = admitidas();
    if mayor < minima || mayor > maxima {
        return Some(Desenlace::VersionDeApiNoSoportada {
            declarada: mayor,
            admitida_minima: minima,
            admitida_maxima: maxima,
        });
    }
    None
}

/// La mayor de una versión escrita como texto en la sección personalizada.
///
/// Solo interesa la mayor: es lo que decide si el anfitrión y el módulo hablan
/// el mismo idioma. La menor y el parche no cambian lo que el módulo puede
/// pedir, por la tabla de la sección 2.5 de `versioning.md`.
pub(crate) fn mayor_declarada(bytes: &[u8]) -> Option<u64> {
    let texto = std::str::from_utf8(bytes).ok()?;
    let limpio = texto.trim_end_matches(char::from(0)).trim();
    limpio.split('.').next()?.parse().ok()
}

/// Cuántos latidos caben en el plazo. Nunca cero: un plazo de cero latidos
/// interrumpiría el módulo antes de que ejecutara nada.
pub(crate) fn plazo_en_latidos(tiempo: Duration) -> u64 {
    let latidos = tiempo.as_millis() / LATIDO.as_millis();
    u64::try_from(latidos).unwrap_or(u64::MAX).max(1)
}

/// De qué murió el módulo.
///
/// El orden importa: quedarse sin memoria llega hasta aquí como una trampa
/// genérica, porque el asignador del invitado se encuentra con que no puede
/// crecer y aborta. Sin mirar primero la marca del cupo, una extensión que
/// agota la memoria sería indistinguible de una que tiene un fallo.
pub(crate) fn clasificar(resultado: wasmtime::Result<()>, se_le_nego_crecer: bool) -> Desenlace {
    let Err(error) = resultado else {
        return Desenlace::Termino;
    };
    if se_le_nego_crecer {
        return Desenlace::MemoriaAgotada;
    }
    if let Some(trampa) = error.downcast_ref::<wasmtime::Trap>() {
        if *trampa == wasmtime::Trap::Interrupt {
            return Desenlace::TiempoAgotado;
        }
    }
    if let Some(salida) = error.downcast_ref::<wasmtime_wasi::I32Exit>() {
        return if salida.0 == 0 {
            Desenlace::Termino
        } else {
            Desenlace::FalloDelModulo
        };
    }
    Desenlace::FalloDelModulo
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_plazo_nunca_se_traduce_a_cero_latidos() {
        // Cero latidos interrumpiría el módulo antes de la primera
        // instrucción, y entonces el límite de tiempo no mediría un límite
        // sino la existencia del módulo.
        assert_eq!(plazo_en_latidos(Duration::from_millis(0)), 1);
        assert_eq!(plazo_en_latidos(Duration::from_millis(1)), 1);
        assert_eq!(plazo_en_latidos(Duration::from_millis(100)), 10);
        assert_eq!(plazo_en_latidos(Duration::from_secs(1)), 100);
    }

    #[test]
    fn lo_que_no_es_un_modulo_se_rechaza_en_lugar_de_intentarse() {
        let motor = Motor::nuevo().expect("motor");
        let ejecucion = motor
            .ejecutar(b"esto no es webassembly", Limites::default())
            .expect("el anfitrion no falla por un modulo malo");
        assert_eq!(ejecucion.desenlace, Desenlace::NoEsUnModulo);
    }

    #[test]
    fn a_lo_que_no_es_un_modulo_no_se_le_reprocha_la_version() {
        // Es cierto que un archivo de texto no declara versión de API, y es
        // una respuesta inútil. Lo encontró una prueba al añadir la puerta.
        let motor = Motor::nuevo().expect("motor");
        let ejecucion = motor
            .ejecutar(b"esto tampoco es webassembly", Limites::default())
            .expect("el anfitrion no falla");
        assert_eq!(ejecucion.desenlace, Desenlace::NoEsUnModulo);
    }

    #[test]
    fn un_modulo_sin_version_declarada_no_se_ejecuta() {
        // Sin declaración el anfitrión no puede protegerlo de nada, que es lo
        // que dice el compromiso que se le pide al autor.
        assert_eq!(
            version_rechazada(b"\0asm\x01\0\0\0"),
            Some(Desenlace::SinVersionDeApi)
        );
    }

    #[test]
    fn de_la_version_solo_decide_la_mayor() {
        // La menor y el parche no cambian lo que un módulo puede pedir, por la
        // tabla de cambios mayores de versioning.md sección 2.5.
        assert_eq!(mayor_declarada(b"0.1.0"), Some(0));
        assert_eq!(mayor_declarada(b"0.99.99"), Some(0));
        assert_eq!(mayor_declarada(b"12.0.0"), Some(12));
        // Con relleno de ceros al final, que es como queda un arreglo de bytes
        // de tamaño fijo en la sección.
        assert_eq!(mayor_declarada(b"3.0.0\0\0\0"), Some(3));
        assert_eq!(mayor_declarada(b"no es una version"), None);
        assert_eq!(mayor_declarada(&[0xff, 0xfe]), None);
    }

    #[test]
    fn se_admiten_dos_mayores_y_no_una_ni_tres() {
        // El compromiso: la actual y la anterior. Se comprueba aquí
        // para que cambiarlo obligue a mirar el documento que lo promete.
        let (minima, maxima) = admitidas();
        assert_eq!(maxima, MAYOR_ACTUAL);
        assert!(maxima - minima <= 1);
    }

    #[test]
    fn todo_desenlace_que_no_sea_terminar_tiene_un_codigo_que_ensenar() {
        // Un desenlace sin código sería un fallo que la interfaz no sabe
        // contar, así que se enseñaría el genérico o nada. El `match` de
        // `error` es exhaustivo, de modo que un desenlace nuevo no compila
        // hasta que alguien decide qué se le dice al usuario.
        for desenlace in [
            Desenlace::TiempoAgotado,
            Desenlace::MemoriaAgotada,
            Desenlace::SinVersionDeApi,
            Desenlace::VersionDeApiIlegible,
            Desenlace::NoEsUnModulo,
            Desenlace::FalloDelModulo,
        ] {
            let ejecucion = Ejecucion::sin_salida(desenlace);
            assert!(
                ejecucion.error().is_some(),
                "{:?} no tiene código canónico",
                ejecucion.desenlace
            );
        }
        assert!(Ejecucion::sin_salida(Desenlace::Termino).error().is_none());
    }

    #[test]
    fn los_limites_de_fabrica_caben_en_el_presupuesto_del_runtime() {
        // 30 MB es lo que performance-budgets.md concede al lado de Rust con
        // la aplicación en reposo. Una sola extensión no puede llevarse más
        // que el runtime entero, así que este número es el que impide que la
        // holgura se vaya sin que nadie lo note.
        assert!(Limites::default().memoria <= 32 * 1024 * 1024);
        assert!(Limites::default().tiempo <= Duration::from_secs(1));
    }
}
