# Presupuestos de rendimiento

> **Estado:** Implementado. Cada presupuesto tiene una medida sobre el binario de publicación o sobre WebKitGTK,
> el motor del producto, y los que pueden comprobarse de forma determinista los hace cumplir la verificación.

Un presupuesto solo sirve si existe antes que el código que lo consume y si algo lo mide. Esta página fija los
límites, dice qué los hace cumplir y recoge las medidas vigentes. Los procedimientos de medida están en
[pruebas de rendimiento](../audit/performance/testing.md).

## 1. Límites

### 1.1 Binario y proceso

| Métrica | Presupuesto | Medida vigente | Mecanismo |
|---|---|---|---|
| Binario de publicación | `< 50 MB` | 29,2 MiB (30.607.304 B), margen 1,63x | Se mide en cada empaquetado |
| Paquetes `.deb` y `.rpm` | `< 50 MB` | 16,4 MB | Se mide en cada empaquetado |
| AppImage | Sin presupuesto | 130,1 MB | Lleva dentro GTK, WebKitGTK y GStreamer por diseño |
| Arranque hasta base lista | `< 200 ms` | 97–162 ms con 152.080 documentos | `pnpm baseline`; `tests/arranque_real.rs` |
| Latencia del puente IPC (ida y vuelta vacía) | `< 5 ms` | 0,92–1,04 ms de media en WebKitGTK | `src-tauri/src/bin/medir_puente.rs` |
| Memoria en reposo | Sin techo absoluto; lo propio, unos 80 MB sobre el suelo del motor | 281–323 MB de PSS total, con WebKitGTK vacío en 220–240 MB | `scripts/medir-memoria-en-reposo.mjs` avisa si lo propio crece más de 10 MB |

La memoria no tiene techo absoluto a propósito: el motor vacío ya gasta más de 200 MB, y fijar un techo propio
obligaría a sacrificar funciones o aspecto para caber. Lo que se vigila es que lo que añade NB Books no crezca sin
que se decida, midiéndolo contra el suelo de WebKitGTK en la misma ejecución.

### 1.2 Interfaz

| Métrica | Presupuesto | Medida vigente | Mecanismo |
|---|---|---|---|
| JavaScript antes de la primera pantalla | `< 250 KB` sin comprimir | 229 KB | `pnpm check:paquete`, en `pnpm verify` |
| Trozos cargados a demanda | Siguen siendo trozos aparte | Cada destino en su trozo | `pnpm check:paquete` comprueba que ningún `import()` vuelve al arranque |
| Tiempo de fotograma | `< 16,6 ms` (60 fps) | Ver sección 3 | Solo se anima `transform` y `opacity`; una animación que no cabe se elimina |
| Nodos del DOM por vista | `< 1.500` | Cumplido por construcción | Páginas de tamaño cerrado (`TAMANOS_DE_PAGINA`), recorrido en WebKitGTK |
| Píxeles descodificados | Biblioteca 20 Mpx, Inicio 15 Mpx, lector 5 Mpx | Miniaturas de 400 px | `pnpm check:webkitgtk` |
| Capas permanentes en reposo | 0 elementos con `will-change` | 0 | `scripts/check-ui.mjs` y `pnpm check:webkitgtk` |

### 1.3 Casos de uso

| Métrica | Presupuesto | Medida vigente |
|---|---|---|
| Paginar un PDF | `< 150 ms` por página | Mediana 74,8 ms, máximo 98,5 ms sobre un PDF de 908 MB; una página ya pintada, menos de 0,05 ms |
| Abrir un PDF de 1 GB hasta la primera página | `< 2 s` | 487 ms (177 ms abrir y 310 ms la primera página) con 39.440 páginas |
| Paginar un EPUB | `< 16 ms` | Cabe en el fotograma en paginado y continuo, con 300 y con 1.600 párrafos |
| Buscar mientras se escribe | `< 50 ms` | FTS5 en SQLite; 17 ms por letra en la barra con la biblioteca real dentro |
| Ordenar o filtrar la biblioteca | `< 16 ms` por página | 2–10 ms por orden; filtros por autor 7–8 ms sobre 152.080 ediciones |

## 2. Almacenamiento derivado

Es la regla que distingue a NB Books de un gestor que duplica la biblioteca: **lo derivado nunca pesa más que la
biblioteca que describe**.

* **Presupuesto:** 20 % de lo que ocupan los documentos, ampliable a 30 o 40 % en Ajustes
  (`settings::PRESUPUESTOS_ADMITIDOS`). Ninguna opción se acerca al 100 %, que es el innegociable.
