# Voz y audiolibros: cómo se oye un libro en NB Books

> **Estado:** Vigente. Recoge los caminos para oír un libro, cuáles ofrece NB Books, cuáles se descartaron y por
> qué. Complementa la [experiencia de lectura](README.md): **el papel no puede leerte, y una pantalla sí**, pero
> esa magia hay que elegirla bien. El contrato técnico está en
> [voice-services](../../architecture/voice-services.md).

## 1. Los caminos posibles

Quien quiere oír un libro tiene delante cinco caminos, y solo tres son síntesis:

| Camino | Qué es | En NB Books |
|---|---|---|
| A. Síntesis del sistema | La voz que ya trae el sistema operativo | Speech Dispatcher, sin red |
| B. Síntesis de un servicio | Un servidor de voz, propio o de un tercero | Con consentimiento propio: lo que sale es el texto del libro |
| C. Síntesis local neuronal | Un modelo que corre en la máquina | Kokoro 82M, descargado bajo petición, y audio preparado por adelantado para libros enteros |
| D. Voz humana grabada | Alguien ya leyó ese libro | EPUB 3 con Media Overlays: la narración suena anclada al texto |
| E. Leer un audiolibro | Transcribir una grabación | No se ofrece: ver la sección 4 |

Los caminos D y E son los más baratos en calidad por esfuerzo, porque el trabajo caro -leer bien un libro entero- ya
lo hizo una persona.

## 2. Voz humana sincronizada con el texto

El estándar existe y no hay que inventarlo: **EPUB 3 Media Overlays**, SMIL que asocia fragmentos del texto con
instantes del audio. NB Books lo reproduce en el panel de voz con el mismo resaltado y la misma posición guardada
que la síntesis.

Encaja sin coste porque la voz ya emite marcas ancladas a `Locator`
([ADR-004](../../adr/004-reader-locator-model.md)) y no a números de bloque: una pista alineada es la misma marca
con otro origen. `narracion.test.ts` fija que la marca lleva el Locator.

## 3. Catálogos de audiolibros de voz humana

**No se ofrecen**, ni en el núcleo ni como extensión. [LibriVox](https://librivox.org/) y la colección
`librivoxaudio` de Internet Archive publican grabaciones voluntarias de dominio público, pero NB Books es un lector
de lo que la persona ya tiene: traer, organizar y reproducir audiolibros de un catálogo es otro producto, con su
propia biblioteca de audio, y sumarlo convertiría el lector en un reproductor. La voz humana que sí suena es la que
viene **dentro** de un EPUB con Media Overlays (sección 2).

Queda escrito lo que se comprobó, para no volver a investigarlo: el catálogo de LibriVox se pagina bien
(`/api/feed/audiobooks/?format=json&extended=1`), pero su búsqueda por título o autor no filtra; la que sí filtra es
la de Internet Archive sobre `collection:(librivoxaudio)`.

## 4. Lo que se descartó y por qué

| Descartado | Motivo |
|---|---|
| **Transcribir audiolibros en el núcleo** | Ejecutar un modelo de transcripción en la máquina del usuario es un subsistema entero, con su motor y sus pesos, para una conducta minoritaria. Si el modelo de datos admite pista de audio, añadir un productor de texto después no exige rehacer el documento |
| **Código AGPL** | El núcleo es Apache 2.0 ([ADR-003](../../adr/003-licensing-model.md)). Un estudio de voz bajo AGPL, como VoiceStudio, no puede entrar ni copiado ni adaptado; hablar con él por red, si alguien lo tiene instalado, sí |
| **Modelos de voz con pesos no comerciales** | El código de un motor puede ser Apache 2.0 y sus pesos CC-BY-NC. La licencia se revisa **por modelo**, no por repositorio: ver [model-reviews](../../legal/model-reviews.md) |
| **Catálogos de audiolibros de voz humana** | Ver la sección 3: es otro producto, no una función del lector |
| **Un formato propio de alineación** | La alineación se expresa en `Locator` y se intercambia en EPUB 3 Media Overlays |

## 5. Herramientas de alineación de referencia

Para quien quiera producir EPUB con narración sincronizada fuera de la aplicación:

| Herramienta | Método |
|---|---|
| [Storyteller](https://storyteller-platform.dev/) | Transcribe con Whisper y empareja el texto transcrito con el del libro por coincidencia difusa |
| [aeneas](https://github.com/readbeyond/aeneas) | Alineación forzada clásica, sin transcribir |
| [syncabook](https://github.com/r4victor/syncabook) | Construye EPUB 3 con Media Overlays a partir de texto de Gutenberg y audio de LibriVox |

## Fuentes

* LibriVox, [API y OPDS](https://librivox.org/2011/04/28/librivox-api-opds/) y
  [página de información de la API](https://librivox.org/api/info).
* Internet Archive, [búsqueda avanzada](https://archive.org/advancedsearch.php).
* [Storyteller](https://storyteller-platform.dev/docs/the-algorithm/), [syncabook](https://github.com/r4victor/syncabook)
  y [aeneas](https://github.com/readbeyond/aeneas).
* [VoiceStudio](https://github.com/debpalash/VoiceStudio), README.
* EPUB 3 Media Overlays, y su soporte en [Thorium](https://thorium.edrlab.org/en/docs/300_accessibility/).
