# Contratos del Lector (Reader Contracts)

> **Estado:** Implementado. Superficie de lectura en `src/lib/ui/lector/`; `Locator` en
> `src-tauri/src/reader/locator.rs`; contrato de proveedor en `src-tauri/src/reader/provider.rs`, con un proveedor
> simulado en `reader/fake.rs` que ejercita el lector sin ningún formato real; proveedores de EPUB, PDF y cómic; y la
> sesión que los abre en `reader/session.rs`. Modelo de posición: [ADR-004](../adr/004-reader-locator-model.md).
> Este documento define la abstracción que permite que el resto del sistema (anotaciones, búsqueda, progreso,
> IA, sincronización) funcione igual con independencia del formato del documento.

Es el documento que hace posible el principio "diseñar para el objetivo, no para el formato".

## 0. Qué puede aportar una extensión, y qué no

Una extensión aporta **de qué está hecho un documento**: sus secciones, con su identificador, su título y su
texto. **No aporta un lector.**

Esa frontera no es comodidad. Sostiene tres garantías del núcleo que dejarían de serlo si dependieran de código
de terceros:

* El **modelo de Locator** de [ADR-004](../adr/004-reader-locator-model.md) —ninguna posición se
  guarda como entero— lo aplica `reader::secciones::DocumentoDeSecciones`, no la extensión.
* El **saneado del marcado** lo hace el núcleo.
* Las **capacidades del lector** las decide el núcleo, que es lo que permite decir qué se puede hacer con un
  documento antes de abrirlo.

**Qué recibe una extensión de formato:** los bytes. No una ruta, porque no toca el disco y no debe saber dónde
vive nada; canonicalizar y acotar ya lo hace el anfitrión. Y su mundo WIT **no importa nada del anfitrión**, así
que tampoco puede salir a la red: lo que un invitado puede hacer se amplía dándole lo que necesita, no dándoselo
por si acaso.

**Dónde vive lo que comparten los formatos de texto.** En `reader::secciones`, que nació como el proveedor
simulado y se separó de él. Lo difícil de un proveedor no es el formato: es la cascada de
anclaje, la progresión acumulada, la extracción de texto en trozos anclados y la negociación de capacidades.
Escrito una vez, un formato nuevo solo tiene que decir cuáles son sus secciones.


## 1. El problema

Un PDF se pagina por páginas fijas. Un EPUB reflowable no tiene páginas: su posición depende del tamaño de la
fuente y de la ventana. Un cómic es una secuencia de imágenes. Un audiolibro es una línea temporal.

Si el sistema modela "página actual" como un entero, el modelo se rompe con el primer EPUB y todas las
anotaciones quedan mal ancladas cuando el usuario cambia el tamaño de letra.

## 2. El Locator

NB Books adopta un **Locator** como unidad universal de posición, alineado con el modelo del ecosistema Readium.
Un locator identifica una posición dentro de una publicación de forma independiente del dispositivo y de la
configuración de lectura.

Contiene, como mínimo:

* Referencia al recurso dentro de la publicación (capítulo, archivo interno, número de página, pista de audio).
* Un tipo de medio.
* Un título legible opcional para mostrar al usuario.
* Un conjunto de **localizaciones** alternativas: progresión relativa dentro del recurso, progresión total dentro
  de la publicación, posición absoluta si el formato la tiene, y un identificador de fragmento cuando el formato
  lo soporta.
* Un **texto de contexto**: la cita exacta más un fragmento anterior y posterior.

El texto de contexto no es decorativo: es lo que permite reanclar una anotación cuando el identificador de
fragmento deja de resolverse (documento actualizado, edición distinta, motor de renderizado diferente).

### 2.1 Forma serializada

La forma es la del `Locator` de Readium, en `camelCase`, con una única adición: un campo `version`. Readium no lo
tiene, y aquí hace falta porque este objeto se exporta, se sincroniza y lo puede leer una versión distinta de la
que lo escribió. Un locator de Readium **sin** `version` se acepta como versión 1; uno con una versión posterior
a la que entiende la aplicación **se rechaza**, por el mismo motivo por el que el motor de migraciones se niega a
abrir una base más nueva: interpretarlo con reglas antiguas colocaría la anotación en un sitio equivocado, y eso
es peor que no abrirla.

```json
{
  "version": 1,
  "href": "OEBPS/chapter05.xhtml",
  "mediaType": "application/xhtml+xml",
  "title": "Capítulo 5",
  "locations": {
    "fragments": ["epubcfi(/6/14!/4/10/3:10)"],
    "progression": 0.42,
    "totalProgression": 0.17,
    "position": 233
  },
  "text": {
    "before": "Muchos años después, ",
    "highlight": "frente al pelotón de fusilamiento",
    "after": ", el coronel Aureliano Buendía"
  }
}
```

Invariantes que el tipo hace cumplir, y que no son formalidades:

* **Un locator ancla al menos por una vía.** Sin fragmento, sin progresión, sin posición y sin cita no
  representa una posición; aceptarlo sería el entero de ADR-004 con más ceremonia.
* Las progresiones son finitas y están en `0..=1`. `NaN` pasa cualquier comparación de rango escrita al revés.
* La posición absoluta empieza en 1, como en Readium. Un 0 casi siempre es un índice de array traducido sin
  darse cuenta.
* La cita no está vacía: una cita vacía encuentra cualquier punto del documento.
* Los campos ausentes no se serializan como `null`. Una biblioteca entera de anotaciones no tiene que arrastrar
  seis nulos por cada una.

