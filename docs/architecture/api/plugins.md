# Especificación de la API de Extensiones

> **Estado:** Vigente. **La API de extensiones es la 1.0.0, estable**; ver [versioning](../versioning.md),
> sección 2.2.

## El manifiesto

> El esquema está publicado en
> [nb-extension.schema.json](nb-extension.schema.json) y sus ejemplos en
> [nb-extension.ejemplos.json](nb-extension.ejemplos.json).

Toda extensión trae un `nb-extension.json`. Existe para que NB Books sepa **qué aportas sin ejecutar ni una
instrucción tuya**: de ahí sale que tus comandos aparezcan en la interfaz antes de que tu código se cargue, y de
ahí sale que a quien te instala se le pueda enseñar lo que va a conceder antes de concederlo.

| Campo | Obligatorio | Qué es |
|---|---|---|
| `id` | Sí | Dominio invertido, minúsculas, **al menos tres segmentos**. Es la clave con la que tu extensión vive en el almacén y aparece en rutas |
| `name` | Sí | El nombre que ve quien la instala |
| `version` | Sí | SemVer completo: tres números, sin ceros a la izquierda y sin sufijos |
| `type` | Sí | `core`, `ui` o `theme` |
| `apiVersion` | Sí | Mayor y menor de la API contra la que compilas. **Tiene que coincidir con la que declara tu módulo** en la sección `nb:api-version` |
| `license` | Sí | Identificador SPDX de tu licencia |
| `capabilities` | No | Lo que pides hacer. Hoy solo existe `network:fetch` |
| `permissions` | No | El detalle de cada capacidad que declaras |
| `contributes` | No | Lo que aportas a la interfaz, de forma declarativa |

**Las dos mitades de un permiso van juntas.** Declarar `network:fetch` en `capabilities` sin detallarlo en
`permissions` deja a quien te instala aprobando algo sin alcance; detallarlo sin declararlo lo deja fuera de la
lista que se le enseña. Las dos cosas se rechazan.

**El comodín no puede estar en el anfitrión.** `https://openlibrary.org/api/*` vale; `https://*/api` no, porque
el permiso se concede por dominio y un comodín ahí sería "toda la red" disfrazada de permiso concreto.

### Qué recibes cuando algo está mal

**Todos los defectos, no el primero.** Quien arregla un campo y vuelve a fallar por el siguiente hace tantas
vueltas como defectos tenga. Cada uno trae un código estable, **el campo concreto** en notación de puntos
—`permissions.network:fetch.urls[0]`— y, cuando ayuda, el valor que se esperaba. Nunca una frase: la frase la
compone la aplicación en el idioma de quien la usa, como hace con los códigos de error.

Los códigos son `NOT_JSON`, `NOT_AN_OBJECT`, `MISSING_FIELD`, `WRONG_TYPE`, `VALUE_NOT_ALLOWED`,
`MALFORMED_VALUE`, `CAPABILITY_WITHOUT_PERMISSION`, `PERMISSION_WITHOUT_CAPABILITY`, `WILDCARD_HOST` y
`API_VERSION_MISMATCH`.

### Cómo se aporta un formato de documento

Se declara en `contributes.providers`, con la clave `format:<tipo-de-medio>` y el módulo como valor:

```json
"contributes": { "providers": { "format:text/x-mi-formato": "formato.wasm" } }
```

Tu componente se compila contra el mundo `format-provider`, que **no importa nada del anfitrión**: un proveedor
de formato no necesita salir a la red, así que no puede. Recibe los bytes del documento —nunca una ruta— y
devuelve sus secciones: identificador, título y texto.

**No devuelves un lector, y eso es deliberado.** El anclaje de posiciones, el saneado y las capacidades los pone
NB Books. Lo que tú dices es de qué está hecho el documento; cómo se lee, no. Es lo que permite que un formato
nuevo tenga las mismas garantías que los que vienen de fábrica sin que tengas que implementarlas.

