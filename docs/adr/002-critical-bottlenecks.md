# ADR-002: Mitigación de Cuellos de Botella Críticos

**Fecha:** 2026-08-24
**Estado:** Aceptado

## Contexto
Al procesar documentos pesados (ej. PDFs médicos de 2GB o con alta carga gráfica) y sincronizar el estado global en NB Books, identificamos tres cuellos de botella teóricos que podrían romper el "Performance Budget" de 60fps (16ms por frame):
1. Overhead de serialización JSON a través del puente IPC de Tauri.
2. Contención de hilos en SQLite (Base de datos bloqueada - `database is locked`).
3. Consumo desmedido de memoria por parte de PDFium al pre-renderizar múltiples páginas.

## Decisión
Implementaremos un tridente de mitigación a nivel de arquitectura base:
1. **Paginación y Streaming IPC:** Jamás enviaremos documentos enteros ni arrays binarios crudos (Uint8Array gigantes) vía IPC si se puede evitar. Rust escribirá el renderizado en un directorio temporal y devolverá a Svelte *la ruta estática*, delegando la carga pesada al protocolo local `asset://` de Tauri (streaming nativo de disco a GPU).
2. **SQLite WAL Mode & Connection Pooling:** La base de datos local SQLite se inicializará obligatoriamente en modo `PRAGMA journal_mode = WAL` para permitir lecturas concurrentes sin bloquear escrituras, usando `r2d2` o el pool de `rusqlite` en Rust.
3. **Caché LRU para PDFium:** Se implementará un mecanismo Least Recently Used (LRU) en la capa de Rust que mantendrá un máximo de 3 a 5 páginas renderizadas en memoria, destruyendo las páginas más antiguas dinámicamente según el scroll del usuario.

## Consecuencias
- **Positivas:** Reducción drástica del uso de RAM, UI ultra-fluida al desplazar PDFs de miles de páginas.
- **Negativas:** Aumenta la complejidad del backend en Rust. La gestión de caché temporal requerirá limpieza en cierres inesperados (solucionado vía el evento `clean_render_cache`).
