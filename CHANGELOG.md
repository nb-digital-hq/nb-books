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
* Servidor opcional para NB Books Mobile en la red local, apagado por defecto: emparejamiento por QR o código con
  el certificado fijado por su huella, elección de qué libros llegan al teléfono, sincronización de posiciones,
  anotaciones, notas y repasos sin que nada se pise, búsqueda en directo y Leer en el PC, que convierte el teléfono
  en el mando del lector.
* Integración con NB Launcher, que puede instalar, actualizar y quitar una copia de NB Books desde su catálogo
  firmado.
* Una sola puerta de red con modo sin red y consentimiento por servidor y propósito; sin telemetría.
* Paquetes para Linux (`.deb`, `.rpm`, `.tar.gz` y AppImage) firmados con Ed25519. Cada uno lleva dos ediciones
  del programa: la universal, para cualquier procesador x86-64, y la rápida, **solo para procesadores compatibles
  con AVX2** (Intel desde 2013, AMD desde 2015), con la que la IA local va unas diez veces más rápida. Al arrancar
  se usa la rápida si el procesador la admite.
* Ediciones **sin probar en equipo real** para Windows (universal, rápida con AVX2 y con CUDA), macOS en Apple
  Silicon (normal y con Metal) y Linux con CUDA (`.deb` y `.rpm`; las de CUDA necesitan el controlador de NVIDIA y
  CUDA 12 instalados), en la misma lista de huellas firmada. No llevan firma de Apple ni
  de Microsoft: macOS pide autorizar la aplicación al abrirla y Windows muestra el aviso de SmartScreen. Los
  informes de esas plataformas son bienvenidos.
