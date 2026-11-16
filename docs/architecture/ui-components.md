# Arquitectura de la Interfaz

> **Estado:** Implementado, según [ADR-012](../adr/012-ui-paradigm.md): tokens, siete temas, anchos y componentes
> base; biblioteca; lector con inmersión y plano del libro; preferencias; estados degradados; modos de lectura y kit
> accesible; Inicio; Estudio; vista dividida y búsqueda.
> Las convenciones que gobiernan todo esto están en [design/ui-conventions.md](../design/ui-conventions.md).
>
>
> Este documento sustituye al paradigma "Book OS" que describía la versión anterior. El escritorio virtual
> quedó descartado con sus motivos en ADR-012; aquí no se vuelve a discutir, se describe lo que sí se construye.

NB Books es una **aplicación de escritorio convencional de una sola ventana**, construida con paneles y vistas.
La ventana de la aplicación es la ventana del sistema operativo: no hay gestor de ventanas emulado, ni dock, ni
fondo de escritorio, ni conmutación de foco propia.

El criterio que gobierna cada decisión de este documento: **el usuario que abre NB Books por primera vez debe
pensar "ya sé usar esto" sin haberlo visto jamás**. La innovación del producto está en el flujo y en la
superficie de lectura, no en el marco.

---

## 1. Las cuatro zonas

La carcasa tiene cuatro zonas, y cada una es un punto de referencia estándar de accesibilidad. Esa
correspondencia no es decorativa: es lo que permite que la navegación por teclado y el lector de pantalla
funcionen sin emular nada.

| Zona | Punto de referencia | Contenido | Dónde vive hoy |
|---|---|---|---|
| Barra superior | `banner` | La búsqueda y los menús que abren Estanterías, Colecciones e Importar. El título de la vista es su encabezado, y solo se ve en el lector | `src/lib/ui/AppShell.svelte`, `Desplegable.svelte`, `Emergente.svelte` |
| Barra lateral | `navigation` | Los destinos de 1.1, y solo los que existen, y debajo sus tarjetas vivas | `AppShell.svelte`, `NavItem.svelte` y `ui/lector/destinos.ts` |
| Área principal | `main` | La vista activa: biblioteca o lector | `LibraryView.svelte`, `ui/lector/ReaderView.svelte` |
| Panel lateral | `complementary` | Índice, anotaciones y ajustes del libro; diccionario | `Panel.svelte` con el contenido de cada vista |

La columna de ubicación describe **dónde está el código hoy**, no dónde debería estar: los directorios `shell/`,
`views/` y `panels/` que esta tabla anunciaba nunca se crearon, y mantener una ruta que no existe
es peor que no dar ninguna.

`dialog` se reserva para lo que es modal de verdad. Un panel no es un diálogo y no atrapa el foco.

**La jerarquía la dan las capas, no los trazos.** Las dos barras están sobre la superficie de la
ventana, y el área principal va en un **lienzo** de la capa de encima, con las esquinas redondeadas; los paneles son
tarjetas dentro de ese lienzo. Lo que separa dos zonas es que una está más arriba que la otra, que es la gramática
de Material 3, y el trazo que había entre ellas sobraba.

**Las tarjetas de la barra lateral son dinámicas.** Aparecen cuando dicen algo y se van cuando dejan de decirlo:
lo que se está leyendo, cómo va el índice de búsqueda mientras no termina, y un aviso cuando algún componente del
runtime necesita atención, que lleva a Ajustes, en Acerca de. Lo que se añada después —metas, por ejemplo, si algún
día existen— entra con la misma forma. Plegada la barra no caben, y en la barra inferior tampoco.

### La barra lateral es de alto completo y se pliega

De alto completo porque es lo que la hace **el marco de la aplicación** y no un cajón dentro de una pantalla.
Plegable porque en una ventana estrecha o en una lectura larga, cada píxel de ancho es del contenido.

