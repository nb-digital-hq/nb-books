//! Inferencia local: el motor, en esta máquina.
//!
//! Contrato: `docs/architecture/ai/local-models.md`.
//!
//! # Qué motor, y por qué se cambió el que decía el contrato
//!
//! **Candle**, el motor de Hugging Face, en Rust puro. `local-models.md` decía
//! `llama.cpp` por FFI u ONNX Runtime desde que se escribió, y se cambió
//! el 2026-09-12 por una razón que este proyecto ya tiene medida en carne
//! propia: las dos cicatrices de la puerta de empaquetado son **el runtime que
//! estuvo semanas sin compilar en Windows** y **el motor de PDF que no viajaba
//! en el instalador**. Un motor de C++ por FFI, con backends acelerados
//! distintos por plataforma y por GPU, es esa misma clase de defecto
//! multiplicada por tres sistemas. Candle se compila como el resto del runtime,
//! en el mismo `cargo build`, y desaparece la clase entera.
//!
//! **Lo que se paga a cambio, y está dicho, no escondido:** en generación
//! cuantizada Candle es más lento que `llama.cpp`. Eso choca -la
//! inferencia nunca degrada el presupuesto de fotograma- y **hay que medirlo
//! antes de prometer generación local**. Para embeddings, que es lo que
//! [ADR-007] necesita de verdad y con el alcance que fijó -el documento abierto,
//! unos 223 vectores-, la diferencia importa mucho menos.
//!
//! [ADR-007]: ../../../docs/adr/decisions/ADR-007-vector-store.md
//!
//! # Qué hace este módulo y qué deja a otros
//!
//! Aquí está **el motor y la máquina**: se detecta qué hay, se elige dónde
//! calcular, y se comprueba que el motor de verdad calcula aquí. Lo que no está
//! son los **pesos**: no hay catálogo, no hay descarga y no hay modelo. Eso es
//! el catálogo de modelos, y por eso un equipo capaz recibe `NOT_INSTALLED`.
//!
//! # Por qué este proveedor todavía no se registra en el arranque
//!
//! Y no es un olvido, que es el defecto más frecuente de este proyecto. `salud.ts`
//! convierte `NOT_INSTALLED` en un aviso **accionable** -"falta algo,
//! instálalo"-, y sin el catálogo no habría ningún sitio al que
//! llevar a quien lo pulse. Registrarlo hoy volvería a poner la tarjeta
//! permanente que la regla de componentes ajenos prohíbe, esta vez invitando a una pantalla que
//! no existe. Se conecta, en el mismo movimiento que le da destino.

use std::path::{Path, PathBuf};

use candle_core::{DType, Device, Tensor};

use super::hardware::{self, Acelerador, Recursos};
use super::{Absence, AiProvider, Availability, Capability, Placement, ProviderInfo};

/// Identificador estable del proveedor local. Se guarda con cada salida.
pub const ID: &str = "local";

/// Las capacidades que este proveedor atenderá.
///
/// Solo embeddings, y es deliberado. La generación local existe como objetivo
/// -es la regla 2 de `philosophy.md`- pero prometerla antes de medir lo que
/// cuesta con Candle sería exactamente lo que la política de dependencias prohíbe. Cuando esté
/// medida, entra aquí y no hay que tocar nada más: el contrato ya la contempla.
const CAPACIDADES: &[Capability] = &[Capability::Embedding];

/// El motor de inferencia local.
pub struct LocalProvider {
    recursos: Recursos,
    dispositivo: Device,
    /// Si el motor de verdad calcula en esta máquina, comprobado y no supuesto.
    motor_vivo: bool,
    /// La carpeta de modelos instalados. Ver `paths::AppPaths::models`.
    modelos: PathBuf,
    /// El modelo de embeddings, cargado al pedirlo y soltado sin uso.
    residente: std::sync::Arc<super::modelo::Residente>,
}

