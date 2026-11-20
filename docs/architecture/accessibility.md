# Accesibilidad (A11y) en Escritorio

> **Estado:** Implementado: puntos de referencia, preferencias, navegación completa por teclado, auditoría
> automática por componente y por vista, y lista de comprobación manual con lector de pantalla. Alineado con
> [ADR-012](../adr/012-ui-paradigm.md).
> NB Books debe ser navegable y comprensible por personas con
> capacidades diferentes, conforme a WCAG 2.1 AA.

## 1. El desafío del webview

NB Books no es nativo (Qt/GTK): corre en un webview (WebKitGTK en Linux, WebView2 en Windows, WKWebView en
macOS). Los lectores de pantalla del sistema (Orca, NVDA, VoiceOver) ven la aplicación como un documento web.

Esa es la razón de la decisión estructural de ADR-012: **si la interfaz usa la semántica que el navegador ya
tiene, el lector de pantalla ya sabe leerla**. Toda desviación de esa semántica hay que pagarla con emulación,
y la emulación de accesibilidad es la que peor envejece.

## 2. Puntos de referencia y roles

La carcasa de [ui-components.md](ui-components.md) se corresponde uno a uno con puntos de referencia
estándar. Esto no es una recomendación de estilo: es el mecanismo de accesibilidad.

| Zona | Rol | Etiquetado |
|---|---|---|
| Barra superior | `banner` | Implícito en `<header>` de primer nivel |
| Navegación primaria | `navigation` | `<nav aria-label="...">`, traducida |
| Área principal | `main` | Uno solo por vista |
| Panel lateral | `complementary` | `<aside aria-labelledby="...">` apuntando a su título |
| Documentos abiertos | `tablist`/`tab`/`tabpanel` | Patrón de pestañas del APG, con flechas |

Reglas que se derivan y no se negocian:

* **`dialog` solo para lo modal de verdad.** Un panel lateral no es un diálogo y no atrapa el foco. Atrapar el
  foco en algo que el usuario percibe como no modal es una de las formas más frecuentes de dejar encerrado a
  quien navega por teclado.
* **`role="application"` no se usa sobre contenedores de la carcasa.** Desactiva el modo de navegación del
  lector de pantalla. Solo se admite dentro de un componente concreto que gestione su propio teclado y ofrezca
  una salida documentada.
* **El orden visual coincide siempre con el orden del DOM.** Cuando divergen, el teclado y el lector de pantalla
  recorren la interfaz en un orden distinto del que se ve, y la interfaz deja de ser usable sin ratón.

## 3. El lector de documentos

* **EPUB:** la estructura HTML interna del libro se inyecta saneada por lista de permitidos y conserva su
  semántica, así que el lector de pantalla la recorre de forma natural. La tipografía es del lector, no del
  documento, y no puede alterar el anclaje.
* **PDF:** las páginas se renderizan como imagen mediante PDFium, y una imagen es **ciega** para un lector de
  pantalla. Por eso el diseño extrae la capa de texto con sus coordenadas normalizadas la superpone: el
  texto real queda disponible para Orca, NVDA y VoiceOver, y para la selección y la anotación. **Un documento sin
  capa de texto se declara como tal**, con su motivo, en lugar de aparentar ser accesible.
* **Cómic:** es una secuencia de imágenes sin texto extraíble. Se declara igual, y se apoya en los metadatos.

## 4. Navegación por teclado

Ninguna acción es exclusiva del ratón, **con una excepción declarada** que se explica al final de esta sección.
Es criterio de aceptación y, no una aspiración.

* Todo elemento interactivo responde a `:focus-visible`, con indicador que cumple el contraste requerido.
* El desplazamiento entre zonas usa la **convención de plataforma para ciclar regiones**, no un Alt+Tab
  emulado. Emular la conmutación de ventanas del sistema colisiona con el sistema.
* **La vía universal son los atajos y el orden de tabulación**, y las dos se comprueban: un registro donde toda
  acción declara su tecla, y una prueba que ejecuta el flujo entero como una lista de pulsaciones.
* **No hay paleta de comandos.** Con las acciones visibles donde se
  usan y su atajo escrito al lado, una segunda vía para invocarlas era superficie que mantener sin nada a
  cambio. La accesibilidad por teclado no dependía de ella y sigue sin depender.
* Los atajos **son visibles**: cada control lleva el suyo en su nombre accesible. Están documentados para el
  usuario en [docs/user/shortcuts.md](../user/shortcuts.md).
* **Ninguna acción es exclusiva del ratón, y hay mecanismo**: `check-ui.mjs` rechaza los gestos de puntero sin
  equivalente de teclado. El compilador de Svelte no los detecta, porque un `div` con `role` y `tabindex` le
  parece correcto.
* **Una tecla hace una cosa.** El lector escucha en `window`, así que un panel que quiera quedarse una tecla
  tiene que **detener la propagación**; y "se está escribiendo" se decide mirando el foco real, no si este
  lector concreto tiene una nota abierta. Lo segundo lo aprendimos: teclear dentro del panel de
  anotaciones disparaba los atajos del lector, porque cada uno creía que el único campo era el suyo.
* **Un panel que se abre con una tecla se lleva el foco, y lo devuelve al cerrarse.** Dejarlo donde estaba
  obliga a tabular un número desconocido de veces para llegar a lo que se acaba de abrir. No es atrapar el
  foco: se sale con el tabulador y con Escape. Y una acción que **destruye** el elemento enfocado —guardar lo
  que se escribía, confirmar un borrado— tiene que devolver el foco a algo, o cae en `body` y la siguiente
  tecla no hace nada.
