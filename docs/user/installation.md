# Instalación

NB Books está verificado en Linux. Elige el paquete según tu distribución:

| Si usas... | Descarga | Cómo se instala |
|---|---|---|
| Ubuntu, Debian, Linux Mint, Pop!_OS | `.deb` | Ábrelo con el instalador de software, o `sudo apt install ./archivo.deb` |
| Fedora, openSUSE | `.rpm` | Ábrelo con el instalador de software, o `sudo dnf install ./archivo.rpm` |
| Arch, Manjaro u otra | `.tar.gz` | Descomprímelo y ejecuta el programa de la carpeta `bin/` |
| Sin permisos de instalación | AppImage | Dale permiso de ejecución y ábrelo con doble clic |

## Windows y macOS

Se publican **sin probar en equipo real**, para que quien los use pueda contar qué funciona y qué no.

| Si usas... | Descarga |
|---|---|
| Windows 10 u 11 | `windows-x64-setup.exe` (o `.msi`). Si tu procesador admite AVX2, la `-avx2`; con tarjeta NVIDIA, la `-cuda` |
| macOS en Apple Silicon (M1 o posterior) | `macos-arm64.dmg`, o la `-metal` para usar la GPU con la IA local |

Las ediciones `-cuda` (Windows y Linux) necesitan el controlador de NVIDIA y CUDA 12 instalados; sin ellos, usa la
universal o la rápida. No llevan firma de Apple ni de Microsoft. En macOS, la primera vez abre la aplicación con clic derecho y Abrir; en
Windows, en el aviso de SmartScreen elige Más información y Ejecutar de todas formas.

## Comprobar que la descarga es auténtica

Junto a cada versión se publican `SHA256SUMS` y su firma `SHA256SUMS.sig`. Si quieres asegurarte de que nadie ha
alterado el archivo, sigue los pasos de [comprobar una publicación](../developer/deployment/releases.md) (sección
6). Es opcional, pero recomendable.

## Desinstalar

Desinstala el paquete como cualquier otro programa. Tus datos (biblioteca, anotaciones, notas) se quedan en
`~/.local/share/nb-books` por si vuelves; bórrala si quieres eliminarlos. **Tus libros nunca se tocan**: siguen en
las carpetas donde estaban.
