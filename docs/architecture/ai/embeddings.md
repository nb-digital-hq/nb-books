# Embeddings y Búsqueda Semántica

> **Estado:** Implementado. Quedan fuera los escaneados sin capa de texto, que exigirían OCR, rechazado en el núcleo.

La búsqueda por palabra clave (FTS5) es rápida y determinista, pero no encuentra conceptos. La búsqueda semántica
la complementa; no la reemplaza.

## 1. Pipeline de indexación

1. **Extracción:** al importar un documento, el texto se extrae en segundo plano (PDFium para PDF, parser de EPUB
   para XHTML). Los documentos escaneados sin capa de texto requieren OCR y quedan fuera del alcance inicial.
2. **Segmentación (chunking):** el texto se divide en fragmentos de hasta 1.920
   caracteres cortando, por este orden, **al final de un párrafo, de una frase o en un espacio**, siempre en la
   segunda mitad del tamaño. **Sin solapamiento**, y es una decisión que cambia lo que aquí se preveía:
   el fragmento es el mismo para el índice léxico y el vectorial —lo que deja a la búsqueda híbrida acotar con uno
   y ordenar con el otro sin traducir entre dos troceados—, y en el léxico el solapamiento duplicaría cada
   coincidencia de la zona compartida. Cortar por párrafo es lo que el solapamiento buscaba: que una idea no quede
   partida. Ver `reader::segmentos` y la sección 5.
3. **Anclaje:** cada fragmento guarda un localizador estable hacia su posición en el documento, de modo que un
   resultado semántico pueda abrir la página o el fragmento exacto. Ver
   [reader-contracts.md](../reader-contracts.md).
4. **Embedding:** se genera un vector con un modelo pequeño ejecutado localmente.
5. **Persistencia:** los vectores se guardan asociados al documento y al localizador.

## 2. Almacenamiento vectorial

**Según [ADR-007](../../adr/007-vector-store.md): ningún motor vectorial de terceros.** Los vectores se guardan cuantizados a 8 bits en una tabla de la base, `chunk_vectors`, atada al
fragmento y a su localizador, y la consulta es por **fuerza bruta sobre los candidatos que acota FTS5**. Se preserva
la portabilidad de "un solo archivo" sin añadir una dependencia ni un segundo sistema de persistencia.

**La pregunta importante no es cuál motor, sino el alcance.** Con 152.080 documentos serían unos 34 millones de
fragmentos, 13 GB cuantizados y entre dos y ocho días de CPU. Con el alcance que se ofrece -el documento abierto o
una colección- un producto escalar es más rápido que construir y mantener un índice aproximado. Además:

* El acceso al índice vectorial se aísla detrás de un trait `VectorIndex` en Rust (`ai::indice`). Es la puerta por la que entraría otro motor el día que una medida enseñe que la fuerza bruta no llega,
  y está probada con dos: el de la base y uno en memoria que solo existe en las pruebas.
* **Los vectores van en `chunk_vectors`** (V33): ocho bits por dimensión con una escala por vector, el modelo que
  los calculó y una clave ajena al fragmento con borrado en cascada, así que reindexar una edición se lleva sus
  vectores. **620 bytes por vector**, medidos; unos 138 KB por documento.
* El índice vectorial es **regenerable**: se trata como caché, no como fuente de verdad. Si hay que migrar de
  motor, se reindexa.
* La decisión de motor concreto se registra y revisa en [ADR-007](../../adr/007-vector-store.md).

## 3. Coste e higiene de recursos

Indexar una biblioteca grande es una operación costosa. Reglas:

* La indexación es incremental, pausable y se detiene con batería baja o bajo presión térmica:
  `AiService::pausa_de_fondo` dice si el trabajo de fondo del proveedor
  local tiene que esperar, y por qué —`AI_SUSPENDED_BATTERY` o `AI_SUSPENDED_HEAT`—, y se pregunta antes de cada
  lote, igual que el indexador léxico pregunta por la batería antes de cada documento. **Lo interactivo no se
  suspende**: el vector de una consulta son décimas de segundo que alguien acaba de pedir, y negárselo a quien
  tiene el 19 % de batería le quitaría una función a cambio de nada que se note. Ver
  [local-models.md](local-models.md), sección 5.
