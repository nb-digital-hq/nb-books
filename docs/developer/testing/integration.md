# Integration Testing (Tauri IPC)

> **Estado:** Implementado. Suite: `src-tauri/tests/integracion_ipc.rs`.
> Testear los comandos Tauri (IPC) es el paso intermedio entre las pruebas unitarias y las E2E.

Las pruebas de integración validan que el sistema de rutas y estados inyectados de Tauri funcione correctamente, pero sin la sobrecarga de lanzar una ventana web real.

## Testeando el Backend Tauri (Rust)

A partir de Tauri v2, el framework expone herramientas de mocking nativas.

### Configuración del Cargo.toml

```toml
[dev-dependencies]
tauri = { version = "2", features = ["test"] }
```

### Ejemplo de Prueba de Integración IPC

Se pueden escribir tests en el directorio `src-tauri/tests/` que inicializan un entorno Tauri simulado y prueban directamente la ejecución del comando:

```rust
#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context};
    use crate::{import_book_file, AppState};

    #[tokio::test]
    async fn test_import_ipc_command {
        let app = mock_builder
            .manage(AppState::new_mocked)
            .build(mock_context(tauri::generate_context!("tauri.conf.json")))
            .unwrap;

        let file_path = "tests/fixtures/minimal.pdf".to_string;
        let result = import_book_file(app.handle.clone, file_path).await;

        assert!(result.is_ok);
        assert_eq!(result.unwrap.format, "pdf");
    }
}
```

### Por qué esto es crítico
Esto asegura que las inyecciones de dependencias (como `tauri::State<'_, AppState>`) no fallen en runtime. Si se olvida inyectar el estado en `main.rs`, este test de integración fallará y evitará que el error llegue a producción.

## Casos de integración obligatorios

| Caso | Qué valida |
|---|---|
| Importar un formato no soportado | El comando devuelve `Err` tipado, no un panic |
| Importar un archivo inexistente | Error `FILE_NOT_FOUND`, sin efectos secundarios en SQLite |
| Ruta fuera del alcance permitido | El comando rechaza la ruta (defensa contra path traversal desde el webview) |
| Doble importación del mismo archivo | Idempotencia: no se crean dos registros para el mismo hash |
| `delete_library_book` de un id inexistente | Error controlado, sin corromper la tabla |

## Lo que la suite deja detrás, medido el 2026-09-08

Una pasada completa de `cargo test` deja **647 MB** repartidos en directorios `/tmp/nb-*`, y **no se limpian
solos**: cada ayudante de prueba compone su ruta con el identificador del proceso, así que la limpieza que hace
al empezar solo alcanza a una ejecución suya anterior con el mismo número, que casi nunca existe. Sesenta y
tantas ejecuciones habían acumulado **9,2 GB**.

**No es un problema teórico.** Ese día `/tmp` —un `tmpfs` de 9,7 GB— llegó al 97 % y el enlazador murió con un
`SIGBUS` al compilar una prueba de documentación. El fallo no señalaba a ningún código: decía
`ld terminated with signal 7`, que es lo que pasa cuando se escribe en un `tmpfs` sin sitio. Diagnosticarlo costó
más que arreglarlo.

**El arreglo pendiente, y por qué no se hizo aquí.** Lo que hace falta es una guarda con `Drop` compartida —una
sola, en `tests/comun/`— que borre el directorio al terminar la prueba, pase o falle, porque el desenrollado de
un `panic!` sí ejecuta destructores. Son unos cuarenta ayudantes casi idénticos, y cambiarlos es mecánico pero no
trivial: el tipo tendría que dejarse usar donde hoy se usa una ruta. Se declara en lugar de dejarlo implícito, y
mientras tanto la limpieza es manual: borrar `/tmp/nb-*` cuando no haya pruebas corriendo.