impl LocalProvider {
    /// Mira qué hay en esta máquina y prepara el motor.
    ///
    /// No falla: una máquina que no da para nada produce un proveedor que lo
    /// dice, que es lo que exige la regla de que la ausencia sea un valor.
    ///
    /// `presupuesto` es el de las preferencias guardadas; se puede cambiar después
    /// con [`super::modelo::Residente::aplicar`].
    #[must_use]
    pub fn detectar(modelos: &Path, presupuesto: super::presupuesto::Presupuesto) -> Self {
        let recursos = hardware::detectar();
        let dispositivo = dispositivo_para(recursos.acelerador);
        let motor_vivo = el_motor_calcula(&dispositivo);

        if !motor_vivo {
            // Se registra porque es lo que separa "esta máquina no da" de "el
            // motor está roto", y sin el registro las dos se ven igual desde
            // fuera. No lleva rutas ni nada del documento.
            tracing::warn!(
                acelerador = %recursos.acelerador,
                "el motor de inferencia no calcula en esta máquina"
            );
        }

        let entrada = &super::catalogo::GRANITE_107M_MULTILINGUAL;
        let residente = std::sync::Arc::new(super::modelo::Residente::new(
            super::instalacion::carpeta(modelos, entrada),
            entrada,
            dispositivo.clone(),
            super::modelo::INACTIVIDAD,
            presupuesto,
        ));

        Self {
            recursos,
            dispositivo,
            motor_vivo,
            modelos: modelos.to_path_buf(),
            residente,
        }
    }

    /// Qué tiene esta máquina. Lo consulta el diagnóstico.
    #[must_use]
    pub const fn recursos(&self) -> Recursos {
        self.recursos
    }

    /// Dónde calcula.
    #[must_use]
    pub const fn dispositivo(&self) -> &Device {
        &self.dispositivo
    }

    /// El modelo residente. Lo guarda el subsistema para cambiarle el presupuesto
    /// sin buscar al proveedor dentro del servicio.
    #[must_use]
    pub fn residente(&self) -> std::sync::Arc<super::modelo::Residente> {
        std::sync::Arc::clone(&self.residente)
    }

    /// Si hay instalado algún modelo del catálogo que atienda lo que este
    /// proveedor declara.
    ///
    /// **Ahora pregunta al catálogo, no al disco.** Hasta entonces
    /// bastaba con que hubiera un `.safetensors` en la carpeta, lo que declaraba
    /// listo a cualquier archivo con esa extensión, estuviera en el catálogo o no,
    /// se hubiera verificado o no. Ahora un modelo está instalado solo si su
    /// carpeta existe con cada archivo del tamaño revisado, y esa carpeta solo
    /// aparece tras verificar la descarga entera. Ver `instalacion`.
    fn hay_pesos(&self) -> bool {
        super::catalogo::CATALOGO
            .iter()
            .filter(|modelo| CAPACIDADES.contains(&modelo.capacidad))
            .any(|modelo| super::instalacion::instalado(&self.modelos, modelo))
    }
}

impl AiProvider for LocalProvider {
    fn id(&self) -> &str {
        ID
    }

    fn label(&self) -> &str {
        "Motor Local"
    }

    fn placement(&self) -> Placement {
        Placement::Local
    }

    fn capabilities(&self) -> &[Capability] {
        CAPACIDADES
    }

    fn readiness(&self) -> Availability {
        // Lo único que de verdad no tiene arreglo es que el motor no arranque. **Un
        // equipo justo de memoria ya no esconde nada** ahora: Ajustes
        // le dice a quien lo tiene lo que es probable que pase y le deja descargar
        // igual -Se decidió: "una cosa es recomendar y otra obligar"-, así
        // que si hay pesos, alguien los eligió.
        if !self.motor_vivo {
            return Availability::Absent(Absence::NotSupported);
        }

        if self.hay_pesos() {
            // Después de los pesos y no antes: sin modelo no hay nada que no quepa,
            // y decir "no cabe en el tope" a quien no ha descargado nada le haría
            // pensar que descargarlo no serviría.
            if !self.residente.cabe_en_memoria() {
                return Availability::Absent(Absence::OverBudget);
            }
            return Availability::Ready(ProviderInfo {
                id: ID.to_owned(),
                label: self.label().to_owned(),
                placement: Placement::Local,
                host: None,
            });
        }

        /*
         * **Sin modelo, nadie lo ha pedido; con rastro de uno roto, sí.** Y la
         * diferencia se ve en pantalla.
         *
         * `local-models.md` sección 4 decía "modelo no descargado: `NOT_INSTALLED`,
         * funciones visibles con invitación a instalar", y esto lo
         * cumplía. Pero `salud.ts` convierte `NOT_INSTALLED` en la tarjeta
         * **"Algo necesita atención"** de la barra lateral, así que en cuanto el
         * proveedor se registrara, **todo equipo capaz sin un modelo opcional
         * descargado tendría esa tarjeta para siempre**. Es el defecto que quitó
         * el mismo defecto con otra causa, y contradice que la IA sea opcional: no descargar
         * un modelo no es algo que necesite atención.
         *
         * Así que la invitación vive donde se configura la IA, en Ajustes, y el
         * estado global dice lo que es: detenido, porque nadie lo ha pedido. Solo
         * cuando queda la carpeta de un modelo que ya no está entero —alguien lo
         * instaló, y se truncó o se borró un archivo— hay algo que el usuario eligió
         * y que ahora falla, y eso sí se anuncia.
         */
        let hubo_uno = super::catalogo::CATALOGO
            .iter()
            .filter(|modelo| CAPACIDADES.contains(&modelo.capacidad))
            .any(|modelo| super::instalacion::hay_rastro(&self.modelos, modelo));
        if hubo_uno {
            Availability::Absent(Absence::NotInstalled)
        } else {
            Availability::Absent(Absence::Disabled)
        }
    }

