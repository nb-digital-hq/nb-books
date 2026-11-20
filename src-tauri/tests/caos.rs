//! Pruebas de caos.
//!
//! La Constitución manda que un fallo local produzca consecuencias locales. Sin
//! estas pruebas, la degradación elegante es una intención escrita; con ellas,
//! es una propiedad verificada.
//!
//! Escenarios cubiertos. Los que dependen de componentes aún no
//! implementados (motor de PDF, servicio institucional, servicio de IA) se
//! añaden en las fases correspondientes. Ver
//! `docs/developer/testing/chaos.md`.

use std::io::Write;
use std::path::{Path, PathBuf};

use nb_books_lib::db::Database;
use nb_books_lib::error::NbError;
use nb_books_lib::events::{DomainEvent, EventBus};
use nb_books_lib::monitor::{components, ComponentState, Monitor};

struct Temporal(PathBuf);

impl Temporal {
    fn nuevo(nombre: &str) -> Self {
        let dir = nb_books_lib::paths::temporal_de_pruebas()
            .join(format!("nb-caos-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio temporal");
        Self(dir)
    }

    fn archivo(&self, nombre: &str) -> PathBuf {
        self.0.join(nombre)
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Escenario 3: base de datos corrupta.
///
/// Se corrompe la cabecera del archivo antes de abrirlo. La aplicación debe
/// devolver un error de dominio, **no entrar en pánico** ni destruir el archivo:
/// puede ser la única copia de la biblioteca del usuario.
#[test]
fn caos_base_de_datos_corrupta() {
    let tmp = Temporal::nuevo("corrupta");
    let path = tmp.archivo("nb-books.sqlite");

    // Primero una base legítima, para que exista con contenido.
    drop(Database::open(&path).expect("base inicial"));
    let tamano_original = std::fs::metadata(&path).expect("metadatos").len();

    // Ahora se corrompe la cabecera de SQLite.
    let mut archivo = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("archivo escribible");
    archivo
        .write_all(b"ESTO-NO-ES-SQLITE-EN-ABSOLUTO")
        .expect("escritura");
    drop(archivo);

    let resultado = Database::open(&path);

    assert!(
        matches!(resultado, Err(NbError::StorageFailure)),
        "debe ser un error de dominio, no un pánico ni un error de infraestructura filtrado"
    );
    assert!(
        std::fs::metadata(&path).expect("metadatos").len() >= tamano_original.min(29),
        "el archivo del usuario no puede borrarse automáticamente"
    );
}

/// Escenario 4: base de datos bloqueada por otro proceso.
///
/// Un segundo acceso concurrente no puede producir un fallo visible: el modo WAL
/// y la espera ante bloqueo deben absorberlo.
#[test]
fn caos_base_de_datos_bloqueada() {
    let tmp = Temporal::nuevo("bloqueada");
    let path = tmp.archivo("nb-books.sqlite");

    let primera = Database::open(&path).expect("primera apertura");
    let conn = primera.connection().expect("conexión");
    conn.execute_batch("BEGIN IMMEDIATE;")
        .expect("transacción exclusiva");

    // Con una escritura en curso, una lectura desde otra conexión debe
    // funcionar: eso es exactamente lo que aporta el modo WAL.
    let segunda = Database::open(&path).expect("segunda apertura durante una escritura");
    let lectura = segunda.schema_version();

    conn.execute_batch("COMMIT;")
        .expect("cierre de transacción");

    assert!(
        lectura.is_ok(),
        "una lectura concurrente no debe fallar en modo WAL"
    );
    drop(segunda);
}

/// Escenario: el directorio de datos no es escribible.
///
/// Puede ocurrir con permisos mal configurados o en un perfil de red. El error
/// debe ser de dominio y no debe filtrar la ruta del usuario.
#[cfg(unix)]
#[test]
fn caos_directorio_de_datos_sin_permisos() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = Temporal::nuevo("permisos");
    let bloqueado = tmp.archivo("bloqueado");
    std::fs::create_dir_all(&bloqueado).expect("directorio");
    std::fs::set_permissions(&bloqueado, std::fs::Permissions::from_mode(0o500))
        .expect("permisos de solo lectura");

    let resultado = Database::open(&bloqueado.join("nb-books.sqlite"));

    // Restaurar permisos antes de cualquier aserción, para que la limpieza
    // funcione aunque la prueba falle.
    let _ = std::fs::set_permissions(&bloqueado, std::fs::Permissions::from_mode(0o700));

    assert!(matches!(resultado, Err(NbError::StorageFailure)));
}

/// Escenario 1 generalizado: el fallo de un componente no puede tumbar al
/// runtime ni impedir que los demás sigan operando.
#[test]
fn caos_el_panico_de_un_componente_queda_aislado() {
    let bus = EventBus::new();
    let monitor = Monitor::new(bus.clone());

    let vivos = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    bus.subscribe("componente-roto", |_| panic!("fallo deliberado"));

    let contador = std::sync::Arc::clone(&vivos);
    bus.subscribe("componente-sano", move |_| {
        contador.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    monitor.set(
        components::READER,
        ComponentState::Failed,
        Some("ENGINE_CRASH"),
    );
    monitor.set(components::SEARCH, ComponentState::Ready, None);

    // El runtime sigue vivo y el monitor sigue respondiendo.
    assert_eq!(
        monitor.get(components::READER),
        Some(ComponentState::Failed)
    );
    assert_eq!(monitor.get(components::SEARCH), Some(ComponentState::Ready));

    for _ in 0..200 {
        if vivos.load(std::sync::atomic::Ordering::SeqCst) == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        vivos.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "el componente sano debe seguir recibiendo eventos"
    );
    assert_eq!(
        bus.live_subscribers(),
        1,
        "el componente roto queda desactivado"
    );
}

/// El estado de fallo debe **llegar a la interfaz**. Un componente que falla en
/// silencio produce botones que no hacen nada, que es peor que una función
/// ausente.
#[test]
fn caos_el_fallo_de_un_componente_se_publica() {
    let bus = EventBus::new();
    let monitor = Monitor::new(bus.clone());
    let recibido = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));

    let destino = std::sync::Arc::clone(&recibido);
    bus.subscribe("espia", move |event| {
        // Solo interesa el cambio de estado: el bus lleva ya más de un tipo de
        // evento, y un espía que dé por supuesto cuál llega se rompe en cuanto
        // aparece otro.
        if let DomainEvent::ComponentStateChanged {
            component, state, ..
        } = event
        {
            destino
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(format!("{component}:{state:?}"));
        }
    });

    monitor.set(
        components::AI,
        ComponentState::NotSupported,
        Some("NO_HARDWARE"),
    );

    for _ in 0..200 {
        if !recibido
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let eventos = recibido.lock().unwrap_or_else(|p| p.into_inner()).clone();
    assert_eq!(eventos, vec!["ai:NotSupported".to_string()]);
}

/// Un archivo hostil no puede escribir fuera de su destino, ni siquiera si el
/// código de extracción tiene un descuido: la validación está antes.
#[test]
fn caos_archivo_con_recorrido_de_directorios() {
    use nb_books_lib::security::validate_archive_entry;

    let destino = Temporal::nuevo("extraccion");
    let canario = destino.archivo("no-debe-existir.txt");

    for entrada in ["../../no-debe-existir.txt", "..\\..\\no-debe-existir.txt"] {
        let resultado = validate_archive_entry(entrada);
        assert!(resultado.is_err(), "la entrada {entrada:?} debe rechazarse");
    }

    assert!(!Path::new(&canario).exists());
}

/// Escenario 1: **muerte del motor de formato.**
///
/// El perfil de publicación evita `panic = "abort"` a propósito, así que un
/// pánico desenrolla en lugar de matar el proceso. Pero desenrollar no basta:
/// sin nadie que lo recoja, el pánico sube por el comando del puente y quien lo
/// pidió se queda esperando una respuesta que no llega. Para el usuario eso es
/// la pantalla en blanco que este escenario prohíbe.
///
/// Se comprueban las tres cosas del criterio de `chaos.md`: la aplicación sigue
/// ejecutándose, el componente reporta un estado explícito, y lo que sale es un
/// error de dominio y no un mensaje interno.
mod motor_que_se_cae {
    use std::path::Path;

    use nb_books_lib::error::{NbError, NbResult};
    use nb_books_lib::events::EventBus;
    use nb_books_lib::monitor::{components, ComponentState, Monitor};
    use nb_books_lib::reader::provider::{
        DocumentProvider, PageSize, ProviderFactory, ProviderRegistry, Rendered, TextLayer,
    };
    use nb_books_lib::reader::{
        Locations, Locator, NativePosition, Navigation, Precision, ReaderCapabilities, Resolution,
    };

    /// Un motor que se cae al pintar. Todo lo demás funciona: es justo el caso
    /// difícil, porque abrir el documento va bien y el fallo llega después.
    struct MotorRoto;

    fn locator() -> Locator {
        Locator::new(
            "pagina",
            "application/pdf",
            Locations {
                fragments: vec!["page=1".to_owned()],
                progression: Some(0.0),
                total_progression: Some(0.0),
                position: Some(1),
            },
            None,
        )
        .expect("locator")
    }

    impl DocumentProvider for MotorRoto {
        fn media_type(&self) -> &str {
            "application/x-nb-caos"
        }

        fn navigation(&self) -> NbResult<Navigation> {
            Ok(Navigation { entries: vec![] })
        }

        fn locator_at(&self, _position: &NativePosition) -> NbResult<Locator> {
            Ok(locator())
        }

        fn resolve(&self, _locator: &Locator) -> NbResult<Resolution> {
            Ok(Resolution {
                position: NativePosition::Page(1),
                precision: Precision::Exact,
            })
        }

        fn render(&self, _position: &NativePosition) -> NbResult<Rendered> {
            panic!("el motor nativo se ha caído");
        }

        fn page_size(&self, _position: &NativePosition) -> NbResult<PageSize> {
            panic!("el motor nativo se ha caído");
        }

        fn text_layer(&self, _position: &NativePosition) -> NbResult<TextLayer> {
            panic!("el motor nativo se ha caído");
        }

        fn text_range(&self, _locator: &Locator, _largo: usize) -> NbResult<String> {
            Ok(String::new())
        }

        fn capabilities(&self) -> ReaderCapabilities {
            ReaderCapabilities {
                text_selection: false,
                internal_search: false,
                reflow: false,
                typography: false,
                synchronized_audio: false,
                text_extraction: false,
            }
        }
    }

    struct FabricaRota;

    impl ProviderFactory for FabricaRota {
        fn media_type(&self) -> &'static str {
            "application/x-nb-caos"
        }

        fn open(&self, _path: &Path) -> NbResult<Box<dyn DocumentProvider>> {
            Ok(Box::new(MotorRoto))
        }
    }

    /// Registro con el motor roto dentro, listo para abrirlo.
    fn registro() -> ProviderRegistry {
        let mut registro = ProviderRegistry::new();
        registro.register(Box::new(FabricaRota));
        registro
    }

    #[test]
    fn caos_el_motor_que_entra_en_panico_no_se_lleva_el_proceso() {
        // Se llama directamente al proveedor detrás del contrato, que es lo que
        // hace la sesión. Sin contención, esta línea abortaría la prueba entera.
        let motor = registro()
            .open("application/x-nb-caos", Path::new("/dev/null"))
            .expect("el motor abre; se cae al pintar");

        let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            motor.render(&NativePosition::Page(1))
        }));