## 3. Estrategia de anclaje de anotaciones

El proyecto adopta un enfoque de **selectores múltiples con degradación**, siguiendo el modelo de datos de
anotación web:

1. Selector estructural del formato (identificador de fragmento canónico en EPUB, coordenadas y objeto de texto
   en PDF).
2. Selector por cita textual con contexto.
3. Selector por posición aproximada dentro del recurso.

Al abrir el documento se intenta el selector 1; si falla, se busca por cita con tolerancia; si falla, se usa la
posición aproximada y **la anotación se marca visiblemente como reanclada de forma aproximada**. Nunca se
descarta silenciosamente una anotación del usuario, y nunca se muestra en un lugar equivocado sin advertencia.

> **Implementado**: `annotations::anchor` aplica la cascada del proveedor a las
> anotaciones vivas de una edición. La derivación de la marca vive en `src/lib/core/reader.ts`, y
> el lector avisa cuando la posición de apertura se recuperó de forma aproximada. El panel de anotaciones pinta la
> marca junto a cada anotación.

### 3.1 Dos marcas, no una

Marcar solo el tercer escalón dejaría al segundo indistinguible de una anotación que nunca se movió, y son
situaciones distintas para quien lee:

| Cómo se resolvió | Qué significa | Qué se muestra |
|---|---|---|
| Fragmento canónico corroborado | Está donde la dejó | Nada. Marcar lo normal enseña a ignorar las marcas |
| Por la cita | Bien situada, pero el documento cambió | Marca discreta: saberlo es cosa suya |
| Por progresión | Se ha adivinado; puede estar en otro párrafo | Aviso visible. Es el que ADR-004 exige |

### 3.2 Lo que no se puede colocar tampoco se pierde

Una anotación que ninguna vía sabe situar **no se descarta**: sale aparte, entera, para que la interfaz la ofrezca
y el usuario decida. "Esta anotación no se puede mostrar aquí" y "esta anotación no existe" son cosas distintas, y
confundirlas es perder trabajo del usuario en silencio, que es la mitad del objetivo de esta iteración.

Esta decisión existe porque los identificadores de fragmento canónicos de EPUB tienen soporte irregular entre
implementaciones y son frágiles ante cambios del documento; depender solo de ellos produce pérdida de anotaciones
a largo plazo, lo que contradice el principio de longevidad del proyecto.

## 4. El contrato de formato

```rust
pub trait DocumentProvider: Send + Sync {
    fn media_type(&self) -> &str;

    /// Estructura navegable: índice, capítulos, pistas.
    fn navigation(&self) -> Result<Navigation, NBError>;

    /// Convierte una posición nativa del formato en un Locator canónico.
    fn locator_at(&self, position: NativePosition) -> Result<Locator, NBError>;

    /// Resuelve un Locator a una posición renderizable, con degradación.
    fn resolve(&self, locator: &Locator) -> Result<Resolution, NBError>;

    /// Texto plano de un rango, para búsqueda, accesibilidad e indexación.
    fn text_range(&self, locator: &Locator, len: usize) -> Result<String, NBError>;

    /// Capacidades soportadas por este formato.
    fn capabilities(&self) -> ReaderCapabilities;

    /// Tamaño intrínseco de lo que se pinta como imagen. Ver 4.4.
    fn page_size(&self, position: &NativePosition) -> Result<PageSize, NBError>;

    /// Capa de texto para superponer sobre la imagen. Ver 5.
    fn text_layer(&self, position: &NativePosition) -> Result<TextLayer, NBError>;

    /// Ancla una cita que el usuario acaba de seleccionar. Ver 5.3.
    fn locator_for_quote(&self, quote: &str, hint: Option<f64>) -> Result<Locator, NBError>;
}
```

Los tres últimos tienen implementación por defecto que responde `FORMAT_UNSUPPORTED`. **Es información, no un
hueco**: un EPUB no tiene tamaño de página —lo tiene la ventana— y un cómic no tiene texto que anclar. Decirlo
es lo que permite a la interfaz omitir el control en lugar de pintar un valor inventado.

`ReaderCapabilities` declara si el formato soporta selección de texto, búsqueda interna, reflujo, cambio de
tipografía, audio sincronizado o extracción de texto. **La interfaz se construye a partir de estas capacidades**:
una función que el formato no soporta no se muestra deshabilitada, se oculta.

La dirección de esa dependencia es lo que importa, y va en un solo sentido: **el proveedor declara y la interfaz
obedece**. Si la interfaz dedujera los controles del tipo de medio, cada formato nuevo obligaría a acordarse de
ir a la pantalla a enseñárselo, y el día que a alguien se le olvidase, el lector ofrecería cambiar la tipografía
de algo que no tiene letras.

Que se **omita** y no se atenúe tampoco es cosmético. Un control deshabilitado es una promesa a medias: el
usuario lo ve, lo intenta y no pasa nada, y lo razonable es concluir que el problema es suyo.

### 4.1 La sesión de lectura

> **Implementado**: `reader/session.rs`, con los comandos `open_document` y
> `close_document`. La lista de controles se deriva en `src/lib/core/reader.ts` y la pinta
> `ui/lector/ReaderView.svelte`, que **recorre el registro de comandos** en lugar de escribir botones: así no
> puede existir un botón cuya condición de aparición no conozca nadie.

Abrir un documento devuelve **una sola respuesta** con el tipo de medio, las capacidades, el índice navegable y
la posición inicial. Junto y no en tres llamadas, porque en tres llamadas la interfaz pinta tres veces mientras
llegan.

