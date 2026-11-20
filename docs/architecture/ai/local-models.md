# Modelos locales

> **Estado:** Implementado. Motor y detección de hardware en `src-tauri/src/ai/local.rs` y `ai/hardware.rs`;
> catálogo, instalación y carga en `ai/catalogo.rs`, `ai/instalacion.rs`, `ai/modelo.rs` y `ai/subsistema.rs`;
> presupuesto en `ai/presupuesto.rs`; generación en `ai/generativo.rs`; voz neuronal en `src-tauri/src/voice/`.

## 1. Motor de inferencia

**Candle**, el motor de Hugging Face, en **Rust puro**. Ningún componente lo instancia directamente: queda detrás
de `AIServiceContract`.

**Por qué no `llama.cpp` ni ONNX Runtime.** Un motor de C++ por FFI, con backends acelerados distintos por
plataforma y por GPU, multiplica por tres sistemas la clase de defecto que más cuesta en una aplicación de
escritorio: un binario que no compila en una plataforma o un motor nativo que no viaja en el instalador. Candle se
compila en el mismo `cargo build` que el resto del runtime y esa clase desaparece. A cambio, en generación
cuantizada es más lento que `llama.cpp`; por eso la generación local se mide y se ofrece con su coste delante
(sección 6).

### 1.1 Aceleración por hardware

* GPU de NVIDIA: CUDA, si la compilación la trae **y** la máquina responde.
* Apple Silicon: Metal, con la misma condición.
* Universal: el procesador, con **la mitad de los núcleos** de fábrica y nunca todos, para no competir con el hilo
  que pinta (sección 5).

**No se decide leyendo qué opciones se compilaron, sino abriendo el dispositivo.** Una opción compilada dice que
el código está en el binario, no que haya tarjeta ni controlador. Además, al arrancar se multiplica un par de
matrices de resultado conocido, en microsegundos y sin pesos: compilar no es calcular, y la distinción entre
"compila" y "funciona en la máquina de quien lo instaló" es la que importa.

## 2. Gestión de pesos

Los pesos no entran en el binario ni en el repositorio. **Solo se admite `safetensors` y GGUF**, más los `json` de
configuración y tokenizador: `pytorch_model.bin` es un `pickle` que puede ejecutar código al abrirse.

* Se descargan a demanda en `<datos>/modelos/<id>/`, en datos y no en caché: limpiar la caché no debe obligar a
  descargarlos otra vez, y un modelo no cuenta contra el presupuesto de lo derivado.
* Cada descarga verifica **la huella compilada en el binario**, no una publicada junto al archivo.
* Todo se descarga dentro de `.<id>.parcial`; cada archivo se compara con su tamaño y su huella **según se
  escribe**, y solo cuando coinciden todos la carpeta se renombra a `<id>`. La carpeta de un modelo **solo puede
  existir verificada**. Un servidor que manda de más se corta al pasarse.
* Se comprueba el espacio libre antes de empezar, con un margen de una vigésima parte.
* Al arrancar se comprueba el **tamaño** de cada archivo, no la huella; un archivo alterado con el mismo tamaño lo
  detecta la carga.
* Una descarga cortada no se reanuda: reanudar sobre bytes sin verificar es justo lo que esta disposición evita.
* Si la descarga redirige a una red de distribución, ese anfitrión pide su propio permiso.
* El modelo se carga al pedirlo y se suelta tras un periodo sin uso (`ai::modelo::Residente`): cinco minutos el de
  embeddings, tres los generativos. El hilo que vigila la inactividad solo existe mientras hay modelo cargado.
* Al soltarlo se llama a `malloc_trim`: sin eso el asignador de glibc se queda la memoria liberada.
* En Ajustes, sección "IA y modelos", se ve el espacio ocupado y se elimina cada modelo.

### 2.1 Requisitos declarados

Cada modelo del catálogo declara memoria mínima, tamaño en disco, licencia, idiomas, lo que ocupa cargado y lo que
añade cada texto de un lote. Las dos últimas cifras se **miden**, no se toman de la ficha del autor.

## 3. Licencias de modelos

Los pesos tienen licencia propia, distinta de la del código. **Ningún modelo entra al catálogo sin una revisión de
licencia registrada** en [model-reviews.md](../../legal/model-reviews.md), y la prueba
`todo_modelo_del_catalogo_tiene_su_revision_registrada` exige que cada entrada tenga ahí su sección con **la misma
licencia, la misma revisión del repositorio y la misma huella de cada archivo**. La interfaz muestra la licencia
antes de descargar.

