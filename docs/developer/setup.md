# Puesta en Marcha del Entorno

> **Estado:** Aplicable en Linux, la plataforma de desarrollo y de publicación.

## 1. Requisitos

| Herramienta | Versión | Cómo se fija |
|---|---|---|
| Rust | 1.98.0 | `rust-toolchain.toml`; `rustup` la instala sola al compilar |
| Fuentes de la biblioteca estándar | La del compilador | **Hacen falta para el editor, no para compilar** |
| Node.js | 22 o superior | `.nvmrc` |
| pnpm | 10 o superior | Campo `packageManager` de `package.json` |

El proyecto **no** admite npm ni yarn: el fichero de bloqueo es de pnpm y usar otro gestor produce un árbol de
dependencias distinto del que verifica la integración continua.

### Dependencias del sistema

**Linux (Debian y derivadas):**
```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Linux (Arch y derivadas):**
```bash
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl \
  libayatana-appindicator librsvg
```

**Windows:** Microsoft C++ Build Tools y el runtime de WebView2 (preinstalado en Windows 11).

**macOS:** herramientas de línea de órdenes de Xcode (`xcode-select --install`).

## 2. Instalación

```bash
git clone https://github.com/nb-digital-hq/nb-books.git
cd nb-books
pnpm install # instala dependencias y activa los ganchos de git
pnpm exec playwright install chromium # el navegador de las pruebas de interfaz
pnpm tauri dev # arranca la aplicación en modo desarrollo
```

> **Sin `--with-deps`.** Ese modificador intenta instalar paquetes del sistema con `apt-get` y falla en cualquier
> distribución que no sea Debian o Ubuntu. En Arch, las bibliotecas que Chromium necesita ya están en el sistema.

El navegador se descarga una sola vez, fuera del repositorio. Sin él, `pnpm verify` falla en las pruebas de
interfaz con un mensaje que dice exactamente qué ejecutar: se prefiere que falle a que se salte en silencio.

#### El aviso de "OS no soportado", y por qué no dice nada del producto

Fuera de Debian y Ubuntu, la descarga avisa:

```
BEWARE: your OS is not officially supported by Playwright;
downloading fallback build for ubuntu24.04-x64.
```

Es cierto y es inocuo. Playwright solo publica compilaciones oficiales de Chromium para unas pocas
distribuciones; en Arch, Fedora o cualquier otra baja la de Ubuntu 24.04, que arranca porque las bibliotecas del
sistema son compatibles. La suite entera pasa con ella.

**Y no dice nada sobre las plataformas que NB Books soporta.** Playwright es dependencia de desarrollo: no entra
en el binario de Tauri, no aparece en los avisos de terceros y no toca el empaquetado. Qué sistemas soporta el
producto lo decide `tauri build`, no esto.

Si la compilación de Playwright no arranca en una distribución, `NB_NAVEGADOR` la sustituye sin tocar nada más:

```bash
NB_NAVEGADOR=chrome pnpm test:e2e # el Chrome del sistema; también `msedge`
NB_NAVEGADOR=/usr/bin/chromium pnpm test:e2e # un ejecutable concreto
```

Por defecto se usa la compilación de Playwright, que es la misma para todo el mundo: dos personas con versiones
distintas de Chrome verían resultados distintos sin saber por qué, y una suite que no es reproducible no sirve
para decidir si algo está roto.

La primera compilación de Rust tarda varios minutos porque construye todo el árbol de dependencias de Tauri. Las
siguientes son incrementales.

## 3. Comprobaciones antes de subir cambios

```bash
pnpm verify # lints, tipos, pruebas del frontend, recorridos de interfaz en un
                      # navegador real, sistema de diseño, integridad documental, contrato
                      # de errores, salida de red, deriva del puente y licencias

cd src-tauri
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

`pnpm verify` necesita la cadena de Rust instalada: la comprobación de deriva del puente regenera los tipos con
`cargo`, y la de avisos de terceros lee `cargo metadata`.

