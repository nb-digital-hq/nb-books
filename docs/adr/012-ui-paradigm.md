# ADR-012: Paradigma de la Interfaz

**Fecha:** 2026-08-27
**Decidido:** 2026-08-27
**Estado:** Aceptado


## Contexto

Un prototipo anterior implementaba NB Books como un **escritorio virtual dentro de la ventana**:
gestor de ventanas propio, dock, panel superior, applets, fondo de escritorio y un Alt+Tab emulado. Ese
paradigma nunca se justificó por escrito, y chocaba con una tensión evidente: *"un gestor de ventanas dentro de una ventana es, por definición, una capa
adicional de interacción"*, frente al principio 6 de la
[filosofía de producto](../design/product-philosophy.md), que dice que **eliminar fricción vale más
que agregar funciones** y que cada clic y cada ventana debe justificar implacablemente su existencia.

La decisión se tomó antes de escribir una línea de la interfaz actual. La informan tres hechos medidos:

1. **La proporción de esfuerzo del prototipo.** En el prototipo, la carcasa de escritorio
   virtual (`src/lib/os/`: gestor de ventanas, dock, panel, menú contextual, applets, workspace) son **2.951
   líneas**. La aplicación de biblioteca son **8** y el lector, **430**. Se invirtió siete veces más esfuerzo en
   la ventana que en lo que va dentro, y aun así el gestor de ventanas estaba incompleto: sin multi-monitor, sin
   equivalentes de teclado para las ocho zonas de redimensión, sin restauración defensiva.
2. **El presupuesto de rendimiento ya lo desaconsejaba.**
   [performance-budgets.md](../architecture/performance-budgets.md) fija un límite de **1.500
   nodos del DOM** y exige que cinco mil documentos se desplacen dentro de ese límite. Una rejilla
   virtualizada consume por sí sola una fracción importante de ese presupuesto. N ventanas flotantes, cada una
   con una aplicación completa dentro, más sombras y desenfoques compuestos, no caben.
3. **El modelo de extensiones ya había votado en contra sin advertirlo.** Sus puntos de contribución son
   comandos, menús contextuales, **vistas y paneles**, con forma de panel y no de ventana, mientras el prototipo montaba los applets en *Containments* del
   escritorio. Además, el diseño promete estabilidad de API a terceros, y no se puede
   prometer estabilidad sobre la superficie de un gestor de ventanas.

## Alternativas consideradas

### A. Escritorio virtual completo ("Book OS")

Gestor de ventanas emulado dentro del webview, con ventanas flotantes movibles y redimensionables, dock, panel,
applets y conmutación de foco propia.

**A favor:** simultaneidad libre en cualquier disposición; una superficie de extensión uniforme, donde todo
plugin es "una aplicación con su ventana"; identidad visual inconfundible.

**En contra:**

* **Accesibilidad: el coste mayor de todos.** No existe patrón ARIA para "ventana flotante no modal".
  `role="application"` desactiva el modo de navegación del lector de pantalla, y entonces el contenido del libro
  necesita su propia vía de escape. El orden del DOM y el orden Z divergen por construcción, que es exactamente
  el caso en que el teclado y el lector de pantalla se pierden. El Alt+Tab emulado colisiona con el real del
  sistema operativo.
* **Duplica, peor, algo que el usuario ya tiene.** Su gestor de ventanas real hace mosaico, abarca varios
  monitores, tiene atajos ya aprendidos y escritorios virtuales. El nuestro no puede nada de eso. A quien más le
  interesaría la simultaneidad se le **quita** capacidad.
* **Fricción, contra el principio 6.** Impone una gramática entera (mover, redimensionar, minimizar, restaurar,
  ajustar, enfocar) antes de leer la primera página. Y contra el principio 7, "lo complejo es interno, lo
  sencillo es externo": es complejidad empujada hacia afuera.
