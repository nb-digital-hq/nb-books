# Modelo de Metadatos e Interoperabilidad

> **Estado:** Implementado. Obra, edición y archivo, contribuidores con rol, materias e identificadores externos
> en `src-tauri/migrations/V2__bibliographic_core.sql` y `src-tauri/src/db/catalog.rs`; procedencia y precedencia
> en `V3__metadata_provenance.sql`, `V40__calibre_provenance.sql` y `src-tauri/src/metadata.rs`; identidad por
> contenido en `src-tauri/src/library.rs`; proveedores en línea en `src-tauri/src/metadata_online.rs`.
> El glosario define el vocabulario del dominio. Este documento define cómo se representa ese vocabulario en
> datos y cómo se comunica con el mundo exterior.

## 0. Una fuente de metadatos aportada por una extensión

Una extensión puede añadir una fuente de metadatos en línea. **Propone; no escribe.**

Todo lo que devuelve pasa por `metadata::propose` con origen `Online`, igual que el proveedor del núcleo. Que la fuente sea de un tercero no le da más autoridad que a la nuestra: le da **la misma**. Una edición
manual gana siempre, y lo que el usuario ya rechazó no se vuelve a aplicar.

**Sale por la puerta de red del anfitrión**, con el permiso de esa extensión y no con el de otra. Sin permiso
concedido recibe `denied` y no llega ni al transporte, y su salida queda en el registro con su nombre.

**Cada candidato lleva su procedencia**, y la interfaz la enseña. Quien elige entre varias fichas tiene que poder
distinguir la del catálogo de siempre de la de una extensión que instaló la semana pasada; sin eso, la decisión
se toma a ciegas y el candidato de un tercero se ve exactamente igual que el nuestro.

**Lo que no puede proponer:** portadas. Descargar una imagen es un segundo salto a la red y una segunda decisión
del usuario; las portadas vienen del proveedor del núcleo.


## 1. Por qué no basta con "título y autor"

Título, autor, formato, ruta y progreso bastan para una biblioteca personal pequeña. Para uso académico,
bibliografías y exportación de citas no bastan, y ampliar un esquema más tarde es una migración costosa. Por eso el
esquema bibliográfico completo existe desde la migración V2: identificadores persistentes (ISBN, DOI, ISSN),
múltiples contribuidores con rol (autor, editor, traductor, ilustrador) y con su orden, idioma, editorial, fecha
de publicación con la precisión que se conozca, mención de edición, serie y materias.

## 2. Alineación con estándares

El proyecto no inventa un esquema propio. Se alinea con estándares establecidos, lo que garantiza longevidad e
interoperabilidad:

| Necesidad | Estándar de referencia |
|---|---|
| Metadatos descriptivos básicos | Dublin Core Terms |
| Metadatos de publicación comercial | ONIX for Books (mapeo de entrada) |
| Metadatos internos de la publicación | El paquete OPF de EPUB, el diccionario de información de PDF y `ComicInfo.xml` en CBZ |
| Exposición de catálogo | OPDS |
| Citas y bibliografía | CSL JSON y BibTeX para exportación |
| Identificadores | ISBN, ISSN, DOI, ORCID para autores |

El modelo interno se define como un superconjunto pragmático de Dublin Core, con mapeos de entrada y salida
documentados. Los mapeos entre esquemas siempre pierden información; esa pérdida debe estar documentada, no ser
una sorpresa.

### 2.1 El papel de quien participa se lee, no se supone

`dc:creator` y `dc:contributor` **no son lo mismo**. Tratarlos igual enlazaría a todos como **autores**, y sobre
un corpus real llenaría el panel de autores de herramientas, porque Calibre estampa
`<dc:contributor opf:role="bkp">calibre (x.y.z)</dc:contributor>` en cada EPUB que produce y sus versiones
acabarían entre los autores con más obras de la biblioteca, por delante de cualquier persona.

Lo que se hace ahora:

| Qué trae el archivo | Qué se guarda |
|---|---|
| `dc:creator` sin `opf:role` | Autor. Es lo que significa "creator" en Dublin Core, y casi ningún EPUB declara el papel |
| `dc:creator` o `dc:contributor` con `opf:role` de los cuatro que el catálogo guarda -`aut`, `edt`, `trl`, `ill`- | Ese papel |
| `dc:contributor` sin `opf:role` | **Nada.** No dice qué hizo, y el catálogo solo sabe guardar cuatro papeles: inventarle uno sería escribir en la biblioteca del usuario algo que su archivo no dice |
| Cualquier otro código de relator, `bkp` incluido | **Nada.** Un productor de libro no es alguien que participara en la obra: es la herramienta que la empaquetó |

Es un mapeo que pierde información a propósito, y por eso está aquí: la pérdida documentada es la regla de esta
sección.

### 2.2 Lo que se quedó sin metadatos vuelve a intentarse