Hay un ejemplo completo en `extensiones/banco/formato-de-texto/`: un formato de texto con encabezados, en unas
sesenta líneas.

### Cómo se aporta una orden

Se declara en dos sitios del manifiesto, y los dos hacen falta:

```json
"contributes": {
  "commands": [{ "id": "mi.extension.hacer", "title": "Hacer Algo", "category": "Metadatos" }],
  "contextMenus": { "library/document": ["mi.extension.hacer"] },
  "providers": { "command": "modulo.wasm" }
}
```

`commands` dice **qué órdenes existes**; `contextMenus` dice **dónde aparecen**. Declarar una sin colocarla es
válido y no aparece en ninguna parte: sirve para órdenes que solo invocas tú.

Tu componente exporta `run-command`, que recibe el identificador de la orden. Si te llega uno que no declaraste,
devuelve `unknown`: significa que tu manifiesto y tu código discrepan, y decirlo es mejor que hacer algo por si
acaso. **NB Books ya lo rechaza antes de compilarte**, porque lo que quien te instaló aprobó es lo que tu
manifiesto decía.

Puedes devolver un texto corto que la aplicación enseña como aviso. Ese texto es tuyo y **no se traduce**, igual
que el título de tu orden.

**Tu código no se carga hasta que alguien pulsa.** Hasta entonces tu orden está en la interfaz y de ti no hay
compilada ni una instrucción. Por eso el manifiesto es declarativo: poblar la interfaz no puede costar lo que
cuesta tu código.

**Dónde aparece.** En la ficha del documento, no en un menú del botón derecho: ADR-012 prohíbe la acción
exclusiva de ratón, y la ficha ya se alcanza con el teclado.

### Cómo se aporta un tema

Un tema **no tiene código**. Es un manifiesto de clase `theme` con colores y nada más:

```json
"type": "theme",
"contributes": {
  "theme": { "--nb-surface": "#161a22", "--nb-text": "#e8ecf2", "--nb-accent": "#7fb2e5" }
}
```

Un tema que declare proveedores, órdenes, vistas o permisos **se rechaza al validarlo**. No se marca para
revisión: se rechaza. Un tema que trae código no es un tema, y dejarlo pasar obligaría a preguntarse en cada
revisión si esta vez sí.

**Solo puedes cambiar colores, y solo los de una lista cerrada.** Ahí no hay ningún token para apagar el contorno
del foco ni para quitar una sombra: lo que no está en la lista no se toca, así que las garantías de
accesibilidad no dependen de que te acuerdes.

**Y tus colores pasan por el contraste antes de aplicarse.** Los mismos umbrales que el sistema de diseño usa
para sí mismo: 4,5 para lo que se lee, 3 para lo que solo se ve. Si alguno no llega, **no se aplica nada** y se
te dice cuál falla, cuánto midió y cuánto hacía falta. Aplicarlo a medias dejaría texto de tu tema sobre fondos
del anterior, que es exactamente el tramo en el que no se lee.

Eso incluye el foco: como no hay forma de apagarlo, la única manera de esconderlo sería darle el color del
fondo, y el umbral de interfaz lo impide.

Hay un ejemplo en `extensiones/tema-de-prueba/`. Son veinte líneas de JSON.

### Cómo se aporta un modo de lectura

Un modo va en el mismo paquete que un tema —clase `theme`, sin código— y es un **perfil completo**, no dos
valores sueltos:

```json
"contributes": {
  "readingModes": [
    { "id": "pergamino", "name": "Pergamino",
      "background": "#f2e8d5", "text": "#33291b",
      "brightness": 96, "warmth": 45 }
  ]
}
```

Fija fondo, texto, brillo y temperatura a la vez, igual que los seis del núcleo. Uno con dos valores sueltos
obligaría a quien lo elige a componer el resto a mano, que es la fricción disfrazada de libertad que
[ADR-012](../../adr/012-ui-paradigm.md) declara como modo de fallo de esta sección.