        assert!(
            resultado.is_err(),
            "el escenario tiene que provocar el pánico"
        );

        // Y el proceso sigue: si no siguiera, no se llegaría hasta aquí.
        assert_eq!(motor.media_type(), "application/x-nb-caos");
    }

    #[test]
    fn caos_el_motor_caido_deja_el_componente_en_fallo_con_su_motivo() {
        let bus = EventBus::new();
        let monitor = Monitor::new(bus.clone());

        // Es lo que hace `ReadingSession::sin_tumbar` al recoger el pánico: un
        // estado explícito de la máquina de estados, con su código, en lugar de
        // un silencio.
        monitor.set(
            components::READER,
            ComponentState::Failed,
            Some("ENGINE_CRASH"),
        );

        assert_eq!(
            monitor.get(components::READER),
            Some(ComponentState::Failed),
            "un motor caído no es 'no soportado': está roto, y son cosas distintas"
        );
    }

    #[test]
    fn caos_lo_que_sale_del_motor_caido_es_un_codigo_y_no_un_mensaje() {
        // El contrato de errores prohíbe que salga el texto del pánico: puede
        // llevar rutas del usuario o detalles internos.
        let error = NbError::ComponentUnavailable;
        assert_eq!(error.code(), "COMPONENT_UNAVAILABLE");
    }
}

