# ADR-015: Continuidad del Stack de Interfaz (Tauri v2 + Svelte 5)

**Fecha:** 2026-10-16
**Estado:** Aceptado

Continúa [ADR-001](001-tech-stack.md), que adoptó Tauri v2 + Rust + TypeScript + Svelte 5 como stack oficial, y
lo confirma con evidencia empírica medida en WebKitGTK.

## Contexto

Al ejercitar la aplicación por primera vez contra el corpus real de 152.080 documentos tras el cierre del desarrollo, la
experiencia observada en Linux fue inaceptable: nueve minutos en blanco antes de aparecer la ventana, bloqueos totales
de la interfaz al cambiar de vista o interactuar con tableros, tirones al desplazar la biblioteca y tiempos de 90 a
233 ms para construir apenas 450 nodos del DOM.

La comparación inmediata con aplicaciones escritas en Qt planteó la pregunta más costosa del proyecto: ¿aguanta el
stack de interfaz de [ADR-001](001-tech-stack.md) lo que el producto necesita, o hay que migrar la interfaz a un
toolkit nativo como Qt antes de seguir construyendo encima?

Para responder con hechos y no con impresiones se hizo una investigación que dejó tres hechos estructurales
establecidos antes de tomar ninguna medida:

1. **Lo que se estaba juzgando no era el motor web, sino dos defectos propios en el hilo principal.**
   * Los 110 comandos del puente IPC carecían del atributo `async`, por lo que Tauri los ejecutaba en el hilo principal
     de la ventana: cada consulta a SQLite congelaba el bucle de eventos gráficos durante segundos.
   * `bootstrap` ejecutaba `integrity-check` sobre el índice FTS5 entero antes de crear la ventana, reteniendo el
     arranque durante nueve minutos sin pintar nada.
   * Ambos defectos habrían congelado de forma idéntica una interfaz en Qt Quick o GTK si las consultas se hubieran
     despachado en el hilo de la GUI.
2. **Tauri no tiene un motor, sino tres.** En Windows usa WebView2 (Chromium), en macOS usa WKWebView (WebKit de Apple)
   y en Linux usa WebKitGTK (2.52.6 con Skia en nuestro entorno de referencia). Toda la evaluación del producto se hizo
   sobre el motor más exigente y menos permisivo de los tres.
3. **Un lector de EPUB lleva un motor web dentro, se elija el toolkit que se elija.** Un EPUB es HTML, CSS, SVG y
   MathML. El componente de texto enriquecido de Qt Quick solo implementa un subconjunto de HTML 4 con CSS 2.1 parcial,
   insuficiente para maquetar libros reales, aplicar temas en variables CSS, hacer lectura biónica o pintar resaltados
   sobre rangos arbitrarios. Por esa razón Calibre empotra Chromium mediante `QtWebEngine`, Foliate usa `WebKitGTK`,
   Thorium usa `Electron` y Readest usa `Tauri v2`.

Corregidos los defectos propios —comandos asíncronos obligatorios, verificación de
integridad fuera del arranque, consultas acotadas, transacciones inmediatas, sistema de
diseño Material 3, miniaturas de portada a 400×600 px, cromo opaco sin `backdrop-filter` sobre lo
que se desplaza y recorrido automático en WebKitGTK—, se ejecutó la medición comparativa de la
página estática y de la aplicación entre Chromium y WebKitGTK.

## Evidencia empírica: WebKitGTK frente a Chromium

Las mediciones se toman en la misma máquina sobre dos escenarios sin intervención de SQLite ni del runtime de Rust,
mediante `scripts/medir-pagina-estatica.mjs` y `scripts/recorrido-webkitgtk.py`.

### 1. Página estática de referencia (cuadrícula de 50 tarjetas, 356 nodos DOM)

Se mide la construcción en frío de 50 tarjetas de biblioteca (356 nodos DOM con contenedor en rejilla CSS, cubierta,
título, autor y metadatos), seguida de disposición síncrona forzada (`offsetHeight`), espera al primer fotograma pintado
(`requestAnimationFrame`) y un segundo de desplazamiento continuo:

