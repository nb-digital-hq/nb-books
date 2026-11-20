# Política de Dependencias (Dependency Policy)

> **Estado:** Aplicable. Cada dependencia directa tiene aquí su ficha con coste, licencia y mantenimiento.
> La deuda técnica más silenciosa de un proyecto es la acumulación de dependencias no auditadas.

## 1. Reglas Generales de Aprobación

Ningún desarrollador puede añadir un nuevo paquete (`cargo add` o `pnpm add`) a la rama principal sin cumplir **todas** estas condiciones:

1. **Justificación de Costo/Beneficio:** ¿Se puede implementar en menos de 100 líneas de código propio? Si sí, no agregues la dependencia (ej. funciones simples de formateo de fechas o debounce).
2. **Actividad y Mantenimiento:** El paquete debe haber tenido commits en los últimos 12 meses. No adoptamos software abandonado.
3. **Auditoría de Seguridad:** Debe pasar limpio por `cargo audit` (Rust) y `pnpm audit` (Node).

## 2. Dependencias del Frontend (NPM / Svelte)

Dado que todo KB afecta el _Performance Budget_ y la latencia del TTI:

- **Prohibidos los Frameworks Secundarios:** No agregar jQuery, React, Vue, o Alpine dentro del ecosistema Svelte.
- **No "Utility Libraries" Gigantes:** Prohibido usar `lodash` o `moment.js`. Usar las alternativas nativas de ES6 o librerías micro (ej. `date-fns`).
- **Tree-shaking obligatorio:** Si una librería no soporta ECMAScript Modules (ESM) para tree-shaking, será rechazada.

## 3. Dependencias del Backend (Rust / Cargo)

El backend de Rust es el garante de la seguridad de memoria de NB Books.

- **Política `unsafe`:** Se auditarán agresivamente los crates que abusen del bloque `unsafe` sin necesidad. Solo se tolera en bindings FFI comprobados (ej. `pdfium-render`).
- **Minimizar Tiempos de Compilación:** Evitar crates monolíticos que dupliquen el tiempo de compilación si solo se necesita una pequeña función.
- **Vendoring:** Para dependencias críticas que podrían desaparecer de _crates.io_ o para modificaciones internas (ej. un conector específico de Wasmtime), se prefiere hacer fork o vendoring dentro del workspace de NB Books.

### 3.1 Wasmtime: la excepción a "minimizar tiempos de compilación", registrada

Aprobada el 2026-09-07. Se escribe aquí porque **incumple literalmente** la regla de
arriba, y una excepción que no se registra es una regla que deja de existir sin que nadie lo decida.

| Qué                  | Dato                                                                                                                                                       |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Paquetes             | `wasmtime` y `wasmtime-wasi`, versión 48.0.1                                                                                                               |
| Licencia             | Apache-2.0 WITH LLVM-exception, ya admitida en `deny.toml`                                                                                                 |
| Mantenimiento        | Bytecode Alliance, publicación mensual. Es el motor de Zed                                                                                                 |
| Coste de compilación | **El enlace en release pasó de 258 a 576 segundos: se multiplicó por 2,2.** Medido el 2026-09-07 en esta máquina, con el mismo árbol y solo esa diferencia |
| Coste de avisos      | Los componentes de Rust en `THIRD-PARTY-NOTICES.md` pasaron de 577 a 649: **72 paquetes nuevos**                                                           |
| Opciones activadas   | `cranelift`, `runtime`, `std` y, por `wasmtime-wasi`, `p1`. Todo lo demás está fuera                                                                       |

**Por qué se acepta.** No hay una función pequeña que copiar: lo que se necesita es un motor de WebAssembly con
aislamiento, límites de memoria y de tiempo, y soporte del modelo de componentes para la interfaz tipada de
La implementación. Escribirlo es inconcebible y las alternativas no cubren el requisito: `wasmi` es un intérprete sin
modelo de componentes ni WIT, así que elegirlo obligaba a inventar el contrato anfitrión-invitado a mano y a
reescribir la sección 2.1 de [plugin-system.md](plugin-system.md).

