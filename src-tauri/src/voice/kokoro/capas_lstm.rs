//! La LSTM que usan el codificador de texto y la prosodia de Kokoro.
//!
//! Solo inferencia, con los pesos tal y como los guarda PyTorch.
//!
//! # Lo que se cambió del original, y por qué se nota
//!
//! El código del que viene esto hacía dos cosas por cada paso de tiempo que
//! costaban casi todo el tiempo de la etapa:
//!
//! 1. **Transponía los dos pesos y los copiaba** —`weight.t()?.contiguous()?`— en
//!    cada paso. Para una matriz de 2048 por 512 eso es copiar cuatro megabytes
//!    doscientas cincuenta veces por dirección y por capa, para obtener siempre lo
//!    mismo. Aquí se transponen **una vez, al cargar**.
//! 2. Multiplicaba la entrada por su peso **paso a paso**. La parte de la entrada
//!    no depende del estado anterior, así que se puede hacer de una vez para toda
//!    la secuencia: una multiplicación grande en lugar de doscientas cincuenta
//!    pequeñas, que es donde una biblioteca de álgebra gana.
//!
//! Lo que queda dentro del bucle es lo único que de verdad es secuencial: la
//! recurrencia sobre el estado.

use candle_core::{Result, Tensor};
use candle_nn::VarBuilder;

/// A single LSTM layer (unidirectional or bidirectional).
///
/// Compatible with PyTorch's `nn.LSTM` weight naming:
/// - `weight_ih_l0` shape `[4 * hidden, input]`
/// - `weight_hh_l0` shape `[4 * hidden, hidden]`
/// - `bias_ih_l0` shape `[4 * hidden]`
/// - `bias_hh_l0` shape `[4 * hidden]`
/// - For reverse: `weight_ih_l0_reverse`, etc.
pub struct Lstm {
    layers: Vec<LstmLayer>,
    bidirectional: bool,
}

/// Los pesos de una dirección, **ya transpuestos**.
struct Direccion {
    /// `weight_ih` transpuesto: `[entrada, 4 * oculto]`.
    w_ih_t: Tensor,
    /// `weight_hh` transpuesto: `[oculto, 4 * oculto]`.
    w_hh_t: Tensor,
    bias_ih: Tensor,
    bias_hh: Tensor,
}

struct LstmLayer {
    adelante: Direccion,
    /// Solo en las bidireccionales.
    atras: Option<Direccion>,
    hidden_size: usize,
}

impl Lstm {
    /// Load an LSTM from a VarBuilder.
    ///
    /// `num_layers`: number of stacked LSTM layers
    /// `input_size`: input feature dimension
    /// `hidden_size`: hidden state dimension
    /// `bidirectional`: whether to use bidirectional LSTM
    pub fn load(
        num_layers: usize,
        input_size: usize,
        hidden_size: usize,
        bidirectional: bool,
        vb: VarBuilder,
    ) -> Result<Self> {
        let mut layers = Vec::with_capacity(num_layers);
        let dir_factor = if bidirectional { 2 } else { 1 };

        for layer_idx in 0..num_layers {
            let in_size = if layer_idx == 0 {
                input_size
            } else {
                hidden_size * dir_factor
            };

            let cargar = |sufijo: &str| -> Result<Direccion> {
                Direccion::cargar(&vb, layer_idx, in_size, hidden_size, sufijo)
            };
            let adelante = cargar("")?;
            let atras = if bidirectional {
                Some(cargar("_reverse")?)
            } else {
                None
            };

            layers.push(LstmLayer {
                adelante,
                atras,
                hidden_size,
            });
        }

        Ok(Self {
            layers,
            bidirectional,
        })
    }

    /// Forward pass through the LSTM.
    ///
    /// `input`: tensor of shape `[batch, seq_len, input_size]`
    ///
    /// Returns output of shape `[batch, seq_len, hidden_size * num_directions]`
    pub fn forward(&self, input: &Tensor) -> Result<Tensor> {
        let mut current = input.clone();

        for layer in &self.layers {
            current = layer.forward(&current, self.bidirectional)?;
        }

        Ok(current)
    }
}

