# Convenciones de la Interfaz

> **Estado:** Aplicable a toda la interfaz.
> Las hace cumplir `scripts/check-ui.mjs` salvo donde se diga lo contrario.

Este documento existe para que NB Books se sienta **una sola aplicación** y no una colección de pantallas que
alguien juntó. Es la parte de la interfaz que no se decide vista por vista.

---

## 1. Nunca un emoji, nunca una letra haciendo de icono

**Prohibido en la interfaz:** emojis, emoticonos de texto, símbolos tipográficos usados como iconos —el aspa de
multiplicar, las flechas tipográficas, el punto medio, la marca de verificación— y letras sueltas dentro de un
botón.

Los motivos no son de gusto:

- Un emoji lo **dibuja el sistema operativo**. Cambia de forma, de color y de peso entre Windows, macOS y Linux;
  no hereda el color del tema; y en alto contraste se queda exactamente igual que estaba, que es donde más falta
  hace que cambie.
- Una letra dentro de un botón **la lee el lector de pantalla**. Un botón cuyo contenido es "B" se anuncia como
  "B".
- Un símbolo tipográfico depende de la fuente instalada y no tiene el mismo peso visual que un trazo dibujado.

Todo icono sale de [icons.md](icons.md) y se pinta con `Icon.svelte`. La lista es cerrada: un icono que no
está obliga a decidir qué significa antes de dibujarlo.

## 2. Títulos y etiquetas

**Los títulos y los subtítulos van en formato de título**: mayúscula inicial en las palabras significativas,
minúscula en las funcionales (artículos, preposiciones y conjunciones), salvo si abren el título.

| Correcto            | Incorrecto          |
| ------------------- | ------------------- |
| Visor de Texto      | Visor De Texto      |
| Visor de Texto      | Visor de texto      |
| Ajustes del Libro   | Ajustes Del Libro   |
| Anotaciones y Notas | Anotaciones Y Notas |

Palabras que van en minúscula dentro de un título: `a`, `al`, `ante`, `bajo`, `con`, `contra`, `de`, `del`,
`desde`, `e`, `el`, `en`, `entre`, `hacia`, `hasta`, `la`, `las`, `lo`, `los`, `ni`, `o`, `para`, `por`, `que`,
`según`, `sin`, `sobre`, `tras`, `u`, `un`, `una`, `y`.

**El resto del texto va con normalidad**: el cuerpo, las descripciones, los mensajes de error, las aclaraciones y
los textos de ayuda se escriben como se escribe en español, con mayúscula solo al principio.

Lo hace cumplir `check-ui.mjs` sobre los títulos de panel, los nombres de destino y los encabezados.

## 3. La interfaz no se explica a sí misma

Cada palabra en pantalla tiene que ganarse el sitio. Una vista que necesita un párrafo para explicarse está mal
diseñada, y el párrafo no lo arregla: lo tapa.

- **Sin textos de ayuda permanentes** junto a los controles. Si un control necesita explicación, o cambia de
  nombre o cambia de sitio.
- **Sin descripciones de lo evidente.** "Aquí verás tus libros" debajo de una cuadrícula de libros sobra.
- **Los estados vacíos dicen qué hacer, en una frase.** No dos.
- **Los mensajes de error dicen qué pasó y qué se puede hacer.** No piden disculpas.

La excepción son los estados en los que el usuario **pierde algo si no entiende**: una anotación reanclada de
forma aproximada, un documento no disponible, una acción que sale a la red. Ahí se explica, y se explica corto.

## 4. Consistencia: la misma cosa se ve y se comporta igual en todas partes

Es la regla que más se rompe sola, porque cada vista la escribe alguien con una idea distinta en la cabeza. Se
fija aquí y se hace cumplir con componentes compartidos, no con disciplina.

| Elemento              | Contrato                                                                                                  |
| --------------------- | --------------------------------------------------------------------------------------------------------- |
| Alineación del texto  | Siempre a la izquierda. El centrado se reserva a un estado vacío o a un mensaje que ocupa la vista entera |
| Números               | Alineados a la derecha y con cifras de ancho fijo, para poder compararlos de un vistazo                   |
| Títulos de vista      | Son el encabezado de la página para quien la oye. A la vista solo en el lector, donde es el del libro     |
| Acciones principales  | A la derecha de la barra superior, y la principal con el contenedor del acento                            |
| Acciones destructivas | Separadas del resto, y nunca la primera del grupo                                                         |
| Paneles               | Siempre a la derecha, siempre con título, siempre cerrables con Escape. Los menús, debajo de su control   |
| Listas y tablas       | Misma altura de fila, misma densidad, mismo comportamiento de selección y de teclado                      |
| Reproductores         | Mismo orden de controles y mismos atajos, sea voz, audiolibro o fondo sonoro                              |
| Estados de carga      | Reserva del hueco, nunca un salto de maquetación                                                          |
| Estados vacíos        | Icono, una frase y una acción. En ese orden                                                               |
| Foco                  | Siempre visible, con el mismo indicador en toda la aplicación                                             |
| Crecimiento           | **Siempre hacia abajo.** Lo que no cabe en una fila baja a la siguiente                                   |

