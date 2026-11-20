# Formato de Exportación e Intercambio

> **Estado:** Implementado. Módulo: `src-tauri/src/portability.rs`.
> Este documento define el formato con el que un usuario saca **todos** sus datos de NB Books y los vuelve a
> meter. Es la garantía material del principio de posesión de los datos: sin él, "el usuario es dueño de sus
> datos" es una frase.

## 1. Qué lleva y qué no

Lleva **la identidad de los documentos, no sus bytes**: su ubicación, el hash de contenido, el tamaño y el tipo
de medio. Una biblioteca real ocupa cientos de gigabytes, y un archivo de exportación que los incluyera no sería
un archivo, sería otra copia de la biblioteca.

Al restaurar en otra máquina, los documentos se reconocen por su hash cuando están presentes, que es exactamente
para lo que sirve la identidad por contenido. Un documento ausente deja su registro con toda su
información —metadatos, anotaciones, procedencia— y su archivo marcado como no disponible.

### 1.1 Ninguna ruta absoluta viaja en el archivo

La ubicación de un archivo son dos datos: la **raíz de biblioteca** en la que vive y su **ruta
relativa** dentro de ella. La exportación lleva las raíces con su nombre y su clase, y **siempre con la ruta
vacía**.

No es un descuido: una ruta absoluta es la dirección de un equipo concreto. Llevarla al equipo de destino dejaría
**toda** la biblioteca referenciada como no disponible al restaurar en otra máquina, o en la misma tras reinstalar
con otro nombre de usuario.

La mudanza queda así:

1. Se restaura. Todas las raíces referenciadas llegan **no localizadas**, y sus documentos aparecen ausentes.
2. El usuario apunta cada raíz a su sitio nuevo. **Una vez por raíz.**
3. Vuelven todos sus documentos, sin reimportar y sin tocar una anotación.

La raíz **administrada** no necesita ese paso: se resuelve sola con la carpeta de biblioteca de la instalación
que abre la base. Por eso su identificador es fijo y no aleatorio, y por eso la importación conserva la del
equipo de destino en lugar de pisarla con la del origen.

Lo comprueba `src-tauri/tests/mudanza.rs`, que además verifica que el JSON exportado **no contiene** ninguna ruta
del equipo de origen.

### 1.2 Lo derivado no viaja

El almacenamiento derivado —el **audio preparado** de capítulos y libros (`audio_sections`, `audio_marks` y
los archivos de audio generados) y los **embeddings** vectoriales— **no viaja en la exportación**.
Se trata de datos reproducibles que ocupan cientos de megabytes o gigabytes y se regeneran localmente cuando
se necesitan. Incluirlos engordaría el archivo con artefactos dependientes de la voz y los modelos del equipo de origen.

## 2. Forma

Un único documento JSON, con sangría, legible sin herramientas.

```json
{
  "format": "nb-books-export",
  "formatVersion": 1,
  "exportedAt": "2026-08-26T10:41:07.123Z",
  "application": { "name": "NB Books", "version": "1.0.0", "schemaVersion": 4 },

  "works": [], "editions": [], "editionIdentifiers": [],
  "libraryRoots": [], "files": [], "contributors": [], "editionContributors": [],
  "notes": [], "noteLinks": [], "noteAnnotationLinks": [],
  "subjects": [], "editionSubjects": [], "annotations": [],
  "metadataProvenance": [], "metadataSuggestions": [],
  "settings": [],
  "collections": [], "collectionItems": [],
  "progress": [], "readingSessions": [],
  "vocabulary": [],
  "studyCards": [], "concepts": [], "aiSends": []
}
```

Las cuatro últimas secciones se poblaron, junto con el arreglo del hueco que las tenía
declaradas y vacías. `collectionItems` y `readingSessions` se añadieron entonces, como campos nuevos y opcionales:
una exportación de antes que no los traiga se sigue leyendo igual.

Dos detalles del contenido que no son arbitrarios:

* Una **colección por regla** exporta su criterio y **ningún miembro**. Sus miembros no son un dato: son el
  resultado de evaluar la regla, y al importarla se vuelven a evaluar solos sobre la biblioteca de destino.
* El **locator** del progreso, de las sesiones y del vocabulario viaja como texto, sin interpretar. Una
  exportación tiene que poder llevarse un locator que esta versión no entienda; rechazarlo al exportar perdería
  la posición del usuario para siempre, que es justo lo contrario de lo que el formato existe para garantizar.
