//! Guardar el audio de la voz **comprimido**, que es lo que decide si un libro
//! entero cabe.
//!
//! # La aritmética que obligó a esto
//!
//! Lo que sale del modelo son 24.000 muestras por segundo de 16 bits: **173 MB por
//! hora**. Un libro de diez horas son 1,7 GB y veinte libros se comen el
//! presupuesto entero de lo derivado sobre un corpus como el de esta máquina. En
//! Opus a 24 kbit/s la misma hora son unos **11 MB**, dieciséis veces menos, y ahí
//! caben cientos de libros.
//!
//! Se decidió el 2026-09-18 con esas cifras delante, y la dependencia se
//! justifica en `dependency-policy.md`.
//!
//! # Por qué Ogg y no los paquetes sueltos
//!
//! Porque el audio tiene que sonar **en la ventana**, con un elemento `<audio>`
//! como cualquier otro. Lo que devuelve el codificador son paquetes que no llevan
//! dónde empiezan ni cuánto duran; sin envoltorio, ningún reproductor sabe
//! abrirlos. Ogg es el envoltorio estándar de Opus y lo entienden los tres motores
//! sobre los que corre esta aplicación.
//!
//! Contrato: `docs/architecture/ai/local-models.md`, sección 7.

use std::path::Path;

use crate::error::{NbError, NbResult};

/// Bits por segundo. **24 kbit/s** para voz mono a 24 kHz.
///
/// Es el ritmo que Opus recomienda para voz de banda ancha en mono, y sobre esta
/// voz no se distingue de 32: lo que se pierde es lo que el modelo no produce.
/// Cada paso arriba multiplica lo que ocupa un libro de diez horas por lo mismo.
const RITMO: i32 = 24_000;

/// Milisegundos de audio por paquete. Veinte es lo que Opus usa por defecto.
const TRAMO_MS: usize = 20;

/// Opus cuenta el tiempo siempre a 48 kHz, venga la señal al ritmo que venga.
const RELOJ: u64 = 48_000;

/// El identificador del flujo. Fijo porque hay uno solo: un archivo, una voz.
const FLUJO: u32 = 0x4e42_0001;

/// Lo que ocupa un segundo de audio preparado, en bytes. Para el presupuesto.
#[must_use]
pub const fn bytes_por_segundo() -> u64 {
    RITMO as u64 / 8
}

fn interno<E>(_: E) -> NbError {
    tracing::warn!("el codificador de audio rechazó la señal");
    NbError::Internal
}

/// Escribe un Ogg Opus **según va llegando el audio**.
///
/// # Por qué no se codifica todo de una vez
///
/// Porque un libro de diez horas son 3,4 GB de muestras en coma flotante, y
/// tenerlas todas en memoria para comprimirlas al final sería pedirle a la
/// máquina justo lo que esta función existe para ahorrar. Cada fragmento entra, se
/// comprime y se suelta; lo único que se guarda entre llamadas es el resto que no
/// completaba un paquete, que son veinte milisegundos como mucho.
pub struct Grabadora {
    codificador: opus::Encoder,
    envoltorio: ogg::PacketWriter<'static, std::io::BufWriter<std::fs::File>>,
    muestreo: u32,
    /// Muestras que sobraron del último fragmento y aún no forman un paquete.
    resto: Vec<f32>,
    /// Posición en el reloj de Opus, que va a 48 kHz.
    reloj: u64,
    /// Muestras entregadas, para saber cuánto dura sin depender del reloj.
    entregadas: u64,
    buffer: Vec<u8>,
}

