//! Exportación de diagnósticos.
//!
//! # Para qué existe
//!
//! Para que alguien pueda contar qué le pasó sin tener que enseñar su
//! biblioteca. Cuando algo falla, lo que hace falta saber es la versión, el
//! esquema, qué componentes están vivos y qué decían los registros; lo que **no**
//! hace falta saber es qué libros tiene, cómo se llama su usuario ni dónde los
//! guarda.
//!
//! # La regla, y por qué se comprueba dos veces
//!
//! El paquete **no puede contener rutas de usuario, títulos ni secretos**. Eso
//! ya lo deberían garantizar `redact_path` y `redact_text` en el punto donde se
//! escribe cada línea de registro; pero ese innegociable es, según el propio
//! contrato del proyecto, **parcial**: nada obliga a llamarlas. Así que aquí se
//! comprueba otra vez, sobre el texto ya montado, justo antes de entregarlo.
//!
//! No es desconfianza de la primera capa: es que la consecuencia de que falle
//! una vez es que un usuario mande su historial de lectura a un desconocido
//! creyendo que manda un informe técnico. Una segunda comprobación barata contra
//! un daño irreversible se paga sola.
//!
//! # Por qué hay vista previa
//!
//! Porque quien lo envía tiene derecho a leer antes lo que envía. Un paquete de
//! diagnóstico que se genera y se sube sin enseñarse es exactamente la clase de
//! cosa que la Constitución no permite: nada sale del dispositivo sin
//! consentimiento, y consentir sin ver no es consentir.
//!
//! Contrato: `docs/audit/privacy.md`.

use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::error::{NbError, NbResult};
use crate::monitor::ComponentStatus;

/// Cuántas líneas de registro entran en el paquete.
///
/// Las últimas, no las primeras: lo que explica un fallo es lo que pasó justo
/// antes, y un archivo de un día entero es ilegible para quien lo recibe.
const LINEAS: usize = 500;

/// Un paquete de diagnóstico, listo para enseñarse antes de guardarse.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct DiagnosticsPackage {
    /// Versión de la aplicación.
    pub version: String,
    /// Versión del esquema de la base. Dice si una migración quedó a medias.
    pub schema_version: u32,
    /// Sistema operativo y arquitectura. Sin nombre de equipo ni de usuario.
    pub platform: String,
    /// Estado de cada componente, con su motivo cuando lo tiene.
    pub components: Vec<ComponentStatus>,
    /// Últimas líneas del registro, ya redactadas.
    pub log: Vec<String>,
    /// Cuántas líneas se dejaron fuera, para que el recorte no engañe.
    pub omitted_lines: usize,
    /// Cuánto ocupa lo que la aplicación deriva, contra lo que ocupan los
    /// documentos que describe. Nulo si no se pudo medir.
    pub storage: Option<DerivedStorage>,
}

/// Lo que pesa lo derivado, y lo que pesa la biblioteca que describe.
///
/// # Por qué está en el diagnóstico y no en una pantalla
///
/// Porque el innegociable -lo derivado nunca pesa más que la biblioteca- necesita
/// **una forma de comprobarse sobre una instalación cualquiera**, no solo sobre
/// la del desarrollo. El diagnóstico ya sale del equipo cuando su dueño lo pide,
/// ya está redactado, y son cuatro números sin una sola ruta dentro.
///
/// Ver `docs/architecture/performance-budgets.md`, sección 4.4.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct DerivedStorage {
    /// Lo que ocupan los documentos disponibles de la biblioteca.
    ///
    /// Número y no `bigint` en el puente: el JSON lo manda como número, y ciento
    /// cincuenta mil libros son unos 10^11 bytes, muy lejos de lo que pierde
    /// precisión en un número de JavaScript.
    #[ts(type = "number")]
    pub documents_bytes: u64,
    /// Lo que ocupa todo lo que la aplicación deriva de ellos: la base, el
    /// índice que lleva dentro, las portadas y las cachés de página.
    #[ts(type = "number")]
    pub derived_bytes: u64,
    /// Lo derivado en tanto por ciento de los documentos. Es la cifra que se
    /// compara con el presupuesto.
    pub percent_of_documents: f64,
    /// El presupuesto vigente, para que el número de arriba se lea solo.
    pub budget_percent: f64,
}

