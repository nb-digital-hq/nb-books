//! La puerta de revisión de un paquete de extensión.
//!
//! # Qué es esto y qué no es
//!
//! Es la comprobación que aplica la [política del
//! registro](../../../docs/legal/governance/extension-policy.md) a un paquete
//! concreto. Cada hallazgo cita el requisito que incumple, con su
//! identificador, porque ese identificador es lo que el autor va a leer en el
//! aviso y lo que la política le explica.
//!
//! No es el cajón de arena. El cajón contiene lo que una extensión hace **al
//! ejecutarse**, y sigue conteniéndolo aunque esta puerta se equivoque; esto
//! mira el paquete **antes de que se ejecute nada**, para no distribuir lo que
//! se sabe que va a rebotar. Las dos cosas se solapan a propósito: un módulo
//! que pide abrir un socket lo detiene el anfitrión al instanciarlo y lo
//! detiene esta puerta al revisarlo, y quien lo escribió se entera antes de
//! publicarlo en vez de después.
//!
//! # Por qué corre en local y no solo en integración continua
//!
//! Porque las Actions de la cuenta no arrancan: ninguna ejecución ha llegado
//! nunca a empezar. Una puerta que solo vive en un CI que no se ejecuta no es
//! un mecanismo, es una intención. Esto se ejecuta sobre cualquier directorio,
//! desde el binario `revisar`, y el trabajo de CI —cuando exista— se limitará a
//! invocarlo.
//!
//! # Se revisa cada versión, no el primer envío
//!
//! El vector clásico es una extensión benigna que se vuelve maliciosa en una
//! actualización. Por eso el banco tiene el mismo módulo en dos
//! versiones que se diferencian **en una sola cosa**, y por eso la prueba de
//! aceptación exige las dos direcciones: la segunda se detiene y **la primera
//! pasa**. Una puerta que solo sabe rechazar no demuestra nada.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{NbError, NbResult};

use super::manifest::{self, Capability, Manifest};
use super::signature;
use super::wasm;
use super::Desenlace;

/// Extensiones de archivo que nombran algo que se carga y se ejecuta.
///
/// La lista es corta a propósito: no están `.zip` ni `.json`, que son datos,
/// ni `.rs`, que aparece en los mensajes de pánico de cualquier módulo escrito
/// en Rust y no es código que nadie vaya a cargar.
const EXTENSIONES_DE_CODIGO: &[&str] = &[
    ".wasm", ".so", ".dll", ".dylib", ".js", ".mjs", ".py", ".lua", ".jar", ".exe",
];

/// Funciones de WASI que conceden sistema de archivos, red o señales.
///
/// **Ninguna capacidad del manifiesto puede concederlas**, ni ahora ni después:
/// lo que una extensión puede pedir está en la lista cerrada del esquema
/// publicado, y ahí solo hay salida de red **por la puerta del anfitrión**. Un
/// módulo que importa una de estas pide algo que no existe forma de conceder, y
/// eso se dice al revisarlo en vez de dejar que lo descubra al ejecutarse.
const IMPORTACIONES_SIN_CONCESION: &[&str] = &[
    "fd_prestat_dir_name",
    "fd_prestat_get",
    "fd_readdir",
    "path_create_directory",
    "path_filestat_get",
    "path_filestat_set_times",
    "path_link",
    "path_open",
    "path_readlink",
    "path_remove_directory",
    "path_rename",
    "path_symlink",
    "path_unlink_file",
    "proc_raise",
    "sock_accept",
    "sock_bind",
    "sock_connect",
    "sock_listen",
    "sock_open",
    "sock_recv",
    "sock_send",
    "sock_shutdown",
];

/// El espacio de identificadores reservado a las extensiones de primera parte.
const PREFIJO_RESERVADO: &str = "lat.nbdigitallabs.books.";

/// Nombres admitidos para el archivo de licencia, en minúsculas.
const NOMBRES_DE_LICENCIA: &[&str] = &[
    "license",
    "license.md",
    "license.txt",
    "licence",
    "licence.md",
    "licence.txt",
    "copying",
    "copying.md",
    "copying.txt",
];

/// El nombre de la interfaz que el anfitrión ofrece a los componentes.
///
/// Un componente que la usa la lleva escrita en sus bytes, porque las
/// importaciones del modelo de componentes se codifican por nombre. Es lo que
/// permite distinguir un paquete que puede ejercer una capacidad de uno que la
/// declara y no podría usarla ni queriendo.
const INTERFAZ_DEL_ANFITRION: &[u8] = b"nb:extension/host";

