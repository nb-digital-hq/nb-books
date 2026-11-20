# La Experiencia de Lectura: qué hace quien lee y qué hacen los lectores que existen

> **Estado:** Vigente. La investigación de voz y audiolibros vive aparte, en
> [audio-and-voice.md](audio-and-voice.md). Es la base de
> [ADR-013](../../adr/013-information-architecture.md).

Este documento existe para **no volver a pensar lo ya pensado**: lo que no está escrito se vuelve a discutir cada
seis meses. Recoge tres cosas: **qué hace de verdad una persona mientras lee**, **qué ofrecen los lectores que ya
existen**, y **qué se adoptó, qué se rechazó y por qué**. La tercera es la única que caduca; cuando caduque, se
revisa con el mismo método.

**Método.** Revisión de la documentación de funciones de nueve lectores de referencia, de la literatura sobre
comportamiento lector y de las quejas públicas de los usuarios de la herramienta dominante. Las fuentes están al
final, con enlace. Si una afirmación no tiene fuente o no se deduce de un documento del proyecto, no está aquí.

---

## 0. El criterio rector: el libro físico, y después la magia

> **Gobierna las secciones siguientes**: cuando una decisión de producto del lector no esté resuelta por este
> documento, se resuelve con este criterio antes que con una comparación de funciones.

El lector de NB Books no se juzga contra la lista de funciones de otro lector. Se juzga contra **un libro
físico**, en dos tiempos y en este orden:

**Primer tiempo: nada de lo que hace un libro de papel puede faltar.** Un libro de papel no es una tecnología
pobre; es una tecnología perfeccionada durante quinientos años, y todo lo que hace, lo hace de una forma que el
lector da por descontada:

* Si pegas una nota adhesiva, **se ve**, sobresale del canto, y sigue ahí hasta que la quites.
* Si subrayas con lápiz, **la raya se queda en la página**, no en un cajón aparte.
* Si resaltas con un marcador, el color está **sobre las palabras**, no en un índice.
* Si dejas un separador, **asoma por arriba** y se ve sin abrir el libro.
* Si escribes en el margen, tu letra convive con el texto.
* Y en cualquier momento puedes **cerrar el libro y mirarlo por el canto**: los colores, los adhesivos y el
  separador te dicen dónde has estado, cuánto llevas y qué te importó, sin abrirlo.

Lo que tienen en común los seis es que **la marca es visible en el sitio donde se hizo**. Una aplicación que
guarda la anotación y no la pinta no ha implementado la anotación: ha implementado la mitad que se puede
comprobar con una prueba.

**Segundo tiempo: estamos en una pantalla, así que hay magia, y no usarla es desperdiciarla.** El papel no puede
leerte en voz alta, ni cambiar de tipografía cuando te cansas la vista, ni buscar una palabra en toda tu
biblioteca, ni traducir un párrafo sin levantarte, ni recordar por dónde ibas en cuarenta libros a la vez, ni
llevarte de una nota a los seis pasajes que la sostienen. Nosotros sí. **Que no falte nada del papel es el
suelo, no el techo.**

### 0.1 El listón, aplicado al lector

Esta tabla se comprueba antes de dar por terminado cualquier cambio del lector.

| Lo que hace el papel | Qué exige en pantalla | Cómo lo cumple NB Books |
|---|---|---|
| El adhesivo se ve, y sigue ahí | La nota se pinta **en el texto**, en su sitio, hasta que se quite | La nota se pinta sobre sus palabras con subrayado punteado, para no depender del color, y señalarla la abre |
| El resaltado está sobre las palabras | El color se pinta sobre el pasaje, en la página | Con la API de resaltado de CSS, medida en WebKitGTK |
| El subrayado se queda en la página | Marca de línea, distinta del resaltado de bloque | Clase propia: raya continua y sin fondo |
| El separador asoma | La posición de lectura se ve **en el documento**, no solo como cifra | Una cinta en el margen del párrafo |
| Se escribe en el margen | Nota corta anclada visible junto al texto | Dónde está la nota se ve en el texto; lo que dice se lee al señalarla |
| El canto cuenta la historia del libro | Un mapa del documento con marcas, resaltados y posición | El mapa del libro, arriba del índice |
| Se lee en el orden que uno quiere | Empezar a leer o a escuchar **desde donde uno señala** | La voz empieza por el cursor o por el primer bloque a la vista |