impl Grabadora {
    /// Abre el archivo y escribe las dos cabeceras que el formato exige.
    ///
    /// # Errors
    ///
    /// - [`NbError::StorageFailure`] si no se puede crear el archivo.
    /// - [`NbError::Internal`] si el codificador no se deja construir.
    pub fn nueva(destino: &Path, muestreo: u32) -> NbResult<Self> {
        let fallo = |_| {
            tracing::warn!("no se pudo abrir el archivo de audio");
            NbError::StorageFailure
        };
        if let Some(carpeta) = destino.parent() {
            std::fs::create_dir_all(carpeta).map_err(fallo)?;
        }
        let archivo = std::fs::File::create(destino).map_err(fallo)?;

        let mut codificador =
            opus::Encoder::new(muestreo, opus::Channels::Mono, opus::Application::Audio)
                .map_err(interno)?;
        codificador
            .set_bitrate(opus::Bitrate::Bits(RITMO))
            .map_err(interno)?;

        // El codificador mira un poco hacia delante, así que las primeras muestras
        // que devuelve no son las primeras que se le dieron. Cuántas descartar al
        // reproducir va en la cabecera, y **siempre contadas a 48 kHz**.
        let adelanto = u64::from(codificador.get_lookahead().map_err(interno)?.unsigned_abs());
        let preskip = adelanto * RELOJ / u64::from(muestreo.max(1));

        let mut envoltorio = ogg::PacketWriter::new(std::io::BufWriter::new(archivo));
        envoltorio
            .write_packet(
                cabecera(preskip, muestreo),
                FLUJO,
                ogg::PacketWriteEndInfo::EndPage,
                0,
            )
            .map_err(interno)?;
        envoltorio
            .write_packet(etiquetas(), FLUJO, ogg::PacketWriteEndInfo::EndPage, 0)
            .map_err(interno)?;

        Ok(Self {
            codificador,
            envoltorio,
            muestreo,
            resto: Vec::new(),
            reloj: preskip,
            entregadas: 0,
            buffer: vec![0_u8; 4_000],
        })
    }

    /// Añade el audio de un fragmento. Devuelve en qué milisegundo termina.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el codificador rechaza la señal.
    pub fn anadir(&mut self, muestras: &[f32]) -> NbResult<u64> {
        self.entregadas += muestras.len() as u64;
        self.resto.extend_from_slice(muestras);
        let por_tramo = self.muestreo as usize * TRAMO_MS / 1000;

        let mut desde = 0;
        while desde + por_tramo <= self.resto.len() {
            let escritos = self
                .codificador
                .encode_float(&self.resto[desde..desde + por_tramo], &mut self.buffer)
                .map_err(interno)?;
            self.reloj += (por_tramo as u64) * RELOJ / u64::from(self.muestreo.max(1));
            self.envoltorio
                .write_packet(
                    self.buffer[..escritos].to_vec(),
                    FLUJO,
                    ogg::PacketWriteEndInfo::NormalPacket,
                    self.reloj,
                )
                .map_err(interno)?;
            desde += por_tramo;
        }
        self.resto.drain(..desde);

        Ok(self.duracion_ms())
    }

    /// Cuánto audio lleva dentro, en milisegundos.
    #[must_use]
    pub fn duracion_ms(&self) -> u64 {
        self.entregadas * 1_000 / u64::from(self.muestreo.max(1))
    }

    /// Cierra el flujo. Sin esto el archivo queda abierto y el reproductor espera
    /// algo que no llega.
    ///
    /// # Errors
    ///
    /// [`NbError::Internal`] si el último paquete no se deja escribir.
    pub fn cerrar(mut self) -> NbResult<u64> {
        let por_tramo = self.muestreo as usize * TRAMO_MS / 1000;
        // Lo que queda se completa con silencio en lugar de tirarse: recortarlo se
        // comería hasta veinte milisegundos de la última palabra.
        let mut ultimo = std::mem::take(&mut self.resto);
        ultimo.resize(por_tramo, 0.0);
        let escritos = self
            .codificador
            .encode_float(&ultimo, &mut self.buffer)
            .map_err(interno)?;
        self.reloj += (por_tramo as u64) * RELOJ / u64::from(self.muestreo.max(1));
        self.envoltorio
            .write_packet(
                self.buffer[..escritos].to_vec(),
                FLUJO,
                ogg::PacketWriteEndInfo::EndStream,
                self.reloj,
            )
            .map_err(interno)?;
        Ok(self.duracion_ms())
    }
}

/// Escribe unas muestras como Ogg Opus mono, de una vez.
///
/// Para un párrafo suelto, que es lo que pide la narración en vivo. Un libro
/// entero va por [`Grabadora`].
///
/// # Errors
///
/// - [`NbError::StorageFailure`] si no se puede escribir.
/// - [`NbError::Internal`] si el codificador rechaza la señal.
pub fn escribir_opus(destino: &Path, muestras: &[f32], muestreo: u32) -> NbResult<()> {
    let mut grabadora = Grabadora::nueva(destino, muestreo)?;
    grabadora.anadir(muestras)?;
    grabadora.cerrar()?;
    Ok(())
}

