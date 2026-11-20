# Evaluación de Calidad de IA

> **Estado:** Implementado: corpus de evaluación, rúbrica automática, métricas medidas y publicadas (sección 8) y
> reporte de salidas incorrectas (sección 6). No hay modelo juez (sección 5), que se permite y no se exige. **Las
> tarjetas superan la rúbrica en cinco de seis textos**, y se publica así.
> Sin evaluación sistemática, "mejoramos el prompt" es una afirmación sin evidencia.

## 1. Por qué existe este documento

En un producto educativo, una salida incorrecta que suena convincente causa daño real: el estudiante memoriza un
error. La evaluación no es un lujo de madurez, es un requisito de responsabilidad.

## 2. Conjuntos de evaluación

Se mantiene un corpus versionado de documentos **libres de derechos** (dominio público, material con licencia
abierta o textos sintéticos) con salidas de referencia revisadas por una persona.

Cobertura mínima:

* Textos en español, inglés y portugués.
* Un texto técnico con fórmulas y tablas.
* Un texto narrativo largo.
* Un artículo académico con citas y bibliografía.
* Un documento con estructura pobre (PDF escaneado con OCR imperfecto).
* Un documento que contiene instrucciones adversarias dirigidas al modelo.

## 3. Métricas por función

| Función | Qué se mide | Método |
|---|---|---|
| Resumen | Fidelidad al texto, cobertura, ausencia de invenciones | Revisión humana con rúbrica y verificación de citas |
| Flashcards | Corrección factual, no trivialidad, respuesta única | Rúbrica y muestreo humano |
| Preguntas de autoevaluación | Ambigüedad, dificultad, corrección de la clave | Rúbrica |
| Búsqueda semántica | Precisión y exhaustividad sobre consultas etiquetadas | Métricas de recuperación (precisión@k, recall@k) |
| Extracción de entidades | Precisión y exhaustividad | Comparación con anotación de referencia |
| Traducción | Adecuación y fluidez | Revisión humana en muestra |

Adicionalmente se miden latencia, uso de memoria y tasa de fallo por proveedor y modelo.

## 4. Proceso

1. Toda modificación de plantilla, modelo por defecto o estrategia de recuperación ejecuta la suite.
2. Los resultados se registran con fecha, versión de plantilla y versión de modelo.
3. Una regresión en fidelidad o en precisión de recuperación bloquea el cambio.
4. Los resultados se publican en el repositorio para que la comunidad pueda auditarlos.

## 5. Uso de un modelo como juez

Se permite usar un modelo para preclasificar salidas y priorizar la revisión humana, siempre que:

* El juez sea distinto del modelo evaluado.
* Se calibre contra un subconjunto etiquetado por personas.
* Nunca sea la única evidencia para aprobar un cambio que afecta la corrección factual.

## 6. Reporte de errores por el usuario

Cada salida generada incluye una acción discreta para marcarla como incorrecta. Ese reporte es local por defecto;
enviarlo al proyecto requiere consentimiento y no debe incluir el texto del documento salvo que el usuario lo
autorice explícitamente para ese reporte.

Cómo se construyó y qué se decidió, en la sección 8.

## 7. Lo construido

**El corpus inicial** está en `src-tauri/tests/fixtures/evaluacion/`, con su procedencia y licencia en el README:
cuatro textos **escritos para el proyecto** —técnico en español, narrativo en inglés, académico en portugués y uno
con estructura de OCR roto— más el adversario. Cada uno trae en `referencias.json` los hechos que una
buena salida tiene que poder sostener. **Faltan**, y siguen: un texto con fórmulas y tablas, uno narrativo
largo, y que **una persona distinta de quien los escribió revise las referencias**, que es lo que pide la sección 2.

**La rúbrica automática** es `ai::rubrica::evaluar`. Mide lo que se puede medir sin una persona:

| Medida | Qué comprueba | Umbral |
|---|---|---|
| Elementos | Que haya al menos tres | 3 |
| Citadas | Proporción de elementos que citan algún fragmento entregado | 80 % |
| Sostenidas | Proporción cuya respuesta —o punto, o clave de la pregunta— tiene al menos el 60 % de sus palabras con contenido en el texto de lo que cita | 70 % |
| No triviales | Proporción cuya pregunta no contiene la respuesta y cuya respuesta no es un párrafo entero | 90 % |
| Repetidas | Dos elementos con la misma pregunta | Ninguna |
| Cobertura | Fragmentos distintos citados sobre los entregados | Se mide, no bloquea |

**Los umbrales se fijaron antes de medir ningún modelo con ellos**, para que no se ajustaran a lo que saliera.