El identificador de la sesión es el del archivo: no hay identificadores de sesión aparte. Abrir dos veces el
mismo documento devuelve la misma sesión, con la posición donde el usuario la dejó. Lo contrario obliga a la
interfaz a llevar la cuenta de las sesiones y a cerrarlas bien, y el día que se le olvide una, el usuario tendrá
el mismo libro abierto dos veces en dos posiciones distintas y ninguna será *la* posición.

El registro de proveedores del arranque y el estado del componente `reader` se construyen a la vez y dicen lo
mismo: lo que el monitor declara degradado es exactamente lo que no está registrado. Separarlos dejaría que la
interfaz ocultase un formato que el runtime abre, o al revés.

### 4.2 Dos ausencias que no son la misma

Un tipo de medio sin proveedor devuelve `COMPONENT_UNAVAILABLE`, no `FORMAT_UNSUPPORTED`. El primero dice "esta
instalación no puede abrir esto"; el segundo, "esto no es un documento". La interfaz reacciona distinto a cada
uno, y confundirlos le diría al usuario que su libro está roto cuando lo que falta es un motor.

### 4.3 Qué se pinta

El contrato incluye `render`, que devuelve o **marcado ya saneado** o la **ruta** de una imagen. La imagen viaja
como ruta y no como bytes porque [data-flows.md](data-flows.md) manda servirla por el protocolo de assets: por
el canal JSON habría que codificarla en base64 y pasarla por un mensaje.

El saneado ocurre **aquí y no en el webview**. Es lo que hace que la política de contenido de Tauri sea la
segunda barrera y no la primera: la política depende de que el webview la respete, y esto no. Se sanea con una
lista de lo permitido, no de lo prohibido: una lista de prohibidos hay que mantenerla al día de cada elemento que
Se inventen y basta con olvidar uno, mientras que una de permitidos falla al revés, dejando fuera algo que
debería pasar, y ese fallo es visible en la pantalla en lugar de silencioso.

### 4.3.1 Las fórmulas se pintan

"Visible en lugar de silencioso" resultó ser falso para las fórmulas: la lista no tenía ni una etiqueta de MathML,
y como un elemento desconocido pierde la etiqueta y conserva el texto, la fórmula general llegaba como
`x=−b±b2−4ac2a`. No se veía como un fallo; se veía como una errata del libro.

Ahora la lista admite **MathML de presentación** -lo de MathML Core y lo que WebKit pinta además, `menclose` y
`mfenced`- con sus atributos de colocación, que **solo valen dentro de una fórmula**. Lo que no entra, y por qué:

| Qué | Por qué |
|---|---|
| `annotation-xml`, **con su contenido** | Puede traer HTML o SVG, y el navegador trata ese punto como uno donde el marcado vuelve a ser HTML: es la puerta de las confusiones de espacio de nombres. Y no se pinta nunca, es otra forma de la misma fórmula |
| `mglyph` | Carga una imagen por su cuenta |
| `style`, `on*`, `xlink:href` | Lo mismo que fuera de una fórmula |
| `href` con esquema | MathML lo admite en cualquier elemento, y pasa la misma comprobación que un enlace |

Comprobado en el motor del producto, no solo en el de la suite: el capítulo de fórmulas que fija
`tests/epub_lector.rs`, pintado en WebKitGTK con `scripts/guiones/formulas.js`, da un `math` en el espacio de
nombres de MathML, el numerador encima del denominador, la raíz con su signo y la fórmula en bloque de 49 px de alto
contra los 29 de una línea de texto. La lectura biónica no entra en una fórmula: una negrita de HTML dentro de un
`<mi>` la rompe.

### 4.3.2 Las ilustraciones vectoriales se ven, acotadas

Hasta esa fecha el saneador quitaba todo `<svg>`, y la decisión no estaba escrita en ningún contrato: una
ilustración vectorial desaparecía, y con ella la portada de los libros que la componen como un dibujo con la imagen
dentro, que son muchos. La ficha pedía una de dos: admitir un subconjunto seguro o declarar el rechazo y decirlo en
la interfaz. **Se admite el subconjunto**: lo que hace falta para dibujar -formas, trazos, texto, degradados,
recortes, máscaras, motivos, marcadores, `use` de un símbolo propio- y la imagen de dentro del dibujo, que se
extrae a la caché y se pide por el protocolo de assets igual que la de un `img`. Los atributos de dibujo solo valen
dentro de un dibujo. Lo que no entra, con su prueba en `cada_exclusion_del_dibujo_tiene_su_prueba`:

| Qué | Por qué |
|---|---|
| `script` | Ejecuta |
| `foreignObject`, **con su contenido** | Mete HTML dentro del dibujo: el mismo punto de confusión que `annotation-xml` en una fórmula |
| `animate`, `animateMotion`, `animateTransform`, `set` | No dibujan nada y pueden reescribir un atributo **después** de sanear: un `href` que pasa a `javascript:` |
| `filter` y sus primitivas | `feImage` carga lo que se le pida |
| `url` que no sea `#algo` en `fill`, `stroke`, `clip-path`, `mask` y `marker-*` | Un relleno con `url(https://…)` es una petición a un servidor |
| `href` fuera de `image` que no sea `#algo` | Un `use` que trae el dibujo de otro archivo no se puede resolver ahí, y lo que no se resuelve no tiene por qué salir a buscarse |
| `href` con esquema, `style`, `on*` | Lo mismo que fuera de un dibujo |

