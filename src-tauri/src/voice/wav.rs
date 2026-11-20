//! Escribir muestras como WAV, **para escucharlas mientras se mide**.
//!
//! # Qué hace esto aquí si el producto guarda Opus
//!
//! Porque no todo lo que se escribe es para el producto. Lo que la aplicación
//! guarda —un párrafo en la caché, un libro preparado— va comprimido: 24 kHz de
//! 16 bits son 173 MB por hora y en Opus son 11, y esa diferencia decide si un
//! libro cabe. Ver [`super::audio`].
//!
//! Esto es la otra cosa: el archivo que se abre para **escuchar** una muestra
//! mientras se mide el motor, sin un codificador de por medio que pueda
//! enmascarar lo que se quiere oír. Lo usa `tests/voz_neuronal.rs` con
//! `NB_VOZ_WAV`, que es el camino por el que se juzgó la calidad de la voz.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 7.

use std::io::Write;
use std::path::Path;

use crate::error::{NbError, NbResult};

/// Bytes por muestra: 16 bits con signo.
const ANCHO: u16 = 2;

/// Cuánto ocupa un segundo de audio a este muestreo, en bytes.
#[must_use]
pub const fn bytes_por_segundo(muestreo: u32) -> u64 {
    muestreo as u64 * ANCHO as u64
}

/// Escribe las muestras en un archivo WAV mono.
///
/// Las muestras vienen en coma flotante entre -1 y 1; lo que se salga se acota en
/// lugar de dar la vuelta, que es la diferencia entre un audio fuerte y un
/// chasquido.
///
/// # Errors
///
/// [`NbError::StorageFailure`] si no se puede escribir.
pub fn escribir(destino: &Path, muestras: &[f32], muestreo: u32) -> NbResult<()> {
    let fallo = |_| {
        tracing::warn!("no se pudo escribir el audio de la voz");
        NbError::StorageFailure
    };
    if let Some(carpeta) = destino.parent() {
        std::fs::create_dir_all(carpeta).map_err(fallo)?;
    }
    let mut archivo = std::io::BufWriter::new(std::fs::File::create(destino).map_err(fallo)?);
    archivo
        .write_all(&cabecera(muestras.len(), muestreo))
        .map_err(fallo)?;
    for muestra in muestras {
        #[allow(clippy::cast_possible_truncation)]
        let entero = (muestra.clamp(-1.0, 1.0) * 32_767.0) as i16;
        archivo.write_all(&entero.to_le_bytes()).map_err(fallo)?;
    }
    archivo.flush().map_err(fallo)
}

/// La cabecera de 44 bytes que espera cualquier reproductor.
fn cabecera(muestras: usize, muestreo: u32) -> Vec<u8> {
    let datos = u32::try_from(muestras * ANCHO as usize).unwrap_or(u32::MAX);
    let mut cabecera = Vec::with_capacity(44);
    cabecera.extend_from_slice(b"RIFF");
    cabecera.extend_from_slice(&(36 + datos).to_le_bytes());
    cabecera.extend_from_slice(b"WAVEfmt ");
    cabecera.extend_from_slice(&16_u32.to_le_bytes());
    // PCM, un canal.
    cabecera.extend_from_slice(&1_u16.to_le_bytes());
    cabecera.extend_from_slice(&1_u16.to_le_bytes());
    cabecera.extend_from_slice(&muestreo.to_le_bytes());
    cabecera.extend_from_slice(&(muestreo * u32::from(ANCHO)).to_le_bytes());
    cabecera.extend_from_slice(&ANCHO.to_le_bytes());
    cabecera.extend_from_slice(&16_u16.to_le_bytes());
    cabecera.extend_from_slice(b"data");
    cabecera.extend_from_slice(&datos.to_le_bytes());
    cabecera
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(nombre: &str) -> std::path::PathBuf {
        let dir =
            crate::paths::temporal_de_pruebas().join(format!("nb-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("carpeta");
        dir.join(nombre)
    }

    /// El archivo dice de sí mismo lo que es: un reproductor que lea mal la
    /// cabecera no da error, da ruido.
    #[test]
    fn la_cabecera_declara_mono_a_su_muestreo_y_el_tamano_real() {
        let ruta = temporal("cabecera.wav");
        escribir(&ruta, &[0.0, 0.5, -0.5, 1.0], 24_000).expect("se escribe");
        let bytes = std::fs::read(&ruta).expect("se lee");

        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "canales");
        assert_eq!(
            u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            24_000
        );
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16, "bits");
        assert_eq!(&bytes[36..40], b"data");
        let datos = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        assert_eq!(datos, 8, "cuatro muestras de dos bytes");
        assert_eq!(bytes.len(), 44 + 8);
    }

    /// Lo que se sale del rango se acota y **no da la vuelta**: un desbordamiento
    /// convierte un pico fuerte en un chasquido al otro lado.
    #[test]
    fn una_muestra_fuera_de_rango_se_acota_en_lugar_de_dar_la_vuelta() {
        let ruta = temporal("acotada.wav");
        escribir(&ruta, &[2.0, -2.0], 24_000).expect("se escribe");
        let bytes = std::fs::read(&ruta).expect("se lee");
        assert_eq!(i16::from_le_bytes([bytes[44], bytes[45]]), 32_767);
        assert_eq!(i16::from_le_bytes([bytes[46], bytes[47]]), -32_767);
    }

    /// Lo que ocupa un segundo, que es lo que el presupuesto necesita saber.
    #[test]
    fn un_segundo_a_24_khz_ocupa_lo_que_dice_el_presupuesto() {
        assert_eq!(bytes_por_segundo(24_000), 48_000);
    }
}
