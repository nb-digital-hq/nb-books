//! Lo que comparten las pruebas del banco de extensiones.
//!
//! Cada archivo de `tests/` es un binario aparte, así que sin esto los dos que
//! usan el banco tendrían que repetir cómo se construye y cómo se lee. Una
//! copia que se queda vieja es peor que no tenerla.

// Cada binario de prueba usa solo la parte que le toca, y el compilador no
// puede saber que la otra mitad la usa el de al lado.
#![allow(dead_code)]

pub mod estado;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// El objetivo para el que se compila el banco. Es el mismo que declara
/// `plugin-system.md` sección 2.1.
pub const OBJETIVO: &str = "wasm32-wasip1";

/// Raíz del repositorio: `src-tauri` es hija suya.
pub fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri tiene padre")
        .to_path_buf()
}

/// Dónde vive el banco.
pub fn raiz_banco() -> PathBuf {
    raiz_repo().join("extensiones").join("banco")
}

/// Si el objetivo no está instalado no hay nada que comprobar, y decirlo es
/// mejor que fallar: solo está en las máquinas donde alguien lo instaló.
pub fn objetivo_instalado() -> bool {
    objetivo_disponible(OBJETIVO)
}

/// Si un objetivo de compilación está instalado en esta máquina.
pub fn objetivo_disponible(objetivo: &str) -> bool {
    Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--print", "target-libdir", "--target", objetivo])
        .output()
        .map(|s| s.status.success())
        .unwrap_or(false)
}

/// Construye el banco una sola vez y devuelve el directorio con los módulos.
///
/// El espacio de trabajo del banco es propio, así que su `target` también lo
/// es: compartirlo con el del runtime mezclaría dos objetivos distintos en el
/// mismo árbol. Se limpian las banderas heredadas porque esta prueba corre
/// dentro de otro `cargo`, y las suyas están pensadas para el anfitrión.
pub fn banco_construido() -> Option<&'static PathBuf> {
    static UNA_VEZ: OnceLock<Option<PathBuf>> = OnceLock::new();
    UNA_VEZ
        .get_or_init(|| {
            if !objetivo_instalado() {
                return None;
            }
            let raiz = raiz_banco();
            let destino = raiz.join("target");
            let salida = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .arg("build")
                .arg("--manifest-path")
                .arg(raiz.join("Cargo.toml"))
                .arg("--target")
                .arg(OBJETIVO)
                .arg("--target-dir")
                .arg(&destino)
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("CARGO_TARGET_DIR")
                .output()
                .expect("cargo del banco");
            assert!(
                salida.status.success(),
                "el banco no compila:\n{}",
                String::from_utf8_lossy(&salida.stderr)
            );
            Some(destino.join(OBJETIVO).join("debug"))
        })
        .as_ref()
}

/// El objetivo de los componentes. Un componente es lo que produce `wasip2`.
pub const OBJETIVO_COMPONENTE: &str = "wasm32-wasip2";

/// Construye el componente del banco una sola vez.
///
/// Va aparte de [`banco_construido`] porque se compila para otro objetivo: los
/// módulos sueltos a `wasip1`, que es el camino que usaría un atacante, y el
/// componente a `wasip2`, que es el camino por el que entran las extensiones de
/// verdad. El anfitrión soporta los dos y conviene que los dos se prueben.
pub fn componente_construido() -> Option<&'static PathBuf> {
    static UNA_VEZ: OnceLock<Option<PathBuf>> = OnceLock::new();
    UNA_VEZ
        .get_or_init(|| {
            if !objetivo_disponible(OBJETIVO_COMPONENTE) {
                return None;
            }
            let raiz = raiz_banco();
            let destino = raiz.join("target");
            let salida = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .arg("build")
                .arg("--manifest-path")
                .arg(raiz.join("Cargo.toml"))
                .arg("--target")
                .arg(OBJETIVO_COMPONENTE)
                .arg("--package")
                .arg("usa-el-mundo")
                .arg("--package")
                .arg("formato-de-texto")
                .arg("--package")
                .arg("fuente-de-metadatos")
                .arg("--package")
                .arg("vista-de-estudio")
                .arg("--target-dir")
                .arg(&destino)
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("CARGO_TARGET_DIR")
                .output()
                .expect("cargo del componente");
            assert!(
                salida.status.success(),
                "el componente no compila:\n{}",
                String::from_utf8_lossy(&salida.stderr)
            );
            Some(destino.join(OBJETIVO_COMPONENTE).join("debug"))
        })
        .as_ref()
}

