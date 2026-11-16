//! Lo que la inferencia puede gastar, y cuándo se aparta.
//!
//! Contrato: `docs/architecture/ai/philosophy.md`, sección 3 —"el presupuesto de
//! recursos es configurable y tiene un tope duro por defecto"—.//! el presupuesto de la IA, cuyo criterio es que **la inferencia nunca degrada el presupuesto de
//! fotograma de la lectura**.
//!
//! # Tres mecanismos, y por qué los tres
//!
//! * **Un grupo de hilos propio con tope.** Candle reparte su trabajo con `rayon`,
//!   y el grupo global de `rayon` usa todos los núcleos. Con todos ocupados, el hilo
//!   que pinta la ventana compite con la inferencia por el procesador. Así que la
//!   inferencia corre **dentro de un grupo propio** con la mitad de los núcleos de
//!   fábrica, y nunca todos. Comprobado en el código de `gemm`, que es quien
//!   multiplica las matrices: reparte con `into_par_iter`, que corre en el grupo
//!   actual, así que el tope acota de verdad y no solo en apariencia.
//! * **Prioridad baja en el sistema operativo.** Un tope de hilos deja núcleos
//!   libres, pero el planificador no sabe que el hilo que pinta importa más. Con la
//!   prioridad bajada, si algo tiene que esperar, espera la inferencia.
//! * **Suspensión con batería baja o presión térmica**, que es lo que la ficha pide
//!   y lo que el indexador ya hace con la batería: un trabajo que puede esperar no
//!   le vacía el portátil a nadie ni empuja al equipo contra su límite térmico.

use std::sync::Arc;

use crate::monitor::ComponentState;

/// Por qué la inferencia está apartada ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suspension {
    /// La batería está por debajo del umbral del trabajo de fondo.
    Bateria,
    /// Alguna zona térmica está a menos de cinco grados de su umbral pasivo.
    Temperatura,
}

impl Suspension {
    /// Código canónico del motivo, para el monitor de salud.
    #[must_use]
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::Bateria => "AI_SUSPENDED_BATTERY",
            Self::Temperatura => "AI_SUSPENDED_HEAT",
        }
    }

    /// `Stopped` y no `Degraded`: `Degraded` es utilizable, y la interfaz ofrecería
    /// una función que no va a responder. `Stopped` la oculta sin pedir atención,
    /// y el motivo se ve en Ajustes. No es una avería: se pasa sola.
    #[must_use]
    pub const fn component_state(self) -> ComponentState {
        ComponentState::Stopped
    }
}

/// Si la inferencia tiene que apartarse ahora.
///
/// Se pregunta en cada petición y no una vez al arrancar, igual que el indexador
/// pregunta por la batería antes de cada documento: una sesión puede empezar
/// enchufada y terminar en un tren.
#[must_use]
pub fn suspension() -> Option<Suspension> {
    suspension_con(crate::power::state(), crate::power::presion_termica())
}

/// Lo mismo, con los datos dados. Existe para poder comprobarlo sin batería ni
/// termómetro.
#[must_use]
pub const fn suspension_con(
    energia: crate::power::Power,
    presion: Option<bool>,
) -> Option<Suspension> {
    if !energia.allows_background_work() {
        return Some(Suspension::Bateria);
    }
    // Lo desconocido no aparta: parar por si acaso dejaría sin IA a quien tiene un
    // equipo que no publica sus zonas térmicas.
    if matches!(presion, Some(true)) {
        return Some(Suspension::Temperatura);
    }
    None
}

/// Lo que la inferencia puede gastar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presupuesto {
    /// Hilos del grupo de la inferencia. Nunca cero y nunca todos los núcleos.
    pub hilos: usize,
    /// Memoria máxima de un modelo cargado, en bytes.
    pub memoria: u64,
}

