# Esquema de Persistencia

> **Estado:** Implementado. Las sentencias `CREATE TABLE` viven en las migraciones embebidas de
> `src-tauri/migrations/`, y su cara tipada en `src-tauri/src/db/catalog.rs`. Decisiones de fondo:
> [ADR-004](../../adr/004-reader-locator-model.md) y [ADR-010](../../adr/010-settings-persistence.md).

La forma de evolucionar este esquema, y las reglas que lo hacen seguro en la máquina del usuario, están en
[migrations.md](../migrations.md) y
[versioning.md](../versioning.md).

## 1. Configuración obligatoria de la conexión

```sql
PRAGMA journal_mode = WAL; -- lecturas concurrentes sin bloquear escrituras (ADR-002)
PRAGMA foreign_keys = ON; -- SQLite las desactiva por defecto
PRAGMA busy_timeout = 5000; -- evita "database is locked" visible para el usuario
PRAGMA synchronous = NORMAL; -- correcto y suficiente en modo WAL
```

Estos ajustes se aplican **en cada conexión del pool**, no una sola vez al crear la base.

## 2. Modelo conceptual

La distinción entre obra, edición y archivo debe existir en el modelo aunque la interfaz la oculte. Sin ella es
imposible representar "tengo tres ediciones del mismo libro" ni fusionar bibliotecas. Ver
[metadata-model.md](../metadata-model.md).

```
work 1---N edition 1---N file
                 |
                 +---N  annotation
                 +---N  reading_session
                 +---N  progress (por perfil)
```

## 3. Tablas del núcleo

Una tabla no se crea antes de que se decida su contenido: adelantarla produce columnas que hay que migrar en cuanto
la decisión real se toma.

