# Gestión de Prompts

> **Estado:** Implementado: plantillas y lectura de salidas (`src-tauri/src/ai/plantillas.rs`, `ai/salidas.rs`),
> tratamiento del contenido no confiable y recuperación de contexto (`ai/contexto.rs`).

Los prompts son código: cambian el comportamiento del producto, tienen regresiones y necesitan control de
versiones y pruebas.

## 1. Reglas de gestión

* Los prompts viven en archivos versionados dentro del repositorio, en
  [`src-tauri/prompts/`](../../../src-tauri/prompts/README.md), nunca incrustados en la lógica de UI. Entran en el
  binario con `include_str!`: la aplicación instalada no lee instrucciones del disco.
* Cada plantilla tiene identificador y versión (`estudio.resumen`, versión 1). **Cambiar el texto es cambiar la
  versión**, y no depende de acordarse: una prueba guarda la huella de cada texto junto a su versión y falla si
  el texto cambia sin que la versión lo haga.
* Todo cambio de plantilla exige pasar la suite de evaluación descrita en [evaluation.md](evaluation.md).
* Las plantillas se separan del idioma: la instrucción declara el idioma de salida como variable. Hoy están
  escritas en español, que es el idioma del corpus de evaluación que existe; medir si una instrucción en inglés
  da mejor resultado con los modelos pequeños es trabajo, no una suposición de este documento.

## 2. Estructura de una plantilla

Cada plantilla declara en su cabecera: identificador, versión, propósito, capacidad, formato de salida y
variables. El cuerpo tiene dos secciones, `# sistema` y `# tarea`. La longitud máxima de contexto no es de la
plantilla sino de quien la ejecuta (32.000 caracteres para un proveedor externo; la ventana del modelo local para
el local), y los modelos compatibles son todos los que atienden la capacidad: la salida se valida igual venga de
donde venga.

**El texto del documento no es una variable.** Lo coloca el runtime entre la instrucción y la tarea, y una
plantilla que intente nombrarlo (`{{fragmentos}}`) no se deja leer. Si fuera una variable, la separación entre lo
que manda el producto y lo que trae un libro dependería de que nadie la pusiera en la instrucción del sistema.

La salida estructurada (JSON) es obligatoria para funciones que alimentan la interfaz. Se lee con
`ai::salidas::leer`, que **no falla nunca**:

* Busca el primer objeto JSON del texto, aunque venga dentro de un bloque de código o tras una frase.
* Lee **elemento a elemento**: lo que cumple se queda, lo que no se cuenta como descartado. Una tarjeta mal
  escrita no hace perder las otras nueve.
* Cada campo se limpia de espacios y se corta a 600 caracteres; se leen como mucho 30 elementos.
* Una cita solo vale si apunta a uno de los fragmentos entregados (`F1` a `Fn`). Las demás se quitan y se
  cuentan; un elemento sin citas válidas es **no verificable**, y así se enseña.
* Si no queda nada, la lectura es *ilegible*: la interfaz lo dice y ofrece repetir, en lugar de un error.

Las reglas propias de cada formato: una tarjeta cuya respuesta repite la pregunta se descarta; una pregunta
necesita de tres a cinco opciones distintas y una clave que exista; los conceptos repetidos se juntan con las
citas de todos.

## 3. Construcción de contexto (RAG)

Nunca se envía un documento completo. El flujo, en `ai::estudio::hacer`:

1. **Un documento y solo uno.** El alcance es un capítulo o una selección de **una** edición, y los fragmentos salen
   de abrir esa edición. No hay forma de pedir una función sobre dos documentos.
2. **Los fragmentos del índice**, con su localizador: los mismos que la búsqueda, troceados por párrafo.
   En un EPUB el capítulo es su recurso; en un PDF, las páginas entre una entrada del índice y la siguiente.
3. **Si no caben, repartidos.** Quien genera declara cuántos caracteres admite (32.000 un externo, 6.000 el local)
   y como mucho se entregan 24 fragmentos. Si el alcance no cabe, se toma uno de cada tramo de todo el capítulo en
   lugar de su principio, y la respuesta dice que se recortó.