| Métrica | Chromium (Playwright) | WebKitGTK 2.52.6 | Observación |
|---|---|---|---|
| Construcción del árbol DOM (`DocumentFragment` + `appendChild`) | 0,7 ms | 1,0 ms | Coste de creación en JavaScript despreciable en ambos motores |
| Disposición síncrona (`offsetHeight`) | 9,1 ms | 28,0 ms | Relación de 3,1x en cálculo inicial de rejilla CSS |
| Primer pintado (`requestAnimationFrame`) | 7,0 ms | 25,0 ms | Composición y subida inicial de teselas |
| **Total en frío hasta primer fotograma** | **16,8 ms** | **54,0 ms** | **3,2x; muy por debajo del umbral perceptible (100 ms) y lejos de los 90–233 ms medidos con el hilo bloqueado** |
| Mediana de fotograma en desplazamiento | 16,7 ms | 17,0 ms | 60 fotogramas por segundo sostenidos en ambos motores |
| Percentil 95 (p95) en desplazamiento | 16,7 ms | 18,0 ms | Dentro del presupuesto de un fotograma (16,7–20 ms) |
| Fotogramas largos (> 20 ms) | 0 | 0 | Sin tirones en ninguno de los dos motores |

El dato que motivó la investigación original —90 a 233 ms para construir 450 nodos— no era el coste de DOM ni de
disposición de WebKitGTK: era la contención del hilo principal provocada por llamadas IPC síncronas y por descodificar
portadas originales a resolución completa. Aislado el motor, construir y pintar por primera vez 50 tarjetas cuesta 54 ms
en WebKitGTK y 16,8 ms en Chromium, y una vez pintadas ambos motores desplazan a 60 fps sin un solo fotograma largo.

### 2. Aplicación compilada estática (`build-e2e`) en las tres vistas principales

Con el árbol real de componentes Svelte 5, las paletas de Material 3 ([ADR-014](014-design-system.md)) y portadas
sintéticas de 400×600 px distintas por obra:

| Vista | Nodos DOM (límite: 1.500) | Píxeles descodificados | Capas `will-change` | Mediana / p95 en Chromium | Mediana / p95 en WebKitGTK |
|---|---|---|---|---|---|
| **Inicio** | 425 | 1,20 Mpx (~4,6 MB, límite 15 Mpx) | 0 | 16,7 ms / 17,1 ms | 17,0 ms / 19,0 ms |
| **Biblioteca** | 219 | 0,96 Mpx (~3,7 MB, límite 20 Mpx) | 0 | 16,7 ms / 16,8 ms | 16,0 ms / 17,0 ms |
| **Lector** (capítulo de 100 párrafos) | 325 | 0,48 Mpx (~1,8 MB, límite 5 Mpx) | 0 | 18,2 ms / 21,2 ms | 17,0 ms / 23,0 ms |

Ambos motores cumplen con holgura los presupuestos deterministas de
[performance-budgets.md](../architecture/performance-budgets.md) y sostienen una mediana de 16 a 18 ms
por fotograma durante el desplazamiento.

## Alternativas consideradas

### A. Mantener Tauri v2 + Rust + Svelte 5 con presupuestos vigilados por guardianes (Adoptada)

Se conserva el stack de [ADR-001](001-tech-stack.md) y el sistema de diseño de [ADR-014](014-design-system.md),
tratando las particularidades de WebKitGTK como restricciones de ingeniería verificadas automáticamente en `pnpm verify`.

### B. Migrar la interfaz a Qt Quick con `QtWebEngine` (Rechazada)

* **A favor:** hilo de renderizado dedicado para el grafo de escena QML, reciclado nativo de delegados (`reuseItems`) y
  caché de texturas asíncrona de fábrica.
* **Motivos de rechazo:**
  1. **No elimina el motor web:** para renderizar EPUB con fidelidad tipográfica, fórmulas y resaltados hace falta
     empotrar `QtWebEngine` (Chromium), igual que hace Calibre.
  2. **Rompe la arquitectura en Rust:** `cxx-qt` no ofrece aún estabilidad de producción en todas sus bibliotecas de
     construcción, lo que obligaría a escribir la capa de interfaz en C++/QML sobre una frontera FFI manual.
  3. **Fricción legal y comercial:** Qt exige cumplir las condiciones de reenlazado dinámico de LGPLv3 o pagar licencias
     comerciales por desarrollador, entrando en tensión directa con el modelo Apache 2.0 de
     [ADR-003](003-licensing-model.md).
  4. **Incumple el presupuesto de tamaño:** empotrar `QtWebEngine` añade el peso de Chromium al paquete, superando el
     límite de 50 MB (nuestro binario completo pesa 22,1 MiB con Wasmtime incluido).
  5. **Destruye todos los guardianes existentes:** `check-ui.mjs`, `check-paquete.mjs`, `recorrido-webkitgtk.py`, los
     contratos de CSS y las más de doscientas pruebas de extremo a extremo en Playwright quedarían invalidados,
     devolviendo los innegociables de interfaz al estado de promesas sin verificación.

### C. Tauri v2 con CEF (Chromium Embedded Framework) en Linux (Rechazada)

* **A favor:** unifica el motor de renderizado de Linux con el de Windows (Chromium), eliminando las diferencias de
  WebKitGTK sin cambiar el código de Svelte 5.
