//! Las plantillas de las funciones de estudio: versionadas, compiladas dentro del
//! binario y separadas del texto del documento.
//!
//! Contrato: `docs/architecture/ai/prompts.md`.
//!
//! # Por qué archivos y no cadenas en el código
//!
//! Porque una plantilla cambia el comportamiento del producto igual que el código,
//! y un cambio de comportamiento tiene que verse en un diff aislado, con su versión,
//! y no enterrado en una función. Viven en `src-tauri/prompts/` y entran con
//! `include_str!`: no se leen del disco al ejecutar, así que nadie puede cambiar
//! una instrucción de la aplicación instalada editando un archivo.
//!
//! # Por qué el documento no es una variable
//!
//! Si `{{fragmentos}}` fuera una variable más, cualquier plantilla podría ponerla
//! dentro de las instrucciones del sistema, y la separación entre lo que manda el
//! producto y lo que trae un libro dependería de que nadie se equivoque al escribir
//! una plantilla. Aquí una plantilla tiene dos partes, la instrucción y la tarea, y
//! el texto del documento lo coloca siempre el runtime **entre las dos y fuera de
//! la instrucción** (ver [`super::contexto`]). Una plantilla que intentara nombrarlo
//! no se deja leer.

use std::sync::LazyLock;

use serde::Serialize;
use ts_rs::TS;

use super::Capability;
use crate::error::{NbError, NbResult};

/// Qué forma tiene la salida que pide una plantilla. Decide con qué se lee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(
    export,
    export_to = "../../src/lib/bindings/",
    rename = "StudyOutputKind"
)]
pub enum FormatoDeSalida {
    Resumen,
    Tarjetas,
    Preguntas,
    Conceptos,
}

impl FormatoDeSalida {
    fn de(nombre: &str) -> Option<Self> {
        Some(match nombre {
            "resumen" => Self::Resumen,
            "tarjetas" => Self::Tarjetas,
            "preguntas" => Self::Preguntas,
            "conceptos" => Self::Conceptos,
            _ => return None,
        })
    }
}

/// Una plantilla ya leída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plantilla {
    /// Identificador estable, como `estudio.resumen`.
    pub id: &'static str,
    /// Sube cada vez que cambia el texto. Viaja con cada salida guardada.
    pub version: u32,
    pub proposito: &'static str,
    pub capacidad: Capability,
    pub salida: FormatoDeSalida,
    /// Las variables que la plantilla usa, y ninguna más.
    pub variables: Vec<&'static str>,
    /// La instrucción del sistema, con sus variables sin rellenar.
    pub sistema: &'static str,
    /// Lo que se pide, que va después del texto del documento.
    pub tarea: &'static str,
}

/// Las plantillas del producto. Añadir una es añadir su archivo aquí.
const FUENTES: &[&str] = &[
    include_str!("../../prompts/estudio.resumen.md"),
    include_str!("../../prompts/estudio.tarjetas.md"),
    include_str!("../../prompts/estudio.preguntas.md"),
    include_str!("../../prompts/estudio.conceptos.md"),
];

/// Todas, leídas una vez. Una que no se deje leer se descarta aquí con su motivo
/// en el registro, y una prueba exige que no haya ninguna así: en la aplicación
/// instalada no puede ocurrir, porque no habría pasado `cargo test`.
static PLANTILLAS: LazyLock<Vec<Plantilla>> = LazyLock::new(|| {
    FUENTES
        .iter()
        .filter_map(|fuente| match leer(fuente) {
            Ok(plantilla) => Some(plantilla),
            Err(motivo) => {
                tracing::error!(motivo, "una plantilla de IA no se deja leer");
                None
            }
        })
        .collect()
});

/// Las plantillas del producto.
#[must_use]
pub fn todas() -> &'static [Plantilla] {
    &PLANTILLAS
}

/// La plantilla de un identificador.
///
/// # Errors
///
/// [`NbError::Internal`] si no existe: los identificadores los escribe el producto,
/// no llegan de fuera.
pub fn plantilla(id: &str) -> NbResult<&'static Plantilla> {
    todas().iter().find(|p| p.id == id).ok_or_else(|| {
        tracing::error!(id, "se pidió una plantilla que no existe");
        NbError::Internal
    })
}

