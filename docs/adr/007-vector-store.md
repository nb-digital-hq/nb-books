# ADR-007: Almacén Vectorial para Búsqueda Semántica

**Fecha:** 2026-08-25. **Reevaluado el 2026-09-12**, que es lo que el propio ADR mandaba hacer al
llegar la búsqueda semántica.
**Decidido:** 2026-09-12, sobre la actualización de abajo y sin cambios sobre lo propuesto.
**Estado:** Aceptado

## Contexto

La búsqueda semántica requiere almacenar y consultar vectores de embeddings. La documentación previa mencionaba,
en distintos lugares y de forma inconsistente, dos extensiones distintas de SQLite para ese fin, sin registrar
una decisión.

Las extensiones de búsqueda vectorial para SQLite son un ecosistema joven y en movimiento. La opción más citada
sigue publicando versiones anteriores a la 1.0 y advierte expresamente de que las versiones menores pueden
introducir cambios incompatibles. Comprometer el formato de persistencia de los usuarios con una dependencia en
ese estado es un riesgo que debe declararse, no ignorarse.

## Alternativas Consideradas

1. **Extensión vectorial de SQLite.** Mantiene el principio de "un solo archivo portable", sin procesos ni
   servicios adicionales. Riesgo de inestabilidad de la dependencia.
2. **Base vectorial embebida independiente.** Mayor madurez en algunos casos, a costa de una segunda tecnología
   de persistencia, más peso en el instalador y ruptura de la portabilidad de archivo único.
3. **Índice vectorial propio en Rust, persistido en archivos.** Control total y sin dependencias externas; coste
   de implementación y mantenimiento significativo.
4. **Sin búsqueda vectorial en la primera versión.** La búsqueda léxica cubre la mayoría de los casos reales.

## Decisión

De la decisión original de 2026-08-25, que aplazaba el motor:

1. **Aplazar la decisión de motor concreto** y no introducir búsqueda vectorial.
2. Definir desde ahora un trait `VectorIndex` en Rust que aísle por completo al resto del sistema del motor
   elegido.
3. Tratar el índice vectorial como **caché regenerable**, nunca como fuente de verdad. Los fragmentos y sus
   localizadores viven en tablas propias del esquema principal; los vectores pueden reconstruirse por completo.
4. Reevaluar el motor cuando se aborde la fase de enriquecimiento con IA, con el criterio de estabilidad de la
   dependencia como factor principal.

## Consecuencias

* **Positivas:** el proyecto no queda atado a una dependencia inestable en su formato de persistencia; se puede
  cambiar de motor con una reindexación, sin migración de datos del usuario; el desarrollo se simplifica.
* **Negativas:** la búsqueda semántica se retrasa respecto a lo que sugería la comunicación del proyecto; la capa
  de abstracción añade indirección.
* **Riesgos:** reindexar una biblioteca grande es costoso en tiempo y energía. La operación debe ser incremental,
  pausable y transparente para el usuario.

---

## Actualización del 2026-09-12: la reevaluación que este ADR se dejó pendiente

El punto 4 de la decisión original decía que el motor se reevalúa "cuando se aborde la fase de enriquecimiento con
IA, con el criterio de estabilidad de la dependencia como factor principal". Es ahora. Y la reevaluación encontró
que **la pregunta importante no era cuál motor**, sino cuántos vectores. Los tres primeros números salen de medidas
propias, no de estimaciones: están en las secciones 4.5 y 4.7 de
[performance-budgets.md](../architecture/performance-budgets.md).

### Cuántos vectores son, con la biblioteca real

| Dato | Valor | De dónde |
|---|---|---|
| Documentos del corpus real | 152.080 | El corpus de pruebas, 162.319.958.712 bytes |
| Fragmentos por obra, con el tamaño | unos 223 | 893 por obra medidos con el fragmento anterior, dividido por cuatro al cuadruplicar su tamaño |
| **Fragmentos de la biblioteca entera** | **unos 34 millones** | Lo anterior por 152.080 |

Con un modelo pequeño de 384 dimensiones, eso ocupa:

> **Nota del 2026-09-16, sin cambiar la decisión.** Las cuentas de abajo siguen valiendo: el modelo
> elegido produce 384 dimensiones. Lo que estaba mal era una suposición que no aparece escrita aquí pero que se
> sacó de aquí: que "un modelo pequeño de 384 dimensiones" pesa unos 100 MB. Ese era `all-MiniLM-L6-v2`, **que
> solo sabe inglés**. Los multilingües de la misma dimensión pesan entre 214 y 470 MB; el elegido,
> `granite-embedding-107m-multilingual`, 214. Afecta a la descarga y a la memoria, no al índice. Ver
> [model-reviews.md](../legal/model-reviews.md).

