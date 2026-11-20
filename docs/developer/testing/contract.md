# Contract Testing (Pruebas de Esquema)

> **Estado:** Implementado. Tipos generados en `src/lib/bindings/`, incluido su `index.ts`:
> `pnpm check:bindings` los regenera y falla si el árbol queda distinto, en `pnpm verify` y en el gancho de
> pre-commit.
> El Frontend (TS) y el Backend (Rust) se comunican mediante JSON sobre IPC. Si las firmas divergen, la app falla en runtime.

## 1. El Problema del Puente IPC
TypeScript da seguridad en tiempo de compilación. Rust también. Pero el puente entre ambos (IPC) es un abismo sin tipos. Si Rust cambia el struct `DocumentRecord` renombrando `file_path` a `url`, TypeScript no se entera hasta que explota en ejecución.

## 2. La Solución: Tipos Generados (Typegen)

Para evitar regresiones en la capa IPC, NB Books genera los tipos en lugar de mantenerlos a mano.

### 2.1 `ts-rs` (Rust a TypeScript)
Todos los structs en Rust que cruzan el puente IPC deben derivar `TS`:

```rust
use ts_rs::TS;
use serde::Serialize;

#[derive(Serialize, TS)]
#[ts(export, export_to = "../src/lib/bindings/")]
pub struct DocumentRecord {
    pub id: String,
    pub title: String,
    pub file_path: String,
}
```

Al ejecutar `cargo test`, se generan las interfaces `.ts` correspondientes. El CI ejecuta la generación y falla si el árbol de trabajo queda sucio (`git diff --exit-code`), lo que significa que alguien cambió un tipo sin regenerar los bindings.

### 2.2 Pruebas de Contrato del Event Bus
Para los eventos asíncronos (`AppEventPayload`), se define un JSON Schema versionado en `docs/architecture/api/schemas/`. Las pruebas de contrato en Rust instancian el payload y afirman que el JSON serializado valida contra el esquema de la versión actual.

### 2.3 Contrato del manifiesto de extensiones
`nb-extension.json` tiene un JSON Schema publicado. El CI del registro de extensiones valida cada manifiesto
enviado contra ese esquema antes de aceptar la extensión.

## 3. Compatibilidad hacia atrás

Un cambio en un tipo IPC es un cambio de API. Las reglas de evolución (añadir campos opcionales, nunca renombrar
en sitio, deprecar antes de eliminar) están definidas en
[versioning.md](../../architecture/versioning.md).
