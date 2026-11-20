//! Qué tiene la máquina donde se está ejecutando esto.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 1.//! //!
//! # Por qué se pregunta, y por qué solo esto
//!
//! La sección 4 de ese contrato dice que sin hardware suficiente el componente
//! se declara `NOT_SUPPORTED` y **la interfaz oculta las funciones**, en lugar
//! de ofrecerlas y fallar. Para poder decir eso hay que saber qué hay, y lo que
//! de verdad decide es poco: cuánta memoria y cuántos núcleos. El resto -modelo
//! de procesador, extensiones vectoriales, versión del controlador- son cosas
//! que el motor ya mira por su cuenta, y duplicar esa lógica aquí produciría dos
//! respuestas para la misma pregunta.
//!
//! # Nada de dependencias nuevas para esto
//!
//! `libc` y `windows-sys` ya viajaban en el árbol -`libc` lo arrastra media
//! biblioteca estándar extendida, y `windows-sys` lo arrastra Tauri-, así que
//! declararlas no añade ningún crate al `Cargo.lock` ni ninguna licencia nueva.
//! Es el mismo criterio con el que entraron `sha2`, `flate2` y `ring`.
//!
//! # Lo que no se ha podido comprobar aquí
//!
//! La lectura de memoria de Windows está escrita y **no ejecutada**: en esta
//! máquina solo se compila para Linux. Se anota en
//! `docs/developer/testing/deferred-verification.md` en lugar de darse por
//! buena. La de Unix cubre Linux y macOS con la misma llamada, así que al menos
//! no son dos caminos distintos sin probar.

use std::fmt;

use candle_core::Device;

/// Dónde puede calcular el motor en esta máquina.
///
/// **Con Candle no hay backends que descargar**, y por eso este enum no tiene
/// ningún estado de "descargando" ni "no instalado": CUDA y Metal son opciones
/// de compilación, no binarios aparte. Es la consecuencia directa de la decisión
/// de motor del 2026-09-12, y lo que hace que la tercera entrega que pedía la
/// ficha -"descarga de backends acelerados bajo demanda"- deje de
/// existir en lugar de construirse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acelerador {
    /// El procesador, con un tope de hilos para no congelar el sistema.
    Cpu,
    /// GPU de NVIDIA. Solo si esta compilación trae la opción `cuda`.
    Cuda,
    /// GPU de Apple. Solo si esta compilación trae la opción `metal`.
    Metal,
}

impl fmt::Display for Acelerador {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nombre = match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
        };
        f.write_str(nombre)
    }
}

/// Lo que esta máquina tiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recursos {
    /// Memoria física total, en bytes.
    ///
    /// `None` cuando no se pudo averiguar. **No es lo mismo que cero**, y por
    /// eso no es un `u64` con el cero haciendo de bandera: mezclarlos haría que
    /// una máquina cuya memoria no supimos leer se tratara como una máquina sin
    /// memoria, y se le ocultarían funciones que sí puede ejecutar.
    pub memoria_total: Option<u64>,
    /// Hilos de ejecución disponibles. Nunca menor que uno.
    pub nucleos: usize,
    pub acelerador: Acelerador,
}

/// Memoria mínima para ofrecer inferencia local, en bytes: **2 GiB**.
///
/// No es un número redondo elegido por sonar bien. La aplicación en reposo mide
/// entre 281 y 318 MB (medido el 2026-09-11 y el 2026-09-12 sobre el
/// binario de release), y el modelo de embeddings del catálogo,
/// `granite-embedding-107m-multilingual`, pesa 214 MB en disco en `bfloat16` y
/// el doble cargado en coma flotante de 32 bits. En una máquina de 2 GiB eso ya
/// compite con el sistema operativo; por debajo, no compite: pierde.
///
/// **Corregido el 2026-09-16.** Aquí ponía que el modelo más pequeño
/// que sirve "ronda los 100 MB". Ese era `all-MiniLM-L6-v2`, que **solo sabe
/// inglés**, y la suposición venía de [ADR-007]. Sobre una biblioteca en español
/// no sirve, y los multilingües de la misma dimensión pesan entre 214 y 470 MB.
/// El suelo se mantiene porque sigue cabiendo; lo que estaba mal era el motivo.
///
/// Es el suelo **del motor**, no el de cada modelo. Cada entrada del catálogo
/// declara la suya en `catalogo::Modelo::memoria`.
///
/// [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md
pub const MEMORIA_MINIMA: u64 = 2 * 1024 * 1024 * 1024;

