# Almacenamiento

> **Estado:** Implementado. Base y transacciones en `src-tauri/src/db/`; biblioteca, raíces y disponibilidad en
> `src-tauri/src/library.rs`; repaso de carpetas en `src-tauri/src/repaso.rs`; importación de Calibre en
> `src-tauri/src/calibre.rs`; rutas y lectura sin hora de acceso en `src-tauri/src/paths.rs`.

Toda la fuente de verdad de NB Books vive en el disco del usuario: una base SQLite con el catálogo y lo que la
persona produce, y sus propias carpetas con los documentos.

## 1. Base de datos: SQLite

SQLite es embebido, no necesita servicios, es rápido para la entrada y salida de metadatos y garantiza integridad
ACID ante cortes de energía. Guarda metadatos, relaciones, colecciones, anotaciones, notas, progreso, sesiones de
lectura, preferencias, consentimientos de red y el índice de búsqueda. Los documentos **no** se guardan dentro: se
quedan en el sistema de archivos.

Configuración por conexión: `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON` y `busy_timeout`.

### 1.1 Toda transacción que escribe se abre con `BEGIN IMMEDIATE`

En WAL hay un solo escritor y `busy_timeout` hace esperar a quien llega segundo, pero eso solo vale para quien aún
no ha leído nada. Una transacción diferida (`BEGIN DEFERRED`, lo que abre `Connection::transaction` por omisión)
que lee y después escribe tiene que ascender su bloqueo, y si entre medias otro escritor confirmó algo SQLite
devuelve `SQLITE_BUSY` **de inmediato y sin respetar el tiempo de espera**.

Sin esta regla, una importación real con la indexación y los metadatos corriendo a la vez produjo 12.156
"database is locked" en una sesión, el 91 % de los títulos se quedaron con el nombre del archivo y otra pasada
perdió 1.207 documentos. Con ella, cero bloqueos y una peor lectura de la interfaz de 1,6 ms.

* **Mecanismo:** `db::write_transaction`, y `scripts/check-transacciones.mjs` en `pnpm verify` y en el gancho de
  pre-commit rechaza `BEGIN`, `BEGIN TRANSACTION`, `BEGIN DEFERRED` y `.transaction`.
* **Reintento acotado:** un fallo de almacenamiento durante una importación masiva casi nunca es del documento, así
  que se reintenta hasta tres veces con una espera corta antes de contarlo como rechazo. Los demás códigos no se
  reintentan: un EPUB roto seguirá roto.
* **Prueba de contención real:** `tests/contencion_de_escritura.rs` usa dos escritores sobre una base **en
  archivo** (en memoria no hay WAL y la prueba pasaría siempre).
* **Lecturas coherentes:** varias consultas que tienen que casar entre sí van dentro de `db::read_snapshot`, una
  transacción de solo lectura que fija su foto y en WAL no bloquea a ningún escritor.

### 1.2 Las transacciones de escritura son cortas

Con el indexador trabajando, una transacción larga puede no conseguir nunca el bloqueo: entre dos intentos suyos
siempre entra otro escritor. Por eso las pasadas de reparación escriben en autocommit, fila a fila; ante un bloqueo
**se aplazan enteras** en lugar de insistir; y una pasada que no termina no pone su marca y se reintenta en el
siguiente arranque.

### 1.3 El archivo devuelve su hueco

SQLite no encoge el archivo al borrar. Cuando a la base le sobra **más de una cuarta parte y al menos 64 MB**, el
arranque diferido la compacta con `VACUUM` antes de arrancar el indexador, que es quien mide el presupuesto de lo
derivado sobre el tamaño del archivo. Por debajo de eso no compensa reescribir la base.

### 1.4 El respaldo de una migración

Antes de migrar se escribe un respaldo con la versión de origen en el nombre:

* Se escribe aparte (`.parcial`), se renombra al terminar y solo entonces se retiran los anteriores: nunca hay cero
  respaldos ni uno a medias, y se conserva solo el último.
* **No es una copia del archivo.** Lleva el esquema entero y todas las filas **salvo el índice del texto**
  (`documents_fts`, `text_chunks`, `chunk_vectors`, `index_probes` y `search_status`), que son exactamente lo que
  vacía `search::reset`. Sobre una base de 23,9 GB el respaldo ocupa 556 MB y tarda 24 s. Restaurarlo cuesta
  reindexar. Una tabla nueva entra en el respaldo sin que haya que acordarse.