### 4.1 Los menús de la barra superior

**Buscar, Importar, Colecciones y Estanterías se abren debajo del control que los abre**. Hasta entonces
eran paneles momentáneos del borde derecho: se pulsaba arriba en el centro y la respuesta aparecía al otro lado de
la ventana. Material 3 pone un menú bajo su control, que es donde el ojo lo busca después de pulsar.

| Regla | Por qué |
|---|---|
| Uno abierto a la vez | Nacen de la misma barra; abrir uno cierra el otro |
| Zona `complementary` con nombre, no diálogo | No es modal y no atrapa el foco, igual que un panel |
| Se cierran con Escape y al pulsar fuera | Con Escape el foco vuelve al control; al pulsar fuera, se queda donde se pulsó |
| El control lleva `aria-expanded` | Es lo que dice que está abierto a quien no ve el color ni la flecha |
| Nunca se salen de la ventana | El ancho se acota en CSS y el desvío se mide al abrir: un menú que nace de un control cercano al borde no puede quedar fuera del alcance |
| Sin sitio para el texto, el control se queda en su icono | Desaparece la palabra, nunca el control, y el nombre pasa al atributo |
| La búsqueda es una caja, no un botón | Es lo más usado de la barra; los resultados se abren al entrar en ella |

Lo implementan `Desplegable.svelte` y `Emergente.svelte`, y lo comprueban los recorridos de `e2e/flujo.spec.ts`
que abren cada menú a cinco anchos y con la letra al doble.

### Nada se desplaza en horizontal

**No hay carruseles que se arrastren, ni contenido detrás de un gesto lateral.** Lo que no cabe en una
fila baja a la siguiente, sea cual sea el ancho de la ventana; la vista crece hacia abajo y se recorre en
vertical, que es la única dirección en la que un desplazamiento no esconde nada.

Los motivos son dos y ninguno es estético:

- **Esconde contenido.** Un carrusel deja fuera lo que no entra en la primera pantalla, y quien no descubre el
  gesto no descubre lo que hay detrás. En una ventana estrecha, esconde casi todo.
- **Deja fuera a quien no hace el gesto.** Arrastrar en horizontal es un gesto de puntero. La alternativa —una
  rejilla que envuelve— no necesita ninguno.

**Las filas de Inicio se paginan, y por eso cumplen la regla.** Inicio usa filas horizontales por categoría. Caben las dos cosas porque lo que la regla prohíbe son sus dos
motivos y no la palabra fila: cada fila enseña **enteras** las obras que caben en su ancho, ninguna cortada por
el borde; las flechas que llevan a las siguientes son **botones**, que se alcanzan con el tabulador; y "Ver
todo" lleva a la lista entera. El contenedor no se desplaza, así que el recorrido que busca barras horizontales
escondidas sigue valiendo sin excepción. Lo implementa `vistas/Fila.svelte`.

**No hay excepciones, tampoco para los controles.** Una fila de controles que no envuelve, a 900 px, empuja un
selector fuera de la vista, y como la zona principal tiene `overflow: auto`, el desbordamiento produce **una barra
escondida en mitad de la vista**. Un control fuera de la ventana no está oculto: está perdido. Todo envuelve.

**Mecanismo.** Cuatro recorridos de `e2e/flujo.spec.ts` recorren los cuatro destinos a 640, 900 y 1440 px, y el
lector con sus paneles a 640, comprobando dos cosas: que ni el documento ni el cuerpo se desplacen en
horizontal, **y que ningún contenedor con `overflow-x` desplazable tenga contenido de más**. Esa segunda
comprobación es la que caza el caso escondido. Comprobado rompiéndolo: quitar el envolvimiento de la barra de la
biblioteca, o la columna explícita de la carcasa, hace fallar la prueba correspondiente.

## 5. Densidad y ritmo