/// El presupuesto de almacenamiento derivado, en tanto por ciento.
///
/// **De dónde sale el número**, que es lo que lo hace un presupuesto y no una
/// preferencia. Medido el 2026-09-10 sobre 300 EPUB del corpus real:
///
/// * El texto plano de un libro pesa el **60,8 %** de lo que ocupa su archivo
///   -mediana 85,5 %, y en uno de cada diez pasa del 145 %, porque el EPUB va
///   comprimido y el texto no-. Guardarlo entero, como se hacía, no cabe bajo
///   ningún presupuesto por debajo del 100 %.
/// * Las portadas a resolución original pesan el **20,7 %**, y las tiene el
///   100 % de los libros.
///
/// Con 20 el presupuesto obliga a lo que hay que hacer y deja sitio para
/// hacerlo: portadas reducidas al tamaño que la interfaz enseña -unos cuatro
/// puntos-, índice sin duplicar el texto, y el catálogo, que es despreciable.
/// Con 100 no obligaría a nada hasta que ya fuera tarde, y con 10 no cabría el
/// índice, que es lo que hace que buscar funcione.
pub const PRESUPUESTO_DERIVADO: f64 = 20.0;

/// Lo que hace que una línea no pueda salir del equipo.
///
/// Se buscan **formas**, no palabras concretas: una ruta absoluta, algo que
/// parece una clave, una dirección de correo. Buscar palabras prohibidas sería
/// perseguir una lista infinita.
fn linea_peligrosa(linea: &str) -> Option<&'static str> {
    let minuscula = linea.to_lowercase();

    // Una ruta absoluta lleva dentro el nombre de usuario del sistema, que es
    // identificable por sí solo.
    if linea.contains("/home/")
        || linea.contains("/Users/")
        || minuscula.contains("c:\\users\\")
        || linea.contains("/var/home/")
    {
        return Some("ruta-de-usuario");
    }

    // Algo con pinta de secreto. El signo igual y las comillas cubren las dos
    // formas en que un valor acaba en un registro.
    for marca in [
        "token",
        "password",
        "contraseña",
        "api_key",
        "apikey",
        "secret",
    ] {
        if minuscula.contains(marca) && (linea.contains('=') || linea.contains(": ")) {
            return Some("credencial");
        }
    }

    // Una dirección de correo identifica a una persona.
    if linea.contains('@') && linea.contains('.') && !linea.contains("@nb-books") {
        let sospechoso = linea
            .split_whitespace()
            .any(|palabra| palabra.contains('@') && palabra.split('@').count() == 2);
        if sospechoso {
            return Some("correo");
        }
    }

    None
}

/// Sustituye una línea que no puede salir por la razón de que no puede salir.
///
/// Se sustituye y **no se borra**: un registro con huecos silenciosos hace
/// perder el tiempo a quien lo lee buscando lo que falta. Diciendo por qué falta,
/// quien diagnostica sabe que ahí hubo algo y sabe qué era.
///
/// El motivo es un **código** y no una frase, y eso no es cuestión de estilo: la
/// primera versión decía "posible secreto", y esa frase contiene la palabra que
/// dispara la comprobación, así que la segunda pasada rechazaba el paquete por
/// culpa de su propio aviso. Una regla que se detecta a sí misma no protege de
/// nada: solo impide entregar nada.
fn sanear(linea: &str) -> String {
    match linea_peligrosa(linea) {
        Some(motivo) => format!("<línea retirada: {motivo}>"),
        None => linea.to_owned(),
    }
}

/// Lee las últimas líneas del registro más reciente del directorio.
///
/// Un registro que no se puede leer **no impide** exportar: el resto del paquete
/// —versión, esquema, componentes— sigue sirviendo, y negarse a generar nada
/// porque falta una parte deja a quien tiene el problema sin poder contarlo.
fn ultimas_lineas(logs_dir: &Path) -> (Vec<String>, usize) {
    let Ok(entradas) = std::fs::read_dir(logs_dir) else {
        return (Vec::new(), 0);
    };

    let mut archivos: Vec<_> = entradas
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    archivos.sort();

    let Some(ultimo) = archivos.last() else {
        return (Vec::new(), 0);
    };
    let Ok(contenido) = std::fs::read_to_string(ultimo) else {
        return (Vec::new(), 0);
    };

    let todas: Vec<&str> = contenido.lines().collect();
    let omitidas = todas.len().saturating_sub(LINEAS);
    let ultimas = todas
        .iter()
        .skip(omitidas)
        .map(|l| sanear(l))
        .collect::<Vec<_>>();

    (ultimas, omitidas)
}

