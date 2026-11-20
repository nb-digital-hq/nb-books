# Contrato de carga y optimizaciones

> **Estado:** Implementado. Las cifras son propiedad de [performance-budgets.md](performance-budgets.md); aquí se
> citan, y si discrepan manda aquel documento.

En una aplicación de escritorio con la interfaz en un webview, el trabajo en Rust y una base local de cientos de
miles de registros, la interfaz no se congela por una consulta lenta suelta: se congela cuando **nada impide** que
una pantalla pida o pinte algo cuyo tamaño depende de la biblioteca. Por eso lo primero no es una optimización,
sino un contrato con mecanismo.

## 1. Por qué se congela una interfaz en un webview

El webview tiene **un solo hilo** para JavaScript, disposición y pintado. `await invoke(...)` no lo bloquea; lo que
lo bloquea es insertar y maquetar miles de nodos en una sola tarea. Y en Tauri, un comando sin `async` corre en el
hilo principal, que es el que atiende la ventana: la ventana entera queda bloqueada mientras dura la consulta.

Cuatro causas pueden coincidir, y se distinguen midiendo, no de oído:

| Causa | Cómo se distingue | Salida |
|---|---|---|
| La consulta devuelve una fila por elemento de la biblioteca | Contar las filas que devuelve el comando | Acotarla en origen |
| Está acotada pero se pinta entera de una vez | Contar nodos contra el presupuesto de 1.500 | Paginar o trocear el pintado |
| El recuento agrupa sobre todo el catálogo sin índice | Leer el plan con `EXPLAIN QUERY PLAN` | Índice de cobertura o clave materializada |
| La respuesta cruza el puente entera | Medir el tramo por separado | Acotar en origen |

## 1.3 El contrato de carga

Seis reglas, con el mismo rango que los innegociables de `AGENTS.md`:

1. **Ninguna consulta de la interfaz devuelve un número de filas que dependa del tamaño de la biblioteca.** Toda
   lista se pide acotada. Mecanismo: `scripts/check-listas.mjs`; un comando que devuelve una colección lleva
   `limit` o declara su cota con `Cota: <cuál>`.
2. **Nada se pinta en una sola tarea si excede el presupuesto de nodos.** Mecanismo: un recorrido de
   `e2e/rendimiento.spec.ts` abre cada destino, cada vista de Estudio y el panel de estanterías y cuenta los nodos
   del documento entero.
3. **Toda petición que puede tardar tiene guarda de respuesta obsoleta**: solo se pinta la última respuesta pedida.
   Implementado en `src/lib/core/carga.ts`. El puente de Tauri no permite cancelar una llamada en vuelo, así que lo
   que se hace es descartar su respuesta.
4. **El estado de carga forma parte del contrato de la vista**: vacío, cargando, con datos y en error se diseñan a
   la vez. El indicador de carga (`Spinner.svelte`) espera un umbral antes de aparecer, porque casi todo contesta
   antes de que nadie lo note y un destello informa menos que el silencio.
5. **Un panel que enumera facetas se sirve como la biblioteca**: por páginas o por búsqueda, nunca "todo", y su
   coste no depende del tamaño. Los recuentos de estanterías se materializan (migración V24).
6. **Un comando que no toca la ventana es asíncrono.** Mecanismo: `scripts/check-comandos.mjs`. La excepción es
   exacta: lo que toca ventana, diálogo nativo, menú o bandeja, porque en Linux la biblioteca gráfica no es segura
   entre hilos.

Cuando una faceta necesita ordenarse o contarse y su clave vive al otro lado de dos uniones, **la salida es
materializar la clave**, no añadir otro índice (`author_sort`, de 26,9 a 2,1 ms).

## 2. Optimizaciones aplicadas, por capa

### 2.1 Puente entre el webview y Rust

