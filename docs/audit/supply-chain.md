# Cadena de suministro

> **Estado:** Implementado. Auditoría de licencias y avisos de dependencias, motor de PDF con hash fijado
> (`scripts/fetch-pdfium.mjs`), procedencia y firma de las publicaciones (`scripts/procedencia.mjs`,
> `src-tauri/src/artefactos.rs`), actualizador verificado (`src-tauri/src/actualizacion.rs`) y puerta de revisión de
> extensiones (`src-tauri/src/extensions/review.rs`).

Una aplicación de escritorio con actualizador es un vector de distribución privilegiado: comprometer su cadena de
construcción permitiría entregar código arbitrario, firmado como legítimo, a toda la base instalada. Por eso tiene
documento propio.

## 1. Activos críticos

| Activo | Impacto si se compromete |
|---|---|
| Clave privada de publicación (Ed25519) | Publicaciones y actualizaciones aceptadas como oficiales |
| Claves de las extensiones propias | Actualizaciones aceptadas de esas extensiones |
| Cuenta con permisos de publicación en `nb-digital-hq/nb-books` | Alteración del código, de los flujos y de las publicaciones |

## 2. Controles sobre las claves

* **Ninguna clave privada entra en el repositorio.** El gancho de pre-commit rechaza binarios y claves; la clave
  pública es lo único que se versiona, en `docs/architecture/api/clave-de-publicacion.pem`.
* La clave de publicación es un archivo PKCS#8 con permisos `0600`, creado con
  `cargo run --features herramientas --bin firmar -- --crear-clave`, y se custodia fuera del repositorio con copia
  de seguridad.
* **La clave de publicación no es la de las extensiones**: perder o exponer una no compromete lo que firma la otra.
* Ningún flujo de integración continua firma, así que los flujos disparados por bifurcaciones no ven secretos de
  firma.
* La clave pública viaja **compilada** en la aplicación: una actualización que no lleve su firma no se instala.
  Sustituir la clave exige una versión firmada con la anterior que incorpore la nueva. Ver
  [security-policy.md](../legal/governance/security-policy.md), sección 6.

## 3. Dependencias

* **Licencias:** `pnpm check:notices` (en `pnpm verify`) y `cargo deny check licenses` contra la
  [política de dependencias](../architecture/dependency-policy.md).
* **Avisos de seguridad:** `pnpm check:avisos` (RustSec, con `cargo-deny`) y `pnpm audit`; Dependabot vigila
  los avisos y no abre propuestas de versión: cada actualización se hace a mano y pasa la verificación completa.
* **Archivos de bloqueo versionados**; ninguna dependencia flotante. `deny.toml` declara `yanked = "deny"`.
* Se miran las dos bases de avisos (RustSec y GitHub) porque no clasifican igual, y quedarse con la que da menos
  alertas sería elegir el resultado.

### 3.1 Avisos conocidos sin acción posible

| Aviso | Dónde | Por qué no aplica o no se puede cerrar |
|---|---|---|
| `glib` 0.18.5, implementación no sólida de `Iterator` en `VariantStrIter` (GitHub, media; RustSec no lo clasifica) | Pila de GTK de Tauri | `gtk` 0.18 y `webkit2gtk` fijan `glib` a `^0.18`; el proyecto no llama a `glib` |
| `cookie` 0.6.0 (npm), caracteres fuera de rango | `@sveltejs/kit`, dependencia de desarrollo | No viaja en el producto: la aplicación sirve `build/`, estático |
| Crates sin mantenimiento (`proc-macro-error`, `unic-*`) | Árbol de Tauri | No son dependencias directas |
| `paste` sin mantenimiento | `candle` (`gemm`, `pulp`) y `tokenizers` | Macro de compilación, sin código en ejecución; no hay versión sin ella |
| `zvariant`, `zvariant_derive` y `zbus_names` sin mantenimiento | `zbus`, vía el llavero y Speech Dispatcher | No son dependencias directas; no hay versión a la que saltar |
| `chrono` sin mantenimiento (RustSec) | `pdfium-render`, fechas de los metadatos del PDF | No es una vulnerabilidad; `pdfium-render` no ofrece versión sin él |

No se fuerzan versiones por debajo de un marco de trabajo (`[patch]` en Cargo, `overrides` en pnpm): cambiaría un
riesgo conocido por uno desconocido.

## 4. Dependencias nativas

PDFium es código C++ que procesa entrada no confiable:

* Se obtiene de una fuente publicada con **hash fijado** (`scripts/fetch-pdfium.mjs`) y nunca vive en el repositorio.
* Se usa la variante sin JavaScript.
* Se sigue su canal de boletines y se publica una versión correctiva cuando hace falta.

Los pesos de modelos y voces se verifican con huellas compiladas en el binario
([ai/local-models.md](../architecture/ai/local-models.md)).

## 5. Procedencia de los artefactos

Cada publicación incluye:

* Los instaladores, `NOTAS.md` y `THIRD-PARTY-NOTICES.md`.
* Un listado de componentes en **CycloneDX 1.5**, generado de la misma lista que los avisos de terceros, más el
  motor de PDF con su huella, y el commit del que salió.
* `SHA256SUMS` con la huella de todo lo anterior y su firma **Ed25519** `SHA256SUMS.sig`.
* Instrucciones de verificación para un administrador de TI, con `sha256sum` y `openssl`, en
  [releases.md](../developer/deployment/releases.md), sección 6; una prueba ejecuta esas mismas órdenes.

`pnpm procedencia` exige el árbol limpio, porque el listado nombra el commit del que sale la publicación, y
comprueba la firma contra la clave del repositorio y no contra la que acaba de usar.

## 6. Registro de extensiones

Las extensiones son la superficie de suministro que el proyecto no controla. Los controles están en la
[política del registro](../legal/governance/extension-policy.md): análisis automatizado de **cada versión**, firma
del autor fijada en la primera instalación y retirada con aviso a quien ya la tenía. La puerta es un binario local:
`cargo run --features herramientas --bin revisar -- <paquete>`, que devuelve 0 si el paquete se publica y 1 si no,
y que cualquier automatismo puede invocar.