/// Monta el paquete de diagnóstico.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede consultar la versión del esquema.
pub fn collect(
    logs_dir: &Path,
    schema_version: u32,
    components: Vec<ComponentStatus>,
    storage: Option<DerivedStorage>,
) -> NbResult<DiagnosticsPackage> {
    let (log, omitted_lines) = ultimas_lineas(logs_dir);

    Ok(DiagnosticsPackage {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version,
        // Sistema y arquitectura, y nada más: el nombre del equipo suele ser el
        // nombre de su dueño.
        platform: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        components,
        log,
        omitted_lines,
        storage,
    })
}

/// Mide lo derivado contra los documentos que describe.
///
/// Los documentos se cuentan por lo que dice el catálogo y no recorriendo el
/// disco: una biblioteca **referenciada** vive en carpetas que no son nuestras,
/// y recorrerlas para medir sería exactamente el tipo de trabajo que este
/// producto existe para no hacerle a nadie. Lo derivado sí se recorre, porque es
/// todo nuestro.
///
/// Devuelve `None` si no se puede medir; un diagnóstico sin esta cifra sigue
/// sirviendo para lo demás.
#[must_use]
pub fn medir_almacenamiento(
    conn: &Connection,
    paths: &crate::paths::AppPaths,
) -> Option<DerivedStorage> {
    let documentos: i64 = conn
        .query_row(
            "SELECT COALESCE(sum(byte_size), 0) FROM files WHERE availability = 'available'",
            [],
            |fila| fila.get(0),
        )
        .ok()?;
    let documentos = u64::try_from(documentos).unwrap_or(0);

    // La base, con su diario: el WAL de una sesión larga no es despreciable.
    let base = paths.database();
    let mut derivado = tamano_de_archivo(&base);
    for sufijo in ["-wal", "-shm"] {
        let mut companero = base.clone().into_os_string();
        companero.push(sufijo);
        derivado += tamano_de_archivo(Path::new(&companero));
    }
    // Y la caché entera: portadas, páginas y recursos.
    derivado += tamano_de_arbol(&paths.cache);

    // Y el audio preparado, que vive en datos pero se deriva de los
    // libros igual que el índice. Es lo que decide cuántos audiolibros caben: a
    // once megabytes por hora, veinte libros de diez horas son 2,2 GB. Contarlo
    // aquí es lo que permite que la preparación se niegue antes de empezar en vez
    // de descubrirlo con el disco lleno.
    derivado += tamano_de_arbol(&paths.audio());

    /*
     * Y el respaldo de la última migración, que es una copia completa de la base
     * y antes no lo contaba nadie. Un duplicado de la base no deja
     * de ocupar por llamarse respaldo, y sobre la biblioteca real eso eran 24 GB
     * invisibles para el presupuesto que existe justamente para vigilar esto.
     */
    if let (Some(directorio), Some(nombre)) = (base.parent(), base.file_name()) {
        let prefijo = format!("{}.v", nombre.to_string_lossy());
        if let Ok(entradas) = std::fs::read_dir(directorio) {
            for entrada in entradas.flatten() {
                let suyo = entrada.file_name();
                let suyo = suyo.to_string_lossy();
                if suyo.starts_with(&prefijo) && suyo.ends_with(".backup") {
                    derivado += tamano_de_archivo(&entrada.path());
                }
            }
        }
    }

    // El tope es el que haya elegido quien tiene el disco, dentro de los
    // que se admiten. Uno ilegible cae en el de fábrica, que es el más estricto.
    let presupuesto = match crate::settings::get(conn, crate::settings::SettingKey::DerivedBudget) {
        Ok(crate::settings::SettingValue::Number(n))
            if crate::settings::PRESUPUESTOS_ADMITIDOS.contains(&n) =>
        {
            n
        }
        _ => PRESUPUESTO_DERIVADO,
    };

    let porcentaje = if documentos == 0 {
        0.0
    } else {
        derivado as f64 * 100.0 / documentos as f64
    };

    Some(DerivedStorage {
        documents_bytes: documentos,
        derived_bytes: derivado,
        percent_of_documents: porcentaje,
        budget_percent: presupuesto,
    })
}