/// Construye las extensiones de primera parte y devuelve su directorio.
///
/// Van en su propio espacio de trabajo, aparte del banco: el banco son módulos
/// adversos y esto son extensiones que se publican. Se compilan igual que un
/// componente, para `wasm32-wasip2`.
pub fn propias_construidas() -> Option<&'static PathBuf> {
    static UNA_VEZ: OnceLock<Option<PathBuf>> = OnceLock::new();
    UNA_VEZ
        .get_or_init(|| {
            if !objetivo_disponible(OBJETIVO_COMPONENTE) {
                return None;
            }
            let raiz = raiz_repo().join("extensiones").join("propias");
            let destino = raiz.join("target");
            let salida = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
                .arg("build")
                .arg("--manifest-path")
                .arg(raiz.join("Cargo.toml"))
                .arg("--target")
                .arg(OBJETIVO_COMPONENTE)
                .arg("--target-dir")
                .arg(&destino)
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env_remove("CARGO_TARGET_DIR")
                .output()
                .expect("cargo de las extensiones propias");
            assert!(
                salida.status.success(),
                "las extensiones propias no compilan:\n{}",
                String::from_utf8_lossy(&salida.stderr)
            );
            Some(destino.join(OBJETIVO_COMPONENTE).join("debug"))
        })
        .as_ref()
}

/// Monta el paquete de una extensión propia en un directorio: su manifiesto,
/// su licencia y su módulo, que es lo que se distribuye.
pub fn paquete_propio(nombre: &str, modulo: &str, en: &Path) -> Option<PathBuf> {
    let construidas = propias_construidas()?;
    let origen = raiz_repo().join("extensiones").join("propias").join(nombre);

    let dir = en.join(format!("paquete-{nombre}"));
    std::fs::create_dir_all(&dir).expect("paquete");
    std::fs::copy(
        origen.join("nb-extension.json"),
        dir.join("nb-extension.json"),
    )
    .expect("manifiesto");
    std::fs::copy(origen.join("LICENSE"), dir.join("LICENSE")).expect("licencia");
    std::fs::copy(construidas.join(modulo), dir.join(modulo)).expect("módulo");
    // Como el empaquetador: fuente y receta con el binario, o R8 lo detiene.
    std::fs::copy(origen.join("Cargo.toml"), dir.join("Cargo.toml")).expect("receta");
    copiar_arbol(&origen.join("src"), &dir.join("src"));
    Some(dir)
}

/// Los bytes del artefacto de una ficha, sea del objetivo que sea.
///
/// Una ficha puede declarar `objetivo` y `archivo`; sin ellos, `wasip1` y
/// `<directorio>.wasm`. Existe porque el banco tiene ya dos objetivos: los
/// módulos sueltos, que son el camino que usaría un atacante, y el componente,
/// que es el camino por el que entran las extensiones de verdad.
pub fn bytes_de_ficha(ficha: &serde_json::Value) -> Option<Vec<u8>> {
    let nombre = directorio(ficha);
    let objetivo = ficha["objetivo"].as_str().unwrap_or(OBJETIVO);
    let archivo = ficha["archivo"]
        .as_str()
        .map_or_else(|| format!("{nombre}.wasm"), ToOwned::to_owned);

    let directorio = if objetivo == OBJETIVO_COMPONENTE {
        componente_construido()?
    } else {
        banco_construido()?
    };
    Some(
        std::fs::read(directorio.join(archivo)).unwrap_or_else(|_| panic!("{nombre} se construyó")),
    )
}

