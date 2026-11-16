//! Las convoluciones del decodificador y del codificador de texto de Kokoro.
//!
//! # Por qué hay una convolución propia y no se llama a la de Candle
//!
//! Porque **ahí se iba el 83 % del tiempo**. Medido el 2026-09-18: de los 11,3 s que
//! costaba decir 6,25 s de audio, 9,4 se iban en el bucle de subida del generador,
//! que es todo convoluciones.
//!
//! La de Candle en procesador recorre cada posición de salida haciendo un producto
//! escalar de longitud `c_in`. Los números son los correctos y el trabajo también,
//! pero se hace sin bloqueo: cada dato de entrada se vuelve a leer `k` veces, y la
//! matriz de pesos entera se recorre por cada posición. Una multiplicación de
//! matrices hace exactamente las mismas operaciones **reutilizando lo que ya está en
//! la caché**, y la de Candle sí está optimizada.
//!
//! Así que aquí la convolución se convierte en una multiplicación: se colocan las
//! ventanas de entrada en una matriz —lo que en la literatura se llama *im2col*— y
//! se multiplica por los pesos. Lo que se gana es la diferencia entre un bucle y una
//! multiplicación de matrices bien hecha; lo que cuesta es la memoria de esa matriz,
//! y por eso se hace **por bloques de tiempo** en lugar de de una vez: sin bloques,
//! una capa de 512 canales por 150.000 posiciones pediría tres gigabytes y medio.
//!
//! El camino rápido solo cubre lo que este modelo usa —paso uno— y lo demás sigue
//! yendo a Candle. Una prueba comprueba que los dos caminos dan el mismo número.

use candle_core::{Module, Result, Tensor};
use candle_nn::VarBuilder;

fn scalar_like(tensor: &Tensor, value: f32) -> Result<Tensor> {
    Tensor::new(value, tensor.device())?.to_dtype(tensor.dtype())
}

/// 1D convolution layer.
pub struct Conv1d {
    weight: Tensor,
    bias: Option<Tensor>,
    stride: usize,
    padding: usize,
    dilation: usize,
    groups: usize,
}

impl Conv1d {
    /// Load a Conv1d layer from a VarBuilder.
    ///
    /// Weight shape: [out_channels, in_channels/groups, kernel_size]
    ///
    /// Supports both plain `weight` tensors and weight-normalized
    /// `weight_g` + `weight_v` pairs (from PyTorch `weight_norm`).
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        in_channels: usize,
        out_channels: usize,
        kernel_size: usize,
        stride: usize,
        padding: usize,
        dilation: usize,
        groups: usize,
        use_bias: bool,
        vb: VarBuilder,
    ) -> Result<Self> {
        let weight_shape = (out_channels, in_channels / groups, kernel_size);
        let weight = match vb.get(weight_shape, "weight") {
            Ok(w) => w,
            Err(_) => {
                // Fall back to weight normalization: weight = g * (v / ||v||)
                let weight_v = vb.get(weight_shape, "weight_v")?;
                // PyTorch stores weight_g as [out, 1, 1]; try 3D first, then 1D
                let weight_g = vb
                    .get((out_channels, 1, 1), "weight_g")
                    .or_else(|_| vb.get(out_channels, "weight_g"))?;
                apply_weight_norm(&weight_v, &weight_g)?
            }
        };
        let bias = if use_bias {
            Some(vb.get(out_channels, "bias")?)
        } else {
            None
        };
        Ok(Self {
            weight,
            bias,
            stride,
            padding,
            dilation,
            groups,
        })
    }

    /// Forward pass: input shape [batch, in_channels, length]
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let out = convolucion(
            x,
            &self.weight,
            self.padding,
            self.stride,
            self.dilation,
            self.groups,
        )?;
        match &self.bias {
            Some(bias) => {
                let bias = bias.unsqueeze(0)?.unsqueeze(2)?;
                out.broadcast_add(&bias)
            }
            None => Ok(out),
        }
    }
}

