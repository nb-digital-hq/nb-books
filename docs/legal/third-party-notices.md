# Avisos de Terceros

> **Estado:** Implementado. `LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md`
> existen en la raíz, se empaquetan en el instalador (`bundle.resources` de `src-tauri/tauri.conf.json`) y se
> enseñan en `Acerca de` dentro de la aplicación; `pnpm verify` falla si `THIRD-PARTY-NOTICES.md` queda
> desactualizado, si aparece una licencia fuera de la política permitida o si alguno de los tres archivos deja de
> viajar en el paquete.

## 1. Obligación

NB Books distribuye software de terceros dentro de sus instaladores. La práctica totalidad de las licencias
permisivas exige conservar el aviso de copyright y el texto de la licencia en las redistribuciones. Incumplirlo
es una infracción de licencia, con independencia de que el proyecto sea gratuito.

## 2. Qué debe incluirse

* Todas las dependencias de Rust presentes en el binario final.
* Todas las dependencias de JavaScript incluidas en el bundle.
* Las bibliotecas nativas distribuidas, en particular el motor de renderizado de PDF y sus componentes de
  terceros.
* Fuentes tipográficas, iconos e imágenes incluidos en el producto.
* Los modelos de IA, cuando se distribuyan o se descarguen desde el catálogo del proyecto, con su licencia
  específica.
* **El código de terceros copiado dentro del repositorio** —vendorizado—, que no aparece en ningún manifiesto de
  dependencias y por tanto no lo ve `cargo metadata`. Es el caso que más fácil se queda fuera y el que peor
  queda: se compila, se distribuye y no lo nombra nadie.

## 3. Generación

El archivo de avisos debe generarse automáticamente durante la compilación a partir de los metadatos de
dependencias, no mantenerse a mano. Un archivo mantenido manualmente queda obsoleto en la primera actualización
de dependencias.

Lo vendorizado no tiene metadatos que leer, así que se declara en el propio generador **y el generador lo
comprueba**: toda carpeta bajo `src-tauri/src/` con un `PROCEDENCIA.md` tiene que estar declarada, y la que no lo
esté hace fallar `pnpm verify`. Así la lista a mano no puede quedarse corta sin que alguien se entere, que es lo
que le pasa a las listas a mano.

El resultado se incluye en el instalador y es accesible desde la aplicación en la pantalla "Acerca de".

## 4. Verificación en CI

El pipeline verifica que todas las dependencias tengan una licencia identificada y permitida por la
[política de dependencias](../architecture/dependency-policy.md). Una dependencia sin licencia
declarada bloquea el build: no se distribuye lo que no se puede atribuir.

## 5. Obligación derivada de Apache 2.0

Desde la adopción de Apache License 2.0 ([ADR-003](../adr/003-licensing-model.md)), la sección 4 de
esa licencia obliga a **conservar y redistribuir el archivo `NOTICE`** en cualquier redistribución del trabajo,
incluidas las obras derivadas. Esto es una obligación que el proyecto impone a quien lo redistribuya y que, en
espejo, debe cumplir respecto de sus propias dependencias Apache 2.0.

## 6. Estado actual

* `LICENSE` y `NOTICE` existen en la raíz del repositorio.
* El listado de terceros se genera automáticamente: `scripts/generate-notices.mjs` produce
  `THIRD-PARTY-NOTICES.md` a partir de `cargo metadata` y `pnpm licenses`. Cubre 458 componentes de Rust y 1 de
  JavaScript.
* `LICENSE`, `NOTICE` y `THIRD-PARTY-NOTICES.md` viajan dentro del paquete de instalación (`bundle.resources` de
  `src-tauri/tauri.conf.json`, comprobado por `pnpm check:notices` y `tests/avisos-de-terceros.test.ts`) y se
  enseñan en `Ajustes → Acerca de → Licencia y Avisos de Terceros` a través del comando `legal_notices`.
* La verificación de licencias se ejecuta en dos sitios: el propio generador falla si una dependencia declara
  una licencia fuera de la lista permitida, y se ejecuta en `pnpm verify`; `cargo deny check licenses` repite la comprobación
  en integración continua.
* La biblioteca nativa de renderizado de PDF (`PDFium`, compilación `chromium/7881` de `pdfium-binaries`, con
  hash fijado en `scripts/fetch-pdfium.mjs`) viaja con sus textos de licencia: `LICENSE-pdfium` y la carpeta
  `licenses/` (PDFium, FreeType, ICU, libjpeg-turbo, libpng, libtiff, OpenJPEG, LittleCMS, zlib, Abseil y
  otros), que el mismo script extrae del archivo publicado junto a la biblioteca.
* **Hay un componente incorporado al código**: el motor de voz de `any-tts` 0.1.3, en
  `src-tauri/src/voice/kokoro/`, con su `PROCEDENCIA.md` al lado. Su licencia declarada es `MIT OR Apache-2.0`,
  y **su repositorio no publica archivo de licencia** así que no hay aviso de
  copyright que reproducir: la atribución nombra el proyecto, su versión y su origen, y lo dice tal cual.