Los nombres se escriben en minúsculas y el analizador de HTML les devuelve la forma de SVG -`linearGradient`- al
leerlos dentro de un `svg`. Hacerlo destapó un defecto antiguo: el saneador ponía en minúsculas el nombre al abrir
un elemento y no al cerrarlo, así que `</linearGradient>` no cerraba nada y el resto del dibujo quedaba dentro del
degradado. Pintado en WebKitGTK con `scripts/guiones/dibujo.js`: espacio de nombres de SVG, `linearGradient`
reconocido, el fondo a 400x200 y relleno con su degradado, y el trazo con su marcador.

### 4.3.3 Lo que no se pinta, se dice

Lo que el saneador sigue quitando y alguien echaría en falta -audio, vídeo, contenido incrustado como `iframe`,
`object`, `embed` o `canvas`, y formularios- viaja con lo que se pinta (`Rendered::Markup.omitted`), y el lector lo
dice debajo de su barra, igual que dice que un escaneado no tiene texto: "Este capítulo trae vídeo y un formulario
que el lector no muestra". Un hueco sin explicación se lee como un defecto del libro.

**Lo que no se dice, a propósito**: los `script` y la hoja de estilo del libro. No son contenido que alguien eche en
falta: son comportamiento y aspecto, y el lector pone su propia tipografía, que es la que el usuario elige.

### 4.4 Lo que se pinta como imagen necesita un tamaño y un encaje

> **Implementado.** El cómic (`reader/comic.rs`), con el cálculo de encaje en `reader/provider.rs`, y
> el PDF: los tamaños se leen del motor **al abrir**, en la misma pasada que el texto, porque entonces
> el documento ya está cargado. La superficie reserva el hueco con ese tamaño antes de pedir la imagen; pedirlo
> después sería pedirlo cuando el salto de maquetación ya ha ocurrido.

PDF y cómic comparten un problema que el EPUB no tiene: se pintan como imagen, así que hace falta saber cuánto
mide la página **antes** de tenerla, para reservar el hueco y no dar el salto de maquetación, y hace falta una
regla que diga cómo encaja en la ventana. Las dos cosas viven en el contrato de proveedor y no en la interfaz,
porque son las mismas para los dos formatos y una regla de encaje duplicada se convierte en dos reglas distintas
en cuanto una de las dos copias se toca.

El tamaño se lee de la **cabecera** de la imagen, sin decodificarla: decodificar un JPEG de veinte megapíxeles
para averiguar dos números costaría más que pintarlo. Un formato cuya cabecera no se sepa leer devuelve
`FORMAT_UNSUPPORTED` en lugar de un tamaño inventado, con el que la página se pintaría deformada.

### 4.5 Un fragmento dentro de rango no prueba nada

La resolución exacta por identificador de fragmento **se corrobora con la cita** cuando el locator la lleva. Un
desplazamiento sigue estando dentro de rango después de que al documento le añadan un prólogo, y entonces apunta
a otra frase: sin corroborar, la cascada nunca llegaría al segundo escalón y las anotaciones se desplazarían en
silencio, que es exactamente lo que ADR-004 existe para impedir.

## 5. Accesibilidad y capa de texto

> **Implementado**: `PdfDocument::text_layer` devuelve los fragmentos con sus
> coordenadas **normalizadas de 0 a 1** sobre el tamaño de la página, no en píxeles, porque la interfaz escala la
> imagen y la superposición tiene que escalar con ella; en píxeles, cambiar el zoom descolocaría el texto
> respecto de lo que hay debajo. El origen se convierte a la esquina superior izquierda, como en el marcado, una
> sola vez y no en cada consumidor.
>
> Cuando no hay texto, la capa se devuelve **declarada como ausente**, no como una lista vacía que sería
> indistinguible de una página en blanco. La superposición visual existe
> (`ui/lector/ReadingSurface.svelte`): los fragmentos van encima de la imagen, transparentes y posicionados en
> porcentaje, así que escalan con ella.
>
> Esos fragmentos son además **las unidades que el teclado selecciona** en un PDF, igual que los párrafos lo son
> en un EPUB. Que sean los mismos que el runtime busca al anclar una cita no es casualidad y no se confía a la
> suerte: hay una prueba que toma el texto de la capa, lo usa como cita y comprueba que ancla exacto.

Para formatos que se renderizan como imagen (PDF, cómic), el proveedor debe exponer la capa de texto para que la
interfaz la superponga de forma accesible al lector de pantalla. Cuando el documento no tiene capa de texto
(escaneo sin OCR), el sistema debe declararlo explícitamente al usuario en lugar de presentar un documento
silenciosamente ilegible para tecnologías de asistencia.

Este punto tiene además implicaciones regulatorias para el uso en instituciones europeas. Ver
[legal/accessibility-compliance.md](../legal/accessibility-compliance.md).

## 5.1 Las anotaciones no se editan

> **Implementado**: `src-tauri/src/annotations.rs` y `V7__immutable_annotations.sql`.

Una anotación es inmutable. Editarla es **darla de baja y crear otra**, con identificador nuevo, que apunta a la
anterior por `supersedes`. La anterior se conserva.

Cuesta más que un `UPDATE`, y compra tres cosas concretas: que "me he cargado la nota buena" tenga arreglo; que
la sincronización entre dispositivos no tenga que decidir quién escribió último sobre la misma fila, porque dos
versiones no compiten sino que conviven; y que exportar el historial sea exportar filas en lugar de reconstruir
un pasado que nunca se guardó.