/// Cuántas posiciones de salida se calculan de una vez.
///
/// La matriz de ventanas ocupa `c_in * k * BLOQUE` números, así que el bloque es lo
/// que decide la memoria de trabajo: con 512 canales y núcleo de 11, 2048 posiciones
/// son 46 MB, que caben en cualquier sitio y ya son bastante grandes para que la
/// multiplicación amortice el coste de colocarlas.
const BLOQUE: usize = 2048;

/// Por debajo de esta longitud de salida no compensa colocar las ventanas.
///
/// **Sale de medir los dos caminos en el mismo binario y en el mismo estado térmico,
/// el 2026-09-18.** Colocar las ventanas cuesta copiar la entrada `k` veces, y ese
/// coste solo se amortiza si la multiplicación que viene después es lo bastante
/// larga. Sobre las convoluciones largas del generador el camino rápido ahorraba
/// novecientos milisegundos; sobre las cortas de la prosodia —250 posiciones— perdía
/// otros novecientos, y el total quedaba igual. Con el listón en la longitud, cada
/// convolución va por donde le conviene.
const LARGO_MINIMO: usize = 4096;

/// Una convolución de una dimensión, por el camino que convenga.
///
/// # Errors
///
/// Los de las operaciones de tensores que la componen.
pub fn convolucion(
    x: &Tensor,
    peso: &Tensor,
    padding: usize,
    stride: usize,
    dilation: usize,
    groups: usize,
) -> Result<Tensor> {
    let (_, _, l_in) = x.dims3()?;
    let (c_out, c_in_g, k) = peso.dims3()?;
    let vano = (k - 1) * dilation + 1;
    let l_out = (l_in + 2 * padding).saturating_sub(vano) / stride + 1;

    // El paso distinto de uno pediría elegir posiciones salteadas, que este modelo no
    // usa: lo que no se puede probar con el modelo delante no se escribe.
    if stride != 1 || l_out < LARGO_MINIMO {
        return x.conv1d(peso, padding, stride, dilation, groups);
    }

    if groups > 1 {
        let entradas = x.chunk(groups, 1)?;
        let pesos = peso.chunk(groups, 0)?;
        let partes = entradas
            .iter()
            .zip(&pesos)
            .map(|(entrada, peso)| convolucion(entrada, peso, padding, 1, dilation, 1))
            .collect::<Result<Vec<_>>>()?;
        return Tensor::cat(&partes, 1);
    }

    let acolchado = if padding > 0 {
        x.pad_with_zeros(2, padding, padding)?
    } else {
        x.clone()
    };
    // Los pesos se aplanan en el mismo orden en que se apilan las ventanas: primero
    // la posición del núcleo, después el canal.
    let pesos = peso
        .transpose(1, 2)?
        .contiguous()?
        .reshape((c_out, k * c_in_g))?
        .unsqueeze(0)?;

    let mut trozos = Vec::with_capacity(l_out.div_ceil(BLOQUE));
    let mut desde = 0;
    while desde < l_out {
        let cuantas = BLOQUE.min(l_out - desde);
        let ventanas = (0..k)
            .map(|tap| acolchado.narrow(2, desde + tap * dilation, cuantas))
            .collect::<Result<Vec<_>>>()?;
        let columnas = Tensor::cat(&ventanas, 1)?.contiguous()?;
        trozos.push(pesos.broadcast_matmul(&columnas)?);
        desde += cuantas;
    }

    if trozos.len() == 1 {
        Ok(trozos.remove(0))
    } else {
        Tensor::cat(&trozos, 2)
    }
}