**Qué se vigila a cambio.** El tiempo de compilación se paga una vez por máquina y por cambio de dependencias;
el peso en el binario se paga en **cada instalación**, y ese es el que importa: **+6.037.008 bytes, un 49 %**,
que dejan el margen sobre el presupuesto del instalador en 2,7x donde había 4,1x. Cabe, y se vigila. La medida y
la trampa en la que estuvo a punto de caer están en la sección 4.2 de
[performance-budgets.md](performance-budgets.md).

### 3.2 `ring`: una dependencia que no añade nada al árbol, registrada igual

Aprobada el 2026-09-07. Se registra porque la regla dice que **toda** dependencia nueva se
justifica, y "ya estaba ahí" es justo el argumento que, sin escribirlo, acaba usándose para cualquier cosa.

| Qué                  | Dato                                                                        |
| -------------------- | --------------------------------------------------------------------------- |
| Paquete              | `ring`, versión 0.17.14                                                     |
| Licencia             | Apache-2.0 AND ISC, las dos ya admitidas en `deny.toml`                     |
| Mantenimiento        | Es la base criptográfica de `rustls`, que ya se usa para el TLS de `net.rs` |
| Coste de compilación | **Ninguno.** Ya se compila hoy: lo arrastra `rustls`                        |
| Coste de peso        | **Ninguno.** Ya viaja en el binario                                         |
| Avisos de terceros   | Ninguno nuevo: `ring` ya figura en `THIRD-PARTY-NOTICES.md`                 |

**Por qué no se aplica la regla de las cien líneas.** Porque a la criptografía no se le aplica. Una firma la
comprueba una implementación auditada o no se comprueba; escribir Ed25519 propio para ahorrarse una línea en
`Cargo.toml` sería exactamente la clase de decisión que esta política existe para impedir. Lo que se necesita
—verificar una firma Ed25519 y generar un par de claves— son dos funciones de un crate que ya está compilado
dentro del binario.

**El precedente que se sigue.** `sha2`, `flate2` y `quick-xml` entraron igual: estaban en el árbol como
dependencias transitivas y pasaron a directas sin añadir ningún crate al `Cargo.lock` ni ninguna licencia nueva.
La diferencia entre eso y una dependencia de verdad nueva es medible, y es la que decide.

### 3.3 La opción `trace` de rusqlite: cero crates, una guarda exacta

Activada el 2026-09-12. Se registra porque la regla dice que **toda** dependencia se justifica, y una
opción de una dependencia que ya está también cambia lo que se compila.

| Qué           | Dato                                                                                                   |
| ------------- | ------------------------------------------------------------------------------------------------------ |
| Paquete       | `rusqlite`, que ya estaba. Se añade su opción `trace`                                                  |
| Licencia      | MIT, ya admitida. Ninguna nueva                                                                         |
| Coste         | **Ninguno en crates**: `trace = []` no arrastra nada. Expone el callback de SQLite por conexión         |
| Coste en ejecución | Ninguno mientras nadie instale un rastro, que es el caso en la aplicación: solo lo usa una prueba |

**Por qué.** Para fijar con exactitud que la comprobación del índice no vuelve a lanzar el `integrity-check` de
cuatro minutos sobre `documents_fts`. Las alternativas no servían: una prueba de tiempo mediría la máquina y no
el código —y sobre una base de pruebas esos cuatro minutos son microsegundos—, y una comprobación por convención
en un documento no es un mecanismo. Con el rastro, la prueba afirma **qué sentencia se ejecuta**, que es lo que
distingue el defecto.
### 3.4 Candle: el motor de inferencia local, y el argumento que se cumplió a medias

Aprobado el 2026-09-12, eligiendo entre cuatro opciones. Sustituye a lo que
[local-models.md](ai/local-models.md) decía desde que se escribió: `llama.cpp` por FFI u ONNX Runtime.