* El usuario ve el progreso y puede excluir documentos o colecciones del índice.
* Se declara el espacio en disco estimado antes de indexar.
* Los vectores se eliminan cuando se elimina el documento.

La generación de vectores (`ai::generacion`) sigue tres decisiones de producto:

* **Nada se prepara sin pedirlo.** Ni al abrir un documento ni al importarlo. La búsqueda del libro abierto y el
  menú de colecciones enseñan primero **cuántos fragmentos, cuánto disco y cuánto tiempo**, y dicen si el tiempo
  está medido en ese equipo o supuesto; el botón de preparar va después. Un recorrido de interfaz comprueba que no se pide empezar antes de pulsar.
* **Dos alcances: el documento abierto y una colección.** El primero no espera al tope de lo derivado, como el
  indexador léxico con lo que se acaba de abrir; una colección sí, y si no cabe no se ofrece.
* **La generación se aparta mientras el lector pinta.** El lector avisa al desplazar
  o pasar página —como mucho cada medio segundo, y solo con un trabajo en marcha— y no se empieza ningún lote
  hasta que lleva segundo y medio quieto.

Y lo demás de esta sección: **incremental** —la segunda vez solo se calcula lo que falta, y cancelar no pierde lo
calculado—, **pausable** por el usuario, **suspendida con batería baja o calor** por `pausa_de_fondo`, y **las
exclusiones** de un documento o de una colección, que además borran lo ya preparado. Un documento indexado con un troceado
anterior se reindexa al prepararlo, para que cada vector quede atado a su fragmento. Lo que
cuesta está medido en [performance-budgets.md](../performance-budgets.md), sección 6.

## 4. Búsqueda híbrida

La consulta combina resultados léxicos (FTS5) y semánticos, fusionando los rankings. La interfaz debe distinguir
visualmente el tipo de coincidencia: una coincidencia exacta y una aproximación semántica no merecen la misma
confianza del usuario.

La búsqueda por significado es `ai::busqueda`, aparte de la léxica y pedida después de
ella, y la fusión por rangos recíprocos está en la interfaz. Cómo, y por qué así, en
[architecture/search.md](../search.md), sección 5.

## 5. Los fragmentos, medidos

Sobre 40 libros reales del corpus, con el fragmento de 1.920 caracteres:

| | Corte por longitud | Corte por párrafo |
|---|---|---|
| Fragmentos | 9.783 | 10.510, **un 7,4 % más** |
| Que acaban en un final de frase | 18,0 % | **94,4 %** |

**El precio son las filas**: un 7,4 % más, cada una con su identificador y su locator (ese coste es el
7,7 % de la biblioteca, así que sube alrededor de medio punto). **Lo que se gana** es que un fragmento sea una
unidad de sentido y no un corte a ciegas: cortando por longitud, cuatro de cada cinco acabarían a mitad de una
frase.

**Los bloques del EPUB no cambian su texto.** Los párrafos se marcaban con un salto de línea que la normalización
convertía en espacio, y con él se perdería dónde acaba cada uno. Se marcan con un carácter que se normaliza
igual y se anota su posición. Como cada anotación y cada posición de lectura guardan desplazamientos sobre ese
texto, una prueba compara el texto normalizado con el de referencia: **1.196 capítulos de 40 libros reales, idénticos**.

**Y el modelo trunca a 512 tokens.** Medido con el tokenizador de `granite-embedding-107m-multilingual` sobre los
mismos fragmentos: mediana de **483 tokens**, p95 de 549, máximo de 607, y **un 25,2 % pasan de 512**. Lo que no
entra en el vector es la cola de esos fragmentos, como mucho 95 tokens; sigue en el índice léxico. No se baja el
tamaño del fragmento por esto, porque el fragmento es también el del índice léxico y su tamaño está decidido por lo
que ocupa.

**Todo fragmento abre su posición exacta**, que es el criterio: su locator resuelve exacto y el texto
que hay ahí es el fragmento. Lo comprueban `todo_fragmento_abre_su_posicion_exacta_y_no_parte_un_parrafo`, con
varios tamaños, y `trocear_por_parrafo_sobre_libros_reales`, sobre los 10.510 fragmentos reales. En PDF la unidad
de posición sigue siendo la página: los trozos de una página larga abren esa página.

