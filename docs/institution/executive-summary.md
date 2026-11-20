# Resumen ejecutivo

> **Audiencia:** dirección y comités de evaluación de software.

## Qué es

NB Books es una aplicación de escritorio de código abierto (Apache 2.0) que reúne en un solo entorno la biblioteca
digital, la lectura de EPUB, PDF y cómics, las anotaciones, la búsqueda en texto completo, las notas de estudio, la
consulta de diccionarios y la lectura en voz alta. Está diseñada para bibliotecas de cientos de miles de documentos
y funciona sin conexión.

## Qué problema resuelve

El estudio digital suele repartirse entre un gestor de biblioteca, varios lectores, un diccionario, un traductor y
una aplicación de notas que no se comunican entre sí, y a menudo depende de servicios en la nube que retienen las
anotaciones del estudiante. NB Books elimina esa fricción y deja los datos en el equipo de la persona.

## Por qué es una adopción de bajo riesgo

| Criterio | Qué ofrece |
|---|---|
| Coste de licencia | Ninguno para la aplicación. Apache 2.0 permite uso, modificación y redistribución, incluso en entornos comerciales |
| Datos personales | La aplicación no los transmite: no hay cuentas, telemetría ni servidores del proyecto que reciban datos |
| Conectividad | Funciona sin red. Las funciones en línea (catálogos, metadatos, consulta) exigen consentimiento por servidor y propósito |
| Dependencia del proveedor | Formatos abiertos, exportación completa de la biblioteca y código auditable |
| Seguridad | Una sola puerta de red, extensiones en cajón de arena, publicaciones firmadas con Ed25519 y modelo de amenazas publicado |
| Accesibilidad | Navegación completa por teclado, compatibilidad con lectores de pantalla, temas de alto y bajo contraste |
| Rendimiento | Bibliotecas muy grandes sin congelar la interfaz; presupuestos de rendimiento verificados automáticamente |

## Plataforma

Linux (paquetes `.deb`, `.rpm`, `.tar.gz` y AppImage). Interfaz en español, inglés y portugués.
