# Servicios de Voz: qué hay, qué dan gratis y qué protocolo hablan

> **Estado:** Vigente. Compara los servicios de síntesis de voz que la aplicación puede usar como "voz de un
> servicio", con cifras tomadas de la documentación de cada uno (consultadas el 2026-09-02), y fija el contrato del
> proveedor. La voz del sistema y la neuronal local están en [ai/local-models.md](ai/local-models.md), sección 7.

## 1. La unidad que hace comparables las ofertas

Las capas gratuitas se anuncian en unidades distintas —caracteres al mes, minutos de audio, créditos, "neuronas"—
y ninguna dice lo único que importa aquí: **cuántos libros se pueden escuchar**.

La conversión que se usa en todo el documento:

* Un libro de tamaño medio son **unos 400.000 caracteres**.
* Narrado a ritmo normal (~150 palabras por minuto, ~6 caracteres por palabra en español) son
  **unas 7,5 horas de audio**.

Con eso, una oferta de "10.000 caracteres al mes" deja de sonar a algo y pasa a ser lo que es: **once minutos de
narración al mes**, o el 2,5 % de un libro.

## 2. La tabla

Ordenada por lo que de verdad rinde. "Permanente" significa que se renueva mientras la cuenta exista, frente a
una prueba que caduca.

| Servicio | Capa gratuita | Permanente | Tarjeta | Español | Protocolo | Libros al mes |
|---|---|---|---|---|---|---|
| **Cloudflare Workers AI**, MeloTTS | 10.000 neuronas/día; MeloTTS cuesta 18,2 por minuto de audio | Sí | **No** | Sin confirmar (ver §4) | Propio | **~37** (9,2 h/día) |
| **Google Cloud TTS** | 1.000.000 car./mes | Sí | Sí | Sí | Propio (JSON, audio en base64) | ~2,5 |
| **Azure Speech** | 500.000 car./mes (neuronal) | Sí | Sí | Sí | Propio (REST + SSML) | ~1,25 |
| **Unreal Speech** | 250.000 car./mes (6 h) | Sin declarar | No | Sí (8 idiomas) | Propio | ~0,6 |
| **Cloudflare**, Deepgram `aura-2-es` | 10.000 neuronas/día; cuesta 2.727 por 1.000 car. | Sí | No | Sí | Propio | ~0,27 (3.667 car./día) |
| **Groq**, PlayAI Dialog | Límites de la capa gratuita general | Sí | No | **No**: inglés y árabe | **`/v1/audio/speech`** | Según el idioma (§5) |
| **ElevenLabs** | 10.000 car./mes | Sí | No | Sí | Propio | ~0,025 (11 min/mes) |
| **Amazon Polly** | 5.000.000 car./mes | **No**: 12 meses | Sí | Sí | AWS SigV4 | — |
| **Lemonfox** | Prueba de 1 mes | **No** | — | Sí | `/v1/audio/speech` | — |
| **DeepInfra** | 1 USD de crédito | **No** | — | — | `/v1/audio/speech` | — |

## 3. Las tres conclusiones que cambian el diseño

1. **El protocolo compatible con OpenAI (`/v1/audio/speech`) solo lo habla de forma gratuita y permanente Groq, que
   no hace español.** Su valor real es que lo hablan también los servidores de síntesis que se levantan en la propia
   máquina o en la red propia.
2. **Todos los servicios remotos piden credencial**, y por eso la credencial vive en el llavero del sistema
   ([authentication.md](security/authentication.md)).
3. **Las ofertas que suenan generosas no lo son, y la que menos lo parece es la mejor.** ElevenLabs da once
   minutos al mes. Cloudflare cobra MeloTTS **por minuto de audio y no por carácter**, y esa diferencia de
   unidad es la que lo convierte en el único con holgura para escuchar libros enteros: sale treinta y siete
   veces más que Google, con una cuenta gratuita y sin tarjeta.

## 4. Lo que no está confirmado, dicho como tal

