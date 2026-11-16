# Modelo de Documentos (Document Model)

> **Estado:** Diseño. La lista de formatos previstos excede ampliamente lo implementado.

NB Books se centra en el recurso y el propósito del conocimiento, no en la extensión técnica del archivo.

## 1. Abstracción sobre la Extensión de Archivo

El sistema debe tratar los archivos simplemente como representaciones técnicas. A nivel de dominio y UI, el usuario interactúa con **Documentos** o **Recursos**, no con "Archivos PDF" o "Archivos EPUB".

El concepto de "Lector" diferirá según el formato, pero las capacidades circundantes (búsqueda, anotación, gestión) deben homologarse todo lo posible.

### Formatos Soportados Previstos

1. **Texto y Libros Electrónicos:** `.epub`, `.kepub`, `.kfx`, `.azw3`, `.pdf`, `.azw4`
2. **Cómics y Mangas:** `.cbz`, `.cbr`, `.cbt`, `.cb7`
3. **Audio:** `.mp3`, `.m4b`, `.m4a`, `.mp4`, `.aax`
4. **Documentos Textuales Planos y Web:** `.txt`, `.rtf`, `.doc`, `.docx`, `.html`, `.htm`
5. **Formatos Heredados / Obsoletos:** `.mobi`, `.azw`, `.prc`

## 2. Gestión de Almacenamiento y Archivos

NB Books debe soportar dos paradigmas de almacenamiento para adaptarse a todos los perfiles de usuario. Esto evita duplicación de datos (un problema común en gestores legacy como Calibre).

### A. Biblioteca Administrada (Managed Library)
* **Comportamiento:** El usuario arrastra o importa un documento, y NB Books copia el archivo a una carpeta de biblioteca interna, organizándolo y administrándolo por completo.
* **Ventaja:** Cero riesgos de links rotos, ideal para la mayoría de los usuarios.

### B. Biblioteca Referenciada (Referenced Library)
* **Comportamiento:** Los archivos permanecen en su ubicación original en el disco duro del usuario. NB Books únicamente extrae metadatos, crea índices y guarda las referencias.
* **Ventaja:** Fundamental para usuarios que tienen sus bibliotecas en NAS, discos duros externos compartidos o servicios de sincronización en la nube (Dropbox, Google Drive, Syncthing), evitando la copia masiva e innecesaria de gigabytes de datos.