/// Lee una plantilla. El error es el motivo, para el registro y para las pruebas.
///
/// # Errors
///
/// Si falta la cabecera, un campo o una sección, si un campo no vale, si el texto
/// usa una variable que no declara o declara una que no usa, o si nombra el texto
/// del documento.
pub fn leer(fuente: &'static str) -> Result<Plantilla, &'static str> {
    let resto = fuente.strip_prefix("---\n").ok_or("falta la cabecera")?;
    let (cabecera, cuerpo) = resto
        .split_once("\n---\n")
        .ok_or("la cabecera no se cierra")?;

    let campo = |nombre: &str| -> Result<&'static str, &'static str> {
        cabecera
            .lines()
            .find_map(|linea| {
                let (clave, valor) = linea.split_once(':')?;
                (clave.trim() == nombre).then(|| valor.trim())
            })
            .filter(|valor| !valor.is_empty())
            .ok_or("falta un campo de la cabecera")
    };

    let id = campo("id")?;
    let version: u32 = campo("version")?
        .parse()
        .map_err(|_| "la versión no es un número")?;
    if version == 0 {
        return Err("la versión empieza en 1");
    }
    let proposito = campo("proposito")?;
    let capacidad = match campo("capacidad")? {
        "summary" => Capability::Summary,
        "flashcards" => Capability::Flashcards,
        "questions" => Capability::Questions,
        "concepts" => Capability::Concepts,
        _ => return Err("la capacidad no es de generación"),
    };
    let salida = FormatoDeSalida::de(campo("salida")?).ok_or("el formato de salida no existe")?;
    // Puede no haber variables, pero la línea tiene que estar: así una plantilla
    // dice explícitamente que no usa ninguna.
    let variables: Vec<&'static str> = cabecera
        .lines()
        .find_map(|linea| linea.strip_prefix("variables:"))
        .ok_or("falta un campo de la cabecera")?
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();

    let cuerpo = cuerpo.trim_start();
    let cuerpo = cuerpo
        .strip_prefix("# sistema\n")
        .ok_or("falta la sección del sistema")?;
    let (sistema, tarea) = cuerpo
        .split_once("\n# tarea\n")
        .ok_or("falta la sección de la tarea")?;
    let (sistema, tarea) = (sistema.trim(), tarea.trim());
    if sistema.is_empty() || tarea.is_empty() {
        return Err("una sección está vacía");
    }

    let usadas = usadas(sistema).chain(usadas(tarea)).collect::<Vec<_>>();
    if usadas.iter().any(|v| v.contains("fragmento"))
        || variables.iter().any(|v| v.contains("fragmento"))
    {
        return Err("el texto del documento no es una variable");
    }
    if usadas.iter().any(|v| !variables.contains(v)) {
        return Err("usa una variable que no declara");
    }
    if variables.iter().any(|v| !usadas.contains(v)) {
        return Err("declara una variable que no usa");
    }

    Ok(Plantilla {
        id,
        version,
        proposito,
        capacidad,
        salida,
        variables,
        sistema,
        tarea,
    })
}

/// Los nombres de variable de un texto, en orden de aparición.
fn usadas(texto: &'static str) -> impl Iterator<Item = &'static str> {
    texto
        .split("{{")
        .skip(1)
        .filter_map(|trozo| trozo.split_once("}}").map(|(nombre, _)| nombre.trim()))
}

/// Una plantilla con sus variables puestas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rellena {
    pub id: &'static str,
    pub version: u32,
    pub sistema: String,
    pub tarea: String,
}

impl Plantilla {
    /// Pone las variables.
    ///
    /// Los valores los escribe el producto —el idioma, cuántas tarjetas—, pero se
    /// les quitan las llaves igual: un valor no puede abrir otra variable.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si falta una variable o sobra una. Es un defecto del
    /// producto, no algo que pueda provocar una persona.
    pub fn rellenar(&self, valores: &[(&str, &str)]) -> NbResult<Rellena> {
        let completas = self.variables.len() == valores.len()
            && self
                .variables
                .iter()
                .all(|v| valores.iter().any(|(nombre, _)| nombre == v));
        if !completas {
            tracing::error!(id = self.id, "se rellenó una plantilla con otras variables");
            return Err(NbError::Internal);
        }
        let poner = |texto: &str| {
            valores
                .iter()
                .fold(texto.to_owned(), |acumulado, (nombre, valor)| {
                    let limpio = valor.replace(['{', '}'], "");
                    acumulado.replace(&format!("{{{{{nombre}}}}}"), &limpio)
                })
        };
        Ok(Rellena {
            id: self.id,
            version: self.version,
            sistema: poner(self.sistema),
            tarea: poner(self.tarea),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una plantilla del producto que no se deja leer no llega a una persona: se
    /// descartaría en silencio al arrancar.
    #[test]
    fn todas_las_plantillas_del_producto_se_dejan_leer() {
        for fuente in FUENTES {
            let leida = leer(fuente);
            assert!(leida.is_ok(), "{:?} en {}", leida.err(), &fuente[..60]);
        }
        assert_eq!(todas().len(), FUENTES.len());
        let mut ids: Vec<_> = todas().iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(
            ids.len(),
            FUENTES.len(),
            "hay dos plantillas con el mismo id"
        );
    }

    /// **Cambiar el texto es cambiar la versión.** La huella de cada plantilla está
    /// escrita aquí con su versión; si el texto cambia y la versión no, esto falla,
    /// y quien lo cambió tiene que subir la versión y actualizar la huella a la vez.
    #[test]
    fn el_texto_de_una_plantilla_no_cambia_sin_cambiar_su_version() {
        let fijadas: &[(&str, u32, u64)] = &[
            ("estudio.resumen", 3, 3998700826714641521),
            ("estudio.tarjetas", 1, 9044807381983220027),
            ("estudio.preguntas", 2, 10041505012780441457),
            ("estudio.conceptos", 2, 15457047563563134070),
        ];
        let mut distintas = Vec::new();
        for (id, version, huella_fijada) in fijadas {
            let p = plantilla(id).expect("existe");
            let actual = huella(&format!("{}\n{}", p.sistema, p.tarea));
            if p.version != *version || actual != *huella_fijada {
                distintas.push(format!("(\"{id}\", {}, {actual})", p.version));
            }
        }
        assert!(
            distintas.is_empty(),
            "sube la versión y fija: {}",
            distintas.join(", ")
        );
    }

    fn huella(texto: &str) -> u64 {
        texto.bytes().fold(1_469_598_103_934_665_603, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(1_099_511_628_211)
        })
    }

    /// El texto del documento no puede entrar por una variable, ni en la
    /// instrucción ni en la tarea.
    #[test]
    fn una_plantilla_que_nombra_el_documento_no_se_deja_leer() {
        let en_el_sistema = "---\nid: x\nversion: 1\nproposito: p\ncapacidad: summary\nsalida: resumen\nvariables: fragmentos\n---\n# sistema\nLee {{fragmentos}}\n\n# tarea\nResume.\n";
        assert_eq!(
            leer(en_el_sistema),
            Err("el texto del documento no es una variable")
        );
        let sin_declarar = "---\nid: x\nversion: 1\nproposito: p\ncapacidad: summary\nsalida: resumen\nvariables: idioma\n---\n# sistema\nEn {{idioma}}\n\n# tarea\n{{fragmentos}}\n";
        assert_eq!(
            leer(sin_declarar),
            Err("el texto del documento no es una variable")
        );
    }

    #[test]
    fn las_variables_declaradas_y_las_usadas_coinciden() {
        let sobra = "---\nid: x\nversion: 1\nproposito: p\ncapacidad: summary\nsalida: resumen\nvariables: idioma, cuantas\n---\n# sistema\nEn {{idioma}}\n\n# tarea\nResume.\n";
        assert_eq!(leer(sobra), Err("declara una variable que no usa"));
        let falta = "---\nid: x\nversion: 1\nproposito: p\ncapacidad: summary\nsalida: resumen\nvariables: idioma\n---\n# sistema\nEn {{idioma}}\n\n# tarea\n{{cuantas}}\n";
        assert_eq!(leer(falta), Err("usa una variable que no declara"));
        let sin_tarea = "---\nid: x\nversion: 1\nproposito: p\ncapacidad: summary\nsalida: resumen\nvariables: \n---\n# sistema\nHola\n";
        assert_eq!(leer(sin_tarea), Err("falta la sección de la tarea"));
    }

    #[test]
    fn rellenar_exige_todas_las_variables_y_un_valor_no_abre_otra() {
        let p = plantilla("estudio.tarjetas").expect("existe");
        assert_eq!(p.rellenar(&[("idioma", "español")]), Err(NbError::Internal));
        let r = p
            .rellenar(&[("idioma", "{{cuantas}}"), ("cuantas", "5")])
            .expect("rellena");
        assert!(!r.sistema.contains("{{") && !r.tarea.contains("{{"));
        assert!(r.tarea.contains("Crea 5 tarjetas"));
        assert!(r.sistema.contains("Responde en cuantas."));
    }
}