| Modelo | Uso | Licencia |
|---|---|---|
| `granite-embedding-107m-multilingual` (IBM) | Embeddings, 384 dimensiones, 12 idiomas, 214 MB | Apache-2.0 |
| `qwen25-15b-instruct-q4km` (Alibaba Cloud) | Generación | Apache-2.0 |
| `qwen3-17b-q8` (Alibaba Cloud) | Generación | Apache-2.0 |
| `qwen3-4b-q4km` (Alibaba Cloud) | Generación | Apache-2.0 |
| `kokoro-82m` (hexgrad) | Voz neuronal | Apache-2.0 |

## 4. Degradación

El servicio de IA se autoevalúa al arrancar, y el orden de las preguntas importa: lo que no tiene arreglo va antes
que lo que sí, para no invitar a descargar gigabytes a quien no podrá ejecutarlos.

| Situación | Estado | Qué ve el usuario |
|---|---|---|
| El motor no puede ejecutarse | `NOT_SUPPORTED` | Las funciones se ocultan |
| Ningún modelo descargado | `STOPPED` | Las funciones se ocultan; la invitación vive en Ajustes, sin aviso de atención |
| Un modelo instalado ya no está entero | `NOT_INSTALLED` | Se anuncia, porque alguien lo eligió y ahora falla |
| El modelo no cabe en el tope de memoria | `STOPPED` con `AI_MEMORY_LIMIT` | Ajustes ofrece subir el tope al necesario |
| Fallo durante la inferencia | `DEGRADED` | Se informa y se permite reintentar o cambiar de proveedor |

**Un equipo justo no esconde nada: se recomienda, no se obliga.** Ajustes ofrece todos los modelos, marca uno como
recomendado con su motivo y dice en cada uno lo que es probable que pase en este equipo (`catalogo::prevision`):
minutos por función, si la edición es lenta, si pide más memoria de la que hay. Un modelo que pide más memoria que
la del equipo se ofrece con confirmación fuerte.

El suelo del motor son 2 GiB de memoria física (`hardware::MEMORIA_MINIMA`). Si no se puede leer cuánta memoria hay,
se responde que sí la hay: una negativa ocultaría funciones sin decir por qué, y el error contrario se ve como
`DEGRADED`.

**Instalar desde la interfaz.** Cada modelo es una ficha con licencia, autor, descarga, memoria e idiomas siempre a
la vista. Pulsar "Descargar" enseña antes de quién es, bajo qué licencia, de qué anfitrión se trae y qué sale del
equipo, y solo entonces ofrece "Autorizar y descargar". Un recorrido de interfaz comprueba que ni el permiso ni la
descarga se piden al runtime antes de confirmar.

En ningún caso un fallo del motor de IA afecta a la lectura.

## 5. Presupuesto de recursos

El criterio es que **la inferencia nunca degrada el presupuesto de fotograma de la lectura**. Se cumple con cuatro
mecanismos, porque ninguno basta solo:

1. **Un grupo de hilos propio, con tope.** Candle reparte su trabajo con `rayon`; la inferencia, la carga y el
   tokenizado corren dentro de un grupo propio, con la mitad de los núcleos de fábrica. Cada `cargo test` comprueba
   cuánto procesador gastaron los hilos `nb-ia-`.
2. **Prioridad baja.** Los hilos del grupo bajan a `nice` 10 al nacer, también frente al proceso de WebKit (Linux).
3. **Un tope de memoria** que decide si el modelo se carga y de qué tamaño son los lotes: el lote es lo que cabe
   entre el modelo y el tope. El tope cuenta memoria anónima, no residente. Con el tope de fábrica, 1 GiB, el lote de
   embeddings es de nueve textos.
4. **El trabajo de fondo se aparta mientras el lector pinta.** El lector avisa al desplazar o pasar página, y la
   generación de vectores no empieza un lote hasta que lleva un segundo y medio quieto.

Los topes se configuran en Ajustes y se aplican **sin reiniciar**; los límites los hace cumplir el runtime al
guardar: hilos de automático a los núcleos menos uno, y memoria de 512 MB, 1, 2 o 4 GB.