impl Recursos {
    /// Si esta máquina da para ejecutar algo que pide `minimo` bytes.
    ///
    /// # Qué se hace cuando no se pudo leer la memoria
    ///
    /// Se contesta que **sí**. Parece lo contrario de prudente y es lo prudente:
    /// `NOT_SUPPORTED` oculta funciones sin invitar a nada, así que responder
    /// que no ante una lectura fallida le quitaría la inferencia local a una
    /// máquina capaz por un fallo de medición, y sin ninguna forma de que su
    /// dueño se entere. Al revés el error es recuperable: se intenta, no cabe, y
    /// el componente pasa a `DEGRADED`, que sí se ve y se puede reintentar.
    #[must_use]
    pub const fn da_para(&self, minimo: u64) -> bool {
        match self.memoria_total {
            Some(total) => total >= minimo,
            None => true,
        }
    }
}

/// Qué hay en esta máquina.
#[must_use]
pub fn detectar() -> Recursos {
    Recursos {
        memoria_total: memoria_total(),
        nucleos: nucleos(),
        acelerador: acelerador(),
    }
}

/// Cuántos núcleos puede usar este proceso. Aparte de [`detectar`] porque el
/// presupuesto lo pide al guardar una preferencia, y no hay por qué
/// sondear aceleradores para eso.
#[must_use]
pub fn nucleos() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
}

/// Cuántos hilos puede usar la inferencia sin llevarse la máquina por delante.
///
/// La mitad de los núcleos, y como mínimo uno. El innegociable es que
/// la inferencia **nunca** degrade el presupuesto de fotograma de la lectura, y
/// un motor que toma todos los núcleos compite con el hilo que pinta. Es el valor
/// automático del presupuesto, que se puede cambiar en Ajustes y está medido en
/// `performance-budgets.md`: ver `presupuesto`.
#[must_use]
pub const fn hilos_para_inferencia(nucleos: usize) -> usize {
    if nucleos / 2 == 0 {
        1
    } else {
        nucleos / 2
    }
}

/// Dónde puede calcular el motor aquí, **preguntándoselo al motor**.
///
/// # Por qué se prueba en lugar de mirarlo en tiempo de compilación
///
/// La primera versión lo decidía con `#[cfg(feature = "cuda")]`, y eso tenía dos
/// problemas. El de fondo: una opción activada dice que el código está en el
/// binario, no que la máquina tenga la tarjeta ni el controlador, así que
/// contestar "cuda" por venir compilado sería prometer lo que no se puede
/// cumplir —es la misma distinción entre compilar y calcular que hay unas líneas
/// más abajo—. Y el práctico: declarar esas opciones en el manifiesto rompe
/// `cargo clippy --all-features`, que es parte de la verificación obligatoria,
/// porque `--all-features` las activa todas y `cuda` exige la cadena de NVIDIA y
/// `metal` exige macOS. Una opción que hace fallar la verificación en cualquier
/// máquina de desarrollo no es una opción: es una trampa.
///
/// Candle ya contesta esta pregunta bien: sin la opción compilada, abrir el
/// dispositivo devuelve un error en lugar de existir a medias. Así que se
/// intenta, y lo que responda es la verdad.
fn acelerador() -> Acelerador {
    if Device::new_cuda(0).is_ok() {
        return Acelerador::Cuda;
    }
    if Device::new_metal(0).is_ok() {
        return Acelerador::Metal;
    }
    Acelerador::Cpu
}