**Plegada sigue siendo la misma barra**: los mismos elementos, en el mismo orden, con el mismo nombre accesible.
Solo desaparece el texto, nunca un destino. En la clase compacta está siempre plegada y pasa al borde inferior,
que es el que queda al alcance sin recolocar la mano.

**Plegada, todo va en un solo eje.** Mide 4,5 rem, y sus destinos son círculos de 2,75 rem —44 px, el
objetivo mínimo de WCAG 2.5.5— centrados en vertical entre la marca y el pie, como el grupo de un riel de Material 3;
la marca queda a la altura de la barra superior, junto al botón que despliega. Antes medía 3,5 rem con destinos de
3 rem: por dentro quedaban 32 px para botones de 48, así que se salían por la derecha y el activo se recortaba
contra el lienzo.

Lo implementa `AppShell.svelte`, que además provee el enlace de salto al contenido.

## 1.1 Los destinos

Qué hay dentro de la navegación primaria lo decide
[ADR-013](../adr/013-information-architecture.md), sobre
[la investigación de la experiencia de lectura](../design/reading-experience/README.md).

| Destino | Contiene | Aparece |
|---|---|---|---|
| **Inicio** | Sigue leyendo, recomendaciones locales, novedades | Siempre. Es la pantalla de arranque |
| **Biblioteca** | Todo, estanterías, colecciones, etiquetas, autores, series, materias; importar; duplicados y no disponibles | Siempre |
| **Descubrir** | Catálogos OPDS añadidos por la persona | Con al menos un catálogo, marcada como en línea |
| **Lector** | El documento abierto y sus paneles | Solo con algo abierto |
| **Estudio** | Notas enlazadas, tableros, anotaciones de todos los libros, vocabulario y repaso | Siempre |
| **Ajustes** | Apariencia, lectura, accesibilidad, idioma, almacenamiento, red y privacidad, extensiones, IA y modelos, acerca de y actualizaciones | Siempre |

### Qué es un destino y qué no

El criterio, y no la lista, es lo vinculante. **Algo es un destino si cumple las tres condiciones:** es un lugar
donde el usuario permanece y trabaja; tiene estado propio que sobrevive a salir y volver; y no es una propiedad
de otro destino.

| Categoría | Definición | Ejemplo |
|---|---|---|
| Destino | Cumple las tres | Biblioteca |
| Panel | Acompaña a un destino y no existe sin él | Anotaciones del documento abierto |
| Ajuste | Se configura y no se visita | Escala tipográfica |
| Transversal | Aparece en varios destinos sin ser ninguno | Búsqueda, IA, lectura en voz alta |

Toda propuesta de "añadir una sección" se contesta con este criterio, no con una discusión de gustos.

### Reglas vinculantes

1. **Se arranca en Inicio, no en Biblioteca.** Una cuadrícula de portadas responde a "qué tengo"; quien abre un
   lector quiere "seguir donde estaba".
2. **El Lector no se cierra al navegar.** Ir a Biblioteca durante una lectura no termina la sesión: se vuelve
   donde se dejó.
3. **Lo que necesita red vive donde funcionalmente corresponde**, marcado, no en una sección aparte. La única
   excepción es Descubrir, cuya razón de ser es el catálogo remoto.
4. **La IA no es un destino**, ni la búsqueda, ni la lectura en voz alta: son transversales.
5. **Un destino contextual no se muestra vacío ni atenuado.** Si no aplica, no está en la barra.

## 2. Comportamiento por ancho

Tres clases, especificadas y verificadas. Los puntos de corte concretos se fijan al
implementarlas; lo que fija este documento es el comportamiento.

| Clase | Disposición |
|---|---|
| Amplio | Navegación, contenido y hasta dos paneles acoplados a la vez |
| Medio | Navegación, contenido y un panel acoplado |
| Compacto | Una sola columna; la navegación pasa a una barra inferior y los paneles son capas superpuestas y descartables |

**Un panel que no sabe colapsar no entra.** Es requisito de aceptación, no recomendación.

