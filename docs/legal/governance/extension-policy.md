# Política del Registro de Extensiones

> **Estado:** En vigor.
> La arquitectura está en [plugin-system.md](../../architecture/plugin-system.md); la referencia para
> autores, en [api/plugins.md](../../architecture/api/plugins.md).

## 1. Principio

Un registro de extensiones convierte al proyecto en un canal de distribución de software de terceros. El usuario
que instala una extensión desde el registro oficial confía en NB Books, no en el autor. Esa confianza obliga.

## 2. Cómo está escrita esta política, y por qué así

Una política de registro que solo enumera buenas intenciones no protege a nadie: llega el día del primer envío
dudoso y cada requisito admite la lectura que le conviene a quien lo lee. Por eso **cada requisito de la sección
3 lleva pegado su mecanismo**, con cuatro estados posibles y ninguno más:

| Estado | Qué significa |
|---|---|
| `automática` | Hay código que lo comprueba en cada envío. Si el código no lo detecta, el requisito no se aplicó |
| `parcial` | Una parte se comprueba sola y la otra no. Se dice cuál es cada una |
| `manual` | Lo mira una persona. Se dice qué mira, porque "revisión manual" sin más es un lugar donde esconder que no se revisa |
| `ninguna` | Hoy no se comprueba. Se dice qué haría falta para comprobarlo |

**Los tres últimos estados no son excusas: son deuda declarada.** Un requisito en `manual` o en `ninguna` sigue
siendo obligatorio para el autor y sigue siendo motivo de retirada; lo que cambia es que el proyecto no puede
prometer que lo detectará solo, y decirlo es más útil que dar por hecho lo contrario.

**Lo que impide que esta tabla mienta con el tiempo** es `scripts/check-politica.mjs`, que corre en
`pnpm verify` y en el gancho de pre-commit. Comprueba que cada requisito tenga sus campos, que el estado sea uno
de los cuatro, que todo mecanismo declarado **exista como archivo** y que ese archivo **cite el identificador
del requisito**. Un mecanismo que se borra o se renombra deja de citar su requisito, y la verificación falla
antes de que la política empiece a prometer algo que ya no ocurre.

## 3. Requisitos

Los identificadores son estables: se citan desde el código, desde los informes de revisión y desde los avisos
que recibe un autor. No se renumeran.

### R1 — Manifiesto válido y completo

* **Exige:** un `nb-extension.json` que valide contra el esquema publicado, con identificador en forma de
  dominio invertido, versión en SemVer completo y clase declarada.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/manifest.rs`, `scripts/check-esquema.mjs`
* **Si falla:** el envío se detiene y se devuelve el campo exacto, qué se esperaba y qué llegó. Un manifiesto
  inválido no se corrige por el proyecto: se devuelve.

### R2 — Identidad que no se hace pasar por oficial

* **Exige:** que ni el identificador ni el nombre visible sugieran que la extensión es un componente oficial de
  NB Books. El espacio `lat.nbdigitallabs.books.*` está reservado a las extensiones de primera parte.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/review.rs`
* **Cómo:** el prefijo reservado se comprueba en el identificador, y **quién envía no se deduce del paquete**: la
  revisión recibe aparte si el envío es de primera parte. En el nombre visible se detiene el nombre del producto
  escrito de cualquier forma ("NB Books Sync", "nb-books", "N.B. Books") y también **lo que se le parece**: se
  pliegan los caracteres que se confunden a la vista (`0` y `o`, `8` y `b`, `z` y `s`...) y se busca un trozo a
  una sola edición de distancia de "nbbooks", así que "NB Bookz" o "N8 Books" se detienen igual.
* **Si falla:** se pide el cambio de nombre antes de admitir el envío. Ver [legal/trademark.md](../trademark.md).

### R3 — Sin carga remota de código

