# Catálogo de Eventos

> **Estado:** Implementado en `src-tauri/src/events.rs` (`DomainEvent`). Todo lo que cambia algo que la interfaz
> pinta se anuncia: ver "Quien escribe, lo anuncia".
> Las reglas del bus están en
> [event-bus.md](../event-bus.md); los recorridos completos, en
> [data-flows.md](../data-flows.md).

## Reglas de este catálogo

* Todo evento declara **emisor**, **consumidores previstos**, **carga útil** y **versión de esquema**.
* Las cargas útiles transportan identificadores y referencias, nunca contenido de documentos.
* Los payloads solo crecen con campos opcionales; los consumidores ignoran campos desconocidos. Ver
  [versioning.md](../versioning.md).
* Un evento se emite **después** de confirmar la transacción que lo origina.

## Quien escribe, lo anuncia

Es el mecanismo de los principios 1 y 2 de la sección 7 de la
[filosofía de producto](../../design/product-philosophy.md): ningún cambio exige reiniciar, y un dato que se
ve en dos pantallas cambia en las dos a la vez. **Toda escritura que cambia algo que la interfaz pinta se anuncia**
en el bus después de confirmarse, la emita un comando o un servicio de fondo. La interfaz no guarda copias que
sincronizar: `src/lib/core/vivo.ts` oye estos anuncios, los agrupa -una importación de cien mil documentos son cien
mil anuncios, y se avisa como mucho cada dos segundos- y le dice a la carcasa **qué** volver a leer. Lo que se ve se
relee en el momento; lo que no se ve, al volver a verse.

| Anuncio | Qué vuelve a leer la interfaz |
|---|---|
| `DocumentImported`, `DocumentDeleted`, `AvailabilityChanged`, `BulkImportFinished`, `MetadataUpdated`, `CoversRecovered` | Las obras: la biblioteca, la ficha enfocada e Inicio |
| `ProgressSaved` | El progreso: lo mismo, más la barra de la ficha con la cifra del propio anuncio |
| `CollectionsChanged` | Las colecciones, la pertenencia de la obra enfocada, y la biblioteca si filtra por una |
| `ExtensionsChanged` | Todo lo que aportan las extensiones: temas, modos, órdenes, vistas, fondos, formatos de cita |

Lo comprueban las dos mitades: `src-tauri/tests/estado_vivo.rs` pide cada comando que escribe por el mismo camino
que el webview y exige su anuncio, y los recorridos de estado vivo de `e2e/flujo.spec.ts` comprueban que la pantalla
cambia sin recargar. **Sin bus**, cada acción del propio usuario avisa por su cuenta; con bus no lo hace, a propósito:
si lo hiciera siempre, un anuncio que el runtime dejara de publicar no se notaría en ninguna pantalla que el usuario
toque.

## Catálogo

### Ciclo de vida de documentos

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `DocumentImported` | Library Service | Metadata, Search, UI | `edition_id`, `file_id`, `media_type`. |
| `DocumentDeleted` | Library Service | Search, Index, UI | `edition_id`. |
| `AvailabilityChanged` | Repaso de carpetas | UI | `editionId`. Un archivo de una raíz referenciada dejó de estar o volvió. La edición no sale de la biblioteca: queda como no disponible, con sus anotaciones |
| `MetadataUpdated` | Metadata Service, comandos de metadatos | UI | `editionId`. Lo emiten el servicio que extrae los metadatos al importar, aplicar un candidato, aceptar una propuesta y traer una portada. **Sin lista de campos**: quien pinta la edición la vuelve a pedir entera, y una lista solo serviría para decidir no refrescar lo que sí cambió |

### Indexación

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `BulkImportProgress` | Library Service | UI | `processed`, `imported`, `skipped`, `rejected`, `duplicates`, `total`. Solo números: una carpeta de mil documentos emitiría mil títulos. `skipped` es lo que ya estaba -el mismo archivo visto otra vez- y `duplicates`, las copias exactas en otra ruta de algo que ya estaba; `BulkImportFinished` lleva las mismas cifras |
| `IndexingProgress` | Search Service | UI | `pending`, `indexed`, `skipped`, `failed`, `paused`, `pauseReason`. Solo números, por lo mismo. `paused` y su motivo viajan porque una barra detenida sin explicación se lee como que la aplicación se ha colgado |