**La clase de ancho que decide si un panel se acopla se mide sobre la columna de contenido, no sobre la
ventana.** El panel no compite por el espacio con la barra lateral: compite con el contenido. Medirlo sobre la
ventana dejaba, en una de 720 px, doscientos ocho píxeles de contenido junto a un panel de doscientos ochenta y
ocho, y el contenido acababa con una barra de desplazamiento horizontal escondida en mitad de la vista.

### 2.1 Dos clases de panel: el que ocupa y el que pasa

No todos los paneles quieren lo mismo, y tratarlos igual sale mal en los dos sentidos.

| Clase | Cuáles | Cómo se comporta |
|---|---|---|
| **De la vista** | Ficha de la biblioteca | Ocupa su hueco junto al contenido. Está mientras estés ahí **y haya obra elegida** |
| **Momentáneo** | Índice del documento, anotaciones, ajustes del libro, modo de lectura | Se pone **encima** de lo que haya, sin quitarle sitio, y se va al pulsar fuera o con Escape |
| **Menú de la barra** | Buscar, importar, colecciones y estanterías | Nace **debajo del control que lo abre**, se superpone igual que un momentáneo y se va igual. Ver `ui-conventions.md`, sección 4.1 |

El criterio es cuánto dura la intención. Un panel de la vista acompaña a lo que estás haciendo; un momentáneo se
consulta y se cierra, y quitarle sitio al contenido para algo que dura veinte segundos es cobrar caro por poco.
Y al superponerse en lugar de sustituir, consultar dónde está algo no hace desaparecer lo que estabas mirando.

**Los del lector son momentáneos, y ahí la regla pesa más que en ninguna parte.** Consultar el índice o subir el
cuerpo de letra dura segundos; estrechar la columna de lectura para eso rompe el renglón y obliga a recolocar la
vista al volver. La ficha de la biblioteca es la única que se queda: no se consulta, se mira.

**Ninguno de los dos es un diálogo.** No atrapan el foco, por la regla de paneles de ADR-012. Que un momentáneo
se cierre al pulsar fuera ayuda; no lo convierte en modal.

**Un panel existe cuando tiene algo dentro.** La ficha se montaba al entrar en Biblioteca y, sin nada
elegido, gastaba 288 píxeles para decir "elige una obra". Y como un panel se **superpone** cuando la columna de
contenido baja de 720 píxeles, en una ventana de 800 la biblioteca entera desaparecía detrás de un panel vacío.
Un panel cuyo único contenido es una invitación a elegir no es un panel: es un hueco con título. La regla vale
para los dos tipos y para los que vengan.

**Al cerrarse por pulsación fuera, un momentáneo deja pasar los controles con `aria-expanded`**, que son
exactamente los que ya alternan por su cuenta. De ahí sale una obligación sobre quien abre un panel: su botón
lleva `aria-expanded` y **nunca** `aria-pressed`. Con `aria-pressed` el mismo clic abre y cierra, y el panel no
llega a verse. Ocurrió con los cinco del lector; el recorrido que lo fija está en `e2e/flujo.spec.ts`.

**Lo que acota lo que se ve no va en la barra lateral.** Los estados —todo, empezados, duplicados, no
disponibles— viven en la barra de filtros de la vista, encima del contenido, y las estanterías y las colecciones
en los menús de la barra superior. La barra lateral lleva **destinos**, que son siete y no cambian; una biblioteca con doscientas estanterías dentro de la
barra la deja inservible, y además mezcla dos cosas que el usuario no piensa juntas: a dónde voy y qué estoy
mirando.

Los puntos de corte tienen **una sola fuente**, `src/lib/ui/anchos.ts`, y la clase vigente viaja como atributo
`data-ancho` en la raíz de la carcasa. No hay consultas de medios por ancho en ninguna hoja: `check-ui.mjs` las
rechaza, porque un punto de corte escrito en dos sitios acaba divergiendo y el síntoma es una interfaz que
parpadea entre dos disposiciones al arrastrar el borde de la ventana.

