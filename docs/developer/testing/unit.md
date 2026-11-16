# Unit Testing

> **Estado:** Implementado. A 2026-08-27: **195 pruebas unitarias** en Rust y
> **483 en total** contando integración, caos, humo y doctest, repartidas en 36 conjuntos. El frontend tiene
> 151. **Sigue faltando la medición de cobertura**, que depende de la integración continua y por eso la implementación
> continúa como parcial: el número de pruebas no es cobertura, y confundirlos sería exactamente la clase de
> afirmación sin mecanismo que este proyecto no se permite.

Las pruebas unitarias aíslan la lógica más pequeña. Tienen que ejecutarse en milisegundos para permitir un flujo de trabajo fluido.

## 1. Unit Tests del Backend (Rust)

Usamos el framework integrado `cargo test`.
Casi toda la lógica pesada debe abstraerse fuera de los comandos de Tauri (`lib.rs`) hacia módulos independientes puros.

### 1.1 Testeando SQLite (`db.rs`)
Para probar la base de datos sin contaminar el disco local del desarrollador, se debe usar la base de datos en memoria o bases temporales.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_memory_db -> Connection {
        let conn = Connection::open_in_memory.unwrap;
        init_db(&conn).unwrap;
        conn
    }

    #[test]
    fn test_import_book {
        let conn = setup_memory_db;
        // Lógica de prueba...
    }
}
```

### 1.2 Mocks de PDFium
No queremos que los Unit Tests dependan del binario dinámico `libpdfium.so`. Para testear el trait
`FormatProvider` o el motor de PDF se usarán implementaciones falsas de las dependencias pesadas.

> **Pendiente.** Nada de esto existe todavía: no hay motor de PDF ni dependencia de `mockall`, y añadirla
> requiere pasar por la [política de dependencias](../../architecture/dependency-policy.md). Se decide en la
> la decisión correspondiente, cuando exista el primer proveedor de formato.

### 1.3 Corpus de archivos de prueba
El repositorio mantiene `src-tauri/tests/fixtures/` con ocho documentos mínimos y **libres de derechos**
(dominio público o generados sintéticamente): un PDF válido, un PDF truncado, un EPUB válido, un EPUB con XML
malformado, un EPUB con entidad externa, un CBZ con rutas de escape, un archivo con extensión falsificada y un
documento con instrucciones adversarias dirigidas a un modelo. Su procedencia está documentada en el propio
directorio. Está prohibido añadir material con copyright vigente al repositorio.

---

## 2. Unit Tests del Frontend (Svelte/TypeScript)

Usamos **Vitest** en conjunto con **Svelte Testing Library**.

### 2.1 Testeando la lógica pura (Runes / Stores)
Cualquier archivo `.ts` o `.svelte.ts` puro debe probarse importándolo directamente en Node (a través de Vitest) sin montar componentes.

```typescript
// test/stores/catalog.test.ts
import { expect, test } from 'vitest';
import { catalogStore } from '$lib/stores/catalog.svelte';

test('El catálogo debe inicializarse vacío',  => {
    expect(catalogStore.items.length).toBe(0);
});
```

### 2.2 Testeando Componentes Visuales
Si un componente Svelte es presentacional, se debe probar que renderiza correctamente los props. No se deben hacer pruebas unitarias en componentes que realicen llamadas directas a Tauri IPC (esos van a integración o E2E).

```typescript
// test/components/BookCover.test.ts
import { render } from '@testing-library/svelte';
import BookCover from '$lib/components/BookCover.svelte';

test('Muestra la imagen por defecto si no hay cover',  => {
  const { getByRole } = render(BookCover, { props: { coverUrl: null } });
  expect(getByRole('img').src).toContain('default-cover.png');
});
```
