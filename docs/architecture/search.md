# Búsqueda

> **Estado:** Implementado. Índices en `V8__search.sql`, `V10__catalog_search.sql`, `V25__estado_de_busqueda.sql` y
> `V27__indice_sin_texto.sql`; motor en `src-tauri/src/search.rs`; estado que ve el usuario en
> `src/lib/core/search.ts`. La capa semántica está en [ai/embeddings.md](ai/embeddings.md).

La búsqueda convierte una biblioteca en conocimiento accesible. Este documento define su diseño léxico, su ciclo
de vida y cómo se combina con la búsqueda por significado.

## 1. Alcances

| Alcance | Qué busca | Devuelve |
|---|---|---|
| Catálogo | Título, autor, materias | **Documentos** |
| Documento abierto | El texto del documento actual | **Pasajes** |
| Toda la biblioteca | El texto de todos los documentos indexados | Pasajes, o una coincidencia representativa por edición en índices muy grandes (sección 2.5) |

Los alcances **no** son un filtro sobre un mismo resultado: son consultas distintas contra índices distintos, y el
resultado viaja etiquetado.

### 1.1 Dos índices, no uno

El catálogo tiene su propio índice, `catalog_fts`, separado del texto de los libros: si el título estuviera en el
mismo índice, buscar "Cien años de soledad" devolvería doscientos párrafos que mencionan la soledad antes que el
libro. Lo mantienen siete disparadores que reconstruyen la fila entera de una edición cuando cambia el título, un
autor o una materia; así un autor que llega del servicio de metadatos después de importar también se encuentra.

### 1.2 Acotar por colección se hace en SQL

El identificador de la colección viaja en el alcance y el cruce lo hace la base: traer una colección de veinte mil
libros al webview en cada pulsación es lo que prohíbe [performance-budgets.md](performance-budgets.md). Se acota por
colecciones manuales; las colecciones por regla no guardan miembros y no se usan como alcance.

## 2. Motor léxico: FTS5

* La tabla del texto es **sin contenido** (`content = ''`): guarda el índice invertido y nada más. Guardar el texto
  costaría el 52,1 % de lo que ocupan los documentos, y el presupuesto de lo derivado es el 20 %.
* `contentless_delete = 1` permite retirar filas por identificador: reindexar un documento no deja fragmentos viejos.
* Cada fila guarda el **Locator** de su fragmento, para que un resultado abra la posición exacta.
* Tokenizador con eliminación de diacríticos: "analisis" encuentra "análisis".
* Fragmentos de 1.920 caracteres (ver [performance-budgets.md](performance-budgets.md) sección 2.1).

### 2.1 Quién trocea el documento

**El proveedor de formato**, no la búsqueda: solo él sabe construir un Locator en su formato. En un PDF todos los
fragmentos de una página comparten el Locator de esa página, porque ahí la unidad de posición es la página. El
corte entre fragmentos retrocede hasta un espacio, para no partir una palabra y dejarla fuera del índice.

### 2.2 Lo que se teclea no es una consulta de FTS5

FTS5 tiene su propio lenguaje (comillas, `AND`, `OR`, `NEAR`, asteriscos). La entrada se convierte en términos
entrecomillados: se buscan **palabras**, y una comilla suelta o un `OR` escrito no cambian la consulta. Solo el
último término lleva prefijo, y solo desde tres caracteres.

### 2.3 De dónde sale el pasaje, y por qué sus marcas no son marcado

El pasaje **se relee del documento** por el Locator del fragmento, por el mismo camino que usa el lector. Un volumen
desconectado no tiene pasaje, y se dice.

El resaltado lo pone la base sobre el pasaje releído con **el mismo tokenizador que indexó**, así que buscar
"senal" resalta "señal". La coincidencia se envuelve entre comillas angulares simples, no en etiquetas HTML: el texto
lo escribió un desconocido, y devolver `<mark>` obligaría a interpretarlo como marcado.

### 2.4 Una búsqueda a la vez, y cuándo se pregunta

Sobre una biblioteca de 152.080 libros, una consulta en el texto tarda de 450 ms a 1,7 s. Por eso:

1. **El runtime corta la búsqueda anterior** con `sqlite3_interrupt` sobre su conexión (`search::BusquedaEnCurso`).
   La cortada responde `CANCELLED`, que la interfaz descarta. La interrupción no puede alcanzar una conexión ya
   devuelta al pool: el turno se retira bajo el mismo cerrojo con el que se interrumpe.
2. **Cada alcance tiene su pausa y su mínimo de letras**: 300 ms y dos letras en el catálogo; 550 ms y tres letras
   en el texto. Con menos letras no se pregunta y se dice cuántas faltan.
3. **La respuesta guarda la pregunta que contesta.** Lo que contestó otro alcance no se pinta; lo que contestó una
   consulta anterior del mismo alcance se muestra atenuado mientras llega la nueva, sin parpadeo. Mientras haya una
   pregunta sin contestar se indica que se está buscando.

### 2.5 Consultas sobre un índice grande

* **Dentro de un documento**, la consulta acota el intervalo de identificadores FTS de esa edición y comprueba la
  edición de cada fragmento. Devuelve pasajes en orden del documento, sin calcular `bm25` sobre todo el índice.
* **En toda la biblioteca**, hasta 300.000 como identificador máximo de fragmento se clasifica por `bm25`. Por
  encima se devuelve una coincidencia representativa por **edición**, en orden de índice; el recorrido salta los
  tramos de una edición ya encontrada y se detiene tras 400.000 coincidencias FTS. Esa cota limita el trabajo y
  puede recortar consultas muy frecuentes: no se promete un recorrido exhaustivo.
* Sobre ese índice grande, el último término con menos de cinco letras busca la palabra exacta; con cinco o más
  conserva el prefijo. El catálogo no cambia.
* Los nombres de capítulo de los resultados salen de la navegación del documento cuando el recurso tiene un título
  inequívoco.

## 3. Ciclo de vida del índice

* La indexación ocurre en segundo plano tras importar, es incremental y reanudable.
* Se detiene con batería por debajo del 20 %, comprobándolo **antes de cada documento** (`src-tauri/src/power.rs`),
  al alcanzar el presupuesto de disco y mientras entra una importación.
* Reindexar **sustituye**: los fragmentos de esa edición se borran antes de escribir los nuevos.
* El índice es **caché**: se reconstruye desde los documentos, y perderlo no es perder datos.
* Eliminar un documento elimina sus filas del índice en la misma transacción.
* Ajustes ofrece reconstruir el índice a petición.

### 3.1 Reanudable sin cursor

La cola vive en memoria. Al arrancar se vuelven a encolar las ediciones pendientes según `search_status`, que se
escribe en la misma transacción que los fragmentos. Un cursor persistido sería un dato más que mantener coherente.

### 3.1.1 En qué orden

Si no cabe toda la biblioteca, lo que cabe es lo que se lee: lo empezado, lo que está en una colección, lo de los
autores que se leen y el resto de lo más reciente a lo más antiguo. **Lo que se abre pasa delante y no espera al
tope de disco.**

### 3.2 Omitir no es fallar

| Resultado | Cuándo | Qué significa |
|---|---|---|
| Indexado | El documento tenía texto y se guardó | Ya es buscable |
| Omitido | No hay archivo disponible, no hay proveedor o no hay texto | No hay nada que hacer |
| Fallido | El documento existe y no se dejó indexar | Algo va mal con ese archivo |

Un archivo en un disco desenchufado se **omite**: contarlo como fallo diría que el documento está roto.

### 3.3 Reparación

Un índice corrupto **no impide arrancar**. El índice del texto no guarda contenido, así que repararlo es volver a
indexar; el del catálogo se rehace solo desde sus tablas.

**Se comprueba cuando hay motivo, no en cada arranque.** Un índice invertido se corrompe cuando el proceso muere a
media escritura. La aplicación marca `cierre_limpio` en falso al arrancar y en cierto al salir bien, y **solo
comprueba el índice si la sesión anterior no terminó como debía**. `bootstrap` lee cómo terminó la anterior antes
de marcar la nueva, y `tests/cierre_de_sesion.rs` arranca la aplicación tres veces (perfil nuevo, tras salir bien,
tras morir) y comprueba lo que ve cada una. Una base sin marca y sin índice es un perfil nuevo y no se comprueba.

