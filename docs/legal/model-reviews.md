# Registro de Revisiones de Modelos

> **Estado:** Vigente. Una revisión por modelo del catálogo.
> **Aviso:** este documento recoge criterios de ingeniería y gestión de riesgo. No constituye asesoría legal.

Los pesos de un modelo tienen licencia propia, distinta de la del código, y algunas restringen el uso comercial o
imponen condiciones de uso aceptable. [licensing.md](licensing.md) sección 5 lo recoge, y
[local-models.md](../architecture/ai/local-models.md) sección 3 fija la regla: **ningún modelo entra al catálogo
sin una revisión de licencia registrada**. Este es el registro.

## 1. Por qué es un registro y no una lista de nombres

Porque **no se revisa un nombre, se revisan unos bytes**. Un repositorio de modelos cambia: se vuelve a subir un
archivo, se corrige la ficha, cambia la licencia. Lo que se revisó el día de la revisión es una revisión concreta
del repositorio y unos archivos con su huella, y eso es lo que queda registrado.

**El mecanismo.** Cada entrada del catálogo —`src-tauri/src/ai/catalogo.rs`— tiene que encontrar aquí una sección
con su identificador, y dentro de ella la misma licencia, la misma revisión y la misma huella de cada archivo.
La prueba `todo_modelo_del_catalogo_tiene_su_revision_registrada` lo comprueba en `cargo test`. Si alguien añade
un modelo al catálogo sin revisarlo, o cambia la revisión o un archivo sin volver a revisar, la verificación falla.
**Lo que el mecanismo no puede comprobar**, y por eso se dice: que la revisión de abajo sea correcta. Comprueba
que existe y que habla de los mismos bytes; el juicio lo firma una persona.

## 2. Qué se revisa de cada modelo

| Qué | Por qué |
|---|---|
| **Licencia de los pesos** y su compatibilidad con Apache-2.0 | El núcleo es Apache-2.0 ([ADR-003](../adr/003-licensing-model.md)) |
| **Uso comercial** | Cualquiera puede usar y redistribuir la aplicación con fines comerciales, así que sus modelos también tienen que permitirlo |
| **Condiciones de uso aceptable** | Algunas licencias de modelos las imponen por encima de la licencia; hay que saber si existen |
| **Procedencia declarada de los datos de entrenamiento** | Un modelo entrenado sobre datos con restricciones puede heredarlas, aunque sus pesos digan otra cosa |
| **Obligaciones de atribución** | Qué hay que enseñar, y dónde |
| **Dónde se descarga y cómo se verifica** | Ver [plugin-system.md](../architecture/plugin-system.md) sección 6.3 |
| **Qué no hace bien** | Idiomas que no cubre, longitudes que trunca. Lo que se acepta tiene que estar escrito |

## 2.1 Y qué se revisa además de un modelo de **voz** 

Un modelo de voz no es un modelo más. La licencia de los pesos dice qué se puede hacer con el archivo; **no dice
nada de la persona cuya voz suena dentro**. Sin un criterio escrito no entra ninguna voz; con él entran las que cumplan.

Una voz entra al catálogo **solo si cumple las cinco**. No son graduales: la que falle una, no entra.

| Requisito | Por qué, y cómo se comprueba |
|---|---|
| **1. Licencia permisiva de los pesos**, compatible con Apache-2.0 | Lo mismo que cualquier modelo (sección 2). Se lee de la ficha y del repositorio, y se fija la revisión |
| **2. Datos de entrenamiento declarados**, con su licencia o su condición de dominio público | Es la diferencia entre "se puede usar el archivo" y "se sabe de dónde salió la voz". La declaración se cita **literal** en la revisión; si el autor no la publica, el modelo no entra, aunque su licencia sea permisiva |
| **3. Sin clonación de la voz de una persona** a partir de una muestra | Un modelo que clona convierte al producto en una herramienta para suplantar a alguien con tres segundos de audio. Es una capacidad que NB Books no necesita —lo que se pide es leer un libro en voz alta— y cuyo daño no se puede contener desde aquí |
| **4. Voces que no imiten a una persona identificable** | Voces sintéticas o de locutores que cedieron su grabación con ese fin. Una voz "que suena como" alguien reconocible involucra sus derechos aunque el modelo sea libre |
| **5. La interfaz dice que la voz es sintética** | Regla 4 de [ai/philosophy.md](../architecture/ai/philosophy.md) aplicada al oído: quien escucha tiene que saber que no hay una persona leyendo. No es una advertencia legal, es lo mismo que marcar una salida generada |