impl Direccion {
    fn cargar(
        vb: &VarBuilder,
        capa: usize,
        entrada: usize,
        oculto: usize,
        sufijo: &str,
    ) -> Result<Self> {
        let w_ih = vb.get((4 * oculto, entrada), &format!("weight_ih_l{capa}{sufijo}"))?;
        let w_hh = vb.get((4 * oculto, oculto), &format!("weight_hh_l{capa}{sufijo}"))?;
        Ok(Self {
            // Transpuestos aquí: dentro del bucle se hacía en cada paso.
            w_ih_t: w_ih.t()?.contiguous()?,
            w_hh_t: w_hh.t()?.contiguous()?,
            bias_ih: vb.get(4 * oculto, &format!("bias_ih_l{capa}{sufijo}"))?,
            bias_hh: vb.get(4 * oculto, &format!("bias_hh_l{capa}{sufijo}"))?,
        })
    }
}

impl LstmLayer {
    fn forward(&self, input: &Tensor, bidirectional: bool) -> Result<Tensor> {
        let fwd = self.forward_direction(input, &self.adelante, false)?;

        if bidirectional {
            let Some(atras) = self.atras.as_ref() else {
                return Ok(fwd);
            };
            let rev = self.forward_direction(input, atras, true)?;
            Tensor::cat(&[&fwd, &rev], 2)
        } else {
            Ok(fwd)
        }
    }