**El brillo y la temperatura son emulados**, y la interfaz los llama así: son una capa de composición encima,
no el brillo del panel ni el filtro de luz nocturna del sistema. Van en porcentaje entero de 50 a 100 y de 0 a
100 porque un manifiesto se escribe a mano: `96` no admite las dos lecturas que admite `0.96`.

**Tu modo pasa por el contraste antes de ofrecerse**, con el mismo umbral que los del núcleo: 4,5, el del
texto, porque lo que pinta un modo es el libro. Si no llega, **no aparece en la lista** y se dice cuál falla.
No se ofrece y luego falla: un modo ilegible en la lista es una trampa, porque quien lo elige descubre el
problema justo cuando ya no puede leer para arreglarlo.

Lo que se guarda al elegirlo es `<tu identificador>/<el id del modo>`, para que dos paquetes puedan llamar
igual a su modo sin pisarse. Y si tu extensión se desinstala, esa preferencia **se vacía sola**: dejarla
apuntando a algo que ya no está sería rastro.

Hay un ejemplo completo en `extensiones/propias/personalizacion/`: un tema y tres modos, sin una sola línea de
código.

### Cómo se revisan metadatos por lotes

```json
"contributes": {
  "providers": { "bulk-metadata": "normalizador.wasm" }
}
```

Tu módulo recibe una lista de documentos —lo que ya está en el catálogo local de
**los que el anfitrión elige**— y devuelve las correcciones que propondría. No recorres la biblioteca, no
eliges qué documentos entran y **no escribes nada**: lo que propongas queda como propuesta, una edición manual
gana siempre, y quien la lea decide. Por eso corregir mil documentos de una vez es reversible sin que haya nada
que revertir.

Cada sugerencia lleva su motivo escrito, en una línea, y no se traduce: es lo que se le enseña a quien decide.

El lote se trocea en cien documentos por llamada. No es por rendimiento: tu módulo tiene un plazo por llamada, y
mil de una vez lo agotarían y se perdería el trabajo entero.

Devolver lista vacía es lo normal. La mayoría de los documentos no tienen nada que corregir, y proponer por
proponer enseña a aceptar sin leer.

Hay un ejemplo en `extensiones/propias/normalizador/`: arregla los espacios de más, los títulos en mayúsculas
enteras y el punto final. No adivina nada más, y eso también es parte del ejemplo.

### Cómo se aporta un formato de cita

```json
"contributes": {
  "providers": { "citation": "citas.wasm" },
  "citationFormats": [
    { "id": "bibtex", "name": "BibTeX", "fileExtension": "bib" }
  ]
}
```

Tu módulo recibe una lista de referencias **ya compuesta** y devuelve el archivo entero. No consultas la
biblioteca, no eliges qué documentos entran y no escribes en disco: las citas salen de las anotaciones y del
catálogo, que son del usuario, y tú recibes lo justo para darles forma.

Devuelves el archivo completo y no una línea por referencia porque un archivo bibliográfico tiene cabecera,
orden y separadores propios de cada formato. Trocearlo obligaría al anfitrión a saber de BibTeX, de RIS y de lo
que venga después, que es justo lo que no debe saber.

**Declara el módulo además de los formatos.** Unos formatos sin proveedor aparecerían en la interfaz y fallarían
al pulsarlos, que es la peor forma de fallar; el validador lo rechaza.

Hay un ejemplo completo en `extensiones/propias/citas/`, con BibTeX y RIS.

### Cómo se aporta un fondo sonoro

```json
"contributes": {
  "soundscapes": [
    { "id": "lluvia", "name": "Lluvia", "kind": "rosa" }
  ]
}
```

Doce ambientes y ninguno más: `blanco`, `rosa` y `marron` —ruido puro— y `lluvia`, `tormenta`, `oleaje`,
`viento`, `fuego`, `bosque`, `cafeteria`, `ventilador` y `tren`. **No traes el sonido**: declaras cuál es y lo
sintetiza la aplicación. Es lo que permite que tu extensión no le añada peso al instalador de quien no la
quiere; una grabación de lluvia de diez minutos pesa más que la aplicación entera y trae una licencia que
revisar.