**Lo que este criterio deja fuera hoy, dicho con nombres.** `MOSS-TTS` y su variante Nano (Apache-2.0, 0,1 B, texto
directo y veinte idiomas, que técnicamente sería la mejor candidata) **no entra**: su ficha no declara datos de
entrenamiento y su función principal es clonar la voz de un hablante desde una muestra corta. Falla la 2 y la 3.

**Lo que deja dentro.** `hexgrad/Kokoro-82M` (Apache-2.0) declara haber sido entrenada **solo** con audio permisivo
o de dominio público y con audio sintético, sin clones de voz de encargo, y cita las dos fuentes CC-BY que usa.
Cumple las cinco, y es la candidata.

**Lo que este criterio no dice, y por eso se escribe aquí:** no es asesoría legal y no sustituye a una. Es el
listón de ingeniería que este proyecto se pone para no firmar lo que no puede sostener.

---

## `granite-embedding-107m-multilingual`

**Revisado el 2026-09-16. Decidido ese mismo día**, frente a las alternativas de la sección 3.

| Campo | Valor |
|---|---|
| Identificador en el catálogo | `granite-embedding-107m-multilingual` |
| Autor | IBM, equipo Granite Embedding |
| Repositorio | `ibm-granite/granite-embedding-107m-multilingual`, en Hugging Face |
| Revisión fijada | `d6cffd338414d6a1c1f5decfad5fec62eebc90d5` |
| Licencia | `Apache-2.0` |
| Capacidad | Embeddings, 384 dimensiones |
| Arquitectura | XLM-RoBERTa, 6 capas, 12 cabezas, 107 millones de parámetros, pesos en `bfloat16` |
| Agrupación | El token CLS, según `1_Pooling/config.json` de la misma revisión |
| Longitud máxima | 512 tokens |
| Idiomas declarados | Inglés, alemán, español, francés, japonés, portugués, árabe, checo, italiano, coreano, neerlandés y chino |

### Archivos, con su huella

Solo los que hacen falta para ejecutarlo. El repositorio trae además `model.onnx` (428 MB) y
`pytorch_model.bin` (770 MB), que **no se descargan**: son el mismo modelo en otros formatos, y `pytorch_model.bin`
es además un formato que puede ejecutar código al abrirse, que es justo lo que `safetensors` existe para evitar.

| Archivo | Bytes | SHA-256 |
|---|---|---|
| `model.safetensors` | 214000232 | `ae6f20cc5dc433041118c900b652d87532e1335dade78c37765f0b94d1161283` |
| `tokenizer.json` | 9081382 | `7fc9e475f2ac473f070052d92b0c46ec2b91d9cd672935feb6fd837a65d33f78` |
| `config.json` | 697 | `aa3b6a5d1d6efeacdb844fb8ba773556463af2195003ad5e70a6f014d11be9cf` |

**Las tres huellas se calcularon aquí el 2026-09-16**, descargando cada archivo de la revisión fijada. La de
`model.safetensors` coincide además con la que Hugging Face publica para el objeto grande; las de `tokenizer.json`
y `config.json` no las publica, porque viajan como archivos corrientes del repositorio, así que para esas dos la
única comprobación es la propia.

### La revisión