* **Exige:** que la extensión no pueda ejecutar código que no venga dentro del paquete revisado, y que el paquete no
  lleve escrita una dirección de la que traerlo.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/review.rs`, `scripts/check-capacidades.mjs`
* **Cómo:** las dos mitades se comprueban solas. **El anfitrión no ofrece al invitado ninguna función para cargar,
  compilar ni instanciar nada**, y `check-capacidades.mjs` rechaza en `pnpm verify` que el mundo WIT gane una: aunque
  una extensión consiga bajarse un binario, no tiene con qué ejecutarlo. Y la revisión detiene el paquete que trae
  escrita una dirección a algo que se carga y se ejecuta.
* **Si falla:** el envío se detiene. En una versión posterior de una extensión ya publicada, además, se retira
  la versión y se avisa a quien la tenga instalada (sección 6): es el vector clásico y se trata como incidente,
  no como corrección.

### R4 — Sin código escondido dentro de los datos

* **Exige:** que ningún módulo lleve dentro otro módulo o componente de WebAssembly, ni en sus segmentos de datos
  ni en sus secciones personalizadas.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/review.rs`, `src-tauri/src/extensions/wasm.rs`
* **Cómo:** la forma comprobable de este requisito es concreta —un módulo empotrado dentro de otro se reconoce
  por su cabecera— y por eso está redactada así y no como "sin ofuscación", que no se puede comprobar de ninguna
  manera. Un componente **lleva módulos dentro por construcción**, así que en él no se busca una cabecera en
  cualquier parte, que encontraría las suyas: se recorre su formato, que enmarca las secciones como un módulo, y se
  busca **dentro de cada módulo que lleva** y en sus secciones personalizadas.
* **Si falla:** igual que R3.

### R5 — Permisos declarados, y solo los que usa

* **Exige:** que toda capacidad que la extensión ejerza esté declarada en su manifiesto, y que no declare ninguna
  que no pueda ejercer.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/permissions.rs`, `scripts/check-capacidades.mjs`,
  `src-tauri/src/extensions/review.rs`
* **Cómo:** las dos direcciones. El anfitrión **no ofrece** al invitado ninguna función sin permiso detrás, así que
  una capacidad no declarada no se puede ejercer; y la revisión detiene la dirección contraria, declarar de más,
  comparando lo declarado con lo que el paquete puede llegar a ejercer, y la importación de funciones que **ninguna
  capacidad puede conceder**, como abrir un socket o un archivo del anfitrión. Cada permiso se le enseña a quien
  instala con su detalle (los anfitriones exactos de red) antes de concederlo.
* **Si falla:** el envío se detiene. Pedir de más no es inocuo: cada permiso se le muestra a quien instala, y una
  lista inflada enseña a aceptar sin leer.

### R6 — Licencia declarada y presente

* **Exige:** un identificador SPDX en el manifiesto y, dentro del paquete, el texto de **esa** licencia.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/manifest.rs`, `src-tauri/src/extensions/review.rs`
* **Cómo:** el manifiesto obliga a declarar la licencia y rechaza el campo vacío; la revisión exige un archivo de
  licencia en el paquete y comprueba que, para cada identificador de la expresión (`MIT OR Apache-2.0` son dos), su
  texto esté presente, reconocido por una frase que solo aparece en esa licencia. Una licencia que la revisión no
  sabe reconocer detiene el envío.
* **Si falla:** el envío se detiene.

### R7 — Un tema no trae código

* **Exige:** que una extensión de clase `theme` no declare permisos, órdenes, vistas ni proveedores. Solo
  colores, de la lista cerrada de tokens.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/manifest.rs`
* **Si falla:** el envío se detiene. **No se marca para revisión manual**: marcar supone que hay alguien revisando y que acertará, y un manifiesto que se declara
  tema y pide permisos no es un caso dudoso, es uno que miente sobre lo que es.

### R8 — Código fuente incluido

* **Exige:** que un paquete con código traiga su código fuente: una carpeta `src` o `source` con archivos y la
  receta con que se construye (`Cargo.toml`, `package.json`, `go.mod`, `build.zig`, `Makefile` o
  `CMakeLists.txt`). Un paquete declarativo -un tema, un fondo sonoro- no trae código y no lo necesita.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/review.rs`
* **Cómo:** la revisión lo comprueba en cada versión. Quien instala puede leer qué hace lo que instala con el paquete
  en la mano, sin depender de que un repositorio externo siga existiendo. Que esa fuente produzca exactamente este
  binario exigiría compilación reproducible, que el registro no promete.
* **Si falla:** el envío se detiene.

### R9 — Sin telemetría propia ni publicidad