Un fondo sonoro puede ir en cualquier clase de extensión, al revés que un modo de lectura. La diferencia tiene
motivo: un modo decide con qué colores se lee, y eso no puede decidirlo algo con permisos y proveedores; un
fondo sonoro no decide nada, y lo único que puede hacer es sonar cuando alguien lo pone.

**Nada de lo que hace el usuario con él se guarda** —ni cuál sonaba ni a qué volumen—, así que desinstalar tu
paquete no deja rastro sin que haya nada que limpiar. Que la aplicación empezara a hacer ruido sola al abrirla
sería peor que volver a ponerlo.

### Cómo se aporta una vista

Se declara en `contributes.views`, con el módulo bajo la clave `view`:

```json
"contributes": {
  "views": [{ "id": "mi.extension.resumen", "title": "Mi Resumen" }],
  "providers": { "view": "modulo.wasm" }
}
```

Tu componente exporta `render`, que recibe el identificador de la vista y devuelve **bloques tipados**:
encabezado, párrafo, lista y dato con etiqueta. **No devuelves marcado**, y eso es a tu favor tanto como al
nuestro: tu vista tiene el mismo foco, el mismo nombre accesible y el mismo aspecto que las nativas sin que
tengas que ocuparte de ninguna de las tres.

Tu pestaña aparece en Estudio marcada como tuya. **No se esconde de dónde viene**, y tampoco se compone hasta
que alguien la abre: hasta entonces de ti no hay compilada ni una instrucción.

**Si entras en bucle, no congelas la aplicación.** No corres en su hilo: corres en el anfitrión, con un plazo, y
quien esperaba recibe un aviso. No es una cortesía, es el diseño.

Hay un ejemplo en `extensiones/banco/vista-de-estudio/`, con una vista que se porta bien y otra que se cuelga a
propósito.

### Cómo se aporta una fuente de metadatos

Se declara con la clave `metadata` en `contributes.providers`, y tu componente se compila contra el mundo
`metadata-provider`. Ese sí **importa `host`**, al revés que el de formato: consultar una fuente en línea es
salir a la red.

Sales por la puerta del anfitrión con **tu** permiso. Declara los anfitriones que necesitas en `permissions`, y
hasta que se te concedan tu `fetch` recibe `denied` y no llega a ninguna parte.

**Propones; no escribes.** Lo que devuelvas pasa por la procedencia de NB Books igual que el catálogo de fábrica:
una edición manual gana siempre y lo que ya se rechazó no se vuelve a aplicar. Y tus candidatos aparecen en la
lista **con tu identificador**, para que quien elija sepa de dónde salió cada ficha.

Devolver lista vacía no es un error: es no saber. Lo que sí es un error es devolver una ficha sin título, porque
no se puede elegir.

Hay un ejemplo en `extensiones/banco/fuente-de-metadatos/`.

### Cómo se compila una extensión

Contra el mundo de [`nb-extension.wit`](../../../src-tauri/wit/nb-extension.wit), a `wasm32-wasip2`, que es el
objetivo que produce un componente. El generador del lado del invitado es `wit-bindgen`, y el mundo se lee del
propio archivo: no lo copies a tu proyecto, porque el día que cambie tendrías dos versiones distintas.

Hay un ejemplo completo y mínimo en `extensiones/banco/usa-el-mundo/`. Hace una sola cosa —sale a la red y
devuelve el código de estado— y eso es todo lo que hace falta para ver la forma.

**No necesitas escribir nada por la salida estándar para contar cómo te fue.** El mundo devuelve un resultado
tipado: un contrato que obligara a imprimir el resultado obligaría a pedir una capacidad **solo para hablar**.

### Qué pasa con tus permisos