**Licencia.** Apache-2.0, declarada en los metadatos de la ficha del modelo y en su texto ("**License:** Apache
2.0"). **El repositorio no trae un archivo `LICENSE`**: la licencia consta solo en la ficha. Se registra porque
es más débil que un archivo de licencia en el repositorio, y porque si la ficha cambiara, la revisión fijada
seguiría siendo la que se revisó. Apache-2.0 es la misma licencia del núcleo: compatible sin matices.

**Uso comercial.** Permitido. Apache-2.0 no distingue usos.

**Condiciones de uso aceptable.** Ninguna por encima de la licencia. No hay política de uso aceptable adjunta, a
diferencia de otros modelos que la imponen como condición.

**Procedencia de los datos.** La ficha declara cuatro fuentes: pares título-cuerpo extraídos de la web, conjuntos
públicos "con licencia permisiva y apta para empresas", datos internos de IBM y datos sintéticos generados por
IBM. Es la declaración más explícita de las tres alternativas revisadas, y la única que habla de licencia de los
datos. **Lo que no se puede verificar desde aquí:** que esa declaración sea exacta. Se registra como declaración
del autor, no como hecho comprobado.

**Atribución.** Apache-2.0 obliga a conservar avisos al **redistribuir**. NB Books no redistribuye estos pesos: los
descarga quien usa la aplicación, desde el repositorio del autor. Aun así, la sección de modelos de Ajustes
enseña el nombre, el autor y la licencia **antes** de descargar, que es además el criterio.

**Dónde se descarga.** De Hugging Face, a la revisión fijada, con la huella de cada archivo **compilada en el
binario**. Quien da fe de los bytes es el binario firmado; el anfitrión solo los transporta, y un archivo que no
coincide con su huella se tira. La sección 6.3 de
[plugin-system.md](../architecture/plugin-system.md) decía "desde el proyecto, no de un tercero", y se
enmendó el 2026-09-16 para los pesos: son datos, no código, y su procedencia la garantiza la huella y no quien
los sirve. La descarga pasa por `huggingface.co` y por la red de distribución a la que redirige; los dos
anfitriones necesitan consentimiento.

**Qué no hace bien, y se acepta.**

* **Doce idiomas y no noventa y cuatro.** Un libro en catalán, gallego o euskera —plausibles en una biblioteca en
  español— se vectoriza con un modelo que no se entrenó para ellos, y sus resultados serán peores.
* **Unos tres puntos menos de calidad** que `multilingual-e5-small`: 55,9 frente a 59,3 en MIRACL y 48,7 frente a
  51,0 en MTEB en español, según el propio informe técnico de IBM.
* **Trunca a 512 tokens.** Los fragmentos del índice, fijados, tienen que caber; se comprueba.

## `qwen25-15b-instruct-q4km`

**Revisado el 2026-09-17 y admitido en el catálogo.**

| Campo | Valor |
|---|---|
| Identificador en el catálogo | `qwen25-15b-instruct-q4km` |
| Autor | Qwen, Alibaba Cloud |
| Repositorio | `Qwen/Qwen2.5-1.5B-Instruct-GGUF`, en Hugging Face |
| Revisión fijada | `91cad51170dc346986eccefdc2dd33a9da36ead9` |
| Licencia | `Apache-2.0` |
| Capacidad | Generación: resúmenes, tarjetas, preguntas y conceptos |
| Arquitectura | Qwen2, 28 capas, 1.540 millones de parámetros, cuantizado en Q4_K_M por su autor |
| Ventana | 32.768 tokens; el producto usa como mucho unos 1.700 de entrada y 900 de salida |
| Idiomas declarados | Más de 29, entre ellos chino, inglés, francés, español, portugués, alemán, italiano, ruso, japonés, coreano, vietnamita, tailandés y árabe |

### Archivos, con su origen y su huella

Los pesos y el tokenizador están en dos repositorios del mismo autor: el cuantizado no trae tokenizador.

| Repositorio | Revisión | Ruta |
|---|---|---|
| `Qwen/Qwen2.5-1.5B-Instruct-GGUF` | `91cad51170dc346986eccefdc2dd33a9da36ead9` | `qwen2.5-1.5b-instruct-q4_k_m.gguf` |
| `Qwen/Qwen2.5-1.5B-Instruct` | `989aa7980e4cf806f80c7fef2b1adb7bc71aa306` | `tokenizer.json` |

| Archivo | Bytes | SHA-256 |
|---|---|---|
| `model.gguf` | 1117320736 | `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e` |
| `tokenizer.json` | 7031645 | `c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539` |

Las dos huellas se calcularon aquí el 2026-09-17 sobre lo descargado de las revisiones fijadas. La del `.gguf`
coincide con la que Hugging Face publica para el objeto grande; la de `tokenizer.json` no la publica, porque viaja
como archivo corriente, y su tamaño coincide con el de la revisión.

### La revisión

**Licencia.** Apache-2.0, en los metadatos de la ficha **y en un archivo `LICENSE`** en los dos repositorios, con
el texto íntegro de la licencia. Es más fuerte que la de Granite, que solo la declaraba en la ficha. Compatible con
el núcleo sin matices. **Se comprobó que no es la licencia "Qwen"** que llevan otros tamaños de la familia: el
Qwen2.5-3B se descartó justamente por eso (sección 3.1).

**Uso comercial.** Permitido.

**Condiciones de uso aceptable.** Ninguna por encima de la licencia en esta revisión.

**Procedencia de los datos.** La ficha no la declara: remite al informe técnico de Qwen2.5, que describe un corpus
de 18 billones de tokens de fuentes web, código, matemáticas y datos sintéticos **sin detallar licencias**. Es la
declaración más débil de las revisadas hasta ahora, y se registra así.

**Atribución.** No se redistribuyen los pesos; Ajustes enseña nombre, autor y licencia antes de descargar.

**Dónde se descarga.** De Hugging Face, a las revisiones fijadas, con las huellas compiladas en el binario.

**Qué no hace bien, y se acepta si se aprueba.**

* **Es un modelo pequeño.** En la medición del 2026-09-17 resumió bien un texto de historia en español, pero se
  dejó una llave de más al cerrar el JSON de tarjetas, escribió la cita en singular (`"cita"`) y en el resumen en
  prosa no citó nada. La lectura de salidas rescata las dos primeras; lo tercero es lo que la marca "no
  verificable" existe para enseñar.
* **Ocupa 1,9 GB cargado**, más del doble de lo que pesa: Candle convierte la tabla de embeddings a coma flotante.
  No cabe en el tope de memoria de fábrica (1 GB); hay que subirlo a 2 GB en Ajustes, y Ajustes lo dice.
* **Solo es usable con AVX2 compilado**: ver `local-models.md` sección 6.

## `qwen3-17b-q8`

**Revisado el 2026-09-17 y admitido en el catálogo.** El catálogo ofrece varios modelos generativos y **quien
tiene la máquina elige**, con la aplicación recomendando uno según el equipo.

| Campo | Valor |
|---|---|
| Identificador en el catálogo | `qwen3-17b-q8` |
| Autor | Qwen, Alibaba Cloud |
| Repositorio | `Qwen/Qwen3-1.7B-GGUF`, en Hugging Face |
| Revisión fijada | `90862c4b9d2787eaed51d12237eafdfe7c5f6077` |
| Licencia | `Apache-2.0` |
| Archivo de licencia | Sí, `LICENSE` en los dos repositorios |
| Capacidad | Generación: resúmenes, tarjetas y conceptos |
| Arquitectura | Qwen3, 1.700 millones de parámetros, cuantizado en Q8_0 por su autor |
| Se recomienda desde | 8 GB de memoria y 6 núcleos |

| Repositorio | Revisión | Ruta |
|---|---|---|
| `Qwen/Qwen3-1.7B-GGUF` | `90862c4b9d2787eaed51d12237eafdfe7c5f6077` | `Qwen3-1.7B-Q8_0.gguf` |
| `Qwen/Qwen3-1.7B` | `70d244cc86ccca08cf5af4e1e306ecf908b1ad5e` | `tokenizer.json` |

| Archivo | Bytes | SHA-256 |
|---|---|---|
| `model.gguf` | 1834426016 | `061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a` |
| `tokenizer.json` | 11422654 | `aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4` |

Las dos huellas se calcularon aquí el 2026-09-17 sobre lo descargado de las revisiones fijadas, y coinciden con las
que publica Hugging Face para los dos objetos grandes.

**Licencia, uso comercial y condiciones:** iguales que `qwen25-15b-instruct-q4km`, y con `LICENSE` en el
repositorio. **Procedencia de los datos:** la ficha no la declara; remite al informe técnico de Qwen3. Se registra
como la declaración más débil, igual que en el otro.

**Qué no hace bien, y se acepta si se aprueba.** Razona antes de contestar y lo escribe: en una función de estudio
eso son cientos de tokens que nadie lee, así que se le abre el turno con el bloque de pensamiento ya cerrado. Ocupa
3,6 GB cargado, así que **no cabe en el tope de memoria de fábrica** ni en el de 2 GB: hay que subirlo a 4 GB.

## `qwen3-4b-q4km`

**Revisado el 2026-09-17.**

| Campo | Valor |
|---|---|
| Identificador en el catálogo | `qwen3-4b-q4km` |
| Autor | Qwen, Alibaba Cloud |
| Repositorio | `Qwen/Qwen3-4B-GGUF`, en Hugging Face |
| Revisión fijada | `bc640142c66e1fdd12af0bd68f40445458f3869b` |
| Licencia | `Apache-2.0` |
| Archivo de licencia | Sí, `LICENSE` en los dos repositorios |
| Capacidad | Generación: resúmenes, tarjetas y conceptos |
| Arquitectura | Qwen3, 4.000 millones de parámetros, cuantizado en Q4_K_M por su autor |
| Se recomienda desde | 12 GB de memoria y 8 núcleos |

| Repositorio | Revisión | Ruta |
|---|---|---|
| `Qwen/Qwen3-4B-GGUF` | `bc640142c66e1fdd12af0bd68f40445458f3869b` | `Qwen3-4B-Q4_K_M.gguf` |
| `Qwen/Qwen3-4B` | `1cfa9a7208912126459214e8b04321603b3df60c` | `tokenizer.json` |

| Archivo | Bytes | SHA-256 |
|---|---|---|
| `model.gguf` | 2497280256 | `7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5` |
| `tokenizer.json` | 11422654 | `aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4` |

**Licencia, uso comercial, condiciones y procedencia:** iguales que los otros dos.

**Qué no hace bien, y se acepta si se aprueba.** Es el más lento —unos cinco minutos por resumen con cuatro
hilos— y ocupa 4,7 GB cargado, que **exige el tope de memoria de 8 GB** añadido con él. Solo se recomienda en un
equipo de 12 GB y ocho núcleos.

## `kokoro-82m`

**Revisado el 2026-09-17**, contra los cinco requisitos de la sección 2.1 además de los de la 2. Es el modelo de
**voz**.

| Campo | Valor |
|---|---|
| Identificador en el catálogo | `kokoro-82m` |
| Autor | hexgrad |
| Repositorio | `hexgrad/Kokoro-82M`, en Hugging Face |
| Revisión fijada | `f3ff3571791e39611d31c381e3a41a3af07b4987` |
| Licencia | `Apache-2.0` |
| Archivo de licencia | Sí, `LICENSE` en el repositorio |
| Capacidad | Síntesis de voz, 24 kHz mono |
| Arquitectura | StyleTTS2 con decodificador ISTFTNet, 82 millones de parámetros |

| Archivo | Bytes | SHA-256 |
|---|---|---|
| `model.pth` | 327212226 | `496dba118d1a58f5f3db2efc88dbdc216e0483fc89fe6e47ee1f2c53f18ad1e4` |
| `config.json` | 2351 | `5abb01e2403b072bf03d04fde160443e209d7a0dad49a423be15196b9b43c17f` |
| `ef_dora.pt` | 523420 | `d9d69b0f8a2b87a345f269d89639f89dfbd1a6c9da0c498ae36dd34afcf35530` |
| `em_alex.pt` | 523420 | `5eac53f767c3f31a081918ba531969aea850bed18fe56419b804d642c6973431` |
| `em_santa.pt` | 523430 | `aa8620cb96cec705823efca0d956a63e158e09ad41aca934d354b7f0778f63cb` |

Los pesos se publican como `kokoro-v1_0.pth` y las voces bajo `voices/`; en la carpeta instalada se llaman como
dice la tabla, igual que ya ocurre con los `.gguf`.

**Los cinco requisitos de voz, uno a uno.**

1. **Licencia permisiva:** Apache-2.0, con archivo en el repositorio.
2. **Datos declarados:** su ficha dice que se entrenó **solo** con audio permisivo o de dominio público y audio
   sintético, y cita las fuentes CC-BY que usa.
3. **Sin clonación:** el modelo produce voz a partir de un vector de estilo publicado, no de una muestra de
   audio. El motor de referencia del que se adaptó el código **sí** traía clonación desde una muestra, y esa
   parte **no se vendorizó**: ver `src-tauri/src/voice/kokoro/PROCEDENCIA.md`.
4. **Voces que no imitan a nadie identificable:** son voces sintéticas con nombre de pila, y así se presentan.
5. **La interfaz lo dice:** lo cumple la implementación, que es donde la voz se elige.

**Solo entran las tres voces de español.** El repositorio publica más, y no se ponen en el catálogo hasta que
exista el conversor de fonemas de su idioma: una voz inglesa leyendo fonemas hechos con reglas del español no
suena peor, suena a otra cosa. Lo comprueba una prueba del catálogo, no la buena voluntad.

**El formato, que es la excepción de la sección 2.** Los pesos vienen en `.pth` y las voces en `.pt`, que son
*pickles* de Python, y la regla del catálogo solo admite formatos que no ejecutan código. Entran porque **aquí no
los abre Python**: los pesos los lee el lector restringido de Candle y las voces, el lector de ZIP del proyecto,
que saca los números del almacén sin tocar el pickle. No se afirma, se prueba:
`tests/voz_neuronal.rs` le da al lector un pickle con una llamada dentro y comprueba que no ocurre.

### Los tres generativos, comparados

| Modelo | Licencia | Descarga | Memoria cargado | Lectura / escritura (tokens/s) | Qué aporta, y qué cuesta |
|---|---|---|---|---|---|
| `Qwen/Qwen3-1.7B` en Q8_0 | Apache-2.0 | 1,83 GB | 3,6 GB | 8-10 / 6,5 | Escribe mejor y **citó con `[F6]` en prosa**; ocupa casi el doble y lee la entrada a la mitad. **En el catálogo**, recomendado desde 8 GB |
| `Qwen/Qwen3-4B` en Q4_K_M | Apache-2.0 | 2,50 GB | 4,7 GB | 6 / 3,3 | **El único que devolvió JSON válido** y las tarjetas mejor escritas. Casi cinco minutos por resumen. **En el catálogo**, recomendado desde 12 GB y ocho núcleos |
| `Qwen/Qwen2.5-3B-Instruct` | Qwen Research (`other`) | — | — | — | **Licencia no admitida**: restringe el uso comercial. No se descargó |

Medido en un equipo de 8 núcleos y 19 GB, con cuatro hilos, AVX2 compilado, la misma entrada de unos 800 tokens
en español y otra carga de trabajo a la vez, así que las velocidades son un suelo y no un techo; las proporciones
entre modelos no cambian.

## 3. Alternativas revisadas y no admitidas

No entraron al catálogo, y se deja por qué para no volver a revisarlas sin un dato nuevo.

| Modelo | Licencia | Descarga | Por qué no |
|---|---|---|---|
| `intfloat/multilingual-e5-small` | MIT | 470 MB | **Mejor calidad** —59,3 en MIRACL, 51,0 en MTEB en español— y 94 idiomas. Pierde por coste: la mitad de rápido (0,30 s por consulta frente a 0,17) y más del doble de descarga, con el presupuesto de disco ajustado y el de fotograma delante. Exige además anteponer `query: ` y `passage: `. **Es la primera candidata si la calidad resulta insuficiente**, y ese dato se mediría con el corpus |
| `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2` | Apache-2.0 | 470 MB | Entrenado para paráfrasis y similitud, no para recuperación, que es lo que se le pide. Mismo peso que e5-small con peor encaje |
| `sentence-transformers/all-MiniLM-L6-v2` | Apache-2.0 | 91 MB | **Solo inglés.** Es el "modelo de unos 100 MB" que suponían [ADR-007](../adr/007-vector-store.md) y la primera versión de `hardware::MEMORIA_MINIMA`, y sobre una biblioteca en español no sirve. La suposición se corrigió en esos dos sitios el 2026-09-16 |

**Fuentes.** Los metadatos, la ficha y la lista de archivos de cada modelo, consultados en la API de Hugging Face el
2026-09-16; y el informe técnico *Granite Embedding Models* ([arXiv 2502.20204](https://arxiv.org/abs/2502.20204)),
tablas 3, 4 y 5, que compara los dos candidatos con la misma metodología.