/// Qué hace la revisión con un hallazgo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Gravedad {
    /// El paquete no se publica.
    Detiene,
    /// Se publica y se anota. Existe para lo que es dudoso, no para lo que
    /// incumple: un requisito de la política que se incumple detiene.
    Advierte,
}

/// Un incumplimiento concreto, con el requisito que lo nombra.
#[derive(Debug, Clone, Serialize)]
pub struct Hallazgo {
    /// El identificador del requisito de la política: `R3`, `R5`... Es estable
    /// y es lo que el autor puede buscar en el documento.
    pub requisito: &'static str,
    pub gravedad: Gravedad,
    /// Dónde: el archivo del paquete, o el campo del manifiesto.
    pub donde: String,
    /// Qué se encontró. Concreto y comprobable, nunca una valoración.
    pub que_pasa: String,
}

/// El resultado de revisar un paquete.
#[derive(Debug, Serialize)]
pub struct Informe {
    pub paquete: String,
    /// El identificador de la extensión, si el manifiesto llegó a leerse.
    pub extension: Option<String>,
    pub version: Option<String>,
    /// Cuántos artefactos de WebAssembly se revisaron.
    pub modulos: usize,
    pub hallazgos: Vec<Hallazgo>,
}

impl Informe {
    /// Si el paquete se publica.
    #[must_use]
    pub fn pasa(&self) -> bool {
        !self
            .hallazgos
            .iter()
            .any(|h| h.gravedad == Gravedad::Detiene)
    }

    /// Los requisitos que este informe señala, sin repetir.
    #[must_use]
    pub fn requisitos(&self) -> BTreeSet<&'static str> {
        self.hallazgos.iter().map(|h| h.requisito).collect()
    }
}

/// Lo que la revisión necesita saber y no puede deducir del paquete.
#[derive(Debug, Clone, Copy)]
pub struct Opciones {
    /// Si el envío viene del propio proyecto.
    ///
    /// Es lo único que separa a una extensión de primera parte de una de
    /// tercera en el espacio de nombres reservado, y **no se deduce del
    /// paquete**: deducirlo de su contenido sería dejar que el paquete
    /// decidiera si puede llamarse oficial.
    pub de_primera_parte: bool,
    /// Si un paquete sin firmar se detiene.
    ///
    /// Por omisión sí, porque esta puerta es la del registro y ahí un paquete
    /// sin firma no se distribuye. Se puede apagar para revisar el contenido de
    /// algo que todavía no se ha firmado, que es lo que hace su autor mientras
    /// lo escribe.
    pub exige_firma: bool,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            de_primera_parte: false,
            exige_firma: true,
        }
    }
}

/// Un artefacto de WebAssembly del paquete.
struct Artefacto {
    nombre: String,
    bytes: Vec<u8>,
}

impl Artefacto {
    fn es_componente(&self) -> bool {
        wasm::es_componente(&self.bytes)
    }

    /// Si este artefacto puede llegar a llamar al anfitrión.
    ///
    /// Un módulo suelto no puede: el contexto que se le da está vacío de
    /// capacidades y no importa nada del mundo. Un componente sí, y se sabe
    /// porque lleva el nombre de la interfaz escrito en sus bytes.
    fn alcanza_al_anfitrion(&self) -> bool {
        self.es_componente() && contiene(&self.bytes, INTERFAZ_DEL_ANFITRION)
    }
}

/// Revisa un paquete —un directorio con su manifiesto, sus módulos y su
/// licencia— y devuelve todo lo que encuentra.
///
/// Devuelve **todos** los hallazgos y no el primero, por el mismo motivo que el
/// validador del manifiesto: un autor que arregla de uno en uno vuelve a enviar
/// tantas veces como defectos tenga.
///
/// # Errors
///
/// [`NbError::FileNotFound`] si el directorio no existe o no se puede leer. Que
/// el paquete esté mal **no es un error**: eso son hallazgos.
pub fn revisar(paquete: &Path, opciones: Opciones) -> NbResult<Informe> {
    if !paquete.is_dir() {
        return Err(NbError::FileNotFound);
    }
    let mut hallazgos = Vec::new();

    let manifiesto = leer_manifiesto(paquete, &mut hallazgos);
    let artefactos = artefactos_de(paquete)?;

    revisar_licencia(paquete, &mut hallazgos);
    // Un paquete declarativo -un tema, un fondo sonoro- no trae código que leer.
    if !artefactos.is_empty() {
        revisar_fuente(paquete, &mut hallazgos);
    }
    revisar_drm(paquete, &artefactos, &mut hallazgos);
    if let Some(manifiesto) = &manifiesto {
        revisar_identidad(manifiesto, opciones, &mut hallazgos);
        revisar_texto_de_licencia(paquete, manifiesto, &mut hallazgos);
        revisar_destinos_de_red(manifiesto, &mut hallazgos);
        revisar_firma(paquete, manifiesto, opciones, &mut hallazgos);
        revisar_capacidades_declaradas(manifiesto, &artefactos, &mut hallazgos);
    }
    for artefacto in &artefactos {
        revisar_artefacto(artefacto, manifiesto.as_ref(), &mut hallazgos);
    }

    Ok(Informe {
        paquete: paquete.display().to_string(),
        extension: manifiesto.as_ref().map(|m| m.id.clone()),
        version: manifiesto.as_ref().map(|m| m.version.clone()),
        modulos: artefactos.len(),
        hallazgos,
    })
}

