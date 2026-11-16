# Cambios de NB Books

## 1.0.0 — 2026-11-16

Primera versión de NB Books.

* Lectura de EPUB, PDF y cómics (CBZ) con modos paginado y continuo, vista dividida, lectura inmersiva y
  tipografía por libro.
* Biblioteca referenciada con identidad por contenido, duplicados, estanterías, colecciones, autores, series y
  materias, e importación de bibliotecas de Calibre.
* Ficha de cada obra con corrección manual de sus datos y baja de la biblioteca, que nunca borra el archivo
  referenciado. Metadatos y portada leídos también de PDF y de `ComicInfo.xml` en CBZ.
* Anclas alfabéticas en bibliotecas grandes ordenadas por título o autor.
* Anotaciones ancladas por localizador, notas enlazadas, tableros, vocabulario y repaso espaciado.
* Búsqueda de texto completo en la biblioteca y en el documento, y búsqueda semántica opcional con un modelo local.
* Consulta en diccionarios StarDict sin conexión; enciclopedia y traducción con consentimiento.
* Lectura en voz alta con voces del sistema, voz neuronal local y audio preparado.
* IA local opcional para funciones de estudio, y proveedores externos solo si se configuran.
* Catálogos OPDS, extensiones en cajón de arena WebAssembly con la API de extensiones 1.0.0.
* Integración con NB Launcher, que puede instalar, actualizar y quitar una copia de NB Books desde su catálogo
  firmado.
* Una sola puerta de red con modo sin red y consentimiento por servidor y propósito; sin telemetría.
* Paquetes para Linux (`.deb`, `.rpm`, `.tar.gz` y AppImage) firmados con Ed25519. Cada uno lleva dos ediciones
  del programa: la universal, para cualquier procesador x86-64, y la rápida, **solo para procesadores compatibles
  con AVX2** (Intel desde 2013, AMD desde 2015), con la que la IA local va unas diez veces más rápida. Al arrancar
  se usa la rápida si el procesador la admite.