### 2.2 La vista dividida: quién manda y qué se ofrece

Cada mitad es **un lector entero** con su estado, su posición y sus paneles. La división la gobierna la carcasa y
no la máquina del lector: meterla ahí obligaría a que un lector supiera de la existencia del otro.

Tres reglas, y las tres salen de conectarla de verdad:

1. **Solo una mitad recibe el teclado.** Los dos lectores escuchan en `window`, así que sin gobernar quién manda
   una sola pulsación llega a los dos y bajar el cursor mueve las dos a la vez. Quién manda lo dice
   `division.foco`, y **se ve**: con dos lectores idénticos en pantalla no hay otra forma de saber a cuál va la
   tecla siguiente.
2. **No se cambia el foco hacia una mitad vacía.** Allí no hay lector que gobierne el teclado, así que el foco
   quedaría en un sitio donde ninguna tecla responde.
3. **Lo que solo sirve con dos mitades no se ofrece con una**, y dividir no se ofrece donde no caben dos columnas
   de lectura. Es la regla —lo que no aplica se omite, no se atenúa— aplicada a un estado en lugar de a
   una capacidad del formato.

El reparto es un número que se mueve a pasos y no un arrastre, porque arrastrar un separador es un gesto de
puntero sin equivalente de teclado. Sus dos comandos llegaron: hasta entonces `repartir` estaba
escrita, probada y sin llamar, y el separador no se podía mover de ninguna manera.

### 2.3 El avisador: dónde contesta la aplicación

Hay **una** zona donde la aplicación dice qué ha pasado, fija en la esquina inferior derecha y por encima de
todo, incluida la lectura inmersiva. No es una zona de la carcasa: vive en el diseño de la ruta, porque la
carcasa se retira entera al leer y un avisador que se fuera con ella callaría justo cuando alguien acaba de
anotar sin verlo.

* **No atrapa el foco, no bloquea y no exige respuesta.** Un aviso que hay que cerrar para seguir es un diálogo
  disfrazado.
* **Dos regiones vivas y no una:** una educada para lo que salió bien y una que interrumpe para lo que no. Son
  dos urgencias distintas y una sola región solo puede declarar una.
* **La cola es pura** (`notificaciones.ts`) y su pintado, aparte. El tope de tres a la vez, el recuento de lo
  repetido y la caducidad por clase se comprueban sin navegador.

La regla de producto —qué se anuncia y qué no— vive en la sección 9 de
[ui-conventions.md](../design/ui-conventions.md), que es donde se decide, no aquí.

## 3. La carcasa desaparece cuando se lee

La lectura inmersiva a pantalla completa es un **modo de primera clase con atajo propio**, no una opción de
menú. Oculta barra, navegación y paneles y deja el texto solo. Los controles reaparecen ante el puntero o una
tecla y se retiran solos.

Existen dos niveles, y se distinguen:

* **Inmersión dentro de la ventana:** la carcasa se retira, la ventana sigue siendo la ventana.
* **Pantalla completa del sistema:** se solicita al runtime (`tauri::window`), y es el sistema operativo quien
  la concede. Si una plataforma no la concede, se declara; no se simula ocultando el escritorio.

**Implementado.** Las tres zonas dependen de un **único** valor, y eso no es un detalle de estilo: con
una condición por zona, olvidarse de una deja la barra superior encima del texto y no se nota hasta verlo. Que se
van las tres se comprueba **sobre el marcado que sale de verdad**, no sobre ese booleano, porque un booleano bien
puesto con una zona que no lo mira sigue dejando la carcasa a medias.

Retirarla se **anuncia** a las tecnologías de asistencia, con la vía de vuelta: quedarse sin la mitad de la
aplicación es información, no un efecto visual. Y Escape sale de un escalón cada vez —la nota, el panel, la
selección, la inmersión—, porque saltárselos sacaría al usuario de la lectura sin que lo hubiera pedido.