fn tamano_de_archivo(ruta: &Path) -> u64 {
    std::fs::metadata(ruta).map(|m| m.len()).unwrap_or(0)
}

/// Suma lo que ocupa un árbol de directorios.
///
/// Sin seguir enlaces simbólicos: uno que apuntara fuera de la caché contaría
/// como derivado algo que no lo es, y uno que apuntara a un ancestro no
/// terminaría nunca.
fn tamano_de_arbol(raiz: &Path) -> u64 {
    let Ok(entradas) = std::fs::read_dir(raiz) else {
        return 0;
    };

    let mut total = 0;
    for entrada in entradas.flatten() {
        let Ok(tipo) = entrada.file_type() else {
            continue;
        };
        if tipo.is_symlink() {
            continue;
        }
        if tipo.is_dir() {
            total += tamano_de_arbol(&entrada.path());
        } else {
            total += entrada.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

/// Serializa el paquete y **comprueba otra vez** que no lleva nada del usuario.
///
/// La comprobación se hace sobre el texto final, que es lo que de verdad sale
/// del equipo: sobre las partes por separado siempre queda la duda de si al
/// juntarlas se coló algo.
///
/// # Errors
///
/// [`NbError::Internal`] si la serialización falla, y [`NbError::PathNotAllowed`]
/// si el texto final contiene algo que no puede salir. Ese segundo error no
/// debería ocurrir nunca: significa que la redacción de la capa de abajo falló, y
/// entonces lo correcto es no entregar nada.
pub fn render(paquete: &DiagnosticsPackage) -> NbResult<String> {
    let texto = serde_json::to_string_pretty(paquete).map_err(|_| NbError::Internal)?;

    for (numero, linea) in texto.lines().enumerate() {
        if let Some(motivo) = linea_peligrosa(linea) {
            tracing::error!(
                linea = numero,
                motivo,
                "el paquete de diagnóstico llevaba algo que no puede salir; no se entrega"
            );
            return Err(NbError::PathNotAllowed);
        }
    }

    Ok(texto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_ruta_de_usuario_no_sale_del_equipo() {
        assert_eq!(
            sanear("abriendo /home/ana/Libros/Ulises.epub"),
            "<línea retirada: ruta-de-usuario>"
        );
        assert_eq!(
            sanear("abriendo C:\\Users\\Ana\\Libros\\Ulises.epub"),
            "<línea retirada: ruta-de-usuario>"
        );
    }

    #[test]
    fn algo_con_pinta_de_secreto_tampoco() {
        assert_eq!(sanear("api_key=sk-abcdef"), "<línea retirada: credencial>");
        assert_eq!(
            sanear("Authorization token: abcdef"),
            "<línea retirada: credencial>"
        );
    }

    #[test]
    fn una_direccion_de_correo_tampoco() {
        assert_eq!(
            sanear("registrado como ana@ejemplo.org"),
            "<línea retirada: correo>"
        );
    }

    #[test]
    fn lo_que_sirve_para_diagnosticar_se_conserva() {
        // Si se retirara esto, el paquete no serviría para nada y la exportación
        // sería una función que se ve bonita y no ayuda a nadie.
        let linea =
            "2026-08-28T10:00:00Z  WARN nb_books_lib::db: no se pudo abrir; code=STORAGE_FAILURE";
        assert_eq!(sanear(linea), linea);
        assert_eq!(sanear("<ruta ext=epub len=42>"), "<ruta ext=epub len=42>");
    }

    #[test]
    fn la_linea_retirada_dice_que_habia_algo() {
        // Un hueco silencioso hace perder el tiempo a quien lee el registro
        // buscando lo que falta.
        assert!(sanear("/home/ana/x.epub").starts_with("<línea retirada"));
    }
}