**Suspensión del trabajo de fondo.** Con batería por debajo del 20 % o una zona térmica a menos de cinco grados de
su umbral pasivo (leído de `/sys/class/thermal`), `AiService::pausa_de_fondo` detiene el trabajo de fondo del
proveedor local y dice por qué. Lo interactivo no se suspende.

Las cifras están en [performance-budgets](../performance-budgets.md), sección 6, y la prueba es
`la_inferencia_dentro_del_presupuesto_no_degrada_el_fotograma` (con `NB_BOOKS_MODELO_REAL`).

## 6. Generación local

Las funciones de estudio se ofrecen **con el modelo local y con un proveedor externo, y elige la persona**
(preferencia `ai_generator`).

### 6.1 Varios modelos, y la máquina recomienda

| Modelo | Descarga | Memoria cargado | Se recomienda desde |
|---|---|---|---|
| Qwen2.5-1.5B Q4_K_M | 1,12 GB | 1,98 GB | 6 GB de memoria y 4 núcleos |
| Qwen3-1.7B Q8_0 | 1,83 GB | 3,59 GB | 8 GB y 6 núcleos |
| Qwen3-4B Q4_K_M | 2,50 GB | 4,68 GB | 12 GB y 8 núcleos |

La recomendación es **el mayor que el equipo aguanta**, no el mejor a secas. Sin memoria legible se recomienda el
más pequeño. Se usa uno a la vez, y elegir otro suelta el que estuviera cargado. La preferencia es
`ai_model_local`; vacía significa "el recomendado para este equipo".

### 6.2 Cómo se acota

* **Entrada:** 6.000 caracteres, unos 1.700 tokens. Lo que no cabe se reparte por todo el capítulo
  (`estudio::repartir`) y la respuesta lo dice.
* **Salida:** 900 tokens.
* **Determinista:** se elige siempre el token más probable, para poder evaluar una plantilla y repetir un fallo.
* **Se cancela** entre tramos de la lectura y entre tokens de la escritura.
* **Una generación a la vez.**
* En el mismo grupo de hilos, con la misma prioridad y el mismo tope de la sección 5.

### 6.3 AVX2 y las dos ediciones

Los núcleos cuantizados de Candle eligen su versión vectorizada **al compilar**, y la compilación por defecto para
x86-64 no activa AVX2: sin ella, un resumen corto tarda unos 20 minutos; con ella, unos 2.

* **Cada paquete lleva las dos ediciones**: la universal (`nb-books`) y la rápida (`nb-books-avx2`), compilada
  con `x86-64-v3` y **solo para procesadores compatibles con AVX2** (Intel desde 2013, AMD desde 2015). Al
  arrancar, la universal cede el proceso a la rápida si el procesador la admite (`main.rs`), así que no hay
  nada que elegir al descargar. Si aun así corre la universal en un procesador con AVX2, la aplicación lo dice
  (`SLOW_BUILD`).
* **En equipos menos potentes, la IA trabajando de fondo puede ralentizar la aplicación.** Se aparta mientras el
  lector pinta (se detiene al desplazarse o pasar página y espera a que la lectura lleve un momento quieta), pero
  en un procesador con pocos núcleos deja fotogramas largos ocasionales; Ajustes, en IA y Modelos, lo advierte.
* Exigir AVX2 en el único binario haría que la aplicación entera muriera con instrucción ilegal en procesadores
  anteriores, **al abrir un libro**, que no usa la IA.
* Sin AVX2 la generación se ofrece igual, marcada como lenta: la espera la decide quien espera.
* En ARM de 64 bits NEON es parte de la arquitectura y no hay edición lenta.
* `pnpm tauri dev` compila con `x86-64-v3` si el procesador lo admite, para que lo que se prueba en desarrollo vaya
  a la velocidad de la edición rápida.

### 6.4 Preguntas de autoevaluación

Con el modelo local, las preguntas de opción múltiple salen a veces con las opciones repetidas o la clave en la
opción equivocada (`generativo::DESACONSEJADAS`). Se ofrecen marcadas como **no recomendadas**, con lo medido
delante, porque una pregunta con la clave corrida hace memorizar un error.

## 7. Voz neuronal local

### 7.1 Motor