## 4. Personalización: dos planos separados

Se separan a propósito, porque son decisiones distintas del usuario y se pierden juntas si se mezclan.

| Plano | Qué controla |
|---|---|
| Aplicación | Tema claro, oscuro y del sistema; densidad; acento; tipografía de interfaz; disposición de paneles |
| Libro | Tipografía, cuerpo, interlineado, márgenes, ancho de línea, justificación, color de fondo y de texto, paginación |

Cambiar el tema de la aplicación no altera cómo se ve un libro, y al revés. Ambos planos persisten en SQLite
según [ADR-010](../adr/010-settings-persistence.md) y entran en la exportación, que
es lo que hace que la personalización sobreviva a un cambio de equipo.

**Implementado.4** para el plano del libro; el de la aplicación. La separación no se confía a
una convención de nombres: **cada preferencia declara su plano** y la capa que compone el libro solo lee las
suyas. Hay dos pruebas, una por lado: en el runtime, que tocar un plano no escribe en el otro; en la interfaz,
que el plano del libro no se altera aunque le llegue el de la aplicación entero. Ninguna preferencia del libro
entra en la caché del primer pintado, porque el libro se abre después de pintar la carcasa.

## 5. Modos de lectura, y temas de la aplicación

Hay dos preajustes con nombre y no uno, porque hay dos planos (sección 4): el **modo de lectura** viste el libro
y el **tema** viste la aplicación. Se eligen por separado y ninguno arrastra al otro; lo único que los conecta
es que cada tema **sugiere** con qué modo acompañarlo, y esa sugerencia se puede ignorar.

### 5.1 Un tema es una sensación, no una paleta

Nadie sabe qué va a leer cada persona: uno abre un manual técnico, otro una novela infantil, otro un manga y
otro un ensayo. **No existe la interfaz correcta para todos, así que no se intenta**: lo que hace la aplicación
es adaptarse a lo que cada uno espera *sentir* al abrirla.

Por eso un tema fija seis cosas y no una:

| Pieza | Qué controla |
|---|---|
| Paleta | Fondo, texto, trazos y acento |
| Tipografía | La de la **interfaz**, nunca la del libro |
| Movimiento | Cuánto hay y con qué curva |
| Textura | Grano, papel, o nada |
| Redondeo | De esquina viva a muy suave |
| Modo sugerido | Con qué modo de lectura acompaña |

Un tema que solo cambiara colores no cambiaría la sensación, que es justamente lo que se le pide.

**La capa inicial son siete más el del sistema**, elegidos para cubrir sensaciones distintas y no matices del
mismo gusto: Grafito (el lápiz: grises de verdad y un solo color), Gabinete (la calma de una biblioteca), Papel
(luz de día), Tinta (precisión sin adornos), Pergamino (lo copiado a mano), Vigilia (la noche) y Musgo (lo vegetal
y lento). **Grafito es el de partida** y el que resuelve el del sistema cuando el escritorio pide oscuro. Un valor
guardado de un tema retirado se resuelve a Grafito, que conserva su acento. **Lo que falte lo traen
las extensiones**, que es para lo que existe el punto de extensión "un tema" de la sección 8: la aplicación
ofrece una base amplia y no intenta agotar los gustos, porque no puede.

**Ningún tema se publica sin medir su contraste.** Un tema con carácter que no se puede leer no es un tema, es
un defecto; la comprobación que ya calcula el contraste sobre `app.css` vale igual para estos. Y con movimiento
reducido, la parte animada de un tema desaparece entera sin cambiar dónde está nada.

### 5.1.1 De dónde sale la paleta desde Material 3

Según [ADR-014](../adr/014-design-system.md):