/// Una convolución transpuesta, escrita como convoluciones normales.
///
/// # Por qué, y qué identidad se usa
///
/// Es la operación que sube la señal del generador a ritmo de audio, y **la que más
/// costaba**: la de Candle en procesador recorre la salida elemento a elemento.
///
/// Una convolución transpuesta de paso `s` es exactamente **`s` convoluciones
/// normales entrelazadas**: la salida en la posición `t` solo recibe los pesos cuya
/// posición en el núcleo cae en la misma clase de resto que `t` módulo `s`. Así que
/// se parte el núcleo en `s` juegos, se hace una convolución normal con cada uno
/// —por el camino rápido de arriba— y se entrelazan las salidas, que es un cambio de
/// forma y no una copia.
///
/// La identidad no es una aproximación: da los mismos números, y una prueba lo
/// comprueba contra Candle sobre las formas que este modelo usa.
///
/// # Errors
///
/// Los de las operaciones de tensores que la componen.
pub fn convolucion_transpuesta(
    x: &Tensor,
    peso: &Tensor,
    padding: usize,
    output_padding: usize,
    stride: usize,
    groups: usize,
) -> Result<Tensor> {
    // Con grupos no se parte: **medido el 2026-09-18**, partirla cuesta más de lo que
    // ahorra. La prosodia tiene una transpuesta por canal —512 grupos—, y hacer 512
    // convoluciones sueltas y volver a juntarlas le añadía 450 ms a una etapa que
    // entera dura 580. La de Candle recorre los grupos por dentro y de una vez.
    if stride <= 1 || groups > 1 {
        return x.conv_transpose1d(peso, padding, output_padding, stride, 1, groups);
    }

    let (b, _, l_in) = x.dims3()?;
    let (_, c_out, k) = peso.dims3()?;
    let l_out = (l_in - 1) * stride + k + output_padding;
    let l_out = l_out.saturating_sub(2 * padding);

    // Cada fase usa las posiciones del núcleo r, r+s, r+2s… y ninguna más.
    let mut fases = Vec::with_capacity(stride);
    let mut ancho = 0;
    for r in 0..stride {
        let taps: Vec<u32> = (0..)
            .map(|m| r + m * stride)
            .take_while(|j| *j < k)
            // Al revés: una transpuesta es una convolución de verdad, no una
            // correlación, y la diferencia entre las dos es el orden del núcleo.
            .map(|j| u32::try_from(j).unwrap_or(0))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if taps.is_empty() {
            fases.push(None);
            continue;
        }
        let indices = Tensor::new(taps.as_slice(), x.device())?;
        let nucleo = peso
            .index_select(&indices, 2)?
            .transpose(0, 1)?
            .contiguous()?;
        let margen = taps.len() - 1;
        let entrada = if margen > 0 {
            x.pad_with_zeros(2, margen, margen)?
        } else {
            x.clone()
        };
        let salida = convolucion(&entrada, &nucleo, 0, 1, 1, 1)?;
        ancho = ancho.max(salida.dim(2)?);
        fases.push(Some(salida));
    }

    // Todas las fases al mismo ancho: lo que le falta a una es cero de verdad, no un
    // hueco —fuera de su alcance la suma no tiene términos—, así que rellenar no
    // inventa nada.
    let mut completas = Vec::with_capacity(stride);
    for fase in fases {
        let completa = match fase {
            Some(salida) => {
                let suyo = salida.dim(2)?;
                if suyo < ancho {
                    salida.pad_with_zeros(2, 0, ancho - suyo)?
                } else {
                    salida
                }
            }
            None => Tensor::zeros((b, c_out, ancho), x.dtype(), x.device())?,
        };
        completas.push(completa.unsqueeze(3)?);
    }

    // Entrelazar: la posición de salida de la fase r en el paso q es q * s + r, que
    // es justo lo que hace juntar las dos últimas dimensiones.
    let entrelazada =
        Tensor::cat(&completas, 3)?
            .contiguous()?
            .reshape((b, c_out, ancho * stride))?;

    let disponible = ancho * stride;
    if padding >= disponible {
        return Tensor::zeros((b, c_out, l_out), x.dtype(), x.device());
    }
    let util = (disponible - padding).min(l_out);
    let recortada = entrelazada.narrow(2, padding, util)?;
    if util < l_out {
        recortada.pad_with_zeros(2, 0, l_out - util)
    } else {
        Ok(recortada)
    }
}

/// 1D transposed convolution layer.
pub struct ConvTranspose1d {
    weight: Tensor,
    bias: Option<Tensor>,
    stride: usize,
    padding: usize,
    output_padding: usize,
    groups: usize,
}