Kokoro 82M, adaptado en `src-tauri/src/voice/kokoro/` a partir de [`any-tts`](https://github.com/TM9657/any-tts)
(MIT o Apache-2.0); la procedencia, qué se tomó y qué no, está en `PROCEDENCIA.md` junto al código. **No se tomó la
clonación de voz.**

* El modelo está en el catálogo de voz (`voice::catalogo`) con su revisión fijada, la huella de cada archivo y las
  tres voces de español. Se instala por la misma puerta verificada que los modelos de IA (rasgo `Instalable`).
* El proveedor (`voice::neuronal`) carga al pedirlo, suelta a los dos minutos sin uso y comprueba el tope de memoria
  antes de cargar. Ocupa unos 585 MiB con el modelo dentro.
* Los pesos de Kokoro son *pickles* de Python y entran porque **no los abre Python**: los lee el lector restringido
  de Candle y las voces, el lector de ZIP del proyecto. `tests/voz_neuronal.rs` comprueba que un pickle con una
  llamada dentro no la ejecuta.

### 7.2 De texto a fonemas

`voice::fonemas`, escrito en Rust con la licencia del proyecto. `espeak-ng` es GPL-3.0 y no se enlaza ni se
distribuye: se usa solo para generar las listas de referencia contra las que se mide.

| Idioma | Método | Medida contra la referencia |
|---|---|---|
| Español de América | Reglas | 153 de 155 palabras idénticas (99 %) |
| Portugués de Brasil | Reglas | 89 % idénticas; 97 % sin contar el timbre de la vocal tónica, que decide el léxico |
| Inglés | Diccionario (`pronunciaciones.txt`, viaja con el modelo) | Lo que no está en el diccionario se aproxima y se avisa antes de hablar |

El conversor de español encadena palabras, no acentúa las átonas, dice los números y pronuncia la jota; el catálogo
solo ofrece voces de los idiomas cuyo conversor existe, y una prueba lo comprueba.

### 7.3 Rendimiento y cómo se ofrece

El proveedor **mide la máquina al cargar**: sintetiza una frase corta y calcula su factor de tiempo real. En un
Ryzen 5 3550H el factor es 1,67 (tarda 1,67 veces lo que dura el audio), así que allí no narra en directo sin
cortes; en procesadores más rápidos baja de uno.

* Por encima de `FACTOR_MAXIMO` (0,8) la voz se ofrece igual para narrar en vivo, con el aviso de que se cortará y
  el factor medido, y se recomienda **preparar** el capítulo o el libro.
* El ajuste de voz tiene tres opciones: la del sistema (Speech Dispatcher, de fábrica en Linux), la neuronal local
  y la de un servicio (`/v1/audio/speech`, que también puede ser un servidor con GPU en la propia red).
* La síntesis trocea en partes de unos 400 caracteres por donde pararía quien lee en voz alta.
* `sintetizar_a_archivo` devuelve una ruta, con caché por voz, velocidad y texto: así adelantar el tramo siguiente
  no sintetiza dos veces y el audio no cruza el puente en base64.

### 7.4 Preparar antes de escuchar

Lo que hace usable la voz en una máquina que no narra en directo: se paga el tiempo una vez, con el coste delante y
pudiendo cancelar.

| Pieza | Qué hace |
|---|---|
| `voice::audio` | Ogg Opus a 24 kbit/s **según va llegando el audio**: unos 11 MB por hora frente a 173 MB en crudo |
| `voice::preparacion` | Cola, cancelación, marcas por fragmento y lo que se guarda |
| Migración V35 | `audio_sections` y `audio_marks` |
| `lector/AudioPanel.svelte` | El panel, con el coste antes y el tiempo restante durante |

* Se guarda con qué voz y qué revisión del modelo se hizo, cuánto ocupa (cuenta contra el presupuesto de lo
  derivado) y las marcas por `Locator` que siguen el texto mientras suena.
* El coste estimado sale de los caracteres (unos 17 por segundo de audio) y del factor medido en esta máquina; sin
  factor medido se dice cuánto durará y no cuánto tardará. En marcha, el tiempo restante sale del ritmo real y no se
  muestra hasta el tercer fragmento.
* Cancelar deja lo hecho, marcado como parcial y escuchable. Preparar otra vez sustituye; olvidarlo borra la fila,
  las marcas y el archivo.
* Antes de decidir se puede escuchar una muestra del propio libro.
