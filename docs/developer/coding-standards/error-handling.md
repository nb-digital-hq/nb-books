# Error Handling

> **Estado:** Vigente. Ningún error viaja como cadena sin tipar.

El manejo de errores en NB Books es uno de nuestros pilares de confiabilidad.

## En Rust (Backend)
- **JAMÁS usar `unwrap` o `expect`** a menos que:
  1. Estés en un bloque de código de prueba (`#[test]`).
  2. Sea matemáticamente imposible que falle y hayas documentado el porqué justo encima de la línea.
- Usa el operador `?` para propagar errores.
- Transforma los errores internos (`std::io::Error`, `rusqlite::Error`) a un tipo de error de dominio (ej. `NbError`) antes de enviarlos por el puente IPC hacia el frontend para no exponer detalles de la infraestructura.

## En TypeScript / Svelte (Frontend)
- Utiliza bloques `try/catch` para todas las llamadas a la API de Tauri (Tauri `invoke`).
- Los errores recuperables deben mostrar un feedback visual no intrusivo (Notificaciones).
- Los errores críticos (ej. fallo al inicializar DB) deben mostrar un Modal de Error bloqueante con un código de error que el usuario pueda copiar.