impl ConvTranspose1d {
    /// Load a ConvTranspose1d from a VarBuilder.
    ///
    /// Weight shape: [in_channels, out_channels, kernel_size]
    ///
    /// Supports both plain `weight` tensors and weight-normalized
    /// `weight_g` + `weight_v` pairs.
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        in_channels: usize,
        out_channels: usize,
        kernel_size: usize,
        stride: usize,
        padding: usize,
        output_padding: usize,
        groups: usize,
        use_bias: bool,
        vb: VarBuilder,
    ) -> Result<Self> {
        let weight_shape = (in_channels, out_channels / groups, kernel_size);
        let weight = match vb.get(weight_shape, "weight") {
            Ok(w) => w,
            Err(_) => {
                let weight_v = vb.get(weight_shape, "weight_v")?;
                // PyTorch stores weight_g as [in, 1, 1]; try 3D first, then 1D
                let weight_g = vb
                    .get((in_channels, 1, 1), "weight_g")
                    .or_else(|_| vb.get(in_channels, "weight_g"))?;
                apply_weight_norm(&weight_v, &weight_g)?
            }
        };
        let bias = if use_bias {
            Some(vb.get(out_channels, "bias")?)
        } else {
            None
        };
        Ok(Self {
            weight,
            bias,
            stride,
            padding,
            output_padding,
            groups,
        })
    }

    /// Forward pass: input shape [batch, in_channels, length]
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let out = convolucion_transpuesta(
            x,
            &self.weight,
            self.padding,
            self.output_padding,
            self.stride,
            self.groups,
        )?;
        match &self.bias {
            Some(bias) => {
                let bias = bias.unsqueeze(0)?.unsqueeze(2)?;
                out.broadcast_add(&bias)
            }
            None => Ok(out),
        }
    }
}

/// Instance Normalization 1D.
///
/// Normalizes each channel independently across the spatial dimension.
/// Formula: (x - mean) / sqrt(var + eps) * weight + bias
pub struct InstanceNorm1d {
    weight: Option<Tensor>,
    bias: Option<Tensor>,
    eps: f32,
}

impl InstanceNorm1d {
    /// Load from VarBuilder with affine parameters.
    pub fn load(num_features: usize, eps: f32, affine: bool, vb: VarBuilder) -> Result<Self> {
        let (weight, bias) = if affine {
            (
                Some(vb.get(num_features, "weight")?),
                Some(vb.get(num_features, "bias")?),
            )
        } else {
            (None, None)
        };
        Ok(Self { weight, bias, eps })
    }

    /// Forward pass: input shape [batch, channels, length]
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let dtype = x.dtype();
        let x_f32 = x.to_dtype(candle_core::DType::F32)?;

        // Compute mean and variance per channel per batch element
        let mean = x_f32.mean_keepdim(2)?;
        let x_centered = x_f32.broadcast_sub(&mean)?;
        let var = x_centered.sqr()?.mean_keepdim(2)?;
        let var = var.broadcast_add(&scalar_like(&var, self.eps)?)?;
        let x_normed = x_centered.broadcast_div(&var.sqrt()?)?;

        let mut result = x_normed.to_dtype(dtype)?;

        if let Some(ref weight) = self.weight {
            let w = weight.unsqueeze(0)?.unsqueeze(2)?;
            result = result.broadcast_mul(&w)?;
        }
        if let Some(ref bias) = self.bias {
            let b = bias.unsqueeze(0)?.unsqueeze(2)?;
            result = result.broadcast_add(&b)?;
        }

        Ok(result)
    }
}

/// Adaptive Instance Normalization (AdaIN).
///
/// Applies InstanceNorm then modulates with style-dependent gamma/beta.
/// Used in Kokoro's ISTFTNet decoder.
pub struct AdaIn1d {
    norm: InstanceNorm1d,
    fc: candle_nn::Linear,
}

impl AdaIn1d {
    /// Load from VarBuilder.
    ///
    /// The InstanceNorm inside AdaIN does NOT have learnable affine
    /// parameters — the style-dependent gamma/beta from `fc` provide
    /// the affine modulation.
    pub fn load(style_dim: usize, num_features: usize, vb: VarBuilder) -> Result<Self> {
        let norm = InstanceNorm1d::load(num_features, 1e-5, true, vb.pp("norm"))
            .or_else(|_| InstanceNorm1d::load(num_features, 1e-5, false, vb.pp("norm")))?;
        let fc = candle_nn::linear(style_dim, num_features * 2, vb.pp("fc"))?;
        Ok(Self { norm, fc })
    }