impl Presupuesto {
    /// El presupuesto que resulta de las preferencias guardadas.
    ///
    /// `hilos` a cero es automático: la mitad de los núcleos.
    #[must_use]
    pub fn desde(hilos_pedidos: f64, memoria_mib: f64, nucleos: usize) -> Self {
        let tope = nucleos.saturating_sub(1).max(1);
        let hilos = if hilos_pedidos >= 1.0 {
            // `as` y no `try_from`: la preferencia ya se validó al guardarse como
            // entero entre cero y los núcleos menos uno, y aquí solo se acota por si
            // la base se escribió a mano o la máquina tiene menos núcleos que
            // cuando se guardó.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let pedidos = hilos_pedidos as usize;
            pedidos.min(tope)
        } else {
            super::hardware::hilos_para_inferencia(nucleos)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let memoria = (memoria_mib.max(0.0) as u64) * 1024 * 1024;
        Self { hilos, memoria }
    }

    /// El de fábrica: automático y 1024 MiB.
    #[must_use]
    pub fn de_fabrica(nucleos: usize) -> Self {
        Self::desde(0.0, 1024.0, nucleos)
    }

    /// El que está guardado en las preferencias, para los núcleos de esta máquina.
    ///
    /// Lo que no se pueda leer cuenta como lo de fábrica: una base que no responde
    /// no puede dejar a la inferencia sin tope, y el de fábrica **es** un tope.
    #[must_use]
    pub fn guardado(conn: &rusqlite::Connection) -> Self {
        use crate::settings::{self, SettingKey, SettingValue};

        let numero = |clave: SettingKey, defecto: f64| match settings::get(conn, clave) {
            Ok(SettingValue::Number(n)) if n.is_finite() => n,
            _ => defecto,
        };
        Self::desde(
            numero(SettingKey::AiThreads, 0.0),
            numero(SettingKey::AiMemoryLimitMb, 1024.0),
            super::hardware::nucleos(),
        )
    }
}

/// Construye el grupo de hilos de la inferencia.
///
/// # Errors
///
/// Devuelve el error de `rayon` si el sistema no deja crear hilos, que el
/// llamante convierte en inferencia no disponible y no en un fallo de la
/// aplicación.
pub fn grupo_de_hilos(hilos: usize) -> Result<Arc<rayon::ThreadPool>, rayon::ThreadPoolBuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(hilos.max(1))
        .thread_name(|i| format!("nb-ia-{i}"))
        .start_handler(|_| bajar_prioridad())
        .build()
        .map(Arc::new)
}

/// Cuánto se baja la prioridad de un hilo de inferencia en Linux: `nice` 10.
///
/// Diez de diecinueve: por debajo de todo lo interactivo y todavía por encima de
/// lo que el propio sistema marca como trabajo de mantenimiento, que suele ir al
/// máximo. Bajar la prioridad no exige permisos; subirla sí.
pub const NICE_DE_LA_INFERENCIA: i32 = 10;