**Las citas llegan ya comprobadas.** El runtime verifica cada cita contra el fragmento que dice citar
y la corrige o la quita (`estudio::comprobar_las_citas`), con la misma medida de apoyo. Así que sobre una salida del
producto, "citadas" y "sostenidas" miden lo mismo: **qué parte de lo que escribió el modelo sale de verdad del
capítulo**. Es la medida contra la invención, y no cambia por dónde se aplique; lo que ya no mide es si el modelo
sabe citar, que es una propiedad del modelo y no de lo que el usuario acaba viendo.

**Lo que la rúbrica no puede decir, y por eso no aprueba sola**: "sostenida" es una aproximación léxica, no una
comprobación de verdad —una respuesta que reordena palabras del texto hasta decir lo contrario pasaría—; y que una
pregunta sea importante, que sus distractores sean plausibles o que su clave sea la única defendible sigue siendo de
una persona (sección 5). La rúbrica decide cuándo algo **no llega**.

**La suite** es `tests/evaluacion_generativa.rs`. Se salta sola sin `NB_MODELOS`, porque exige el modelo descargado y
minutos por texto; se ejecuta a mano con AVX2 compilado, y con `NB_EVALUACION` escribe los resultados en JSON. Con
`NB_MODELO` se mide **uno de los tres del catálogo**; sin él, el que se recomienda para el equipo. Mide lo que el
proveedor declara —el local no atiende la autoevaluación— y su aserción es el criterio: **las tarjetas
superan la rúbrica en todo el corpus**. Los resúmenes y los conceptos se miden y se publican: un resumen que se
queda corto no es una tarjeta equivocada, y confundirlos haría que un umbral de estilo bloqueara una función
correcta. Que una modificación de plantilla obligue a volver a medir lo hace cumplir
`scripts/check-evaluacion.mjs` (sección 8, "Plantillas sin medir").

## 8. Lo medido y lo publicado

### El corpus, completo

Seis textos en tres idiomas. Entre ellos, **`formulas-es.txt`**, con fórmulas de riego
y una tabla de coeficientes -la clase que pone a prueba si una tarjeta pregunta por una cifra que está en el texto
o por una que se inventó-, y **`narrativo-largo-en.txt`**, cuatro mil caracteres en cinco partes, que es la clase
donde un resumen puede perder el orden de los hechos. Los dos están escritos para el proyecto, con su procedencia
y su licencia en el README de la carpeta.

**Y el corpus se sostiene solo**: `tests/corpus_de_evaluacion.rs` comprueba que cada texto tiene hechos de
referencia, que cada hecho está sostenido por su texto -tres de cada cinco palabras con contenido, y no todas,
porque el texto de OCR tiene las palabras partidas a propósito- y que las seis clases exigidas están cubiertas en
los tres idiomas. Existe porque un corpus de evaluación se pudre en silencio: alguien corrige una frase, el hecho
de referencia deja de estar en el texto, y la rúbrica sigue dando un número que ya no significa lo que decía.

Lo que esta prueba no sustituye es la revisión humana de los hechos de referencia: una comprobación automática no
se presenta como tal.

### Las métricas de recuperación, medidas

| Medida | Umbral fijado antes | Medido |
|---|---|---|
| Acierto entre los 5 primeros | 0,80 | **0,95** (18 de 19 preguntas) |
| Rango recíproco medio | 0,60 | **0,81** |
| Vectorizar la consulta | — | mediana **29 ms**, peor 36 ms |

Diecinueve preguntas sobre cuatro textos del corpus, en tres idiomas, **escritas con palabras distintas de las del
párrafo que las contesta** siempre que se pudo: si la pregunta repite la frase del texto, lo que se mide es la
coincidencia de palabras y no el significado, que es justo lo que esta búsqueda aporta sobre la léxica.

La única que se fue fuera de los cinco primeros salió en la décima posición, y es instructiva: *"what did she
notice when she compared the money with the carts?"* sobre el texto largo, donde el párrafo que lo cuenta habla de
peajes y de un tercio y no de comparar. El modelo prefirió el párrafo que resume la conclusión. No es un fallo del
índice: es el techo de un modelo de 107 M sin reordenador, y queda dicho aquí en lugar de esconderse en la media.

Los números están publicados en [`resultados/`](resultados), con la suite que los produjo y el equipo donde se
midieron.

### Latencia, memoria y tasa de fallo por proveedor

La suite generativa registra cuántos intentos, **cuántos fallaron y con qué código**,
latencia mediana y peor, y memoria residente máxima. Un fallo se anota y la suite sigue, porque parar en el
primero deja sin saber cuántas de las demás iban bien.

Con Qwen2.5 1.5B en la edición rápida:
**24 intentos y ningún fallo**, latencia mediana de **80 s** por función y peor de **375 s** -la autoevaluación sobre
el texto largo-, y **2,1 GB** de memoria residente como máximo. Publicado en
[`resultados/generativa-2026-09-22.json`](resultados/generativa-2026-09-22.json), salida por salida.