- **Una sola escala de espaciado**, la de `app.css`. Los valores intermedios no existen.
- **Una sola escala tipográfica**, también la de `app.css`, y multiplica por `--nb-font-scale`.
- **Una sola escala de redondeo**, alineada con la de formas de Material y multiplicada por `--nb-radio-escala`,
  que es lo que da a cada tema su esquina.

> **Las tres escalas tienen guardián** en `check-ui.mjs`. Sin él, adoptar un sistema de diseño nuevo sustituye
> fácilmente la escala por valores en píxeles escritos a mano: ADR-014 cambia el origen de los **colores** y no dice
> nada de las otras tres.
>
> Lo que se pierde no es solo el ritmo. Las escalas están en `rem` y multiplican por los ajustes del usuario, y
> los literales no: con la tipografía al 200 %, 139 tamaños se quedaban quietos y 225 márgenes también, así que
> quien amplía la letra acababa con el texto pegado a los bordes. Y el redondeo por tema dejó de aplicarse, con
> lo que los siete temas perdieron su esquina. Una regla sin mecanismo dura hasta la primera prisa.
- **Cuatro alturas de fila** y ninguna más: compacta, normal, cómoda y de tarjeta.
- La aplicación **responde antes de terminar**: toda acción que tarde más de lo que dura un fotograma muestra su
  **avance** en el sitio donde ocurrió, no en una esquina. El avance de una importación va en el panel de
  importar; el de una consulta, en el panel de consulta. Su **desenlace** es otra cosa y va en la esquina: ver la
  sección 9.

## 6. Nada exige reiniciar

**Ningún cambio de la aplicación se aplica al reiniciar, ni al volver a entrar en la vista, ni al reabrir el
documento. Se aplica cuando se hace.** Vale para el tema, el acento, la escala tipográfica, el alto contraste, el
movimiento reducido, el idioma, la composición del libro y el modo de lectura.

No es una comodidad. Quien sube la escala tipográfica lo hace porque **no está leyendo bien ahora**; quien pone
alto contraste, igual. Decirle que reinicie es decirle que aguante. Y un ajuste que se guarda sin verse es peor
que uno que no existe: parece que funciona, así que quien lo usa cree que su problema no tiene arreglo.

La única excepción admisible es un plugin que cambie algo estructural, y se decidirá cuando existan plugins. No
Se usa como coartada para nada de lo que hay hoy.

**Cómo se cumple.** SQLite es la fuente de verdad y responde con el valor guardado; la interfaz aplica **ese**
valor al DOM en cuanto llega, no el que creía estar escribiendo (regla 2 de
[ADR-010](../adr/010-settings-persistence.md)). La caché del primer pintado se reescribe en la
misma operación, que es lo que hace que al arrancar de nuevo tampoco haya destello.

**Mecanismo.** Dos recorridos de `e2e/flujo.spec.ts`: uno cambia el tema y comprueba `data-theme` en la raíz sin
recargar nada, y otro sube la escala tipográfica y enciende el alto contraste y comprueba que el token y el
atributo cambian al momento. Los dos eligen un valor **distinto del que ya está puesto**: comprobar contra el
inicial deja pasar una prueba que no prueba nada, y así estaba escrita la del tema hasta que una mutación lo
destapó.

## 6.1 La portada de una obra sin portada

**Un hueco gris con un icono dentro no es una portada, es una ausencia.** Con el corpus real dentro, 982 obras de
152.080 no traen imagen, y esas son las que ve quien recorre la biblioteca hasta el fondo. Cuando no hay imagen se
dibuja una, y estas son sus reglas:

| Regla | Por qué |
|---|---|
| El color sale del **título**, nunca al azar | La misma obra tiene siempre la misma portada, aquí y en cualquier otra pantalla. Con un azar, cada repintado cambiaría el color y la rejilla dejaría de ser reconocible |
| Del título sale **solo el tono**; la luminosidad y el croma los pone el tema | Así una rejilla llena no desentona al cambiar de tema, y el contraste con la tinta está fijado por el tema y no por el azar de un título |
| Los tonos **se reparten** | El título pasa por FNV-1a y el resultado se multiplica por el ángulo dorado. Con la versión anterior -resto módulo 360 en cada letra- títulos parecidos caían en el mismo color: de veinticinco obras salían seis tonos distintos, y una biblioteca así es medio camino hacia el hueco gris |
| El título **se lee**, y el autor también | Un recorrido mide el contraste de cada portada generada contra su fondo y exige 4,5. El autor va debajo del título, con una línea entre los dos, no en letra diminuta pegada al borde |
| Ni un color escrito a mano | Todo sale de `oklch` con la luz y el croma del tema. Ver [ADR-014](../adr/014-design-system.md) |