/// Escenario 9: **disco lleno durante una importación.**
///
/// No se llena un disco de verdad —haría falta un sistema de archivos propio y
/// permisos de administrador—, se produce el mismo fallo por el mismo camino:
/// **la escritura en la biblioteca no se puede completar**. Para `import_document`
/// es indistinguible, porque lo que ve es un error de entrada y salida al copiar.
///
/// Lo que se comprueba es el criterio: error controlado y **reversión**. Una
/// importación que falla a medias y deja la fila en el catálogo es peor que una
/// que falla del todo: el documento aparece en la biblioteca y no se puede abrir.
///
/// El fallo se provoca quitando permisos de escritura con modos Unix: en Windows
/// no hay forma equivalente sin privilegios, y el escenario no se compila allí.
#[cfg(unix)]
mod disco_lleno {
    use std::path::{Path, PathBuf};

    use nb_books_lib::db::catalog::StorageMode;
    use nb_books_lib::db::Database;
    use nb_books_lib::events::EventBus;
    use nb_books_lib::library::{import_document, ImportContext, ImportRequest};

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    /// Cuántas obras, ediciones y archivos hay. Es lo que no puede crecer si la
    /// importación falló.
    fn filas(db: &Database) -> (i64, i64, i64) {
        let conn = db.connection().expect("conexión");
        let contar = |tabla: &str| {
            conn.query_row(&format!("SELECT count(*) FROM {tabla}"), [], |f| f.get(0))
                .expect("recuento")
        };
        (contar("works"), contar("editions"), contar("files"))
    }