    /// Forward pass.
    ///
    /// `x`: [batch, channels, length]
    /// `s`: [batch, style_dim]
    pub fn forward(&self, x: &Tensor, s: &Tensor) -> Result<Tensor> {
        let h = self.fc.forward(s)?;
        let h = h.unsqueeze(2)?;
        let half = h.dim(1)? / 2;
        let gamma = h.narrow(1, 0, half)?;
        let beta = h.narrow(1, half, half)?;

        let normed = self.norm.forward(x)?;
        let one = Tensor::ones_like(&gamma)?;
        normed
            .broadcast_mul(&one.add(&gamma)?)?
            .broadcast_add(&beta)
    }
}

/// Adaptive Layer Normalization.
///
/// Used in Kokoro's DurationEncoder.
pub struct AdaLayerNorm {
    fc: candle_nn::Linear,
    eps: f32,
    num_features: usize,
}

impl AdaLayerNorm {
    /// Load from VarBuilder.
    pub fn load(style_dim: usize, num_features: usize, vb: VarBuilder) -> Result<Self> {
        let fc = candle_nn::linear(style_dim, num_features * 2, vb.pp("fc"))?;
        Ok(Self {
            fc,
            eps: 1e-5,
            num_features,
        })
    }

    /// Forward pass.
    ///
    /// `x`: [batch, length, features]
    /// `s`: [batch, style_dim]
    pub fn forward(&self, x: &Tensor, s: &Tensor) -> Result<Tensor> {
        let h = self.fc.forward(s)?;
        let half = self.num_features;
        let gamma = h.narrow(1, 0, half)?.unsqueeze(1)?;
        let beta = h.narrow(1, half, half)?.unsqueeze(1)?;

        // Layer normalization
        let dtype = x.dtype();
        let x_f32 = x.to_dtype(candle_core::DType::F32)?;
        let mean = x_f32.mean_keepdim(candle_core::D::Minus1)?;
        let x_centered = x_f32.broadcast_sub(&mean)?;
        let var = x_centered.sqr()?.mean_keepdim(candle_core::D::Minus1)?;
        let var = var.broadcast_add(&scalar_like(&var, self.eps)?)?;
        let x_normed = x_centered.broadcast_div(&var.sqrt()?)?;
        let x_normed = x_normed.to_dtype(dtype)?;

        let one = Tensor::ones_like(&gamma)?;
        x_normed
            .broadcast_mul(&one.add(&gamma)?)?
            .broadcast_add(&beta)
    }
}

/// Weight normalization wrapper.
///
/// Applies weight normalization to a weight tensor: w = g * (v / ||v||)
/// During inference, the normalized weight is precomputed.
pub fn apply_weight_norm(weight: &Tensor, weight_g: &Tensor) -> Result<Tensor> {
    // weight_g: [out_channels, 1, 1] or [out_channels]
    // weight_v: [out_channels, in_channels, kernel_size]
    let norm = weight
        .sqr()?
        .sum_keepdim(candle_core::D::Minus1)?
        .sum_keepdim(candle_core::D::Minus2)?
        .sqrt()?;
    let weight_g = if weight_g.dims().len() == 1 {
        weight_g.unsqueeze(1)?.unsqueeze(2)?
    } else {
        weight_g.clone()
    };
    weight.broadcast_div(&norm)?.broadcast_mul(&weight_g)
}

/// Channel-wise Layer Normalization.
///
/// Like LayerNorm but loads `gamma`/`beta` instead of `weight`/`bias`.
/// Used by Kokoro's ChannelLayerNorm in the text encoder CNN.
pub struct ChannelNorm {
    gamma: Tensor,
    beta: Tensor,
    eps: f32,
}

impl ChannelNorm {
    /// Load from VarBuilder (expects `gamma` and `beta` tensors).
    pub fn load(num_features: usize, vb: VarBuilder) -> Result<Self> {
        let gamma = vb.get(num_features, "gamma")?;
        let beta = vb.get(num_features, "beta")?;
        Ok(Self {
            gamma,
            beta,
            eps: 1e-5,
        })
    }