/// Los bytes del componente del banco.
pub fn componente(nombre: &str) -> Option<Vec<u8>> {
    let directorio = componente_construido()?;
    Some(
        std::fs::read(directorio.join(format!("{nombre}.wasm")))
            .unwrap_or_else(|_| panic!("{nombre} se construyó")),
    )
}

/// Los bytes de un módulo del banco.
pub fn modulo(nombre: &str) -> Option<Vec<u8>> {
    let directorio = banco_construido()?;
    Some(
        std::fs::read(directorio.join(format!("{nombre}.wasm")))
            .unwrap_or_else(|_| panic!("{nombre} se construyó")),
    )
}

/// Las fichas del banco, tal y como las declara `banco.json`.
pub fn fichas() -> Vec<serde_json::Value> {
    let texto =
        std::fs::read_to_string(raiz_banco().join("banco.json")).expect("banco.json existe");
    let documento: serde_json::Value = serde_json::from_str(&texto).expect("banco.json es JSON");
    documento["modulos"]
        .as_array()
        .expect("banco.json tiene modulos")
        .clone()
}

pub fn directorio(ficha: &serde_json::Value) -> String {
    ficha["directorio"]
        .as_str()
        .expect("directorio")
        .to_string()
}

/// Un ZIP con sus entradas sin comprimir y en el orden dado.
///
/// Para las pruebas que necesitan un EPUB con una forma concreta -una
/// superposición, un audio- y no la de `epub_lector.rs`. El lector de contenedores
/// no comprueba el CRC, y sin comprimir no hace falta nada más que la estructura.
pub fn zip_sin_comprimir(entradas: &[(&str, &[u8])]) -> Vec<u8> {
    let mut salida = Vec::new();
    let mut central = Vec::new();
    for (nombre, datos) in entradas {
        let offset = u32::try_from(salida.len()).expect("cabe");
        let largo = u32::try_from(datos.len()).expect("cabe");
        let nombre = nombre.as_bytes();
        let largo_nombre = u16::try_from(nombre.len()).expect("cabe").to_le_bytes();
        salida.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0]);
        salida.extend_from_slice(&[0; 8]);
        salida.extend_from_slice(&largo.to_le_bytes());
        salida.extend_from_slice(&largo.to_le_bytes());
        salida.extend_from_slice(&largo_nombre);
        salida.extend_from_slice(&[0, 0]);
        salida.extend_from_slice(nombre);
        salida.extend_from_slice(datos);

        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&largo.to_le_bytes());
        central.extend_from_slice(&largo.to_le_bytes());
        central.extend_from_slice(&largo_nombre);
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(nombre);
    }
    let cuantas = u16::try_from(entradas.len()).expect("cabe").to_le_bytes();
    let offset_central = u32::try_from(salida.len()).expect("cabe");
    let tamano_central = u32::try_from(central.len()).expect("cabe");
    salida.extend_from_slice(&central);
    salida.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    salida.extend_from_slice(&cuantas);
    salida.extend_from_slice(&cuantas);
    salida.extend_from_slice(&tamano_central.to_le_bytes());
    salida.extend_from_slice(&offset_central.to_le_bytes());
    salida.extend_from_slice(&[0, 0]);
    salida
}

/// Silencio en WAV, mono a 8 kHz y 16 bits: lo justo para que haya un audio que
/// extraer y que un reproductor acepte.
pub fn silencio_wav(segundos: u32) -> Vec<u8> {
    let muestras = 8000 * segundos;
    let datos = muestras * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + datos).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&8000_u32.to_le_bytes());
    wav.extend_from_slice(&16000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&datos.to_le_bytes());
    wav.resize(wav.len() + datos as usize, 0);
    wav
}

fn copiar_arbol(de: &Path, a: &Path) {
    std::fs::create_dir_all(a).expect("carpeta");
    for entrada in std::fs::read_dir(de).expect("fuente").flatten() {
        let destino = a.join(entrada.file_name());
        if entrada.path().is_dir() {
            copiar_arbol(&entrada.path(), &destino);
        } else {
            std::fs::copy(entrada.path(), destino).expect("archivo");
        }
    }
}