La corrupción sin cierre sucio (un sector que falla con la aplicación cerrada) la detecta la propia búsqueda, que
recibe el error de SQLite, y se resuelve con la reconstrucción a petición.

### 3.4 Cómo se comprueba un índice que no guarda su contenido

Sobre una tabla sin contenido, `integrity-check` solo mira que las estructuras cuadren consigo mismas: con los
bloques del índice puestos a cero responde que todo está bien y las búsquedas devuelven cero filas sin error. Por
eso el índice del texto se comprueba con **sondas**:

* Al indexar cada edición se guarda en `index_probes` una palabra suya y el fragmento donde tiene que aparecer.
  Comprobar el índice es pedirle dieciséis sondas repartidas por el rango de identificadores; si no las devuelve, el
  índice ya no contiene lo que se le metió. Cuesta unos veinte bytes por edición.
* **La sonda sustituye a `integrity-check` sobre el índice del texto**: el recorrido completo costaría cuatro
  minutos con el bloqueo de escritura retenido sobre una base de 9,9 GB, sin ver la pérdida que importa. Sobre
  `catalog_fts`, que sí tiene contenido con el que compararse, se conserva (2 s).
* **Una sonda se comprueba antes de guardarse**, para que una que no responde solo pueda significar índice dañado y
  nunca palabra mal elegida.
* Las sondas detectan que el índice se ha ido, no que le falte una palabra concreta.

## 4. Documentos no indexables

Un documento sin texto se marca como no buscable **con su motivo**, y se le dice al usuario. La detección ocurre
justo después de importar, fuera de la transacción de importación.

### 4.1 Cuatro motivos

| Motivo | Qué significa |
|---|---|
| `IMAGE_ONLY` | Un cómic: no tiene texto |
| `NO_TEXT_LAYER` | Un PDF escaneado sin capa de texto |
| `FILE_UNAVAILABLE` | El volumen está desconectado; se arregla solo |
| `NO_ENGINE` | Esta instalación no puede abrir ese formato |

El motivo se deduce del formato del documento, no del error, porque `FORMAT_UNSUPPORTED` dice lo mismo para un cómic
que para un escaneo. Presentar como buscable algo que no lo es destruye la confianza en la búsqueda entera.

### 4.2 Pendiente no es lo mismo que no buscable

Una edición de la que aún no se sabe nada está **pendiente**, y así se dice mientras la biblioteca se indexa. La fila
de `search_status` la pone **un disparador de la base** (V25) al crear la edición, venga de donde venga: importar
un archivo, descargar de un catálogo o cualquier otro camino. El invariante `count(editions) =
count(search_status)` lo fija una prueba. Un `CHECK` impide un "no buscable" sin motivo.

## 5. Búsqueda híbrida

Cuando el modelo de embeddings está instalado, los resultados léxicos y por significado se combinan:

* La búsqueda léxica es el comando `search` y **nunca depende de la IA**. La de significado es otro comando,
  `semantic_search`, que la interfaz pide **después** y con una pausa de 700 ms.
* Lo léxico se pinta al llegar. Cuando contesta lo de significado, las dos listas se funden por rangos recíprocos
  (`fusion.ts`), porque un rango de FTS5 y un coseno no están en la misma escala.
* Cada resultado dice si es **exacto**, **por significado** o **las dos cosas**, y lo que sale en las dos se muestra
  una vez.
* Si lo de significado falla o no hay modelo, la lista léxica se queda como estaba. Sin nada preparado en el
  alcance, el runtime contesta vacío sin calcular el vector.

`tests/busqueda_hibrida.rs` comprueba que la léxica devuelve exactamente lo mismo con la IA sana, sin modelo,
fallando y en pánico.

## 6. Privacidad

Las consultas revelan intereses e intenciones: no se envían a ningún servidor y no se escriben en los registros.
