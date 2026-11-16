# Verificación con Lectores de Pantalla

> **Estado:** Aplicable a cada publicación.
> Complementa [architecture/accessibility.md](../../architecture/accessibility.md), que cubre el cómo técnico, y
> [legal/accessibility-compliance.md](../../legal/accessibility-compliance.md), que cubre qué exige la norma.

## 1. Por qué esto es manual, y por qué no se puede saltar

La comprobación automatizada detecta una fracción de los problemas reales. Encuentra un botón sin nombre; no
encuentra que ese nombre sea inútil, que el orden en que se anuncian tres cosas sea incomprensible, o que la
aplicación anuncie un cambio tan a menudo que quien la usa acabe apagando los avisos.

**Ninguna herramienta puede sustituir esto**, y por eso el criterio de aceptación no es un número: es
que una persona complete el recorrido con el lector de pantalla de su sistema. `docs/legal/` lo dice con otras
palabras y con consecuencias contractuales: una institución europea puede quedar impedida de contratar el
producto si esto no se ha hecho.

**Este documento no declara conformidad.** Declara qué hay que probar, con qué, y dónde se apunta el resultado.
Mientras la tabla de la sección 5 esté vacía, la respuesta a "¿funciona con lector de pantalla?" es **no lo
sabemos**, y decirlo es parte del trabajo.

## 2. Lectores de referencia

Uno por sistema, y el que la gente usa de verdad, no el más cómodo de automatizar.

| Sistema | Lector | Motor del webview | Por qué este |
|---|---|---|---|
| Windows | NVDA | WebView2 (Chromium) | El más usado con diferencia entre quien no puede pagar una licencia |
| macOS | VoiceOver | WKWebView | Viene con el sistema; no hay alternativa realista |
| Linux | Orca | WebKitGTK | El único con soporte de escritorio mantenido |

**Los tres motores no coinciden**, y ahí está el riesgo: lo que anuncia bien WebView2 puede quedarse mudo en
WebKitGTK. Es el mismo motivo por el que la selección de texto del lector va por bloques y no por caracteres
([accessibility.md](../../architecture/accessibility.md), sección 4).

## 3. El recorrido

Es el del criterio de aceptación: **importar, abrir, leer y anotar**. Se hace entero, del principio al final,
sin ratón y con la pantalla apagada o los ojos cerrados. Mirar mientras se escucha esconde exactamente los
defectos que esto busca.

### 3.1 Arranque y carcasa

- [ ] Al abrir, el lector anuncia la aplicación y la vista en la que está.
- [ ] El enlace de salto al contenido se alcanza con el primer tabulador y funciona.
- [ ] La lista de puntos de referencia distingue navegación, contenido principal y paneles, **por nombre**.
- [ ] Cambiar de destino se anuncia; el foco queda en un sitio que tiene sentido.
- [ ] El idioma del documento raíz es el correcto: el texto no se pronuncia con las reglas de otro idioma.

### 3.2 Importar

- [ ] El panel de importación se alcanza y se anuncia con su nombre.
- [ ] Los campos dicen qué son y qué se espera en ellos.
- [ ] El avance de una importación larga se anuncia **sin interrumpir** lo que se esté leyendo.
- [ ] El resultado —cuántos entraron, cuántos ya estaban, cuántos se rechazaron— se anuncia entero.
- [ ] Un documento rechazado dice por qué, con el motivo traducido y no con un código.

### 3.3 Biblioteca

- [ ] La cuadrícula se recorre con el teclado y cada elemento se anuncia con título y autor.
- [ ] La portada no se anuncia como imagen sin nombre ni repite el título.
- [ ] Ordenar y filtrar se alcanzan, se anuncian y su efecto se comunica.
- [ ] Un documento no disponible se anuncia como tal, no como uno normal.
- [ ] La ficha lateral se alcanza y se anuncia al abrirse.

### 3.4 Leer

- [ ] Abrir un documento anuncia que se abrió y dónde quedó la lectura.
- [ ] **La prosa se lee como prosa**: el lector de pantalla recorre el texto con sus comandos habituales de
      lectura continua, no elemento a elemento.
- [ ] Los encabezados del documento se anuncian como encabezados.
- [ ] El cursor de bloque se anuncia al moverse, sin repetir el párrafo entero cada vez.
- [ ] Pasar página anuncia el cambio y no pierde el foco.
- [ ] El índice se recorre como árbol y el anidamiento se anuncia.
- [ ] **PDF con capa de texto:** el texto de la página se lee.
- [ ] **PDF escaneado sin OCR:** se anuncia que el documento no tiene texto, en lugar de quedarse en silencio.
- [ ] Entrar en lectura inmersiva se anuncia, y el aviso dice por dónde se vuelve.

### 3.5 Anotar

- [ ] Seleccionar bloques se anuncia, y extender la selección dice cuánto abarca.
- [ ] Abrir la nota lleva el foco al campo y lo anuncia.
- [ ] Guardar y cancelar se anuncian, y el foco vuelve a un sitio con sentido.
- [ ] El panel de anotaciones se alcanza, se recorre con las flechas y cada fila se anuncia entera: clase, cita
      y avisos.
- [ ] Una anotación reanclada o aproximada **anuncia su aviso**; no basta con que se vea.
- [ ] Borrar pide confirmación y la confirmación se anuncia.
- [ ] Tras guardar o borrar, el foco no se queda en el vacío.

### 3.6 Voz y ajustes