    fn forward_direction(
        &self,
        input: &Tensor,
        pesos: &Direccion,
        reverse: bool,
    ) -> Result<Tensor> {
        let (batch, seq_len, entrada) = input.dims3()?;
        let device = input.device();
        let dtype = input.dtype();

        // La parte de la entrada no depende del estado, así que sale de una sola
        // multiplicación para toda la secuencia.
        let puertas_de_entrada = input
            .reshape((batch * seq_len, entrada))?
            .contiguous()?
            .matmul(&pesos.w_ih_t)?
            .broadcast_add(&pesos.bias_ih)?
            .reshape((batch, seq_len, 4 * self.hidden_size))?;

        let mut h = Tensor::zeros((batch, self.hidden_size), dtype, device)?;
        let mut c = Tensor::zeros((batch, self.hidden_size), dtype, device)?;
        let mut outputs = Vec::with_capacity(seq_len);

        let range: Vec<usize> = if reverse {
            (0..seq_len).rev().collect()
        } else {
            (0..seq_len).collect()
        };

        for &t in &range {
            let gates = puertas_de_entrada
                .narrow(1, t, 1)?
                .squeeze(1)?
                .add(&h.matmul(&pesos.w_hh_t)?.broadcast_add(&pesos.bias_hh)?)?;

            let gate_size = self.hidden_size;
            let i_gate = candle_nn::ops::sigmoid(&gates.narrow(1, 0, gate_size)?)?;
            let f_gate = candle_nn::ops::sigmoid(&gates.narrow(1, gate_size, gate_size)?)?;
            let g_gate = gates.narrow(1, 2 * gate_size, gate_size)?.tanh()?;
            let o_gate = candle_nn::ops::sigmoid(&gates.narrow(1, 3 * gate_size, gate_size)?)?;

            c = f_gate.mul(&c)?.add(&i_gate.mul(&g_gate)?)?;
            h = o_gate.mul(&c.tanh()?)?;

            outputs.push(h.unsqueeze(1)?);
        }

        if reverse {
            outputs.reverse();
        }

        Tensor::cat(&outputs, 1)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use candle_core::Device;

    fn direccion(entrada: usize, oculto: usize, device: &Device) -> (Direccion, Tensor, Tensor) {
        let w_ih = Tensor::randn(0f32, 0.1, (4 * oculto, entrada), device).unwrap();
        let w_hh = Tensor::randn(0f32, 0.1, (4 * oculto, oculto), device).unwrap();
        let bias_ih = Tensor::randn(0f32, 0.1, 4 * oculto, device).unwrap();
        let bias_hh = Tensor::randn(0f32, 0.1, 4 * oculto, device).unwrap();
        (
            Direccion {
                w_ih_t: w_ih.t().unwrap().contiguous().unwrap(),
                w_hh_t: w_hh.t().unwrap().contiguous().unwrap(),
                bias_ih: bias_ih.clone(),
                bias_hh: bias_hh.clone(),
            },
            w_ih,
            w_hh,
        )
    }

    /// La recurrencia tal y como estaba escrita antes: paso a paso, con la
    /// multiplicación de la entrada dentro del bucle. Es la referencia contra la
    /// que se comprueba que acelerar no cambió el resultado.
    fn como_estaba(
        input: &Tensor,
        w_ih: &Tensor,
        w_hh: &Tensor,
        bias_ih: &Tensor,
        bias_hh: &Tensor,
        oculto: usize,
    ) -> Tensor {
        let (batch, seq, _) = input.dims3().unwrap();
        let mut h = Tensor::zeros((batch, oculto), input.dtype(), input.device()).unwrap();
        let mut c = h.clone();
        let mut salidas = Vec::new();
        for t in 0..seq {
            let x = input.narrow(1, t, 1).unwrap().squeeze(1).unwrap();
            let gates = x
                .contiguous()
                .unwrap()
                .matmul(&w_ih.t().unwrap().contiguous().unwrap())
                .unwrap()
                .broadcast_add(bias_ih)
                .unwrap()
                .add(
                    &h.matmul(&w_hh.t().unwrap().contiguous().unwrap())
                        .unwrap()
                        .broadcast_add(bias_hh)
                        .unwrap(),
                )
                .unwrap();
            let i = candle_nn::ops::sigmoid(&gates.narrow(1, 0, oculto).unwrap()).unwrap();
            let f = candle_nn::ops::sigmoid(&gates.narrow(1, oculto, oculto).unwrap()).unwrap();
            let g = gates.narrow(1, 2 * oculto, oculto).unwrap().tanh().unwrap();
            let o = candle_nn::ops::sigmoid(&gates.narrow(1, 3 * oculto, oculto).unwrap()).unwrap();
            c = f.mul(&c).unwrap().add(&i.mul(&g).unwrap()).unwrap();
            h = o.mul(&c.tanh().unwrap()).unwrap();
            salidas.push(h.unsqueeze(1).unwrap());
        }
        Tensor::cat(&salidas, 1).unwrap()
    }

    /// **La propiedad que sostiene la optimización.** Sacar la multiplicación de la
    /// entrada del bucle y transponer los pesos una sola vez da **el mismo
    /// resultado** que hacerlo paso a paso.
    ///
    /// Sin esta prueba, "va más rápido" y "dice otra cosa" se parecen demasiado:
    /// un error aquí no da error, da una voz que suena mal.
    #[test]
    fn acelerar_la_recurrencia_no_cambia_el_resultado() {
        let device = Device::Cpu;
        let (oculto, entrada) = (32, 16);
        let (pesos, w_ih, w_hh) = direccion(entrada, oculto, &device);
        let capa = LstmLayer {
            adelante: pesos,
            atras: None,
            hidden_size: oculto,
        };
        let bias_ih = capa.adelante.bias_ih.clone();
        let bias_hh = capa.adelante.bias_hh.clone();

        let input = Tensor::randn(0f32, 1.0, (2, 10, entrada), &device).unwrap();
        let ahora = capa.forward(&input, false).unwrap();
        let antes = como_estaba(&input, &w_ih, &w_hh, &bias_ih, &bias_hh, oculto);

        let diferencia = (ahora - antes)
            .unwrap()
            .abs()
            .unwrap()
            .max_all()
            .unwrap()
            .to_scalar::<f32>()
            .unwrap();
        assert!(
            diferencia < 1e-5,
            "la recurrencia acelerada se aparta {diferencia}"
        );
    }

    #[test]
    fn la_salida_tiene_la_forma_que_declara() {
        let device = Device::Cpu;
        let (oculto, entrada) = (32, 16);
        let (pesos, _, _) = direccion(entrada, oculto, &device);
        let lstm = Lstm {
            layers: vec![LstmLayer {
                adelante: pesos,
                atras: None,
                hidden_size: oculto,
            }],
            bidirectional: false,
        };
        let input = Tensor::randn(0f32, 1.0, (2, 10, entrada), &device).unwrap();
        assert_eq!(lstm.forward(&input).unwrap().dims(), &[2, 10, oculto]);
    }
}