Cada tema es una **semilla** —un color— y `src/lib/ui/generadorM3.ts` deriva de ella los cincuenta y nueve
tokens `--m3c-*` que pintan la interfaz. Las catorce paletas de fábrica —siete temas por dos niveles de contraste— se
calculan **al construir** y viajan en `src/paletas.css`, así que el primer fotograma ya sale con su tema. En tiempo
de ejecución solo se calcula la de un acento propio, con la utilería de color cargada a demanda y la última paleta
guardada para no esperarla al arrancar ([performance-budgets.md](performance-budgets.md), sección 4.11). Las otras cinco piezas
de la tabla de arriba siguen viniendo de `app.css` por `data-theme`: la tipografía, el movimiento, la textura, el
redondeo y el modo sugerido. Un tema sigue siendo una sensación y no una paleta.

**El alto contraste es un nivel, no otra semilla.** Material deriva cada color contra su fondo con una curva de
contraste, y el nivel es un parámetro de esa curva. Sustituir la semilla por blanco o negro no subiría el contraste
y cambiaría el tono del tema. Con el nivel puesto, Gabinete pasa de 14,36 a 18,51 de texto sobre fondo y de 5,83 a
16,22 en los bordes, y sigue siendo ámbar.

**Grafito no tiñe sus grises.** Material deriva de la semilla también las paletas neutras, y con una semilla
naranja el fondo sale café. Grafito conserva la primaria, la terciaria y la de error, y pone a croma cero las
neutras y la secundaria —la de los contenedores de selección—, así que el color queda solo en el acento. Es una
propiedad del tema, `NEUTROS_GRISES` en `tema.ts`, y no de la semilla: otro acento sobre Grafito deja los grises en
gris. Lo comprueba `tema.test.ts` midiendo el croma de cada neutro.

**El acento del usuario entra por la semilla y no por encima.** Como semilla, Material lo pasa por la misma curva
y deriva de él una paleta medida. Como variable en línea —`--nb-accent`, que gana a cualquier hoja de estilos— se
pintaría sin medir, y por eso ese segundo camino se sigue cerrando cuando hay alto contraste puesto.

**Lo que lo hace cumplir**: `src/lib/ui/tema.test.ts` mide WCAG AA sobre los tokens `--m3c-*` de los siete temas
—no sobre `app.css`, que ya casi no se pinta—, comprueba que pedir alto contraste sube el contraste de verdad,
rechaza cualquier token de Material que se pinte sin que el generador lo derive y fija la carga del acento: sin
acento no se escribe nada en línea, una paleta que llega tarde no pisa a la elegida después y la guardada se pinta
sin esperar. `pnpm check:paquete` comprueba que la utilería siga fuera del paquete de arranque.

### 5.1.2 Un tema que aporta una extensión gobierna la apariencia entera

Un tema de extensión declara como mucho catorce colores `--nb-*` (la lista cerrada de `TOKENS_DE_TEMA`), y la
interfaz se pinta con las variables de Material. Escribir solo esos catorce en línea dejaría una pantalla a medias:
medido en WebKitGTK, un tema oscuro sobre una paleta clara dejaba texto a 1,13 de contraste.

**La regla:** un tema de extensión no es una capa de colores encima del tema del núcleo. Es *el* tema, y de él
sale todo lo demás.

| Qué | De dónde sale |
|---|---|
| El esquema, claro u oscuro | De su `--nb-surface`: bajo 0,2 de luminancia relativa es oscuro. El umbral no es 0,5 porque la luminancia no es lineal y el gris medio está en 0,216 |
| Los doscientos y pico tokens que **no** declara | Del tema neutro del núcleo de ese mismo esquema: Grafito si es oscuro, Papel si es claro. Neutro para no teñir con el matiz de otro tema lo que el autor no eligió |
| La paleta de Material | Se genera con su `--nb-accent` como semilla y en su esquema, y encima se ponen los colores que sí declara, cada uno sobre su equivalente de Material |
| La selección en Ajustes | **Una sola.** Elegir un tema del núcleo retira el de la extensión, y al revés |

