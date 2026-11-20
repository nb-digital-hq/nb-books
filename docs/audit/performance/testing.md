# Pruebas de rendimiento

> **Estado:** Implementado. Los valores límite viven en un único lugar,
> [performance-budgets.md](../../architecture/performance-budgets.md); este documento describe **cómo se miden**.
> Si un número aparece en los dos y difieren, manda el de presupuestos.

## 1. Qué se mide y con qué

| Capa | Mecanismo | Qué mide | Cuándo |
|---|---|---|---|
| Paquete de la interfaz | `scripts/check-paquete.mjs` | Peso del arranque (menos de 250 KB) y que los trozos diferidos sigan diferidos | `pnpm verify`, tras compilar |
| Interfaz | `e2e/rendimiento.spec.ts` (Playwright) | Nodos del DOM con 150.000 obras, desplazamiento, capas de composición, nada animándose en reposo, cromo opaco, `:hover` sin mover cajas | `pnpm verify` |
| Runtime | `src-tauri/tests/recorrido_biblioteca.rs` y las pruebas de presupuesto de cada módulo | Latencia de las consultas de catálogo sobre miles de documentos sintéticos | `cargo test` |
| Binario | `pnpm baseline` (`scripts/measure-baseline.mjs`) | Arranque del runtime, peso del paquete y del binario en release, sobre la aplicación vacía | A mano, antes de publicar |
| Corpus real | `src-tauri/tests/corpus_real.rs` | Importación, indexación y consultas sobre una biblioteca real | A mano, con `NB_BOOKS_CORPUS` |

La medición base registrada está en [baseline-2026-08-26.json](baseline-2026-08-26.json).

## 2. Simulación en lugar de corpus en la verificación obligatoria

La verificación obligatoria no puede depender de un volumen que solo existe en un equipo. Las pruebas de
`pnpm verify` y `cargo test` usan bibliotecas **sintéticas** que conservan la variedad que importa medir: rutas,
portadas y longitudes distintas. Un banco que colapsa la dimensión medida convierte cada medición en una
confirmación.

## 3. El corpus de referencia

El corpus de referencia es una **biblioteca real de unos 152.000 EPUB repartidos en unos 18.800 directorios de
autor**, del orden de 150 GiB, con su índice de metadatos en JSON y CSV. No vive en el repositorio ni se
distribuye con él: es un volumen local, y las pruebas que lo usan se omiten solas
cuando no está.

Las reglas de uso no son negociables:

* **Solo lectura.** Ninguna prueba escribe, renombra ni cambia permisos dentro del corpus. Se monta siempre en
  modo **referenciado**, nunca administrado, porque el modo administrado copiaría archivos.
* **No se duplica.** Su tamaño lo hace inviable, y una copia parcial deja de ser representativa.
* **El directorio de datos va aislado** con `NB_BOOKS_TEST_DIR`, como el resto de las pruebas de extremo a
  extremo.
* **Nada derivado del corpus entra en el repositorio ni en los registros:** ni rutas, ni títulos, ni autores, ni
  fragmentos. Son obras con derechos de autor vigentes, y además lo prohíbe ya la regla de redacción de
  [logging.md](../../developer/coding-standards/logging.md). Lo que se publica de una medición son números agregados.
* **Las cifras se citan aproximadas**, porque el corpus se actualiza y una cifra exacta en un documento envejece
  peor que un orden de magnitud.

Para pruebas repetibles se toma una muestra determinista en lugar de recorrer el corpus entero: recorrerlo
completo mide el disco, no el producto.

Las pruebas que lo usan viven en `src-tauri/tests/corpus_real.rs` y se localizan con la variable de entorno
`NB_BOOKS_CORPUS`. La ruta no está codificada en el repositorio a propósito: es de cada equipo y el repositorio es público.

```bash
NB_BOOKS_CORPUS=/ruta/al/corpus cargo test --test corpus_real -- --nocapture
```

Sin esa variable las pruebas se omiten solas y lo dicen. **No pueden formar parte de la suite obligatoria**,
porque el corpus solo existe en una máquina.

## 4. Política de regresión

* Un presupuesto de [performance-budgets.md](../../architecture/performance-budgets.md) que tiene prueba
  automática falla la verificación al superarse; no hay margen de tolerancia aparte.
* Las pruebas de tiempo son sensibles a la carga del equipo: se ejecutan con la máquina sin otro trabajo pesado, y
  un fallo bajo carga se repite antes de tomarse por regresión.
