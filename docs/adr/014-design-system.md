# ADR-014: Sistema de Diseño Visual

**Fecha:** 2026-09-10
**Decidido:** 2026-09-09
**Estado:** Aceptado

Continúa [ADR-012](012-ui-paradigm.md), que decidió la **forma** de la interfaz, y
[ADR-013](013-information-architecture.md), que decidió **qué hay en ella**. Este decide **cómo se ve**.

## Contexto

Al usar la aplicación con los 152.080 documentos del corpus real, el diagnóstico de producto sobre el aspecto fue
directo y no se discute: no es fea, pero **se siente como una web, y se nota que la hizo una máquina**. Con ese
aspecto no se comercializa, y el proyecto no se publica hasta que el rincón más alejado esté terminado.

El problema es real y tiene nombre técnico. Lo que hay hoy es un sistema de tokens correcto —los colores viven
solo en `app.css`, hay siete temas, ninguna clase se queda sin regla— y **eso no es un lenguaje visual**. Faltan
las cuatro cosas que separan una aplicación cuidada de una plantilla: escala tipográfica con jerarquía real,
sistema de elevación, capas de estado que se sienten al pulsar y al enfocar, y un vocabulario de forma
coherente. Ninguna se consigue afinando lo que hay; son un sistema completo o no son nada.

### La objeción que hubo que retirar

La primera reacción a adoptar una biblioteca de componentes fue defender las reglas propias del proyecto: el
guardián que prohíbe colores literales, el sistema de siete temas y la política de dependencias. El equipo lo
rechazó, y tenía razón: **son reglas nuestras, y una regla nuestra que impide que el producto alcance su
potencial se cambia, no se defiende.** Este ADR lo asume como criterio y lo deja escrito en la sección de
consecuencias, porque es la primera vez que el proyecto deroga una regla propia a propósito.

Se retira también un dato que se afirmó y era falso: se dijo que Svelte Material UI estaba en alfa para Svelte 5.
No lo está. Va por la **9.0.2**, compila contra Svelte 5.56 y desde la v9 no depende de ningún paquete
`@material`. La afirmación venía de una búsqueda desactualizada sobre la v8 y se corrige aquí.

## Alternativas consideradas

### A. Seguir afinando el sistema propio

**A favor:** cero dependencias, control total, ningún guardián que tocar.
**En contra:** es lo que se ha hecho hasta ahora y produjo el aspecto que motiva este ADR. Construir a mano una
escala tipográfica, un sistema de elevación, capas de estado y un vocabulario de forma coherentes es diseñar un
lenguaje visual completo, que es un oficio entero y no un ajuste.

### B. Svelte Material UI (SMUI), Material 2

Maduro, cuarenta y seis paquetes, 3.443 estrellas, mantenido —último empuje del 2026-08-31— y sin dependencias
de Google desde la v9.

**En contra, y es decisivo:** **no tiene nada de Material 3.** Se buscó en todo su código y hay cero
apariciones. Su linaje es `material-components-web`, **archivado desde enero de 2025**. Adoptarlo sería adoptar
Material 2, que es el aspecto de Android de hace años, no el de un producto de 2026.

### C. m3-svelte, Material 3

La adoptada. Se detalla abajo.

### D. `@material/web`, los componentes de Google para Material 3

Google los dejó **sin equipo en 2024**; el repositorio no está archivado y tiene arreglos reales de agosto de
2026, así que no está muerto. Son componentes web, con un modelo de integración distinto al de Svelte.

**En contra:** el mantenimiento depende de que la comunidad siga apareciendo, y mezclar elementos personalizados
con componentes de Svelte añade una frontera que m3-svelte no tiene. Queda como salida si C falla.

## Decisión

Se adopta **Material 3 como lenguaje visual del producto, implementado con `m3-svelte`.**

### Por qué m3-svelte es suficiente, comprobado componente a componente

| Lo que el producto necesita | Lo que trae |
|---|---|
| Barra de destinos de la carcasa (ADR-013) | `NavigationRail` con elemento y conmutador, y `NavCMLX` para adaptarse al ancho |
| Los nueve grupos de Ajustes | `Switch`, `Slider`, `Select`, `Radio`, `TextField` en cuatro variantes, `Checkbox` |
| Biblioteca y sus filtros | `Card`, `ListItem`, `Chip`, `ChipChooser`, `Divider` |
| Diálogos, menús y avisos | `Dialog`, `Menu`, `MenuItem`, `Snackbar`, `BottomSheet`, `StandardSideSheet` |
| Importación e indexación | `LinearProgress`, `CircularProgress` y sus variantes estimadas y onduladas |
| Tema | Color dinámico desde una semilla, capas de estado, sistema de formas |
| **No trae** | Tooltip, acordeón, tabla de datos, barra superior, autocompletado |

**El argumento no es el tamaño del hueco sino su forma:** trae los componentes **difíciles** —riel de
navegación, campos, selector, deslizador, selectores de fecha y hora, color dinámico— y le faltan los
**fáciles**, de los cuales el proyecto ya tiene varios construidos.

### El riesgo, y por qué se acepta

