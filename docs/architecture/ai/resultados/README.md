# Resultados de evaluación publicados

Contrato: [evaluation.md](../evaluation.md), sección 4, reglas 2 y 4. Iteración **la implementación**.

Cada archivo de esta carpeta es **una ejecución de una suite**, con su fecha, la suite que lo produjo, el modelo
medido y, si la función usa plantillas, la versión de cada una. Se publican para que cualquiera pueda auditarlos,
que es la regla 4, y para que un cambio de plantilla no pase inadvertido, que es la regla 1: `check-evaluacion.mjs`
cruza estos archivos con `src-tauri/prompts/` en cada `pnpm verify`.

| Archivo | Fecha | Suite | Qué mide |
|---|---|---|---|
| `recuperacion-2026-09-19.json` | 2026-09-19 | `tests/metricas_de_recuperacion.rs` | Si la búsqueda por significado encuentra el párrafo que contesta, y en qué posición. Repetida el 2026-09-22 con las mismas cifras |
| `tarjetas-qwen3-4b-2026-09-22.json` | 2026-09-22 | `tests/evaluacion_generativa.rs`, solo tarjetas | Las tarjetas con Qwen3 4B, para decidir el cierre de la fase. **Tampoco supera**: cinco de seis, fallando otro texto |
| `generativa-2026-09-22.json` | 2026-09-22 | `tests/evaluacion_generativa.rs` | Las cuatro funciones de estudio con el modelo local recomendado, salida por salida, con latencia, memoria y fallos. **No supera**: las tarjetas pasan cinco de seis textos |

## Lo que todavía no hay aquí, dicho como tal

**Resultados completos de los otros dos modelos del catálogo**: de Qwen3 4B solo están las tarjetas, y de Qwen3
1.7B nada. El que se mide es el recomendado para
el equipo donde se ejecuta, que es lo que usaría quien instala sin tocar nada; los otros dos se ofrecen, y medirlos
es cambiar `NB_MODELO`.

## Cómo se produce un archivo de estos

```bash
# Recuperación, con el modelo de embeddings del catálogo
NB_BOOKS_MODELO_REAL=<carpeta del modelo> NB_METRICAS=docs/architecture/ai/resultados/recuperacion-<fecha>.json \
  cargo test --release --test metricas_de_recuperacion -- --nocapture

# Funciones generativas, con AVX2 compilado y el modelo instalado
RUSTFLAGS="-C target-cpu=x86-64-v3" NB_MODELOS=<carpeta> \
  NB_EVALUACION=docs/architecture/ai/resultados/generativa-<fecha>.json \
  cargo test --release --test evaluacion_generativa -- --nocapture
```

Al archivo de la suite hay que añadirle a mano `fecha`, `suite`, `iteracion` y `equipo`, que la suite no sabe.
