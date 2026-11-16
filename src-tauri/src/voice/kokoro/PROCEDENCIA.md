# Procedencia de este motor de voz

El motor de **Kokoro-82M** que hay en esta carpeta está **adaptado de
[`any-tts` 0.1.3](https://github.com/TM9657/any-tts)**, de TM9657, publicado bajo
**MIT o Apache-2.0**, según declara su `Cargo.toml`. Aquí se dice qué se tomó, qué se
cambió y por qué no se depende del crate.

**Su repositorio no publica archivo de licencia**, comprobado el 2026-09-18: ni
`LICENSE`, ni `LICENSE-MIT`, ni `LICENSE-APACHE`, y la API de GitHub no detecta
ninguna. Lo único que concede la licencia es la declaración SPDX de su `Cargo.toml`,
que es la declaración del editor y lo que publica crates.io. Así que **no hay aviso de
copyright que reproducir**, y la atribución nombra el proyecto, su versión y su
origen, que es lo que se puede sostener. Está dicho así en `THIRD-PARTY-NOTICES.md` y
**es una decisión que puede revisarse**: pedirle al autor que publique el
archivo, o rehacer lo que queda copiado.

## Qué se tomó

| Archivo de aquí | De allí | Qué es |
|---|---|---|
| `albert.rs` | `src/models/kokoro/albert.rs` | El codificador de fonemas (PL-BERT) |
| `atencion.rs` | `src/tensor_utils.rs`, parcial | La atención, con el camino rápido de Candle |
| `capas_conv.rs` | `src/layers/conv.rs` | Convoluciones con normalización adaptativa |
| `capas_lstm.rs` | `src/layers/lstm.rs` | La LSTM que usan la prosodia y el texto |
| `config.rs` | `src/models/kokoro/config.rs` | La forma del modelo y su vocabulario |
| `decoder.rs` | `src/models/kokoro/decoder.rs` | El generador de onda (ISTFTNet) |
| `prosody.rs` | `src/models/kokoro/prosody.rs` | Duración, tono y ruido |
| `text_encoder.rs` | `src/models/kokoro/text_encoder.rs` | El codificador de texto |

## Qué **no** se tomó, y por qué

* **Su fonemizador** (`espeak_compat.rs`, `english_g2p.rs`, `phonemizer.rs`). El
  nuestro está en `voice::fonemas`, y existe por dos motivos medidos el 2026-09-17:
  el suyo convierte `/x/` en `/k/` en todos los idiomas —en español "rojo" sonaba
  "roko"— y solo habla español de España. Ver `local-models.md` sección 7.2.
* **La clonación de voz** (`style_encoder.rs`, `voice_from_reference`). No entra por
  decisión escrita: el criterio de la pregunta F2
  ([model-reviews.md](../../../../docs/legal/model-reviews.md) sección 2.1) prohíbe
  que el producto clone la voz de una persona a partir de una muestra.

## Por qué adaptado y no como dependencia

Se decidió el 2026-09-17, con los datos delante: su API no admite fonemas
propios —así que su fonemizador decidiría la calidad del español—, y el camino
crítico de una función de lectura no puede pasar por un crate de 30 commits y un
solo mantenedor. Adaptándolo, el mantenimiento es nuestro y la calidad también.

## Cambios sobre el original

* Los errores son los del proyecto (`NbError`), no los suyos.
* La entrada son **fonemas**, no texto: quien los produce es `voice::fonemas`.
* Se quitó todo lo relativo a clonación de voz y a los otros modelos del crate.
* Los comentarios y la documentación se reescriben en español, como el resto del
  repositorio, conservando lo que explican.
