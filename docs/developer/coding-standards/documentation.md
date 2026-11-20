# Estándares de Documentación de Código

> **Estado:** Vigente.
> "El código se lee diez veces más de lo que se escribe."

No todo el código necesita comentarios. Si tienes que explicar *qué* hace una variable, probablemente su nombre está mal (`let d = 5; // dias` -> `let days = 5;`).
Los comentarios deben explicar el **POR QUÉ** (decisiones de diseño, workarounds de librerías, invariantes).

## 1. Rust (Backend)

En Rust, la documentación no es un agregado opcional, es parte del ecosistema (`cargo doc`).

### Reglas:
1. **Módulos (`mod.rs` o similares):** Todo módulo principal debe tener un comentario a nivel de módulo (`//!`) explicando su propósito arquitectónico.
2. **API Pública (`pub`):** Todas las funciones públicas, structs y traits deben tener un comentario doc (`///`).
3. **Ejemplos:** Si un Trait es complejo de usar, debe incluir un bloque de código markdown dentro del `///` que `cargo test` validará automáticamente (Doc Tests).
4. **`Panics` y `Errors`:** Si una función devuelve un `Result`, el doc comment debe tener una sección `# Errors`. Si puede hacer panic, debe tener una sección `# Panics`.

**Ejemplo de Función Rust:**
```rust
/// Renders a specific page of a PDF document to a temporary JPEG file.
///
/// # Errors
///
/// Returns `NBError::PdfEngineFailure` if PDFium fails to load the document.
/// Returns `NBError::FileNotFound` if the target path does not exist.
#[tauri::command]
pub async fn render_pdf_page(doc_path: String, page: u16) -> Result<String, NBError> {
    // ...
}
```

---

## 2. TypeScript / Svelte (Frontend)

Para el frontend usamos **JSDoc/TSDoc** estándar, pero con un enfoque pragmático.

### Reglas:
1. **Interfaces/Types exportados:** Deben estar documentados (`/** ... */`).
2. **Funciones complejas de Store:** Deben explicar qué mutan.
3. **Componentes Svelte Complejos:** Usar el bloque `<!-- @component -->` para describir qué Props recibe y su propósito visual.

**Ejemplo de Componente Svelte:**
```html
<!--
  @component
  El componente `LibraryGrid` renderiza la vista de cuadrícula del catálogo.
  Se suscribe internamente a `catalogStore` para mantener la reactividad.

  @prop isSelectable - Habilita el modo de selección múltiple. Default: false.
-->
<script lang="ts">
  let { isSelectable = false } = $props;
</script>
```

---

## 3. Comentarios de Marcador (TODOs)

Se prohíbe el uso de `TODO` sueltos en el código base, ya que se convierten en deudas técnicas olvidadas.

**Regla:** Todo marcador debe tener un contexto y una fecha o issue asignado.
```typescript
// Incorrecto:
// TODO: arreglar este hack después

// Correcto:
// TODO(performance, 2026-09): Reemplazar este forEach por reduce para evitar O(n^2).
// FIXME(#142): Tauri WebView2 pierde el foco en Windows al abrir este modal.
```

## 4. Relación con la documentación de `docs/`

La documentación de código describe **cómo funciona una unidad**. La carpeta `docs/` describe **por qué existe el
sistema y cómo encajan las piezas**. Un cambio con impacto arquitectónico no está terminado hasta que ambas
capas reflejan la realidad (Constitución, regla 5).

> **Nota histórica:** la documentación del proyecto se generó en su origen con scripts que escribían el mismo
> contenido en varias rutas, lo que corrompió el 27 % del corpus. Esos scripts se eliminaron el 2026-08-25. La
> documentación se escribe a mano; lo que se genera automáticamente es la referencia extraída del código
> (`cargo doc`) y el archivo de avisos de terceros.
