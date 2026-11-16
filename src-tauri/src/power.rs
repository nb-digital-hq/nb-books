//! Estado de la alimentación, para no vaciarle la batería a nadie.
//!
//! # Por qué el runtime mira la batería
//!
//! Indexar una biblioteca grande es la tarea más cara que hace esta aplicación:
//! lee cada documento entero y escribe cientos de miles de filas. Hacerlo
//! mientras alguien va en un tren con el 8 % de batería es gastarle la tarde por
//! un trabajo que puede esperar a que enchufe el portátil.
//!
//! # Lo que se puede saber, y lo que no
//!
//! No hay forma portable de preguntar esto. En Linux se lee de `sysfs`, que es
//! la interfaz que el propio núcleo publica y no requiere ninguna dependencia.
//! En las demás plataformas, de momento, la respuesta es **desconocida**, y ante
//! lo desconocido se sigue trabajando: parar por si acaso dejaría sin índice a
//! quien tiene el equipo enchufado a la corriente todo el día.
//!
//! Que la respuesta sea "desconocida" y no "hay batería llena" es deliberado:
//! son cosas distintas, y una prueba que las confunda no detecta que la sonda de
//! una plataforma se ha roto.

/// Cómo está alimentado el equipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Enchufado a la corriente, o sin batería que gastar.
    External,
    /// Con batería, y el porcentaje que queda.
    Battery(u8),
    /// Esta plataforma todavía no lo sabe decir.
    Unknown,
}

/// Por debajo de este porcentaje, el trabajo de fondo se detiene.
///
/// Veinte: es donde los sistemas operativos empiezan a avisar, así que coincide
/// con el momento en que el usuario ya está pensando en enchufar.
pub const UMBRAL: u8 = 20;

impl Power {
    /// Indica si conviene hacer trabajo pesado ahora.
    ///
    /// Lo desconocido cuenta como que sí: ver el porqué en la cabecera.
    #[must_use]
    pub const fn allows_background_work(self) -> bool {
        match self {
            Self::External | Self::Unknown => true,
            Self::Battery(nivel) => nivel >= UMBRAL,
        }
    }
}

/// Lee el estado de la alimentación.
#[must_use]
pub fn state() -> Power {
    #[cfg(target_os = "linux")]
    {
        leer_sysfs().unwrap_or(Power::Unknown)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Power::Unknown
    }
}

#[cfg(target_os = "linux")]
fn leer_sysfs() -> Option<Power> {
    let raiz = std::path::Path::new("/sys/class/power_supply");
    let mut bateria: Option<u8> = None;
    let mut enchufado = false;

    for entrada in std::fs::read_dir(raiz).ok()?.filter_map(Result::ok) {
        let dir = entrada.path();
        let tipo = std::fs::read_to_string(dir.join("type")).ok()?;

        match tipo.trim() {
            // Un cargador conectado manda sobre el nivel de la batería: con la
            // corriente puesta, da igual que esté al 5 %.
            "Mains" | "USB" => {
                if std::fs::read_to_string(dir.join("online")).is_ok_and(|v| v.trim() == "1") {
                    enchufado = true;
                }
            }
            "Battery" => {
                if let Ok(nivel) = std::fs::read_to_string(dir.join("capacity")) {
                    if let Ok(valor) = nivel.trim().parse::<u8>() {
                        // La menor de las baterías, no la primera: un portátil con
                        // dos y una casi vacía está más cerca de apagarse de lo
                        // que diría la otra.
                        bateria = Some(bateria.map_or(valor, |previa: u8| previa.min(valor)));
                    }
                }
            }
            _ => {}
        }
    }

    if enchufado {
        return Some(Power::External);
    }
    match bateria {
        Some(nivel) => Some(Power::Battery(nivel)),
        // Ni cargador ni batería: es un equipo de sobremesa.
        None => Some(Power::External),
    }
}

/// Margen por debajo del umbral pasivo en el que ya se considera que hay presión
/// térmica, en milésimas de grado: **cinco grados**.
///
/// El umbral pasivo es donde el propio núcleo empieza a estrangular el procesador.
/// Esperar a llegar a él es esperar a que la máquina ya vaya lenta; cinco grados
/// antes es suficiente para que la inferencia se aparte antes de empeorarlo.
pub const MARGEN_TERMICO: i64 = 5_000;