/// R1: el manifiesto existe y vale.
fn leer_manifiesto(paquete: &Path, hallazgos: &mut Vec<Hallazgo>) -> Option<Manifest> {
    let ruta = paquete.join(manifest::ARCHIVO);
    let Ok(texto) = std::fs::read_to_string(&ruta) else {
        hallazgos.push(Hallazgo {
            requisito: "R1",
            gravedad: Gravedad::Detiene,
            donde: manifest::ARCHIVO.to_owned(),
            que_pasa: "el paquete no trae manifiesto, o no se puede leer".to_owned(),
        });
        return None;
    };
    match manifest::parse(&texto) {
        Ok(manifiesto) => Some(manifiesto),
        Err(defectos) => {
            for defecto in defectos {
                let campo = if defecto.field.is_empty() {
                    manifest::ARCHIVO.to_owned()
                } else {
                    format!("{}:{}", manifest::ARCHIVO, defecto.field)
                };
                let esperado = defecto
                    .expected
                    .map(|e| format!(", y se esperaba: {e}"))
                    .unwrap_or_default();
                hallazgos.push(Hallazgo {
                    requisito: "R1",
                    gravedad: Gravedad::Detiene,
                    donde: campo,
                    que_pasa: format!("{:?}{esperado}", defecto.code),
                });
            }
            None
        }
    }
}

/// R2: ni el identificador ni el nombre visible se hacen pasar por oficiales.
///
/// El prefijo reservado se comprueba en el identificador. Y, en el nombre
/// visible, **el nombre del producto escrito de cualquier forma**: sin mayúsculas, sin
/// espacios, sin puntos ni guiones, "NB Books Sync", "nb-books" y "N.B. Books" son lo
/// mismo. Lo que no se ve así es el parecido -"NB Bookz"-, y eso sigue siendo de una
/// persona.
fn revisar_identidad(manifiesto: &Manifest, opciones: Opciones, hallazgos: &mut Vec<Hallazgo>) {
    if opciones.de_primera_parte {
        return;
    }
    if manifiesto.id.starts_with(PREFIJO_RESERVADO) {
        hallazgos.push(Hallazgo {
            requisito: "R2",
            gravedad: Gravedad::Detiene,
            donde: format!("{}:id", manifest::ARCHIVO),
            que_pasa: format!(
                "usa el espacio reservado a las extensiones de primera parte: {PREFIJO_RESERVADO}*"
            ),
        });
    }
    let plegado: String = manifiesto
        .name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if plegado.contains("nbbooks") || se_parece_al_producto(&manifiesto.name) {
        hallazgos.push(Hallazgo {
            requisito: "R2",
            gravedad: Gravedad::Detiene,
            donde: format!("{}:name", manifest::ARCHIVO),
            que_pasa: "el nombre visible lleva el nombre del producto, y se leería como una \
                       extensión oficial"
                .to_owned(),
        });
    }
}

/// R2, el parecido: un nombre que se lee como el del producto aunque no lo
/// escriba igual -"NB Bookz", "N8 Books", "nb-bo0ks"-.
///
/// Se pliegan los caracteres que se confunden a la vista con letras (`0` con
/// `o`, `8` con `b`, `z` y `5` con `s`, `1` con `l`...) y se busca en el nombre un
/// trozo a una sola edición de distancia de "nbbooks". Un nombre legítimo que
/// solo contiene "books" no se acerca: le faltan las dos primeras letras.
fn se_parece_al_producto(nombre: &str) -> bool {
    const OBJETIVO: &[u8] = b"nbbooks";
    let plegado: Vec<u8> = nombre
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| match c.to_ascii_lowercase() {
            '0' => 'o',
            '8' => 'b',
            'z' | '5' => 's',
            '1' | 'i' => 'l',
            '3' => 'e',
            '4' => 'a',
            otra => otra,
        })
        .map(|c| c as u8)
        .collect();
    let objetivo: Vec<u8> = OBJETIVO
        .iter()
        .map(|&c| if c == b'i' { b'l' } else { c })
        .collect();
    (OBJETIVO.len() - 1..=OBJETIVO.len() + 1).any(|largo| {
        plegado
            .windows(largo)
            .any(|trozo| distancia(trozo, &objetivo) <= 1)
    })
}

