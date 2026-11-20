# Filosofía de Ingeniería de IA

> **Estado:** Implementado. Las seis reglas de la sección 1 tienen prueba detrás: las 1, 2 y 4 en
> `src-tauri/src/ai/mod.rs` (la ausencia es un valor y no un error, el camino automático no puede salir del
> dispositivo, y una salida generada lleva su procedencia en el tipo); la 3 en la puerta de red; la 5 en
> `ai/salidas.rs` (toda afirmación generada cita el fragmento del que sale, o se marca como no verificable); la 6 en
> la interfaz, que enseña lo que se descarga, lo que ocupa cargado y cuánto texto sale hacia un proveedor externo.

La IA en NB Books es una **capacidad de enriquecimiento opcional, no la identidad del producto**. Esta frase tiene
consecuencias técnicas concretas, no solo retóricas.

## 1. Reglas no negociables

1. **La lectura nunca depende de la IA.** Si el subsistema de IA no existe, no arranca o falla, la aplicación
   debe abrir, leer, buscar y anotar con normalidad.
2. **Local primero.** El proveedor por defecto es inferencia local. El proveedor cloud es una elección consciente
   del usuario, nunca un fallback silencioso.
3. **Sin contenido sin permiso.** Ningún fragmento del documento sale del dispositivo sin consentimiento explícito
   e informado para ese proveedor.
4. **La IA sugiere, no decide.** Toda salida generada es editable, descartable y claramente marcada como generada.
5. **Atribución de origen.** Toda afirmación generada sobre el documento debe poder rastrearse a un fragmento del
   documento (cita o localizador), o presentarse explícitamente como no verificada.
6. **Coste visible.** El usuario debe saber cuándo una acción consume su cuota de API de pago o cuándo va a
   cargar varios gigabytes de modelo. Antes de enviar a un proveedor externo se dice cuánto texto sale, en
   caracteres y en tokens aproximados, sin inventar precios.

## 2. Qué problemas resuelve la IA aquí

La IA se justifica solo cuando reduce fricción en el objetivo real del usuario: comprender y retener conocimiento.

| Caso de uso | Valor | Riesgo principal |
|---|---|---|
| Resumen de capítulo | Orientación rápida | Alucinación, sesgo de omisión |
| Generación de flashcards | Retención | Tarjetas triviales o incorrectas |
| Preguntas de autoevaluación | Estudio activo | Preguntas ambiguas |
| Búsqueda semántica en la biblioteca | Descubrimiento | Falsos positivos, coste de indexación |
| Extracción de entidades y conceptos | Mapas conceptuales | Ruido |
| Traducción | Comprensión | Ya resuelta en Consultar sin IA (diccionarios locales y remotos) |

Cualquier función que no encaje en esta tabla necesita justificación en un ADR antes de implementarse.

## 3. Fronteras arquitectónicas

* Todo acceso a IA pasa por el contrato `AIServiceContract` (ver
  [component-model.md](../component-model.md)). Ningún componente instancia un motor concreto.
* El servicio de IA se ejecuta fuera del hilo principal y no puede bloquear la UI ni el arranque.
* El presupuesto de recursos (RAM, VRAM, hilos) es configurable y tiene un tope duro por defecto.
* La ausencia de IA es un estado de primera clase (`NOT_INSTALLED` / `NOT_SUPPORTED`), no un error.

## 4. Honestidad sobre las limitaciones

La documentación de usuario debe declarar sin adornos que los modelos generan errores, que un resumen no
sustituye la lectura y que las respuestas pueden ser incorrectas incluso cuando suenan seguras. En un producto
educativo, presentar salidas generadas como si fueran hechos verificados es un problema pedagógico, no solo
técnico.

Documentos relacionados: [local-models.md](local-models.md), [embeddings.md](embeddings.md),
[prompts.md](prompts.md), [privacy.md](privacy.md), [evaluation.md](evaluation.md).