La regla no vive solo en Rust. Dos disparadores la hacen cumplir también por SQL directo: uno rechaza cualquier
`UPDATE` de las columnas que definen la anotación, y otro impide revivir una baja escribiendo nulo encima.
Reaparecer sin rastro es justo lo que este modelo evita: recuperar una anotación borrada es crear una versión
nueva que la suceda, y entonces queda registrado que ocurrió.

Un **resaltado sin cita no se acepta**. Sin cita no hay segundo escalón de la cascada de la sección 3, así que
ese resaltado se perdería en cuanto el documento cambiase, y aceptarlo sería prometer un reanclaje que no puede
ocurrir. Un marcador sí: es un punto, no un trozo de texto.

## 5.2 El contrato de consulta sobre la selección

> Implementado en `src-tauri/src/lookup/`: diccionarios locales, panel de consulta y proveedores en línea.

Diccionario, enciclopedia y traducción son **la misma operación** vistas desde la interfaz: se recibe una
selección y se devuelve algo legible junto a ella, sin perder la posición de lectura. Por eso hay un contrato y
no tres funciones: escribirlas por separado obligaría al panel a saber a quién preguntar, y añadir después un
traductor en línea o un modelo local significaría reescribirlo.

Es el mismo patrón que el contrato de formato de la sección 4, que el proveedor de voz y que el
servicio de IA.

### Lo que un proveedor declara de sí mismo

| Dato | Para qué |
|---|---|
| Identificador y nombre legible | Procedencia: el usuario distingue lo que dice un diccionario de lo que dice una máquina |
| Clase: diccionario, enciclopedia o traducción | Fija el orden estable de los resultados |
| Idiomas, en etiquetas BCP-47 | Decide si participa. Lista vacía significa "cualquiera" |
| **Si necesita red** | Lo declara él; nadie lo deduce desde fuera |
| **A dónde saldría esta consulta** | Por consulta y no fijo: el destino de la enciclopedia depende del idioma del documento. Es lo que permite pedir permiso **para ese destino** en lugar de un permiso genérico |
| **Si un anfitrión es suyo** | Lo comprueba el runtime antes de conceder un permiso, para que el destino no lo decida el webview |

Que la necesidad de red la declare el proveedor es lo que permite cumplir la regla 3 de
[ADR-013](../adr/013-information-architecture.md) —lo que sale del dispositivo se anuncia—
sin mantener en la interfaz una lista de cuáles salen.

### Qué pasa cuando alguien no puede responder

**Se omite, y se dice por qué.** Un proveedor que no cubre el idioma del documento no puede aparecer como una
pestaña vacía, y uno que falla no puede llevarse por delante a los demás: es la regla 1 de la Constitución.

La consulta devuelve, junto a las entradas, la lista de quién quedó fuera y su motivo. Eran tres y
son **cuatro**, porque el motivo de red resultó ser dos:

| Motivo | Qué significa | Qué ofrece la interfaz |
|---|---|---|
| Idioma no cubierto | Ningún proveedor de esa clase responde en el idioma del documento | Instalar el idioma que falta |
| Modo sin red | El interruptor general está puesto, así que no se intentó salir | Apagar el modo sin red |
| Destino no autorizado | Hay que salir a un sitio que el usuario todavía no ha permitido | Conceder el permiso, **diciendo antes qué se envía** |
| Fallo | Lo intentó y no pudo | Reintentar |

Partirlo no fue una preferencia: es la misma distinción que ya hacían `NETWORK_DISABLED` y `CONSENT_REQUIRED` en
el contrato de errores, y con un solo motivo la interfaz ofrecería la acción que no toca la mitad de las veces.
Por eso el omitido por falta de permiso viaja además **con su anfitrión**: sin él se podría decir que falta un
permiso y no para qué.

**Falta de permiso no es un fallo.** Un proveedor al que la puerta de red le dice que no, no ha fallado: es una
decisión que el usuario no ha tomado todavía, y contarla como fallo dejaría al panel diciendo "no respondió".

**Sin ningún proveedor instalado** la consulta responde `COMPONENT_UNAVAILABLE`, que es "esta instalación no
sabe consultar" y la interfaz oculta la función. Es deliberadamente distinto de que existan proveedores y
ninguno cubra el idioma: eso no es error, es una respuesta vacía **explicada**, y la interfaz debe decir qué
falta instalar. La misma distinción que en la sección 4 entre `COMPONENT_UNAVAILABLE` y `FORMAT_UNSUPPORTED`.

### Diccionarios locales

> Existe en `src-tauri/src/lookup/stardict.rs` y `lookup/store.rs`.

El formato adoptado es **StarDict**. El motivo no es
técnico: es que **es el formato en el que ya existen los diccionarios**. Lo leen KOReader y GoldenDict, y en él
se distribuyen las obras de licencia libre en español y en inglés, que son los dos idiomas con los que se
inaugura el catálogo. Un formato propio nos obligaría a convertir cada obra a mano y dejaría al usuario sin
poder traer la suya.

Leerlo no costó ninguna dependencia nueva: `.dict.dz` es gzip válido, y `flate2` ya estaba en el árbol desde
La implementación.

**Se normaliza al instalar**, no al consultar, y eso tiene tres consecuencias que importan:

1. **La validación ocurre antes.** Un diccionario truncado, con el índice apuntando fuera de las definiciones, o
   con un marcado que no sabemos convertir a texto plano, se rechaza **al instalarlo**. Aceptarlo y fallar en
   cada consulta sería peor: el usuario creería tenerlo y no entendería por qué nunca responde.
