# Despliegue y distribución

> **Estado:** Implementado para Linux.

Cómo llega NB Books a la máquina de quien lo usa. La automatización está en [ci-cd.md](../ci-cd.md) y el corte de
versiones en [releases.md](../releases.md).

| Documento | Contenido |
|---|---|
| [packaging.md](packaging.md) | Contenido del paquete y dependencias nativas |
| [linux.md](linux.md) | `.deb`, `.rpm`, AppImage y `.tar.gz`; WebKitGTK y GStreamer |
| [releases.md](releases.md) | Publicaciones, firma, actualizador integrado y cómo comprobar una publicación |

## Principios

1. **Instalación sin privilegios cuando es posible.** El AppImage y el `.tar.gz` se instalan para el usuario actual
   sin administrador.
2. **Desinstalar no borra la biblioteca.** Los datos se quedan en la carpeta de datos del usuario y los documentos en
   sus carpetas.
3. **Ubicaciones estándar del sistema** (especificación XDG).
4. **Publicaciones verificables:** `SHA256SUMS` firmado con Ed25519, listado de componentes CycloneDX y avisos de
   terceros.
5. **Sin sorpresas de red.** Un binario recién instalado no hace ninguna petición; la búsqueda de actualizaciones
   la dispara la persona desde Ajustes.

## Rutas de datos

| Dato | Ruta |
|---|---|
| Base de datos, registros, modelos, audio preparado y biblioteca administrada | `$XDG_DATA_HOME/nb-books` (`~/.local/share/nb-books`) |
| Configuración | `$XDG_CONFIG_HOME/nb-books` |
| Caché regenerable (portadas, páginas rendidas) | `$XDG_CACHE_HOME/nb-books` |

La caché se puede borrar sin pérdida de información.