* **Lo generado que alguien guardó** —`studyCards` y `concepts`— llegó, también como campos
  nuevos y opcionales. Entra aunque lo escribiera una máquina: guardarlo, editarlo y repasarlo es trabajo del
  usuario. **Viaja con su marca de generado**: quién lo generó y con qué plantilla, y si se editó después. Lo que no
  se guardó no existe, así que no hay nada suyo que exportar.
* El **registro de envíos a un proveedor de IA externo** (`aiSends`) llegó y cierra lo que
  [ai/privacy.md](../ai/privacy.md) sección 3 pedía: local, **exportable** y borrable. Lleva el mensaje
  entero tal como salió —con fragmentos de documentos del usuario— y nunca la credencial, que va en una cabecera.
* El **vocabulario** llegó, como campo nuevo y opcional: una exportación anterior que no lo traiga
  se sigue leyendo igual. Entra por la misma razón que las sesiones de lectura: es trabajo de lectura del
  usuario, y dejarlo fuera lo convertiría en un dato que solo existe mientras use esta instalación.

## 3. Versionado

Sigue las reglas de [versioning.md](../versioning.md): SemVer con la compatibilidad
hacia atrás como prioridad. En el archivo viaja solo el número mayor.

* Un cambio **compatible** añade campos opcionales y no mueve `formatVersion`.
* Un cambio **incompatible** la sube, y entonces la versión antigua se niega a leer el archivo.

Esa negativa es deliberada y es la misma disciplina que aplican el motor de migraciones y el `Locator`:
interpretar un archivo nuevo con reglas antiguas restauraría datos incompletos, y **una restauración incompleta
es peor que ninguna, porque parece haber funcionado**.

Un archivo que no declara `"format": "nb-books-export"` se rechaza: no se adivina de dónde salió.

## 4. Restaurar, no fusionar

La importación restaura sobre una base vacía y **se niega a operar sobre una biblioteca que ya tiene contenido**.

Fusionar dos bibliotecas es otro problema, que este formato no resuelve: exige decidir qué gana cuando ambas
versiones de una anotación difieren.

Todo se restaura dentro de **una única transacción**. Una importación a medias dejaría una biblioteca que parece
la del usuario y no lo es, que es peor que un fallo limpio.

## 5. Los identificadores se conservan

Se restauran tal cual venían. Es lo que hace que la biblioteca restaurada sea **equivalente** y no solo parecida:
una anotación que apuntaba a una edición tiene que seguir apuntando a la misma. Es también la razón por la que
los identificadores son aleatorios de 128 bits y no enteros correlativos
([schemas.md](schemas.md), sección 3.1).

**Con una excepción desde V38: los autores repetidos.** Un archivo exportado antes puede traer al mismo
autor en varias filas, y la base ya solo admite una. Al restaurarlo, la primera fila de cada autor conserva su
identificador y las demás se funden en ella -por ORCID si lo traen, por el nombre sin mayúsculas ni tildes si no-;
sus autorías pasan a la que se queda y ninguna obra pierde a su autor. Es lo mismo que hizo la migración, y es
mejor que fallar entera por el índice único el día que hace falta restaurar.

## 6. Lo que sobrevive, y por qué importa que lo haga

| Sección | Qué se pierde si no sobrevive |
|---|---|
| Anotaciones con su `target` | El trabajo de lectura de años, ancladas por Locator y no por un número |
| `metadataProvenance` | La corrección que el usuario hizo a mano deja de ganar sobre la actualización automática |
| `metadataSuggestions` | Las propuestas que quedaban por revisar, que también son información |
| `settings` | Las preferencias de accesibilidad, que son las que más caro cuesta reconstruir |
| `files` con su hash | La capacidad de reconocer los documentos al restaurar en otra máquina |
| `libraryRoots` | La mudanza entera: sin ellas, cada documento habría que volver a señalarlo uno a uno |
| `notes` y `noteLinks` | Lo que el usuario escribió por su cuenta, y la red que lo conecta. **Los retroenlaces no se exportan**: son los mismos enlaces leídos del otro lado, y llevarlos aparte permitiría que las dos direcciones dejaran de coincidir al restaurar |

## 7. Estabilidad del archivo

Dos exportaciones del mismo estado producen **el mismo archivo**, salvo la marca de tiempo: cada sección va
ordenada por identificador. Un archivo que cambia sin que cambien los datos no se puede comparar ni versionar, y
convierte cualquier copia de seguridad incremental en una copia completa.

## 8. Alcance

* El archivo contiene **referencias** a los documentos, no los documentos: para mudarse de máquina se copian las
  carpetas de libros aparte y se indica su nueva ubicación.
* La exportación es de la biblioteca entera; el formato admite estructuralmente una exportación parcial.