* **Contra el principio 5.** La filosofía rechaza "interfaces que parecen un frankenstein de módulos
  inconexos". Convertir cada función en una aplicación con su ventana es precisamente eso.
* No degrada a anchos pequeños, y conducirlo con Playwright por WebDriver es mucho más frágil que
  conducir una carcasa de paneles.

### B. Aplicación de escritorio convencional, con paneles y vistas

Una sola ventana del sistema. Carcasa fija con navegación primaria, área principal, paneles acoplables,
pestañas o vistas para varios documentos y paleta de comandos.

**A favor:** semántica estándar que los lectores de pantalla ya conocen; el gestor de ventanas real sigue
mandando; cabe en los presupuestos; degrada a cualquier ancho; da un contrato de extensión acotado y por tanto
estabilizable; fricción de aprendizaje nula.

**En contra:** la simultaneidad libre desaparece, sustituida por vista dividida; y existe el riesgo de parecerse
a las herramientas de las que toma la forma.

### C. Híbrido con ventanas reales del sistema operativo

La carcasa de B, y para la simultaneidad, una **ventana real** de Tauri en lugar de una emulada.

**A favor:** consigue el objetivo que perseguía A delegándolo en el gestor de ventanas del usuario, que es mejor
que el nuestro y gratuito; la accesibilidad de una segunda ventana real es nativa.

**En contra:** varios webviews consumen memoria; obliga a definir qué estado cruza entre ventanas; y reparte la
aplicación en superficies que el usuario debe administrar.

### D. Híbrido interno: carcasa convencional con un "modo taller" de escritorio virtual

**Se descarta por pagar los dos costes.** Sigue exigiendo construir y mantener el gestor de ventanas, sigue
debiendo la emulación ARIA completa, sigue teniendo el orden Z divergente del orden del DOM, y añade encima un
conmutador de modo: una segunda gramática y un momento en que el usuario debe elegir sin saber todavía qué
elige. Solo se justificaría si ese modo fuese por sí mismo la razón de existir del producto.

## Decisión

Se adopta la **alternativa B**. NB Books es una **aplicación de escritorio convencional, de una sola ventana,
construida con paneles y vistas**. No hay gestor de ventanas emulado, ni dock, ni fondo de escritorio, ni
conmutación de foco propia. La ventana de la aplicación es la ventana del sistema operativo.

### Por qué no la C, que parecía técnicamente superior

C resolvía la simultaneidad sin ninguno de los costes de A, y era la recomendación previa. Se descarta por una
razón de producto que pesa más que la técnica: **el objetivo es el mejor lector que exista, y eso exige
centralizar, no repartir**. Un producto que abre ventanas se convierte en un producto que el usuario administra.
El criterio no es la comodidad de construirlo, sino la comodidad de tenerlo.

Queda no obstante una salida acotada, para no cerrar la puerta a la comparación de documentos: **si en algún
momento hace falta simultaneidad real, se abre una ventana real del sistema, nunca una emulada**. No se
implementa y no forma parte del paradigma; es la única excepción admisible y solo con una necesidad
demostrada detrás.

### Qué significa "B, pero bien hecha"

La alternativa B es una decisión sobre la carcasa, no sobre la calidad. El riesgo real de B no es la
accesibilidad ni el rendimiento: es la indistinción. Se contrarresta con siete compromisos, que este ADR fija
como vinculantes y para las que dependan de ella.

**1. La carcasa no se inventa.** El usuario que abre NB Books por primera vez debe pensar "ya sé usar esto" sin
haberlo visto jamás. Se toman las convenciones ya establecidas por Google Play Libros, la aplicación de Kindle y
los lectores de escritorio existentes, en lugar de proponer una gramática nueva. Innovar aquí es replantear el
flujo, no el marco (principio 3 de la filosofía). Todo elemento de la carcasa sin equivalente reconocible en una
herramienta que el usuario ya use debe justificarse en su PR.

