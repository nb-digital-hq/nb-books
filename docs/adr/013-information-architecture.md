# ADR-013: Arquitectura de Información de la Interfaz

**Fecha:** 2026-08-27
**Decidido:** 2026-08-27
**Estado:** Aceptado
**Enmendado:** 2026-09-12: las recomendaciones de Inicio pasan de deterministas a reproducibles. Ver
[la enmienda](#enmienda-del-2026-09-12-recomendaciones-reproducibles). 2026-11-15: se retira el destino Cursos; ver
[la segunda enmienda](#enmienda-del-2026-11-15-sin-destino-cursos).

Continúa [ADR-012](012-ui-paradigm.md), que decidió **la forma** de la interfaz. Este decide **su
contenido**: qué destinos existen, qué vive en cada uno y qué no es un destino. Se apoya en
[la investigación de la experiencia de lectura](../design/reading-experience/README.md).

## Contexto

ADR-012 estableció que NB Books es una aplicación convencional de una sola ventana, con paneles y vistas. No
dijo **cuántas vistas ni cuáles**, y la decisión correspondiente no puede empezar sin saberlo: construir la vista de
biblioteca sin saber si existe una pantalla de inicio delante determina si la biblioteca es la raíz del producto
o una sección de él, y eso es caro de cambiar después.

Al plantear la pregunta aparecieron dos problemas de fondo, uno de omisión y otro de método.

**El de omisión.** El plan describía un producto capaz y delgado: importar, leer, anotar, buscar. Comparado con
lo que el propio proyecto promete, faltaban cosas **ya escritas y sin sitio donde verse**:

* [product-decisions](product-decisions.md) §1 promete "progreso y estadísticas:
  historial local, progreso de lectura, métricas de sesión". Ninguna iteración lo pintaba.
* El mismo documento promete "clasificación mediante metadatos locales **e internet** (descarga de portadas)".
  No hay una sola línea de red en el proyecto ni iteración que la introduzca.
* El diccionario figuraba entre las herramientas locales prometidas y se había desplazado a una extensión.

**El de método.** La superficie de producto se había fijado por intuición. La intuición resultó ser en gran parte
correcta, pero eso solo se supo al comprobarla. La comprobación está en el documento de experiencia de lectura y
produjo tres hallazgos que ningún participante tenía:

1. **El diccionario, la Wikipedia y la traducción sobre la selección los tienen los cuatro lectores serios de
   referencia** (KOReader, Thorium, Readest, Foliate) y también Kindle. Es base, no diferenciación.
2. **OPDS** es el estándar por el que se navegan y descargan catálogos abiertos y de bibliotecas. En los
   documentos del proyecto aparecía una vez, y como algo que exponemos, no que consumimos.
3. **Las quejas públicas sobre Calibre no son por funciones que falten**, sino por duplicados, series, edición
   masiva y lentitud. Tres de esas cuatro ya están resueltas en nuestro runtime y ninguna está en pantalla.

## El criterio, antes que la lista

La lista de destinos caduca; el criterio no. **Algo es un destino si cumple las tres condiciones:**

1. Es un lugar donde el usuario **permanece y trabaja**, no un sitio por el que pasa.
2. Tiene **estado propio que sobrevive** a salir y volver.
3. **No es una propiedad de otro destino.**

De ahí se derivan las otras tres categorías:

| Categoría | Definición | Ejemplo |
|---|---|---|
| **Destino** | Cumple las tres condiciones | Biblioteca |
| **Panel** | Acompaña a un destino y no existe sin él | Anotaciones del documento abierto |
| **Ajuste** | Se configura y no se visita | Escala tipográfica |
| **Transversal** | Aparece dentro de varios destinos sin ser ninguno | Búsqueda, IA, lectura en voz alta |

Este criterio es la parte vinculante de este ADR. Toda propuesta futura de "añadir una sección" se contesta con
él y no con una discusión de gustos.

## Alternativas consideradas

### A. Dos destinos: Biblioteca y Lector

Lo que hacen Foliate y, en lo esencial, Readest.

**A favor:** mínimo, imposible de perderse, y es lo que el plan implicaba sin decirlo.
**En contra:** no tiene dónde poner nada de lo prometido y no visto —progreso, estadísticas, notas propias,
catálogos, vocabulario— y obliga a esconderlo en menús. Es la arquitectura de un lector, no la de un entorno de
trabajo con conocimiento, que es lo que el
[resumen ejecutivo](../institution/executive-summary.md) dice que construimos.

### B. La biblioteca como raíz, y todo lo demás dentro de ella

El modelo de Calibre.

**A favor:** una sola cosa que aprender.
**En contra:** responde a la pregunta equivocada. Una cuadrícula de portadas contesta "¿qué tengo?", y quien
abre un lector quiere "seguir donde estaba". Y es exactamente la arquitectura cuya interfaz los usuarios de
Calibre describen como que no escala.

### C. Siete destinos, dos de ellos contextuales

La adoptada. Se detalla abajo.

**En contra, y hay que decirlo:** siete es más de lo que cabe cómodo en una barra lateral. Se acepta porque los
contextuales no se ven hasta que sirven.

### D. Destinos por modo de trabajo: Leer, Estudiar, Gestionar

**A favor:** conceptualmente elegante y encaja con la filosofía de diseñar para el objetivo.
**En contra:** obliga al usuario a declarar en qué modo está antes de hacer nada, y las tareas reales cruzan los
modos constantemente: se busca un libro mientras se estudia, se anota mientras se lee. Es un conmutador de modo,
que es la misma clase de fricción que ADR-012 rechazó en su alternativa D.

## Decisión

Se adopta la **alternativa C**. La navegación primaria tiene siete destinos, dos contextuales.

| Destino | Qué contiene | Cuándo aparece |
|---|---|---|
| **Inicio** | Sigue leyendo; recomendaciones locales; metas del día; novedades de la biblioteca | Siempre. **Es la pantalla de arranque** |
| **Biblioteca** | Todo, estanterías, colecciones, etiquetas, autores, series, materias. Importar. Duplicados y documentos no disponibles | Siempre |
| **Descubrir** | Catálogos OPDS que añade la persona. **Única zona cuya razón de ser es la red** | Con al menos un catálogo, marcada como en línea |
| **Lector** | El documento abierto y sus paneles | **Solo con algo abierto** |
| **Estudio** | Notas propias enlazadas; anotaciones de todos los libros; vocabulario consultado; metas y plan de lectura. Más adelante, tarjetas y conceptos | Siempre |
| **Ajustes** | Apariencia, lectura, accesibilidad, idioma, almacenamiento, red y privacidad, extensiones, IA y modelos, cuenta | Siempre |

### Cinco reglas que se derivan y son vinculantes

**1. La aplicación arranca en Inicio, no en Biblioteca.** Es la decisión de producto más consecuente de este
ADR. Una cuadrícula de portadas responde a "qué tengo"; quien abre un lector quiere "seguir donde estaba".
Cuesta una consulta sobre datos que ya existen.

**2. El Lector no se cierra al navegar.** Ir a Biblioteca desde una lectura en curso **no termina la sesión**:
el destino Lector permanece y se vuelve a él donde se dejó. Sin esta regla, consultar algo en la biblioteca
mientras se lee castiga al usuario, y es la conducta más frecuente que hay.

**3. Lo que necesita red vive donde funcionalmente corresponde, no en una sección aparte.** Descargar la portada
de un libro está en la ficha de ese libro, no en un menú "Internet". La única excepción es Descubrir, cuya razón
de ser **es** el catálogo remoto. La observabilidad no se consigue segregando, sino marcando: cada acción con
red se anuncia antes de ejecutarse y queda en un registro que el usuario puede consultar, como ya exige
[privacy.md](../audit/privacy.md).

**4. La IA no es un destino.** [product-decisions](product-decisions.md) §4 dice que es
"una capacidad transversal de enriquecimiento opcional, **nunca la identidad del producto**". Un destino de
primer nivel es precisamente la identidad. La IA aparece **dentro** del Lector, de la Búsqueda y del Estudio; sus
modelos y proveedores se configuran en Ajustes. La misma regla vale para la búsqueda y para la lectura en voz
alta: transversales, no destinos.

**5. Un destino contextual no se muestra vacío ni deshabilitado.** Lector sin documento abierto y Descubrir sin
catálogos **no están en la barra**. Un elemento permanentemente atenuado es publicidad de algo
que el usuario no tiene, y contradice la regla de estados degradados: lo no disponible se omite, no se
atenúa.

### Qué queda explícitamente fuera del producto

Esta lista existe para contestar de una vez, con motivo, y no volver a discutirlo. El razonamiento largo está en
la [sección 4 de la investigación](../design/reading-experience/README.md).

* Conversión entre formatos: superficie de mantenimiento enorme y no es leer. Candidato a extensión.
* Envío a dispositivos de tinta electrónica y sincronía con ellos.
* Descarga de noticias y fuentes RSS convertidas a libro.
* Valoraciones, reseñas y seguimiento entre usuarios. Exige cuentas y servidor, y contradice local-first.
* Rachas con presión. Metas que el usuario se pone, sí; culpa administrada por el software, no.
* Lienzo espacial infinito. El enlazado entre notas va al núcleo; el lienzo, finito, va a una extensión con la
  regla de que todo tablero tiene equivalente en lista.

## Consecuencias

**Positivas.**

* el diseño puede empezar sabiendo que la biblioteca es una sección y no la raíz, que es la decisión que habría
  sido cara de revertir.
* Lo ya prometido y sin sitio —progreso, estadísticas, portadas en línea, diccionario— pasa a tener destino y,
  por tanto, iteración.
* El criterio de la sección "El criterio, antes que la lista" cierra las discusiones futuras sobre añadir
  secciones sin reabrir este ADR.
* Descubrir resuelve el problema de la biblioteca vacía, que ningún documento había planteado: quien instala
  NB Books hoy se encuentra un producto que hay que llenar antes de que sirva.
* La regla 3 resuelve la tensión entre local-first y las funciones en línea sin esconder las segundas.

**Negativas.**

* **Amplía el alcance del producto de forma significativa**: consulta sobre la selección, catálogos OPDS e Inicio.
  Se acepta porque es la superficie que el propio proyecto ya prometía, no superficie nueva.
* Siete destinos son más difíciles de mantener coherentes que dos.
* Descubrir introduce la segunda salida de red del producto, con su coste de privacidad, de consentimiento y de
  pruebas.

**Riesgos.**

* **Que Inicio se convierta en un escaparate.** Una pantalla de recomendaciones puede degenerar en algo que
  empuja en lugar de acompañar. Mitigación: las recomendaciones son locales, **reproducibles** y explicables
  —el usuario puede ver por qué aparece cada una— y se pueden desactivar enteras. "Deterministas" decía hasta
  la enmienda del 2026-09-12.
* **Que Estudio y Lector se solapen.** Las anotaciones se ven en los dos sitios. Mitigación: en el Lector se ven
  las **del documento abierto**, y en Estudio las **de todos**; son el mismo dato con dos alcances, igual que la
  búsqueda de catálogo y la de corpus.
* **Que Descubrir se perciba como una tienda.** Mitigación: solo catálogos que el usuario añade, ninguno
  preconfigurado con ánimo comercial, y sin ninguna transacción.

## Enmienda del 2026-09-12: recomendaciones reproducibles

**Qué cambia.** Donde decía que las recomendaciones de Inicio son *locales, deterministas y explicables*, dice
*locales, explicables y reproducibles*. 

**Por qué.** Porque "determinista" se leyó como "fija", y fija resultó ser el defecto. Con 152.080 obras, Inicio
enseñaba dos filas que no cambiaban nunca, con obras repetidas entre ellas y alguna de cinco libros. En uso real:
"cada vez que entre debe cambiar, hay unas que se deben mantener", y "si vamos a tener cinco libros, es mejor
no tener la fila". Lo que la palabra protegía no era la quietud, sino que Inicio **no empuje**: que nada
aparezca sin un porqué que el usuario pueda leer, y que nada dependa de algo que el usuario no ve.

**Qué se conserva, y es lo que hace falta para que no empuje.**

* **Locales.** Todo sale de la biblioteca y de lo que el usuario hace en ella. Cero red y cero IA.
* **Explicables.** Cada fila dice por qué aparece: "Más de *autor*, porque estás leyendo *título*", "Una
  materia de tu biblioteca", "Diez obras distintas en cada visita". Una fila sin porqué no se pinta.
* **Reproducibles.** Lo que cambia entre visitas lo decide una semilla que pone la interfaz al salir de Inicio,
  y la misma semilla da la misma pantalla. No hay un historial de lo que se enseñó, ni aprendizaje, ni nada que
  el runtime recuerde para decidir: por eso cambiar no es perfilar.
* **Desactivables.** Un interruptor en Ajustes, Apariencia, deja solo lo que no sugiere nada: lo empezado y las
  novedades. **Estaba escrito y no existía**: la vista aceptaba la propiedad y nadie se la pasaba.
  Se conectó en la misma iteración.

**Dos reglas nuevas que la acompañan.** Ninguna fila con menos de ocho obras se pinta, y ninguna obra sale dos
veces en la misma pantalla. Lo que responde a lo que el usuario hace -lo empezado, las novedades, "porque
estás leyendo"- se mantiene entre visitas; lo que invita a descubrir -materias, autores, al azar- cambia.

**Dónde vive.** `src-tauri/src/home.rs`, `filas`, con sus pruebas en `src-tauri/tests/inicio.rs`.

## Documentos que este ADR obliga a cambiar

| Documento | Cambio |
|---|---|
| [ui-components.md](../architecture/ui-components.md) | Añade la arquitectura de información y el criterio de destino |
| [storage.md](../architecture/storage.md) | Raíces de biblioteca y rutas relativas |
| [reading-experience](../design/reading-experience/README.md) | Documento nuevo: la investigación que sostiene este ADR |


## Enmienda del 2026-11-15: sin destino Cursos

**Qué cambia.** Se retira el destino **Cursos**. NB Books no incluye integración con sistemas de gestión del
aprendizaje ni vinculación institucional: esas capacidades pertenecen a la Suite Institucional de NB Digital, que
se distribuye por separado. La navegación queda en seis destinos: Inicio, Biblioteca, Descubrir, Lector, Estudio y
Ajustes, de los que Lector y Descubrir son contextuales.

**Por qué.** Un destino que ninguna instalación puede llegar a mostrar es código y documentación sin función, y la
regla 5 de este ADR ya exige no anunciar lo que la persona no tiene.