| Qué                  | Dato                                                                                                                                                                   |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Paquete              | `candle-core`, versión 0.11.0, **sin opciones por defecto**                                                                                                            |
| Licencia             | MIT OR Apache-2.0. El árbol que arrastra trae MIT y Apache-2.0, todas ya admitidas en `deny.toml`; `cargo deny check licenses` pasa en verde                            |
| Mantenimiento        | Hugging Face. Es el motor de sus propias herramientas de inferencia en Rust                                                                                            |
| Coste de avisos      | Los componentes de Rust en `THIRD-PARTY-NOTICES.md` pasaron de 649 a 715: **66 paquetes nuevos**                                                                        |
| Coste de compilación | **Un `cargo build --release` completo pasó de 936 a 1.178 segundos: +242 s, un 26 %.** Medido el 2026-09-12 en esta máquina, con el mismo árbol y solo esa diferencia   |
| Coste de peso        | **El binario de release pasó de 23.872.096 a 23.881.696 bytes: +9.600 B, un 0,04 %.** Misma medición, mismo árbol                                                       |
| Opciones activadas   | Ninguna. Y la aplicación **no declara** `cuda` ni `metal` como opciones propias: hacerlo rompe `cargo clippy --all-features`, que las activa todas y exige la cadena de NVIDIA y macOS. El acelerado se pide al compilar con `--features candle-core/cuda`, y el runtime lo detecta abriendo el dispositivo |

**Medido de verdad, con el modelo cargándose:** `candle-nn` y `candle-transformers` añadieron solo
**4 paquetes** a los avisos (715 a 719) y el binario pasó de 24.469.472 a **27.378.848 bytes, +2,9 MB, un
11,9 %**, que es lo que pesa ejecutar XLM-RoBERTa. El margen sobre el presupuesto del binario baja de 2,0x a 1,8x.
Las dos crates se aprobaron el 2026-09-16, y `tokenizers` no añade nada: ya lo traía `candle-core` con la
misma opción. El peso medido con los modelos cargados está en [performance-budgets.md](performance-budgets.md).

**Una medida de peso sin uso engaña.** El código genérico de Candle no se monomorfiza si nadie lo usa, así que
medirlo sin instanciar un modelo da una cifra casi nula; el peso que vale es el medido con el modelo cargado.

**Por qué se acepta.** No hay una función pequeña que copiar: un motor de tensores con álgebra lineal
optimizada, cuantización y soporte de GPU no se escribe. Se eligió entre cuatro opciones y la que decidió fue
esta: las dos cicatrices de la puerta de empaquetado de este proyecto son **el runtime que estuvo semanas sin
compilar en Windows** y **el motor de PDF que no viajaba en el instalador**, y un motor de C++ con su propio
sistema de construcción y backends acelerados distintos por plataforma y por GPU es esa misma clase de defecto
multiplicada por tres sistemas y por fabricante de tarjeta.

**Y aquí está la parte del argumento que se cumplió a medias, comprobada al medir y no supuesta.**
`candle-core` **depende obligatoriamente** de `tokenizers` -no es opcional, es una dependencia de destino para
todo lo que no sea `wasm32`- y la declara con la opción `onig`, que es **Oniguruma, una biblioteca de C**; con
ella entra además `esaxx-rs`, que es **C++**. Las dos se compilan con `cc` como parte del `cargo build`, y no hay
forma de apagarlas desde aquí: la unificación de opciones de Cargo es aditiva, así que declarar `tokenizers` con
`default-features = false` por nuestra parte no desactiva lo que otro crate activó.

Así que "sin nada de C" es falso, y no se escribe. **Lo que sí es cierto, y es lo que sostiene la decisión, es
otra cosa:** no hay un sistema de construcción ajeno, ni CMake, ni binarios de backend por plataforma y por GPU
que fabricar, firmar y distribuir aparte. `onig` y `esaxx-rs` se compilan igual que el SQLite empotrado que
`rusqlite` ya trae -este proyecto **nunca** tuvo un árbol sin C- y salen del mismo
`cargo build --target <lo que sea>` que todo lo demás. La clase de defecto que se quería eliminar era la del
motor que hay que construir y empaquetar por separado, y esa sigue eliminada.