Las cuatro primeras filas son la misma exigencia con cuatro caras: **lo que el lector marca se ve donde lo
marcó**. Una aplicación que guarda la marca y no la enseña es, en palabras de quien la usa, "muy secreta: haces un
marcador y no sabes qué marcaste".

### 0.2 Por qué esto no contradice la sección 1.1

La sección 1.1 dice que resaltar no mejora el aprendizaje y que la interfaz debe empujar hacia la nota. **Sigue
vigente y no se toca.** Lo que dice esta sección es distinto: no que se facilite resaltar por encima de anotar,
sino que **lo que ya se ha marcado se vea**. Una nota invisible no empuja hacia la nota; empuja hacia no volver
a escribir ninguna, porque quien la escribió no la encuentra donde la dejó. Las dos secciones piden lo mismo:
que la anotación constructiva sea la conducta barata. Pintarla es lo que la hace barata de recuperar.

---

## 1. Qué hace una persona mientras lee

No "qué funciones usa": qué hace. La diferencia importa, porque el producto se diseña contra la conducta y no
contra la lista de funciones de la competencia.

| Conducta | Qué exige de la interfaz | Cómo lo resuelve NB Books |
|---|---|---|
| Retomar donde lo dejó | Que la aplicación **abra por ahí**, no que haya que buscarlo | "Seguir leyendo" en Inicio y posición guardada por Locator |
| Ajustar el texto a sus ojos y a la luz | Tipografía, cuerpo, interlineado, márgenes, fondo | Ajustes del libro (`T`) y temas |
| Tropezar con una palabra que no conoce | Consulta **sin salir del texto** y sin perder la posición | Diccionarios StarDict sin conexión (`C`) |
| Tropezar con un idioma que no domina | Traducción de la selección, en el sitio | Traductor en línea con consentimiento |
| Querer saber quién era ese personaje | Índice de entidades con dónde aparecen | Nombres del libro, local y sin IA |
| Marcar algo para volver | Que marcar cueste **menos** que no marcar | Marcador con una tecla (`B`) |
| Escribir qué le pareció | Nota anclada, recuperable sin abrir el libro | Notas ancladas y vista de anotaciones en Estudio |
| Seguir el texto con la voz | Lectura en voz alta con seguimiento visual | Voces del sistema, voz neuronal y audio preparado |
| Poner un fondo sonoro para concentrarse | Que no haya que salir de la aplicación | Extensión propia de fondo sonoro (`N`) |
| Saber cuánto lleva y cuánto le falta | Progreso real y ritmo propio | Progreso y "unos 40 min más, a tu ritmo" |
| Comparar dos textos | Vista dividida | Vista dividida (`D`) |
| Encontrar algo que leyó y no recuerda dónde | Búsqueda en el texto, no solo en el catálogo | Búsqueda de texto completo y semántica opcional |

### 1.1 Un hallazgo incómodo que cambia el diseño

La literatura sobre comprensión lectora es clara en dos puntos, y los dos van contra el diseño obvio:

1. **Resaltar, por sí solo, no mejora el aprendizaje.** Lo que funciona es la anotación *constructiva*: escribir
   algo propio. El resaltado sin texto produce la sensación de haber trabajado sin el trabajo.
2. **Quien lee en digital casi no subraya ni marca**, mucho menos que en papel. La causa razonable es la
   fricción: seleccionar con precisión en una pantalla cuesta más que pasar un lápiz.

**Consecuencia de diseño, vinculante:** anotar tiene que costar **menos** que resaltar, y la interfaz debe
empujar hacia la nota y no hacia el color. Un lector que solo facilita resaltar está optimizando la conducta que
no sirve.

> **Cómo se cumple.** De la única forma que no es una opinión: **escribir una nota no
> obliga a resaltar antes**. Casi todos los lectores exigen resaltar y después elegir "añadir nota", con lo que
> la conducta que sirve cuesta siempre una acción más que la que no. Aquí la nota sale directa de la selección y
> responde a la tecla de aceptar; el resaltado tiene la suya. Hay una prueba que lo afirma sobre el registro de
> comandos, así que no se puede deshacer sin que algo falle.

---

## 2. Qué ofrecen los lectores que ya existen