2. **La consulta es una búsqueda binaria con dos posicionamientos por sonda**, sin cargar el índice en memoria
   ni descomprimir nada. Un diccionario de medio millón de palabras son decenas de megabytes de índice, y
   tenerlos residentes por cada idioma instalado no cabe en el presupuesto de memoria.
3. **Ningún diccionario entra en el instalador.** Llegan siempre de fuera.

**La lista de diccionarios instalados es una carpeta**, no una preferencia ni una tabla. Eso hace que
"desinstalar no deja rastro" sea cierto por construcción y no por disciplina: no hay un segundo sitio del que
acordarse de borrarlo.

Un diccionario **sin idioma declarado no se instala** salvo que el usuario lo indique: sin idioma no hay forma
de decidir cuándo ofrecerlo, y acabaría respondiendo a documentos de cualquier lengua.

Los diccionarios son obras de terceros con licencia propia. Ninguno entra al catálogo descargable sin revisión
de licencia registrada, igual que los modelos, y la atribución que su licencia exija viaja en el
campo de procedencia de cada entrada.

### Idiomas

La comparación es por **subetiqueta primaria**: un diccionario que declara `es` responde a un documento en
`es-CO`. Exigir coincidencia exacta dejaría sin diccionario a casi todo el mundo. Un documento sin idioma
declarado se consulta igual: ofrecer una consulta de más cuesta una lista vacía; no ofrecerla cuesta la función.

### Proveedores en línea

> Existen en `src-tauri/src/lookup/online.rs`.

Dos, y entran por el mismo contrato sin que el panel cambie una línea. Esa es la prueba de que el contrato
servía para lo que se escribió.

**La enciclopedia** consulta Wikipedia en el idioma del documento. Trae destino porque no hay una segunda
respuesta razonable a "una enciclopedia abierta": no es una elección comercial, es la obra. El anfitrión sale
del idioma, y el idioma lo escribe quien hizo el EPUB, así que **se valida antes de convertirlo en un
anfitrión**: sin eso, el destino de la petición lo elegiría un tercero.

**El traductor** no trae ninguno. Traducir manda el texto que se está leyendo a un servidor ajeno, y elegir por
el usuario a quién no le corresponde al programa. Habla el protocolo de LibreTranslate —software libre, que se
puede levantar en la propia máquina— y sin anfitrión configurado **no se registra**: un proveedor que aparece en
el panel y nunca puede responder es peor que ninguno. Es la misma regla que se aplica a los catálogos.

Los dos traducen o resumen **con el párrafo como contexto** cuando lo hay: una palabra suelta sin su frase da la
acepción equivocada la mitad de las veces, y ese es exactamente el campo `context` que declara el contrato.

Ninguno de los dos tiene cliente HTTP: tienen la puerta de `net.rs`, que comprueba el interruptor y el
consentimiento antes de abrir nada. No hay forma de que se la salten.

## 5.3 Anclar lo que el usuario acaba de seleccionar

> **Implementado**: `locator_for_quote` en el contrato de proveedor, el comando
> `locator_for_selection` y la selección por bloques de `ui/lector/`.

Para anotar hace falta convertir una selección en un `Locator`. La selección la produce el webview, que es **zona
no confiable**, y la cita es justo lo que sostiene el segundo escalón de la cascada de la sección 3. Por eso la
cita **se busca en el documento en lugar de creérsela**: si aparece, el locator queda anclado donde de verdad
está; si no aparece, no hay anotación. Una anotación con una cita que no está en el libro no se podría reanclar
nunca, y el usuario no se enteraría hasta perderla.

Entre dos apariciones de la misma frase gana **la más cercana a donde se está leyendo**, y la pista la pone la
sesión, no quien llama: quien selecciona está leyendo ahí. Sin ese desempate, seleccionar una frase corriente del
capítulo veinte deja la nota en el capítulo uno.

### Qué se selecciona, y por qué no son caracteres

La unidad es el **bloque del documento**: un párrafo en un formato de marcado, un fragmento de la capa de texto
en uno que se pinta como imagen. No es una simplificación:

* **No hay navegación por cursor de texto garantizada en un webview.** WebKitGTK, WebView2 y WKWebView no
  coinciden, así que una selección que dependiera de ella funcionaría en una plataforma y no en otra. El criterio
  es que el flujo entero sea operable con teclado en las tres.
* **Es la misma regla para los dos formatos.** Una regla de selección por formato serían dos reglas distintas en
  cuanto una de las dos se tocara.
* **Hace que anotar cueste una tecla.** Estar sobre un párrafo ya es tenerlo seleccionado, y de ahí sale
  directamente la nota. Es lo que exige la sección 1.1 de la
  [investigación de la experiencia de lectura](../design/reading-experience/README.md): anotar tiene que
  costar menos que resaltar.

### 5.3.1 Cómo se pintan las marcas sobre el documento

> **Implementado** en `src/lib/ui/lector/proyeccion.ts`, `src/lib/ui/lector/resaltados.ts` y
> `src/lib/ui/lector/ReadingSurface.svelte`.

El anclaje en el texto se calcula una sola vez por capítulo o página con `proyectarEnNodos` (`proyeccion.ts`): se
recorre la secuencia de nodos de texto reales del contenedor —saltando las capas superpuestas `aria-hidden="true"`—
y se construye el `Range` exacto que abarca la cita, verificando que `rango.toString` coincida con lo que dice la
anotación.

