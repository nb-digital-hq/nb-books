# Flujos de datos

> **Estado:** Implementado. Traza los recorridos completos de los datos entre la interfaz, el runtime, el disco y
> la red, para revisar dónde se validan, dónde se persisten y dónde podrían filtrarse.

## 1. Importación de un documento

```mermaid
sequenceDiagram
  actor U as Persona
  participant UI as Interfaz
  participant RT as Runtime (library.rs)
  participant DB as SQLite
  participant Bus as Bus de eventos
  participant M as Metadatos (metadata.rs)
  participant S as Indexador (search.rs)
  U->>UI: elige una carpeta (diálogo nativo)
  UI->>RT: ruta elegida
  RT->>RT: canonicaliza y acota la ruta
  RT->>RT: detecta el formato por contenido y calcula el hash
  RT->>DB: BEGIN IMMEDIATE, inserta obra, edición y archivo
  DB-->>RT: confirmado
  RT->>Bus: DocumentImported
  Bus->>M: metadatos internos y miniatura de portada
  Bus->>S: encola la indexación
  Bus->>UI: refresco de la biblioteca
```

Puntos de control:

* La transacción se confirma antes de emitir el evento; ningún consumidor observa estado a medias.
* En modo referenciado se guarda la ruta **relativa a su raíz de biblioteca** y el hash; el archivo no se copia ni
  se modifica.
* Un archivo inválido no deja registro huérfano. Las copias exactas se cuentan aparte y no entran dos veces.
* **Una sola importación masiva a la vez.** Pedir otra mientras hay una en marcha responde `running: true` y la
  interfaz muestra el avance de la que hay.
* **El avance dice el total desde el primer aviso**: la carpeta se recorre entera antes de importar nada.
* **La indexación cede el paso mientras entra una importación**, y se reanuda al terminar. Con mil documentos
  reales, la peor lectura de la interfaz baja de 3,3 a 1,4 ms y la importación de 9,3 a 7,9 s.

## 2. Lectura de un documento

```mermaid
flowchart LR
  UI["Interfaz: pedir sección o página"] --> RT["Runtime: resolver la edición a una ruta segura"]
  RT --> C{"¿En caché?"}
  C -- sí --> REF["Referencia al recurso"]
  C -- no --> P["Proveedor: EPUB saneado, PDF con PDFium, cómic"] --> REF
  REF --> UI2["Interfaz: carga por el protocolo de recursos, no por el canal JSON"]
  UI2 --> PR["Progreso con antirrebote"] --> DB[("SQLite: progress, por Locator")]
```

* El progreso se guarda con antirrebote, y cerrar o salir del lector escribe lo pendiente.
* **Un documento que nunca se ha leído se abre por la primera sección con texto de verdad** (doscientos caracteres
  visibles, mirando como mucho doce secciones), no por la cubierta. Si no la hay, se abre por la primera.
* Las páginas de PDF y cómic se sirven desde la caché de la aplicación; ningún archivo del usuario es alcanzable
  por el protocolo de recursos.

## 3. Anotación

```mermaid
flowchart LR
  SEL["Selección por bloques o con el ratón"] --> LOC["Interfaz: Locator con selectores"]
  LOC --> V["Runtime: valida el Locator"]
  V --> DB[("SQLite: annotations, inmutable")]
  DB --> EV["Bus: AnnotationCreated"] --> PINTA["Interfaz: pinta la marca en el texto"]
```

Una anotación es inmutable (migración V7): editarla es darla de baja y crear otra con nuevo identificador, que
conserva su historia y simplifica el reanclaje.

## 4. Búsqueda

```mermaid
flowchart LR
  Q["Consulta"] --> L["FTS5: catálogo y contenido"]
  Q --> V["Vectores, si el modelo está instalado"]
  L --> F["Fusión de clasificaciones"]
  V --> F
  F --> R["Resultados acotados, cada uno con su Locator"]
  R --> UI["Interfaz: abrir la posición exacta"]
```

El filtrado nunca ocurre en JavaScript sobre colecciones completas.

### 4.1 Recorrer por facetas

Un panel de autores, series o materias no agrupa el catálogo: cada estantería tiene su recuento al día en
`shelf_counts`, que mantienen los disparadores de la migración V24, y el panel lo lee por un índice de cobertura.
Para saber cuáles son las veinte estanterías mayores habría que contar todas, porque `LIMIT` recorta el resultado
y no el trabajo: con 55.740 autores eran 4.141 ms, y materializado son 6.

Las bajas cuelgan de las tablas de enlace, porque en la versión de SQLite que se compila (3.53.4) la cascada sí
dispara los disparadores. `tests/estanterias.rs` lo comprueba, así que una versión futura que cambie ese
comportamiento hace fallar la suite.

## 5. Funciones de estudio con IA

```mermaid
flowchart TB
  P["Petición: resumen, tarjetas, preguntas o conceptos"] --> R["Recuperación local de fragmentos, con Locator"]
  R --> C["Composición: instrucción del producto separada del texto del documento"]
  C --> D{"Proveedor elegido"}
  D -- local --> L["Inferencia en el equipo"]
  D -- externo --> K{"¿Consentimiento para este proveedor?"}
  K -- no --> STOP["Se pide, mostrando qué sale y a quién"]
  K -- sí --> E["Envío por net.rs: fragmentos necesarios y la petición"]
  L --> S["Salida validada, con citas verificables"]
  E --> S
  S --> UI["Interfaz: texto plano, nunca HTML"]
  E --> LOG[("ai_sends: registro local, exportable y borrable")]
```

Nunca sale un documento completo, y el primer envío a un proveedor nuevo muestra exactamente qué se enviaría.

## 6. Diagnóstico

```mermaid
flowchart LR
  A["La persona pide el diagnóstico"] --> B["Registros y estado de componentes"]
  B --> C["Redacción: rutas, nombres de usuario y títulos"]
  C --> D["Vista previa en Ajustes"]
  D --> E["Archivo en la ubicación que la persona elija"]
```

El envío nunca es automático.