| Técnica | Nota |
|---|---|
| No cruzar binario: devolver rutas y que el motor lea por el protocolo propio | [ADR-002](../adr/002-critical-bottlenecks.md). Pedir datos es menos del 4 % de un cambio de página |
| Protocolo propio para servir bytes | Sirve portadas e imágenes. El audio se trae con `fetch` y se entrega como `blob:`, porque WebKitGTK no reproduce medios del esquema propio |

### 2.2 Base de datos

| Técnica | Nota |
|---|---|
| `journal_mode=WAL` y `synchronous=NORMAL` | En WAL, `NORMAL` no corrompe; solo arriesga los últimos commits ante un corte de luz |
| `BEGIN IMMEDIATE` en toda transacción que escribe | [storage.md](storage.md) 1.1, con guardián |
| `busy_timeout` y agrupación de conexiones | Mismo sitio |
| Índices de cobertura por orden | Páginas de 2 a 10 ms sobre 152.080 ediciones |
| Claves y recuentos materializados | `author_sort` (V19) y `shelf_counts` (V24) |
| Guardián que lee el **plan** y no el reloj | `recorrido_biblioteca.rs`; se reutiliza en cada consulta nueva |
| Dos índices separados, catálogo y contenido | [search.md](search.md): buscar un título no devuelve doscientos párrafos |

### 2.3 Hilos

| Técnica | Nota |
|---|---|
| Trabajo pesado fuera del hilo que atiende la ventana | Todos los comandos son asíncronos |
| Trabajo de fondo que cede | La indexación se detiene con poca batería, por el presupuesto de disco o mientras el lector pinta |
| Cola reanudable sin cursor persistido | Se reencola lo que no tiene fragmentos |
| Inferencia en su propio grupo de hilos, con prioridad baja y tope | [ai/local-models.md](ai/local-models.md) sección 5 |

### 2.4 Imágenes

| Técnica | Nota |
|---|---|
| Miniaturas al tamaño al que se pintan | 400 px para pintar a 188x282 en pantalla de densidad doble |
| Carga perezosa y `decoding="async"` | Solo se descodifica lo visible |
| Adelantar la página siguiente, acotado a lo que cabe en pantalla | De 265 a 112 ms de mediana; adelantar la página entera empeoraba la cola |
| `content-visibility: auto` en listas largas | Evita maquetar lo que está fuera de pantalla |

### 2.5 Interfaz

| Técnica | Nota |
|---|---|
| Estado global en almacenes dedicados | [state-management.md](state-management.md) |
| Datos derivados con `$derived`; efectos solo para el mundo exterior | Misma sección |
| Un solo punto de acceso al puente | Innegociable, con lint |
| El paquete partido por destino | Arranque en 229 KB de 250; `pnpm check:paquete` |
| `contain: layout size` en la barra superior | Corta la disposición por letra en WebKitGTK |

## 3. Descartadas

| Técnica | Motivo |
|---|---|
| `createImageBitmap` para descodificar fuera del hilo | Solo Chromium descodifica de verdad fuera del hilo principal, y el producto corre en WebKitGTK |
| Caché de datos de la biblioteca en la interfaz | Contestar la consulta es menos del 4 % de un cambio de página; lo caro es rehacer el DOM y descodificar |
| Virtualización continua de la biblioteca | Con 152.080 obras un desplazamiento continuo no lleva a ninguna parte, y el bloque dependía del desplazamiento; se usan páginas discretas de tamaño cerrado |
| Indicadores de carga animados permanentes | Una animación permanente compite con el desplazamiento por el mismo fotograma |

## 4. El método

* **Medir antes de concluir.** Un diagnóstico de oído se ha equivocado más de una vez en este proyecto.
* **Medir en WebKitGTK**, el motor del producto, y no en Chromium, el de la suite.
* **Leer el plan de la consulta**, no el reloj: el plan dice si crece con la biblioteca; el reloj dice cómo de
  ocupada estaba la máquina.
* **Comprobar que el banco permite que la medida salga mal** antes de fiarse de ella.
* **Convertir cada regla en un guardián**: una regla que solo vive en la disciplina de quien programa vuelve a
  romperse.