**El consentimiento es tuyo, no del servidor.** Que otra extensión ya tenga permiso para `openlibrary.org` no te
da permiso a ti. Cada extensión se autoriza aparte, y eso es deliberado: si se heredara, quien te instala habría
autorizado algo que nunca vio.

**Antes de instalarte, se le enseña todo lo que podrás hacer**, no solo lo que tu versión nueva añade. Y el texto
que acompaña a `network:fetch` dice que no podemos acotar qué envías, porque tu código no es nuestro.

**Si tu actualización pide un anfitrión nuevo, no entra sola.** Hasta que quien la tiene instalada decida, sigue
puesta la versión anterior. No se instala nada a medias.

**Al desinstalarte se borran tus permisos.** Si alguien vuelve a instalarte más tarde, se le vuelve a preguntar.

**Solo se te puede conceder lo que declaras.** Un anfitrión que no está en tu manifiesto no se puede autorizar
para ti, ni siquiera a propósito.

### El esquema no se escribe a mano

Ni el esquema ni los ejemplos. Los dos los genera la misma tabla de la que sale el validador del anfitrión, y
`scripts/check-esquema.mjs` falla si el archivo publicado deja de ser el que esa tabla produce.

Ese guion hace además algo que a ti te afecta directamente: **ejecuta la expresión regular del esquema sobre los
mismos ejemplos con los que se comprueba la función de Rust**, y falla si no dicen lo mismo. Sin eso, el día que
las dos escrituras de una regla se separaran, tú validarías tu manifiesto contra el esquema publicado, te
saldría bien, y NB Books te lo rechazaría. El coste sería tuyo y la causa nuestra.

## Alcance planificado

1. **Manifiesto:** hecho. Ver arriba.
2. **Puntos de contribución (`contributes`):** comandos, menús contextuales, vistas, proveedores de formato,
   proveedores de metadatos, proveedores de IA y temas.
3. **Bindings WIT:** contratos de interfaz para compilar extensiones de Rust a WebAssembly.
4. **API del sandbox:** llamadas disponibles y cómo se solicitan permisos.
5. **Ciclo de vida:** activación perezosa, desactivación, actualización y desinstalación.
6. **Publicación:** requisitos de auditoría y proceso de envío al registro.

## Compromiso de estabilidad

La API de extensiones es un contrato público. **Los números viven en un solo
sitio**, la [sección 2 de versioning.md](../versioning.md), y este documento no los
repite: dos copias de una cifra son una contradicción esperando a que alguien actualice una sola. Aquí está lo
que ese compromiso significa para quien escribe una extensión.

**Lo que el proyecto se compromete a hacer**

* Decir qué versión de la API habla cada versión de la aplicación, y si es estable: Ajustes, sección Extensiones,
  lo lee del anfitrión en lugar de repetirlo. La 1.0.0 es estable.
* Soportar a la vez la mayor actual y la anterior, de modo que subir de mayor nunca rompe todo el mismo día.
* Dar una ventana de soporte tras cada deprecación, medida a la vez en tiempo de calendario y en versiones
  publicadas.
* Avisar en las notas de la versión, en el registro y **dentro de la aplicación a quien tenga instalada una
  extensión afectada**.
* No destruir nada tuyo cuando una extensión deja de poder cargarse: ni sus datos, ni su configuración, ni su
  sitio en la lista.

**Lo que se te pide a cambio**

* Declarar la versión de API contra la que compilas, en el manifiesto y en la sección personalizada del módulo.
  Si no lo declaras, el anfitrión no puede protegerte de nada.
* Leer qué cuenta como cambio mayor. La tabla de la sección 2.5 incluye una fila que sorprende a mucha gente:
  **cambiar lo que una función hace sin tocar su firma también es mayor**, y se trata como tal, precisamente
  porque tú no puedes verlo compilando.

**Lo que no se te promete**