/// Distancia de edición (inserción, borrado, sustitución y trasposición).
fn distancia(a: &[u8], b: &[u8]) -> usize {
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, fila) in d.iter_mut().enumerate() {
        fila[0] = i;
    }
    for (j, celda) in d[0].iter_mut().enumerate() {
        *celda = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let coste = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + coste);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// R6: el texto de la licencia es el de la licencia declarada.
///
/// Cada identificador SPDX de la expresión del manifiesto tiene que tener su
/// texto en alguno de los archivos de licencia del paquete, reconocido por una
/// frase que solo aparece en esa licencia. Un identificador que la revisión no
/// sabe reconocer detiene el envío: dar por buena una licencia que nadie ha
/// leído es justo lo que el requisito quiere evitar.
fn revisar_texto_de_licencia(paquete: &Path, manifiesto: &Manifest, hallazgos: &mut Vec<Hallazgo>) {
    let textos: String = std::fs::read_dir(paquete)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.starts_with("license") || n.starts_with("licence") || n.starts_with("copying")
        })
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    if textos.is_empty() {
        return; // Lo dice `revisar_licencia`.
    }
    for id in manifiesto
        .license
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|p| !p.is_empty() && !matches!(*p, "OR" | "AND" | "WITH"))
    {
        let frase = FRASES_DE_LICENCIA
            .iter()
            .find(|(prefijo, _)| id == *prefijo || id.starts_with(&format!("{prefijo}-")))
            .map(|(_, frase)| *frase);
        let que_pasa = match frase {
            None => format!("la revisión no reconoce la licencia {id}"),
            Some(frase) if !textos.contains(&frase.to_lowercase()) => {
                format!(
                    "se declara {id} y el texto que viaja en el paquete no es el de esa licencia"
                )
            }
            Some(_) => continue,
        };
        hallazgos.push(Hallazgo {
            requisito: "R6",
            gravedad: Gravedad::Detiene,
            donde: format!("{}:license", manifest::ARCHIVO),
            que_pasa,
        });
    }
}

/// Una frase que solo aparece en el texto de cada licencia admitida.
const FRASES_DE_LICENCIA: &[(&str, &str)] = &[
    ("MIT", "Permission is hereby granted, free of charge"),
    ("Apache-2.0", "Apache License"),
    (
        "BSD-2-Clause",
        "Redistribution and use in source and binary forms",
    ),
    (
        "BSD-3-Clause",
        "Redistribution and use in source and binary forms",
    ),
    ("ISC", "Permission to use, copy, modify, and/or distribute"),
    ("MPL-2.0", "Mozilla Public License"),
    ("LGPL", "GNU LESSER GENERAL PUBLIC LICENSE"),
    ("GPL", "GNU GENERAL PUBLIC LICENSE"),
    ("AGPL", "GNU AFFERO GENERAL PUBLIC LICENSE"),
    ("CC0-1.0", "CC0"),
    ("Unlicense", "This is free and unencumbered software"),
    ("Zlib", "This software is provided 'as-is'"),
];

/// R8: el código fuente viaja con el paquete.
///
/// La forma comprobable del requisito: una carpeta `src` o `source` con algún
/// archivo dentro y la receta con que se construye (`Cargo.toml`,
/// `package.json`, `go.mod`, `build.zig`, `Makefile` o `CMakeLists.txt`). Que esa
/// fuente produzca exactamente este binario exigiría compilación reproducible,
/// que el registro no promete; lo que sí garantiza es que quien instala puede
/// leer qué hace lo que instala, con el paquete en la mano y sin depender de que
/// un repositorio externo siga existiendo.
fn revisar_fuente(paquete: &Path, hallazgos: &mut Vec<Hallazgo>) {
    let con_archivos = |dir: &Path| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .any(|e| e.path().is_file() || e.path().is_dir())
    };
    let hay_fuente = ["src", "source"]
        .iter()
        .any(|d| con_archivos(&paquete.join(d)));
    let hay_receta = RECETAS.iter().any(|r| paquete.join(r).is_file());
    if !(hay_fuente && hay_receta) {
        hallazgos.push(Hallazgo {
            requisito: "R8",
            gravedad: Gravedad::Detiene,
            donde: paquete.display().to_string(),
            que_pasa: format!(
                "no trae su código fuente: hace falta una carpeta src o source y una de {}",
                RECETAS.join(", ")
            ),
        });
    }
}