Las marcas no se registran en `CSS.highlights` (`::highlight(...)`): en WebKitGTK, tener rangos vivos en
`CSS.highlights` hace que **cualquier inserción de nodo o cambio de texto en
cualquier parte del documento** cueste en proporción al número de rangos y a la longitud del capítulo (456–485 ms
por mutación con 20 rangos en un capítulo de 1.600 párrafos, frente a 20 ms sin ellos). Por eso
`CSS.highlights.size === 0` siempre, y las marcas (`highlight`, `note`, `underline`) se pintan así:

* Se miden los rectángulos de cliente de cada `Range` (`rango.getClientRects`) con `medirMarcas` (`resaltados.ts`),
  fundiendo en una sola caja los rectángulos colineales de una misma línea cuando la lectura biónica parte cada
  palabra en `<b>` y el resto del nodo de texto (`fundirRectangulos`), pero manteniendo separados los rectángulos de
  líneas o columnas paginadas distintas.
* Se pintan en una capa dedicada `.nb-capa-marcas` (`aria-hidden="true"`, `pointer-events: none`), colocada con
  `z-index: -1` dentro de `.nb-texto` (`isolation: isolate`) para quedar **detrás de los glifos del texto** y encima
  del fondo de `.nb-superficie`, y con `z-index: 1` y tintas translúcidas (`color-mix(..., transparent)`) sobre la
  imagen en `.nb-pagina` (PDF).
* El fondo del párrafo con foco o seleccionado usa `color-mix(..., transparent)` para no tapar las marcas que van
  detrás del texto, y el foco de línea (`data-foco-de-linea="true"`) atenúa las marcas de los bloques no enfocados
  (`opacity: 0.35`) igual que atenúa sus párrafos.
* La geometría se recalcula al cambiar cualquier propiedad de composición (`cuerpo`, `interlineado`, `margen`,
  `ancho`, `familia`, `alineacion`, `paginado`, kit de dislexia/biónica) y cuando `ResizeObserver` detecta un cambio
  de tamaño del lienzo.

## 5.4 El contrato de proveedor de voz

> Implementado en `src/lib/voz/`: la síntesis del sistema, la voz neuronal local y la de un servicio entran por el
> mismo contrato.

**Vive en el frontend y no en Rust**, a diferencia de los otros contratos, y el motivo es que el primer
proveedor del navegador solo existe en el webview. El proveedor neuronal vive en Rust y entra como un proveedor
más cuya síntesis cruza el puente; el audio se
reproduce donde se reproduce todo, que es la ventana.

### Lo que transporta, y por qué no es solo audio

**Marcas ancladas al `Locator`.** Es la parte que la ficha subraya, y la razón es que el seguimiento
visual del texto no se puede añadir después sobre un contrato que solo lleve sonido: haría falta saber, en cada
instante, qué parte del documento se está diciendo. Un proveedor que no sepa alinear emite una marca por tramo,
que es lo mínimo para resaltar por dónde va; uno que sepa más, una por palabra, y lo **declara**.

| Dato | Para qué |
|---|---|
| Identificador | Procedencia: el usuario sabe si le habla el sistema o un modelo |
| **Si puede funcionar en esta máquina**, con su motivo | Lo declara él. WebKitGTK no siempre trae síntesis, y eso se dice en lugar de enseñar un botón mudo |
| Si necesita red | Lo declara él; nadie lo deduce desde fuera |
| Si sabe marcar por palabra | Decide la finura del seguimiento visual, no si lo hay |

### Qué se narra

**Bloques**, que es lo mismo que se selecciona y lo mismo que se ancla. De ahí sale, sin ninguna rama por
formato, que la voz funcione "sobre EPUB y sobre PDF con capa de texto": la superficie ya entrega párrafos del
marcado en uno y fragmentos de la capa de texto en el otro. Un escaneado sin OCR no tiene bloques, y ahí no hay
nada que narrar: se declara con `motivoSinTexto`, que es la otra mitad del criterio.

### Una sola voz en toda la aplicación

Hay una salida de audio. Con la vista dividida hay dos lectores y sigue habiendo una voz: narrar dos documentos
a la vez no se entiende, y un narrador por mitad obligaría a inventar quién calla a quién. Empezar a narrar en
una mitad para la otra, como en cualquier reproductor.

## 5.5 La narración que trae el propio libro

> **Implementado**: `reader/epub.rs` (`superposiciones_de`, `pares_de`, `reloj`), el comando `synchronized_audio`,
> `src/lib/ui/lector/NarracionDelLibro.svelte`, y `tests/narracion_del_libro.rs` detrás.

Un EPUB 3 puede traer **superposiciones** -Media Overlays-: un SMIL por capítulo que dice qué tramo de qué audio lee
qué elemento del texto.

**Cómo se ancla.** Cada `par` dice `capitulo.xhtml#f12`, y un fragmento por identificador no es un `Locator`. El
desplazamiento del elemento se calcula en **la misma pasada** que el texto plano del capítulo, con una marca que la
normalización quita sin dejar rastro, así que cae sobre exactamente el mismo texto que usan las anotaciones y la
posición de lectura; una prueba comprueba que el texto no se mueve ni un carácter. Con él se construye el `Locator`
del tramo por la función de siempre, `locator_at`.