    /// Vectores del modelo instalado, cargándolo si no lo está.
    ///
    /// Sin modelo instalado, o sin sitio en el tope de memoria, responde que no
    /// está disponible y no intenta cargar nada: `readiness` ya lo dijo, y quien
    /// pide sin preguntar recibe el error de quien no preguntó.
    fn embed(&self, texts: &[String]) -> crate::error::NbResult<Vec<super::Embedding>> {
        if !self.readiness().is_ready() {
            return Err(crate::error::NbError::ComponentUnavailable);
        }
        self.residente.vectores(texts)
    }
}

/// Dónde calcula el motor.
///
/// Recibe lo que [`hardware::detectar`] ya averiguó probando, así que aquí solo
/// se vuelve a abrir el dispositivo que respondió. Caer al procesador es
/// correcto y **no es un fallo** —es el camino normal en la mayoría de los
/// equipos—, así que no se registra como tal.
pub(crate) fn dispositivo_para(acelerador: Acelerador) -> Device {
    match acelerador {
        Acelerador::Cuda => Device::new_cuda(0).unwrap_or(Device::Cpu),
        Acelerador::Metal => Device::new_metal(0).unwrap_or(Device::Cpu),
        Acelerador::Cpu => Device::Cpu,
    }
}

/// Comprueba que el motor **calcula de verdad** en este dispositivo.
///
/// # Por qué no basta con que compile
///
/// Porque son dos cosas distintas y este proyecto ya se quemó confundiéndolas:
/// el motor de PDF compilaba y no viajaba en el instalador. Que Candle esté
/// enlazado no dice que pueda reservar un tensor y multiplicar en esta máquina;
/// con CUDA o Metal compilados y sin GPU delante, precisamente, no puede. Una
/// multiplicación de dos matrices pequeñas con un resultado conocido responde
/// esa pregunta en microsegundos y sin ningún peso descargado.
/// Si la suma que devolvió el motor es la que tenía que devolver.
///
/// Está aparte de [`el_motor_calcula`] para poder comprobarla: dentro, un motor
/// que devolviera ceros pasaría la prueba igual, porque no fallaría. Lo destapó
/// una mutación —quitar la comparación y aceptar cualquier `Ok`— que ninguna
/// prueba veía. La comprobación de que el motor *arranca* y la de que *acierta*
/// son dos cosas, y la segunda no se puede provocar con Candle delante.
///
/// Diez es la suma de la matriz `[[1,2],[3,4]]`, que es lo que tiene que salir
/// de multiplicarla por la identidad.
const fn es_el_resultado_esperado(suma: f32) -> bool {
    // Coma flotante: se compara con margen, no con igualdad.
    (suma - 10.0) < 1e-4 && (10.0 - suma) < 1e-4
}

