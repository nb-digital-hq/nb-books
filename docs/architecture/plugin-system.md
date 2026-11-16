# Sistema de extensiones

> **Estado:** Implementado. Anfitrión en `src-tauri/src/extensions.rs` y `src-tauri/src/extensions/`; mundo WIT en
> `src-tauri/wit/nb-extension.wit`; puerta de revisión en `src-tauri/src/bin/revisar.rs`; firma en
> `src-tauri/src/bin/firmar.rs`. La referencia para autores está en [api/plugins.md](api/plugins.md).
> **Inspiración arquitectónica:** Zed (cajón de arena WASM), VS Code (contribuciones declarativas), NixOS
> (almacén inmutable).

El núcleo de NB Books es deliberadamente pequeño: formatos adicionales, fuentes de metadatos, formatos de cita,
temas, modos de lectura, órdenes y vistas llegan por un sistema de extensiones seguro y modular.

## 1. Clases de extensión

| Clase | Qué es | Ejecuta código |
|---|---|---|
| **Lógica (`core`)** | Módulos WebAssembly que aportan proveedores: formato de documento, metadatos, metadatos por lotes, citas, órdenes y vistas | Sí, en el cajón de arena |
| **Personalización (`theme`)** | Colores de tema, modos de lectura y fondos sonoros, declarados en el manifiesto | No |

Una extensión no ejecuta JavaScript en la interfaz: **devuelve bloques tipados y la interfaz la pinta el núcleo**
(sección 2.2).

Un proveedor de formato declara `format:<tipo-de-medio>` en `contributes.providers` y se registra al arrancar junto
a los formatos de fábrica; el registro del lector no distingue de dónde sale cada fábrica. Ver
[reader-contracts.md](reader-contracts.md).

## 2. Ejecución

### 2.1 Cajón de arena WebAssembly (Wasmtime)

El anfitrión ejecuta **módulos sueltos** de `wasm32-wasip1` y **componentes** de `wasm32-wasip2` compilados contra
el mundo WIT `nb:extension@1.0.0`. Los dos caminos conviven a propósito: los componentes son por donde entran las
extensiones reales, y los módulos sueltos son el camino que usaría alguien que intenta salirse, así que el banco
adverso de `extensiones/banco/` se escribe con ellos.

* **Contexto WASI vacío de capacidades:** ni un directorio preabierto, ni variables de entorno, ni argumentos, ni
  descriptores heredados. Existe porque la biblioteca estándar de Rust enlaza `wasi:io/poll` y sin él ningún
  componente compilado desde Rust llegaría a instanciarse.
* **Dos mecanismos de contención.** Una función que el anfitrión implementa sin recurso (por ejemplo `sock_accept`
  sin ningún socket) falla con un código de error. Una función que el anfitrión no ofrece se detecta **antes de
  instanciar**, auditando las importaciones, y se devuelve con su nombre.
* **Plazo por llamada en tiempo de pared**, por interrupción de época: lo que se protege es el fotograma de quien
  lee, y un módulo puede consumir poco combustible mientras espera.
* **Límite de memoria con su propia marca**, para distinguir quedarse sin memoria de un fallo del autor.
* **La salida estándar no se hereda**: se queda en un búfer acotado. Heredarla metería lo que escriba una extensión
  en el registro de la aplicación sin pasar por la redacción.
* **Versión de API declarada en el binario** (sección personalizada `nb:api-version`). Se lee de los bytes antes de
  compilar; un módulo fuera del rango soportado, o que no declara nada, no se ejecuta. Ver
  [versioning.md](versioning.md).
* **Las funciones del mundo WIT son pocas a propósito**, y ninguna se ofrece sin un permiso detrás:
  `scripts/check-capacidades.mjs` cruza el mundo con las capacidades que admite el esquema publicado.

### 2.2 Interfaz: bloques tipados

Una extensión devuelve bloques tipados (encabezado, párrafo, lista y dato con etiqueta) y la interfaz los pinta con
sus propios componentes. Frente a ejecutar marcado ajeno, esto garantiza que:

* el **sistema de diseño** y las **garantías de accesibilidad** siguen siendo del núcleo;
* **no hay nada que sanear**;
* **una extensión que entra en bucle no congela la aplicación**: corre en el anfitrión con su plazo, y la interfaz
  solo espera una respuesta.

