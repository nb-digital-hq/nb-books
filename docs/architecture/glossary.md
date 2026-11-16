# Glosario (Glossary)

> **Estado:** Vigente. Vocabulario obligatorio en código, discusiones y documentación.

Para mantener coherencia en el código base, las discusiones arquitectónicas y la documentación, todo contribuidor de NB Books debe utilizar los siguientes términos con su significado estricto.

## Conceptos Core Documentales

* **Resource (Recurso):** La abstracción más alta. Un recurso puede ser cualquier activo de conocimiento (un documento físico digitalizado, un libro, un audio, etc.). Es la entidad teórica.
* **Document (Documento):** Un archivo procesable por el sistema. NB Books se centra en el "Documento", abstrayendo su extensión de archivo subyacente.
* **Publication (Publicación):** Un documento empaquetado y comercial o académico, usualmente dotado de ISBN, DOI o metadatos formales.
* **Book (Libro):** Subtipo de Publication. Texto extenso generalmente estructurado por capítulos (ej. novelas, libros de texto).
* **Article (Artículo):** Subtipo de Publication. Texto breve e investigativo (ej. papers científicos, artículos de revistas).
* **Comic / Graphic Novel:** Documento centrado en narrativa visual estática (archivos cbz, cbr).
* **Audiobook:** Recurso auditivo diseñado para el aprendizaje o esparcimiento.
* **Collection (Colección):** Agrupación manual o dinámica de recursos gestionada por el usuario.
* **Library (Biblioteca):** El conjunto total de recursos de un perfil.

## Conceptos de Interacción

* **Reading Session (Sesión de Lectura):** Período de tiempo medible durante el cual un usuario interactúa con un Documento usado para las estadísticas locales.
* **Annotation (Anotación):** Texto, trazo libre o comentario vinculado a un punto específico (o rango semántico) de un documento. Son siempre locales.
* **Bookmark (Marcador):** Referencia posicional simple para regresar a una parte del documento.

## Términos Técnicos Restringidos

* **Format (Formato):** Se refiere a la extensión técnica del archivo (`.epub`, `.pdf`, `.mp3`). NB Books diseña para el *Document*, no para el *Format*.
* **SPOF (Single Point of Failure):** Punto Único de Fallo. Componente crítico cuya falla derriba todo el sistema. NB Books está diseñado para prohibir SPOFs arquitectónicos.
* **Graceful Degradation:** La habilidad del entorno de seguir funcionando con herramientas limitadas si un componente de red, servicio o proceso local colapsa.