| Tabla | Propósito | Notas de diseño |
|---|---|---|
| `works` | La obra como concepto | Título canónico, idioma original |
| `editions` | Una publicación concreta de una obra | Editorial, fecha posiblemente parcial, mención de edición, serie |
| `edition_identifiers` | Identificadores externos de la edición | ISBN, ISSN, DOI y otros. Tabla propia porque una edición tiene varios a la vez |
| `notes` | Una nota propia del usuario, que no cuelga de ningún documento | Título único, que es el nombre con el que se la enlaza, y cuerpo. **Se edita**, a diferencia de una anotación |
| `note_links` | Un enlace entre dos notas | Origen y destino. **El retroenlace no se guarda**: es esta misma fila leída del otro lado |
| `note_annotation_links` | Un enlace de una nota a una anotación anclada | Tabla aparte y no una columna polimórfica: una clave foránea polimórfica no la puede hacer cumplir la base |
| `library_roots` | Un lugar declarado donde viven archivos | Nombre legible, clase administrada o referenciada, ruta absoluta (nula si no está localizada) |
| `files` | El archivo en disco | **Raíz y ruta relativa**, hash de contenido (`sha256:` y 64 hexadecimales), modo administrado o referenciado, estado de disponibilidad. La ruta relativa no puede salirse de su raíz, y desde V28 eso se comprueba **por componentes**: hasta entonces se rechazaba `..` en cualquier posición, lo que dejaba fuera 667 de los 152.080 archivos del corpus real -elipsis en los títulos- sin detener una sola travesía |
| `contributors` | Personas y entidades | Separado para permitir varios autores con rol. **Una fila por autor** desde V38: la clave `name_key`, sin mayúsculas ni tildes, es única entre los que no tienen ORCID |
| `edition_contributors` | Relación con rol | Autor, editor, traductor, ilustrador |
| `subjects` / `edition_subjects` | Materias y clasificación | |
| `annotations` | Anotación inmutable | Selectores serializados, no una posición numérica. Editar es dar de baja y crear otra, encadenadas por `supersedes`; dos disparadores rechazan cualquier `UPDATE` de las columnas que definen la anotación, y también revivir una baja escribiendo nulo |
| `metadata_provenance` | Origen de cada campo de metadato | Archivo, proveedor en línea, LMS o edición manual |
| `metadata_suggestions` | Cambio automático que no se aplicó porque el usuario ya había decidido ese campo | Una sola propuesta viva por campo y origen. **Lo rechazado no se vuelve a ofrecer ni se escribe**: se compara el valor, no solo el campo, para que rechazar un título no condene el campo para siempre |
| `progress` | Progreso de lectura | **Un Locator serializado**, nunca un entero. Cuelga de la edición, no del archivo: el archivo puede moverse o reimportarse y el usuario espera seguir donde estaba. Guarda además con qué precisión se restauró la última vez |
| `settings` | Preferencias del usuario | Fuente de verdad; sustituye al almacenamiento del webview (ADR-010). Valor como escalar JSON y versión de esquema por preferencia. Dos disparadores rechazan cualquier clave con pinta de secreto |
| `collections` / `collection_items` | Agrupaciones del usuario | Manuales o por regla. Las por regla **no guardan miembros**: los evalúan al consultar, así que no pueden quedarse obsoletas. |
| `reading_sessions` | Sesiones medibles | Base de la analítica local. Dos Locator, no un número de páginas: las páginas dependen de la tipografía, así que "leí 30 páginas" no significa lo mismo el martes que el jueves. Una fila sin `ended_at` dice que hubo un cierre inesperado, y eso es información |
| `documents_fts` | Índice de texto completo | Tabla FTS5 **sin contenido** (`content = ''`, `contentless_delete = 1`): guarda el índice invertido y nada más, porque la copia del texto pesaba el 52,1 % de la biblioteca. Tokenizador `unicode61 remove_diacritics 2`. El alta la escribe `search::index_edition_con`, no un disparador: sin texto en la tabla, el disparador no tenía de dónde sacarlo. La baja sí sigue siendo un disparador, que para borrar basta el identificador |
| `text_chunks` | Fragmentos con su localizador | Base común de búsqueda y recuperación semántica. **Sin una sola columna numérica**: la posición vive en el locator y solo ahí. Tampoco guarda el texto: el pasaje se relee del documento. Es caché y se regenera de los documentos |
| `index_probes` | Una palabra por edición, y dónde tiene que encontrarse | La muestra con la que se comprueba un índice que ya no puede compararse con su contenido. La palabra **ya está dentro del índice**, así que no vuelve a guardar texto: son unos veinte bytes por edición. Se comprueba antes de guardarse, porque una sonda mal elegida haría borrar un índice sano |
| `catalog_fts` | Índice de título, autor y materias | FTS5 propio, separado del texto: buscar un libro y buscar dentro de un libro son preguntas distintas. Siete disparadores lo mantienen al día, incluido cuando el autor llega después por el servicio de metadatos |
| `search_status` | Si un documento se puede buscar, y por qué no | Cuatro motivos distintos, no uno: solo el escaneo sin OCR tiene arreglo. Un `CHECK` impide un "no buscable" sin motivo y un "buscable" con él |
| `vocabulary` | Una palabra consultada en el diccionario, con dónde apareció | **Un Locator serializado**, nunca un entero: desde cada entrada se salta al punto exacto. Guarda el lema y la forma seleccionada, que casi nunca coinciden. Sin `UNIQUE` por lema: la misma palabra en otro libro es otro dato, y el rebote se descarta al escribir |
| `study_cards` | Una tarjeta o una pregunta de autoevaluación que el usuario guardó | Solo entra lo que alguien aceptó: lo generado y no guardado no se escribe. **Sin una sola columna numérica**: las fuentes son un objeto con locators, la opción correcta va dentro de `choices` y la versión de la plantilla, dentro de `template`. `provider` y `template` son obligatorios, así que no puede existir una fila que no diga quién la generó; `edited_at` dice que se editó, y editar no borra la procedencia |
| `concepts` | Un concepto o una entidad del texto, con su clase y su procedencia | `UNIQUE (edition_id, name)` con `COLLATE NOCASE`: uno por libro, y guardarlo otra vez junta sus fuentes. El mismo nombre en dos libros **no se guarda como enlace**: se cuenta al consultar, como los retroenlaces de las notas. Es la base del grafo de conocimiento personal |
| `network_consents` / `network_log` | A dónde se puede salir, y a dónde se salió | El consentimiento es por anfitrión **y por propósito**: permitir que un servidor sirva libros no lo autoriza a recibir el texto de una selección. El registro no guarda la ruta ni la consulta, solo el anfitrión y el propósito |
| `catalogs` | Un catálogo OPDS que el usuario añadió | Dirección y nombre, nada más: **no se guarda lo que el catálogo devuelve**. Un `CHECK` exige `https`, y la tabla nace vacía: ninguno viene preconfigurado |
| `import_runs` | Una importación, terminada o a medias | Una fila con `finished_at` nulo significa "se empezó y no se terminó", y esa señal **funciona si el proceso muere**: nadie tiene que escribir nada al morir. Es lo que permite ofrecer continuar. Se conservan las últimas veinte |
| `import_rejections` | Un archivo que no entró, y por qué | La ruta y el **código canónico** de `error.rs`, nunca el mensaje: se traduce al pintarlo. Se guardan hasta mil por ejecución, y el recuento de la ejecución sigue siendo el total, así que la cifra no miente aunque la lista se acote |
| `import_duplicates` | Una copia exacta que no entró, y de qué es copia | La ruta de la copia, relativa a la carpeta elegida, y el archivo registrado que tiene el mismo contenido; su título y su ruta se consultan, no se copian, y la fila se va en cascada si el original sale de la biblioteca. Hasta mil por ejecución, con el total en `import_runs.duplicates`, que V37 separó de `already_in_library`: aquello es el mismo archivo visto otra vez, y esto, el mismo libro dos veces en el disco |
| `shelf_counts` | Recuento materializado de cada estantería | Lo mantienen disparadores (V24): abrir un panel de facetas no agrupa el catálogo |
| `chunk_vectors` / `vector_excluded_editions` / `vector_excluded_collections` | Vectores de los fragmentos y lo que el usuario no quiere vectorizar | Ver [ADR-007](../../adr/007-vector-store.md) |
| `ai_sends` / `ai_reports` | Lo enviado a un proveedor de IA externo, y lo que el usuario marca como incorrecto | Locales, exportables y borrables; el reporte no guarda el texto del documento |
| `audio_sections` / `audio_marks` | Audio preparado y sus marcas por Locator | Cuenta contra el presupuesto de lo derivado |
| `note_boards` / `note_board_columns` / `note_board_cards` | Tableros de notas por columnas con nombre | Cada tablero tiene su equivalente en lista |
| `extension_withdrawals` | Avisos de retirada de extensiones | Sobreviven al reinicio |
| `app_metadata` | Datos de la propia aplicación | Versión que creó la base |