La carcasa ofrece ranuras en los puntos de extensión de [ui-components.md](ui-components.md); una extensión aporta
contenido a una ranura, no redefine la disposición.

### 2.3 Temas: la contribución que no ejecuta nada

Un tema es un manifiesto de clase `theme` con colores, y nada más. Un manifiesto que se llame tema y declare módulo,
permisos, órdenes, vistas o proveedores **se rechaza al validarlo**. Por eso un tema no puede tener un defecto de
ejecución.

* **Lista cerrada de tokens** (`TOKENS_DE_TEMA`, catorce colores). No existe ningún token para apagar el foco,
  quitar una sombra o cambiar un tamaño o un punto de corte: la garantía de foco visible de
  [ADR-012](../adr/012-ui-paradigm.md) no depende de que una validación se acuerde de comprobarla.
* **Solo `#rrggbb`.** Sin transparencia: un token traslúcido rompe el contraste sin que se vea en el manifiesto.
* **El contraste se mide en la interfaz** con `src/lib/ui/contraste.ts`, el mismo código que mide los temas del
  núcleo (4,5 para lo que se lee, 3 para lo que solo se ve). Se aplica entero o no se aplica, y si un par falla se
  dice cuál, cuánto midió y cuánto hacía falta.
* Se elige en Ajustes junto a los temas del núcleo, con su miniatura.

### 2.4 Modos de lectura

Van en un paquete de clase `theme` y aportan un **perfil completo**: fondo, texto, brillo y temperatura. El brillo
y la temperatura se escriben como porcentaje entero. El contraste del texto (4,5) se comprueba **antes de ofrecer**
el modo. Al desinstalar la extensión, `personalization::olvidar` vacía la preferencia que la nombraba en la misma
operación; reinstalar no la restaura.

### 2.5 Formatos de cita

El dato va **del núcleo a la extensión**: el anfitrión compone las referencias a partir de las anotaciones y el
catálogo, y la extensión recibe lo justo para escribir el archivo entero. No consulta la biblioteca ni escribe en
disco.

### 2.6 Fondos sonoros

Una extensión declara **qué ambiente** (`CLASES_DE_RUIDO`) y el núcleo lo sintetiza: el paquete no lleva ni un
byte de audio. Cada ambiente es una receta de ruido de base, capas de filtro con vaivén lento y granos cortos
horneados en un búfer de doce segundos que se repite. Lo periódico va a nodos de audio y lo aleatorio al búfer, así
que mientras suena no cuesta fotogramas. Todos se nivelan al mismo volumen respetando el pico. No se guarda cuál
sonaba ni el volumen: al abrir la aplicación no suena nada hasta que se elige.

## 3. El manifiesto (`nb-extension.json`)

El esquema publicado está en [api/nb-extension.schema.json](api/nb-extension.schema.json) y se **genera** desde la
tabla `CAMPOS` de `src-tauri/src/extensions/manifest.rs`, la misma que usa el validador.
`scripts/check-esquema.mjs` ejecuta el esquema y el validador sobre los mismos ejemplos.

```json
{
  "id": "lat.nbdigitallabs.books.metadata.openlibrary",
  "name": "Open Library Metadata Provider",
  "version": "1.0.0",
  "type": "core",
  "apiVersion": "1.0",
  "license": "MIT",
  "capabilities": ["network:fetch"],
  "permissions": {
    "network:fetch": { "urls": ["https://openlibrary.org/api/*"] }
  },
  "contributes": {
    "commands": [
      { "id": "openlibrary.fetch", "title": "Buscar metadatos en Open Library", "category": "Metadata" }
    ],
    "contextMenus": { "library/document": ["openlibrary.fetch"] },
    "providers": { "metadata": "wasm/openlibrary.wasm" }
  }
}
```

El espacio de nombres `lat.nbdigitallabs.books.*` está reservado a las extensiones de primera parte.

## 4. Almacén, generaciones y activación

El almacén vive bajo `<datos>/extensiones/` (`src-tauri/src/extensions/store.rs`):

1. **Almacén direccionado por contenido:** cada extensión se guarda en una carpeta nombrada por su hash.
2. **Inmutabilidad:** los archivos del almacén no se modifican.
3. **Generaciones:** `active_profile.json` lista los hashes activos.
4. **Vuelta atrás atómica:** restaurar una generación es apuntar el perfil a la anterior.

### 4.1 El punto de compromiso