**Qué se vigila a cambio.** El peso en el binario con los modelos cargados de verdad, que está medido en
[performance-budgets.md](performance-budgets.md), y **el rendimiento en generación**, donde Candle es más lento que
`llama.cpp`: por eso la generación local se ofrece con su coste medido delante
([ai/local-models.md](ai/local-models.md) sección 6).

### 3.5 `libc` y `windows-sys`: memoria física, sin añadir nada al árbol

Aprobadas junto con Candle el 2026-09-12. Se registran porque la regla dice que **toda** dependencia
se justifica, y sigue el precedente de la sección 3.2.

| Qué                  | Dato                                                                                                                      |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Paquetes             | `libc` 0.2 en Unix; `windows-sys` 0.61 con la opción `Win32_System_SystemInformation` en Windows                          |
| Licencia             | MIT OR Apache-2.0 las dos, ya admitidas                                                                                   |
| Mantenimiento        | `libc` es del equipo de Rust; `windows-sys` es de Microsoft                                                               |
| Coste                | **Ninguno.** Las dos ya viajaban en el árbol: `libc` lo arrastra media biblioteca y `windows-sys` lo arrastra Tauri        |
| Para qué             | Leer la memoria física de la máquina y poder decir `NOT_SUPPORTED` en un equipo que no da, en lugar de ofrecerle y fallar |

**Por qué no se escribió a mano.** En Unix sí se escribió: es una llamada a `sysconf`, la misma en Linux y en
macOS, así que las dos plataformas comparten camino en lugar de tener cada una el suyo sin probar. En Windows la
información solo la da `GlobalMemoryStatusEx`, que es una función del sistema y no hay nada que copiar.

**`libc` hace además dos cosas, en Linux y sin cambiar su coste:** `setpriority` y `gettid`, para bajar
la prioridad de los hilos de la inferencia. En macOS y Windows no se usan.

### 3.6 `rayon`, declarada: el grupo de hilos de la inferencia

Declarada el 2026-09-16. **No añade ningún crate ni cambia ninguna versión**: `candle-core` ya la trae, y
es por donde Candle reparte su trabajo.

| Qué           | Dato                                                                                                           |
| ------------- | -------------------------------------------------------------------------------------------------------------- |
| Paquete       | `rayon` 1.12, la misma que ya estaba en `Cargo.lock`                                                           |
| Licencia      | MIT OR Apache-2.0, ya admitida                                                                                 |
| Mantenimiento | Del grupo `rayon-rs`; es la biblioteca de paralelismo de referencia en Rust                                    |
| Coste         | **Ninguno**: ni un crate más, ni un byte más en el binario, porque ya se compilaba                             |
| Para qué      | Construir un grupo de hilos **con tope** y ejecutar la inferencia dentro, en lugar del grupo global, que ocupa todos los núcleos |

**Por qué hace falta declararla y no basta con la que trae Candle.** Candle no ofrece forma de decirle cuántos
hilos usar salvo la variable de entorno `RAYON_NUM_THREADS`, que es del proceso entero, se lee una vez y no se puede
cambiar desde Ajustes. Lo que sí respeta es el grupo en el que se le llama: `gemm`, que es quien multiplica las
matrices, reparte con `into_par_iter`, que corre en el grupo actual. Así que el tope se pone construyendo un grupo y
llamando a Candle dentro, y para construirlo hay que nombrar el crate.

### 3.7 `opus` y `ogg`: guardar un libro entero en audio sin llenar el disco

Aprobadas el **2026-09-18**, con la aritmética delante y el coste dicho.

| Qué           | Dato                                                                                                             |
| ------------- | ---------------------------------------------------------------------------------------------------------------- |
| Paquetes      | `opus` 0.4 (envoltura segura) y `ogg` 0.9 (envoltorio, Rust puro). `opus` arrastra `opusic-sys` 0.7               |
| Licencias     | MIT OR Apache-2.0 la primera; BSD-3-Clause las otras dos. Las tres ya admitidas                                   |
| Mantenimiento | `ogg`, diez millones de descargas; `opus` y `opusic-sys`, dos millones y actualizadas en agosto de 2026            |
| Coste         | **C en la compilación**: `opusic-sys` trae las fuentes de libopus y las compila con `cmake`. Nada que instalar en la máquina de quien usa la aplicación |
| Para qué      | Guardar el audio preparado de un capítulo o de un libro                                                           |