Nueve referencias, elegidas para cubrir el rango entero: desde el gestor dominante hasta el lector accesible de
nicho, pasando por el competidor directo más cercano.

| Producto | Qué es | Por qué está aquí |
|---|---|---|
| Calibre | Gestor de biblioteca de escritorio | Es el dominante y el punto de comparación inevitable |
| KOReader | Lector de tinta electrónica, muy configurable | Es el más completo en funciones de lectura pura |
| Thorium Reader (EDRLab) | Lector de escritorio centrado en accesibilidad | Es la referencia de accesibilidad, y el producto la exige |
| Readest | Lector moderno multiplataforma, Tauri v2 | **Es nuestro competidor directo más cercano**: misma pila técnica |
| Foliate | Lector GTK para GNOME | Referencia de integración con el escritorio |
| Moon+ Reader | Lector de Android, muy pulido | Referencia de acabado visual y animación |
| ReadEra, Librera | Lectores de Android | Referencia de sencillez y de compatibilidad de formatos |
| Kindle | Ecosistema cerrado dominante | Fija las expectativas del usuario común |
| Bookly, StoryGraph, Bookmory | Rastreadores de lectura | Delimitan qué es "estadísticas de lectura" como producto |

### 2.1 Lo que comparten los cuatro lectores serios

Esto es **base**, no diferenciación. Un lector que no lo tenga se percibe incompleto:

| Función | Calibre | KOReader | Thorium | Readest | Foliate | Kindle |
|---|---|---|---|---|---|---|
| Diccionario sobre la selección | — | Sí (StarDict) | — | Sí | Sí (Wiktionary, dictd) | Sí |
| Wikipedia sobre la selección | — | Sí | — | Sí | Sí | Sí |
| Traducción de la selección | — | Sí | — | Sí (DeepL) | Sí | Sí |
| Lectura en voz alta | — | — | Sí | Sí | — | Sí |
| Resaltados y notas | — | Sí | Sí | Sí | Sí | Sí |
| Exportación de anotaciones | — | Sí | — | Sí | Sí (Markdown) | Sí |
| Tipografía y tema controlables | — | Sí | Sí | Sí | Sí | Sí |
| Vista dividida | — | — | — | **Sí** | — | — |
| Catálogos OPDS | Sí (sirve) | Sí (consume) | Sí (consume) | — | — | — |

**La conclusión que más nos afecta:** el diccionario, la Wikipedia y la traducción sobre la selección los tienen
**los cuatro** lectores de referencia. No es un extra: es lo que un usuario da por hecho, y NB Books lo trae en el
núcleo.

### 2.1.1 Referentes de forma, no de función

La sección anterior recoge **qué hace** cada lector. Esto recoge **cómo se ven** las aplicaciones que resuelven
bien el mismo problema de marco. No se copia
ninguna: se toma el patrón y el motivo.