* Se lee con una conexión propia, que ve también lo que está en el `-wal`.
* Los respaldos cuentan en la medida de lo derivado.
* **Se retira solo** cuando el esquema migrado ya se dio por bueno: tras tres sesiones que terminan bien sobre él
  (`migrations::SESIONES_PARA_RETIRAR_RESPALDO`). Lo que el respaldo protege es que la migración haya dejado mal
  la base, y eso se nota en las primeras sesiones; después solo ocuparía disco. Una sesión que no cerró bien no
  cuenta.

### 1.5 Conexiones

El pool tiene entre seis y doce conexiones según los núcleos del equipo. En WAL las lecturas no se bloquean entre sí
y hay un solo escritor, así que más conexiones no añaden contención de escritura; con un número fijo pequeño, las
tareas de fondo (índice, metadatos, vectores y audio) bastaban para que la interfaz esperase. Una espera de más de
50 ms queda en el registro con cuántas estaban en uso.

## 2. Documentos en el sistema de archivos

| Modo | Qué hace | Para quién |
|---|---|---|
| **Referenciado** | El archivo se queda donde estaba; se guarda su ubicación y su hash. Es el modo de las importaciones de carpeta | Bibliotecas existentes, discos externos, carpetas compartidas |
| **Administrado** | La aplicación copia el archivo a su carpeta de datos | Quien solo quiere arrastrar un libro y leer |

**Disponibilidad.** `refresh_availability` distingue `available`, `missing` (la ruta no existe, típicamente un
volumen desconectado) y `unreadable` (existe pero no se deja abrir). Un archivo que falta **no sale del catálogo**:
su documento sigue visible con metadatos y anotaciones, y vuelve a estar disponible al reconectar el volumen. Un
archivo administrado que falta se cuenta aparte, porque de esa copia responde la aplicación.

**Baja de un documento.** Se retira el registro y todo lo que colgaba de él en una transacción, y solo después se
toca el disco. Qué pasa con el archivo es un parámetro obligatorio, y **un archivo referenciado no se borra
nunca**, aunque se pida.

### 2.1 Raíces de biblioteca: la ubicación no es una ruta absoluta

Una ruta absoluta es la dirección de un equipo concreto: al restaurar en otro equipo, al reinstalar con otro nombre
de usuario o al montar el disco en otro punto, todos los archivos referenciados quedarían no disponibles a la vez.

| Parte | Qué es | Ejemplo |
|---|---|---|
| **Raíz de biblioteca** | Un lugar declarado por el usuario, con identificador y nombre | `Disco externo` |
| **Ruta relativa** | La dirección del archivo dentro de la raíz | `Filosofía/Spinoza/Etica.epub` |

1. **Toda ubicación es relativa a una raíz.** La ruta absoluta de la raíz vive solo en la tabla de raíces.
2. **Una importación de carpeta declara una raíz**: la carpeta elegida, no una por subcarpeta.
3. **La biblioteca administrada es una raíz más**, no un caso especial.
4. **La exportación lleva raíz y ruta relativa, nunca la absoluta.** Al restaurar en otro equipo, una raíz que no
   existe queda **no localizada** y se vuelve a apuntar **una vez**; con eso reaparecen todos sus documentos.
5. **Reapuntar una raíz no reimporta nada**; el hash confirma que el contenido es el que era.
6. **Una raíz no localizada no impide arrancar.**

No se busca el archivo por su hash recorriendo el disco: exigiría recorrer sistemas de archivos sin permiso
explícito y en un tiempo indeterminado.

### 2.2 Lo que cambia en el disco se ve sin pedirlo

Añadir o quitar un archivo en una raíz referenciada se refleja sin pedir un reescaneo. Se eligió un **repaso
incremental** frente a un vigilante de `inotify`, medido sobre 18.841 carpetas:

| | Vigilante (`inotify`) | Repaso incremental |
|---|---|---|
| Memoria en reposo | 2.004 kB del núcleo y los nodos de índice clavados en caché | Cero: la foto vive en la caché del disco |
| Coste por pasada | Ninguno: avisa el núcleo | 61–124 ms de procesador |
| Lo que cambió con la aplicación cerrada | No lo ve | Lo ve |