/// Las recetas de construcción que la revisión reconoce.
const RECETAS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "go.mod",
    "build.zig",
    "Makefile",
    "CMakeLists.txt",
];

/// R9: una extensión con red no sale a servicios de medición ni de publicidad.
///
/// Sin la capacidad de red una extensión no puede medir nada: la única salida es
/// `net.rs`. Con ella, los destinos están en su manifiesto y son los únicos a los
/// que el anfitrión le deja salir; se rechazan los de servicios conocidos de
/// analítica, rastreo de fallos y publicidad.
fn revisar_destinos_de_red(manifiesto: &Manifest, hallazgos: &mut Vec<Hallazgo>) {
    let Some(red) = &manifiesto.permissions.network_fetch else {
        return;
    };
    for (indice, url) in red.urls.iter().enumerate() {
        let anfitrion = url
            .trim_start_matches("https://")
            .split(['/', ':'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if DESTINOS_DE_MEDICION
            .iter()
            .any(|d| anfitrion == *d || anfitrion.ends_with(&format!(".{d}")))
        {
            hallazgos.push(Hallazgo {
                requisito: "R9",
                gravedad: Gravedad::Detiene,
                donde: format!(
                    "{}:permissions.network:fetch.urls[{indice}]",
                    manifest::ARCHIVO
                ),
                que_pasa: format!("{anfitrion} es un servicio de medición o de publicidad"),
            });
        }
    }
}

/// Servicios de analítica, rastreo de fallos y publicidad.
const DESTINOS_DE_MEDICION: &[&str] = &[
    "google-analytics.com",
    "googletagmanager.com",
    "analytics.google.com",
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "app-measurement.com",
    "firebaseio.com",
    "segment.io",
    "segment.com",
    "mixpanel.com",
    "amplitude.com",
    "heap.io",
    "hotjar.com",
    "fullstory.com",
    "sentry.io",
    "bugsnag.com",
    "datadoghq.com",
    "newrelic.com",
    "posthog.com",
    "plausible.io",
    "matomo.cloud",
    "facebook.net",
    "graph.facebook.com",
    "ads-twitter.com",
    "adnxs.com",
    "criteo.com",
    "taboola.com",
    "outbrain.com",
    "appsflyer.com",
    "branch.io",
    "adjust.com",
];

/// R10: el paquete no trae herramientas conocidas de eliminación de DRM.
///
/// Se buscan en los módulos y en los nombres de los archivos las marcas de las
/// herramientas publicadas para quitar la protección de libros electrónicos.
fn revisar_drm(paquete: &Path, artefactos: &[Artefacto], hallazgos: &mut Vec<Hallazgo>) {
    let mut nombres = Vec::new();
    let mut pila = vec![paquete.to_path_buf()];
    while let Some(dir) = pila.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                pila.push(e.path());
            }
            nombres.push(e.file_name().to_string_lossy().to_lowercase());
        }
    }
    for marca in MARCAS_DE_DRM {
        let en_nombre = nombres.iter().find(|n| n.contains(marca));
        let en_modulo = artefactos
            .iter()
            .find(|a| contiene_sin_mayusculas(&a.bytes, marca.as_bytes()));
        let donde = match (en_nombre, en_modulo) {
            (Some(n), _) => n.clone(),
            (None, Some(a)) => a.nombre.clone(),
            (None, None) => continue,
        };
        hallazgos.push(Hallazgo {
            requisito: "R10",
            gravedad: Gravedad::Detiene,
            donde,
            que_pasa: format!("contiene la marca de una herramienta para quitar DRM: {marca}"),
        });
    }
}

/// Marcas, en minúsculas, de las herramientas conocidas para quitar DRM.
const MARCAS_DE_DRM: &[&str] = &[
    "dedrm",
    "nodrm",
    "ineptepub",
    "ineptpdf",
    "ignobleepub",
    "adobekey",
    "kindlekey",
    "k4mobidedrm",
    "kfxdedrm",
    "obok",
    "lcpdedrm",
    "adeptkey",
];

fn contiene_sin_mayusculas(bytes: &[u8], aguja: &[u8]) -> bool {
    bytes
        .windows(aguja.len())
        .any(|v| v.eq_ignore_ascii_case(aguja))
}