| Patrón | Dónde está resuelto | Por qué se adopta |
|---|---|---|
| Barra lateral de alto completo que se pliega a solo iconos, con un borde interactivo que la alterna | El patrón de barra lateral de [shadcn/ui](https://ui.shadcn.com/docs/components/base/sidebar) y el de [Nuxt UI](https://ui.nuxt.com/docs/components/sidebar) | Es la convención que un usuario de escritorio ya conoce, y libera ancho para el contenido sin esconder ningún destino |
| Barra lateral del lector con índice virtualizado y búsqueda dentro | [Readest](https://deepwiki.com/readest/readest/4-features), nuestro competidor más cercano | Confirma que el índice y la búsqueda del documento son panel y no destino, que es lo que ADR-013 ya decidía por criterio |
| Consistencia por encima de cantidad en el catálogo de iconos | Recogido en [la comparativa de bibliotecas de iconos de 2026](https://muz.li/blog/best-free-icon-libraries-for-ui-design-in-2026/): trescientos iconos coherentes valen más que cinco mil con lenguajes mezclados | Es el argumento de que la lista sea **cerrada** y de que el lenguaje visual —rejilla, grosor, extremos— esté escrito |
| Plegar la navegación para recuperar ancho en vistas de datos | [Guía de barras laterales de UXPin](https://www.uxpin.com/studio/blog/sidebar-tutorial/) | Menos recorrido de la vista y menos carga cognitiva; en una biblioteca de 152.080 obras, el ancho es la vista |
| Aviso corto y no modal en una esquina, con acción y sin robar el foco | El `AdwToastOverlay` de [Foliate](https://github.com/johnfactotum/foliate) | Es la respuesta probada a "¿ha pasado algo?" para una aplicación que no debe interrumpir la lectura. Se adopta entera |
| Barra de posición permanente con capítulo, página y porcentaje | Los cuatro lectores de referencia, y la [queja pública contra el visor de Calibre 4](https://www.mobileread.com/forums/showthread.php?t=336012) por quitar el número de página | Enseñar **todas** las formas que el formato conozca en lugar de elegir una: quien lee por páginas y quien lee por porcentaje no son la misma persona |
| Herramientas de la selección agrupadas aparte de las de navegación | El emergente de selección de Foliate y la barra de herramientas de [Readest](https://deepwiki.com/readest/readest/4.3-reader-controls-and-navigation) | Confirma que las acciones sobre lo elegido son **contextuales**, no permanentes. Aquí viven en la barra y no flotando, porque se selecciona por bloques y con el teclado |

**Lo que las tres primeras filas tienen en común.** Ninguna de las referencias hace de la respuesta a una acción
una decisión de cada pantalla: hay **un sitio** donde la aplicación contesta y **un sitio** donde dice dónde
estás, y los dos son los mismos siempre. Ver la sección 9 de
[ui-conventions.md](../ui-conventions.md).

**Lo que no se toma de ninguna de ellas:** la sobreexplicación. Ninguna vista de NB Books lleva textos de ayuda
permanentes junto a los controles, y los estados vacíos son una frase y una acción. Está escrito como contrato en
[design/ui-conventions.md](../ui-conventions.md), sección 3.

### 2.2 Lo que vale la pena robar, con su fuente

Ideas concretas que existen, funcionan y encajan con lo que ya construimos:

| Idea | De dónde | Por qué encaja aquí |
|---|---|---|
| **Lista de vocabulario consultado** | Kindle ("Vocabulary Builder") | Se exporta en CSV, que abre cualquier hoja de cálculo e importa Anki, y se convierte en tarjetas con la palabra en su contexto delante y la definición del diccionario local detrás, sin IA y sin red |
| **Índice de entidades del libro** | Kindle ("X-Ray") | Local y sin IA: los nombres que se repiten, con dónde aparecen según el índice de búsqueda. **No distingue** personajes de lugares, porque sin un modelo sería adivinar. No se guarda nada |
| **Mapa del libro** | KOReader ("Book Map") | Arriba del índice del lector: capítulos por tramos, lo leído, dónde se está y las anotaciones, cada una llevando a su sitio por Locator. Agrupa para que lo que se pinta no crezca con lo anotado |
| **"¿Dónde estoy?"** | Thorium | Rastro de encabezados para quien usa lector de pantalla (`P`) |
| **Perfiles de lectura** | KOReader | Los modos de lectura son **perfiles**, no paletas |
| **Vista dividida** | Readest | Confirma la respuesta de [ADR-012](../../adr/012-ui-paradigm.md) a la simultaneidad |
| **Animaciones de pase de página** | Moon+ Reader | Solo dentro del presupuesto de fotograma, y desaparecen con movimiento reducido |
| **Reproductor integrado** | Librera | El fondo sonoro no es una excentricidad |
| **OPDS** | Gutenberg, Standard Ebooks, Internet Archive, OAPEN | Ver la sección 3 |
| **Fórmulas que se leen** | Thorium (MathML vía MathJax) | El saneador admite MathML de presentación y WebKitGTK lo pinta sin MathJax. Ver [reader-contracts.md](../../architecture/reader-contracts.md) |
| **Audio sincronizado con el texto** | Thorium (Media Overlays de EPUB3) | La narración del libro suena en el panel de voz, anclada por Locator, con el resaltado de la voz y la posición guardada |
| **Ritmo de lectura y cuánto falta** | Bookly, sin su cronómetro manual | Sacado de las sesiones que el lector abre y cierra solo. Sin cronómetro ni metas que empujen; los saltos por el índice y los ratos sin leer no cuentan, y sin diez minutos de lectura no se dice nada |
| **Repetición espaciada sobre lo subrayado** | Readwise | Las tarjetas y el vocabulario se repasan con un algoritmo de repaso espaciado, sin extraer nada de una base ajena |
| **Decir qué llegó duplicado** | Komga (hashes) | Al importar, las copias exactas se cuentan aparte y se listan con el título de lo que ya estaba; no entran dos veces. Ver [storage.md](../../architecture/storage.md) sección 2.3 |

### 2.5 El panorama ampliado

El análisis cubre gestores y lectores de escritorio, lectores móviles y ecosistemas cerrados, servidores
autohospedados y rastreadores de lectura, en cuatro ejes: qué comparten todos, qué hace único a cada uno, qué es lo
peor de cada uno y cómo es su interfaz.

**Lo que comparten todos, lo tenemos**: EPUB2 y 3, control de tipografía y márgenes, temas, índice, marcadores,
metadatos y OPDS. Nada de eso nos distingue y todo es condición para competir.

**Los dolores ajenos son, casi uno a uno, garantías nuestras ya escritas.** Calibre duplica los archivos del usuario
en su propia estructura: nosotros referenciamos. Kavita no deja editar metadatos desde su interfaz y obliga a salir a
Calibre: nosotros editamos dentro. Komga exige una convención de carpetas dictatorial y una máquina virtual de Java:
nosotros no exigimos ninguna de las dos. Audiobookshelf pide Docker y proxies: nosotros somos plug and play por la
puerta de empaquetado. Readwise y Bookly cobran por lo básico: nuestro núcleo es abierto. Kindle y Apple encierran:
nosotros exportamos la biblioteca entera desde Ajustes.

**Y hay uno donde NB Books no es mejor, así que se dice.** El dolor que todo el mundo le achaca a Thorium es el
consumo de memoria de Electron. NB Books en reposo usa unos 320 MB, de los que unos 240 son el motor WebKitGTK vacío.
No se fija un techo absoluto -no se quita una función ni se empeora el aspecto para caber- y lo propio se vigila
como referencia de regresión. Ver [performance-budgets.md](../../architecture/performance-budgets.md).

### 2.3 El kit moderno de lectura accesible

Existe y se ha estandarizado de hecho fuera de los lectores, en extensiones de navegador. Es casi todo
tipografía y composición, es decir, **muy barato para nosotros y de valor alto**:

* Fuentes pensadas para dislexia y baja visión: OpenDyslexic, Lexend, Atkinson Hyperlegible.
* Lectura biónica: la primera mitad de cada palabra en negrita como punto de fijación.
* Regla de lectura y foco de línea: atenuar todo salvo la línea en curso.
* Superposiciones de color y control de temperatura.

NB Books lo incluye en los ajustes del libro: tipografía para dislexia (OpenDyslexic, si está instalada en el
sistema) y para baja visión (Atkinson Hyperlegible), lectura biónica, regla de lectura y foco de línea. Las
fuentes no viajan en el instalador: se usan las del sistema.

### 2.4 Por qué se queja la gente de Calibre

Es el dato competitivo más útil de toda esta investigación, porque **nadie se queja de que le falten funciones**.
Las quejas públicas de sus usuarios son, literalmente: detección de duplicados que funcione, agrupación
automática por series, edición masiva de metadatos sin sufrir, lentitud con bibliotecas grandes, y una interfaz
que no escala.

| Queja | Qué tenemos ya |
|---|---|
| Duplicados | **Identidad por contenido**: distingue duplicado, mudanza y copia. Resuelto mejor que en Calibre, y **se ve**: el informe de la importación dice qué copias exactas no entraron y de qué son copia |
| Series y agrupación | **Colecciones por regla**, evaluadas al consultar, así que no se quedan obsoletas |
| Edición masiva | **Hecha**, como extensión propia: el editor de metadatos por lotes |
| Lentitud | Los [presupuestos de rendimiento](../../architecture/performance-budgets.md), medidos sobre 152.080 documentos |

La conclusión estratégica: no se compite con Calibre añadiendo funciones, se compite enseñando lo que ya está
hecho. De aquí salen además dos funciones: el filtro "Duplicados" enseña lo que la gente llama duplicado -mismo
título y mismo autor, con distinto contenido-, y quien viene de Calibre puede traerse su biblioteca con lo que
corrigió en ella.

---

## 3. OPDS

**Open Publication Distribution System** es el estándar para publicar y navegar catálogos de libros. Lo sirven
Project Gutenberg, Standard Ebooks, Internet Archive y OAPEN (académico), y **lo usan las bibliotecas
institucionales**. Calibre lo sirve; KOReader y Thorium lo consumen.

Consumirlo resuelve tres problemas a la vez:

1. **La biblioteca vacía.** Quien instala NB Books no tiene nada. Descubrir un catálogo abierto es
   la diferencia entre una aplicación que hay que llenar y una que ya sirve.
2. **De dónde salen los libros**, sin que tengamos que inventar una tienda ni negociar con nadie.
3. **Las bibliotecas públicas y universitarias**, porque es el mismo protocolo que muchas ya sirven.

Es además la ocasión correcta para materializar la regla de red de la Constitución, porque es una salida
observable, consentida y acotada a un catálogo que el usuario añadió a mano.

---

## 4. Qué rechazamos, y por qué

Un documento de investigación que solo dice que sí no sirve para decidir.

| Rechazado | Motivo |
|---|---|
| **Conversión de formatos** | Es la joya de Calibre y una superficie de mantenimiento enorme: un escritor por formato, cada uno con sus casos límite. No es leer. Candidato a extensión, nunca a núcleo |
| **Enviar al dispositivo y sincronía con lectores de tinta** | Trabajo de gestor de biblioteca, no de lector. Superficie grande para un público pequeño dentro de nuestra audiencia |
| **Descarga de noticias y RSS convertidos a EPUB** | Mantenimiento perpetuo de decenas de recetas que se rompen cuando cambia cada sitio web |
| **Valoraciones, reseñas, seguir a otras personas** | Es Goodreads. Exige cuentas y servidor, y contradice local-first y la regla de privacidad de la Constitución |
| **Sección de navegación dedicada a la IA** | Contradice [product-decisions](../../adr/product-decisions.md) §4: la IA es transversal y opcional, **nunca la identidad del producto**. Si es un destino de primer nivel, es la identidad |
| **Rachas y presión por no romperlas** | La Constitución dice que el usuario mantiene el control. Metas que se pone él, sí; culpa administrada por el software, no. La diferencia entre acompañar y manipular es exactamente esa |
| **Lienzo espacial infinito** | Ver la sección 5 |
| **Reflujo de PDF a dos y tres columnas** | Lo mejor de KOReader, y el hueco más caro que existe: reescribe la maquetación de un PDF forzando la extracción de texto de cajas rígidas. Es un subsistema entero para una minoría dentro de nuestra audiencia, y en escritorio el problema que resuelve -pantallas diminutas de tinta- casi no aparece. **Candidato a extensión, nunca a núcleo** |
| **OCR incorporado** | Hoy decimos que un escaneado no tiene capa de texto, que es lo honesto. Empotrar Tesseract añade un motor, sus datos de idioma y horas de CPU del usuario al instalador. Extensión |
| **Transcodificación de audio al vuelo** | De Audiobookshelf, que es un servidor. Nosotros no servimos a clientes remotos: reproducimos en la misma máquina donde está el archivo |
| **API de tipo streaming con webhooks** | De Komga, y tiene sentido para un servidor con clientes móviles. Aquí no hay clientes: la interfaz vive en el mismo proceso |
| **Servir catálogos OPDS a otros** | Consumimos OPDS (sección 3) y eso es lo que quiere un lector. Emitirlos es ser un servidor, con su puerto, su autenticación y su superficie. Extensión |
| **Recorte de viñetas por red neuronal** | El *Bubble Zoom* de Google es impresionante y exige un modelo entrenado, mantenido y ejecutado en la máquina del usuario, para cómics, que no son el centro de nuestra audiencia |
| **Gamificación** | De Bookly: mascotas, logros y partículas al cumplir metas. Es la misma familia que las rachas con presión, ya rechazadas: acompañar no es administrar culpa |
| **Mapeo de botones físicos y lectura RSVP** | De Moon+ y Librera. El primero es un problema de móvil; el segundo es un modo de lectura legítimo pero minoritario. Extensión |

---

## 5. El espacio de notas: qué va al núcleo y qué no

La propuesta original era un lienzo espacial infinito de notas. La investigación sobre herramientas de
conocimiento enlazado apunta a algo más fino: **el grafo no se dibuja, emerge de los enlaces**. Cada enlace
interno produce un retroenlace, y la vista de grafo es una lectura de esos datos, no su fuente.

De ahí sale la separación:

* **Al núcleo, el enlazado.** Notas propias con enlaces y retroenlaces. Es texto: completamente accesible,
  barato, y ya contiene toda la potencia.
* **Fuera del lienzo infinito, tableros finitos.** Columnas con nombre en las que se ordenan las notas. Finitos y no infinitos
  por una razón medible: el [presupuesto de nodos del DOM](../../architecture/performance-budgets.md)
  es de 1.500, y un lienzo infinito no tiene forma de respetarlo.
* **Regla que lo mantiene sano:** todo tablero tiene su **equivalente en lista**, sobre los mismos datos. Sin
  esa regla, el lienzo reintroduce por la puerta de atrás el problema que resuelve
  [ADR-012](../../adr/012-ui-paradigm.md): orden visual que no coincide con orden del documento.

---

## 6. Fondo sonoro

Se rechaza como núcleo y se acepta como extensión de primera parte, por tres razones concretas:

* **La conducta existe.** Quien estudia pone ruido de fondo, y sin esto saldría de la aplicación para hacerlo. Eso es
  exactamente lo que el principio 5 de la [filosofía](../product-philosophy.md) llama fallar.
* **Hay respaldo.** El ruido ambiente moderado favorece la concentración, y no somos los primeros: Librera ya
  trae reproductor integrado.
* **Pero no debe pesar.** El ruido blanco, rosa y marrón se **sintetizan**: cero archivos. Las grabaciones de
  naturaleza y la música sí pesan y tienen licencia, y no deben entrar en el instalador de quien no las quiere.

Es el caso de manual: se instala quien lo quiere y se desinstala quien no.

---

## 7. Cómo se usa este documento

* Antes de proponer una función de producto, se comprueba aquí si ya se evaluó.
* Una función rechazada en la sección 4 no se reabre sin un dato nuevo. El dato nuevo se añade a este documento
  con su fuente, y entonces sí se rediscute.
* Cuando una de las referencias de la sección 2 cambie de forma relevante, se revisa la tabla y se anota la
  fecha. Una comparación sin fecha es una comparación que miente.

---

## Fuentes

Consultadas el 2026-08-27.

* KOReader, [lista de funciones](https://github.com/koreader/koreader/wiki/Features-list) y
  [complemento de estadísticas](https://github.com/koreader/koreader/wiki/Statistics-plugin).
* EDRLab, [accesibilidad de Thorium Reader](https://thorium.edrlab.org/en/docs/300_accessibility/).
* [Readest](https://readest.com/), lector construido sobre Tauri v2.
* [Foliate](https://johnfactotum.github.io/foliate/), y sus
  [componentes de interfaz](https://deepwiki.com/johnfactotum/foliate/5-user-interface) (consultado el
  2026-08-29): emergente de selección, emergente de anotación y avisos con `AdwToastOverlay`.
* Readest, [controles y navegación del lector](https://deepwiki.com/readest/readest/4.3-reader-controls-and-navigation)
  y [barra lateral e índice](https://deepwiki.com/readest/readest/4.4-sidebar-and-table-of-contents) (consultado
  el 2026-08-29).
* Amazon, [funciones de Kindle](https://www.amazon.com/b?ie=UTF8&node=17717476011).
* Calibre, [manual de la interfaz](https://manual.calibre-ebook.com/gui.html), y discusión pública sobre su
  interfaz en [Hacker News](https://news.ycombinator.com/item?id=43434421).
* [Especificación OPDS](https://opds-spec.org/catalogs/), [catálogos de Project Gutenberg](https://www.gutenberg.org/ebooks/offline_catalogs.html)
  y [Standard Ebooks](https://en.wikipedia.org/wiki/Standard_Ebooks).
* [Comparativa de rastreadores de lectura](https://www.pickupreader.com/blog/reading-tracker-comparison).
* [Herramientas de lectura accesible](https://focusflowapp.in/blog/adhd-reading-tools).
* Mason y otros, [resaltado y comprensión en lectura digital](https://onlinelibrary.wiley.com/doi/10.1111/jcal.12903),
  *Journal of Computer Assisted Learning*, 2024.
* [Noisli](https://www.noisli.com/), sobre ruido ambiente y concentración.