* **Las listas largas usan tabulación móvil**: la lista entera es una parada de tabulación y las flechas mueven
  dentro. Es lo que hace que "orden de tabulación predecible" signifique algo cuando hay cuarenta elementos con
  tres acciones cada uno.

### Seleccionar texto sin ratón

Es el punto donde esto se juega de verdad, y merece decirse aquí: **no hay navegación por cursor de texto
garantizada en un webview**. WebKitGTK, WebView2 y WKWebView no coinciden, así que una selección que dependiera
de ella dejaría el criterio cumplido en una plataforma e incumplido en otra sin que nadie lo notase.

La selección es por **unidades del documento** —párrafos en el marcado, fragmentos de la capa de texto en lo que
se pinta como imagen—, con **una sola parada de tabulación** para el texto entero: la del bloque donde está el
cursor. Las dos alternativas son peores, y las dos se probaron. Con todos los párrafos en el recorrido, pasar al
siguiente control cuesta cien pulsaciones. Con ninguno, no se puede entrar al texto con el tabulador —hay que
saber que se entra con la flecha abajo, y eso no lo anuncia nadie—, y además la zona que desplaza se queda sin
acceso por teclado, que es una violación seria de WCAG 2.1.1: la encontró la auditoría en modo
inmersivo, donde no queda ningún otro control dentro.

La prosa conserva su semántica, así que el lector de pantalla la sigue leyendo como prosa, y no hace falta
`role="application"`, que ADR-012 prohíbe sobre la carcasa.

### 4.1 La única excepción: el trazo libre

> Añadida, y se declara en lugar de disimularse.

**Dibujar a mano alzada no tiene equivalente de teclado, y no lo va a tener.** Un trazo es una forma continua que
sale del movimiento de una mano; no hay secuencia de teclas que la produzca sin convertirla en otra cosa —una
figura elegida de una lista ya no es un trazo libre—. Fingir lo contrario con un atajo que dibuje algo
predeterminado sería cumplir la letra de la regla incumpliendo su motivo.

Lo que sí se sostiene es la razón de la regla, que no es que cada gesto tenga su tecla sino que **ninguna
capacidad quede fuera del alcance de quien no usa ratón**. Lo que un trazo comunica —señalar un pasaje, destacarlo,
dejar constancia de que importa— se alcanza con **subrayado, resaltado y nota**, las tres con su atajo y las tres
comprobadas en la prueba que ejecuta el recorrido entero como una lista de pulsaciones. Un trazo es otra forma de
decir lo mismo, no la única.

Lo que sí tiene teclado es **salir**: con el lápiz apoyado, Escape cancela el trazo a medias, para que un apoyo
sin querer no obligue a levantar y borrar después.

`check-ui.mjs` sigue exigiendo un manejador de teclado en el mismo elemento, y por eso lo hay. Que exista no
significa que haga lo mismo que el gesto, y el propio guardián lo dice en su comentario: lo que descarta es no
haber puesto ninguna, no demuestra equivalencia. Aquí la equivalencia **no existe** y queda escrito.

## 5. Preferencias de accesibilidad

Viven en el plano de aplicación de la personalización y persisten en SQLite por
[ADR-010](../adr/010-settings-persistence.md), que existe precisamente porque **las
preferencias de accesibilidad son las que más importa preservar y las que más se perdían** en el
almacenamiento del webview.

* Alto contraste, que además es uno de los modos de lectura del catálogo.
* Movimiento reducido, honrando también `prefers-reduced-motion` del sistema.
* Escala tipográfica de la interfaz, independiente de la del libro.
* Lectura en voz alta, sobre EPUB y sobre PDF con capa de texto. La voz del sistema es el **suelo**:
  no depende de que el equipo aguante un modelo ni de que exista una voz neuronal en el idioma del documento, y
  no se sustituye nunca. Cuando la plataforma no la tiene —WebKitGTK no siempre la trae—, **se dice, con su
  motivo**: "no hay síntesis aquí" y "no hay ninguna voz instalada" son dos situaciones distintas, y la segunda
  se arregla instalando una. El contrato de proveedor está en
  [reader-contracts.md](reader-contracts.md), sección 5.4.

## 6. Verificación

| Nivel | Qué comprueba | Estado |
|---|---|---|
| Automática por componente | Avisos de accesibilidad del compilador de Svelte, bloqueantes, más las reglas de ADR-012 en `scripts/check-ui.mjs` | Implementado |
| Medida sobre la paleta | El contraste de los cinco temas se calcula sobre `app.css`, no se afirma | Implementado |
| Automática por vista | axe sobre las vistas, los paneles, el alto contraste y la lectura inmersiva. Cero violaciones serias o críticas; su aparición bloquea. Corre en `pnpm verify` | Implementado |
| Manual con lector de pantalla | Importar, abrir, leer y anotar con el lector de referencia de cada sistema. La lista y su listón están en [testing/screen-readers.md](../developer/testing/screen-readers.md) | Manual, en cada publicación |
| Del flujo por teclado, sin navegador | El flujo de lectura y anotación como una lista de pulsaciones sobre la máquina pura, y la retirada de la carcasa sobre el marcado que produce `svelte/server` | Implementado |
| Del panel de anotaciones, sin navegador | Recorrer, saltar, editar y borrar como una lista de pulsaciones, y el orden de tabulación sobre el marcado | Implementado |
| Del flujo por teclado, en un navegador real | Las mismas secuencias con eventos de teclado de verdad sobre Chromium, que es lo único que comprueba que una tecla no hace dos cosas | Implementado |

La comprobación automática detecta una fracción de los problemas reales. Por eso la pasada manual existe: no
se declara conformidad WCAG apoyándose solo en un analizador.