**2. La carcasa desaparece cuando se lee.** La lectura inmersiva a pantalla completa no es una opción de menú:
es un modo de primera clase, con su atajo, que oculta navegación, paneles y barras y deja el texto solo. La
distinción de NB Books se juega en la superficie de lectura, no en el chrome. El chrome debe poder irse entero.

**3. Responsive de verdad, en tres clases de ancho.** Compacto (una sola columna, paneles como capas
superpuestas y descartables), medio (contenido más un panel acoplado) y amplio (contenido más dos paneles). Los
tres se especifican y se verifican. Un panel que no sabe colapsar no entra.

**4. Personalización en dos planos, separados a propósito.** El plano de la **aplicación** (tema claro, oscuro y
del sistema, densidad, acento, tipografía de la interfaz, disposición de paneles) y el plano del **libro**
(tipografía, cuerpo, interlineado, márgenes, ancho de línea, justificación, color de fondo y de texto, modo de
paginación). Se guardan por separado: cambiar el tema de la aplicación no debe alterar cómo se ve un libro, y al
revés. Ambos planos persisten en SQLite según [ADR-010](010-settings-persistence.md) y entran en la
exportación.

**5. Modos de lectura, no un interruptor de tema.** El lector ofrece un conjunto de modos con nombre, cada uno
un preajuste completo de fondo, texto, contraste y temperatura: día, sepia, nocturno, alto contraste y modos de
tipo papel inspirados en las pantallas NXTPAPER de TCL y en la tinta electrónica (monocromo de alto contraste y
color atenuado con filtro de luz azul por temperatura). Un modo es un preajuste, no una jaula: todo valor que
fija puede ajustarse a mano después, y el resultado se puede guardar como modo propio del usuario.

**6. Todo lo que no es nativo se emula, y se declara como emulado.** Un webview no controla el brillo del
panel físico ni la temperatura de color del hardware: NB Books los emula con una capa de composición sobre la
superficie de lectura. Esa emulación se nombra como tal en la interfaz y en la documentación de usuario, nunca
se presenta como control del dispositivo. Es la misma regla que el proyecto ya aplica a las capacidades
degradadas: lo que no está, se declara, no se disimula.

**7. La velocidad es parte del diseño.** Los presupuestos de
[performance-budgets.md](../architecture/performance-budgets.md) gobiernan la interfaz igual que
gobiernan el runtime. Cambiar de modo de lectura, abrir un panel o pasar página no puede salirse del
presupuesto de fotograma. Una animación que no cabe se elimina, no se recorta.

### Lo que este ADR prohíbe explícitamente

* Gestores de ventanas, docks, paneles de tareas, fondos de escritorio y conmutadores de foco emulados.
* `role="application"` sobre contenedores de la carcasa. Solo se admite dentro de un componente concreto que
  gestione su propio teclado y ofrezca salida documentada.
* Cualquier contenedor cuyo orden visual no coincida con su orden en el DOM.
* Elementos superpuestos y arrastrables sobre la superficie de lectura.

### Efecto sobre ADR-001, que no se edita

[ADR-001](001-tech-stack.md) justificaba en parte la elección de Svelte por su reactividad granular *"para
un paradigma de escritorio virtual con decenas de componentes interactivos simultáneos"*. Ese escenario deja de
existir. **ADR-001 no se modifica ni se marca como superado**, porque los ADR son inmutables y su decisión
—Svelte 5 con runes— sigue siendo la correcta por sus otros motivos: sin DOM virtual, sin coste de tiempo de
ejecución y compatible con los presupuestos de bundle y de fotograma. Lo que caduca es uno de sus argumentos de
apoyo, y queda registrado aquí para que nadie lo cite como vigente.

## Consecuencias

**Positivas.**

* La accesibilidad pasa de ser un problema de emulación a ser un problema de disciplina: `banner`, `navigation`,
  `main`, `complementary`, `tablist`/`tab`/`tabpanel` y `dialog` solo para lo que es modal de verdad. El objetivo
  WCAG 2.1 AA de [accessibility.md](../architecture/accessibility.md) deja de depender de patrones
  sin precedente.