**La aritmética que las justifica.** Lo que produce el modelo son 24.000 muestras por segundo de 16 bits: **173 MB
por hora**. Un libro de diez horas son 1,7 GB, y veinte libros se comen el 20 % de lo derivado sobre un corpus de
154 GB —el de esta máquina— sin dejar sitio al índice ni a los vectores. En Opus a 24 kbit/s la misma hora son unos
**11 MB**: dieciséis veces menos, y ahí caben cientos de libros. La alternativa sin dependencias era guardarlo en
crudo y decirle a la persona cuántos libros caben, que en la práctica es pedirle que elija cuáles puede escuchar.

**Lo que cuesta, sin adornos.** Vuelve a entrar C en la compilación, como con `onig` y `esaxx-rs`: `opusic-sys`
compila libopus desde sus fuentes. Se acepta porque la alternativa no era "sin C", era "sin la función".

**Por qué también `ogg`.** Lo que devuelve el codificador son paquetes que no llevan dónde empiezan ni cuánto
duran; sin envoltorio, ningún reproductor los abre, y el audio tiene que sonar en la ventana con un `<audio>` como
cualquier otro. Ogg es el envoltorio estándar de Opus y lo entienden los tres motores sobre los que corre esta
aplicación. Es Rust puro, así que no añade nada al riesgo de arriba.

### 3.8 CUDA y Metal: opciones de Candle, no dependencias del manifiesto

Preparadas el **2026-09-22**, **sin probar**: aquí no hay GPU de NVIDIA ni Mac.

| Qué           | Dato                                                                                                             |
| ------------- | ---------------------------------------------------------------------------------------------------------------- |
| Paquetes      | Ninguno en el manifiesto ni en `Cargo.lock`. Las ediciones con GPU activan `candle-core/cuda` o `candle-core/metal` (y lo mismo en `candle-nn` y `candle-transformers`) **al compilar**, con la sintaxis `dependencia/opción` |
| Lo que arrastran | `cudarc` y `candle-kernels` con CUDA; `metal` y `candle-metal-kernels` con Metal. MIT o Apache-2.0 todas, ya admitidas |
| Coste         | Solo en esas ediciones: CUDA exige la cadena de NVIDIA para compilar, y quien la ejecuta necesita el controlador de su tarjeta. La edición universal y la rápida no cambian en nada |
| En CI         | `Jimver/cuda-toolkit` instala la cadena de CUDA en el flujo `ediciones.yml` (con `ilammy/msvc-dev-cmd` para el compilador de Visual Studio en Windows), igual que ya se usan acciones de terceros para Rust y para `cargo deny` |

**Por qué no son opciones del manifiesto**, que era lo natural: `cargo clippy --all-features` es verificación
obligatoria, y activaría `cuda` y `metal` en cualquier máquina de desarrollo, donde CUDA exige la cadena de NVIDIA
y Metal exige macOS. Una opción que hace fallar la verificación en todas partes no es una opción. Ya estaba escrito
en `hardware::acelerador`, que por eso **pregunta al motor** qué dispositivo abre en lugar de mirar la compilación.

**Sin verificar en equipo real**: las ediciones con la opción puesta se construyen en `ediciones.yml` y se marcan
como no verificadas.

### 3.9 Los complementos de GStreamer que viajan en el AppImage

Revisión registrada el **2026-09-22**, que es lo que la sección 4 exige para LGPL.

