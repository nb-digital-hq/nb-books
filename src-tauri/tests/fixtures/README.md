# Corpus de archivos de prueba

> **Procedencia:** todos estos archivos se generan con `scripts/generate-fixtures.py`, con contenido escrito
> para el proyecto. **Ninguno contiene material con derechos de autor vigentes**, y el repositorio no puede
> contenerlo. Ver [copyright-and-drm.md](../../../docs/legal/copyright-and-drm.md).
>
> **Licencia:** Apache-2.0, igual que el resto del repositorio.

Regenerar: `python3 scripts/generate-fixtures.py`

## Documentos válidos

| Archivo | Para qué sirve |
|---|---|
| `minimo-valido.pdf` | Camino feliz de PDF: una página, un texto, tabla de referencias correcta |
| `minimo-valido.epub` | Camino feliz de EPUB 3: contenedor, paquete y un capítulo |
| `minimo-valido.cbz` | Camino feliz de cómic: cuatro páginas PNG numeradas 1, 2, 9 y 10, más tres entradas que **no** son páginas (`ComicInfo.xml`, basura de macOS y un archivo oculto). Los números saltan de 9 a 10 a propósito: en orden alfabético "10" va antes que "9" |

## Documentos dañados

| Archivo | Qué debe ocurrir |
|---|---|
| `truncado.pdf` | Error `DOCUMENT_CORRUPTED` controlado, sin pánico ni registro huérfano |
| `xml-malformado.epub` | Error controlado al analizar el capítulo |

## Entrada hostil

Estos archivos existen para que las defensas se prueben, no se supongan. Ver
[threat-model.md](../../../docs/audit/threat-model.md).

| Archivo | Ataque | Defensa esperada |
|---|---|---|
| `entidad-externa.epub` | Entidad externa de XML que apunta a `/etc/passwd` | El analizador no resuelve entidades externas |
| `recorrido-directorios.cbz` | Entradas `../../` y `..\\..\\` dentro del ZIP | `security::validate_archive_entry` las rechaza |
| `extension-falsificada.epub` | Un PDF que se anuncia como EPUB | El formato se detecta por contenido, no por extensión |
| `bomba-comprimida.cbz` | 20 MiB de ceros en 20 KB: ratio de 1000:1 | El índice rechaza el contenedor **antes** de descomprimir nada |

> **Aviso para quien escriba pruebas:** `extension-falsificada.epub` es **byte a byte el mismo archivo** que
> `minimo-valido.pdf`. Es lo correcto para lo que prueba, pero significa que importar los dos no da dos
> documentos: la identidad por contenido (F1.1.4) reconoce el segundo como duplicado del primero. Una prueba que
> necesite dos PDF distintos tiene que fabricar el segundo, por ejemplo añadiendo un comentario después del
> `%%EOF`.
| `adversario-prompt.txt` | Instrucciones dirigidas a un modelo de lenguaje | El contenido del documento nunca tiene autoridad sobre el sistema |

`adversario-prompt.txt` no se usa todavía: es el corpus de la iteración F4.3.2. Se incluye ahora para que la
defensa se diseñe contra un caso concreto en lugar de contra una idea abstracta.