/// R13: el paquete va firmado, y la firma corresponde a lo que trae.
///
/// La firma no dice quién es el autor: dice que quien firmó tenía esa clave. Lo
/// que ata la clave a un autor es que sea **la misma de siempre**, y eso lo
/// comprueba el almacén al instalar, no esta puerta. Aquí solo se comprueba que
/// la firma exista y corresponda, que es lo que se puede saber mirando un
/// paquete suelto.
fn revisar_firma(
    paquete: &Path,
    manifiesto: &Manifest,
    opciones: Opciones,
    hallazgos: &mut Vec<Hallazgo>,
) {
    let estado = match signature::verificar(paquete, &manifiesto.id, &manifiesto.version) {
        Ok(estado) => estado,
        Err(_) => {
            hallazgos.push(Hallazgo {
                requisito: "R13",
                gravedad: Gravedad::Detiene,
                donde: signature::ARCHIVO.to_owned(),
                que_pasa: "no se pudo comprobar la firma: el paquete no se deja leer entero"
                    .to_owned(),
            });
            return;
        }
    };
    match estado {
        signature::SignatureState::Valida { .. } => {}
        signature::SignatureState::SinFirma if opciones.exige_firma => hallazgos.push(Hallazgo {
            requisito: "R13",
            gravedad: Gravedad::Detiene,
            donde: signature::ARCHIVO.to_owned(),
            que_pasa: "no va firmada: el registro no distribuye lo que nadie firma".to_owned(),
        }),
        signature::SignatureState::SinFirma => {}
        signature::SignatureState::Invalida { motivo } => hallazgos.push(Hallazgo {
            requisito: "R13",
            gravedad: Gravedad::Detiene,
            donde: signature::ARCHIVO.to_owned(),
            que_pasa: format!("trae firma y no vale: {}", frase_de_firma(motivo)),
        }),
    }
}

fn frase_de_firma(motivo: signature::SignatureFault) -> &'static str {
    match motivo {
        signature::SignatureFault::ArchivoIlegible => "el archivo no es el JSON que se espera",
        signature::SignatureFault::AlgoritmoDesconocido => "declara un algoritmo que no se admite",
        signature::SignatureFault::FormatoIlegible => {
            "la clave o la firma no tienen el largo que toca"
        }
        signature::SignatureFault::NoCorresponde => {
            "la firma no corresponde al contenido del paquete"
        }
    }
}

/// R6: la licencia viaja dentro del paquete, no solo declarada.
fn revisar_licencia(paquete: &Path, hallazgos: &mut Vec<Hallazgo>) {
    let hay = std::fs::read_dir(paquete).is_ok_and(|entradas| {
        entradas.flatten().any(|entrada| {
            NOMBRES_DE_LICENCIA.contains(
                &entrada
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .as_str(),
            )
        })
    });
    if !hay {
        hallazgos.push(Hallazgo {
            requisito: "R6",
            gravedad: Gravedad::Detiene,
            donde: paquete.display().to_string(),
            que_pasa: format!(
                "no trae el texto de la licencia. Se admite: {}",
                NOMBRES_DE_LICENCIA.join(", ")
            ),
        });
    }
}

/// R5, en la dirección de declarar de más.
///
/// Una capacidad que ningún artefacto del paquete puede ejercer sobra, y no es
/// inocuo que sobre: cada permiso se le enseña a quien instala, y una lista
/// inflada enseña a aceptar sin leer.
fn revisar_capacidades_declaradas(
    manifiesto: &Manifest,
    artefactos: &[Artefacto],
    hallazgos: &mut Vec<Hallazgo>,
) {
    if manifiesto.capabilities.is_empty() {
        return;
    }
    if artefactos.iter().any(Artefacto::alcanza_al_anfitrion) {
        return;
    }
    for capacidad in &manifiesto.capabilities {
        let nombre = match capacidad {
            Capability::NetworkFetch => "network:fetch",
        };
        hallazgos.push(Hallazgo {
            requisito: "R5",
            gravedad: Gravedad::Detiene,
            donde: format!("{}:capabilities", manifest::ARCHIVO),
            que_pasa: format!(
                "declara {nombre} y ningún artefacto del paquete puede ejercerla: \
                 solo un componente que importe la interfaz del anfitrión llega a usarla"
            ),
        });
    }
}