* **Exige:** que la extensión no salga a servicios de medición, rastreo de fallos ni publicidad.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/net.rs`, `src-tauri/src/extensions/review.rs`
* **Cómo:** una extensión sin la capacidad de red **no puede** medir nada: la única salida del dispositivo es
  `net.rs`, con el interruptor general y el consentimiento por anfitrión, propósito y origen. Con red, sus destinos
  son los que declara su manifiesto, los únicos a los que el anfitrión le deja salir, y la revisión detiene los de
  servicios conocidos de analítica, rastreo de fallos y publicidad.
* **Si falla:** el envío se detiene; en una extensión publicada, retirada inmediata, tratada como incidente.

### R10 — Sin elusión de medidas tecnológicas de protección

* **Exige:** que el paquete no traiga herramientas conocidas para quitar la protección de libros electrónicos
  (ver [legal/copyright-and-drm.md](../copyright-and-drm.md)).
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/review.rs`
* **Cómo:** la revisión busca, en los módulos y en los nombres de los archivos, las marcas de las herramientas
  publicadas para ese fin (DeDRM, ineptepub, ineptpdf, ignobleepub, obok y sus claves de cuenta, entre otras).
* **Si falla:** el envío se detiene, y si ya estaba publicada, retirada inmediata.

### R11 — Versión de API declarada y soportada

* **Exige:** que el módulo declare la versión de la API contra la que se compiló, en la sección personalizada
  `nb:api-version`, y que esté dentro del rango soportado. En un componente la sección viaja en sus secciones
  personalizadas o en los módulos que lleva dentro, de modo que la revisión tiene dos declaraciones que
  contrastar también en componentes.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions.rs`, `src-tauri/src/extensions/review.rs`
* **Si falla:** el módulo no se ejecuta, ni siquiera se compila. Un módulo que no declara nada tampoco se
  ejecuta: sin declaración el anfitrión no puede protegerlo, y ejecutar algo que no se sabe qué espera es peor
  que no ejecutarlo.

### R12 — Los artefactos son lo que dicen ser

* **Exige:** que todo archivo del paquete que termine en `.wasm` sea un módulo o un componente de WebAssembly.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/review.rs`
* **Si falla:** el envío se detiene. No es una formalidad: un archivo que no se puede leer como WebAssembly no
  se puede revisar, y dejarlo pasar sería dar por limpio justo lo que nadie ha mirado.

### R13 — Firma del autor

* **Exige:** que el paquete vaya firmado, que la firma corresponda a su contenido, y que una versión nueva de
  una extensión ya instalada la firme **la misma clave** que firmó la anterior.
* **Comprobación:** automática
* **Mecanismo:** `src-tauri/src/extensions/signature.rs`, `src-tauri/src/extensions/store.rs`,
  `src-tauri/src/extensions/review.rs`
* **Si falla:** el envío se detiene, y una actualización firmada por otra clave **no se instala** en la máquina
  de quien ya tenía la extensión, aunque llegue a distribuirse. Es lo que convierte el vector clásico —la
  extensión benigna que se vuelve maliciosa en una actualización— en algo que exige robarle la clave al autor.

**Lo que la firma no dice.** No dice quién es el autor: dice que quien firmó tenía esa clave, y cualquiera puede
generar un par de claves. Lo que ata una clave a un autor es que sea la misma de siempre para esa extensión, y
eso lo fija el almacén en la primera instalación. Mientras no exista el registro que publique las claves, **la
primera instalación es un acto de fe** y se dice así en lugar de dar a entender que ya está protegida.

## 4. Revisión

* **Automatizada, en cada versión publicada.** No basta con revisar el primer envío: el vector clásico es una
  extensión benigna que se vuelve maliciosa en una actualización posterior, y por eso R3 y R4 se comprueban
  igual en la versión 1.0.0 y en la 1.0.1.
* La revisión automatizada es una **puerta ejecutable en local** sobre cualquier paquete, no un trabajo de
  integración continua. El trabajo de CI, cuando exista, se limita a invocarla. El motivo está escrito en la
  ficha: una puerta que solo vive en un CI que nunca ha llegado a ejecutarse no es un mecanismo, es
  una intención.
* **Manual** para extensiones destacadas, muy instaladas o reportadas, y siempre para los requisitos que la
  sección 3 declara en `manual`.