Los recorridos de interfaz (`pnpm test:e2e`) compilan el frontend para producción y lo conducen con Playwright.
Tardan unos diez segundos y van dentro de `pnpm verify` a propósito: son el único mecanismo que detecta lo que
solo ocurre con el DOM compuesto —un bucle de efectos, un rechazo sin capturar, un foco que no llega—, y una
suite que hay que acordarse de lanzar deja de lanzarse. Lo que cubren y lo que **no**, en
[testing/regression.md](testing/regression.md).

### El registro del servidor de desarrollo, y un aviso que no era un error

Si al arrancar aparecen quince líneas como esta:

```
[vite-plugin-svelte:load] failed to load virtual css module .../LibraryView.svelte?svelte&type=style&lang.css
```

**el CSS se está sirviendo bien.** Es una carrera del servidor de desarrollo: el navegador pide el CSS de un
componente antes de que la transformación de su JS haya registrado los estilos en la caché de
`vite-plugin-svelte`, así que ese complemento avisa y devuelve el control, y otro sirve el CSS correcto detrás.
Medido pidiendo la misma URL en frío y en caliente: **respuesta idéntica byte a byte, 200 las dos**.

Sale con el webview de Tauri y no con Chromium, porque piden en distinto orden, y solo en desarrollo: al
compilar, el CSS va en un archivo.

Aun así se quitó, porque quince líneas rojas al arrancar enseñan a no leer el registro y entonces el error que sí
importa pasa desapercibido. `server.warmup` transforma los componentes al arrancar, antes de que nadie los pida.
Comprobado con la misma secuencia de peticiones en los dos casos: sin esa línea, dos avisos; con ella, ninguno.

**Se puede verificar con `pnpm tauri dev` abierto.** Esa compilación escribe en `.svelte-kit-e2e/` y
`build-e2e/`, no en los directorios que usa el servidor de desarrollo, y el servidor no vigila ninguna salida de
compilación; si compartieran sitio, la aplicación de desarrollo se quedaría sin estilos con un
`failed to load virtual css module` sin pista de la causa. Lo sostienen tres pruebas en `tests/aislamiento-de-compilacion.test.ts`.

### `pnpm tauri dev` compila la edición rápida si el procesador la admite

`pnpm tauri` pasa por `scripts/tauri.mjs`. Con `dev`, en Linux x86-64 con AVX2 y FMA, compila con
`-C target-cpu=x86-64-v3` -lo mismo que la edición rápida que se empaqueta- y en su propia carpeta,
`src-tauri/target/desarrollo`, para que lo que se prueba en desarrollo vaya a la velocidad de la edición rápida. La
carpeta aparte es para que cambiar `RUSTFLAGS` no invalide lo que comparten `cargo build` y las
pruebas.

`NB_EDICION=universal pnpm tauri dev` compila la universal, para ver lo que ve quien no tiene AVX2; un `RUSTFLAGS`
puesto a mano también manda. El resto de subórdenes, incluido `build`, pasa sin tocar. Ver
[local-models.md](../architecture/ai/local-models.md), sección 6.3.

**Lo que no cambia**: el crate propio se sigue compilando en desarrollo con `opt-level = 0`, y la voz neuronal
tiene bucles propios -la transformada inversa y las LSTM- que sin optimizar van bastante más despacio. En
desarrollo la voz tarda más que en la edición empaquetada; la IA generativa, que pasa casi entera por Candle
(`opt-level = 2` en las dependencias), mucho menos.

### El motor de PDF, aparte

```bash
pnpm pdfium # descarga la biblioteca nativa con hash fijado
pnpm check:pdfium # dice si está y si es la versión esperada
```

**No hace falta para compilar ni para arrancar.** Sin ella la aplicación funciona, lee EPUB y declara el
componente de lectura como degradado, con el PDF oculto. Es deliberado: el repositorio no versiona binarios, y
enlazarla al compilar haría que un clon limpio dejara de compilar sin ejecutar antes un script.

Los ganchos de git ejecutan parte de esto automáticamente al hacer commit. Se activan con `pnpm install`; si los
desactivaste, vuelve a activarlos con `pnpm run prepare`. **No son una red completa:** se saltan con
`--no-verify`. Ejecutar `just verify` antes de subir no es opcional; el flujo de integración continua lo repite
sobre cada cambio.