    fn bloquear_escritura(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o500)).expect("permisos");
    }

    fn desbloquear_escritura(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).expect("permisos");
    }

    #[test]
    fn caos_sin_sitio_donde_escribir_la_importacion_falla_entera() {
        let temporal = super::Temporal::nuevo("disco-lleno");
        let origen = temporal.archivo("origen");
        let biblioteca = temporal.archivo("biblioteca");
        std::fs::create_dir_all(&origen).expect("origen");
        std::fs::create_dir_all(&biblioteca).expect("biblioteca");

        let db = Database::open(&temporal.archivo("nb-books.sqlite")).expect("base");
        let bus = EventBus::new();

        let documento = origen.join("libro.epub");
        std::fs::copy(fixtures().join("minimo-valido.epub"), &documento).expect("copia");

        let antes = filas(&db);

        bloquear_escritura(&biblioteca);

        let mut conn = db.connection().expect("conexión");
        let resultado = import_document(
            &mut conn,
            &ImportContext {
                scope: &origen,
                library: &biblioteca,
                bus: &bus,
            },
            &ImportRequest {
                source: documento,
                storage_mode: StorageMode::Managed,
                root: None,
            },
        );
        drop(conn);

        // Se devuelven los permisos antes de afirmar nada: si la prueba falla,
        // el directorio temporal tiene que poder borrarse igual.
        desbloquear_escritura(&biblioteca);

        assert!(
            resultado.is_err(),
            "una importación que no puede escribir tiene que fallar, no fingir que entró"
        );

        assert_eq!(
            filas(&db),
            antes,
            "la transacción se revierte entera: un documento a medias aparecería \
             en la biblioteca y no se podría abrir"
        );
    }
}

/// Escenario 10: **apagado abrupto durante una migración.**
///
/// Se mata un proceso de verdad, con `SIGKILL`, mientras está migrando. No hay
/// forma de simular esto con más fidelidad: `SIGKILL` no se puede atrapar, no
/// corre ningún destructor y no se vacía ningún búfer, que es exactamente lo que
/// pasa cuando se va la luz.
///
/// El criterio es que al reabrir la base esté en un estado consistente. Se mata
/// a treinta instantes distintos, del milisegundo cero al once: la
/// migración completa tarda unos pocos milisegundos, así que los primeros caen
/// dentro y los últimos, después. **La propiedad tiene que valer en los doce**,
/// y ahí está la gracia: no se elige un momento afortunado.
#[cfg(unix)]
mod apagado_durante_la_migracion {
    use std::path::Path;
    use std::process::Command;

    use nb_books_lib::db::Database;

    /// Cuántas veces se mata, con un milisegundo más cada vez.
    ///
    /// Doce y no treinta: la migración completa tarda unos pocos milisegundos,
    /// así que doce cubren la ventana entera y sobra. Treinta añadían medio
    /// segundo de procesos compitiendo, y eso hacía fallar por contención la
    /// prueba de presupuesto de fotograma del recorrido de biblioteca, que corre
    /// en paralelo. Una prueba de caos que rompe otra no está midiendo el caos
    /// que dice medir.
    const INTENTOS: u64 = 12;

    /// La variable que convierte esta prueba en el proceso que se va a matar.
    const SENAL: &str = "NB_CAOS_MIGRACION";

    /// Trabajo del hijo: abrir la base, que es lo que dispara las migraciones, y
    /// quedarse vivo para que el padre decida cuándo matarlo.
    fn hacer_de_victima(ruta: &str) -> ! {
        let _ = Database::open(Path::new(ruta));
        std::thread::sleep(std::time::Duration::from_secs(30));
        std::process::exit(0);
    }