* Una revisión que solo sabe rechazar no demuestra nada: la puerta se prueba contra un par de módulos que se
  diferencian **en una sola cosa** —la misma extensión antes y después de introducir la carga remota— y tiene
  que dejar pasar el primero.

## 4.1 Cómo se envía una extensión

1. **Se prepara el paquete**: un directorio con el manifiesto, los artefactos y el texto de la licencia.
2. **Se crea una clave**, una sola vez, y se guarda como se guarda una llave:

   ```bash
   cargo run --features herramientas --bin firmar -- --crear-clave ~/.nb-books/mi-clave.pkcs8
   ```

   Quien tenga ese archivo puede publicar actualizaciones de esas extensiones, y quien lo pierda no puede
   volver a publicarlas. No hay recuperación posible y no la habrá: una forma de recuperar la identidad de un
   autor sin su clave es, por definición, una forma de suplantarlo.
3. **Se firma y se revisa**, en ese orden, y **cada vez que cambie algo**: la firma cubre la huella del paquete
   entero, así que deja de valer al primer byte.

   ```bash
   cargo run --features herramientas --bin firmar  -- ruta/al/paquete --clave ~/.nb-books/mi-clave.pkcs8
   cargo run --features herramientas --bin revisar -- ruta/al/paquete
   ```

   `revisar` devuelve 0 si el paquete se publica y 1 si no. Es la misma puerta que corre al publicar: lo que
   pase aquí, pasa allí.
4. **Se envía al registro**, escribiendo a **nbdigitalsas@gmail.com** con un asunto que empiece por
   `[NB Books][Extensiones]` y el enlace al paquete firmado. La revisión del paso 3 es la que decide, y **no hay
   intervención del equipo** en un envío que la pasa.

**La primera versión fija al autor.** La clave con la que se firma la primera instalación queda apuntada en la
máquina de quien la instaló, y a partir de ahí solo entra lo que firme esa misma clave. Cambiar de clave obliga
a publicar bajo otro identificador, y eso es deliberado: si cambiar de clave fuera fácil, la clave no protegería
de nada.

## 5. Permisos y consentimiento

El usuario ve, antes de instalar, la lista de permisos en lenguaje llano y qué implica cada uno. Una
actualización que solicita permisos nuevos requiere consentimiento renovado y no se aplica automáticamente.

## 6. Retirada

Una extensión puede ser retirada del registro si:

* Incumple esta política y el autor no corrige en un plazo razonable.
* Resulta maliciosa, en cuyo caso la retirada es inmediata y sin aviso previo.
* Queda abandonada e incompatible, con aviso previo al autor.

Cuando se retira una extensión maliciosa, los usuarios que la tengan instalada deben ser advertidos desde la
aplicación. Retirarla del catálogo sin avisar a quien ya la tiene no resuelve el problema: quien la tiene
instalada la sigue ejecutando.

El registro publica en `retiradas.json` qué ha retirado, con el motivo y
las versiones que afecta; la aplicación descarga **la lista entera** y la compara en el dispositivo. Podría
preguntarse por las extensiones instaladas y sería más barato en bytes, pero le contaría al registro qué tiene
cada usuario, y eso no hace falta enviarlo para que esto funcione.

De las tres causas de retirada, **solo la maliciosa apaga la extensión**; las otras dos avisan. Apagar por todo
enseñaría a reactivar sin leer, y entonces el apagado dejaría de significar algo el día que importe. Y apagar no
es desinstalar: la extensión se queda con sus permisos y sus datos, y quien la quiera de vuelta puede
encenderla. Decidir por el usuario que se borra sería perder algo suyo sin preguntar.

El aviso **se guarda**, así que sobrevive a cerrar la aplicación y a quedarse sin red: una extensión apagada sin
el motivo delante sería una desaparición sin explicación.

## 7. Responsabilidad

El registro no garantiza la calidad ni la seguridad de las extensiones de terceros; aplica controles razonables
y proporcionados. Esta limitación debe estar declarada de forma visible en el registro, sin usarla como excusa
para no aplicar los controles: la sección 3 dice exactamente cuáles son, cuáles se comprueban solos y cuáles no,
y esa lista es la medida de lo que el proyecto sí promete.