/// Baja la prioridad del hilo actual.
///
/// **Solo en Linux**, que es donde se puede comprobar. En macOS y Windows las
/// llamadas son otras —una clase de calidad de servicio, una prioridad de hilo— y
/// escribirlas sin poder ejecutarlas es la clase de código que estuvo semanas sin
/// compilar en Windows. Allí el tope de hilos se aplica igual y la prioridad no se
/// toca: está anotado en `deferred-verification.md` (V23).
pub fn bajar_prioridad() {
    #[cfg(target_os = "linux")]
    // SEGURIDAD: `gettid` no recibe argumentos, y `setpriority` recibe dos
    // enteros; ninguna toca memoria del proceso. En Linux, `PRIO_PROCESS` con el
    // identificador de un hilo afecta solo a ese hilo, que es lo que se quiere.
    unsafe {
        let hilo = libc::gettid();
        #[allow(clippy::cast_sign_loss)]
        let resultado = libc::setpriority(
            libc::PRIO_PROCESS,
            hilo as libc::id_t,
            NICE_DE_LA_INFERENCIA,
        );
        if resultado != 0 {
            tracing::info!("no se pudo bajar la prioridad de un hilo de inferencia");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::Power;

    /// Nunca todos los núcleos, ni pidiéndolos ni en automático: el hilo que pinta
    /// tiene que tener sitio siempre.
    #[test]
    fn el_presupuesto_nunca_da_a_la_inferencia_todos_los_nucleos() {
        for nucleos in 2..=64_usize {
            for pedidos in [0.0, 1.0, (nucleos - 1) as f64, nucleos as f64, 1_000.0] {
                let p = Presupuesto::desde(pedidos, 1024.0, nucleos);
                assert!(p.hilos >= 1);
                assert!(
                    p.hilos < nucleos,
                    "con {nucleos} núcleos y {pedidos} pedidos se dieron {}",
                    p.hilos
                );
            }
        }
        assert_eq!(Presupuesto::de_fabrica(8).hilos, 4, "de fábrica, la mitad");
        assert_eq!(Presupuesto::de_fabrica(8).memoria, 1024 * 1024 * 1024);
    }

    /// Lo guardado es lo que se usa, y lo que no se puede leer es lo de fábrica:
    /// una preferencia rota no deja a la inferencia sin tope.
    #[test]
    fn el_presupuesto_sale_de_las_preferencias_guardadas() {
        use crate::settings::{self, SettingKey, SettingValue};

        let db = crate::db::Database::open_in_memory().expect("base");
        let conn = db.connection().expect("conexión");
        let nucleos = crate::ai::hardware::nucleos();

        assert_eq!(
            Presupuesto::guardado(&conn),
            Presupuesto::de_fabrica(nucleos)
        );

        settings::set(
            &conn,
            SettingKey::AiMemoryLimitMb,
            &SettingValue::Number(2048.0),
        )
        .expect("memoria");
        settings::set(&conn, SettingKey::AiThreads, &SettingValue::Number(1.0)).expect("hilos");
        let guardado = Presupuesto::guardado(&conn);
        assert_eq!(guardado.memoria, 2048 * 1024 * 1024);
        assert_eq!(guardado.hilos, 1);
    }

    /// Con batería baja o calor se aparta; enchufado y frío, no; y lo desconocido
    /// no aparta.
    #[test]
    fn la_inferencia_se_aparta_con_bateria_baja_o_presion_termica() {
        assert_eq!(
            suspension_con(Power::Battery(5), Some(false)),
            Some(Suspension::Bateria)
        );
        assert_eq!(
            suspension_con(Power::External, Some(true)),
            Some(Suspension::Temperatura)
        );
        assert_eq!(suspension_con(Power::External, Some(false)), None);
        assert_eq!(suspension_con(Power::Unknown, None), None);
        assert_eq!(suspension_con(Power::Battery(80), None), None);
    }

    /// El grupo tiene los hilos pedidos, y dentro de él `rayon` ve ese número y no
    /// el de la máquina. Es lo que hace que el tope acote el trabajo de Candle.
    #[test]
    fn dentro_del_grupo_de_la_inferencia_solo_hay_los_hilos_del_presupuesto() {
        let grupo = grupo_de_hilos(2).expect("grupo");
        assert_eq!(grupo.install(rayon::current_num_threads), 2);
    }

    /// Los hilos del grupo corren con la prioridad bajada. Comprobado leyendo la
    /// prioridad del propio hilo desde dentro, no suponiéndola.
    #[cfg(target_os = "linux")]
    #[test]
    fn los_hilos_de_la_inferencia_corren_con_la_prioridad_bajada() {
        let grupo = grupo_de_hilos(1).expect("grupo");
        let nice = grupo.install(|| {
            // SEGURIDAD: lectura de la prioridad del hilo actual; solo enteros.
            unsafe {
                let hilo = libc::gettid();
                #[allow(clippy::cast_sign_loss)]
                libc::getpriority(libc::PRIO_PROCESS, hilo as libc::id_t)
            }
        });
        assert!(
            nice >= NICE_DE_LA_INFERENCIA,
            "el hilo de inferencia corre con nice {nice}"
        );
    }
}