| Qué           | Dato                                                                                                             |
| ------------- | ---------------------------------------------------------------------------------------------------------------- |
| Qué viaja     | Dieciocho complementos de `gstreamer`, `gst-plugins-base` y `gst-plugins-good`, y el explorador `gst-plugin-scanner`. La lista cerrada está en `scripts/preparar-gstreamer.mjs`, cada uno con para qué |
| Licencias     | **LGPL-2.1 o posterior** todos, y el decodificador de MP3 enlaza `libmpg123`, también LGPL-2.1. Opus es BSD |
| Enlazado      | **Dinámico**: GStreamer los carga en tiempo de ejecución desde su carpeta dentro del AppImage. Es el caso que la política admite |
| Qué no viaja  | Nada de `gst-plugins-ugly` ni `gst-libav`, que son GPL y en una distribución como Arch están en la misma carpeta. Por eso la lista es cerrada y no "todos los del sistema", que es lo que haría el complemento de `linuxdeploy` por su cuenta |
| Para qué      | Que el AppImage suene sin instalar nada. El `.deb` y el `.rpm` usan los del sistema y los declaran como dependencia |

Comprobado reproduciendo un Ogg Opus con **solo esos dieciocho** en la ruta de complementos, y comprobado al revés:
sin el de Opus, GStreamer dice que falta el decodificador.

## 4. Política de Licencias

NB Books se publica bajo **Apache License 2.0** (ver [ADR-003](../adr/003-licensing-model.md)).
La licencia del proyecto determina qué dependencias puede incorporar.

| Licencia de la dependencia                                 | Situación                                                                                                                                                                                           |
| ---------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| MIT, Apache 2.0, BSD 2 y 3 cláusulas, ISC, Unlicense, Zlib | Permitidas                                                                                                                                                                                          |
| SIL OFL 1.1                                                | Permitida **solo para tipografías**, que es para lo que existe. No impone reciprocidad sobre el software que las usa; su única obligación es distribuir el texto de la licencia junto a las fuentes |
| MPL 2.0, LGPL                                              | Caso por caso, solo con enlazado dinámico y revisión registrada en el PR                                                                                                                            |
| GPLv2                                                      | **Prohibida.** Apache 2.0 es incompatible con GPLv2; no existe forma de combinarlas                                                                                                                 |
| GPLv3, AGPL                                                | **Prohibida** en el núcleo distribuido. Apache 2.0 es compatible en un solo sentido: se puede incorporar código Apache en un proyecto GPLv3, no al revés                                            |
| Sin licencia declarada                                     | **Prohibida.** No se distribuye lo que no se puede atribuir                                                                                                                                         |

El CI verifica esto de forma automatizada con `cargo deny check licenses` y su equivalente para el frontend. Una
dependencia con licencia no permitida bloquea el build; no es una advertencia.

**Herramientas de desarrollo** que no se distribuyen con el producto (linters, generadores, formateadores) no
están sujetas a esta tabla, porque no se incorporan al binario.

**Registro de las que se han acogido a esa exención:**

| Dependencia | Licencia | Fecha | Para qué, y por qué no se escribió aquí |

### 2.1 `m3-svelte`: el lenguaje visual de Material 3

Aprobada el 2026-09-09, en ADR-014. Se documenta porque es una dependencia fundacional que gobernará toda la interfaz.

| Qué               | Dato                                                                                                                                                                                                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Paquetes          | `m3-svelte`                                                                                                                                                                                                                                                                          |
| Licencia          | Apache-2.0 / GPL-3.0                                                                                                                                                                                                                                                                 |
| Mantenimiento     | Mantenido por la comunidad. Componentes en Svelte y CSS puros sin dependencias profundas.                                                                                                                                                                                            |
| Por qué se acepta | Proveer el ecosistema de Material 3 (riel de navegación, capas de estado dinámico, color derivado de semilla) que exigiría construir un sistema visual gigantesco de cero. El tamaño de los componentes está en línea con nuestro presupuesto de rendimiento (el desarrollo.4.2 lo audita). |
| Alternativas      | SMUI se descartó porque usa Material 2; la implementación de Google está sin equipo. Ver ADR-014.                                                                                                                                                                                    |

**Cómo se usa.** El paquete de arranque cabe en su presupuesto porque la utilería de color y los destinos se cargan
a demanda ([performance-budgets.md](performance-budgets.md) sección 1.2).

