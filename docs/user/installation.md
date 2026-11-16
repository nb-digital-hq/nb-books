# Instalación

NB Books funciona en Linux. Elige el paquete según tu distribución:

| Si usas... | Descarga | Cómo se instala |
|---|---|---|
| Ubuntu, Debian, Linux Mint, Pop!_OS | `.deb` | Ábrelo con el instalador de software, o `sudo apt install ./archivo.deb` |
| Fedora, openSUSE | `.rpm` | Ábrelo con el instalador de software, o `sudo dnf install ./archivo.rpm` |
| Arch, Manjaro u otra | `.tar.gz` | Descomprímelo y ejecuta el programa de la carpeta `bin/` |
| Sin permisos de instalación | AppImage | Dale permiso de ejecución y ábrelo con doble clic |

## Comprobar que la descarga es auténtica

Junto a cada versión se publican `SHA256SUMS` y su firma `SHA256SUMS.sig`. Si quieres asegurarte de que nadie ha
alterado el archivo, sigue los pasos de [comprobar una publicación](../developer/deployment/releases.md) (sección
6). Es opcional, pero recomendable.

## Desinstalar

Desinstala el paquete como cualquier otro programa. Tus datos (biblioteca, anotaciones, notas) se quedan en
`~/.local/share/nb-books` por si vuelves; bórrala si quieres eliminarlos. **Tus libros nunca se tocan**: siguen en
las carpetas donde estaban.