**Por qué se pone encima en lugar de dejar que Material derive todo.** Porque un tema que se ve "parecido" no es el
tema: si las superficies y los textos salieran de la derivación, el autor habría elegido colores que nadie usa. Y por
qué no se deja solo lo que declara: porque son catorce de más de doscientos. Las dos mitades hacen falta.

**Dónde vive.** El tema es parte de `Apariencia`, y lo pinta `aplicarApariencia` como el resto: es lo que garantiza
que haya un único sitio que escribe color en línea y que sepa retirar lo que puso el tema anterior.
`temaDeExtension.ts` se queda solo con lo que hay que decidir antes de aplicar: el contraste. Lo sostienen pruebas de
unidad sobre las tres derivaciones y un recorrido que exige una sola selección y **ni un texto por debajo de 4,5** con
el tema puesto sobre una paleta clara, que es el caso donde la mezcla se veía. Comprobado rompiéndolo por los dos
lados.

### 5.2 Modos de lectura

Un modo es un **preajuste completo** de fondo, texto, contraste y temperatura, con nombre. Es la vía principal
de personalización del lector; el ajuste fino es secundario y siempre disponible. Todo lo que un modo fija se
puede retocar y guardar como modo propio del usuario.

El catálogo inicial: día, sepia, nocturno, alto contraste, y dos modos de tipo papel inspirados en las
pantallas NXTPAPER de TCL y en la tinta electrónica: monocromo de alto contraste, y color atenuado con filtro
de temperatura.

**Máxima personalización no es un panel de preferencias interminable.** Ese es el modo de fallo de esta
sección: libertad que se convierte en fricción. La defensa es que los modos con nombre resuelven el caso
normal, todo valor tiene un defecto sensato, y siempre existe una vía de retorno.

## 6. Lo emulado se declara como emulado

Un webview no controla el brillo del panel físico ni la temperatura de color del hardware. NB Books los emula
por composición sobre la superficie de lectura, y **la interfaz lo nombra como emulación**, nunca como control
del dispositivo.

Es la misma regla que el proyecto ya aplica a la degradación elegante y a los documentos no indexables: lo que
no está, se declara; no se disimula.

## 6.1 Cómo se hace cumplir todo lo anterior

Nada de este documento depende de que alguien lo recuerde en revisión.

| Regla | Mecanismo |
|---|---|
| Los colores salen del sistema de diseño, nunca de un componente | `check-ui.mjs` rechaza un color literal en un componente, **en cualquiera de sus formas**: código hexadecimal, función de color y nombre CSS. Hasta el 2026-09-01 solo miraba las dos primeras, y quince `color-mix(in srgb, white 4%, …)` habían entrado por el hueco. Desde ADR-014 se definen en `paletas.css`, que genera el mismo generador que usa la aplicación, y en `app.css`; `pnpm check:paletas` impide que el archivo y el generador se separen |
| El indicador de foco no se anula | `check-ui.mjs` rechaza `outline: none` |
| La paleta cumple WCAG 2.1 AA | `tema.test.ts` **calcula** el contraste sobre `app.css` y falla bajo el umbral |
| Ningún tema pierde un token | `tema.test.ts` compara el juego de tokens de los nueve bloques de paleta |
| Elegir oscuro y heredarlo dan lo mismo | `tema.test.ts` compara los dos bloques |
| Los puntos de corte no se duplican | `check-ui.mjs` rechaza consultas de medios por ancho |
| Ninguna zona queda sin nombre | `check-ui.mjs` exige `aria-label` o `aria-labelledby` en `nav` y `aside` |
| Ningún `role="application"` ni `tabindex` positivo | `check-ui.mjs` |
| Ninguna acción exclusiva del ratón | `check-ui.mjs` rechaza los gestos de puntero sin equivalente de teclado. El compilador de Svelte no los ve: un `div` con `role` y `tabindex` le parece correcto, y ese era el hueco |
| Los defectos de accesibilidad del marcado | Avisos del compilador de Svelte, bloqueantes con `--fail-on-warnings` |