**Y el banco de pruebas tiene que poder enseñarlas.** El simulador de la suite sirve una portada por obra para
medir el coste de descodificarlas, y la opción `sinPortadas` ejercita la portada generada, que es lo que ve buena
parte de una biblioteca real.

## 7. Movimiento

Se admite solo cuando **transporta significado** —dirección, posición o continuidad— y siempre dentro del
presupuesto de fotograma. Con `prefers-reduced-motion`, desaparece por completo y su ausencia no cambia dónde
está nada.

**Esta no es una aplicación de música.** Lo que se lee pide calma, así que el movimiento es escaso y lento, y
cada uno tiene que poder explicarse en una frase:

| Qué se mueve                                      | Qué significa                              |
| ------------------------------------------------- | ------------------------------------------ |
| El indicador del destino activo, deslizándose     | De dónde vengo y a dónde voy               |
| El hilo de progreso del libro en curso, creciendo | Que la lectura avanza                      |
| El panel que entra desde su borde                 | De dónde ha salido y por dónde se va       |
| La página al pasar                                | La dirección y la posición en el documento |

**Las curvas no se pasan del final**. `--nb-easing` era `cubic-bezier(0.34, 1.56, 0.64, 1)`,
que sobrepasa el destino y vuelve, y sobre el ancho de la barra lateral eso eran literalmente los "rebotes" que
Se describió. Ahora es la curva estándar de Material 3, `cubic-bezier(0.2, 0, 0, 1)`, con su nombre propio
`--nb-easing-estandar`. Una curva con sobrepaso es expresiva, y aquí nada lo es.

**Y un ancho no se anima.** Cambiar el ancho de una columna la recompone en cada fotograma, y con la biblioteca
detrás eso es recomponer cien tarjetas sesenta veces por segundo. Plegar la barra lateral se hace en tres
tiempos: su contenido se funde, la barra cambia de forma **una vez** con la columna desplazada exactamente lo que
Se movió su borde, y la columna vuelve a su sitio con una transformación. La barra tiene el color de la ventana,
así que lo que el ojo sigue es el borde del lienzo, y eso es lo que se mueve. Medido en Chromium con la CPU a un
cuarto y cien obras, en seis cambios: la disposición bajó de 455 a 108 ms y los fotogramas de más de 34 ms, de
25 a 8. Lo comprueba `e2e/flujo.spec.ts` leyendo que la barra no tenga transición de ancho.

Lo que **no** se mueve: nada al cargar, nada al pasar el ratón por encima salvo el cambio de color, y ningún
elemento que aparezca solo para llamar la atención. Un movimiento que no significa nada es ruido, y en una
aplicación donde la gente pasa horas leyendo, el ruido cansa.

## 9. La aplicación contesta: qué se anuncia y dónde

> Añadida, después de comprobar que hasta entonces **la aplicación no decía nunca qué había pasado**.

### El defecto que la origina

Cada `catch` de la interfaz escribía el mensaje en una variable, y esa variable **solo se pintaba dentro del
panel de estado del runtime**, que a su vez solo se anuncia cuando algún componente está en mal estado. Medido
sobre la aplicación real: abrir un documento que falla, guardar una nota que falla o importar una carpeta que
falla dejaban la pantalla **idéntica** a si hubiera ido bien. Lo confirmado sufría lo mismo por el otro lado:
anotar, resaltar, marcar o descargar no dejaban ninguna señal, así que la única forma de saber si la pulsación
había llegado era ir a buscar el resultado a otra pantalla.

### La regla

**Toda acción del usuario que no se vea a sí misma termina con una frase.** Va en la esquina inferior derecha,
sobre todo lo demás, y no bloquea nada.

| Qué pasó                                                       | Dónde se dice                                               | Cuánto dura              |
| -------------------------------------------------------------- | ----------------------------------------------------------- | ------------------------ |
| Una acción **en curso**                                        | En el sitio donde ocurre: su panel, su botón, su fila       | Hasta que termina        |
| Una acción que **salió bien** y cuyo efecto no está a la vista | Esquina, en verde de acento                                 | Cuatro segundos          |
| Algo que hay que saber y no es un fallo                        | Esquina, neutro                                             | Seis segundos            |
| Un **fallo**                                                   | Esquina, en color de peligro, con el código canónico debajo | **Quince segundos**, y la cuenta se detiene mientras se mira |