**Cómo suena.** Con el reproductor de lo preparado de la voz: un audio y sus marcas por `Locator` tienen la
misma forma, y para quien escucha son lo mismo. El audio se extrae a la caché y se pide por el protocolo de assets
como `blob:`, que es lo único que WebKitGTK reproduce. Cada tramo que empieza mueve el lector ahí -y con eso se
guarda la posición, que sobrevive a cerrar y volver a abrir- y el bloque que suena **se resalta con el mismo
atributo y el mismo desplazamiento que la voz en vivo**: el reproductor dice el `Locator`, el lector lo convierte en
el bloque que lo contiene. Al acabar un capítulo se pide el siguiente que traiga narración y se sigue.

**Dónde se ofrece.** En el panel de voz, **encima** de la voz sintética y como la recomendada, sin esconderla:
recomendar no es obligar. No añade destino ni modo. Un libro sin superposiciones no declara la capacidad y se
comporta exactamente como antes.

**Lo que no hace, dicho:**

* **Un archivo de audio por capítulo.** Si la narración de un capítulo se reparte entre varios, se escucha hasta
  donde llega el primero. Es como los publican casi todos, y el reproductor sigue un archivo cada vez.
* **AAC no suena en el AppImage**: su decodificador no está en los complementos LGPL que la política de licencias
  deja distribuir. MP3, Opus y WAV sí. Si no suena, el panel lo dice.
* **Una grabación de otra edición** -la pregunta J7- no se alinea: esto solo sigue lo que el libro trae alineado.
* Las clases de estilo que el libro declara para lo que suena (`media:active-class`) no se aplican: el resaltado es
  el del lector.

## 6. Progreso de lectura

> **Implementado**: `reader/progress.rs` y `V6__progress.sql`.

El progreso se persiste como Locator, no como entero. Solo así el progreso sobrevive a un cambio de tamaño de
fuente, a la sincronización entre dispositivos con pantallas distintas y a la reimportación del documento.

Cuelga de la **edición** y no del archivo: el archivo puede moverse, volver a importarse o existir en dos copias,
y en los tres casos el usuario espera seguir donde estaba.

### 6.1 Antirrebote, y por qué no puede perder la última posición

Leer mueve la posición constantemente, así que escribir cada movimiento convertiría pasar una página en decenas
de transacciones. Las escrituras se agrupan: como mucho una por documento cada cinco segundos.

Lo que hace que eso no sea una pérdida es que **cerrar vacía lo pendiente**. Un antirrebote que guarda "cada
cinco segundos" y se olvida de lo que quedaba al cerrar produce exactamente el defecto que esta iteración existe
para impedir: el usuario cierra el libro donde lo dejó y al reabrirlo aparece cinco segundos más atrás. Por eso
`close_document` es parte del contrato y no una cortesía de la interfaz, y por eso el vaciado ocurre también al
desalojar una sesión por el tope de documentos abiertos y al salir de la aplicación.

Queda una ventana declarada: si el proceso muere de golpe se pierde lo apuntado desde la última escritura, como
mucho ese intervalo. Cerrarla del todo obligaría a escribir en cada movimiento, que es lo que el antirrebote
viene a evitar.

**Salir del lector sin cerrar el libro también escribe lo pendiente**. El lector no se cierra
al ir a otro destino, y la biblioteca y el lector tienen que decir la misma cifra: `flush_progress` escribe la
última posición apuntada sin terminar la sesión. Y cada escritura, venga de donde venga, se anuncia en el bus como
`ProgressSaved` con su progresión, que es lo que mueve la barra de las demás pantallas sin volver a preguntar.

**Llegar al final es estar en el último bloque.** La posición que se apunta al desplazarse es el primer bloque que
Se ve; con el final del capítulo en pantalla, ese bloque queda unos párrafos antes del último, y un libro leído
hasta la última línea se quedaba en el 95 % para siempre, sin llegar nunca a terminado. Cuando la vista llega al
final de lo que hay que leer, la posición es el último bloque.

### 6.2 Restaurar no es empezar

Al abrir, el progreso guardado manda sobre el principio del documento. Si ya no resuelve exactamente, se coloca
donde la cascada de la sección 3 alcance y **se marca**: devolver al principio en silencio pierde trabajo del
usuario sin decírselo. Un progreso almacenado que no se deja interpretar es un error, no un "sin progreso", por
el mismo motivo.

La sesión distingue además "no hay progreso guardado" de "el progreso apunta al principio". Una cosa es no haber
empezado y otra haber vuelto a la primera página a propósito.

## 7. Alineación con estándares

El proyecto se alinea deliberadamente con el modelo de Readium en lugar de inventar uno propio: reduce el riesgo
de longevidad, facilita la interoperabilidad de exportación de anotaciones y permite adoptar en el futuro
componentes del ecosistema sin rediseñar el
núcleo. Ver [ADR-004](../adr/004-reader-locator-model.md).

## 8. Presentación y apertura de documentos

**Implementado:** el contador de PDF usa la página física y el total de páginas, incluso cuando toda su navegación
comparte el mismo recurso. La rasterización solicita hasta 2.000 píxeles de ancho y adapta la resolución a la
proporción de la página para no superar 5.000.000 de píxeles. La clave de caché incluye ambos límites para no reutilizar imágenes anteriores
de menor resolución o superiores al presupuesto. La cabecera usa el título de la sesión.

Una posición solicitada desde búsqueda se descarta al abrir otro documento o cerrar el actual. Así, un locator
pendiente no se guarda como posición del documento siguiente. La lista de continuar leyendo ordena por la
última lectura e incluye una apertura reciente aunque todavía marque cero de progreso.

Los modos de lectura aportados por extensiones se propagan hasta la superficie de lectura y sus colores de
fondo. No basta con que aparezcan en el selector si el lector no recibe su definición.