El vigilante no ve lo que pasó con la aplicación cerrada, así que el repaso hacía falta igualmente; además
`inotify` tiene un tope de marcas por usuario y llevarlo a otros sistemas exigía una dependencia nueva.

* **Cómo funciona:** añadir o quitar un archivo cambia la fecha de su carpeta. Solo se listan las carpetas cuya
  fecha cambió, se importa lo que no está en la base por la misma puerta que una importación, y lo que ya no está
  queda como no disponible.
* **Cuándo:** al terminar el arranque diferido, al volver a la ventana y al entrar en Biblioteca o Inicio, con tres
  segundos como mínimo entre pasadas y nunca durante una importación.
* **La primera vez solo mira:** sin foto guardada se toma la foto y no se importa nada, para no convertir en una
  importación masiva callada lo que alguien dejó fuera a propósito.
* **Una raíz que no está no se toca.**
* **La foto no va en la base**: es derivada y se regenera sola.

### 2.3 Lo duplicado se dice al importar

La identidad por contenido distingue el mismo archivo visto otra vez, el mismo archivo movido (se reubica) y **una
copia exacta en otra ruta**. Ninguno crea una entrada nueva, y la copia exacta es su propio resultado
(`ImportOutcome::Duplicate`): se cuenta aparte en el avance, en el informe y en la ejecución guardada, con de qué
es copia (hasta mil filas por ejecución, con el recuento siempre entero; migración V37). El panel de importar la
enseña con el título de lo que ya estaba.

No se registra la copia como segunda ubicación, y no se agrupa como duplicado otra edición u otro formato del mismo
libro: eso no es un duplicado exacto.

#### El filtro "Duplicados"

Enseña lo que la gente llama duplicado: ediciones con **el mismo título y el mismo autor**, sin distinguir
mayúsculas ni tildes. Solo el título no basta (en una biblioteca real hay 7.538 títulos repetidos con 21.094
ediciones; con el autor quedan 14 ediciones en 7 grupos). Lo hacen posible V38 (un autor, una fila) y V39, que
guarda el título comparable en `editions.title_key`. El filtro agrupa por ese título y la cuenta dice qué cuenta.
El recuento cuesta unos 100 ms y se pide en segundo plano.

### 2.4 Importar una biblioteca de Calibre

Conserva títulos corregidos, autores con su orden de clasificación, series y numeración, etiquetas e
identificadores (`isbn`, `doi`, `asin`).

1. **Siempre referenciada**: la carpeta de Calibre se registra como raíz referenciada.
2. **Solo lectura estricta**: `metadata.db` se abre con `SQLITE_OPEN_READ_ONLY`, `PRAGMA query_only = ON` y
   `temp_store = MEMORY`. Nada se escribe en la carpeta de Calibre; las pruebas lo comprueban con permisos `0o555`
   y `mtime` inalterado.
3. **Prevalidación**: `validar_biblioteca` comprueba la carpeta y las tablas antes de tocar el catálogo.
4. **Idempotencia**: volver a importar no duplica nada.
5. **Portadas en la caché**: se genera la miniatura; el original no se toca ni se duplica.
6. **Procedencia `calibre`**, que gana sobre lo extraído del archivo y en línea y pierde frente a lo editado a mano
   ([metadata-model.md](metadata-model.md)).

### 2.5 Leer sin tocar la hora de acceso

Toda lectura de archivos de la biblioteca (hash, detección de formato, disponibilidad, metadatos, portadas,
indexación y lectura) pasa por `paths::open_noatime` y `paths::read_bytes_noatime`.

* **Linux:** se abre con `O_NOATIME`, y el núcleo no actualiza `st_atime`. Si el archivo es de otro usuario o el
  sistema de archivos no admite la bandera, se reintenta con una apertura normal, para que ningún libro quede
  inaccesible.
* **macOS y Windows:** no hay bandera equivalente por descriptor; los sistemas restringen por omisión la
  actualización de la hora de acceso.

## 3. Caché y datos temporales

Portadas, páginas rendidas de PDF y cómic y demás artefactos regenerables viven en el directorio de caché. Si se
borra, se regeneran.

Todo archivo de caché se escribe de forma **atómica**: a un temporal en el mismo directorio que se renombra al
terminar (`paths::write_atomic`). Sin eso, quien comprueba si una portada existe podría encontrarla a medio
escribir.