### Lectura y anotación

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `ProgressSaved` | Reader Service | UI | `editionId`, `progression`. Se emite **cada vez que el registrador escribe**, no cada vez que se apunta: lo que se anuncia es lo que ya está en la base, así que quien vuelva a preguntar lee lo mismo. Lleva la progresión y no el Locator porque es lo único que pintan las demás pantallas |

### Colecciones y extensiones

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `CollectionsChanged` | Comandos de colecciones | UI | `collectionId`, nulo al crear o borrar porque lo que cambia es la lista. Añadir lo que ya estaba no se anuncia: no cambió nada |
| `ExtensionsChanged` | Comandos de extensiones | UI | Ninguna. Instalar, apagar, encender, desinstalar, revocar un permiso, volver atrás y una comprobación de retiradas que traiga algo. Sin identificador porque lo que aporta una extensión se pide en listas enteras |

### Modelos de IA

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `AiModelProgress` | AI Service | UI | `modelId`, `done`, `total`. Bytes y no porcentaje, para que la interfaz pueda decir cuánto falta. Uno por megabyte y no por bloque: a 64 KiB por bloque serían miles de eventos para una barra que no se distingue a esa resolución |
| `AiModelFinished` | AI Service | UI | `modelId`, `outcome` (`installed`, `cancelled`, `needsConsent`, `failed`), `host`, `reasonCode`. `needsConsent` trae el anfitrión porque puede no ser el pedido sino la red de distribución a la que redirigió |

Los escucha la aplicación y no Ajustes, igual que la importación: salir de Ajustes no para la descarga, y volver
tiene que encontrarla donde iba.

### Preparación de la búsqueda por significado

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `VectorJobProgress` | AI Service | UI | `editionId`, `done`, `total`, `queued`, `pauseReason` (`USER_PAUSED`, `READER_ACTIVE`, `AI_SUSPENDED_BATTERY`, `AI_SUSPENDED_HEAT` o nulo). Uno por lote de cuatro fragmentos, y uno cada vez que empieza o deja de esperar |
| `VectorJobFinished` | AI Service | UI | `outcome` (`done`, `cancelled`, `budget`, `failed`), `reasonCode`. Se emite con la cola vacía, no por documento |

Los escucha un estado compartido que solo cargan las piezas que los enseñan —la búsqueda, las colecciones y el
lector—, porque la carcasa está en el paquete de arranque.

### Preparar audio de un libro

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `AudioJobProgress` | Voz | UI | `editionId`, `done`, `total`, `remainingMs`, `seconds`, `queued`. Uno por fragmento dicho; `remainingMs` sale del ritmo medido en esa pasada y llega nulo hasta el tercero |
| `AudioJobFinished` | Voz | UI | `editionId`, `outcome` (`done`, `cancelled`, `budget`, `failed`), `reasonCode`. Lo cancelado deja lo hecho utilizable |

### Portadas recuperadas

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `CoversRecovered` | Metadatos | UI | `count`. Se emite tras volver a sacar del documento las portadas que la caché ya no tenía, y sirve para que la pantalla que las pidió vuelva a pedir su página |

Existe porque la caché es caché: un limpiador del sistema la vacía y el catálogo sigue apuntando el nombre de cada
portada; sin esto, la biblioteca quedaría llena de imágenes rotas.

### Voz del runtime

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `VoiceFinished` | Voz | UI | `block`. La voz terminó de decir un bloque. Va por el bus y no como respuesta porque hablar dura lo que dura el párrafo |
| `VoiceFailed` | Voz | UI | `reasonCode` |

### Salud del sistema

| Evento | Emisor | Consumidores | Carga útil |
|---|---|---|---|
| `ComponentStateChanged` | Monitor | UI | `component`, `state`, `reasonCode` |

Es el mecanismo que hace observable la degradación elegante. Ver
[component-model.md](../component-model.md).

El discriminante serializado, el valor que devuelve `DomainEvent::name` y el nombre de esta tabla son la misma
cadena. Una prueba lo verifica: si divergen, el webview filtra por un nombre y recibe otro.