/// `OpusHead`: los diecinueve bytes que dicen qué hay dentro.
fn cabecera(preskip: u64, muestreo: u32) -> Vec<u8> {
    let mut cabecera = Vec::with_capacity(19);
    cabecera.extend_from_slice(b"OpusHead");
    cabecera.push(1); // versión
    cabecera.push(1); // canales
    #[allow(clippy::cast_possible_truncation)]
    cabecera.extend_from_slice(&(preskip as u16).to_le_bytes());
    cabecera.extend_from_slice(&muestreo.to_le_bytes());
    cabecera.extend_from_slice(&0_i16.to_le_bytes()); // ganancia de salida
    cabecera.push(0); // familia de mapeo: mono o estéreo sin más
    cabecera
}

/// `OpusTags`: quién lo hizo. **Ni el título ni la ruta del libro**, que es
/// material con derechos y no tiene por qué viajar dentro del audio.
fn etiquetas() -> Vec<u8> {
    const QUIEN: &[u8] = b"NB Books";
    let mut etiquetas = Vec::with_capacity(8 + 4 + QUIEN.len() + 4);
    etiquetas.extend_from_slice(b"OpusTags");
    #[allow(clippy::cast_possible_truncation)]
    etiquetas.extend_from_slice(&(QUIEN.len() as u32).to_le_bytes());
    etiquetas.extend_from_slice(QUIEN);
    etiquetas.extend_from_slice(&0_u32.to_le_bytes()); // sin comentarios
    etiquetas
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una señal con forma, para que comprimirla y volver a abrirla signifique algo:
    /// un tono que sube, que es lo que más se parece a una voz de lo que se puede
    /// generar sin el modelo.
    fn señal(segundos: f32, muestreo: u32) -> Vec<f32> {
        let total = (segundos * muestreo as f32) as usize;
        (0..total)
            .map(|i| {
                let t = i as f32 / muestreo as f32;
                let frecuencia = 120.0 + 40.0 * t;
                (t * frecuencia * std::f32::consts::TAU).sin() * 0.3
            })
            .collect()
    }

    fn temporal(nombre: &str) -> std::path::PathBuf {
        let dir =
            crate::paths::temporal_de_pruebas().join(format!("nb-opus-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("carpeta");
        dir.join(nombre)
    }

    fn codificar(muestras: &[f32], muestreo: u32) -> Vec<u8> {
        let ruta = temporal(&format!("{}.opus", muestras.len()));
        escribir_opus(&ruta, muestras, muestreo).expect("codifica");
        std::fs::read(&ruta).expect("se lee")
    }

    /// **La propiedad que sostiene la elección del formato**: lo que sale es un Ogg
    /// con Opus dentro, y ocupa lo que se dijo que ocuparía.
    #[test]
    fn un_segundo_de_voz_ocupa_lo_que_dice_el_presupuesto() {
        let bytes = codificar(&señal(1.0, 24_000), 24_000);

        assert_eq!(&bytes[0..4], b"OggS", "no es un contenedor Ogg");
        assert!(
            bytes.windows(8).any(|v| v == b"OpusHead"),
            "no declara Opus dentro"
        );
        assert!(
            bytes.windows(8).any(|v| v == b"OpusTags"),
            "le falta el bloque de etiquetas, que el formato exige"
        );

        // A 24 kbit/s, un segundo son unos 3 KB. El margen es ancho a propósito: lo
        // que se comprueba es el orden de magnitud, no el codificador.
        assert!(
            (2_000..6_000).contains(&bytes.len()),
            "un segundo ocupó {} bytes",
            bytes.len()
        );
        // Y frente a lo que ocuparía en crudo: 48.000 bytes.
        assert!(bytes.len() * 8 < 48_000, "no comprime lo suficiente");
    }

    /// Y se puede volver a abrir: se lee el envoltorio, se descodifica y sale la
    /// misma duración. Sin esto, "comprime mucho" y "se perdió el audio" se
    /// parecen demasiado.
    #[test]
    fn lo_comprimido_se_vuelve_a_abrir_con_la_misma_duracion() {
        let original = señal(2.0, 24_000);
        let bytes = codificar(&original, 24_000);

        let mut lector = ogg::PacketReader::new(std::io::Cursor::new(bytes));
        let mut descodificador =
            opus::Decoder::new(24_000, opus::Channels::Mono).expect("descodificador");
        let mut salida = Vec::new();
        let mut buffer = vec![0.0_f32; 24_000];
        let mut cabeceras = 0;
        while let Some(paquete) = lector.read_packet().expect("se lee") {
            // Los dos primeros paquetes son las cabeceras, no audio.
            if cabeceras < 2 {
                cabeceras += 1;
                continue;
            }
            let muestras = descodificador
                .decode_float(&paquete.data, &mut buffer, false)
                .expect("descodifica");
            salida.extend_from_slice(&buffer[..muestras]);
        }

        assert_eq!(cabeceras, 2, "faltan las cabeceras del formato");
        // La misma duración, con el margen del adelanto del codificador y del
        // último tramo completado con silencio: medio tramo por cada lado.
        let diferencia = salida.len().abs_diff(original.len());
        assert!(
            diferencia < 24_000 / 10,
            "la duración cambió en {diferencia} muestras"
        );
        // Y no es silencio: si lo fuera, la duración coincidiría igual.
        let pico = salida.iter().fold(0.0_f32, |a, m| a.max(m.abs()));
        assert!(pico > 0.05, "lo descodificado salió mudo: pico {pico}");
    }

    /// **Lo que hace posible un libro entero**: añadir por fragmentos da lo mismo
    /// que codificarlo todo de una vez, y sin tener el libro en memoria.
    ///
    /// Se comprueba porque entre fragmento y fragmento queda un resto que no
    /// completa un paquete, y perderlo o repetirlo son las dos formas de que un
    /// audiolibro salga con un chasquido cada pocos segundos.
    #[test]
    fn anadir_por_fragmentos_suena_igual_que_de_una_vez() {
        let entera = señal(1.5, 24_000);
        let de_una_vez = codificar(&entera, 24_000);

        let ruta = temporal("por-partes.opus");
        let mut grabadora = Grabadora::nueva(&ruta, 24_000).expect("se abre");
        // Trozos que **no** son múltiplos del paquete, que es el caso real: un
        // fragmento de texto no dura veinte milisegundos exactos.
        for trozo in entera.chunks(7_333) {
            grabadora.anadir(trozo).expect("se añade");
        }
        let duracion = grabadora.cerrar().expect("se cierra");
        let por_partes = std::fs::read(&ruta).expect("se lee");

        assert_eq!(duracion, 1_500, "la duración no cuadra");
        assert_eq!(
            por_partes, de_una_vez,
            "codificar por fragmentos no dio lo mismo que de una vez"
        );
    }

    /// Y va diciendo por dónde va, que es de donde salen las marcas del texto.
    #[test]
    fn cada_fragmento_dice_en_que_milisegundo_termina() {
        let ruta = temporal("marcas.opus");
        let mut grabadora = Grabadora::nueva(&ruta, 24_000).expect("se abre");

        let primero = grabadora.anadir(&señal(0.5, 24_000)).expect("primero");
        let segundo = grabadora.anadir(&señal(1.0, 24_000)).expect("segundo");
        grabadora.cerrar().expect("se cierra");

        assert_eq!(primero, 500);
        assert_eq!(segundo, 1_500);
    }

    /// Un tramo sin nada que decir no produce un archivo roto.
    #[test]
    fn un_tramo_vacio_produce_un_flujo_cerrado_y_no_un_archivo_a_medias() {
        let bytes = codificar(&[], 24_000);
        assert_eq!(&bytes[0..4], b"OggS");
        let mut lector = ogg::PacketReader::new(std::io::Cursor::new(bytes));
        let mut paquetes = 0;
        while lector.read_packet().expect("se lee").is_some() {
            paquetes += 1;
        }
        assert!(paquetes >= 2, "faltan las cabeceras");
    }
}