* La navegación por teclado se apoya en la convención de plataforma (ciclo de regiones) en lugar de emular
  Alt+Tab, con lo que deja de colisionar con el sistema operativo.
* Se recupera todo el esfuerzo que el prototipo gastaba en la carcasa y se invierte en la superficie de lectura,
  que es donde el producto se diferencia.
* El contrato de extensión queda acotado y estabilizable: barra de navegación, panel, vista,
  comando, menú contextual y tema. Se puede prometer compatibilidad sobre eso.
* Los presupuestos de nodos y de fotograma dejan de estar en conflicto con la interfaz.
* Conducir el binario con pruebas de interfaz se vuelve viable: se conducen paneles y no arrastres de ventanas.

**Negativas.**

* Se pierde la simultaneidad libre. Comparar dos documentos dependerá de vista dividida, y no de disponerlos a
  voluntad.
* Se renuncia a la identidad visual inmediata del escritorio virtual. La distinción hay que ganarla en la
  calidad de la lectura, que es más difícil y menos vistoso en una demostración.
* La personalización profunda del lector es trabajo real y sostenido: los siete compromisos de arriba no son
  gratis, y varios (modos de lectura, emulación de brillo y temperatura) añaden superficie que hay que probar en
  las tres plataformas.

**Riesgos.**

* **Indistinción.** El riesgo declarado de B. Se mitiga con los compromisos 1, 2, 5 y 6: si la superficie de
  lectura no se distingue de la de un lector genérico, no está terminada.
* **Personalización que se convierte en configuración.** Máxima personalización mal entendida es un panel de
  preferencias interminable, que es fricción disfrazada de libertad. Mitigación: los modos con nombre son la vía
  principal y el ajuste fino es secundario; todo valor tiene un valor por defecto sensato y una vía de retorno.
* **La emulación de brillo y temperatura puede engañar.** Mitigación: el compromiso 6, que obliga a nombrarla
  como emulación en la interfaz.

## Documentos que este ADR obliga a cambiar

Se actualizan en el mismo PR, por la regla 5 de la Constitución:

| Documento | Cambio |
|---|---|
| [ui-components.md](../architecture/ui-components.md) | Reescrito entero: sustituye el paradigma "Book OS" |
| [accessibility.md](../architecture/accessibility.md) | Elimina la emulación de gestor de ventanas y fija los patrones estándar |
| [state-management.md](../architecture/state-management.md) | La restauración de sesión deja de restaurar ventanas y posiciones |
| [performance-budgets.md](../architecture/performance-budgets.md) | Las métricas dejan de referirse al Dock y al "Escritorio" |
| [system-overview.md](../architecture/system-overview.md) | La UI deja de implementar el paradigma "Book OS" |
| [plugin-system.md](../architecture/plugin-system.md) | Los puntos de montaje dejan de ser Containments del escritorio |
| [architecture/README.md](../architecture/README.md) | `ui-components.md` pasa de "En revisión" a "Vigente" |
| [accessibility-compliance.md](../legal/accessibility-compliance.md) | El alcance nombra la carcasa real, no escritorio, dock y ventanas |
| [testing/regression.md](../developer/testing/regression.md) | La prueba de humo deja de esperar un `WindowShell` |
| [naming.md](../developer/coding-standards/naming.md) | La convención "Applet" se sustituye por el nombre de la ranura ocupada |
| [logging.md](../developer/coding-standards/logging.md) | El ejemplo deja de registrar la carga de un applet de calendario |


## Prototipo de baja fidelidad

[prototipo-interfaz](../assets/prototipo-interfaz.html) muestra la carcasa en las tres clases de ancho, el modo
de lectura inmersiva y los modos de lectura del compromiso 5. Es un boceto en `docs/assets/`, deliberadamente fuera
de `src/` y de la compilación de la aplicación: un prototipo de decisión no es implementación.