    /// Forward pass: input shape [batch, features] or [batch, seq_len, features].
    /// Normalizes over the last dimension.
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let dtype = x.dtype();
        let x_f32 = x.to_dtype(candle_core::DType::F32)?;
        let mean = x_f32.mean_keepdim(candle_core::D::Minus1)?;
        let x_centered = x_f32.broadcast_sub(&mean)?;
        let var = x_centered.sqr()?.mean_keepdim(candle_core::D::Minus1)?;
        let var = var.broadcast_add(&scalar_like(&var, self.eps)?)?;
        let x_normed = x_centered.broadcast_div(&var.sqrt()?)?;
        let x_normed = x_normed.to_dtype(dtype)?;
        x_normed
            .broadcast_mul(&self.gamma)?
            .broadcast_add(&self.beta)
    }
}

/// Linear layer with weight normalization.
pub struct LinearNorm {
    weight: Tensor,
    bias: Option<Tensor>,
}

impl LinearNorm {
    /// Load from VarBuilder.
    pub fn load(in_features: usize, out_features: usize, vb: VarBuilder) -> Result<Self> {
        let weight = vb.get((out_features, in_features), "weight")?;
        let bias = vb.get(out_features, "bias").ok();
        Ok(Self { weight, bias })
    }

    /// Forward pass: x shape [..., in_features] → [..., out_features]
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let w_t = self.weight.t()?;
        let out = x.broadcast_matmul(&w_t)?;
        match &self.bias {
            Some(bias) => out.broadcast_add(bias),
            None => Ok(out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_instance_norm_shape() {
        let device = Device::Cpu;
        let x = Tensor::randn(0f32, 1.0, (2, 16, 100), &device).unwrap();

        let norm = InstanceNorm1d {
            weight: Some(Tensor::ones(16, candle_core::DType::F32, &device).unwrap()),
            bias: Some(Tensor::zeros(16, candle_core::DType::F32, &device).unwrap()),
            eps: 1e-5,
        };

        let out = norm.forward(&x).unwrap();
        assert_eq!(out.dims(), &[2, 16, 100]);
    }

    #[test]
    fn test_linear_norm() {
        let device = Device::Cpu;
        let weight = Tensor::randn(0f32, 0.1, (32, 16), &device).unwrap();
        let bias = Tensor::zeros(32, candle_core::DType::F32, &device).unwrap();
        let linear = LinearNorm {
            weight,
            bias: Some(bias),
        };
        let x = Tensor::randn(0f32, 1.0, (2, 10, 16), &device).unwrap();
        let out = linear.forward(&x).unwrap();
        assert_eq!(out.dims(), &[2, 10, 32]);
    }
}

#[cfg(test)]
mod pruebas_de_la_convolucion {
    use super::*;
    use candle_core::{Device, IndexOp};

    fn diferencia(a: &Tensor, b: &Tensor) -> f32 {
        (a - b)
            .unwrap()
            .abs()
            .unwrap()
            .max_all()
            .unwrap()
            .to_scalar::<f32>()
            .unwrap()
    }

    /// **La propiedad que sostiene el camino rápido**: da lo mismo que el de Candle.
    ///
    /// Se comprueba sobre las formas que el modelo usa de verdad —núcleos de 1, 3, 7
    /// y 11, con dilatación y con grupos—, y con tamaños por encima y por debajo del
    /// bloque, porque partir en bloques es justo donde se pierde una posición sin que
    /// nadie lo note.
    #[test]
    fn la_convolucion_rapida_da_lo_mismo_que_la_de_candle() {
        let device = Device::Cpu;
        // Las longitudes pasan de `LARGO_MINIMO` a propósito: por debajo, los dos
        // lados de la comparación serían el mismo código y la prueba no probaría
        // nada. La última va por debajo, que es el otro camino y también tiene que
        // dar lo mismo.
        let largo = LARGO_MINIMO + 300;
        let casos = [
            // (c_in, c_out, k, padding, dilatación, grupos, longitud)
            (8, 16, 3, 1, 1, 1, largo),
            (8, 16, 7, 3, 1, 1, largo),
            (8, 16, 11, 15, 3, 1, largo),
            (8, 16, 3, 2, 2, 1, largo),
            (8, 16, 1, 0, 1, 1, largo),
            (8, 8, 3, 1, 1, 2, largo),
            // En el borde de un bloque y justo después: partir en bloques es donde se
            // pierde una posición sin que nadie lo note.
            (4, 4, 3, 1, 1, 1, 2 * BLOQUE),
            (4, 4, 3, 1, 1, 1, 2 * BLOQUE + 7),
            (4, 4, 3, 1, 1, 1, 100),
        ];
        for (c_in, c_out, k, padding, dilation, groups, largo) in casos {
            let x = Tensor::randn(0f32, 1.0, (1, c_in, largo), &device).unwrap();
            let peso = Tensor::randn(0f32, 0.2, (c_out, c_in / groups, k), &device).unwrap();
            let rapida = convolucion(&x, &peso, padding, 1, dilation, groups).unwrap();
            let candle = x.conv1d(&peso, padding, 1, dilation, groups).unwrap();
            assert_eq!(rapida.dims(), candle.dims(), "formas distintas con k={k}");
            let d = diferencia(&rapida, &candle);
            assert!(d < 1e-4, "k={k} dilatación={dilation} se aparta {d}");
        }
    }

    /// Con dos elementos en el lote tampoco cambia: la multiplicación se difunde
    /// sobre el lote y equivocarse ahí daría el primer elemento repetido.
    #[test]
    fn el_camino_rapido_respeta_el_lote() {
        let device = Device::Cpu;
        let x = Tensor::randn(0f32, 1.0, (2, 8, LARGO_MINIMO + 100), &device).unwrap();
        let peso = Tensor::randn(0f32, 0.2, (16, 8, 3), &device).unwrap();
        let rapida = convolucion(&x, &peso, 1, 1, 1, 1).unwrap();
        let candle = x.conv1d(&peso, 1, 1, 1, 1).unwrap();
        assert!(diferencia(&rapida, &candle) < 1e-4);
        // Y los dos elementos del lote son distintos, que es lo que la difusión mal
        // hecha rompería sin cambiar la forma.
        let primero = rapida.i(0).unwrap();
        let segundo = rapida.i(1).unwrap();
        assert!(diferencia(&primero, &segundo) > 1e-3);
    }

    /// **La identidad de la transpuesta, comprobada contra Candle.**
    ///
    /// Sobre las formas que el modelo usa de verdad —paso 10 con núcleo 20 y paso 6
    /// con núcleo 12— y sobre casos que las rodean: con relleno, con relleno de
    /// salida y con grupos. Partir el núcleo en fases y volver a entrelazarlo es
    /// justo la clase de cosa que sale casi bien.
    #[test]
    fn la_transpuesta_por_fases_da_lo_mismo_que_la_de_candle() {
        let device = Device::Cpu;
        let casos = [
            // (c_in, c_out, k, paso, relleno, relleno de salida, grupos, largo)
            (16, 8, 20, 10, 5, 0, 1, 40),
            (8, 4, 12, 6, 3, 0, 1, 40),
            (8, 4, 4, 2, 1, 0, 1, 40),
            (8, 4, 5, 2, 1, 1, 1, 40),
            (8, 4, 3, 2, 0, 0, 1, 40),
            // Con grupos va por el camino de Candle, y también tiene que dar lo mismo.
            (8, 8, 4, 2, 1, 0, 2, 40),
        ];
        for (c_in, c_out, k, stride, padding, output_padding, groups, largo) in casos {
            let x = Tensor::randn(0f32, 1.0, (1, c_in, largo), &device).unwrap();
            let peso = Tensor::randn(0f32, 0.2, (c_in, c_out / groups, k), &device).unwrap();
            let nuestra =
                convolucion_transpuesta(&x, &peso, padding, output_padding, stride, groups)
                    .unwrap();
            let candle = x
                .conv_transpose1d(&peso, padding, output_padding, stride, 1, groups)
                .unwrap();
            assert_eq!(
                nuestra.dims(),
                candle.dims(),
                "formas distintas con paso {stride} y núcleo {k}"
            );
            let d = diferencia(&nuestra, &candle);
            assert!(d < 1e-4, "paso {stride}, núcleo {k}: se aparta {d}");
        }
    }
}
