# Distribución en Linux

> **Estado:** Implementado: `.deb`, `.rpm`, AppImage y `.tar.gz`, generados por `scripts/empaquetar-linux.mjs`.

Linux es la plataforma de desarrollo principal del proyecto y, por su presencia en entornos educativos, un
objetivo de primera clase.

## 1. Formatos de paquete

| Formato | Uso | Notas |
|---|---|---|
| AppImage | Distribución universal directa | Sin instalación, portable. Se actualiza desde Ajustes |
| `.tar.gz` | Arch y derivadas, y cualquier distribución sin `.deb` ni `.rpm` | El programa solo, con las bibliotecas del sistema. Ver 1.3 |
| `.deb` | Debian, Ubuntu y derivadas | Declara dependencias de WebKitGTK |
| `.rpm` | Fedora, openSUSE | Declara dependencias de WebKitGTK |

### 1.1 El AppImage necesita `NO_STRIP=1` en una distribución moderna

`tauri build` produce el `.deb` y el `.rpm` sin problemas y **falla al llegar al AppImage**, con un mensaje que no
dice nada: `failed to run linuxdeploy`. El error real solo aparece con `--verbose`, y es el mismo repetido en cada
biblioteca del sistema:

```text
ERROR: Strip call failed: .../strip: .../libwebkit2gtk-4.1.so.0:
       unknown type [0x13] section `.relr.dyn'
