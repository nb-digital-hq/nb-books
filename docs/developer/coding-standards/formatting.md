# Guía de Formateo (Formatting & Linting)

> **Estado:** Vigente.
> Las discusiones sobre si usar tabs o espacios, o dónde va la llave, roban tiempo. El CI rechazará cualquier código que no pase los linters automatizados.

## 1. Reglas Universales

* **Codificación:** `UTF-8` (sin BOM).
* **Fines de línea:** `LF` (Unix). Obligatorio configurar `core.autocrlf = input` en Windows.
* **Espacios al final:** Ninguno.
* **Fin de archivo:** Una única línea vacía al final de todos los archivos.

---

## 2. Rust Backend

El backend se rige estrictamente por la configuración estándar de `rustfmt`.

**Comandos obligatorios antes de un commit:**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```
> **Nota:** Si Clippy da una advertencia, **es un error**. Se debe arreglar. No se permite silenciar clippy globalmente (ej. `#![allow(clippy::all)]`). Se permiten silencios granulares (`#[allow(clippy::specific_lint)]`) solo si hay justificación explícita en comentario.

---

## 3. TypeScript, Svelte y CSS (Frontend)

En lugar de mantener docenas de configuraciones superpuestas de Prettier y ESLint, el proyecto utiliza **Biome** (para TS/JS/JSON) + **Prettier** (solo para Svelte/CSS).

### Reglas clave del Frontend:
1. **Indentación:** 2 espacios (Tabs prohibidos).
2. **Comillas:** Comillas simples (`'`) para código JS/TS, comillas dobles (`"`) para atributos HTML en Svelte.
3. **Punto y coma:** Obligatorio (semi: true).
4. **Largo de línea (Print Width):** 100 caracteres.

**Comandos de verificación:**
```bash
# Validar TS/JS/JSON
pnpm biome check .

# Validar Svelte y CSS
pnpm prettier --check "**/*.{svelte,css}"

# Validar tipos (Svelte)
pnpm svelte-check
```

---

## 4. Ordenamiento de Imports

Para facilitar la resolución de conflictos (merge conflicts) y la legibilidad:

### Rust
Agrupar los imports separándolos con una línea en blanco:
1. `std::*` y `core::*`
2. Librerías externas (`rusqlite`, `serde`, `tauri`)
3. Módulos internos (`crate::*` o `super::*`)

### TypeScript / Svelte
El linter está configurado para auto-ordenar. El orden conceptual es:
1. Built-ins de Node (si aplican)
2. Librerías de terceros (ej. `svelte`, `@tauri-apps/api`)
3. Componentes Svelte absolutos (`$lib/components/*`)
4. Stores y helpers absolutos (`$lib/stores/*`)
5. Imports relativos locales (`./helper`)

---

## 5. Automatización (Git Hooks)

Los desarrolladores no deben recordar ejecutar esto manualmente. El repositorio provee una configuración de `husky` (o `git hooks` nativos).
Al hacer `git commit`, los linters correrán automáticamente sobre los archivos en *stage*.

Prettier cubre **solo** Svelte y CSS. Los documentos de `docs/` se ajustan a mano a 110 columnas: son prosa
técnica y el reajuste automático produce diferencias ilegibles en cada revisión.
