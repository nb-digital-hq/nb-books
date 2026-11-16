# Marca e identidad

> **Estado:** Vigente. Los archivos fuente están en [assets/branding](../assets/branding/README.md); el uso de la
> marca en la interfaz, en [icons.md](icons.md), sección "La marca"; la política de uso por terceros, en
> [trademark.md](../legal/trademark.md).

## 1. Identidad verbal

* **Nombre:** NB Books, con las dos mayúsculas y un espacio. Nunca "NBBooks", "Nb Books" ni "NB-Books" en texto;
  `nb-books` solo como nombre de paquete o de repositorio.
* **Quién lo publica:** NB Digital.
* **En una frase:** una biblioteca y un lector de escritorio que funcionan sin conexión, sin cuenta y sin
  telemetría.

## 2. Identidad visual

| Elemento | Valor |
|---|---|
| Símbolo | Dos hojas que se abren desde un eje central, sobre un disco |
| Disco | `#FEB61A` |
| Silueta | `#FEF9F2` |
| Fuente vectorial | `docs/assets/branding/NB_Books.svg` |
| Icono del sistema | `docs/assets/branding/app-icon.png`, 1024 px, y los tamaños que genera `pnpm tauri icon` |

Dentro de la aplicación la silueta no lleva color propio: toma el del tema vigente sobre un disco del color de
acento (ver [icons.md](icons.md)). Fuera de la aplicación, donde no hay tema del que heredar, se usan los dos
colores de la tabla.

**Usos incorrectos:** deformar o girar el símbolo, cambiar sus colores fuera de la aplicación, ponerle sombras o
contornos, o acompañarlo de otro nombre.

## 3. Tono

Profesional, directo y sin adornos, en la interfaz, en la documentación y en las notas de cada versión. Sin
emoticonos. La aplicación habla de lo que hace, no de sí misma.

## 4. Por qué la marca va aparte del código

El nombre y el símbolo son lo único que **no** se licencia con el código bajo Apache 2.0: permiten distinguir la
distribución oficial de una bifurcación e impiden que un tercero distribuya una versión modificada con el mismo
nombre. Ver [TRADEMARKS.md](../../TRADEMARKS.md).
