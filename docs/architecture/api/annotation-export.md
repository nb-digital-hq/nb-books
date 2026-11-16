# Exportación de Anotaciones

> **Estado:** Implementado (`src-tauri/src/annotations/web.rs`); el Markdown y el texto
> plano de la sección 4 (`src-tauri/src/annotations/texto.rs`).
> Este documento define el formato con el que las anotaciones **salen de NB Books hacia cualquier otra
> herramienta**. No es el mismo que [export-format.md](export-format.md): aquel devuelve la biblioteca a NB
> Books, y este devuelve las anotaciones a quien sea.

## 1. Por qué son dos formatos y no uno

Sirven a dos propósitos que no se parecen:

| | [export-format.md](export-format.md) | Este documento |
|---|---|---|
| Para qué | Recuperar la biblioteca entera | Llevarse las anotaciones a otra parte |
| Quién lo lee | NB Books | Cualquiera |
| Qué lleva | Todo, incluido el historial de versiones | Las anotaciones vivas |
| Vocabulario | Propio y versionado | El del W3C |

Un solo formato tendría que elegir, y elegiría mal para uno de los dos usos.

## 2. El formato

Es **JSON-LD** con el contexto del modelo de anotación web del W3C. El archivo es una `AnnotationPage` con sus
`items`:

```json
{
  "@context": "http://www.w3.org/ns/anno.jsonld",
  "type": "AnnotationPage",
  "items": [
    {
      "@context": "http://www.w3.org/ns/anno.jsonld",
      "id": "urn:uuid:9f1c2e30-4b5a-4c6d-8e7f-0a1b2c3d4e5f",
      "type": "Annotation",
      "motivation": "highlighting",
      "created": "2026-08-27T09:14:22.031Z",
      "target": {
        "source": "OEBPS/cap05.xhtml",
        "selector": [
          { "type": "TextPositionSelector", "start": 120, "end": 153 },
          {
            "type": "TextQuoteSelector",
            "exact": "frente al pelotón de fusilamiento",
            "prefix": "Muchos años después, ",
            "suffix": ", el coronel"
          }
        ]
      },
      "https://nbdigitallabs.lat/books/ns#locations": {
        "totalProgression": 0.17,
        "progression": 0.42,
        "position": 233
      }
    }
  ]
}
```

### 2.1 La motivación dice qué clase de anotación es

| Clase en NB Books | `motivation` |
|---|---|
| Resaltado | `highlighting` |
| Nota | `commenting` |
| Marcador | `bookmarking` |

### 2.2 Varios selectores, y el más preciso primero

La lista de `selector` es la cascada de [ADR-004](../../adr/004-reader-locator-model.md) escrita en el
vocabulario de otro. Quien lea el archivo elige el que sepa aplicar.

El orden no lo exige el modelo, pero se respeta a propósito: un consumidor sencillo aplica el primero que
entiende, y darle el peor primero lo dejaría en un sitio peor del que podíamos ofrecerle.

| Lo que guarda el Locator | Cómo sale |
|---|---|
| `#char=N` más la cita | `TextPositionSelector` con `start` y `end` en **caracteres** |
| `epubcfi(...)` | `FragmentSelector` con `conformsTo` apuntando a la especificación de EPUB CFI |
| `page=N` | `FragmentSelector` con `conformsTo` apuntando a RFC 3778 |
| Cita con contexto | `TextQuoteSelector` con `exact`, `prefix` y `suffix` |

`conformsTo` no es decoración: sin él, `page=12` es una cadena cualquiera; con él, es un fragmento con una
definición publicada detrás.

### 2.3 Lo que el estándar no cubre

El modelo del W3C no tiene ningún selector para la **progresión relativa**, que es la tercera vía de la cascada.
Sale bajo `https://nbdigitallabs.lat/books/ns#locations`, fuera del vocabulario estándar.

El dominio es `.invalid` a propósito: no resuelve y no puede resolver nunca, así que nadie va a pedirle una
definición a un servidor que no existe, y queda claro que es una extensión local y no vocabulario publicado.

Un consumidor que no conozca esa clave la ignora y sigue teniendo la cita y la posición. Meter la progresión
dentro del vocabulario estándar con un nombre inventado sería peor: haría que un archivo nuestro pareciera decir
algo que el estándar no dice.

## 3. Qué no sale

* **Las anotaciones dadas de baja.** El historial de versiones no significa nada fuera de aquí. Va en
  [export-format.md](export-format.md), que es el formato para volver.
* **Datos del usuario que nadie pidió.** No hay `creator`: NB Books no tiene identidad de usuario, e
  inventarse una para rellenar un campo del estándar sería peor que dejarlo fuera.
* **La ruta del archivo en disco.** `source` es el recurso **dentro** de la publicación, no dónde vive el
  documento en esta máquina. Ver [security/privacy.md](../../audit/privacy.md).

## 4. En Markdown y en texto plano, para leerlo

El JSON de arriba lo entienden otras herramientas, y no lo lee nadie. Foliate saca además un archivo que se abre
y se lee; aquí también, por el mismo comando -`save_annotations`, con `format` a `markdown` o
`text`- y desde el mismo sitio, el panel de anotaciones del lector. Sin `format`, el comando escribe el JSON de
siempre: lo que ya funcionaba no cambia.

Lo que lleva cada anotación, y por qué:

| Qué | Cómo sale | Por qué |
|---|---|---|
| El orden | El del libro, por `totalProgression` | Es el orden en que se lee lo exportado; el de creación solo lo entiende quien anotó |
| El capítulo | Un encabezado por capítulo, no uno por anotación | Es como se busca un pasaje de memoria |
| La cita | Como cita de Markdown (`> `) o entre comillas angulares | Es lo que se quería guardar |
| La nota | Debajo de su cita | Una nota sin su pasaje no se entiende |
| Dónde está, para una persona | La clase y el porcentaje del libro | Nunca un número de página inventado: ver [ADR-004](../../adr/004-reader-locator-model.md) |
| Dónde está, para una máquina | El recurso y el fragmento del Locator, `OEBPS/cap05.xhtml#char=120` | Con él otra herramienta, o esta después de reinstalar, vuelve al sitio exacto. En Markdown va entre acentos graves para que ningún visor lo convierta en enlace |

Se escribe **en el idioma de la interfaz** (`language`): "Highlight" dentro de una exportación en español sería
un defecto. Lo compone el runtime y no la interfaz por lo mismo que el JSON: el webview no escribe en disco.

Lo que no sale es lo mismo que en la sección 3, por los mismos motivos.

## 5. Lo que todavía no está

La exportación de citas en CSL JSON y BibTeX que menciona
[metadata-model.md](../metadata-model.md), sección 6. Es un formato bibliográfico, no de
anotación, y se decidirá con su iteración.