El contrato IPC interno, el esquema de la base de datos y el aspecto o la posición de las ranuras de interfaz.
Lo que se promete de una ranura es que existe y que lo que montes en ella se monta; dónde queda depende de la
clase de ancho, por [ADR-012](../../adr/012-ui-paradigm.md).

**Si tu módulo es rechazado**, el mensaje trae las tres cosas con las que se arregla: la versión que declara tu
módulo, el rango que admite ese anfitrión, y contra qué versión hay que recompilar.

**Qué de esto lo comprueba una máquina.** La parte de la versión: el anfitrión lee tu
declaración antes de compilar tu módulo y rechaza lo que no habla, con los tres números que necesitas. El resto
—la ventana de soporte, la tabla de cambios mayores, los avisos— lo sostiene la revisión humana de cada cambio.
Está dicho en la sección 2.10 de `versioning.md` y está dicho aquí, porque un compromiso que exagera su propia
solidez es peor que no tenerlo.

## Requisitos mínimos para publicar una extensión

Estos requisitos son parte del contrato, no recomendaciones:

* Incluir un archivo de licencia y respetar las licencias del código de terceros utilizado.
* No ofuscar el código ni cargar código remoto en tiempo de ejecución.
* No incluir telemetría ni publicidad.
* Declarar todos los permisos solicitados y justificarlos en la descripción.
* No usar la marca NB Books de forma que induzca a pensar que es un componente oficial.

La lista completa, con **qué se comprueba solo y qué no**, está en
[governance/extension-policy.md](../../legal/governance/extension-policy.md). Cada requisito tiene identificador
—`R1`, `R2`...— y es el que verás citado si tu envío se detiene.

### Firma tu extensión

Sin firma no se distribuye. Se crea una clave una vez:

```bash
cargo run --features herramientas --bin firmar -- --crear-clave ~/.nb-books/mi-clave.pkcs8
```

y se firma cada vez que cambies algo, porque la firma cubre la huella del paquete entero y deja de valer al
primer byte:

```bash
cargo run --features herramientas --bin firmar -- ruta/a/mi-extension --clave ~/.nb-books/mi-clave.pkcs8
```

Eso escribe `firma.json` dentro de tu paquete. Lo firmado es la huella del contenido, tu identificador y tu
versión, así que tu firma no vale para otra extensión ni para otra versión aunque el contenido sea idéntico.

**Guarda la clave como guardarías una llave.** Quien la tenga puede publicar actualizaciones de tus
extensiones; quien la pierda no puede volver a publicarlas. No hay forma de recuperarla, y no la habrá: una
manera de recuperar tu identidad sin tu clave sería una manera de suplantarte.

**Por qué importa que sea siempre la misma.** La primera vez que alguien instala tu extensión, su NB Books
apunta tu clave. A partir de ahí solo acepta actualizaciones firmadas por ella: ni sin firmar, ni firmadas por
otro. Es lo que hace que nadie pueda entregar una versión maliciosa de *tu* extensión sin robarte la clave
antes.

### Pásale la revisión antes de enviarla

La misma puerta que corre al publicar la puedes correr tú, sobre el directorio de tu extensión:

```bash
cargo run --features herramientas --bin revisar -- ruta/a/mi-extension
```

Devuelve 0 si tu paquete se publica y 1 si no, y en medio te dice qué requisito incumple, en qué archivo y qué
encontró. Con `--json` sale el informe entero, por si lo quieres procesar.

Revisa el manifiesto, la licencia y **todos** los `.wasm` que traiga el directorio, los nombre el manifiesto o
no: lo que viaja dentro del paquete se distribuye. Y te da todos los hallazgos de una vez, no el primero, para
que no tengas que enviar tantas veces como defectos tengas.

Dos avisos que verás y que **no detienen** nada, porque son sobre el alcance de la revisión y no sobre tu
paquete: si envías un componente, la comprobación del código escondido y la de la versión de API no llegan
hasta dentro. Están dichos en el informe en vez de callados, porque un informe limpio que no miró se lee como
una garantía que nadie te dio.
