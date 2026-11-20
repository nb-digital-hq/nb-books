# Plantillas de las funciones de estudio

Contrato: [docs/architecture/ai/prompts.md](../../docs/architecture/ai/prompts.md). Iteración **F4.3.1**.

Cada archivo es una plantilla: una cabecera con su identificador, su versión, su propósito, sus variables y el
formato de su salida, y dos secciones, `# sistema` y `# tarea`. Se compilan dentro del binario, así que una
plantilla que no se deja leer hace fallar `cargo test` y no llega a una persona.

**Cambiar el texto de una plantilla es cambiar su versión.** La versión viaja con cada salida guardada, y así se
puede saber con qué instrucción se generó cada tarjeta o cada resumen. Una prueba guarda la huella de cada texto
con su versión, y falla si el texto cambia sin que la versión lo haga.

**El texto del documento no es una variable de la plantilla.** Lo pone el runtime entre la instrucción del
sistema y la tarea, delimitado y neutralizado (F4.3.2), y ninguna plantilla puede colocarlo en otro sitio: el
documento es entrada no confiable y nunca entra en las instrucciones del sistema.
