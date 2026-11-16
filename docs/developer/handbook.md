# Engineering Handbook

> **Estado:** Vigente como norma de ingeniería. Varias de las herramientas que menciona todavía no están configuradas.
**NB Books - Engineering Culture & Standards**

Este handbook define la cultura, los principios y las expectativas técnicas para cualquier ingeniero que contribuya a NB Books. Inspirado en los handbooks de ingeniería de misiones críticas (ej. Apple, Google), este documento no negocia la calidad.

## 1. Nuestra Filosofía de Ingeniería

### 1.1 Offline-First Extremo
No asumimos conectividad intermitente; asumimos **cero conectividad**. La aplicación debe iniciar, leer, buscar y escribir anotaciones en un entorno completamente aislado (Air-Gapped). La nube es un accesorio de sincronización, no una dependencia funcional.
- **Prohibido**: Bloquear la UI esperando una respuesta de red.
- **Prohibido**: Almacenar estado crítico exclusivamente en memoria volatil sin un respaldo en SQLite.

### 1.2 "Symbiotic Stack" (Rust + Svelte)
Usamos Rust para la seguridad de memoria, concurrencia de bajo nivel y rendimiento pesado (PDFium, SQLite, FTS5). Usamos Svelte 5 para una reactividad quirúrgica y fluida sin el overhead de un Virtual DOM.
- **La Regla de la Frontera**: El IPC (Inter-Process Communication) a través de Tauri es nuestro cuello de botella. Nunca envíes arrays gigantes de objetos por IPC. Usa Rust para filtrar y paginar; envía solo lo que la vista necesita renderizar.

## 2. Calidad de Código y Revisión

- **Cero Warnings**: El código no se fusiona (merge) si el compilador de Rust o el linter de TypeScript emiten advertencias.
- **Propiedad del Rendimiento**: Si introduces una regresión que aumenta el tiempo de renderizado de una página PDF en más de 16ms (rompiendo los 60fps), el PR será revertido automáticamente.
- **Defensive Programming**: En Rust, evita `unwrap` y `expect` a menos que estés demostrando un invariante matemático. Propaga errores con `?` y manéjalos elegantemente en el frontend.

## 3. Arquitectura del Estado (State Management)
Usamos las Runes de Svelte 5 (`$state`, `$derived`, `$effect`).
- El estado global reside en *stores* centralizados (ej. `workspace`, `catalog`).
- Los componentes solo deben recibir o consumir el estado que necesitan para evitar re-renderizados en cascada.
- **Cero Estado Huérfano**: Todo estado que deba sobrevivir a un reinicio debe persistirse a través del Event Bus hacia SQLite.

## 4. Seguridad por Diseño (Security Posture)
- **Zero Trust**: Trata cada archivo PDF importado como un vector de ataque potencial. El parseo ocurre estrictamente en el backend de Rust (PDFium) bajo sandboxing.
- **Sanitización**: La UI renderiza SVG e imágenes provenientes de Rust; cualquier metadato de texto debe escapar HTML preventivamente.

## 5. Telemetría y Observabilidad Local
Dado que no enviamos datos de usuarios a servidores de terceros, la observabilidad es local.
- Escribir logs estructurados (tracing en Rust) en la carpeta de AppData del usuario.
- Esto permite que el usuario exporte un archivo `.zip` de diagnóstico si desea reportar un bug.