Toda la instalación se compromete en **una sola escritura**, la de `active_profile.json`, por renombrado atómico.
Lo anterior (copiar el contenido bajo su hash) es aditivo y lo posterior (archivar la generación) es derivado. Una
instalación interrumpida solo puede dejar una carpeta sin referencia, que retira la recolección, o una generación
sin su archivo histórico, que se reconstruye. La suite lo comprueba **matando un proceso real** en cada punto
(`store::PUNTO_DE_MUERTE_ENV`).

### 4.2 Lo que se comprueba antes de copiar nada

Un paquete no entra si:

* no habla una versión de API admitida;
* su firma no es la del autor que ya se tenía (sección 5.1);
* le falta algún módulo que declara o alguno no es WebAssembly. En ese caso la instalación devuelve `Incomplete`
  con los nombres de los módulos que faltan, y se comprueba **antes que los permisos**.

La ruta del módulo la escribe un tercero, así que se acota con `security::resolve_within` a la carpeta de esa
extensión en el almacén.

### 4.3 Activación perezosa

Al arrancar se leen el perfil y los manifiestos y **no se compila ningún módulo**: `Registro::cargar` no recibe un
motor. Cargar el registro de veinte extensiones cuesta 1,04 ms; compilar y ejecutar una sola, 162 ms. Compilar las
veinte al arrancar costaría unos 3,2 s, dieciséis veces el presupuesto de arranque.

* **Desactivar no es desinstalar:** una extensión desactivada conserva sus permisos.
* **Una extensión rota no impide arrancar:** si su manifiesto no se puede leer, se omite y se anota.

### 4.4 El hash

* La ruta relativa entra en la huella, no solo los bytes: mover un módulo de carpeta cambia el hash.
* Se guarda en hexadecimal sin prefijo, porque es el nombre de una carpeta y los dos puntos no son válidos en
  Windows.

## 5. Registro de extensiones

La seguridad del ecosistema descansa en tres capas:

1. **Aislamiento técnico:** Wasmtime y capacidades declaradas y consentidas.
2. **Revisión automatizada de cada versión publicada**, con una puerta ejecutable en local:
   `cargo run --features herramientas --bin revisar -- <paquete>`.
3. **Política y revisión manual** de las extensiones destacadas o reportadas:
   [extension-policy.md](../legal/governance/extension-policy.md).

### 5.1 La firma del autor

Ed25519, verificado con `ring` (`src-tauri/src/extensions/signature.rs`). Se firma la **huella del paquete**
(SHA-256 de rutas y contenido) junto al identificador, la versión y un prefijo de propósito: una firma legítima no
se puede mover a otra extensión ni a otra versión. La firma no entra en la huella, así que firmar no cambia la
dirección del paquete en el almacén.

**La confianza viene de que sea la misma clave de siempre.** El almacén fija la clave del autor en el perfil al
instalar, y una actualización firmada por otra clave, o sin firmar, no entra. La clave fijada vuelve atrás con su
generación. La primera instalación de una extensión es un acto de confianza en su origen; instalar sin firma desde
el disco propio está permitido y la interfaz lo dice en la fila, pero **nunca se actualiza con algo sin firmar lo
que sí estaba firmado**.

### 5.2 La puerta de revisión

`src-tauri/src/extensions/review.rs` revisa el manifiesto, la licencia y **todos** los artefactos WebAssembly del
paquete, y devuelve todos los hallazgos citando el requisito de la política que incumple.

**Detiene:** una dirección de la que traer código escrita en el artefacto, un módulo empotrado dentro de otro, la
importación de una función que ninguna capacidad concede, capacidades declaradas que el paquete no puede ejercer,
la falta de la licencia, el espacio de nombres reservado en un envío de terceros, el nombre del producto en el
nombre visible, una versión de API que el anfitrión no habla, una discrepancia entre la versión del manifiesto y la
del binario, un manifiesto inválido y un `.wasm` que no lo es.

La defensa real contra la carga remota es estructural: el anfitrión no ofrece ninguna función para cargar,
compilar ni instanciar código. La puerta detecta la intención escrita en los bytes. El banco tiene el mismo módulo
en dos versiones que difieren en una sola cosa, y la prueba exige que una se detenga y la otra pase.

### 5.3 Retirada

El registro publica `retiradas.json` con lo retirado, el motivo y las versiones afectadas
(`src-tauri/src/extensions/withdrawals.rs`, migración V22). La aplicación descarga la lista **entera** y compara en
local: preguntar por lo instalado le contaría al registro qué tiene cada usuario.