**Firma tus commits.** La declaración de origen es obligatoria:

```bash
git commit -s -m "feat(library): añadir importación por lotes"
```

### El directorio de compilación crece, y hay que vaciarlo de vez en cuando

`src-tauri/target/` **no se limpia solo**. Cargo no recoge la basura de artefactos antiguos, y cada iteración
deja atrás las versiones anteriores de todos los binarios de prueba, que enlazan SQLite estáticamente y pesan
varios megabytes cada uno. A eso se suma la compilación incremental.

Puede llegar a decenas de gigabytes; una compilación completa desde cero, con toda la suite de pruebas, ocupa unos
**15 GB**, y el resto es basura de compilaciones anteriores.

```bash
cd src-tauri && cargo clean # libera el directorio entero
```

El coste es una recompilación completa la siguiente vez, que son minutos. No se pierde nada más:
`src-tauri/vendor/libpdfium.so` vive **fuera** de `target/` justamente por esto, así que no hay que volver a
descargarlo.

Conviene hacerlo cuando el disco apriete, y comprobarlo de vez en cuando con `du -sh src-tauri/target`.

## 4. Estructura del repositorio

```
docs/ Fuente única de verdad: arquitectura, decisiones, plan, legal, gobernanza
src/ Frontend (SvelteKit 2 + Svelte 5)
  lib/core/      Capa de acceso al runtime; ningún componente invoca el puente directamente
  routes/        Rutas de la aplicación
src-tauri/ Runtime (Rust + Tauri 2)
  src/error.rs   Tipo de error de dominio; ningún error viaja como cadena
  src/paths.rs   Rutas por plataforma; datos y caché separados
  src/ipc.rs     Superficie IPC: la frontera de seguridad
  src/*.rs       Un módulo por servicio del runtime
scripts/ Utilidades de verificación
assets/ Recursos de marca provisionales
```

## 5. Editor

`.vscode/extensions.json` recomienda las extensiones necesarias; el editor las ofrecerá al abrir el repositorio.
La configuración compartida solo contiene ajustes del proyecto, no preferencias personales.

## 6. Aislamiento durante las pruebas

La variable `NB_BOOKS_TEST_DIR` desvía **todas** las rutas de la aplicación a un directorio temporal. Úsala
siempre que ejecutes pruebas que escriban en disco, para no tocar tu biblioteca real:

```bash
NB_BOOKS_TEST_DIR=/tmp/nb-books-pruebas pnpm tauri dev
```

## Las fuentes de la biblioteca estándar, y el error que aparece sin ellas

`cargo build`, `cargo test` y `clippy` **no necesitan** las fuentes de la biblioteca estándar. `rust-analyzer`
sí, y sin ellas marca errores que no existen: los más llamativos aparecen dentro de las macros de `tracing`
—`info!`, `warn!`, `error!`— y en los `match` sobre tipos del propio proyecto, porque no puede resolver lo que
esas macros expanden.

El síntoma engaña, y por eso queda escrito: el editor enseña doce errores en un archivo que compila sin una sola
advertencia, ni siquiera con `-D warnings`. Antes de buscar el fallo en el código, compruébalo:

```bash
ls /usr/lib/rustlib/src/rust/library # con rustup: rustup component list --installed | grep rust-src
cargo clippy --all-targets --all-features -- -D warnings
```

Si lo primero no existe y lo segundo pasa, el problema es del editor. Se resuelve instalando las fuentes:

```bash
rustup component add rust-src # instalación con rustup
sudo pacman -S rust-src # Arch y derivadas, con Rust del sistema
```

Y después, en el editor, recargar el espacio de trabajo (`rust-analyzer: Restart Server`).

**Con Rust del sistema en lugar de `rustup`**, `rust-toolchain.toml` no hace nada: lo lee `rustup`, y sin él la
versión es la que haya instalada. Comprueba que coincide con la de la tabla; el proyecto no compila con menos.
