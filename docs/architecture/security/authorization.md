# Autorización (Authorization)

> **Estado:** Implementado. Capacidades del webview en `src-tauri/capabilities/default.json`; permisos de
> extensiones en `src-tauri/src/extensions/permissions.rs`.

Este documento cubre **qué puede hacer** cada parte del sistema. NB Books tiene dos sujetos de autorización, y
confundirlos es una fuente clásica de vulnerabilidades:

1. El **webview** frente al núcleo Rust.
2. Las **extensiones** frente al anfitrión.

---

## 1. Webview frente al núcleo (Tauri Capabilities)

Tauri v2 modela esto con *capabilities*: un webview que no coincide con ninguna capability no tiene acceso alguno
a la capa IPC.

Reglas del proyecto:

* Cada ventana recibe **solo** las capabilities que necesita. Una ventana de lectura no necesita permisos de
  diálogo de archivos.
* El contenido de terceros (previsualización de EPUB, paneles de extensiones) vive en ventanas o webviews con
  capability propia y mínima.
* Todo comando nuevo debe justificar en el PR por qué debe ser alcanzable desde el frontend.
* El frontend nunca es fuente de verdad sobre permisos: Rust revalida en cada comando.

### Lo que la ventana principal tiene concedido

`src-tauri/capabilities/default.json`: `core:default`, **`core:window:allow-set-fullscreen`** y
**`dialog:allow-open`**. La lectura inmersiva tiene dos niveles y el segundo es la pantalla completa **del sistema**: la concede
el sistema operativo, no la aplicación. Simularla ocultando la carcasa y llamándola pantalla completa sería
exactamente lo que ADR-012 llama disimular en lugar de declarar.

El **diálogo nativo** es la vía por la que una ruta del disco entra al proyecto **con consentimiento por
construcción**: la devuelve el sistema operativo después de que el usuario señale una carpeta en su propio
escritorio, así que no es una ruta que el webview se invente. El runtime la canonicaliza igual, porque una ruta
que llega por el puente no deja de venir del webview por haber salido antes de un diálogo. Escribir la ruta a
mano sigue existiendo, y no es un apaño: es la vía de teclado, y una acción exclusiva del ratón está prohibida.

El **protocolo de assets** está habilitado, con el alcance acotado en `tauri.conf.json`
a los directorios de caché de la aplicación. Sirve las páginas ya rendidas de PDF y de cómic, que es lo que evita
codificarlas en base64 para pasarlas por el canal JSON, prohibido por [data-flows.md](../data-flows.md).
**Ningún archivo de la biblioteca del usuario es alcanzable por esa vía**: lo que se sirve son artefactos que
produjo el runtime, no lo que el usuario importó.

### Superficie IPC

El conjunto de comandos expuesto se documenta en [api/services.md](../api/services.md). Todo comando que
reciba una ruta del sistema de archivos debe:

1. Resolverla canónicamente.
2. Verificar que está dentro de un directorio permitido (raíz de biblioteca administrada, o una ruta
   explícitamente autorizada por el usuario mediante diálogo nativo).
3. Rechazar enlaces simbólicos que escapen de ese ámbito.

---

## 2. Extensiones frente al host (Capabilities declaradas)

1. La extensión declara sus requerimientos en `nb-extension.json`.
2. Al instalarla, el usuario ve un consentimiento explícito que enumera los permisos en lenguaje llano.
3. El núcleo Rust registra las capabilities concedidas. Si el módulo WASM intenta una operación no concedida, la
   llamada se bloquea en el host.
4. Los permisos son revocables en cualquier momento y se revalidan cuando la extensión se actualiza y pide más de
   lo que tenía.
5. El permiso de red se concede por dominio, no de forma global.
6. **Y por extensión.** Conceder a una extensión salida a un anfitrión no se la concede a ninguna otra.

### 2.1 Cómo se cumple

**La tercera pregunta del consentimiento.** El consentimiento de red responde a dos: a dónde y para qué. Las
extensiones obligan a una tercera, **quién**, y sin ella el modelo se cae: conceder a una extensión
salida a `openlibrary.org` la concedería a la vez a todas las demás, presentes y futuras, porque el permiso
viviría pegado al anfitrión. Instalar una segunda heredaría en silencio lo que el usuario aprobó para la primera.
La migración V21 añade el origen a la tabla de consentimientos y al registro de salidas.

**Por la misma puerta.** La red de una extensión sale por `net.rs`, igual que el catálogo, las portadas, la
consulta y la voz. No hay una segunda puerta: sería una segunda superficie de auditoría, y
`scripts/check-network.mjs` la rechazaría de todos modos.

**Qué se le enseña al usuario, y por qué así.** La lista incluye **todo** lo que la extensión podrá hacer, no
solo lo que añade una actualización: decidir sobre la diferencia sin ver el conjunto es decidir a ciegas. Y el
texto de qué se envía dice lo que se puede prometer y nada más —*"lo que la extensión decida enviar, que no
podemos acotar"*—, porque su código no es nuestro. Inventar una lista concreta sería mentir.

**Una sola puerta de entrada.** `permissions::instalar` es el único camino por el que una extensión entra con
permisos. Si pide algo que no tiene concedido no se instala **nada**: ni contenido, ni generación nueva. La
versión que ya estaba sigue puesta hasta que el usuario decide.

**Desinstalar se lleva los permisos.** Si sobrevivieran, reinstalarla más tarde saldría a la red sin volver a
preguntar, y el usuario habría concedido a algo que ya no recuerda.

**Solo se puede conceder lo que el manifiesto pide.** Un anfitrión que la extensión no declara no se puede
autorizar: un permiso que no corresponde a nada que el usuario haya visto en la lista no es un permiso.

---

## 3. Datos del usuario frente a terceros

Ningún tercero, institución incluida, recibe anotaciones, notas, progreso ni la biblioteca personal: la aplicación
no tiene ningún canal que los envíe. Lo único que sale del equipo es lo que la persona autoriza por anfitrión y
propósito, descrito en [privacy.md](../../audit/privacy.md).