* **De la biblioteca se usan cuatro componentes** —`Button`, `Card`, `LinearProgress` y `TextFieldOutlined`— en
  dos archivos. El resto de la interfaz está escrita a mano con los tokens `--m3c-*`, que son los nombres que usa
  la propia biblioteca. No es necesariamente un error, pero cambia la cuenta: lo que hoy justifica la dependencia
  no es su ecosistema de componentes sino su **vocabulario de tokens y su hoja de estilos**.

### 2.2 `@ktibow/material-color-utilities-nightly`: la paleta derivada de una semilla

Entró con `m3-svelte` y está declarada como dependencia directa, así que lleva su ficha.

| Qué               | Dato                                                                                                                                                                                                                                     |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Paquetes          | `@ktibow/material-color-utilities-nightly`                                                                                                                                                                                               |
| Licencia          | Apache-2.0                                                                                                                                                                                                                               |
| Coste             | **106 KB** del paquete de JavaScript, medido. Es el mayor coste unitario identificado de la interfaz                                                                                                                                      |
| Mantenimiento     | **Es el riesgo que hay que decir.** Es una publicación *nightly* del proyecto de Google, empaquetada por el autor de `m3-svelte`, y la versión declarada es `^0.4.1772748028000`: un rango que flota sobre publicaciones automáticas       |
| Por qué se acepta | Es lo que deriva las catorce paletas -siete temas por dos niveles de contraste- y lo que hace que el alto contraste sea un nivel de contraste de verdad y no otra semilla. Reimplementar HCT y las curvas de contraste a mano no es opción |
| Qué falta         | Fijar la versión en lugar de dejarla flotar, y sacar el cálculo del arranque: las catorce paletas no dependen de nada del usuario salvo el acento propio, así que pueden calcularse al construir. Ver 4.9 de los presupuestos             |

|---|---|---|---|
| `@axe-core/playwright` y `axe-core` | MPL-2.0 | 2026-08-28 | La auditoría de accesibilidad. El criterio de la iteración es "cero violaciones **serias o críticas**", y esas son dos de las cuatro severidades que asigna axe: unas comprobaciones propias cubrirían un puñado de reglas y no podrían clasificar ninguna, así que el criterio quedaría cumplido de nombre. Solo se ejecuta en la suite de navegador; no entra en el binario de Tauri, y MPL-2.0 está además en la tabla de permitidas |
| IBM Plex Sans y Literata | OFL-1.1 | 2026-09-03 | Las tipografías de la interfaz. **Se descubrió que no estaban en ninguna parte**: el sistema de diseño las nombraba, no había ni un `@font-face` y no estaban instaladas en la máquina de desarrollo, así que la interfaz se pintaba con Noto Sans en Linux, Segoe UI en Windows y SF Pro en macOS. Lo que se protegía no era la identidad visual sino **que lo medido valga**: las métricas de texto cambian con la fuente, y la auditoría visual estaba midiendo una que no era la de producción en ninguna plataforma. La OFL no impone nada al software que las usa; el texto va en `static/fuentes/LICENCIAS.md`, que es su única obligación. 477 KB, solo los subconjuntos latinos y los pesos que el diseño usa |

### 3.2 `wit-bindgen`, en el banco y solo en el banco

Aprobado el 2026-09-07. Se registra aunque **no viaje en el producto**, porque entra al
repositorio y esa es la frontera que marca la sección 1.

| Qué                  | Dato                                                                                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Paquete              | `wit-bindgen` 0.61, en `extensiones/banco/usa-el-mundo/`                                                                                                                        |
| Licencia             | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT                                                                                                                             |
| Mantenimiento        | Bytecode Alliance, el mismo proyecto que Wasmtime                                                                                                                               |
| Coste en el producto | **Ninguno.** El banco es un espacio de trabajo aparte: no entra en `cargo metadata` de la aplicación, así que no toca el binario, ni el instalador, ni `THIRD-PARTY-NOTICES.md` |