**El español de MeloTTS en Cloudflare.** El esquema del modelo declara `lang` como cadena libre, con `en` y `fr`
de ejemplo y sin lista de valores válidos. Hay un
[informe abierto contra la documentación de Cloudflare](https://github.com/cloudflare/cloudflare-docs/issues/23308)
en el que `lang: "es"` devuelve `8002: Invalid input`, y se cerró sin publicar la lista.

La hipótesis, que **no está comprobada**: MeloTTS nombra sus idiomas en mayúsculas —`EN`, `ES`, `FR`, `ZH`,
`JP`, `KR`— y Cloudflare parece pasar el valor tal cual al modelo, así que quien probó `es` habría fallado por
la caja de la cadena y no por falta de soporte. La primera prueba real contra el servicio tiene que ser esa, y
hasta que se haga, la fila de la tabla se queda como está: sin confirmar.

## 5. Por qué un servicio que no hace español sigue sirviendo

Porque el idioma que importa no es el de quien lee: es el del libro. Un catálogo con libros en inglés se narra
con un servicio de inglés, y descartarlo por no hacer español sería descartarlo por una pregunta que no se le
estaba haciendo.

Eso convierte el idioma en **parte del contrato del proveedor**, no en un detalle de configuración: cada
servicio declara qué idiomas hace, el documento ya sabe en cuál está, y un servicio que no cubre el idioma del
libro **no se ofrece para ese libro**. Sin eso, la alternativa es que alguien encienda Groq, abra una novela en
español y escuche un acento inglés leyendo fonética española, sin ninguna pista de por qué.

## 5.1 Cómo se escribe el servidor, y qué hace la aplicación con lo escrito

Cada guía escribe la dirección a su manera, y quien configura copia la suya. Se admiten todas y llevan al mismo
punto: con o sin `http://`, con o sin `/v1`, con `/v1/audio` o la dirección entera del punto de síntesis, con o
sin barra final y con el esquema o la ruta en mayúsculas. Lo interpreta **el runtime** (`speech::Destino`), y
Ajustes enseña debajo del campo la dirección exacta que resulta, para no descubrirla al fallar.

**Sin esquema, el esquema depende de a dónde apunta.** Suponer siempre `https` haría que escribir `localhost:8880`
intentara hablar cifrado con un servidor local que no lo está. Hacia la propia máquina o la red propia, por dirección
numérica, se supone `http`; hacia fuera, `https`. Quien decide si el resultado puede salir sigue siendo la puerta
de red, que no admite el claro hacia internet.

## 5.2 Atribución de voz y modelo en el audio preparado y regeneración

El audio preparado de un capítulo o de un libro almacena en la base de datos (`audio_sections`, V35) la **voz**
(`voice`) y la **revisión de modelo** (`model_rev`) con que se sintetizó.

Cuando se consulta lo preparado de un alcance:
1. El runtime comprueba si la preparación disponible en disco coincide con la voz configurada actualmente y la
   revisión del sintetizador (`matches_current`).
2. Las vistas (`AudiolibrosPanel`, `AudioPanel`) muestran de forma explícita qué voz y revisión de modelo tiene el audio.
3. Si la voz o el modelo difieren de la elección actual, la interfaz lo señala y **ofrece regenerarlo con la voz
   actual**, sin hacerlo de manera automática. La síntesis consume tiempo y cómputo considerables; sobreescribir audio
   existente sin consentimiento explícito violaría el principio de control del usuario sobre sus recursos.
4. Por ser almacenamiento derivado, el audio preparado **no viaja al exportar la biblioteca**.

**Compatible se conserva; incompatible se borra solo y se avisa.** Lo preparado con otra voz o con otra revisión del
modelo es compatible: el audio sigue sonando y sus marcas apuntan al texto, así que se conserva y se ofrece
regenerar (punto 3). Lo preparado con una voz que el catálogo ya no tiene -una actualización la retiró- no se puede
regenerar igual ni continuar: al arrancar se borra con su archivo
(`Preparador::retirar_de_voces_retiradas`), y la interfaz lo dice una vez con cuántas preparaciones se fueron.

## 6. Lo que se descartó, y por qué

* **Los extremos internos de Edge de Microsoft** (`speech.platform.bing.com`), que es lo que usa Readest. No es
  un servicio: es un extremo interno alcanzado por ingeniería inversa, ya con credenciales antiabuso y filtrado
  de rangos de nube. Un puente local como `openai-edge-tts` no cambia el fondo, solo mueve de sitio quién hace
  la petición. Queda como opción que **el usuario puede instalar y apuntar**, no como algo que el producto trae.
* **Las pruebas que caducan** (Lemonfox, DeepInfra, Polly). Una función que deja de existir al mes no es una
  función: es una cuenta atrás.

## 7. Fuentes

* [Cloudflare Workers AI — precios y capa gratuita](https://developers.cloudflare.com/workers-ai/platform/pricing/)
* [Cloudflare Workers AI — MeloTTS](https://developers.cloudflare.com/workers-ai/models/melotts/) y
  [`aura-2-es`](https://developers.cloudflare.com/workers-ai/models/aura-2-es/)
* [Groq — Text to Speech](https://console.groq.com/docs/text-to-speech) y
  [PlayAI Dialog](https://console.groq.com/docs/model/playai-tts)
* [Unreal Speech — API](https://docs.unrealspeech.com/reference/getting-started-with-our-api) y
  [precios](https://unrealspeech.com/pricing)
* [Comparativa de capas gratuitas de TTS](https://freeapihub.com/blog/best-free-text-to-speech-apis)