Un mantenedor principal y 458 estrellas. Lo que lo hace asumible es concreto y verificado: son **componentes
Svelte y CSS sin ninguna dependencia de terceros por debajo**, con licencia **Apache-2.0 o GPL-3.0**, que encaja
con la del proyecto sin fricción. Si el proyecto se abandonara, se bifurca y se mantiene dentro del repositorio.
Esa salida no existe con una dependencia binaria ni con un servicio, y es lo que convierte el riesgo en
asumible en lugar de teórico.

### Lo que este ADR **no** decide

Los componentes no deciden que Inicio tenga presencia. **La arquitectura visual de cada pantalla —jerarquía,
densidad, qué es grande y qué es pequeño— es trabajo de diseño y va en las iteraciones**, no en la elección de
biblioteca. Esperar que una biblioteca de componentes resuelva eso es el error que haría fracasar esta decisión.

## Consecuencias

### Reglas que este ADR deroga o modifica

Se listan **con su mecanismo**, porque una regla derogada en un solo sitio es peor que no tenerla: la
verificación seguiría afirmándola.

| Regla | Dónde vive | Qué pasa con ella |
|---|---|---|
| Ningún componente escribe un color literal | `scripts/check-ui.mjs` | **Se conserva y se adapta.** Material 3 también se tematiza con tokens; lo que cambia es de dónde salen |
| Los colores viven solo en `app.css` | `check-ui.mjs`, ui-conventions | **Se modifica**: el origen pasa a ser el esquema derivado de la semilla, y `app.css` deja de ser la única fuente |
| Ningún emoji ni letra haciendo de icono | `check-ui.mjs` | **Se conserva sin cambios** |
| Ningún `:hover` desplaza la caja de su elemento | `check-ui.mjs`, `e2e/rendimiento.spec.ts` | **Se conserva**: nació de un defecto medido y Material no lo necesita |
| Ningún `:hover` cambia una propiedad que fuerza disposición | `check-ui.mjs` | **Se conserva** por el mismo motivo |
| Nada se anima con la aplicación en reposo | `e2e/rendimiento.spec.ts` | **Se conserva.** Las capas de estado de Material se activan al interactuar, no en reposo |
| Ningún desenfoque en cromo permanente, ningún `will-change` | `check-ui.mjs` | **Se conserva**: son coste por fotograma, no estética |
| Siete temas y alto contraste | `app.css` | **Se conserva como requisito**, reconstruido sobre el color dinámico. El alto contraste y el movimiento reducido son compromisos de accesibilidad y no se pierden |
| Ninguna clase que se pinta se queda sin regla de CSS | `check-ui.mjs` | **Se adapta**: las clases de la biblioteca no son nuestras |

La regla general que sale de aquí, y que gobierna las derogaciones futuras: **una regla que protege al usuario
—consentimiento, redacción de registros, accesibilidad— no se toca sin una razón del mismo peso. Una regla que
protege de un defecto medido se revisa con el dato delante. Una regla estética se cambia cuando el producto lo
pide.** Y cualquiera de las tres se cambia **en su documento y en su guardián a la vez, en el mismo PR**.

### Positivas

* El producto pasa a tener un lenguaje visual completo y reconocible en lugar de un conjunto de tokens.
* Los componentes difíciles dejan de ser trabajo propio.
* El color dinámico da los siete temas con menos código del que hay hoy.

### Negativas

* Una dependencia nueva bajo toda la interfaz, con un mantenedor principal.
* **Los cinco componentes que faltaban:** Se abordaron con la regla R1
  ("ningún componente existe sin una pantalla que lo use"). `Tooltip.svelte` se completó y se usa en los destinos
  plegados de la barra lateral (`NavItem.svelte`) montándose vía portal fuera del contenedor que lo recortaba.
  Los otros cuatro (`TopAppBar`, `DataTable`, `Accordion`, `Autocomplete`) se retiraron formalmente: la barra superior
  la gestiona `AppShell.svelte` con su banner de escritorio; no existen vistas de datos tabulares (las listas usan `.nb-fila`);
  la navegación compleja usa paneles (`Panel.svelte`) y secciones (`Seccion.svelte`) en vez de tarjetas de acordeón;
  y las búsquedas se integran en la cabecera (`Emergente.svelte`) o filtros por fichas (`Chip`), sin necesidad de comboboxes.
  `scripts/check-ui.mjs` previene automáticamente futuros componentes huérfanos.
* **Coste por medir, no estimado:** las capas de estado pintan en cada interacción y el paquete crece. El
  presupuesto son 250 KB y hoy se usan 182: quedan 68. La iteración piloto lo mide en WebKitGTK, que es el motor
  del producto, y no en Chromium.

### Riesgos

* **Que Material se coma la identidad.** Mitigación: la semilla de color, la tipografía y la densidad son
  nuestras; Material aporta la gramática, no la voz. NB Books tiene que seguir pareciéndose a sí mismo.
* **Que el piloto se declare bueno sin medir.** Mitigación: la iteración piloto tiene tres criterios y dos son
  numéricos —fotogramas en el motor real y kilobytes de paquete—.

## Documentos que este ADR obliga a cambiar

| Documento | Cambio |
|---|---|
| [ui-conventions.md](../design/ui-conventions.md) | Las reglas de la tabla de arriba, en el mismo PR que su guardián |
| [ui-components.md](../architecture/ui-components.md) | El catálogo de componentes pasa a apoyarse en la biblioteca |
| [dependency-policy.md](../architecture/dependency-policy.md) | Justificación de coste, licencia y mantenimiento de `m3-svelte` |