### Lo que dice la suite generativa, sin redondear

| Función | Superan la rúbrica | Lo que falla |
|---|---|---|
| **Tarjetas** -el criterio- | **5 de 6** | `narrativo-largo-en.txt`: 7 de 9 citadas, 0,78 frente a 0,80. Dos citas apuntaban al fragmento de al lado y el runtime las quitó, que es lo que debe hacer |
| Resúmenes | 4 de 6 | Pocos puntos en el técnico; uno repetido en el largo |
| Conceptos | 4 de 6 | Sin citas en los dos narrativos: nombra personas que están en el texto sin decir dónde |
| Autoevaluación | 1 de 6 | Por eso se ofrece como no recomendada con el modelo local |

**Se intentó una segunda versión de la plantilla de tarjetas, y no se queda.** Pedía respuestas que se entendieran
solas, la cita del fragmento exacto y menos tarjetas antes que tarjetas mal citadas. Arregló el texto largo -10
tarjetas, todas citadas- y **rompió el de fórmulas**, que pasó de superar a un 50 % citado con una repetida. Las dos
versiones pasan cinco de seis textos, cada una un texto distinto: es el techo de un modelo de 1.500 millones de
parámetros, no algo que arregle el texto de la instrucción. Queda la versión 1, que es la que tiene una ejecución
completa publicada.

**Y se midió el modelo grande**, el que mejor escribe las tarjetas: Qwen3 4B **también supera cinco de seis**, pero al revés -pasa el narrativo largo con 9 de 10
citadas y **falla el de fórmulas**, con 3 de 4 citadas y 1 de 4 sostenida-, y tarda más del doble: tres minutos de
mediana por texto y nueve el peor. Ninguna de las tres combinaciones medidas -dos plantillas con el pequeño, una con
el grande- supera los seis textos, y cada una falla uno distinto.

**Los umbrales no se tocaron**, y es lo que hace que el resultado valga algo: se fijaron antes de medir ningún
modelo, y bajarlos ahora para que pasara convertiría la suite en una confirmación.

### Plantillas sin medir

La regla 1 de la sección 4 -toda modificación de plantilla ejecuta la suite- tiene **mecanismo**:
`scripts/check-evaluacion.mjs`, en `pnpm verify`. Cruza las plantillas con los resultados publicados y exige que
cada una esté medida **en su versión actual** o declarada aquí, con su versión y su motivo. Cambiar el
texto de una plantilla sube su versión -eso lo fija una prueba de `plantillas.rs`- y con ello invalida tanto el
resultado publicado como la declaración, así que el cambio no pasa sin volver a medir o sin escribir por qué no.

Lo que el guardián **no** hace es ejecutar la suite: con modelos de un gigabyte no cabe en `pnpm verify`.

Hoy **ninguna** está sin medir: las cuatro tienen resultados publicados en su versión actual. La tabla conserva su
encabezado porque el guardián la lee, y una plantilla nueva que no se pueda medir vendría aquí con su motivo.

| Plantilla | Versión | Por qué no está medida |
|---|---|---|

### Lo que marca el usuario

La rúbrica mide lo que se puede medir sin una persona. **Lo que solo ve quien conoce el libro** -que la respuesta
sea falsa aunque esté bien citada, que la pregunta no venga a cuento, que el resumen se deje lo importante- lo
recoge ahora la propia aplicación: al lado de quién generó hay un enlace discreto, "Marcar como Incorrecta", con
una lista cerrada de motivos y una nota opcional.

Tres decisiones, y las tres son criterio:

* **Lista cerrada y no un campo libre**, porque lo que hace falta es contar: veinte avisos del mismo motivo sobre
  la misma plantilla apuntan a una plantilla concreta, y veinte textos libres no apuntan a nada.
* **Lo que se anota es la plantilla, su versión, el proveedor y el modelo.** Sin eso, un aviso dice que algo salió
  mal y no con qué.
* **El texto del documento no entra, y no por disciplina: porque no hay dónde ponerlo.** La tabla de V36 no tiene
  columna para él, y una prueba recorre el esquema para que no aparezca una. Es la regla de esta iteración -"sin
  incluir el texto del documento salvo autorización explícita"- puesta donde no se puede olvidar.

**Local, y se dice.** Ajustes enseña lo marcado, deja quitarlo de uno en uno o entero, y guardarlo en un archivo
para mirarlo. **No hay botón de enviar**, y tampoco es un descuido: no existe un servicio del proyecto que los
reciba, y ofrecer un envío que no va a ninguna parte sería peor que no ofrecerlo. Cuando exista, el consentimiento
se pedirá como el de cualquier otra salida, por `net.rs`.