La auditoría con motor de accesibilidad sobre vistas completas la hace `e2e/accesibilidad.spec.ts`. Una comprobación
automática detecta una fracción de los problemas reales; la verificación con lector de pantalla se describe en
[screen-readers.md](../developer/testing/screen-readers.md).

## 7. Rendimiento

Los [presupuestos](performance-budgets.md) gobiernan la interfaz igual que el runtime. Cambiar de modo, abrir
un panel o pasar página no puede salirse del presupuesto de fotograma. **Una animación que no cabe se elimina,
no se recorta.**

**La biblioteca se pasa por páginas, y por eso ciento cincuenta mil documentos no son ciento cincuenta mil
nodos.** El tamaño de página lo elige quien lee entre una lista cerrada, así que el presupuesto de nodos se cumple por construcción y no por una aritmética que haya que
demostrar. El orden y el filtro siguen siendo de SQL, sobre el catálogo entero: una página es una ventana sobre
un orden ya hecho, no un trozo que se ordena aparte.

**Y un estado de un control no puede mover su propia caja**, que es la sección 11 de
[ui-conventions.md](../design/ui-conventions.md): un `:hover` que se desplaza mueve también su área sensible,
así que el elemento se sale de debajo del ratón y el hover se enciende y se apaga sesenta veces por segundo. Ese
coste no aparece en ningún presupuesto de nodos y se ve igual de mal.

## 8. Puntos de extensión

Lo que una extensión puede aportar a la interfaz, y nada más:

* Un elemento en la navegación primaria.
* Un panel lateral.
* Una vista en el área principal.
* **Una vista dentro de Estudio**, junto a Notas, Anotaciones y Vocabulario. Ese sitio existe desde el
  principio y con el mismo aspecto que las nativas. La extensión devuelve **bloques tipados** y la
  pinta el núcleo: por eso una vista de extensión tiene el mismo foco, el mismo nombre accesible y el mismo
  presupuesto de fotograma que una nativa, y no depende de que su autor se acordara.
* Un elemento en un menú contextual, con una decisión que conviene dejar escrita:
  esas contribuciones se pintan **en la ficha del documento**, no en un menú que se abre con el botón derecho.
  La restricción 5 de la sección 9 prohíbe la acción exclusiva de ratón, y un menú contextual clásico lo sería.
  La ficha **es** el contexto de un documento y ya se alcanza con el teclado, así que la contribución tiene
  sitio sin inventar un gesto que dejaría fuera a quien no usa ratón.
* Un tema, o un modo de lectura, con una garantía que no depende de revisar nada:
  un tema solo puede cambiar colores de una **lista cerrada**, y en esa lista no hay ningún token para apagar el
  contorno del foco ni para quitar una sombra. Sus colores pasan además por los mismos umbrales de contraste que
  el sistema de diseño usa para sí mismo, y si alguno no llega **no se aplica nada**.

Esta lista es corta a propósito: el diseño promete estabilidad de API a terceros, y solo se puede prometer sobre
una superficie acotada. Ver [plugin-system.md](plugin-system.md).

## 9. Restricciones estrictas

1. **Ningún gestor de ventanas, dock, barra de tareas, fondo de escritorio ni conmutador de foco emulado.**
2. **`role="application"` no se usa sobre contenedores de la carcasa.** Solo dentro de un componente concreto
   que gestione su propio teclado y ofrezca una salida documentada.
3. **Ningún contenedor cuyo orden visual difiera de su orden en el DOM.** Es la causa raíz de que el teclado y
   el lector de pantalla se pierdan.
4. **Nada superpuesto y arrastrable sobre la superficie de lectura.**
5. **Ninguna acción exclusiva del ratón.** Ver [accessibility.md](accessibility.md).
6. **Ningún texto visible incrustado en el marcado.** Ver [i18n.md](i18n.md); lo hace cumplir
   `scripts/check-i18n.mjs`.
