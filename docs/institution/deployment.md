# Despliegue en equipos gestionados

> **Audiencia:** áreas de TI que administran laboratorios, aulas o flotas de equipos.
> **Estado:** Implementado para Linux.

## 1. Paquetes disponibles

| Paquete | Uso recomendado | Dependencias |
|---|---|---|
| `.deb` | Debian, Ubuntu y derivadas; repositorios internos APT | Declara WebKitGTK y GStreamer |
| `.rpm` | Fedora, openSUSE, RHEL y derivadas; repositorios internos DNF/Zypper | Declara WebKitGTK y GStreamer |
| `.tar.gz` | Cualquier distribución; despliegue por copia en `/opt` | Usa WebKitGTK y GStreamer del sistema |
| AppImage | Equipos sin privilegios de instalación | Autocontenido |

Cada publicación incluye `SHA256SUMS`, su firma Ed25519 `SHA256SUMS.sig` y un registro de procedencia. Antes de
publicar un paquete en un repositorio interno, la firma debe verificarse según
[releases](../developer/deployment/releases.md), sección 6.

## 2. Instalación desatendida

Los paquetes `.deb` y `.rpm` no hacen preguntas durante la instalación, de modo que se distribuyen con cualquier
herramienta de gestión de configuración (Ansible, Salt, Puppet, Foreman) o desde un repositorio interno:

```bash
# Debian / Ubuntu
sudo apt-get install -y ./nb-books_<versión>_amd64.deb
# Fedora / RHEL
sudo dnf install -y ./nb-books-<versión>-1.x86_64.rpm
```

## 3. Datos y perfiles

| Qué | Dónde (por usuario) |
|---|---|
| Base de datos y registros | `$XDG_DATA_HOME/nb-books` (por omisión `~/.local/share/nb-books`) |
| Configuración | `$XDG_CONFIG_HOME/nb-books` |
| Caché regenerable | `$XDG_CACHE_HOME/nb-books` |
| Credenciales de servicios opcionales | Llavero del sistema (Secret Service) |

* Las bibliotecas compartidas en red (NFS, SMB) se añaden como **referenciadas**: la aplicación las lee y no
  escribe en ellas, por lo que pueden montarse en solo lectura.
* En equipos compartidos, cada cuenta del sistema tiene su propia biblioteca, anotaciones y progreso.
* La caché puede borrarse sin pérdida de datos.
* La copia de seguridad de una persona consiste en su carpeta de datos, o en la exportación completa desde Ajustes.

## 4. Red

* El **modo sin red** bloquea toda conexión.
* Sin él, ninguna conexión sale sin un consentimiento por servidor y propósito que concede la propia persona.
* Para cortafuegos con lista blanca, los únicos destinos posibles son los que la persona autorice: catálogos OPDS,
  servicios de metadatos, enciclopedia y traducción, descarga de modelos, comprobación de versión, registro de
  extensiones y proveedores de IA que configure. Ver la [declaración de privacidad](../audit/privacy.md).

## 5. Actualizaciones

Las actualizaciones se distribuyen como nuevos paquetes firmados. En equipos gestionados se recomienda publicarlos
en el repositorio interno tras verificar su firma, y mantener desactivada la comprobación de versión en la
aplicación para que el ciclo lo controle TI.
