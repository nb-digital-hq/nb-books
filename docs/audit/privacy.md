# Privacidad y Telemetría (Privacy by Design)

> **Estado:** Implementado. La única puerta de salida está en `src-tauri/src/net.rs`, con consentimiento por
> anfitrión, propósito y origen y registro de salidas; no hay telemetría; la exportación completa y el borrado por
> alcances existen. La declaración vinculante en inglés es `PRIVACY.md`, en la raíz del repositorio.
> **Constitución, regla 3:** los hábitos de lectura son un dato íntimo.

Este documento define qué datos existen, dónde viven y qué sale del dispositivo. El cifrado que los protege se
describe en [encryption.md](../architecture/security/encryption.md); las obligaciones legales derivadas, en
[legal/data-protection.md](../legal/data-protection.md).

## 1. Inventario de datos

| Categoría | Ejemplos | Ubicación | ¿Sale del dispositivo? |
|---|---|---|---|
| Contenido | Archivos EPUB, PDF y CBZ | Carpetas del usuario | Nunca |
| Metadatos de biblioteca | Título, autor, formato, rutas relativas | SQLite local | Solo el título o identificador que se consulta, con el propósito `metadata` |
| Anotaciones y notas | Resaltados, notas, tableros, vocabulario | SQLite local | Nunca |
| Progreso y sesiones | Posición, tiempo de lectura | SQLite local | Nunca |
| Consultas a IA | Petición y fragmentos de contexto | Proveedor externo, si se elige | Solo con proveedor externo y consentimiento |
| Diagnósticos | Registros redactados | Carpeta de datos (`logs`) | Solo si el usuario guarda y entrega el archivo |
| Metadatos de red | Peticiones a catálogos, portadas, consulta | Terceros consultados | Sí, y se registran en `network_log`, visible en Ajustes |
| Lo que comparte NB Books Mobile | Libros elegidos, anotaciones, progreso, notas, vocabulario y repasos | Teléfonos emparejados en la red local | Solo hacia los teléfonos que la persona empareja, y solo lo que elige; nunca a internet. Ver [contrato móvil](../architecture/mobile-contract.md) |

## 2. Telemetría cero por defecto

* No hay analítica de terceros embebida ni SDKs de seguimiento.
* El diagnóstico se prepara solo cuando la persona lo pide, con vista previa del contenido, y nunca se envía solo.
* Toda petición de red saliente es observable desde la interfaz: el usuario debe poder saber qué salió,
  hacia dónde y por qué.

## 3. Consentimiento

El consentimiento en NB Books es **granular, revocable y verificable**:

* Granular: catálogos, metadatos en línea, consulta, voz, modelos, IA externa y actualizaciones son decisiones
  independientes.
* Revocable: revocar debe detener el flujo y ofrecer el borrado de lo ya enviado cuando sea técnicamente posible.
* Verificable: Ajustes enumera los consentimientos activos y el registro de salidas.
* No se usan patrones oscuros: la opción que preserva la privacidad nunca está oculta ni penalizada.

## 3.1 La única puerta de salida

El innegociable "nada sale del dispositivo sin consentimiento explícito" no puede descansar en la ausencia de código
de red: esa garantía dura hasta la primera petición. Lo hacen cumplir cuatro cosas.

**1. Una sola puerta, y se comprueba.** Toda petición saliente pasa por `net.rs`. `scripts/check-network.mjs`
rechaza cualquier uso del cliente HTTP fuera de ese archivo, y se ejecuta en `pnpm verify` y en el gancho de
pre-commit. Es el mismo mecanismo que mantiene el puente IPC dentro de `$core/ipc`, y por el mismo motivo: una
regla que solo vive en la cabeza de quien programa no es una regla.

**2. Consentimiento por anfitrión, por propósito y por origen.** Autorizar un servidor no es autorizarlo para
todo. A un catálogo al que se le permite servir libros no se le permite por ello recibir el texto de una selección
para traducirlo. Los propósitos son una lista cerrada —catálogo, catálogo sin cifrar, metadatos, consulta,
actualización, **registro**, **voz**, **extensión**, **modelo** e **IA**— y cada uno declara **qué envía**, en lenguaje
llano, junto al propio propósito, para que la explicación no se quede vieja cuando el propósito cambie. **La
lista vive en un solo sitio**, `net::Purpose::TODOS`, y una prueba exige que cada propósito se pueda leer desde el
puente y tenga su etiqueta en los tres idiomas.