* **Qué cuenta:** la base con su índice, las portadas, las cachés de página y el audio preparado. Los pesos de
  modelos no cuentan: pesan lo mismo con un libro que con ciento cincuenta mil.
* **Cómo se mide:** los documentos se cuentan por el catálogo, sin recorrer las carpetas del usuario. El resultado
  viaja en el diagnóstico (`DerivedStorage`) como cuatro números y ninguna ruta.
* **Cómo frena:** el indexador mide lo derivado cada veinte documentos mientras hay margen y en cada vuelta cuando
  no lo hay, y se detiene al alcanzar el presupuesto publicando `STORAGE_BUDGET_REACHED`. Parado por el tope,
  espera a que cambie algo (el tope, el espacio libre, un libro abierto) en lugar de sondear.
* **Qué se indexa primero:** lo empezado, lo que está en una colección, lo de los autores que se leen y el resto
  de lo más reciente a lo más antiguo. Lo que se abre se indexa aunque no quepa.
* **El audio preparado** cuenta lo encolado y lo que se está preparando, no solo lo escrito, y la interfaz no ofrece
  empezar algo que no cabe.

### 2.1 De dónde salen las decisiones

| Medida, sobre EPUB reales | Valor |
|---|---|
| Texto plano sobre el tamaño del archivo | 60,8 % agregado; p90 145,2 % |
| Portada a resolución original sobre el archivo | 20,7 % |
| Índice invertido, con independencia del tamaño de fragmento | 19–22 % |
| Coste por fila de fragmento (identificador y locator) | unos 300 bytes |

De ahí salen tres decisiones de arquitectura:

1. **El texto no se guarda en la base.** Guardarlo costaría el 61 % de la biblioteca antes del índice; el índice
   FTS5 es externo y se reconstruye desde el documento.
2. **Fragmentos de 1.920 caracteres.** El índice invertido depende del vocabulario y no del número de filas, así
   que el tamaño del fragmento solo amortiza el coste por fila. Más de 3.840 caracteres empeora, porque la fila
   desborda la página de SQLite.
3. **Solo miniaturas de portada**, de 400 px y unos 50 KB. Las originales que llegaron antes las convierte una
   pasada de fondo (`covers::compactar`) que no toca la base.

## 3. Fotogramas en el motor real

La suite de interfaz corre en Chromium y la aplicación en WebKitGTK, y no se comportan igual: el mismo
desplazamiento dio 17 ms por fotograma en uno y 40 en el otro. Por eso:

* `pnpm check:webkitgtk` (`scripts/recorrido-webkitgtk.py`) recorre Inicio, Biblioteca y el lector en WebKitGTK y
  **decide solo con lo determinista**: nodos, píxeles descodificados y capas. Se omite limpiamente donde no hay
  WebKitGTK ni servidor gráfico.
* Los intervalos entre fotogramas se registran sin decidir: dentro de una ventana GTK también los marca el
  compositor del escritorio.

| Recorrido, en WebKitGTK | Resultado |
|---|---|
| Desplazar Inicio | 0 fotogramas largos (> 20 ms) |
| Pasar de página en las filas de Inicio | 9–10 largos de 116–120, sin animación de entrada |
| Mover el cursor del lector por un capítulo de 400 párrafos | Mediana 18 ms, p95 19 ms |
| Abrir el mapa del libro con 1.600 resaltados | Como mucho 130 nodos; el cálculo no crece con lo anotado |
| Abrir un panel del lector con 1.600 párrafos | Los paneles flotan; la columna de texto no cambia de ancho |
| Pasar de página en la biblioteca, 50 portadas distintas | Mediana 112 ms, con la siguiente página adelantada |

Pasar de página en la biblioteca no cabe en un fotograma y se dice: rehacer el DOM cuesta unos 110 ms y
descodificar las portadas visibles el resto. Se adelantan solo las portadas que caben en pantalla de la página
siguiente; adelantar las cincuenta empeora la cola porque el motor desaloja mapas de bits.

### 3.1 Lo que el motor no reproduce

* **WebKitGTK no reproduce medios servidos por el esquema propio de la aplicación**: entrega el audio a GStreamer,
  que no conoce ese esquema. El audio se trae con `fetch` y se entrega como `blob:`. `scripts/check-ui.mjs`
  prohíbe cualquier `<audio>`, `<video>` o `new Audio` con una dirección del esquema propio.
