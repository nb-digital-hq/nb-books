# Bus de Eventos Interno

> **Estado:** Implementado. Módulo: `src-tauri/src/events.rs`.

La comunicación entre el frontend y el backend, y entre los propios componentes del backend, se rige por un bus
de publicación y suscripción.

## Por qué un bus de eventos

Los componentes no deben invocar métodos de otros componentes de forma síncrona salvo que sea imprescindible:
genera acoplamiento y propensión a bloqueos del hilo principal.

El servicio emisor publica sin saber quién escucha. El consumidor reacciona sin saber quién emitió.

## Flujo estándar

1. Ocurre una acción: el usuario importa un documento.
2. La interfaz invoca un comando; el `Library Service` persiste en SQLite y emite `DocumentImported`.
3. Reacción distribuida:
   * `Metadata Service` obtiene la portada.
   * `Search Service` inicia la extracción e indexación en segundo plano.
   * El puente de la interfaz notifica al frontend para que actualice la vista.

## Reglas del bus

* **Disparar y olvidar.** Salvo que se requiera confirmación explícita, emitir no bloquea al emisor.
* **Idempotencia.** Los consumidores deben tolerar recibir el mismo evento dos veces sin corromper el estado.
* **Payloads ligeros.** Los eventos transportan identificadores y referencias, nunca el contenido de un
  documento. El consumidor solicita lo que necesite por la vía adecuada.
* **Emisión posterior a la confirmación.** Un evento se emite después de confirmar la transacción de base de
  datos, nunca antes: ningún consumidor debe observar estado a medias.
* **Orden no garantizado.** Los consumidores no pueden asumir orden de entrega entre eventos distintos.
* **Esquema versionado.** Los payloads solo crecen con campos opcionales; los consumidores ignoran campos
  desconocidos. Ver [versioning.md](versioning.md).

## Manejo de fallos en los consumidores

Un consumidor que falla no puede tumbar al emisor ni al bus. El despachador captura el pánico del consumidor, lo
registra y **desactiva ese suscriptor**, para no repetir el mismo fallo en cada evento. El emisor y los demás
consumidores no se enteran.

Dos consecuencias de esta regla que conviene tener presentes:

* El perfil de compilación de release **no** usa `panic = "abort"`. Reduciría el tamaño del binario, pero
  abortaría el proceso ante el pánico de cualquier consumidor, que es exactamente lo que la regla 1 de la
  Constitución prohíbe.
* La entrega ocurre en un hilo despachador propio, de modo que emitir nunca bloquea al emisor.

Ambas están cubiertas por pruebas en `events.rs` y en la suite de caos.

## Catálogo

Los eventos implementados y previstos se catalogan en [api/events.md](api/events.md).
