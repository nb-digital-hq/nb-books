# Convenciones de Nombrado (Naming Conventions)

> **Estado:** Vigente.
> Las siguientes reglas aplican a todo el código fuente de NB Books. Ningún PR será aprobado si viola estas convenciones.

Debido a que el proyecto es un ecosistema políglota (Rust + TypeScript + Svelte + CSS + SQL), es fácil perder la coherencia. Estas son las reglas estrictas para cada lenguaje.

---

## 1. Rust (Backend)

Sigue el estándar idiomático de Rust (`rustfmt`).

| Entidad | Regla | Ejemplo |
|---|---|---|
| Módulos y Archivos | `snake_case.rs` | `pdf_engine.rs` |
| Tipos, Structs, Enums, Traits | `PascalCase` | `DocumentRecord`, `FormatProvider` |
| Variables y Funciones | `snake_case` | `render_pdf_page`, `doc_id` |
| Constantes y Estáticos | `SCREAMING_SNAKE_CASE` | `MAX_RENDER_THREADS` |
| Lifetimes | Cortos y en minúscula | `'a`, `'ctx` |

**Precedencia de dominio:** No prefixar los tipos del dominio con `Nb` (ej. no usar `NbDocument`). Los namespaces resuelven la ambigüedad (`nb::models::Document`).

> **Excepción declarada:** el tipo de error del dominio y su alias sí llevan prefijo (`NbError`, `NbResult`,
> `NbErrorPayload`). El motivo es que `Error` y `Result` colisionan con los nombres de la biblioteca estándar y
> obligarían a renombrar en cada importación, y porque `NbError` es además el nombre del tipo que el frontend
> recibe por el puente, donde no hay espacios de nombres que desambigüen. Es la única excepción; cualquier otra
> necesita entrada aquí.

---

## 2. TypeScript / JavaScript (Frontend)

| Entidad | Regla | Ejemplo |
|---|---|---|
| Archivos fuente genéricos | `camelCase.ts` | `appResolver.ts`, `ipc.ts` |
| Clases e Interfaces | `PascalCase` | `Locator`, `StoreManager` |
| Instancias, Variables, Funciones | `camelCase` | `fetchMetadata`, `isLoaded` |
| Tipos de Eventos (Event Bus) | `PascalCase` | `ResourceImported` |
| Constantes globales | `SCREAMING_SNAKE_CASE` | `DEFAULT_ANIMATION_MS` |

**Comandos IPC:** Los nombres de comandos exportados por Tauri (macros en Rust) usan `snake_case`. Sin embargo, el wrapper de TypeScript (`src/lib/core/ipc.ts`) debe exponerlos como `camelCase`.
* Rust: `#[tauri::command] fn import_book_file`
* TypeScript export: `export async function importBookFile`

---

## 3. Svelte (Componentes y Stores)

| Entidad | Regla | Ejemplo |
|---|---|---|
| Archivos de Componente | `PascalCase.svelte` | `LibraryGrid.svelte`, `ReaderSurface.svelte` |
| Rutas (SvelteKit) | `+page.svelte`, `+layout.svelte` | Estándar de SvelteKit. Nombres de directorios en `snake_case`. |
| Stores (Runes) | `camelCase.svelte.ts` | `settingsStore.svelte.ts` |
| Props del componente | `camelCase` | `let isDraggable = $props;` |

**Convención de punto de extensión:** los componentes que una extensión monta en una ranura de la carcasa se
nombran por la ranura que ocupan, no por un tipo genérico: `...Panel.svelte`, `...View.svelte`,
`...NavItem.svelte`. Ver los puntos de extensión en
[architecture/ui-components.md](../../architecture/ui-components.md).

---

## 4. CSS y Estilos

El proyecto utiliza CSS semántico. **No usamos Tailwind** (ver [Filosofía de Producto](../../design/product-philosophy.md) respecto al marcado limpio). Usamos la convención **BEM modificado**.

| Entidad | Regla | Ejemplo |
|---|---|---|
| Clases (Bloques y Elementos) | `kebab-case` | `.reader-sidebar`, `.book-cover` |
| Estados (Modificadores) | Prefix `is-` o `has-` | `.is-active`, `.has-error` |
| Custom Properties (Tema global) | `--theme-{prop}` | `--theme-bg-primary`, `--theme-accent` |
| Custom Properties (Sistema OS) | `--os-{prop}` | `--os-window-shadow`, `--os-panel-height` |

---

## 5. Base de Datos (SQLite)

| Entidad | Regla | Ejemplo |
|---|---|---|
| Tablas | `snake_case` (Plural) | `documents`, `sync_events` |
| Columnas | `snake_case` (Singular) | `file_path`, `added_at` |
| IDs únicos | Prefix semántico | `doc_12345`, `evt_98765` |