```

La causa no es del proyecto. `linuxdeploy` trae empotrado un `strip` compilado en julio de 2024 que **no entiende
la sección `.relr.dyn`**, que las bibliotecas de una distribución de actualización continua ya usan. Falla en
WebKitGTK, wayland, libxml2, libwebp, zstd y una docena más.

La salida es decirle que no haga `strip`:

```bash
NO_STRIP=1 pnpm tauri build
```

**Lo que cuesta.** El AppImage sale sin desnudar de símbolos: 107 MB frente a los 9 del `.deb`. La mayor parte no
son símbolos sino WebKitGTK entero, que un AppImage tiene que llevar dentro por definición; el `strip` habría
ahorrado una parte, no el grueso.

**Lo que hay que vigilar.** Esto se arregla solo el día que `linuxdeploy` publique una versión con un `binutils`
más nuevo. Conviene comprobarlo de vez en cuando en lugar de dejar la variable puesta para siempre sin
preguntarse por qué está.

### 1.2 Instalar desde el código: el programa fuera de la carpeta de datos

`pnpm instalar:local` (`just install-local`), después de `NO_STRIP=1 pnpm empaquetar:linux`, extrae el `.tar.gz` en
`~/.local/opt/nb-books` y escribe el lanzador en `~/.local/share/applications/nb-books.desktop`. Los datos se
quedan aparte, en `~/.local/share/nb-books`: el programa nunca vive dentro de la carpeta de datos. El cambio se
hace con un `rename` sobre una carpeta hermana, así que una instalación a medias no deja al equipo sin
aplicación, y lo que la instalación antigua dejó entre los datos se borra por nombre, nunca por patrón.

### 1.3 El `.tar.gz`: el programa sin WebKitGTK dentro

`nb-books_<versión>_linux-x86_64.tar.gz` trae una sola carpeta, `nb-books-<versión>-linux-x86_64/`, con `bin/`
(la edición universal y la rápida), `lib/NB Books/` (el motor de PDF y los avisos legales) y `share/` (el
lanzador y los iconos). Se descomprime donde se quiera y se ejecuta `bin/nb-books`: el binario busca el motor de
PDF y la edición rápida **respecto a sí mismo**, así que no necesita instalarse en `/usr` ni un lanzador que le
prepare el entorno.

**Por qué existe.** En una distribución sin `.deb` ni `.rpm`, la alternativa sería el AppImage, que lleva WebKitGTK
entero: 140 MB comprimido y 380 extraído. El `.tar.gz` pesa lo que el `.deb` y usa las bibliotecas del sistema. El
actualizador integrado lo actualiza en su sitio.

**Lo que pide al sistema**, y no trae: WebKitGTK 4.1, GTK 3, y GStreamer con los complementos `base` y `good`
para el audio. En Arch, `webkit2gtk-4.1`, `gst-plugins-base` y `gst-plugins-good`. Sin ellos no arranca, y no hay
gestor de paquetes que lo avise: es el precio de un archivo que no es un paquete.

**De dónde sale.** Del mismo árbol que el empaquetador monta para el `.deb`, no de una lista de archivos propia
(`scripts/empaquetar-linux.mjs`, paso 4).

## 2. Dependencia de WebKitGTK

La aplicación depende del motor WebKitGTK del sistema. Esto implica que:

* El comportamiento del renderizado varía entre distribuciones y versiones.
* Las vulnerabilidades del motor se corrigen por el sistema, no por el proyecto.
* Debe declararse una versión mínima verificada y probarse en al menos una distribución de ciclo largo y una de
  liberación continua.

Las diferencias de motor deben tratarse como un riesgo de calidad real, no como un detalle: es la causa más
frecuente de errores visuales reportados exclusivamente en Linux.

### 2.1 El sonido pasa por GStreamer, y hay que llevarlo

WebKitGTK reproduce todo lo que suena -el `<audio>` de la voz neuronal y del audio preparado, y los fondos
sonoros- con **GStreamer**, y sus complementos no vienen con el motor. Lo encontró la puerta de empaquetado de la
El desarrollo: el AppImage arrancado sobre un perfil limpio decía `GStreamer element appsink not found`. Ninguna prueba
podía verlo, porque los recorridos corren en Chromium y el binario de desarrollo usa el GStreamer del sistema.

* **AppImage**: `bundle.linux.appimage.bundleMediaFramework` mete GStreamer y sus complementos dentro. Lo hace más
  grande, y es el precio de que funcione sin instalar nada, que es para lo que existe un AppImage.
* **`.deb` y `.rpm`**: declaran `gstreamer1.0-plugins-base` y `-good` (`gstreamer1-plugins-*` en RPM) como
  dependencias. Con eso está el demultiplexor de Ogg, el decodificador de Opus y la salida de audio.

**Cómo se construye el AppImage**, en local y en `ediciones.yml`: hace falta `patchelf` -el complemento de
`linuxdeploy` lo exige en el `PATH` del anfitrión y sin él falla con `Error: patchelf not found`, aunque el propio
`linuxdeploy-x86_64.AppImage` lo lleva dentro en `usr/bin/patchelf`- y una carpeta con **solo los complementos que la
aplicación usa**, que prepara `node scripts/preparar-gstreamer.mjs`. Si el sistema no tiene `patchelf`
instalado aparte, `preparar-gstreamer.mjs` lo extrae de `linuxdeploy` a `src-tauri/target/gstreamer/bin/patchelf` y
`scripts/tauri.mjs` añade esa carpeta al `PATH` automáticamente. Sin la carpeta de complementos filtrada, el
complemento copiaría todos los del sistema, que en Arch son cerca de trescientos e incluyen los de `ugly` y `libav`,
que son GPL:

```bash
node scripts/preparar-gstreamer.mjs # dice las dos variables GSTREAMER_*_DIR
GSTREAMER_PLUGINS_DIR=... GSTREAMER_HELPERS_DIR=... NO_STRIP=1 pnpm tauri build
```

Comprobado: con solo esos dieciocho en la ruta de complementos, GStreamer reproduce un Ogg Opus de principio a fin,
y sin el de Opus dice que falta el decodificador. El AppImage pesa 130,1 MB.

### 2.2 Lo que la ventana puede leer del disco

La ventana lee algunos archivos directamente con el protocolo de archivos de Tauri, y solo lo que cae dentro de
`app.security.assetProtocol.scope`: la caché y `$DATA/<aplicación>/audio/**`, donde vive el audio preparado, y
**nada más de datos**: abrir la carpeta entera daría a la
ventana lectura de la base, que es la biblioteca. Lo fija `tests/alcance_de_archivos.rs`, en las dos
direcciones.

## 3. Integración con el escritorio

Los paquetes instalan un lanzador `.desktop` de la categoría `Education` y los iconos en los tamaños estándar del
tema, generados a partir de `docs/assets/branding/NB_Books.svg`. Los documentos se abren desde la propia aplicación
(Biblioteca, Importar).

## 4. Accesibilidad

El lector de pantalla de referencia en Linux es Orca, que accede al árbol de accesibilidad expuesto por
WebKitGTK. La verificación con Orca forma parte de la lista de comprobación de cada publicación (ver
[architecture/accessibility.md](../../architecture/accessibility.md)).

## 5. Despliegue en equipos gestionados

Instalación desatendida por paquete del sistema, datos por usuario y red: ver
[institution/deployment.md](../../institution/deployment.md).