### 3.1 Convenciones del esquema

Vigentes desde la V2 y detalladas en
[migrations.md](../migrations.md): identificadores `TEXT` de 32 hexadecimales generados
por la base, marcas de tiempo en ISO-8601 UTC, `STRICT` en todas las tablas, `updated_at` mantenido por
disparador y listas de valores cerradas con `CHECK`, reflejadas en un `enum` de Rust.

### 3.2 La cara tipada del esquema

`src-tauri/src/db/catalog.rs` es la representación en Rust de estas tablas. Vive bajo `db` y no dentro de un
servicio porque no pertenece a ninguno: la biblioteca, los metadatos, el lector y la búsqueda leen y escriben las
mismas entidades, y meterla en uno de ellos obligaría a los demás a depender de él.

## 4. Reglas no negociables del esquema

1. **Los archivos no se guardan como BLOB.** Los documentos viven en el sistema de archivos; la base guarda ruta
   y hash. Ver [storage.md](../storage.md).
2. **El progreso y las anotaciones se anclan con Locator**, no con enteros ni con desplazamientos de caracteres.
3. **La procedencia del metadato se almacena**, para que una actualización automática nunca sobrescriba una
   edición manual del usuario sin avisar. Lo que no se puede aplicar tampoco se descarta: queda en
   `metadata_suggestions`, porque tirarlo en silencio pierde información igual que sobrescribir.
4. **Los identificadores externos no son clave primaria.** El mismo libro tiene varios ISBN según edición y
   formato.
5. **Caché y verdad están separadas.** Portadas, páginas renderizadas, índice FTS y vectores son regenerables y
   viven fuera del directorio de datos.
6. **Ninguna columna se renombra en sitio ni se elimina** sin pasar por el ciclo de deprecación.
7. **Los secretos no entran aquí.** Tokens y claves de API viven en el almacén de credenciales del sistema
   operativo. Ver [security/authentication.md](../security/authentication.md).

## 5. Definición formal

Las sentencias `CREATE TABLE` definitivas viven en los archivos de migración embebidos
(`src-tauri/migrations/`), que son la fuente de verdad ejecutable. Este documento explica el porqué; aquellos
archivos declaran el qué. Si divergen, gana la migración y este documento se corrige.