| Representación | Por vector | La biblioteca entera |
|---|---|---|
| Coma flotante de 32 bits | 1.536 B | **52 GB** |
| Enteros de 8 bits | 384 B | **13 GB** |
| Binaria, un bit por dimensión | 48 B | **1,6 GB** |

Y el presupuesto de almacenamiento derivado, que es el 20 % del corpus y **frena al indexador**, son
**32,5 GB para todo**: texto, índice léxico, portadas y lo que venga. El texto y el índice ya se comen la mayor
parte.

A eso se suma el tiempo. Generar 34 millones de vectores en la CPU del usuario, a un ritmo realista de entre 50 y
200 fragmentos por segundo, son **entre dos y ocho días de máquina encendida**. La aplicación que promete
"se instala y funciona" no puede empezar por ahí.

### Lo que de verdad hay que decidir, entonces

**Primero el alcance, y después el motor.** La búsqueda semántica de toda la biblioteca, generada por defecto, no
cabe ni en el presupuesto de disco ni en el de paciencia. Lo que sí cabe:

1. **El documento abierto**, que son unos 223 vectores. Se generan al abrirlo o al pedirlo, en menos de lo que
   tarda una descarga, y se tiran cuando haga falta.
2. **Una colección que el usuario elija**, con su coste dicho antes de empezar -cuántos documentos, cuánto disco,
   cuánto tiempo- y con el mismo freno de presupuesto que ya tiene el indexador.
3. **Lo ya leído**, que es lo que de verdad se quiere volver a encontrar.

### Lo que se decidió el 2026-09-12

1. **Ningún motor vectorial de terceros en el núcleo.** Los vectores se guardan cuantizados a 8 bits en una
   columna de la base que ya existe, junto al fragmento y su localizador. Siguen siendo **caché regenerable**,
   como ya dijo este ADR.
2. **La consulta es híbrida y por fuerza bruta sobre candidatos**, que es lo que el diseño ya planeaba: el índice
   FTS5 acota, y sobre ese conjunto -o sobre el documento abierto- se calcula la distancia. Con 223 vectores por
   obra, o con unos pocos miles de candidatos, la fuerza bruta es más rápida que construir y mantener un índice
   aproximado.
3. **El `trait VectorIndex` de la decisión original se mantiene**, y con él la puerta abierta: el día que una
   medida enseñe que la fuerza bruta no llega, entra un motor detrás de ese contrato sin tocar nada más.
4. **El alcance por defecto es el documento abierto.** Todo lo demás lo pide el usuario, con el coste delante.

**Por qué no `sqlite-vec` ni un motor embebido ahora.** El miedo original -atar el formato de persistencia del
usuario a una dependencia anterior a la 1.0- está en buena parte neutralizado por tratar los vectores como caché:
si el formato rompe, se regenera. Lo que no se neutraliza es lo otro: una dependencia más en el instalador y un
segundo sistema de persistencia para un problema que, con el alcance de arriba, se resuelve con un producto
escalar sobre unos pocos miles de vectores. Se adopta cuando haya una medida que lo pida, no antes.

**Lo que se acepta a cambio.** No habrá "busca en toda mi biblioteca por significado" de fábrica. Con 152.080
documentos, eso hoy es una promesa que cuesta días de CPU y no cabe en el disco que el propio producto se puso
como límite. Decirlo es mejor que enseñar una barra de progreso de dos días.

## Construido

Sin cambiar la decisión, y con tres cosas que conviene dejar dichas.

**Una tabla y no una columna.** Donde arriba dice "en una columna de la base que ya existe, junto al fragmento", lo
que importa se cumple —la misma base, ocho bits, atado al fragmento— y la forma es la tabla `chunk_vectors` (V33),
con clave ajena al fragmento y borrado en cascada. Tres motivos: casi ningún fragmento tiene vector, así que una
columna no diría de qué modelo es cada uno; vaciar el índice es borrar una tabla; y un vector no puede sobrevivir a
su fragmento, que es lo que la cascada garantiza sin que el código tenga que acordarse cuando se reindexa.

**Lo que ocupa, medido: 620 bytes por vector**, índice incluido, de los que 384 son el vector. Un documento de unos
223 fragmentos son **unos 138 KB**. Cuantizar a ocho bits mueve la similitud menos de una centésima, y con los
vectores reales del modelo no cambia el orden de ningún par.

**El contrato se probó con dos motores.** `VectorIndex` tiene una implementación, `EnLaBase`; las pruebas del
contrato pasan también con un índice en memoria escrito en la prueba, que no comparte nada con ella, y regenerado en
él encuentra lo mismo. Es lo que sostiene "cambiar de motor solo requiere reindexar" sin tener que creérselo. Ver
`tests/indice_vectorial.rs`.

