//! La atención de ALBERT, con el camino rápido de Candle cuando aplica.
//!
//! **Procedencia.** Adaptado de `any-tts` 0.1.3 (`src/tensor_utils.rs`), de TM9657,
//! bajo MIT o Apache-2.0. Ver `src/voice/kokoro/PROCEDENCIA.md`.

use candle_core::{DType, Device, Result, Tensor};

fn cpu_flash_attention_shape_eligible(q_heads: usize, k_heads: usize, q_seq: usize) -> bool {
    q_heads == k_heads || q_seq == 1
}

pub fn cpu_flash_attention(
    q: &Tensor,
    k: &Tensor,
    v: &Tensor,
    mask: Option<&Tensor>,
    scale: f32,
) -> Result<Option<Tensor>> {
    let (batch_size, q_heads, q_seq, _head_dim) = q.dims4()?;
    let (_k_batch, k_heads, k_seq, _k_head_dim) = k.dims4()?;

    if !matches!(q.device(), Device::Cpu)
        || !matches!(k.device(), Device::Cpu)
        || !matches!(v.device(), Device::Cpu)
    {
        return Ok(None);
    }

    if q.dtype() != DType::F32 || k.dtype() != DType::F32 || v.dtype() != DType::F32 {
        return Ok(None);
    }

    // Single-token incremental decode is a useful fast path even for grouped
    // attention, but full-sequence grouped-query attention fell back better in
    // OmniVoice-style runs. Keep the CPU fused path conservative.
    if !cpu_flash_attention_shape_eligible(q_heads, k_heads, q_seq) {
        return Ok(None);
    }

    if let Some(mask) = mask {
        if !matches!(mask.device(), Device::Cpu) || mask.dtype() != DType::F32 {
            return Ok(None);
        }

        let supported_mask = match mask.dims() {
            [mask_batch, mask_heads, mask_q_seq, mask_k_seq] => {
                *mask_batch == batch_size
                    && *mask_heads == 1
                    && *mask_q_seq == q_seq
                    && *mask_k_seq == k_seq
            }
            [mask_batch, mask_q_seq, mask_k_seq] => {
                *mask_batch == batch_size && *mask_q_seq == q_seq && *mask_k_seq == k_seq
            }
            _ => false,
        };

        if !supported_mask {
            return Ok(None);
        }
    }

    let q = q.transpose(1, 2)?.contiguous()?;
    let k = k.transpose(1, 2)?.contiguous()?;
    let v = v.transpose(1, 2)?.contiguous()?;
    // La API nueva de Candle: la máscara viaja como `AttnMask` en vez de como un
    // tensor opcional. La vieja quedó obsoleta en 0.11.
    let mascara = match mask {
        Some(m) => candle_nn::attention::AttnMask::Mask(m.clone()),
        None => candle_nn::attention::AttnMask::None,
    };
    let attn = candle_nn::attention::flash_attn::<f32>(&q, &k, &v, scale, mascara, None, None)?;
    Ok(Some(attn))
}