**El propósito `modelo` trae cientos de megabytes y no envía nada de la biblioteca**: *"solo se pide el
archivo del modelo, y quien lo sirve sabe que lo descargas"*. Lo segundo se dice porque es verdad y porque es lo
que hay que poder sopesar. Una descarga **sigue las redirecciones pidiendo el permiso de cada anfitrión**, y eso
importa aquí más que en ningún otro sitio: `huggingface.co` redirige a su red de distribución, que tiene otro
nombre por región, y autorizar el primero no autoriza la segunda. Y durante la transferencia la puerta **no
retiene ninguna conexión de la base**: pide una para comprobar y otra para anotar, porque una descarga dura minutos
y el pool tiene cuatro.

**El propósito `ai` manda texto del libro a un modelo de lenguaje que el usuario configuró**: *"los
fragmentos del documento que la función necesite y lo que pidas, nunca el documento entero; quien lo recibe puede
guardarlo según su propia política"*. Va aparte de `voz` aunque los dos manden texto, porque no se manda lo mismo ni
a quien se manda lo mismo, y porque salir del dispositivo con la IA es una elección que no puede venir de haber
autorizado otra cosa. **El primer envío a cada anfitrión se enseña entero antes de autorizarlo**, y lo que sale
queda registrado aparte, con el contenido: ver [ai/privacy.md](../architecture/ai/privacy.md) sección 5.

**El propósito `registro` es el único que declara que no envía nada**. Sirve para preguntarle al
registro de extensiones qué ha retirado, y lo que envía es, literalmente, *"nada sobre lo que tienes instalado:
solo se pide la lista"*. Podría haberse hecho al revés —mandar los identificadores instalados y recibir solo lo
que afecta— y habría costado menos bytes; lo que habría costado es contarle al registro qué tiene cada usuario.
Se descarga la lista entera y se compara en el dispositivo. Va aparte de `actualización` porque lo que se envía
no es lo mismo, y el consentimiento se da leyendo justo eso.

**El origen** existe por las extensiones: sin él, conceder a una extensión
salida a un anfitrión se la habría concedido a todas las demás, presentes y futuras. El registro de salidas dice
también quién salió, así que ver que algo salió y no saber qué lo mandó deja de ser posible. Vacío significa la
propia aplicación.

**El propósito `extension` dice lo que se puede prometer y nada más:** *"lo que la extensión decida enviar, que no
podemos acotar"*. Su código no lo escribimos nosotros, así que enumerar lo que envía sería inventarlo.

**La voz es el propósito que más manda, y por eso está solo.** Lo que sale con él es el texto del libro, párrafo
a párrafo, mientras se escucha; no un identificador ni una selección. Tiene tres cosas que ninguno de los otros
cuatro necesitaba:

* **Se apaga de fábrica y no hay servidor puesto.** El interruptor viene en local y el campo del servidor,
  vacío. Elegir por el usuario a quién le manda el texto de sus libros no nos corresponde, y sin servidor no se
  ofrece nada.
* **Escribir el servidor no lo autoriza.** Es la misma separación que los catálogos: una cosa es anotar una
  dirección y otra conceder el permiso, y el botón de conceder está aparte y desaparece cuando ya se concedió.
* **Se dice de dónde viene cada vez que se abre el panel de voz.** Un permiso que se concede una vez y no se
  vuelve a ver deja de ser informado, así que la procedencia se enseña donde se usa y no solo donde se
  configuró.

Y hay una excepción de transporte que solo existe por esto: **`http` en claro se admite dentro de la propia
red** —la propia máquina y las direcciones privadas de RFC 1918, enlace local y sus equivalentes de IPv6— y
**nunca hacia internet**. Permite apuntar la voz a un servidor de síntesis propio sin montar una autoridad
certificadora para hablar con una máquina que está a un metro.

El caso real es un servidor en una máquina virtual o en otro equipo de la casa. Exigirle `https` no produce más
seguridad: produce que la función no se use, y que quien la quiera acabe buscando una salida peor.

Las dos mitades de esa excepción no son iguales, y la interfaz **lo dice en lugar de callarlo**: hacia la propia
máquina no hay red que escuche; hacia `192.168.x.x` la hay, y el texto del libro viaja sin cifrar por ella.
Ajustes advierte de eso al escribir una dirección de red, porque un permiso que no explica lo que concede no es
un permiso informado.