**Por qué no se escribe a mano.** Se puede: `wasm32-wasip2` produce componentes y `wasm-component-ld` está
instalado. Pero habría que acertar a mano el nombre mangleado de cada exportación del mundo y su ABI canónico, y
eso se rompe en silencio cada vez que el mundo cambia, que va a ser en cada iteración.

### 4.1 Cuando evitar una licencia cuesta una garantía

Este criterio existe porque el caso ya se dio, y para que no haya que volver a discutirlo cada vez.

Una dependencia puede plantear esta elección: **incorporar una licencia permisiva que no está en la tabla, o
sacrificar una propiedad verificable del proyecto**. Propiedad verificable significa algo que se puede
comprobar y que se paga todos los días si se pierde: que un clon limpio compile sin bibliotecas nativas
instaladas, que el binario sea reproducible entre plataformas, que un componente ausente degrade en lugar de
romper.

**Gana la propiedad verificable**, siempre que la licencia cumpla las tres condiciones:

1. Es **permisiva y sin reciprocidad**: no impone nada al código que la acompaña.
2. Es **compatible con Apache 2.0** en el sentido que necesitamos, que es incorporarla y redistribuirla.
3. Se puede **atribuir**, y la atribución entra en `THIRD-PARTY-NOTICES.md`.

El motivo: la tabla de licencias es una herramienta para proteger la viabilidad legal del proyecto, no un fin en
sí misma. Una licencia permisiva no amenaza esa viabilidad. Una garantía de ingeniería perdida, en cambio, se
cobra en cada compilación, en cada plataforma y en cada persona que clone el repositorio.

Lo que **no** autoriza este criterio: incorporar una licencia recíproca, una sin declarar, o una que restrinja
el uso. Ahí la tabla de la sección 4 sigue mandando sin excepción, y la respuesta es cambiar de dependencia.

**Procedimiento.** Ampliar la lista de licencias admitidas exige tocar **tres sitios que deben coincidir**:
esta sección, `deny.toml` y `scripts/generate-notices.mjs`. Que coincidan no depende de que alguien se acuerde:
`pnpm check:notices` compara las tres listas y falla si divergen. Y la ampliación se documenta con su fecha, la
dependencia que la motivó y la garantía que se estaba protegiendo.

### 4.2 Licencias de datos incorporados

Una dependencia puede traer **datos** y no código: listas, tablas, catálogos. Su licencia se revisa igual, pero
la tabla de arriba está escrita para código y no siempre encaja.

| Licencia            | Situación                                | Revisión  |
| ------------------- | ---------------------------------------- | --------- |
| CDLA-Permissive-2.0 | **Permitida**, incorporada el 2026-08-27 | Ver abajo |

**CDLA-Permissive-2.0**, incorporada el 2026-08-27 al adoptar el cliente HTTP, por el criterio de la
sección 4.1. La garantía que se estaba protegiendo: que un clon limpio compile sin bibliotecas nativas
instaladas. La trae el paquete de raíces de
confianza de Mozilla, que es el catálogo de autoridades certificadoras que usa cualquier cliente TLS del
ecosistema Rust. Es una licencia de datos permisiva, **sin reciprocidad**: permite usar, modificar y
redistribuir sin más condición que conservar los avisos. Es compatible con Apache 2.0 y no impone nada al
código que la acompaña.

**Por qué no se evitó.** La alternativa era usar la pila TLS del sistema operativo, que no necesita ese
catálogo. Se descartó porque en Linux arrastra `openssl-sys`, y con él la necesidad de tener las cabeceras de
OpenSSL instaladas para compilar. Eso rompería la propiedad que el proyecto sostiene: un clon
limpio compila sin bibliotecas nativas instaladas. Es exactamente el caso que resuelve el criterio de la
sección 4.1.

### Modelos de IA

Los pesos de los modelos tienen licencia propia, distinta de la del código, y algunas restringen el uso
comercial, lo que es incompatible con la licencia Apache 2.0 del núcleo. Ningún modelo entra al catálogo por defecto sin revisión
de licencia registrada. Ver [ai/local-models.md](ai/local-models.md).