La extracción ocurre **fuera de la ruta de importación**: escucha el bus y trabaja después. Si falla -por
contención de escritura, por un archivo que en ese momento no se dejaba abrir, o porque la aplicación se cerró
antes de que le tocara-, el documento se queda con el nombre de su archivo por título y sin nadie a quien
atribuirlo, y **nadie lo vuelve a intentar**: el evento ya pasó. En la instalación real eso fue el **20 % del
catálogo**, unas treinta mil ediciones.

Arreglar la causa -la transacción diferida- evita que vuelva a pasar, pero no repara lo que ya está
dentro. `metadata::reconciliar_metadatos` sí: busca ediciones con archivo disponible y **ninguna autoría**, y
vuelve a extraer. Corre en el arranque diferido, acotada a doscientas por sesión, así que converge en unas
cuantas aperturas en lugar de convertir un arranque en una reimportación.

La señal es no tener ninguna autoría, y no el título ni la portada: un libro puede llamarse igual que su archivo
legítimamente, y hay EPUB que no traen portada, así que mirarlos daría trabajo garantizado sin resultado
posible en cada arranque. En un PDF o un cómic, que casi nunca declaran autor, la marca de que ya se leyeron es
la portada: la pasada solo vuelve a uno que no la tenga, para no pintar su primera página en cada arranque.

### 2.2.1 Qué se lee de cada formato

| Formato | Metadatos | Portada |
|---|---|---|
| EPUB | Paquete OPF: título, autores con su papel, idioma, editorial, fecha, serie, sinopsis, materias e identificadores | La que declara el paquete |
| PDF | Título y autor del diccionario de información; un título que es un nombre de archivo o la firma del programa que generó el PDF se descarta | La primera página, pintada por el motor |
| CBZ | `ComicInfo.xml`, si lo trae: título (o la serie), serie y número, guionistas, dibujantes, editorial, fecha, idioma, sinopsis y géneros | La primera imagen en orden natural |

### 2.2.2 Corrección manual y baja

Desde la ficha de una obra se corrigen a mano título, serie, número, editorial, fecha, idioma y sinopsis
(`edit_edition_field`). Entra con procedencia `manual`, gana a todo lo demás y retira las propuestas vivas de
ese campo. El autor no se corrige ahí: es un contribuidor compartido con otras obras, y cambiarlo desde una
ficha renombraría a esa persona en toda la biblioteca.

La baja (`delete_document`) sigue [storage.md](storage.md), sección 2: el archivo referenciado no se toca nunca, y
la copia administrada solo se borra si se pide.

### 2.3 Un autor es una fila

Una importación que da de alta al autor de cada obra otra vez llena la tabla de filas repetidas: sobre una biblioteca
real eran 207.282 filas de `contributors` para 61.187 nombres. Por eso cada autor es una sola fila.

**Qué es el mismo autor:** el mismo nombre **sin distinguir mayúsculas ni tildes**. Dos
homónimos quedan juntos, como en Calibre, porque sin otro dato no se distinguen; el único que existe para eso, el
ORCID, queda fuera de la regla, y quien lo tiene no se funde con nadie.

**Cómo se garantiza.** `contributors.name_key` es una columna generada con la clave -minúsculas, sin tildes, sin
espacios de sobra- y lleva un **índice único** para las filas sin ORCID. Así es la base quien impide la segunda
fila, venga la escritura de la importación, de una restauración o de una migración futura. La clave es SQL puro,
`lower` y una cadena de `replace`, y no una función registrada desde Rust, para que la tabla se pueda leer con
cualquier `sqlite3`. La misma expresión vive en `db::catalog::clave_de_nombre_sql` y una prueba exige que sean la
misma cadena. **Lo que no pliega**: `lower` de SQLite solo conoce ASCII, y la tabla de tildes cubre el latín de
Europa, así que un nombre en cirílico o en griego se compara tal cual, mayúsculas incluidas.

**Lo que hace la importación**: `Contributor::ensure` inserta sin pisar nada y después busca por la clave, así
que dos importaciones que traen al mismo autor nuevo a la vez no lo crean dos veces. **Lo que hace una
restauración** de un archivo exportado antes de V38: funde las filas repetidas igual que la migración, por nombre o
por ORCID, en lugar de fallar entera por el índice.

**La migración** se queda con el que más obras tiene y, a igualdad, el más antiguo; pasa cada vínculo con un
`UPDATE`, para que los disparadores muevan la estantería de un nombre al otro y rehagan el orden por autor; quita
el vínculo que sobra cuando la obra ya tenía al superviviente; y rehace el catálogo de búsqueda de las obras cuyo
autor cambió de forma escrita, que no tiene disparador de cambio. Sobre una biblioteca real queda en 60.534
autores, sin referencias rotas y con las estanterías y el orden por autor cuadrando con el catálogo. Una migración
que recorre la biblioteca entera corre antes de abrir la ventana, así que se evita salvo cuando no hay alternativa.

## 3. Procedencia y confianza del metadato

Cada campo de metadato guarda su **origen**: extraído del archivo (`file`), obtenido de un proveedor en línea
(`online`), traído de una biblioteca de Calibre (`calibre`) o editado por el usuario (`manual`).

La jerarquía de precedencia es estricta:

$$\text{file} < \text{online} < \text{calibre} < \text{manual}$$

Reglas:

* La edición manual del usuario tiene la máxima precedencia y **nunca** es sobrescrita por una actualización
  automática.
* Los metadatos traídos de una biblioteca de Calibre conservan el trabajo que el usuario ya realizó en ella: una extracción
  automática de metadatos internos del archivo (`file`) no puede pisarlos y queda como propuesta.
* Una actualización automática que discrepe de un valor existente de mayor o igual precedencia se propone, no se aplica.
* La interfaz permite ver de dónde salió un dato y revertirlo a su valor original.

Esta es la diferencia entre un gestor en el que se puede confiar y uno que corrompe silenciosamente el trabajo
del usuario.

## 4. Identidad del documento

Un documento se identifica internamente por un identificador propio, y adicionalmente por:

* Un hash del contenido, para detectar duplicados y archivos movidos.
* Los identificadores externos que tenga (ISBN, DOI), que **no** son únicos ni fiables como clave primaria: la
  misma obra tiene múltiples ISBN por edición y formato.

### 4.1 Forma de la identidad por contenido

Implementado (`src-tauri/src/library.rs`).

La identidad es `sha256:` seguido de 64 hexadecimales en minúscula. Lleva el prefijo del algoritmo por una razón
concreta: si algún día hiciera falta otro, hay que poder distinguir los hashes viejos de los nuevos sin
recalcular la biblioteca entera para averiguarlo. Es la misma convención de las sumas de integridad de
subrecurso y de los digests de contenedor.

Se calcula **en flujo**, con un búfer fijo de 128 KiB. Un PDF escaneado pasa del gigabyte, y leerlo entero en
memoria para calcular su identidad convertiría una importación masiva en un fallo por falta de memoria.

Lo que la identidad permite responder, y lo que **no** decide:

| Situación | Qué se sabe | Quién decide qué hacer |
|---|---|---|
| Contenido nunca visto | `New` | Se importa |
| Mismo contenido, misma ruta | `AlreadyRegistered` | No se crea un registro nuevo |
| Mismo contenido, otra ruta que ya no existe | `Moved` | Se reubica, conservando metadatos y anotaciones |
| Mismo contenido, otra ruta que sí existe | `Copy` | Copia exacta: se cuenta aparte y no entra dos veces |

La distinción entre `Moved` y `Copy` no es un detalle: reubicar cuando el original sigue en su sitio borraría del
registro una copia que existe.

La distinción entre **obra**, **edición** y **archivo** debe existir en el modelo, aunque la interfaz la oculte.
Sin ella es imposible representar correctamente "tengo tres ediciones del mismo libro" ni fusionar bibliotecas.

## 5. Proveedores de metadatos en línea

* El núcleo trae un proveedor de portadas y metadatos (`metadata_online.rs`), y las extensiones pueden aportar
  más, con la misma autoridad: proponen, no escriben.
* Se prefieren fuentes con licencia abierta y condiciones de uso claras. Consultar una API sin verificar sus
  términos es un riesgo legal, especialmente para un producto con edición comercial.
* Cada consulta en línea es una fuga de metadatos hacia un tercero y debe ser visible para el usuario
  (ver [security/privacy.md](../audit/privacy.md)).

## 6. Exportación

La exportación no es una función secundaria: es la garantía material del principio de posesión de los datos.

| Qué | Formato | Especificación |
|---|---|---|
| El catálogo completo, con anotaciones, procedencia y preferencias | JSON abierto y versionado | [api/export-format.md](api/export-format.md) |
| Las anotaciones de un documento, con sus localizadores | Modelo de anotación web del W3C, y texto | [api/annotation-export.md](api/annotation-export.md) |
| Citas bibliográficas | Los formatos que aporte una extensión de citas; la propia trae BibTeX | [plugin-system.md](plugin-system.md) sección 2.5 |

## 7. Reparación de entidades de EPUB

**Implementado:** el importador resuelve las entidades XML de los atributos y las capas escapadas de la
sinopsis, sin cargar DTD ni entidades externas. Conserva comparaciones como `2 < 3` y los marcadores sin cierre. Al arrancar, una reparación de fondo corrige las series y
sinopsis antiguas que contienen esas secuencias; no ejecuta una migración global de la base.

Cada edición se repara en una transacción de escritura inmediata. Se compara el valor actual con el leído y se
excluyen los campos con procedencia de mayor prioridad que el archivo, incluida la manual y la de Calibre. La propuesta de archivo correspondiente se actualiza en la misma
transacción para que no restaure el texto anterior. Un error deja la reparación sin marca de finalización y
permite reintentar en el próximo arranque; no se registra contenido del usuario.

Las pasadas de metadatos ceden tiempo entre escrituras para que guardar una preferencia, nota o posición no
quede detrás de una cadena continua de actualizaciones. La pausa de reparación de entidades, series y sinopsis es al menos de 15 ms y al
menos tan larga como el trabajo de escritura de esa edición; no constituye una garantía de latencia de SQLite. La pasada solo se inicia si el estado de energía permite trabajo de fondo.
