# Empaquetado

> **Estado:** Implementado para Linux (`scripts/empaquetar-linux.mjs`).

## 1. Contenido del paquete

El paquete contiene el binario de Rust (edición universal y edición rápida con AVX2), los recursos compilados del
frontend, los iconos, el motor de PDF y los avisos legales (`LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md`). **No**
contiene pesos de modelos de IA ni voces: se descargan bajo petición y verificados por huella (ver
[ai/local-models.md](../../architecture/ai/local-models.md)).

## 2. Dependencias nativas

### 2.1 PDFium

* **No vive en el repositorio.** `pnpm pdfium` (`scripts/fetch-pdfium.mjs`) la obtiene de una fuente publicada con
  hash fijado, y se empaqueta en `lib/NB Books/`.
* **Sin JavaScript:** se usa la variante `pdfium-*` y no `pdfium-v8-*`.
* **Licencia BSD** con componentes de terceros declarados en los avisos.
* Es código C++ que procesa entrada no confiable: se actualiza ante boletines de seguridad. Ver el
  [modelo de amenazas](../../audit/threat-model.md).

### 2.2 Motor del webview

Tauri usa el motor del sistema: WebKitGTK 4.1 en Linux. Su comportamiento y sus vulnerabilidades dependen de la
distribución, y por eso los paquetes declaran la dependencia y el `.tar.gz` la exige al sistema.

## 3. Configuración de compilación

* Perfil de publicación optimizado; las herramientas (`revisar`, `firmar`, `medir_puente`,
  `comprobar_artefactos`) solo se construyen con `--features herramientas`, para que nunca entren en el paquete.
* El binario que se ejecuta en desarrollo y el que se empaqueta son el mismo (`default-run`, `mainBinaryName` y
  `tests/identidad_del_binario.rs`).
* Ninguna ruta absoluta de una máquina en la configuración.
* En Linux el AppImage requiere `NO_STRIP=1` ([linux.md](linux.md), sección 1.1).

## 4. Reproducibilidad y procedencia

* Versiones de herramientas fijadas (`rust-toolchain.toml`, `packageManager` de pnpm y `engines` de Node).
* Archivos de bloqueo versionados.
* Listado de componentes CycloneDX por publicación, con el commit del que salió.
* `SHA256SUMS` firmado con Ed25519. Ver [releases.md](releases.md).

## 5. Tamaño

El paquete tiene un presupuesto declarado en [performance-budgets.md](../../architecture/performance-budgets.md).
Lo que lo supere se justifica o se descarga bajo petición.
