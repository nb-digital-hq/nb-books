# Contratos y APIs

Interfaces técnicas vigentes de NB Books. A diferencia de [architecture](../README.md), que describe el diseño,
esta sección documenta los **contratos** como referencia de consulta.

> **Estado:** Implementado. Cada documento declara su propia etiqueta de estado.

## Documentos

| Documento | Contenido |
|---|---|
| [services.md](services.md) | Catálogo de los comandos IPC registrados |
| [schemas.md](schemas.md) | Esquema de persistencia |
| [export-format.md](export-format.md) | Formato de exportación e intercambio, versión 1 |
| [annotation-export.md](annotation-export.md) | Salida de anotaciones en el modelo de anotación web del W3C |
| [events.md](events.md) | Catálogo de eventos del bus |
| [plugins.md](plugins.md) | API de extensiones |
| [nb-extension.schema.json](nb-extension.schema.json) | Esquema del manifiesto de extensión, generado |
| [clave-de-publicacion.pem](clave-de-publicacion.pem) | Clave pública con la que se verifican las publicaciones |
| [clave-del-launcher.pub](clave-del-launcher.pub) | Clave minisign pública con la que NB Launcher verifica su catálogo |

## Extensibilidad

La funcionalidad avanzada que no sea de uso general debe implementarse como extensión, para no cargar el runtime
de todos los usuarios. El producto base, sin embargo, debe ser completo: no se puede obligar al usuario a
instalar diez extensiones para poder leer de forma satisfactoria.

Las reglas de compatibilidad de estos contratos están en
[versioning.md](../versioning.md). Las reglas de seguridad del puente IPC están en
[ipc-contracts.md](../ipc-contracts.md).