* **Escribir en un campo de la barra superior** costaba 120 ms de disposición por letra en WebKitGTK y nada en
  Chromium. `contain: layout size` en la barra lo corta, y `check-ui.mjs` lo vigila.
* **Una animación permanente compite con el desplazamiento**: el fondo animado de la carcasa costaba 13
  fotogramas largos de 77, y sin él 3 de 87. Nada se anima con la aplicación en reposo, y el cromo permanente
  sobre lo que se desplaza es opaco ([UI conventions](../design/ui-conventions.md), sección 12).

## 4. Consultas que no crecen con la biblioteca

Lo que decide si una consulta escala no es el reloj sino su plan. Las pruebas de `recorrido_biblioteca.rs` leen el
plan de cada orden y filtro y fallan si aparece un recorrido completo de `editions`, una subconsulta correlacionada
o un árbol temporal para ordenar.

| Técnica | Dónde | Efecto medido sobre 152.080 ediciones |
|---|---|---|
| Página decidida antes de rellenarse | `browse.rs` | De 45 a 2–10 ms por página |
| Clave de autor materializada con disparadores | V19 | De 26,9 a 2,1 ms; 0,37 ms por autoría al escribir |
| Recuentos de estanterías materializados | V24 | Panel de 55.740 autores de 4.141 a 6 ms |
| Índices de filtro | V29 | Filtro por autor de 905 a 7 ms |
| Título comparable para duplicados | V39 | Recuento de duplicados en unos 105 ms, en segundo plano |

Toda transacción que escribe se abre con `BEGIN IMMEDIATE` y ningún comando del puente corre en el hilo de la
ventana; ver [storage](storage.md) e [IPC contracts](ipc-contracts.md).

## 5. Arranque

* La ventana se abre antes de cualquier trabajo largo: `bootstrap` prepara rutas, registro, base y componentes, y
  lo que crece con la biblioteca (comprobar el índice, averiguar qué falta por indexar) corre después en su
  propio hilo con la ventana ya abierta.
* La comprobación de integridad del índice no corre en cada arranque: se ofrece como reconstrucción a petición.
* Una migración copia la base antes de aplicarse; con bases de decenas de gigabytes esa copia domina el tiempo de
  migrar, por eso el respaldo excluye lo que se puede reconstruir y se conserva solo el último.

## 6. Inteligencia artificial

| Medida | Valor |
|---|---|
| Lo que añade Candle al binario con el modelo cargado de verdad | Incluido en los 29,2 MiB del binario |
| Inferencia de embeddings contra el fotograma | No degrada el fotograma: corre en su grupo de hilos con prioridad baja y se aparta mientras el lector pinta |
| Preparar un libro para búsqueda semántica | 263 fragmentos de media, unos 398 ms por fragmento con el presupuesto de fábrica |
| Generación local | Qwen2.5 1.5B: 1,98 GB en memoria, 14 tokens/s de lectura y 4,4 de escritura en un Ryzen 5 3550H |

La aplicación mide el equipo, recomienda el modelo que cabe y muestra el coste estimado antes de aceptar; todos
los modelos se pueden elegir. Ver [AI: modelos locales](ai/local-models.md).

## 7. Exportación de la biblioteca

| Paso, 150.000 documentos | Tiempo (perfil de depuración) |
|---|---|
| Exportar y serializar | 12,3 s |
| Releer e importar en una base vacía | 47,8 s |
| Tamaño del archivo | 177,2 MB, 1.238 bytes por documento |

La comprobación de que no se pierde nada compara el conjunto de huellas de contenido antes y después, no los
recuentos.

## 8. Disciplina de medida

* **Un banco que colapsa la dimensión medida convierte cualquier medida en una confirmación.** Antes de medir se
  comprueba que el banco permite que la medida salga mal: portadas distintas, rutas distintas, longitudes
  distintas. Los corpus reales se usan a través de `NB_BOOKS_CORPUS` y las pruebas que los necesitan se omiten sin
  ellos.
* **Mínimo de varias repeticiones** para las pruebas de tiempo dentro de la suite, porque la suite corre en paralelo
  y una medida suelta mide también la contención.
* **Caché fría y caché caliente** se distinguen: una consulta que tarda 722 ms en caliente puede tardar cinco
  minutos en frío sobre una base grande.
* **Una pasada de control** resta el coste de lo que no se está midiendo, como el de la propia inserción al medir
  un disparador.
