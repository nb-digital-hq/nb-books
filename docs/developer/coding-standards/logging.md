# Guía de Logging y Observabilidad

> **Estado:** Implementado. Registro en `src-tauri/src/telemetry.rs`, con `tracing`, escritura rotada por día y
> redacción probada; paquete de diagnóstico en `src-tauri/src/diagnostics.rs`.

El sistema de logs en un entorno local-first (donde no tenemos acceso directo al PC del usuario) es la única herramienta para depurar errores en producción.

## 1. El Backend (Rust)

Usamos el ecosistema `tracing` (no `log`). Es asíncrono y permite logs estructurados.

### 1.1 Niveles de Log
Se prohíbe usar logs de alto nivel para ruido.
* `tracing::error!`: Crítico. La app falló una operación principal (ej. SQLite corrupto). El usuario vio un error visible.
* `tracing::warn!`: Degradación. Algo falló pero el sistema se recuperó (ej. falló la red al proveedor de metadatos, se usa metadata offline).
* `tracing::info!`: Hitos del sistema. Arranque completo, inicialización de PDFium, fin de una importación. (No usar en bucles de renderizado).
* `tracing::debug!`: Información para desarrolladores (ej. tiempos de ejecución, payloads de IPC entrantes). Se activa con `NB_BOOKS_LOG=debug`.
* `tracing::trace!`: Nivel máximo. Dump de bytes o iteraciones profundas.

### 1.2 Logs Estructurados
No concatenes strings. Usa los pares clave-valor de `tracing` para permitir análisis posterior:

```rust
// Incorrecto
tracing::error!("Fallo al abrir el archivo: {} en la ruta {}", err, path);

// Correcto
tracing::error!(
    error = %err,
    path_hash = %hash_path(&path),
    "Fallo al intentar abrir el archivo de documento"
);
```

### 1.3 Dónde se escribe
Los registros se escriben en el directorio de registros de la aplicación como `nb-books.log`, rotado por día
(`tracing-appender`). El nivel se controla con `NB_BOOKS_LOG`, con la sintaxis de filtros de `tracing`; por
defecto, `info`.

---

## 2. El frontend (Svelte / TypeScript)

El frontend no tiene registro propio que persista. `console.log` es un aviso de Biome (`noConsole`, que solo
admite `warn` y `error`), y `e2e/flujo.spec.ts` falla si cualquier recorrido de la interfaz produce un error de
consola. Los errores que importan ocurren en el runtime y se registran allí, con su código canónico.

---

## 3. PII y Privacidad Estricta

**Constitución, regla 3 (Privacidad por Diseño):** el libro que lee el usuario es información altamente confidencial.

<!-- check-docs: allow-absolute-path -->
* **Prohibido** loguear el título del libro, nombres de autor, o rutas completas que revelen el nombre de
  usuario del sistema operativo (ej. `C:\Users\JohnDoe\...`). La ruta anterior aparece aquí como ejemplo de
  lo prohibido; el verificador de documentación la permite gracias al marcador de la línea anterior.
* Los logs **solo** pueden contener IDs generados (ej. `doc_789456`), hashes, y descripciones técnicas.

Si un error salta al intentar importar un archivo, el log debe decir:
`Import failed for format EPUB. Size: 1.2MB. Error: Malformed XML root.`

**Mecanismo.** `scripts/check-registro.mjs`, en `pnpm verify` y en el gancho de
pre-commit, lee cada `trace!`, `debug!`, `info!`, `warn!`, `error!` y `event!` del runtime campo a campo: un campo
cuyo nombre o cuyo valor nombra algo que suele ser del usuario -una ruta, un título, un autor, una consulta, un
texto, una URL- tiene que pasar por `telemetry::redact_path` o `redact_text`, registrar su medida (`.len`) o
declarar dentro de la propia invocación, con `// no es del usuario: <motivo>`, por qué no lo es.
**Y la segunda barrera:** lo que el guardián no puede ver -el contenido de un error registrado con
`error = %error`, o un valor del usuario guardado en una variable cuyo nombre no lo delata- se redacta al escribirse.
Todo lo que llega al archivo pasa por `telemetry::redactar_linea`: la carpeta personal y el nombre de la cuenta se
quitan, y cualquier palabra con forma de ruta se sustituye por su forma redactada (extensión y longitud). Las
direcciones web se conservan: son de servicios, no del usuario.

### 3.1 El paquete de diagnóstico
Ajustes ofrece un paquete de diagnóstico (`diagnostics_preview` y `save_diagnostics`) con la versión, el esquema,
el estado de los componentes y las últimas líneas del registro. Antes de entregarlo se comprueba **otra vez**,
sobre el texto ya montado, que no lleva rutas de usuario, títulos ni secretos. El usuario **ve el contenido
antes de guardarlo**, y lo guarda donde elige: la aplicación no lo envía a ningún sitio.