4. **Citar por número.** El modelo recibe `F1`…`Fn`, nunca el localizador, que no sale del dispositivo; al leer, el
   número vuelve a ser el localizador y la interfaz abre el sitio exacto.
5. **La cita del modelo se comprueba, no se cree.** Medido el 2026-09-17 con Qwen2.5-1.5B: hubo tarjetas correctas
   citando el fragmento de al lado y preguntas cuyo fragmento citado no decía nada de la respuesta. Una cita que
   abre un sitio que no sostiene lo que afirma es **peor que ninguna**, porque parece verificada. Así que
   `estudio::comprobar_las_citas` mide el apoyo léxico de cada afirmación en el fragmento que dice citar: si lo
   sostiene, se queda; si no, se busca el que lo sostiene; y si ninguno lo sostiene, el elemento se queda **sin
   cita**.
6. **Lo que no cita nada es no verificable**, y así se enseña. Lo que se comprueba es que la afirmación salga de ese
   fragmento y no de la nada; **que sea cierta no lo dice nadie**, y por eso la marca existe y el resumen se lee con
   el libro al lado.

**La clave de una pregunta se pide copiada, no numerada.** El mismo modelo contestaba `"correcta": 1` señalando la
primera opción, contando desde uno aunque la plantilla dijera desde cero; una pregunta con la clave corrida es
exactamente el daño que describe `philosophy.md`. Copiando la opción no hay nada que desplazar, y el runtime la busca
entre las opciones.

Cuando el capítulo no cabe se reparte, en lugar de recuperar por significado: los vectores son opcionales, y
repartir funciona sin ellos.

## 4. Contenido no confiable

El texto de un documento importado es **entrada no confiable**. Un documento puede contener instrucciones
dirigidas al modelo (inyección de prompt indirecta) que intenten alterar su comportamiento, extraer contexto de
otros documentos o inducir salidas dañinas.

**Ninguna composición impide que un modelo obedezca un texto.** Lo que se garantiza es que obedecer no sirva de
nada, y cada mitigación tiene su mecanismo:

| Mitigación | Mecanismo |
|---|---|
| Separación estructural entre instrucciones y contenido | `ai::Request` separa `instruction`, `passages` y `task`; `ai::contexto::mensajes` pone el documento solo en el mensaje del usuario, entre delimitadores y tras un aviso de que es texto de un libro. Una plantilla que nombre el documento no se deja leer |
| Un fragmento no puede salir de su delimitador | `contexto::neutralizar` sustituye **todos** los signos de etiqueta por comillas angulares y quita caracteres de control y de formato invisibles. Un libro no puede escribir `</fragmento>` ni fabricar un `<fragmento id="F9">` |
| Sin contexto de otros documentos | El alcance es de una edición y los fragmentos salen de abrirla: el modelo nunca ha visto otro libro |
| Sin herramientas con efectos secundarios | El modelo no tiene herramientas. Su salida se lee como datos (`ai::salidas::leer`): los campos que no existen en el formato —una "acción", una "url"— se ignoran |
| Citas solo a lo entregado | La lectura quita toda cita que no sea `F1`…`Fn` y la cuenta |
| Salidas como texto plano | El panel del Lector y Estudio pintan lo generado como texto de Svelte, nunca con `{@html}`, y no convierten nada en enlace |
| Nada se dispara sin confirmación | Guardar tarjetas o conceptos exige pulsar; ninguna salida borra, exporta ni envía |

**La prueba del criterio** (`tests/estudio_adversario.rs`) usa `adversario-prompt.txt` contra el peor modelo posible:
uno que **obedece** al documento, repite todo lo que ha visto, cita fragmentos que no existen y escribe marcado con
código y una acción de enviar. Se comprueba que la instrucción y la tarea son idénticas con el documento adversario
y con uno benigno, que nada de otro libro llega al modelo ni a la salida, y que de lo obedecido solo queda texto con
sus citas inválidas quitadas. Un recorrido de interfaz comprueba que el marcado se enseña como caracteres y no se
ejecuta.

## 5. Idioma y registro

Las respuestas se generan en el idioma del documento o en el idioma de la interfaz, según preferencia del
usuario. En contexto educativo el registro debe ser neutro y explicativo, sin florituras.
