# Privacidad en el Subsistema de IA

> **Estado:** Implementado en `src-tauri/src/ai/externo.rs`, `ai/envios.rs` y la migración V32. La sección 6 dice
> cómo se cumple cada regla.
> Complementa [security/privacy.md](../../audit/privacy.md) con las reglas específicas de inferencia.

## 1. Proveedores y flujo de datos

| Proveedor | Dónde se procesa el texto | Requisito |
|---|---|---|
| Local | En el dispositivo | Ninguno adicional |
| Cloud con clave del usuario | Servidores del proveedor elegido | Consentimiento explícito por proveedor |
| Servidor propio | Un servidor compatible que la persona o su organización levanta | Consentimiento explícito por anfitrión |

El proveedor activo debe ser visible de forma permanente en la interfaz de IA, no escondido en Configuración.

## 2. Reglas de envío a proveedores externos

1. Nunca se envía el documento completo.
2. Solo se envían los fragmentos recuperados necesarios para responder, más la consulta del usuario.
3. Antes del primer envío a un proveedor nuevo, se muestra qué se va a enviar, con posibilidad de inspeccionarlo.
4. Se registra localmente qué se envió y cuándo, para que el usuario pueda auditarlo.
5. Las claves de API se guardan en el almacén de credenciales del sistema, nunca en SQLite ni `localStorage`.

## 3. Metadatos que también son datos personales

Aunque no se envíe el texto, la actividad de IA revela información sensible: qué se lee, cuándo y con qué
dificultad. Por tanto:

* Las consultas de IA no se envían a ningún servicio del proyecto ni a terceros, salvo al proveedor externo elegido.
* El historial de IA es local, exportable y borrable por el usuario.
* Ninguna métrica derivada de la actividad de IA se comparte de forma individualizada.

## 4. Entrenamiento

NB Books no entrena modelos con datos de usuarios. Si en el futuro se contemplara cualquier forma de recolección
con ese fin, requeriría un ADR aprobado, consentimiento explícito por separado y una actualización de la política
legal. Por defecto, la respuesta es no.

Al usar un proveedor cloud, el proyecto no controla las políticas de retención de ese tercero. La interfaz debe
advertirlo y enlazar a la política del proveedor antes de la activación.

## 5. Cómo se cumple cada regla

**Protocolos: el compatible con OpenAI y el nativo de Anthropic.** El primero porque es el que hablan también los
servidores que se levantan en la propia máquina o en la de una organización, y el segundo porque Anthropic no habla el primero de forma nativa y adaptarlo por un intermediario
sería mandar el texto a un tercero más. Se configuran en Ajustes, sección "IA y Modelos".

| Regla | Mecanismo | Nota |
|---|---|---|
| Proveedor activo visible siempre (sección 1) | Arriba de "IA y Modelos": "solo este equipo" o el modelo y el anfitrión, con la advertencia de que sale. **También en el panel de estudio del Lector**, antes de generar y junto a lo generado, con el anfitrión si el texto sale; y en Estudio, cada tarjeta y concepto guardado dice quién lo generó | La búsqueda por significado no lo enseña porque nunca sale: los vectores son siempre locales |
| 1. Nunca el documento completo | `externo::MAXIMO_DE_TEXTO`: 32.000 caracteres por envío, unos dieciséis fragmentos. No puede saber qué es un documento completo, pero sí impedir que salga uno | — |
| 2. Solo los fragmentos y la consulta | El mensaje lleva la instrucción del producto como sistema y los fragmentos numerados como usuario, **sin su localizador**, que no le hace falta al modelo y lleva identificadores internos | Los fragmentos los elige la recuperación local |
| 3. Enseñar antes del primer envío | Sin consentimiento para el anfitrión y el propósito `ai`, **la puerta de red no deja salir**; y la prueba de Ajustes devuelve el mensaje entero que saldría, a dónde y qué hace con él quien lo recibe, antes de ofrecer autorizar. Un recorrido de interfaz y `tests/proveedores_externos.rs` comprueban que nada se autoriza ni se envía antes | Una función de estudio con el externo sin autorizar no envía nada: el panel dice a dónde saldría y qué, y ofrece autorizar. **Lo que no enseña es el mensaje entero** antes de autorizar, como sí hace la prueba de Ajustes: el mensaje lleva el capítulo, y pintar miles de caracteres en un panel lateral no lo hace más legible. Queda en el registro de envíos, byte a byte |
| 4. Registro local auditable | Tabla `ai_sends` (V32): **el mensaje entero tal como salió**, sin la credencial, con anfitrión, modelo, función, tamaño y código de respuesta. Se lee en Ajustes y una prueba exige que lo registrado sea byte a byte lo que recibió el servicio. No se anota lo que no salió —sin permiso o sin red—, que ya está en el registro de salidas | — |
| 5. Claves en el llavero | `secrets::cuenta_de_ia`, atada al anfitrión: cambiar de servidor no le manda a uno la clave del otro. Nunca en SQLite ni en el webview | — |
| Local, exportable y borrable (sección 3) | Local y borrable desde Ajustes; **exportable** en la exportación de la biblioteca junto a las tarjetas y conceptos guardados | — |
| Advertir de la retención del tercero (sección 4) | La confirmación del primer envío dice que quien lo recibe puede guardarlo según su política y que NB Books no controla qué hace con ello | No se enlaza la política de cada proveedor: con un protocolo que sirve cualquiera no hay una lista que enlazar, y una lista de "conocidos" envejece y sugiere una curaduría que no se hace |

**Nunca embeddings.** El proveedor externo no declara la capacidad, así que no hay forma de pedírselos: los
vectores se generan sobre la biblioteca, por cientos, y mandarlos fuera sería mandar libros troceados.

**Nunca como repliegue.** `Choice::Auto` solo mira lo local, y el externo solo se usa si una función lo nombra, que
es lo que hace `Subsistema::eleccion` cuando está configurado. Un fallo del externo no pasa la petición a otro sitio.