/// R3, R4, R5 y R11 sobre un artefacto concreto.
fn revisar_artefacto(
    artefacto: &Artefacto,
    manifiesto: Option<&Manifest>,
    hallazgos: &mut Vec<Hallazgo>,
) {
    let donde = artefacto.nombre.clone();

    // R12: antes que nada, que esto sea lo que su nombre dice. Un archivo que
    // termina en .wasm y no lo es no se puede revisar, y revisarlo igual sería
    // dar por limpio lo que nadie ha mirado.
    if !wasm::es_modulo(&artefacto.bytes) && !artefacto.es_componente() {
        hallazgos.push(Hallazgo {
            requisito: "R12",
            gravedad: Gravedad::Detiene,
            donde,
            que_pasa: "termina en .wasm y no es ni un módulo ni un componente de \
                       WebAssembly"
                .to_owned(),
        });
        return;
    }

    for url in urls_de_codigo(&artefacto.bytes) {
        hallazgos.push(Hallazgo {
            requisito: "R3",
            gravedad: Gravedad::Detiene,
            donde: donde.clone(),
            que_pasa: format!("lleva dentro una dirección de la que traer código: {url}"),
        });
    }

    // Un componente **contiene** módulos: es su forma normal, no un escondite. Así
    // que en un componente R4 no busca una cabecera en cualquier parte, que
    // encontraría las suyas, sino **dentro de cada módulo que lleva** y en sus
    // secciones personalizadas, que es donde no tiene por qué haber ninguna.
    // Antes aquí se avisaba de que la comprobación no llegaba, porque
    // faltaba recorrer el formato de componente; recorrerlo son veinte líneas.
    if artefacto.es_componente() {
        for (orden, modulo) in wasm::modulos_de_componente(&artefacto.bytes)
            .into_iter()
            .enumerate()
        {
            if let Some(desplazamiento) = modulo_empotrado(modulo) {
                hallazgos.push(Hallazgo {
                    requisito: "R4",
                    gravedad: Gravedad::Detiene,
                    donde: donde.clone(),
                    que_pasa: format!(
                        "el módulo {} del componente lleva otro dentro, en su byte {desplazamiento}",
                        orden + 1
                    ),
                });
            }
        }
        if wasm::secciones_personalizadas(&artefacto.bytes)
            .into_iter()
            .any(|seccion| modulo_empotrado_en(seccion).is_some())
        {
            hallazgos.push(Hallazgo {
                requisito: "R4",
                gravedad: Gravedad::Detiene,
                donde: donde.clone(),
                que_pasa: "una sección personalizada del componente lleva WebAssembly dentro"
                    .to_owned(),
            });
        }
    } else if let Some(desplazamiento) = modulo_empotrado(&artefacto.bytes) {
        hallazgos.push(Hallazgo {
            requisito: "R4",
            gravedad: Gravedad::Detiene,
            donde: donde.clone(),
            que_pasa: format!(
                "lleva otro módulo de WebAssembly dentro, en el byte {desplazamiento}"
            ),
        });
    }

    for funcion in wasm::importaciones(&artefacto.bytes) {
        if IMPORTACIONES_SIN_CONCESION.contains(&funcion.as_str()) {
            hallazgos.push(Hallazgo {
                requisito: "R5",
                gravedad: Gravedad::Detiene,
                donde: donde.clone(),
                que_pasa: format!(
                    "importa {funcion}, que ninguna capacidad del manifiesto puede conceder"
                ),
            });
        }
    }

    // R11 se lee de los bytes, tanto en módulos sueltos como en componentes
    //: un componente declara también su versión de API en el binario
    // (en sus secciones personalizadas o en los módulos que lleva dentro) y
    // así la revisión tiene dos declaraciones que contrastar en ambos caminos.
    if let Some(motivo) = super::version_rechazada(&artefacto.bytes) {
        hallazgos.push(Hallazgo {
            requisito: "R11",
            gravedad: Gravedad::Detiene,
            donde: donde.clone(),
            que_pasa: frase_de_version(&motivo),
        });
    }
    if let Some(manifiesto) = manifiesto {
        if let Some(defecto) = manifest::coherente_con_el_modulo(manifiesto, &artefacto.bytes) {
            let en_el_modulo = defecto.expected.unwrap_or_else(|| "otra".to_owned());
            hallazgos.push(Hallazgo {
                requisito: "R1",
                gravedad: Gravedad::Detiene,
                donde,
                que_pasa: format!(
                    "el manifiesto declara la API {} y el módulo trae la {en_el_modulo}",
                    manifiesto.api_version
                ),
            });
        }
    }
}

/// El desenlace de la versión de API, dicho para quien escribió la extensión.
///
/// No se usa el formato de depuración del desenlace: eso enseña el nombre de
/// una variante de Rust a alguien que no tiene por qué saber que existe, y le
/// esconde justo el número que necesita para arreglarlo.
fn frase_de_version(motivo: &Desenlace) -> String {
    match motivo {
        Desenlace::SinVersionDeApi => format!(
            "no declara contra qué versión de la API se compiló: falta la sección {}",
            super::SECCION_DE_VERSION
        ),
        Desenlace::VersionDeApiIlegible => {
            "declara una versión de API que no se puede leer".to_owned()
        }
        Desenlace::VersionDeApiNoSoportada {
            declarada,
            admitida_minima,
            admitida_maxima,
        } => format!(
            "declara la API {declarada} y este anfitrión admite de la \
             {admitida_minima} a la {admitida_maxima}"
        ),
        otro => format!("{otro:?}"),
    }
}

