# Recursos de marca

> **Estado:** Vigente.

| Archivo | Qué es |
|---|---|
| `NB_Books.svg` | El icono: un disco `#FEB61A` y la silueta `#FEF9F2`, en vector. De aquí sale todo lo demás, y no se edita: se sustituye entero |
| `app-icon.png` | El maestro del icono que ve el sistema operativo: el SVG trazado a 1024 px, sin retocar |

Ninguno de los dos es el archivo que usa la interfaz: esa monta `static/logo.png` como máscara y toma el color
del tema vigente, y por eso no puede llevar color dentro. El icono del sistema sí lo lleva, porque fuera de la
aplicación no hay tema del que heredar. Qué se le hizo a cada uno, y por qué, está en
[icons.md](../../design/icons.md), sección "La marca".

Para regenerar todo tras sustituir el SVG:

```bash
rsvg-convert -w 1024 -h 1024 docs/assets/branding/NB_Books.svg -o docs/assets/branding/app-icon.png
rsvg-convert -w 128 -h 128 docs/assets/branding/NB_Books.svg -o static/favicon.png
pnpm tauri icon docs/assets/branding/app-icon.png
rm -rf src-tauri/icons/android src-tauri/icons/ios
```

`static/logo.png` se rehace aparte, porque es solo la silueta: el procedimiento está en
[icons.md](../../design/icons.md), sección "Qué se le hizo al archivo".

NB Books se distribuye solo para escritorio; los iconos de plataformas móviles se descartan.