/// Memoria física total en bytes, o `None` si no se pudo leer.
#[cfg(unix)]
fn memoria_total() -> Option<u64> {
    // `_SC_PHYS_PAGES` por `_SC_PAGESIZE`. Es la misma llamada en Linux y en
    // macOS, así que las dos plataformas comparten camino en lugar de tener cada
    // una el suyo sin probar.
    //
    // SEGURIDAD: `sysconf` no recibe punteros ni memoria del proceso; solo lee
    // un parámetro del sistema y devuelve un entero. Un valor negativo significa
    // "no definido", y por eso se comprueba antes de convertir.
    let (paginas, tamano) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };

    if paginas <= 0 || tamano <= 0 {
        return None;
    }

    u64::try_from(paginas)
        .ok()
        .zip(u64::try_from(tamano).ok())
        .and_then(|(p, t)| p.checked_mul(t))
}

/// Memoria física total en bytes, o `None` si no se pudo leer.
///
/// **Escrita y no ejecutada:** aquí solo se compila para Linux. Ver el aviso del
/// encabezado del módulo.
#[cfg(windows)]
fn memoria_total() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    // SEGURIDAD: la estructura se inicializa entera a cero y se le pone su
    // propio tamaño, que es lo que la llamada exige para saber qué versión
    // recibe. Se le pasa un puntero a memoria de la pila que vive durante toda
    // la llamada.
    let mut estado = MEMORYSTATUSEX {
        dwLength: u32::try_from(std::mem::size_of::<MEMORYSTATUSEX>()).ok()?,
        ..unsafe { std::mem::zeroed() }
    };

    if unsafe { GlobalMemoryStatusEx(&raw mut estado) } == 0 {
        return None;
    }

    Some(estado.ullTotalPhys)
}

/// Memoria física total en bytes, o `None` si no se pudo leer.
#[cfg(not(any(unix, windows)))]
fn memoria_total() -> Option<u64> {
    None
}

/// Bytes libres en el sistema de archivos donde está `ruta`, para quien no es
/// administrador. `None` si no se pudo saber.
///
/// Existe: descargar doscientos megabytes y descubrir en el último
/// que no caben es gastar la red de quien la paga y dejarle el disco lleno. Se
/// mira antes de empezar.
///
/// Lo que **no** sustituye es comprobar la escritura: entre mirar y escribir
/// puede llenarse el disco por otro lado, y por eso la descarga sigue tratando un
/// fallo de escritura como tal y limpiando lo que dejó.
#[cfg(unix)]
#[must_use]
pub fn disco_libre(ruta: &std::path::Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;

    let c_ruta = std::ffi::CString::new(ruta.as_os_str().as_bytes()).ok()?;
    // SEGURIDAD: `statvfs` lee la ruta, que es una cadena C terminada en nulo y
    // viva durante toda la llamada, y escribe en una estructura de la pila
    // inicializada a cero. No retiene ninguno de los dos punteros.
    let mut datos: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_ruta.as_ptr(), &raw mut datos) } != 0 {
        return None;
    }
    // Los dos campos cambian de ancho entre Linux y macOS —en macOS `f_bavail`
    // es de 32 bits—, y `u64::from` los ensancha en los dos sin truncar. En
    // Linux ya son `u64`, y por eso Clippy ve la conversión inútil: lo es en
    // esa plataforma, no en las otras.
    #[allow(clippy::useless_conversion)]
    let bloques = u64::from(datos.f_bavail);
    #[allow(clippy::useless_conversion)]
    let tamano = u64::from(datos.f_frsize);
    bloques.checked_mul(tamano)
}