fn el_motor_calcula(dispositivo: &Device) -> bool {
    let intento = || -> candle_core::Result<f32> {
        let a = Tensor::new(&[[1.0_f32, 2.0], [3.0, 4.0]], dispositivo)?;
        let b = Tensor::new(&[[1.0_f32, 0.0], [0.0, 1.0]], dispositivo)?;
        // Multiplicar por la identidad tiene que devolver la primera matriz. Un
        // motor que devolviera ceros, o que fallara al traer el resultado a la
        // memoria del proceso, no sirve y hay que saberlo ahora.
        a.matmul(&b)?.to_dtype(DType::F32)?.sum_all()?.to_scalar()
    };

    match intento() {
        Ok(suma) => es_el_resultado_esperado(suma),
        Err(error) => {
            // El texto del error del motor puede llevar rutas del sistema, así
            // que no se propaga: solo se registra que falló y con qué código de
            // dominio se va a contestar.
            tracing::warn!(clase = %error, "el motor de inferencia no pudo calcular");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::catalogo::{Modelo, GRANITE_107M_MULTILINGUAL};

    /// Deja en `carpeta` los archivos de un modelo con el tamaño revisado.
    ///
    /// Con `set_len`, que en los sistemas de archivos corrientes crea un archivo
    /// disperso: la prueba no escribe doscientos megabytes para comprobar un
    /// tamaño.
    fn disponer(carpeta: &Path, modelo: &Modelo) {
        std::fs::create_dir_all(carpeta).expect("carpeta");
        for archivo in modelo.archivos {
            std::fs::File::create(carpeta.join(archivo.nombre))
                .and_then(|f| f.set_len(archivo.bytes))
                .expect("archivo del tamaño revisado");
        }
    }

    fn fabrica() -> crate::ai::presupuesto::Presupuesto {
        crate::ai::presupuesto::Presupuesto::de_fabrica(8)
    }

    fn temporal(nombre: &str) -> PathBuf {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-ia-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("directorio");
        dir
    }

    /// Un motor que no falla pero devuelve otra cosa **no sirve**, y hasta que
    /// esta prueba existió nada lo decía: la comprobación aceptaba cualquier
    /// `Ok`. Lo encontró una mutación.
    #[test]
    fn un_motor_que_devuelve_otra_cosa_no_cuenta_como_que_calcula() {
        assert!(es_el_resultado_esperado(10.0));
        assert!(es_el_resultado_esperado(10.000_01));
        // Ceros es lo que devolvería un motor que no multiplica de verdad.
        assert!(!es_el_resultado_esperado(0.0));
        assert!(!es_el_resultado_esperado(9.0));
        assert!(!es_el_resultado_esperado(11.0));
    }

    /// **La integración del motor, comprobada ejecutándolo.** Que Candle esté
    /// enlazado no dice que calcule aquí; esto lo dice.
    #[test]
    fn el_motor_calcula_en_esta_maquina() {
        assert!(
            el_motor_calcula(&Device::Cpu),
            "el motor no pudo multiplicar dos matrices en el procesador"
        );
    }

    /// Sin acelerador, el dispositivo es el procesador. Devolver otra cosa sería
    /// prometer una GPU que no está.
    #[test]
    fn sin_acelerador_se_calcula_en_el_procesador() {
        assert!(matches!(dispositivo_para(Acelerador::Cpu), Device::Cpu));
    }

    /// Y lo que se detecta en esta máquina es coherente con lo que el motor
    /// acepta abrir: si se dijera "cuda" y abrir fallara, el proveedor se
    /// declararía listo sobre un dispositivo que no existe.
    #[test]
    fn el_acelerador_detectado_es_uno_que_el_motor_puede_abrir() {
        let recursos = hardware::detectar();
        let dispositivo = dispositivo_para(recursos.acelerador);

        match recursos.acelerador {
            Acelerador::Cpu => assert!(matches!(dispositivo, Device::Cpu)),
            // En una máquina con GPU no puede haber caído al procesador: eso
            // significaría que detectar y abrir no dicen lo mismo.
            Acelerador::Cuda | Acelerador::Metal => {
                assert!(!matches!(dispositivo, Device::Cpu));
            }
        }
        assert!(el_motor_calcula(&dispositivo));
    }

    /// **El criterio, como quedó el 2026-09-22.** Decía "en un equipo sin
    /// hardware suficiente el componente reporta no soportado", y esta prueba lo
    /// exigía. Ese día se fijó que la aplicación recomienda y no obliga, así que
    /// un equipo justo **ya no esconde la IA**: sin modelo está detenida -nadie la ha
    /// pedido- y no se declara no soportada, porque eso quitaría la descarga de
    /// Ajustes sin que su dueño pudiera elegir.
    #[test]
    fn un_equipo_sin_recursos_no_esconde_la_ia() {
        let modelos = temporal("sin-recursos");
        let mut proveedor = LocalProvider::detectar(&modelos, fabrica());
        proveedor.recursos = Recursos {
            memoria_total: Some(512 * 1024 * 1024),
            nucleos: 1,
            acelerador: Acelerador::Cpu,
        };

        assert_ne!(
            proveedor.readiness(),
            Availability::Absent(Absence::NotSupported),
            "un equipo justo volvió a esconder la IA"
        );
    }

    /// Segunda mitad del criterio: **sin error visible**. La ausencia sale como
    /// un valor del contrato, no como un `Err` que alguien tenga que tratar ni
    /// como un pánico.
    #[test]
    fn un_equipo_sin_recursos_no_produce_ningun_error() {
        let modelos = temporal("sin-error");
        let mut proveedor = LocalProvider::detectar(&modelos, fabrica());
        proveedor.recursos = Recursos {
            memoria_total: Some(512 * 1024 * 1024),
            nucleos: 1,
            acelerador: Acelerador::Cpu,
        };

        let mut servicio = super::super::AiService::new();
        servicio.register(Box::new(proveedor));

        // Sin modelo, nadie ha pedido nada: detenido, sin error y sin aviso. Desde
        // el 2026-09-22 ya no es `NOT_SUPPORTED`, porque un equipo justo **no
        // esconde** la descarga; la previsión de Ajustes dice lo que es probable que
        // pase, y quien lo tiene decide.
        let (estado, _) = servicio.component_state();
        assert!(
            !estado.is_usable(),
            "se ofrecen funciones sin ningún modelo descargado"
        );
        assert_ne!(
            estado,
            crate::monitor::ComponentState::Failed,
            "un equipo sin recursos produjo un error"
        );
    }

    /// Un equipo capaz sin ningún modelo **no pide atención**: nadie ha pedido la
    /// IA, que es opcional. Si se anunciara como instalable, todo equipo capaz
    /// tendría para siempre la tarjeta "Algo necesita atención", que es el
    /// defecto con otra causa. La invitación vive en Ajustes.
    #[test]
    fn un_equipo_capaz_sin_ningun_modelo_no_pide_atencion() {
        let modelos = temporal("sin-pesos");
        let mut proveedor = LocalProvider::detectar(&modelos, fabrica());
        proveedor.recursos = Recursos {
            memoria_total: Some(16 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: Acelerador::Cpu,
        };

        assert_eq!(
            proveedor.readiness(),
            Availability::Absent(Absence::Disabled)
        );
    }

    /// Con la carpeta de un modelo que ya no está entero, sí: alguien lo instaló
    /// y ahora falla, que es algo que eligió y puede arreglar reinstalándolo. Es
    /// otra cosa distinta de no soportar, y confundirlas es el defecto que esta distinción
    /// arregló en su otra dirección.
    #[test]
    fn un_modelo_instalado_que_se_rompio_invita_a_reinstalar() {
        let modelos = temporal("roto");
        std::fs::create_dir_all(super::super::instalacion::carpeta(
            &modelos,
            &GRANITE_107M_MULTILINGUAL,
        ))
        .expect("carpeta de un modelo sin sus archivos");
        let mut proveedor = LocalProvider::detectar(&modelos, fabrica());
        proveedor.recursos = Recursos {
            memoria_total: Some(16 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: Acelerador::Cpu,
        };

        assert_eq!(
            proveedor.readiness(),
            Availability::Absent(Absence::NotInstalled)
        );
    }

    /// Con pesos delante y máquina capaz, el proveedor se declara listo. Sin
    /// esta mitad, la prueba de arriba pasaría igual con un `readiness` que
    /// dijera siempre `NotInstalled`.
    #[test]
    fn un_equipo_capaz_con_pesos_se_declara_listo() {
        let modelos = temporal("con-pesos");
        disponer(
            &super::super::instalacion::carpeta(&modelos, &GRANITE_107M_MULTILINGUAL),
            &GRANITE_107M_MULTILINGUAL,
        );

        let mut proveedor = LocalProvider::detectar(&modelos, fabrica());
        proveedor.recursos = Recursos {
            memoria_total: Some(16 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: Acelerador::Cpu,
        };

        assert!(proveedor.readiness().is_ready());
    }

    /// **Un tope de memoria por debajo del modelo lo deja sin cargar**, con su
    /// propio motivo: ni `NotSupported`, que diría que no hay nada que hacer, ni
    /// `NotInstalled`, que invitaría a descargar otra vez lo que ya está. Y subir
    /// el tope lo devuelve sin reiniciar.
    #[test]
    fn un_modelo_que_no_cabe_en_el_tope_de_memoria_no_se_ofrece() {
        use crate::ai::presupuesto::Presupuesto;

        let modelos = temporal("tope");
        disponer(
            &super::super::instalacion::carpeta(&modelos, &GRANITE_107M_MULTILINGUAL),
            &GRANITE_107M_MULTILINGUAL,
        );
        let mut proveedor = LocalProvider::detectar(&modelos, Presupuesto::desde(0.0, 512.0, 8));
        proveedor.recursos = Recursos {
            memoria_total: Some(16 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: Acelerador::Cpu,
        };

        assert_eq!(
            proveedor.readiness(),
            Availability::Absent(Absence::OverBudget)
        );
        assert_eq!(
            proveedor.embed(&["hola".to_owned()]),
            Err(crate::error::NbError::ComponentUnavailable)
        );

        proveedor
            .residente()
            .aplicar(Presupuesto::desde(0.0, 1024.0, 8));
        assert!(proveedor.readiness().is_ready());
    }

    /// Lo que no es un modelo del catálogo instalado no cuenta, aunque lo
    /// parezca.
    ///
    /// Tres casos que la versión daba por buenos o no distinguía: un
    /// `.safetensors` suelto que no es del catálogo, una descarga a medias con los
    /// archivos ya completos en su carpeta temporal, y la carpeta del modelo con
    /// un archivo truncado. Los tres harían que el proveedor se declarara listo y
    /// fallara al usarse.
    #[test]
    fn lo_que_no_es_un_modelo_del_catalogo_instalado_no_cuenta() {
        let modelos = temporal("basura");
        std::fs::write(modelos.join("otro.safetensors"), b"no es del catalogo").expect("archivo");
        disponer(
            &modelos.join(format!(".{}.parcial", GRANITE_107M_MULTILINGUAL.id)),
            &GRANITE_107M_MULTILINGUAL,
        );
        let carpeta = super::super::instalacion::carpeta(&modelos, &GRANITE_107M_MULTILINGUAL);
        disponer(&carpeta, &GRANITE_107M_MULTILINGUAL);
        std::fs::File::options()
            .write(true)
            .open(carpeta.join("model.safetensors"))
            .and_then(|f| f.set_len(1))
            .expect("truncar");

        let mut proveedor = LocalProvider::detectar(&modelos, fabrica());
        proveedor.recursos = Recursos {
            memoria_total: Some(16 * 1024 * 1024 * 1024),
            nucleos: 8,
            acelerador: Acelerador::Cpu,
        };

        assert_eq!(
            proveedor.readiness(),
            Availability::Absent(Absence::NotInstalled)
        );
    }

    /// Que no exista el directorio de modelos es lo normal en una instalación
    /// recién hecha, y no puede ser un error.
    #[test]
    fn sin_directorio_de_modelos_el_proveedor_se_construye_igual() {
        let inexistente = crate::paths::temporal_de_pruebas().join("nb-ia-no-existe-jamas");
        let _ = std::fs::remove_dir_all(&inexistente);

        let proveedor = LocalProvider::detectar(&inexistente, fabrica());
        assert!(!proveedor.hay_pesos());
        assert!(!proveedor.readiness().is_ready());
    }

    /// El proveedor local **no puede** declararse externo: si pudiera, el texto
    /// del documento saldría del dispositivo por el camino automático.
    #[test]
    fn el_proveedor_local_se_queda_en_el_dispositivo() {
        let proveedor = LocalProvider::detectar(&temporal("placement"), fabrica());

        assert_eq!(proveedor.placement(), Placement::Local);
        assert!(!proveedor.placement().sale_del_dispositivo());
        assert!(proveedor.host().is_none());
    }
}