* **Motivos de rechazo:** incrementa el tamaño distribuido en ~170 MB (más de tres veces el presupuesto total de 50 MB),
  carece de integración oficial estable en la rama abierta de Tauri v2 y añade una carga de mantenimiento nativo
  desproporcionada.

### D. Arquitectura híbrida: carcasa nativa (GTK4 o Qt) y superficie de lectura en webview (Rechazada)

* **A favor:** es el camino probado por Foliate; los menús, listas y paneles usan widgets nativos del escritorio.
* **Motivos de rechazo:** obliga a mantener dos sistemas de diseño, dos árboles de foco y accesibilidad, y dos modelos
  de temas simultáneos, mientras que la superficie donde el usuario pasa el 90 % del tiempo —el lector— sigue corriendo
  exactamente sobre el mismo motor web.

### E. Migrar a Slint (Rechazada)

* **A favor:** integración nativa con Rust sin pasar por C++ y consumo de recursos muy reducido.
* **Motivos de rechazo:** Slint no incluye un motor de disposición HTML/CSS capaz de renderizar un EPUB, por lo que
  seguiría necesitando empotrar un webview para la lectura, y sus propios autores mantienen el soporte de escritorio
  avanzado en evolución.

### F. Renderizador propio de documentos sin motor web, al estilo KOReader / crengine / MuPDF (Rechazada)

* **A favor:** independencia total de motores web del sistema y consumo mínimo de memoria.
* **Motivos de rechazo:** el soporte de CSS moderno, tipografía avanzada, MathML, SVG, tablas complejas y resaltados no
  destructivos en esos motores es una fracción del de un motor web actual; adoptarlo obligaría a renunciar a la
  fidelidad editorial del lector o a reimplementar un navegador parcial desde cero.

## Decisión

1. **Tauri v2 + Rust + TypeScript + Svelte 5 se queda como stack definitivo de NB Books**, confirmando
   [ADR-001](001-tech-stack.md) y [ADR-014](014-design-system.md).
2. **Las reglas que mantienen a WebKitGTK en 60 fps son parte del contrato de verificación continua (`pnpm verify`):**
   * Ningún comando IPC bloquea el hilo de la ventana (`scripts/check-comandos.mjs`).
   * Toda colección devuelta por el puente está acotada (`scripts/check-listas.mjs`).
   * Ninguna transacción de escritura se abre en diferido (`scripts/check-transacciones.mjs`).
   * El árbol DOM se mantiene por debajo de 1.500 nodos, con 0 capas `will-change` permanentes y miniaturas acotadas en
     píxeles descodificados (`scripts/recorrido-webkitgtk.py` y `e2e/rendimiento.spec.ts`).
   * El cromo que se apoya sobre contenido desplazable es opaco, sin `backdrop-filter`, y ningún `:hover` desplaza su
     propia caja (`scripts/check-ui.mjs`).
3. **Secuencia de las dos comprobaciones restantes del dossier:**
   * La medición sobre **Windows (WebView2)** se ejecutará como parte de la verificación en máquina Windows después de
     la primera publicación en Linux (`AGENTS.md` sección 9).
   * La construcción de un **prototipo en Qt** queda descartada y solo se reabrirá si una medición futura contradice los
     presupuestos anteriores sin solución dentro del motor web.

## Consecuencias

* **Positivas:**
  * Se preserva íntegra la inversión de ingeniería del desarrollo: el núcleo en Rust, los 190 comandos asíncronos
    del puente IPC, los siete destinos en Material 3 y la batería completa de guardianes estáticos y pruebas E2E.
  * El binario de distribución se mantiene en 22,1 MiB (dentro del presupuesto de 50 MB) y bajo licencia Apache 2.0 sin
    dependencias con obligaciones LGPLv3 ni costes por puesto.
  * La diferencia entre Chromium y WebKitGTK queda cuantificada y vigilada automáticamente por `pnpm check:webkitgtk` y
    `scripts/medir-pagina-estatica.mjs`.
* **Negativas:**
  * El equipo debe seguir diseñando cada componente de interfaz pensando en el techo de composición de WebKitGTK en
    Linux (sin desenfoques sobre áreas móviles, sin listas virtuales desbocadas y sin imágenes sin redimensionar), en
    lugar de asumir la holgura de Chromium.
* **Riesgos:**
  * Una actualización futura de WebKitGTK en distribuciones Linux puede introducir regresiones de renderizado o de
    multimedia ajenas a nuestro código; se mitiga con `scripts/comprobar-el-motor.py` y `scripts/recorrido-webkitgtk.py`
    como diagnóstico reproducible sobre el motor del sistema.
