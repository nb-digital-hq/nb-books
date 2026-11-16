# Inteligencia artificial

> **Estado:** Implementado en `src-tauri/src/ai/`: contrato, motor local con Candle, catálogo y descarga
> verificada de modelos, presupuesto de recursos, búsqueda por significado, funciones de estudio y proveedores
> externos opcionales.

La IA en NB Books es una **capacidad de enriquecimiento, no la identidad del producto**. Se rige por *Privacy by
Design* y *Local First*: ver [philosophy.md](philosophy.md) y [privacy.md](privacy.md).

## 1. Qué hace

| Función | Dónde | Documento |
|---|---|---|
| Búsqueda por significado, combinada con la léxica | `ai/indice.rs`, `ai/busqueda.rs` | [embeddings.md](embeddings.md), [search.md](../search.md) |
| Resúmenes, tarjetas, preguntas de autoevaluación y conceptos, citando los fragmentos de origen | `ai/estudio.rs`, `ai/generacion.rs`, `ai/salidas.rs` | [prompts.md](prompts.md) |
| Cuaderno de estudio y repaso espaciado | `ai/cuaderno.rs`, `ai/repaso_espaciado.rs` | [ADR-013](../../adr/013-information-architecture.md) |
| Evaluación reproducible de calidad | `ai/rubrica.rs`, `ai/reportes.rs` | [evaluation.md](evaluation.md) |

## 2. Motores

1. **Motor local: Candle**, en Rust puro. Sin motor de C++ por FFI ni backends por plataforma: el motivo está en
   la sección 1 de [local-models.md](local-models.md).
2. **Embeddings locales**, cuantizados a 8 bits en una columna de la base, junto al fragmento y su Locator; la
   consulta es por fuerza bruta sobre los candidatos que acota FTS5. Ver [ADR-007](../../adr/007-vector-store.md).
3. **Proveedores externos (opcionales):** el protocolo compatible con OpenAI (que hablan también Ollama, vLLM,
   LM Studio y la mayoría de servicios de pago) y el nativo de Anthropic. Se usan **únicamente** si el usuario los
   configura, nunca generan embeddings, y el primer envío enseña entero qué saldría
   (`ai/externo.rs`, `ai/envios.rs`).

Los modelos son componentes de primera parte: se descargan bajo petición, se verifican por huella y su licencia
consta en el [registro de revisiones](../../legal/model-reviews.md).

## 3. Degradación y recursos

El subsistema de IA es un componente con estado de salud (`ai/subsistema.rs`). Sin modelo instalado está
`NotInstalled`; si el equipo no puede ejecutarlo, `NotSupported`; si falla, `Degraded`. En los tres casos la
interfaz oculta lo que depende de él y la lectura sigue intacta. `ai/hardware.rs` mide el equipo y
`ai/presupuesto.rs` limita la memoria y los hilos de la inferencia.
