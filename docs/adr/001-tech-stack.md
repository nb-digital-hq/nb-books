# ADR-001: Selección de Stack Tecnológico (Core y UI)

**Fecha:** 2026-08-23
**Actualizado:** 2026-08-24
**Estado:** Aceptado

## Contexto
NB Books requiere un stack tecnológico que permita cumplir con una exigente lista de requisitos: procesamiento intensivo local (parsing de PDFs/EPUBs, embeddings de IA, indexación en SQLite), bajo consumo de recursos, funcionamiento multiplataforma (Windows, Linux, macOS), seguridad de memoria y un ecosistema moderno para la Interfaz de Usuario (UI).

## Alternativas Consideradas
1. **Flutter (Dart puro):** Excelente para consistencia visual, pero con limitaciones de rendimiento en bajo nivel para cargas masivas (ej. integración con `llama.cpp` o procesamiento pesado de metadatos locales sin recurrir a FFI complejo).
2. **C++ con Qt/GTK:** Máximo rendimiento nativo, pero ciclos de desarrollo excesivamente lentos y alta fricción para lograr interfaces modernas de vanguardia.
3. **Tauri v2 + Rust (backend) + TypeScript/React (frontend):** Combina el máximo rendimiento nativo en el core con la flexibilidad de la web, pero React introduce el coste de un DOM virtual innecesario para una aplicación de escritorio con reactividad quirúrgica.
4. **Tauri v2 + Rust (backend) + TypeScript/Svelte 5 (frontend):** Mismas ventajas que la opción 3, con reactividad granular compilada, menor tamaño de bundle y un modelo de estado más predecible mediante runes.

## Decisión
Se adopta la **opción 4 (Tauri v2 + Rust + TypeScript/Svelte 5)** como stack oficial de NB Books.

* **Core / Runtime:** `Rust`. Garantiza seguridad de memoria, control absoluto del I/O local y rendimiento para la base de datos y la inferencia local.
* **UI:** `Tauri v2` + `TypeScript` + `Svelte 5` con el sistema de runes (`$state`, `$derived`, `$effect`). Svelte 5 compila la reactividad a código imperativo mínimo, eliminando el coste del DOM virtual y produciendo bundles muy ligeros.
* **Toolkit de UI:** `SvelteKit` con adaptador estático, operando como aplicación de página única dentro del webview de Tauri.
* **Gestor de paquetes:** `pnpm`, por su eficiencia en disco y velocidad de resolución.

### Por qué Svelte 5 y no React
1. **Sin DOM virtual.** NB Books implementa un paradigma de escritorio virtual con decenas de componentes interactivos simultáneos. La reactividad granular minimiza el trabajo de renderizado en ese escenario.
2. **Estado nativo.** Los runes eliminan la necesidad de bibliotecas externas de gestión de estado, reduciendo la superficie de dependencias.
3. **Tamaño de bundle.** En una aplicación Tauri, cada kilobyte del frontend se embebe en el binario.
4. **Modelo mental alineado.** Los archivos `.svelte` combinan marcado, lógica y estilos de forma autocontenida.

### Riesgo asumido
El ecosistema de componentes de terceros de Svelte es menor que el de React, y Svelte 5 es reciente. Se acepta a cambio de las ventajas anteriores y porque el proyecto construye sus propios componentes de interfaz.

## Mandato Estricto de UX: "Native Feel"
Aunque la tecnología de presentación se basa en tecnologías web (WebView), **queda estrictamente prohibido que la interfaz se "sienta" como una página web.**
La UI debe simular la respuesta, solidez y simpleza de una aplicación nativa pura (al estilo Qt, GTK o Swift). Esto implica:
* Deshabilitar la selección de texto accidental en elementos de la interfaz.
* Eliminar los menús contextuales genéricos del navegador (click derecho).
* Manejar el enrutamiento sin recargas (Single Page Application pura).
* Soportar atajos de teclado del sistema operativo (Ctrl+S, Cmd+, etc).
* Prevenir comportamientos web clásicos como "arrastrar imágenes fantasma" o "spinners genéricos de navegador".
* Respetar las guías de diseño de los sistemas operativos (sombras, bordes, animaciones de ventana).

## Consecuencias

* **Positivas:** binarios ligeros, consumo de memoria muy inferior al de una solución basada en Electron, acceso de bajo nivel sin restricciones, reactividad compilada sin coste de DOM virtual y gestión de estado sin dependencias externas.
* **Negativas:** la barrera de entrada al proyecto es más alta por Rust. El ecosistema de Svelte es más pequeño que el de React en componentes de terceros. El equipo debe mantener la coherencia entre el estado de Rust y el del frontend a través del puente IPC.
* **Riesgos:** la dependencia del motor de webview del sistema implica diferencias de comportamiento entre plataformas que el proyecto no controla. Ver [deployment](../developer/deployment/README.md).