/// Bytes libres en el sistema de archivos donde está `ruta`, para quien no es
/// administrador. `None` si no se pudo saber.
///
/// **Escrita y no ejecutada:** aquí solo se compila para Linux. Ver V20 en
/// `deferred-verification.md`.
#[cfg(windows)]
#[must_use]
pub fn disco_libre(ruta: &std::path::Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let ancha: Vec<u16> = ruta.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut libres: u64 = 0;
    // SEGURIDAD: la ruta es UTF-16 terminada en nulo y vive durante la llamada;
    // los dos punteros nulos piden que no se rellenen esos datos, que la API
    // admite.
    let hecho = unsafe {
        GetDiskFreeSpaceExW(
            ancha.as_ptr(),
            &raw mut libres,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (hecho != 0).then_some(libres)
}

/// Bytes libres en el sistema de archivos donde está `ruta`.
#[cfg(not(any(unix, windows)))]
#[must_use]
pub fn disco_libre(_ruta: &std::path::Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En cualquier máquina donde esto corra hay al menos un hilo y, si la
    /// memoria se pudo leer, no es cero. Una lectura que devolviera cero se
    /// trataría como "sin memoria" y ocultaría funciones en una máquina capaz.
    #[test]
    fn lo_detectado_en_esta_maquina_es_coherente() {
        let recursos = detectar();

        assert!(recursos.nucleos >= 1);
        if let Some(memoria) = recursos.memoria_total {
            assert!(memoria > 0, "la memoria se leyó como cero");
            // Un byte no es una máquina. Si la lectura se equivocara de unidad
            // -páginas en vez de bytes, por ejemplo- esto lo caza.
            assert!(
                memoria >= 64 * 1024 * 1024,
                "la memoria leída no parece estar en bytes: {memoria}"
            );
        }
    }

    /// El suelo tiene que separar de verdad a una máquina pequeña de una
    /// normal. Si `da_para` contestara siempre que sí, el criterio de aceptación
    /// no se podría cumplir nunca.
    #[test]
    fn una_maquina_por_debajo_del_suelo_no_da_para_la_inferencia_local() {
        let pequena = Recursos {
            memoria_total: Some(1024 * 1024 * 1024),
            nucleos: 2,
            acelerador: Acelerador::Cpu,
        };
        let normal = Recursos {
            memoria_total: Some(8 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: Acelerador::Cpu,
        };

        assert!(!pequena.da_para(MEMORIA_MINIMA));
        assert!(normal.da_para(MEMORIA_MINIMA));
    }

    /// Hay espacio libre en el directorio de pruebas, y la cifra está en bytes y
    /// no en bloques: con bloques de 4 KiB, confundir unidades daría un disco
    /// cuatro mil veces más pequeño y ninguna descarga cabría nunca.
    #[test]
    fn el_espacio_libre_de_esta_maquina_se_lee_en_bytes() {
        let libre = disco_libre(&crate::paths::temporal_de_pruebas())
            .expect("se puede leer el espacio libre");
        assert!(
            libre >= 16 * 1024 * 1024,
            "el espacio libre leído no parece estar en bytes: {libre}"
        );
    }

    /// Una lectura fallida no puede ocultarle funciones a una máquina capaz: el
    /// dueño no tendría forma de enterarse, porque `NOT_SUPPORTED` no invita a
    /// nada. El error recuperable es el contrario.
    #[test]
    fn no_saber_cuanta_memoria_hay_no_oculta_la_inferencia() {
        let desconocida = Recursos {
            memoria_total: None,
            nucleos: 4,
            acelerador: Acelerador::Cpu,
        };

        assert!(desconocida.da_para(MEMORIA_MINIMA));
    }

    /// La inferencia no puede quedarse con la máquina entera: compite por los
    /// mismos núcleos que el hilo que pinta.
    #[test]
    fn la_inferencia_nunca_pide_todos_los_nucleos() {
        for nucleos in 1..=64_usize {
            let hilos = hilos_para_inferencia(nucleos);
            assert!(hilos >= 1, "con {nucleos} núcleos no quedó ningún hilo");
            assert!(
                hilos < nucleos || nucleos == 1,
                "con {nucleos} núcleos la inferencia pidió {hilos}"
            );
        }
    }
}