/// Las direcciones del artefacto que apuntan a algo que se carga y se ejecuta.
///
/// Se busca sobre **todos** los bytes y no solo sobre los segmentos de datos:
/// una dirección escondida en una sección personalizada viaja igual dentro del
/// paquete, y quien la puso ahí lo hizo por algo.
///
/// El texto se recoge en crudo, sin intentar delimitar dónde acaba la cadena:
/// en WebAssembly las cadenas van concatenadas y sin terminador, así que a una
/// dirección le sigue casi siempre otro texto. Por eso la comprobación es *que
/// el tramo contenga* una extensión de código, y por eso el hallazgo devuelve
/// el tramo entero: quien lo lea ve exactamente lo que hay.
fn urls_de_codigo(bytes: &[u8]) -> Vec<String> {
    const MAXIMO: usize = 200;
    let mut salida = Vec::new();
    let mut vistas = BTreeSet::new();

    for esquema in [&b"https://"[..], &b"http://"[..]] {
        let mut desde = 0;
        while let Some(posicion) = buscar(&bytes[desde..], esquema) {
            let inicio = desde + posicion;
            let mut fin = inicio;
            while fin < bytes.len() && fin - inicio < MAXIMO && es_de_direccion(bytes[fin]) {
                fin += 1;
            }
            let tramo = String::from_utf8_lossy(&bytes[inicio..fin]).into_owned();
            let minusculas = tramo.to_lowercase();
            if EXTENSIONES_DE_CODIGO
                .iter()
                .any(|extension| minusculas.contains(extension))
                && vistas.insert(tramo.clone())
            {
                salida.push(tramo);
            }
            desde = inicio + esquema.len();
        }
    }
    salida
}

/// Los caracteres que pueden formar parte de una dirección escrita en un
/// binario. Deliberadamente estrecho: fuera de aquí la dirección se acabó.
fn es_de_direccion(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b":/.-_~?=&%+#@!$'()*,;[]".contains(&byte)
}

/// Dónde empieza un módulo o un componente empotrado dentro de un módulo, si lo
/// hay. El primer byte se salta: es la cabecera del propio módulo.
fn modulo_empotrado(bytes: &[u8]) -> Option<usize> {
    modulo_empotrado_en(bytes.get(1..)?).map(|posicion| posicion + 1)
}

/// Dónde aparece la cabecera de un módulo o de un componente en unos bytes.
fn modulo_empotrado_en(bytes: &[u8]) -> Option<usize> {
    const MODULO: &[u8] = b"\0asm\x01\0\0\0";
    const COMPONENTE: &[u8] = b"\0asm\x0d\0\x01\0";
    [buscar(bytes, MODULO), buscar(bytes, COMPONENTE)]
        .into_iter()
        .flatten()
        .min()
}

fn buscar(donde: &[u8], que: &[u8]) -> Option<usize> {
    if que.is_empty() || donde.len() < que.len() {
        return None;
    }
    donde.windows(que.len()).position(|ventana| ventana == que)
}

fn contiene(donde: &[u8], que: &[u8]) -> bool {
    buscar(donde, que).is_some()
}

/// Los artefactos de WebAssembly del paquete, en orden estable.
///
/// **Todos los que trae**, y no solo los que el manifiesto nombra: lo que viaja
/// dentro del paquete se distribuye, lo cite alguien o no.
fn artefactos_de(paquete: &Path) -> NbResult<Vec<Artefacto>> {
    let mut rutas = Vec::new();
    recoger(paquete, paquete, &mut rutas)?;
    rutas.sort();

    let mut salida = Vec::new();
    for (nombre, ruta) in rutas {
        let bytes = std::fs::read(&ruta).map_err(|_| NbError::FileNotFound)?;
        salida.push(Artefacto { nombre, bytes });
    }
    Ok(salida)
}

fn recoger(raiz: &Path, dentro: &Path, salida: &mut Vec<(String, PathBuf)>) -> NbResult<()> {
    let entradas = std::fs::read_dir(dentro).map_err(|_| NbError::FileNotFound)?;
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_dir() {
            recoger(raiz, &ruta, salida)?;
            continue;
        }
        if ruta.extension().is_some_and(|e| e == "wasm") {
            let relativa = ruta.strip_prefix(raiz).unwrap_or(&ruta);
            salida.push((relativa.to_string_lossy().replace('\\', "/"), ruta));
        }
    }
    Ok(())
}