Y solo por **dirección numérica**, nunca por nombre. Un nombre lo resuelve el DNS, y lo que hoy apunta a una
dirección privada mañana apunta a otra: admitir `mi-servidor.example` en claro sería dejar que quien controle
esa zona decida a dónde va el texto.

**3. Registro de lo que salió, y de lo que se intentó.** Cada intento queda anotado con su anfitrión, su
propósito, su resultado, el estado y el tamaño. También los denegados: un rechazo es justo lo que el usuario
querría poder ver. **No se guarda la ruta ni la consulta**, porque una ruta puede llevar dentro el título de un
documento o el texto que se seleccionó; es la misma regla de redacción que rige los registros de la aplicación.
El registro se poda a las mil líneas más recientes.

**4. Un interruptor general.** `offline_mode` corta todo, incluso lo que ya tenía consentimiento. Viene
**apagado** de fábrica, y eso no es una concesión: la garantía real es el consentimiento, sin el cual no sale
nada de todos modos. Encenderlo de partida añadiría una segunda puerta que abrir sin proteger de nada más.

Reglas que se derivan y no se negocian:

* **`https`, salvo dos excepciones que se dicen.** Una dirección en claro deja el destino y el contenido a la
  vista de cualquiera en la red. La primera excepción es la red propia, arriba. La segunda son **los catálogos**
: un catálogo que solo se sirve en claro se admite con **su propio
  consentimiento**, el propósito `catalog-cleartext`, que dice qué queda a la vista -la dirección, lo que se
  busca y lo que se descarga-. Tener permiso para el mismo anfitrión por `https` no lo concede, y al añadir la
  dirección Ajustes ya avisa. Lo que sale con el texto del usuario -la voz, las consultas- y lo que manda una
  extensión **siguen sin poder salir en claro hacia internet**: un catálogo no envía nada del usuario más allá de
  qué catálogo mira, y el texto de un libro sí.
* **Una redirección vuelve a pasar por la puerta.** Se sigue, porque casi todo servidor real redirige, pero el
  anfitrión nuevo necesita su permiso, y **la interfaz recibe cuál es** para poder pedirlo. De cifrado a claro no
  se redirige nunca. El actualizador es la única excepción acotada: el gesto de buscar actualizaciones concede el
  propósito `update` a `github.com` y `*.githubusercontent.com`, y a ningún otro anfitrión.
* **El anfitrión se normaliza** —sin puerto, sin credenciales, en minúsculas— para que un mismo servidor no
  pida consentimiento varias veces según cómo se escriba su dirección.

## 4. Derechos del usuario sobre sus datos

* **Exportación:** el usuario puede exportar biblioteca, anotaciones y progreso en un formato abierto y
  documentado, sin pérdida de información (`portability::export`, `save_library_export`).
* **Borrado:** desde Ajustes (`Dónde Viven tus Libros`), el usuario puede borrar en una sola transacción
  inmediata (`crate::db::write_transaction`):
  1. **El historial de lectura** (`clear_reading_history`: `reading_sessions` y `progress`, más el vaciado en memoria de
     `ProgressTracker`), conservando el catálogo, las anotaciones, las notas, el vocabulario y los ajustes.
  2. **Las anotaciones y las notas** (`clear_annotations_and_notes`: `annotations`, `notes`, `note_links`,
     `note_annotation_links` y `note_boards`), conservando el catálogo, el historial de lectura, el vocabulario y los
     ajustes.
  3. **La biblioteca entera en la aplicación** (`clear_library`: catálogo, raíces referenciadas, colecciones personales,
     historial, anotaciones, notas, vocabulario e índices de búsqueda), conservando los ajustes, diccionarios, catálogos
     OPDS y permisos de red.
  Cada alcance explica antes qué se borra y qué se conserva, declara explícitamente que **los archivos del usuario en el
  disco no se borran nunca**, y exige confirmación antes de ejecutarse (`tests/borrado_de_datos.rs`).
* **Portabilidad:** el formato de exportación debe permitir migrar a otra herramienta. Retener al usuario por
  incompatibilidad deliberada contradice la filosofía del proyecto.

## 5. Menores de edad

La aplicación puede usarse en contextos educativos con menores. Como no recoge ni transmite datos de uso, no hay
perfilado ni publicidad, y la inferencia local permite usar las funciones de estudio sin enviar texto a terceros.
Las obligaciones de quien despliega están en [legal/data-protection.md](../legal/data-protection.md).