* **Solo lo malicioso se apaga**; el incumplimiento sin corregir y el abandono avisan. Un motivo desconocido se trata
  como incumplimiento.
* **Apagar no es desinstalar**, y el aviso se guarda para que sobreviva al reinicio.
* La dirección del registro se configura en Ajustes (`registry_host`), y la consulta sale por la puerta de red con el
  propósito `registry`, que no envía nada sobre lo instalado.

## 6. La frontera entre núcleo y extensión

### 6.1 El criterio

Algo es **núcleo** si cumple **las cuatro** condiciones; si falla una, es extensión o componente opcional.

| # | Condición | Pregunta |
|---|---|---|
| 1 | **Está en la promesa** | Sin ello, ¿la aplicación deja de hacer lo que su documentación dice que hace **para todo el mundo**? |
| 2 | **No depende del hardware** | ¿Funciona igual en el equipo más modesto soportado? |
| 3 | **Cabe en el instalador base** | ¿Su peso deja el instalador dentro de su presupuesto para quien no lo usará? |
| 4 | **No impone nada a quien no lo eligió** | ¿Trae una licencia, un derecho de terceros o una superficie de red que alguien aceptaría sin pedirla? |

Las cuatro preguntan lo mismo desde cuatro ángulos: **qué le cuesta a quien no lo quiere**.

### 6.2 Aplicado

| Función | 1 | 2 | 3 | 4 | Dónde va |
|---|---|---|---|---|---|
| Motor de EPUB | - | - | - | - | Núcleo |
| Motor de PDF (PDFium) | - | - | - | - | Núcleo: pesa, pero cabe, y sin él no se lee un PDF |
| Lector de diccionarios StarDict | - | - | - | - | Núcleo |
| Las obras de diccionario | x | - | x | x | Se instalan aparte; cada obra tiene su licencia y su peso |
| Voz del sistema | - | - | - | - | Núcleo |
| Voz neuronal local | x | x | x | x | Componente opcional de primera parte |
| Motor de inferencia local | x | x | x | x | Componente opcional de primera parte |
| Proveedor de metadatos académicos | x | - | - | - | Extensión |
| Temas y modos de lectura | x | - | - | - | Extensión, sobre el mecanismo |
| Exportación de citas | x | - | - | - | Extensión |
| Fondo sonoro | x | - | x | x | Extensión |
| Editor de metadatos por lotes | x | - | - | - | Extensión |
| Tableros de notas | x | - | - | - | **Núcleo**: ver 6.4 |

### 6.3 Motores nativos

El cajón de arena no puede alojar un motor nativo: prohíbe bibliotecas nativas por diseño y no alcanza la GPU. Los
motores nativos se distribuyen como **componentes opcionales de primera parte**:

* Los trae la aplicación, verificados por hash y por la cadena de [supply-chain.md](../audit/supply-chain.md).
* **Los pesos de modelos** se descargan del repositorio de su autor a una revisión fija, y la huella SHA-256 de cada
  archivo viaja **compilada en el binario** (`src-tauri/src/ai/catalogo.rs`). Solo se admiten formatos que no
  ejecutan código al abrirse.
* No pasan por el cajón de arena: su garantía es la procedencia, como la de PDFium.
* Son desinstalables, y sin ellos la función **se oculta** en lugar de fallar.

**El registro no distribuye código nativo de terceros. Nunca.** No existe una segunda clase de extensión con
permisos elevados: un control que vale para unos paquetes y no para otros no es un control.

### 6.4 El caso de los tableros de notas

Por la tabla serían extensión, pero dos razones los ponen en el núcleo:

* **Sus datos son las notas del usuario.** Una extensión que dibujara el tablero necesitaría el grafo entero de
  notas y una vía para escribir posiciones: sería la clase con permisos elevados que no existe.
* **Un tablero es interacción, no contenido.** Arrastrar, mover con el teclado y guardar dónde quedó cada cosa es la
  interfaz misma, y una extensión aporta bloques, no interfaz.

### 6.5 Qué puede hacer un tercero

Todo menos traer un binario: publicar proveedores en WebAssembly que hablen con un motor ya instalado o con un
servicio externo por su permiso de red, y aportar formatos, metadatos, órdenes, vistas, temas y modos de lectura.
Un motor de OCR o de inferencia nuevo solo entra como componente de primera parte.