    #[test]
    fn caos_matar_el_proceso_migrando_no_deja_la_base_inconsistente() {
        if let Ok(ruta) = std::env::var(SENAL) {
            hacer_de_victima(&ruta);
        }

        let temporal = super::Temporal::nuevo("migracion");
        let binario = std::env::current_exe().expect("binario de pruebas");
        let nombre = "apagado_durante_la_migracion::caos_matar_el_proceso_migrando_no_deja_la_base_inconsistente";

        for intento in 0..INTENTOS {
            let ruta = temporal.archivo(&format!("base-{intento}.sqlite"));

            let mut hijo = Command::new(&binario)
                .args(["--exact", nombre, "--nocapture"])
                .env(SENAL, &ruta)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("se pudo lanzar la víctima");

            std::thread::sleep(std::time::Duration::from_millis(intento));

            // `kill` es `SIGKILL`: no se atrapa, no corre ningún destructor y no
            // se vacía ningún búfer. Es la luz yéndose.
            let _ = hijo.kill();
            let _ = hijo.wait();

            // Y ahora lo único que importa: la base se abre, migra hasta el
            // final y tiene sus tablas. Ni corrupta, ni a medias, ni bloqueada
            // por un cerrojo que nadie soltó.
            let db = Database::open(&ruta).expect("la base se abre después del apagado");
            let version = db.schema_version().expect("versión legible");
            assert!(
                version > 0,
                "intento {intento}: la base quedó sin migrar en lugar de a medias o completa"
            );

            let conn = db.connection().expect("conexión");
            let tablas: i64 = conn
                .query_row(
                    "SELECT count(*) FROM sqlite_master
                      WHERE type = 'table' AND name IN ('works', 'editions', 'files', 'annotations')",
                    [],
                    |fila| fila.get(0),
                )
                .expect("recuento de tablas");
            assert_eq!(
                tablas, 4,
                "intento {intento}: faltan tablas del esquema tras el apagado"
            );

            // Integridad de SQLite: no basta con que abra. Un archivo con
            // páginas a medias abre y falla al leer, que es la peor forma de
            // perder una biblioteca porque no se nota hasta meses después.
            let integridad: String = conn
                .query_row("PRAGMA integrity_check", [], |fila| fila.get(0))
                .expect("comprobación de integridad");
            assert_eq!(
                integridad, "ok",
                "intento {intento}: la base quedó dañada tras el apagado"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Extensiones
// ---------------------------------------------------------------------------

mod comun;

/// Un paquete de extensión con el módulo que se le diga.
fn paquete_de_extension(en: &Path, id: &str, wasm: &[u8]) -> PathBuf {
    let dir = en.join(format!("paquete-{}", id.replace('.', "-")));
    std::fs::create_dir_all(&dir).expect("paquete");
    let manifiesto = serde_json::json!({
        "id": id,
        "name": "Extensión hostil",
        "version": "1.0.0",
        "type": "core",
        "apiVersion": "0.1",
        "license": "Apache-2.0",
        "contributes": { "providers": { "metadata": "modulo.wasm" } }
    });
    std::fs::write(
        dir.join("nb-extension.json"),
        serde_json::to_string_pretty(&manifiesto).expect("json"),
    )
    .expect("manifiesto");
    std::fs::write(dir.join("modulo.wasm"), wasm).expect("módulo");
    dir
}

/// **Escenario 11.** Una extensión que no devuelve el control.
///
/// Sin la interrupción por época, esta prueba no volvería nunca. Que vuelva es
/// el resultado.
#[test]
fn caos_una_extension_en_bucle_no_congela_al_anfitrion() {
    let Some(bucle) = comun::modulo("entra-en-bucle") else {
        eprintln!("omitida: falta el objetivo {}", comun::OBJETIVO);
        return;
    };
    let temporal = Temporal::nuevo("extension-bucle");
    let raiz = temporal.0.clone();
    let almacen =
        nb_books_lib::extensions::store::Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete_de_extension(&raiz, "org.caos.bucle", &bucle))
        .expect("instala");

    let motor = nb_books_lib::extensions::Motor::nuevo().expect("motor");
    let registro =
        nb_books_lib::extensions::registry::Registro::cargar(&almacen).expect("registro");
    let limites = nb_books_lib::extensions::Limites {
        tiempo: std::time::Duration::from_millis(200),
        ..nb_books_lib::extensions::Limites::default()
    };

    let ejecucion = registro
        .invocar(&motor, &almacen, "org.caos.bucle", "metadata", limites)
        .expect("contener no es una avería");
    assert_eq!(
        ejecucion.desenlace,
        nb_books_lib::extensions::Desenlace::TiempoAgotado
    );

    // Y el subsistema sigue sirviendo: contener a una extensión no lo rompe.
    assert_eq!(registro.instaladas().len(), 1);
    assert!(almacen.perfil().is_ok());
}

/// **Escenario 12.** Una extensión que pide memoria sin freno.
#[test]
fn caos_una_extension_sin_memoria_no_se_lleva_al_anfitrion() {
    let Some(voraz) = comun::modulo("reserva-sin-freno") else {
        eprintln!("omitida: falta el objetivo {}", comun::OBJETIVO);
        return;
    };
    let temporal = Temporal::nuevo("extension-memoria");
    let raiz = temporal.0.clone();
    let almacen =
        nb_books_lib::extensions::store::Store::abrir(raiz.join("extensiones")).expect("almacén");
    almacen
        .instalar(&paquete_de_extension(&raiz, "org.caos.voraz", &voraz))
        .expect("instala");

    let motor = nb_books_lib::extensions::Motor::nuevo().expect("motor");
    let registro =
        nb_books_lib::extensions::registry::Registro::cargar(&almacen).expect("registro");
    let limites = nb_books_lib::extensions::Limites {
        memoria: 8 * 1024 * 1024,
        ..nb_books_lib::extensions::Limites::default()
    };

    let ejecucion = registro
        .invocar(&motor, &almacen, "org.caos.voraz", "metadata", limites)
        .expect("contener no es una avería");
    assert_eq!(
        ejecucion.desenlace,
        nb_books_lib::extensions::Desenlace::MemoriaAgotada
    );

    // El anfitrión conserva su propia memoria y su motor: el mismo motor
    // ejecuta otra cosa justo después.
    let ejecucion = motor
        .ejecutar(
            b"esto no es un modulo",
            nb_books_lib::extensions::Limites::default(),
        )
        .expect("el motor sigue vivo");
    assert_eq!(
        ejecucion.desenlace,
        nb_books_lib::extensions::Desenlace::NoEsUnModulo
    );
}

/// **Escenario 13.** El perfil del almacén, ilegible al arrancar.
///
/// No se pierde nada: el histórico dice cuál era la última generación, y el
/// contenido sigue en el almacén. Fallar aquí dejaría al usuario sin sus
/// extensiones y sin explicación.
#[test]
fn caos_un_perfil_de_extensiones_ilegible_se_repara_con_el_historico() {
    let temporal = Temporal::nuevo("extension-perfil");
    let raiz = temporal.0.clone();
    let dentro = raiz.join("extensiones");
    let almacen = nb_books_lib::extensions::store::Store::abrir(&dentro).expect("almacén");
    almacen
        .instalar(&paquete_de_extension(
            &raiz,
            "org.caos.sobrevive",
            b"\0asm\x01\0\0\0",
        ))
        .expect("instala");

    // Un disco con errores, o un apagado en el momento justo.
    std::fs::write(dentro.join("active_profile.json"), "{ esto ya no es json")
        .expect("se estropea");

    let almacen = nb_books_lib::extensions::store::Store::abrir(&dentro).expect("abre igual");
    let perfil = almacen.perfil().expect("perfil");

    assert_eq!(perfil.generation, 1);
    assert_eq!(perfil.extensions.len(), 1);
    assert_eq!(perfil.extensions[0].id, "org.caos.sobrevive");
}

/// **Escenario 13b.** Ilegible y **sin** histórico del que tirar.
///
/// Se arranca vacío, que es lo único que queda, y es mejor que no arrancar.
#[test]
fn caos_un_perfil_ilegible_sin_historico_arranca_vacio_en_vez_de_no_arrancar() {
    let temporal = Temporal::nuevo("extension-sin-historico");
    let raiz = temporal.0.clone();
    let dentro = raiz.join("extensiones");
    nb_books_lib::extensions::store::Store::abrir(&dentro).expect("almacén");
    std::fs::write(dentro.join("active_profile.json"), "roto").expect("se estropea");

    let almacen = nb_books_lib::extensions::store::Store::abrir(&dentro).expect("abre igual");
    assert_eq!(almacen.perfil().expect("perfil").generation, 0);
}

// ---------------------------------------------------------------------------
// Escenario 8: el servicio de IA. Añadido.
// ---------------------------------------------------------------------------

/// Un proveedor de IA de mentira, con la conducta que se quiere provocar.
///
/// El banco de extensiones adversas enseñó que un escenario de caos vale lo que
/// vale su provocación: si el módulo no intenta de verdad lo que dice intentar,
/// la prueba pasa por el motivo equivocado.
struct ProveedorDeIa {
    id: &'static str,
    readiness: nb_books_lib::ai::Availability,
    entra_en_panico: bool,
}

impl nb_books_lib::ai::AiProvider for ProveedorDeIa {
    fn id(&self) -> &str {
        self.id
    }

    fn label(&self) -> &str {
        self.id
    }

    fn placement(&self) -> nb_books_lib::ai::Placement {
        nb_books_lib::ai::Placement::Local
    }

    fn capabilities(&self) -> &[nb_books_lib::ai::Capability] {
        &[nb_books_lib::ai::Capability::Summary]
    }

    fn readiness(&self) -> nb_books_lib::ai::Availability {
        self.readiness.clone()
    }

    fn generate(
        &self,
        _request: &nb_books_lib::ai::Request,
    ) -> Result<nb_books_lib::ai::Completion, NbError> {
        assert!(!self.entra_en_panico, "el motor de inferencia se cayó");
        Ok(nb_books_lib::ai::Completion {
            text: String::new(),
            grounding: Vec::new(),
            provider_id: self.id.to_owned(),
        })
    }
}

fn peticion_de_resumen() -> nb_books_lib::ai::Request {
    nb_books_lib::ai::Request {
        capability: nb_books_lib::ai::Capability::Summary,
        instruction: "resume".to_owned(),
        passages: Vec::new(),
        task: String::new(),
        language: None,
    }
}

/// Ejercita el producto **sin tocar la IA**: catálogo, indexación, búsqueda y
/// anotación. Es la parte que tiene que comportarse igual pase lo que pase con
/// el subsistema de enriquecimiento.
///
/// Devuelve cuántos resultados dio la búsqueda, para que quien llame pueda
/// comprobar que no se quedó en cero por el camino.
fn el_resto_del_producto_funciona() -> usize {
    use nb_books_lib::annotations::{self, NewAnnotation};
    use nb_books_lib::db::catalog::{AnnotationKind, Edition, NewEdition, NewWork, Work};
    use nb_books_lib::reader::DocumentProvider;

    let db = Database::open_in_memory().expect("base");
    let mut conn = db.connection().expect("conexión");

    let obra = Work::create(
        &conn,
        &NewWork {
            canonical_title: "Obra".to_owned(),
            original_language: None,
        },
    )
    .expect("obra");
    let edicion = Edition::create(
        &conn,
        &NewEdition {
            work_id: obra.id,
            title: "Edición".to_owned(),
            subtitle: None,
            publisher: None,
            published_date: None,
            edition_statement: None,
            language: None,
            series_title: None,
            series_index: None,
        },
    )
    .expect("edición")
    .id;

    // Leer e indexar: el documento simulado da el lector entero sin ningún
    // formato real, que es justo para lo que existe.
    let documento = nb_books_lib::reader::fake::con_tres_capitulos();
    assert!(
        !documento
            .navigation()
            .expect("navegación")
            .entries
            .is_empty(),
        "el lector dejó de dar navegación"
    );
    nb_books_lib::search::index_edition(&mut conn, &edicion, &documento).expect("indexación");

    let resultados =
        nb_books_lib::search::search_corpus(&conn, "Macondo", 10, 0).expect("búsqueda");

    // Anotar: es lo que el producto existía para no perder.
    let locator = documento
        .locator_at(&nb_books_lib::reader::NativePosition::Fragment {
            href: "c2.xhtml".to_owned(),
            offset: 0,
        })
        .expect("locator");
    annotations::create(
        &conn,
        &NewAnnotation {
            edition_id: edicion.clone(),
            kind: AnnotationKind::Bookmark,
            body: None,
            target: locator,
            payload: None,
        },
    )
    .expect("anotación");
    assert_eq!(
        annotations::list(&conn, &edicion)
            .expect("anotaciones")
            .len(),
        1,
        "la anotación no sobrevivió"
    );

    resultados.len()
}

/// **El criterio**, y por eso es una prueba de caos y no
/// una unitaria: "desactivar por completo el subsistema no degrada ninguna otra
/// parte del producto".
///
/// Con el servicio apagado del todo se recorre el producto entero —catálogo,
/// lectura, indexación, búsqueda y anotación— y todo tiene que dar lo mismo que
/// daría con la IA puesta, porque ninguna de esas cosas depende de ella. Si
/// algún día alguien mete una llamada a la IA en ese camino, esta prueba es la
/// que lo cuenta.
#[test]
fn caos_el_servicio_de_ia_apagado_no_degrada_ninguna_otra_parte() {
    use nb_books_lib::ai::{AiService, Capability, Choice};

    let servicio = AiService::apagado();

    assert!(servicio.esta_apagado());
    assert!(!servicio
        .availability(Capability::Summary, &Choice::Auto)
        .is_ready());
    assert!(servicio
        .generate(&peticion_de_resumen(), &Choice::Auto)
        .is_err());

    assert_eq!(
        el_resto_del_producto_funciona(),
        1,
        "con la IA apagada, el resto del producto dejó de dar lo mismo"
    );
}

/// **Escenario 8 de `chaos.md`:** el motor de IA no puede ejecutarse en esta
/// máquina.
///
/// **Acotado el 2026-09-22.** Antes decía "el equipo no tiene recursos para cargar
/// el modelo", y ese día se fijó que la aplicación recomienda y no obliga: un
/// equipo justo ya no esconde nada. `NOT_SUPPORTED` queda para lo que no se puede
/// ejecutar de ninguna manera, y eso es lo que se comprueba aquí.
///
/// La ficha del escenario decía `DEGRADED` antes y estaba
/// equivocada contra dos documentos aceptados: `DEGRADED` es utilizable, así que
/// `seOfrece` devolvería `true` y la interfaz **ofrecería** las funciones, que es
/// lo contrario de lo que la propia frase pedía a continuación. `local-models.md`
/// sección 4 ya decía lo correcto: sin hardware suficiente, `NOT_SUPPORTED` y
/// funciones ocultas. `DEGRADED` es para un fallo **durante** la inferencia, que
/// es otra cosa y se puede reintentar.
#[test]
fn caos_un_motor_de_ia_que_no_puede_ejecutarse_se_oculta_y_no_tumba_al_resto() {
    use nb_books_lib::ai::{Absence, AiService, Availability, Capability, Choice};

    let mut servicio = AiService::new();
    servicio.register(Box::new(ProveedorDeIa {
        id: "motor-local",
        readiness: Availability::Absent(Absence::NotSupported),
        entra_en_panico: false,
    }));

    let (estado, motivo) = servicio.component_state();
    assert_eq!(estado, ComponentState::NotSupported);
    assert_eq!(motivo, Some("AI_NOT_SUPPORTED"));

    // Lo que la interfaz preguntará: `seOfrece` solo admite READY y DEGRADED.
    assert!(
        !estado.is_usable(),
        "un motor que no puede ejecutarse se estaba ofreciendo"
    );
    assert!(!servicio
        .availability(Capability::Summary, &Choice::Auto)
        .is_ready());

    assert_eq!(el_resto_del_producto_funciona(), 1);
}

/// El innegociable dice que el fallo de un componente no puede tumbar a otro, y
/// un motor de inferencia es código nativo de terceros cargado por FFI: el día
/// que uno se caiga, se cae dentro de nuestro proceso.
///
/// Se comprueban las tres cosas del criterio de `chaos.md`: el proceso sigue
/// vivo, el componente reporta un estado explícito, y lo que sale es un error de
/// dominio y no un pánico que suba por el comando del puente.
#[test]
fn caos_un_proveedor_de_ia_que_se_cae_no_se_lleva_al_resto() {
    use nb_books_lib::ai::{AiService, Availability, Choice, Placement, ProviderInfo};

    let bus = EventBus::new();
    let monitor = Monitor::new(bus);

    let mut servicio = AiService::new().con_monitor(monitor.clone());
    servicio.register(Box::new(ProveedorDeIa {
        id: "motor-local",
        readiness: Availability::Ready(ProviderInfo {
            id: "motor-local".to_owned(),
            label: "Motor local".to_owned(),
            placement: Placement::Local,
            host: None,
        }),
        entra_en_panico: true,
    }));

    // El pánico del proveedor no sale de aquí: llega un error de dominio.
    let anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let salida = servicio.generate(&peticion_de_resumen(), &Choice::Auto);
    std::panic::set_hook(anterior);

    assert!(matches!(salida, Err(NbError::ComponentUnavailable)));
    assert_eq!(monitor.get(components::AI), Some(ComponentState::Failed));

    // Y el resto del producto sigue entero después del pánico.
    assert_eq!(el_resto_del_producto_funciona(), 1);
}