### Los tres porqués que la gobiernan

1. **Un fallo dura más, y la cuenta se detiene mientras se mira.** Un acierto se lee de reojo; un fallo hay que
   poder volver a leerlo y hay que poder copiar su código, así que dura quince segundos y **el tiempo no corre
   mientras el puntero está encima o el foco dentro**. Un aviso que no caduca nunca acaba llenando la esquina, y un
   rincón permanentemente lleno deja de leerse; la pausa permite caducar sin perder lo que se quería leer.
2. **Nunca se enseña `error.message`.** En un `NbError` el mensaje **es** el código, y el texto en palabras lo
   pone `traducirError`. El código va en la segunda línea, en monoespaciada y seleccionable: dice _qué fue_, no
   _qué pasó_.
3. **Lo que se ve no se anuncia.** Cambiar de tema no lleva aviso: el cambio es la propia pantalla. Anotar sí,
   porque la anotación se guarda en un panel que puede estar cerrado y en lectura inmersiva no hay panel alguno.
   Un avisador que confirma lo evidente enseña a ignorar los avisos, y entonces el que importa tampoco se lee.

### De dónde sale

Del `AdwToastOverlay` de [Foliate](https://github.com/johnfactotum/foliate): un aviso corto, no modal, que no
roba el foco y que se va solo. Es la única pieza de su interfaz que se copia tal cual, porque el problema es
idéntico y su respuesta lleva años probada en manos de gente.

### Mecanismo

`src/lib/ui/notificaciones.ts` es la cola, y es pura: la duración por clase, el tope de tres a la vez y el
recuento de lo repetido se comprueban en `notificaciones.test.ts` sin navegador. Que el aviso **se vea** lo
comprueban tres recorridos de `e2e/flujo.spec.ts`: uno confirma un marcador, otro provoca un fallo del runtime y
comprueba que se lee sin abrir ningún panel, y el tercero espera cinco segundos para comprobar que el fallo sigue
ahí y que se puede cerrar.

## 10. Estados de interacción: cinco, no dos

> Añadida.

Todo control acepta cinco estados y **ninguno es opcional**: reposo, ratón encima, pulsado, con el foco y, si lo
tiene, elegido o abierto.

El que faltaba en todo el sistema de diseño era **pulsado**. Sin él, sobre un puente lento —el escenario 2 de
[chaos.md](../developer/testing/chaos.md)— no hay forma de saber si la pulsación llegó, y lo que hace la gente
entonces es pulsar otra vez. Faltaba también el de **elegido** en los botones de solo icono: la cuadrícula y la
lista de la biblioteca se veían exactamente igual estuviera puesta la que estuviera.

**Abierto y elegido no son lo mismo, aunque se vean igual.** Un botón que despliega un panel lleva
`aria-expanded`; uno que expresa una elección vigente lleva `aria-pressed`. La distinción no es de vocabulario:
`Panel` deja pasar los controles con `aria-expanded` al cerrarse por pulsación fuera, porque son los que ya
alternan por su cuenta. Cambiar uno por otro hizo que el mismo clic abriera el panel y lo cerrara a
continuación, y ninguno de los cinco paneles del lector llegaba a verse con el ratón. Lo fija el recorrido
"abrir un panel del lector con el ratón lo deja abierto" de `e2e/flujo.spec.ts`.

## 11. Un estado de un control no puede mover su propia caja

> Añadida, después de ver la interfaz temblar.

El estado "ratón encima" **no desplaza el elemento que lo recibe**. Puede agrandarlo, oscurecerlo, elevarlo con
sombra o cambiarle el borde; lo que no puede es moverlo.

El motivo es mecánico y no estético. `transform: translateY(-4px)` mueve la caja **y su área sensible**: con el
puntero en los últimos píxeles del elemento, levantarlo lo saca de debajo del ratón, el hover se apaga, el
elemento vuelve a su sitio y el hover se enciende otra vez. A sesenta fotogramas por segundo eso se ve temblar, y
no es un defecto de pintado: es el hover encendiéndose y apagándose de verdad. Estaba en Inicio, en la
biblioteca y en Descubrir a la vez.

`scale` por encima de 1 sí vale, porque solo puede **agrandar** el área y el ciclo no puede darse. La elevación
la da la sombra, que es lo que se lee como relieve; el desplazamiento no aportaba nada que la sombra no dijera
ya.

La regla es del sujeto del selector, no del selector entero: `.tarjeta:hover .portada` mueve la portada, y el
área de la tarjeta no cambia, así que ahí sí se puede. `:active` tampoco entra: durante una pulsación el puntero
está capturado.

**Se hace cumplir dos veces, y no es redundante.** `check-ui.mjs` mira la regla de CSS y rechaza cualquier
desplazamiento en un `:hover` cuyo sujeto sea el hovereado; `e2e/rendimiento.spec.ts` mide la caja ya compuesta.

### 11.1 Y tampoco puede cambiar una propiedad que fuerza disposición

> Añadida, después de medir por qué Inicio parpadeaba con la rueda.

La regla de arriba mira si el elemento **se mueve**. Esta mira **lo que cuesta**. Un `:hover` que cambia `width`,
`padding`, `font-size` o cualquier otra propiedad que rehaga la caja obliga al motor a recalcular la disposición
de ese subárbol, y a comprobar si algo por encima cambia de tamaño. Ocurre al entrar y otra vez al salir.

Medido en WebKitGTK sobre los dos textos de la tarjeta de "sigue leyendo", que pasaban de
`width: 100%` a `max-content`: **sesenta encendidos y apagados cuestan 392 ms, 6,5 ms cada uno**, sobre un
presupuesto de fotograma de 16. Y no hace falta que nadie mueva el ratón adrede: con el puntero quieto y la rueda
girando, el contenido pasa por debajo y el hover entra y sale de un elemento tras otro. En Inicio hay cuarenta y
ocho candidatos y veinticuatro a la vez en pantalla.

**Lo que esa cifra no dice, y conviene no dárselo por dicho.** No dice que esto sea la causa del parpadeo que se
describió al mover la rueda deprisa. Se intentó medir simulando el recorrido del puntero, y el intento no sirvió:
`document.elementFromPoint` obliga al motor a resolver la disposición, así que **la sonda costaba más que lo
medido**. Con el control puesto —recorrer llamando a la sonda y sin cambiar ningún estilo— salió que la sonda se
llevaba el 78 % de la diferencia, la sombra un 9 % y esto un 5 %. Un instrumento cuyo propio coste domina la
señal no decide nada, igual que el hueco entre fotogramas dentro de una ventana GTK. La regla se sostiene por los
6,5 ms, que sí están medidos y sí se comparan con el presupuesto; la causa del parpadeo sigue abierta.

A diferencia de la 11, esta **no** distingue si el sujeto es el hovereado: cambiar la caja de un descendiente
cuesta igual, y encima puede reajustar a sus antepasados.

Color, sombra, contorno, `opacity`, `filter` y `transform` no entran: se componen o se repintan, y ninguno rehace
la disposición. Lo hace cumplir `check-ui.mjs`.
Comprobado rompiéndolo: un `translateY(-2px) scale(1.01)` sobre una tarjeta alta lo detecta el guardián y **no**
lo detecta la prueba, porque el escalado devuelve el borde inferior justo a donde estaba; un `translateY(-4px)`
los detecta los dos. El guardián impide escribirlo, la prueba comprueba lo que se ve.

### 11.2 Un pseudoelemento absoluto necesita que su elemento esté posicionado

> Añadida, después de que un rectángulo del tamaño de la vista tapara Inicio.

`position: absolute` se coloca contra el **antepasado posicionado más cercano**. Si el elemento que lleva el
pseudoelemento no lo es, ese antepasado acaba siendo cualquier otro —normalmente uno enorme— y un `inset: 0` que
debía cubrir una tarjeta pasa a cubrir media aplicación.

Pasó al mover la sombra del ratón a un pseudoelemento: `.nb-ficha` y `.nb-sugerencia` lo comparten pero tienen
bloques de reglas separados, y el `position: relative` se añadió solo al primero. Al pasar el ratón por "Quizá te
interese" aparecía un rectángulo redondeado del tamaño de la vista, con su sombra difuminada asomando contra la
barra lateral y la superior.

**Se percibía como un parpadeo**, y esa es la parte que conviene recordar: el cambio duraba un fotograma y, una
vez aplicado, no llamaba la atención. Quien lo sufría lo describió como imposible de capturar en pantalla. No era
un problema de rendimiento; era un elemento en el sitio equivocado.

Lo rechaza `check-ui.mjs`, y hace falta un guardián propio porque **es una interacción entre dos reglas**: cada
una por separado es correcta, y las demás comprobaciones miran una regla cada vez.

## 12. Nada se anima en reposo, y el cromo que se apoya sobre lo que se desplaza es opaco

> Añadida, después de medir por qué seguía temblando.

**En reposo, nada se mueve.** Ni un fondo, ni un texto, ni una capa detrás de nada. Una animación permanente
compite por el mismo fotograma que el desplazamiento, y el desplazamiento siempre gana en importancia: quien
recorre su biblioteca está haciendo algo, y quien mira un degradado moverse despacio no.

No es una cuestión de qué se anima. El fondo vivo de la carcasa ya se había pasado de `background-position` a
`transform` —de repintar a componer, que es mucho más barato— y **seguía sin caber**. Medido con
desplazamiento continuo y cien obras por página: con el fondo, 13 fotogramas de 77 pasaban de 20 ms y el peor
llegaba a 31,6; sin él, 3 de 87 y 25,1. Lo mismo valía para el marquee que enseñaba el título completo al pasar
el ratón: en bucle infinito, sobre una rejilla de cincuenta tarjetas, es una animación permanente por cada
elemento que se roce.

**No hay marquesinas de texto.** Recortar una animación que no cabe en lugar de eliminarla es lo que prohíbe la
línea de abajo, y lo caro de una marquesina no es la animación sino el cambio de `width: 100%` a `max-content` que
la acompaña: 6,5 ms de disposición cada vez que el ratón entra o sale. Ver la sección 11.1.

Se retira de los cuatro sitios. Lo que queda es el corte con puntos suspensivos, que sí dice que el título no
cabe, y el título entero a un clic en la ficha. Es el argumento que `Cover.svelte` ya tenía escrito para las
portadas —movimiento que no transporta significado y que solo llega con el ratón, que no puede ser vía única— y
tenerlo aplicado en un componente y contradicho en otros cuatro era la clase de contradicción que la regla 1 de
`AGENTS.md` no admite.

[performance-budgets.md](../architecture/performance-budgets.md) ya lo decía: **una animación que
no cabe se elimina, no se recorta**.

**Y el cromo permanente que se apoya sobre una zona que se desplaza va opaco.** Un color translúcido hay que
mezclarlo con lo que hay debajo, y la mezcla se rehace cada vez que lo de debajo se mueve. Es el mismo problema
del desenfoque en menor grado, y la misma respuesta. Medido con el fondo animado ya retirado, el peor fotograma
bajaba de 25,1 a 22,7 ms al hacer opaco el banner.

Esto no alcanza a un panel **momentáneo**, que se abre encima y se va: ahí la translucidez dura lo que dura el
panel y sirve para decir que lo de debajo sigue estando. Ni a un indicador de carga, que se anima mientras tapa
a lo que espera y desaparece con ello.

### 12.1 Cómo se dice que algo está cargando

**El indicador de carga espera 300 ms antes de aparecer.** Por debajo no se pinta nada; por encima se pinta y se
va con lo que esperaba.

La tensión era real y estaba sin resolver: un indicador se mueve, y aquí nada se anima en reposo. No hay
contradicción formal -cargando no se está en reposo- pero faltaba decir **cuándo**, y de las tres salidas que
existen -marcador estático, barra determinada, o no enseñar nada porque no se tarda- la tercera era la que mejor
encajaba con este producto **y dependía de que las esperas fueran cortas de verdad**. Ahora lo son:
los comandos no bloquean la ventana, y las consultas van acotadas. Abrir el panel de
estanterías sobre 55.740 autores cuesta 6 ms.

Así que la respuesta no es ni prohibirlo ni permitirlo siempre: es **esperar**.

**De dónde sale el umbral**, que es lo que lo hace una regla y no un gusto. Un cambio de página de la biblioteca
cuesta entre 90 y 233 ms, medido en 4.3.5 de
[performance-budgets.md](../architecture/performance-budgets.md), y el panel de estanterías 6, en
4.3.9. Un umbral por debajo de eso destellaría en las dos operaciones más frecuentes de la aplicación, y un
destello llama la atención sin decir nada. Por arriba, el presupuesto de *time to interactive* son 800 ms, así
que a partir de ahí ya hay que decir algo o el silencio parece un fallo. **300 ms cae entre las dos cosas.**

**Y la espera vive dentro del componente**, no en cada vista. Una vista que quiera decir que está cargando pone
`Spinner` y ya está esperando; no hay forma de poner un indicador sin umbral sin escribir otro componente, y eso
lo rechaza `check-ui.mjs`, que no admite ninguna animación en bucle fuera de ese archivo. La regla se cumple por
construcción y no por acordarse.

Con `prefers-reduced-motion` el indicador **sigue estando y deja de girar**. Quien pide menos movimiento no pide
menos información.

Lo comprueban dos recorridos de `e2e/flujo.spec.ts`, y hacen falta los dos: uno con una espera larga, que exige
que aparezca, y otro con una de 100 ms, que exige que **no llegue a verse**. El segundo mira con un observador
desde antes de pulsar, porque comprobarlo después no vale para nada: cuando la aserción corre, la carga ya
terminó y la prueba pasaría igual con umbral y sin él.

**Las dos reglas se comprueban en `e2e/rendimiento.spec.ts`, y son estructurales y no de tiempo.** Se escribió
antes una prueba que medía el hueco entre fotogramas y pasaba en solitario y fallaba con las demás en paralelo:
cuatro navegadores en la misma máquina contaminan la medida, y una prueba intermitente es peor que ninguna. La
medida es el instrumento de diagnóstico; el mecanismo es la regla.

## 8. Qué se hace cumplir automáticamente hoy

| Regla                                                                                                     | Mecanismo                                  |
| --------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| Ningún emoji ni símbolo usado como icono en un componente                                                 | `check-ui.mjs`                             |
| Ninguna letra suelta como contenido de un botón                                                           | `check-ui.mjs`                             |
| Formato de título en títulos de panel y nombres de destino                                                | `check-ui.mjs`                             |
| Ningún color literal (salvo variables `--m3c-*`), ningún `outline: none`, ningún punto de corte duplicado | `check-ui.mjs`                             |
| Ningún espaciado, redondeo ni tamaño de letra fuera de su escala                                          | `check-ui.mjs`                             |
| Ningún token `--m3c-*` que se pinta sin que el generador lo derive                                        | `src/lib/ui/tema.test.ts`                  |
| La paleta de Material cumple WCAG AA **en los tokens que se pintan**, y el alto contraste contrasta más   | `src/lib/ui/tema.test.ts`                  |
| Ningún gesto de puntero sin equivalente de teclado                                                        | `check-ui.mjs`                             |
| Ninguna clase propia (`nb-*`) que se pinta se queda sin regla de CSS                                      | `check-ui.mjs`                             |
| Ningún `:hover` que desplace la caja del elemento que lo recibe                                           | `check-ui.mjs` y `e2e/rendimiento.spec.ts` |
| Nada animándose con la aplicación en reposo                                                               | `e2e/rendimiento.spec.ts`                  |
| Ninguna animación en bucle fuera del indicador de carga                                                   | `check-ui.mjs`                             |
| El indicador de carga espera 300 ms y no destella en una espera corta                                     | `e2e/flujo.spec.ts`                        |
| Cromo permanente opaco sobre lo que se desplaza                                                           | `e2e/rendimiento.spec.ts`                  |
| El catálogo de iconos y su documentación no divergen                                                      | `iconos.test.ts`                           |
| Un ajuste se ve sin recargar, y la caché del primer pintado queda al día                                  | `e2e/flujo.spec.ts`                        |
| Ningún panel se sale de la ventana ni corta su contenido, en cuatro anchos                                | `e2e/flujo.spec.ts`                        |
| Un fallo del runtime se ve sin abrir nada, dura quince segundos y se detiene mientras se mira             | `e2e/flujo.spec.ts`                        |
| Una acción que sale bien y no se ve a sí misma se confirma                                                | `e2e/flujo.spec.ts`                        |
| La cola de avisos no se apila, no repite y caduca por clase                                               | `notificaciones.test.ts`                   |
| Un ajuste del libro y del kit accesible cambia el texto al momento                                        | `e2e/flujo.spec.ts`                        |
| La vista dividida se abre, cambia de mitad y reparte el ancho                                             | `e2e/flujo.spec.ts`                        |
| La barra del lector no ofrece lo que no haría nada                                                        | `teclado.test.ts` y `e2e/flujo.spec.ts`    |
| Un botón de panel se abre con el ratón y se queda abierto                                                 | `e2e/flujo.spec.ts`                        |
| Una imagen del libro se pide por el protocolo de assets, no al servidor                                   | `e2e/flujo.spec.ts`                        |

La fila del hover que desplaza tiene además su prueba, `tests/hover-que-no-desplaza.test.ts`, que comprueba que el
guardián la aplica también sobre CSS formateado.

Lo que **no** se comprueba solo: la alineación, la densidad y el ritmo de las secciones 4 y 5. Dependen de que las
vistas usen los componentes compartidos, y eso se revisa leyendo. Decir lo contrario sería exactamente el tipo de
afirmación que este proyecto no se permite.