- [ ] La lectura en voz alta y el lector de pantalla **no se pisan**: si se solapan, se anota como defecto.
- [ ] Cuando no hay síntesis en la plataforma, el panel lo anuncia con su motivo.
- [ ] Alto contraste, movimiento reducido y escala tipográfica se alcanzan y su estado se anuncia.
- [ ] Cambiar de idioma cambia lo que el lector de pantalla pronuncia.

### 3.7 Lo que no puede pasar en ningún punto

- [ ] Ningún control se anuncia solo como "botón" o con una letra suelta.
- [ ] Ningún foco queda atrapado: de todo se sale con el tabulador o con Escape.
- [ ] Ninguna región `status` o `alert` repite el mismo aviso en bucle.
- [ ] Ningún cambio de estado ocurre en silencio.

### 3.8 El listón: qué es "suficientemente bueno"

"Mejor que antes" no es un criterio de ingeniería. Para que una pasada en la sección 5 pueda declararse **apta**, una
persona con la pantalla apagada y usando únicamente el teclado y el lector de pantalla de referencia de su sistema
(Orca en Linux, NVDA en Windows, VoiceOver en macOS) debe superar **tres condiciones medibles**:

#### A. Seis tareas nucleares dentro de su tope de pulsaciones

Se cuenta cada pulsación de tecla que mueve el foco o activa un control desde el punto de partida de la tarea (sin
contar las teclas de escritura libre dentro de un campo de búsqueda o de una nota):

| # | Tarea comprobable | Qué debe escucharse | Tope de pulsaciones |
|---|---|---|---|
| **T1. Ir al contenido principal al abrir** | Desde el arranque de la aplicación, alcanzar el enlace de salto o la región principal | Nombre de la aplicación, vista activa (`Inicio`) y primer elemento interactivo del lienzo | **≤ 2 pulsaciones** (`Tab` + `Enter`, o salto por punto de referencia) |
| **T2. Cambiar de destino y situarse** | Ir desde `Inicio` a `Biblioteca` y quedar situado en la barra de filtrado o en la primera tarjeta | Anuncio del cambio a `Biblioteca` y del control que recibe el foco, sin recorrer toda la barra lateral | **≤ 4 pulsaciones** |
| **T3. Encontrar y abrir un libro** | Desde el buscador o la primera tarjeta de `Biblioteca`, elegir una obra y abrirla | Título y autor anunciados una sola vez (sin leer la ruta de la portada ni "imagen"), seguidos del anuncio de apertura del capítulo | **≤ 5 pulsaciones** (fuera de las letras escritas si se filtra) |
| **T4. Leer tres bloques y pedir orientación** | En el lector, avanzar tres párrafos, pasar una página y pulsar `P` (`¿Dónde estoy?`) | Cada párrafo leído como prosa continua; el paso de página anunciado sin perder el foco; y `P` anunciando capítulo, encabezado cercano y porcentaje | **1 pulsación por bloque** (`Flecha abajo`) y **1 pulsación** (`P`) para la orientación |
| **T5. Resaltar y escribir una nota** | Con un bloque elegido, resaltar (`R`), abrir nota (`Intro`), escribir y guardar (`Ctrl+Intro`) | Anuncio del bloque seleccionado, del campo de nota al abrirse, del guardado y del retorno del foco al mismo bloque del documento | **≤ 4 pulsaciones** (fuera del texto escrito en la nota) |
| **T6. Recorrer anotaciones y saltar a una** | Abrir el panel de anotaciones, recorrer dos entradas y activar una para ir a su sitio en el libro | Clase de marca, cita y aviso de reanclaje (si existe) en cada fila; al pulsar `Enter`, salto al bloque anotado con anuncio de posición | **≤ 5 pulsaciones** hasta quedar situado en el párrafo del libro |

#### B. Cero violaciones en las cuatro invariantes de la sección 3.7

Un solo control anunciado únicamente como "botón" sin etiqueta, una sola trampa de foco de la que no se salga con `Tab` o
`Escape`, una región `aria-live` que repita avisos en bucle o una acción que cambie de estado en silencio basta para que
la pasada quede **no apta**, aunque el resto de casillas estén marcadas.

#### C. Umbral del recorrido completo (secciones 3.1 a 3.6)

De las **31 casillas** de las secciones 3.1 a 3.6, deben cumplirse al menos **28 a la primera**, ninguna de las 3 de
margen puede bloquear las tareas nucleares T1–T6, y toda casilla no superada debe quedar registrada con número de
incidencia en el registro de la pasada.

## 4. Cómo se registra una pasada

Una pasada es **una persona, un sistema, una versión**. Se registra como una incidencia del repositorio con la
etiqueta `accessibility`, con:

* La fecha y la versión del producto.
* El sistema, el lector de pantalla y sus versiones.
* El resultado de las seis tareas nucleares (T1–T6) con sus pulsaciones reales y cuántas casillas pasan de las 31.
* **Los defectos encontrados, con su número de incidencia.** Una pasada sin defectos apuntados y sin decir que
  no los hubo no es una pasada: es una casilla marcada.

Un defecto encontrado aquí se corrige y **se le añade una prueba de regresión** donde se pueda: casi siempre en
la suite de navegador, que corre en `pnpm verify`. Lo que no se pueda automatizar se queda en esta lista, y
por eso la lista crece.

## 5. Personas que usan tecnologías de asistencia

[legal/accessibility-compliance.md](../../legal/accessibility-compliance.md) exige además **pruebas con personas que
usan tecnologías de asistencia a diario**. No es lo mismo que esta lista: esta la puede recorrer quien programa, con
el lector encendido; aquella la recorre quien lleva años usándolo y sabe cuándo algo es incómodo en lugar de
imposible.