/// Si el equipo está bajo presión térmica. `None` si no se puede saber.
///
/// # De dónde sale el umbral
///
/// **No se inventa: lo declara el sistema.** Cada zona térmica publica sus puntos
/// de disparo, y el de tipo `passive` es la temperatura a la que el núcleo empieza
/// a estrangular por su cuenta. Un número fijo escrito aquí valdría para un portátil
/// y no para otro. Lo desconocido cuenta como que no hay presión, por el mismo
/// motivo que la batería: parar por si acaso dejaría sin IA a quien tiene un equipo
/// que no publica sus zonas.
#[must_use]
pub fn presion_termica() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        presion_termica_en(std::path::Path::new("/sys/class/thermal"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Lo mismo que [`presion_termica`], leyendo de una raíz dada. Existe para poder
/// comprobarlo con una zona térmica fabricada.
#[cfg(target_os = "linux")]
#[must_use]
pub fn presion_termica_en(raiz: &std::path::Path) -> Option<bool> {
    let leer = |ruta: std::path::PathBuf| -> Option<i64> {
        std::fs::read_to_string(ruta).ok()?.trim().parse().ok()
    };

    let mut alguna = false;
    for zona in std::fs::read_dir(raiz).ok()?.filter_map(Result::ok) {
        let dir = zona.path();
        let nombre = zona.file_name();
        if !nombre.to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let Some(temperatura) = leer(dir.join("temp")) else {
            continue;
        };
        // El menor umbral pasivo de la zona, que es el primero que se alcanza.
        let pasivo = (0..16)
            .filter(|i| {
                std::fs::read_to_string(dir.join(format!("trip_point_{i}_type")))
                    .is_ok_and(|t| t.trim() == "passive")
            })
            .filter_map(|i| leer(dir.join(format!("trip_point_{i}_temp"))))
            .filter(|&t| t > 0)
            .min();
        let Some(pasivo) = pasivo else {
            continue;
        };
        alguna = true;
        if temperatura >= pasivo - MARGEN_TERMICO {
            return Some(true);
        }
    }
    alguna.then_some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una zona térmica fabricada con su temperatura y su umbral pasivo.
    #[cfg(target_os = "linux")]
    fn zona(raiz: &std::path::Path, n: usize, temperatura: i64, pasivo: Option<i64>) {
        let dir = raiz.join(format!("thermal_zone{n}"));
        std::fs::create_dir_all(&dir).expect("zona");
        std::fs::write(dir.join("temp"), format!("{temperatura}\n")).expect("temp");
        std::fs::write(dir.join("trip_point_0_type"), "critical\n").expect("tipo");
        std::fs::write(dir.join("trip_point_0_temp"), "120000\n").expect("umbral");
        if let Some(pasivo) = pasivo {
            std::fs::write(dir.join("trip_point_1_type"), "passive\n").expect("tipo");
            std::fs::write(dir.join("trip_point_1_temp"), format!("{pasivo}\n")).expect("umbral");
        }
    }

    #[cfg(target_os = "linux")]
    fn raiz(nombre: &str) -> std::path::PathBuf {
        let dir = crate::paths::temporal_de_pruebas()
            .join(format!("nb-termica-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("raíz");
        dir
    }

    /// Cerca del umbral pasivo hay presión; lejos, no. Y lo que decide es el umbral
    /// **pasivo**, no el crítico: esperar al crítico es esperar a que el equipo se
    /// apague.
    #[cfg(target_os = "linux")]
    #[test]
    fn la_presion_termica_se_mide_contra_el_umbral_pasivo_que_declara_el_sistema() {
        let fria = raiz("fria");
        zona(&fria, 0, 50_000, Some(105_000));
        assert_eq!(presion_termica_en(&fria), Some(false));

        let caliente = raiz("caliente");
        zona(&caliente, 0, 50_000, Some(105_000));
        zona(&caliente, 1, 101_000, Some(105_000));
        assert_eq!(
            presion_termica_en(&caliente),
            Some(true),
            "a cuatro grados del pasivo no se vio presión"
        );

        // Una zona sin umbral pasivo no sirve para decidir nada.
        let sin_pasivo = raiz("sin-pasivo");
        zona(&sin_pasivo, 0, 119_000, None);
        assert_eq!(presion_termica_en(&sin_pasivo), None);
    }

    #[test]
    fn la_sonda_termica_de_esta_maquina_no_falla() {
        // Qué devuelve depende de la máquina; lo que no puede es entrar en pánico.
        let _ = presion_termica();
    }

    #[test]
    fn con_bateria_baja_el_trabajo_de_fondo_se_detiene() {
        assert!(!Power::Battery(5).allows_background_work());
        assert!(!Power::Battery(UMBRAL - 1).allows_background_work());
        assert!(Power::Battery(UMBRAL).allows_background_work());
        assert!(Power::Battery(100).allows_background_work());
    }

    #[test]
    fn enchufado_se_trabaja_pase_lo_que_pase_con_la_bateria() {
        assert!(Power::External.allows_background_work());
    }

    #[test]
    fn lo_desconocido_no_impide_trabajar() {
        // Parar por si acaso dejaría sin índice a quien tiene el equipo
        // enchufado a la corriente todo el día en una plataforma cuya sonda
        // todavía no está escrita.
        assert!(Power::Unknown.allows_background_work());
        assert_ne!(
            Power::Unknown,
            Power::Battery(100),
            "desconocido y batería llena son cosas distintas; confundirlas \
             esconde que la sonda de una plataforma se ha roto"
        );
    }

    #[test]
    fn la_sonda_de_esta_maquina_responde_algo_coherente() {
        // No se afirma qué devuelve: depende de si esto corre en un portátil o
        // en un servidor. Se afirma que no entra en pánico y que si dice que hay
        // batería, el porcentaje es un porcentaje.
        if let Power::Battery(nivel) = state() {
            assert!(nivel <= 100);
        }
    }
}
