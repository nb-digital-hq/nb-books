# Proceso de publicación

> **Estado:** Implementado. Qué se publica, cómo se firma y cómo lo consume el actualizador está en
> [deployment/releases.md](deployment/releases.md); la compilación, en [ci-cd.md](ci-cd.md). Este documento es la
> lista de quien publica.

## 1. Versionado

NB Books sigue Versionado Semántico para la aplicación:

* **MAJOR:** ruptura de compatibilidad para el usuario o para las extensiones.
* **MINOR:** funcionalidad nueva compatible.
* **PATCH:** correcciones compatibles.

El esquema de base de datos, el contrato IPC y la API de extensiones tienen sus propias reglas de compatibilidad,
definidas en [versioning.md](../architecture/versioning.md).

Hay un solo canal: las publicaciones de `nb-digital-hq/nb-books` en GitHub. El actualizador integrado consulta la
última.

## 2. Lista previa

1. `just verify` verde en el árbol que se va a publicar.
2. `just package-linux` sin errores (la AppImage con `NO_STRIP=1`).
3. Prueba de actualización real: instalar la versión anterior, actualizar y comprobar que los datos sobreviven.
4. `CHANGELOG.md` con la sección de la versión, que es de donde salen las notas.

## 3. Corte y publicación

1. La versión se cambia a la vez en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`.
2. Se crea la etiqueta `vX.Y.Z`.
3. `pnpm procedencia --clave <archivo PKCS#8>` con el árbol limpio: genera `NOTAS.md`, el listado de componentes,
   `SHA256SUMS` y su firma, y comprueba la firma contra la clave pública del repositorio.
4. Se crea la publicación con todos los archivos de
   `src-tauri/target/publicacion/<versión>/`. El actualizador solo ofrece una versión si encuentra
   `SHA256SUMS.sig` válido.

## 4. Reversión

Una migración de esquema no se revierte (ver [migrations.md](../architecture/migrations.md)). Ante un fallo grave:

1. Se retira la publicación defectuosa.
2. Se publica una versión de corrección hacia adelante.
3. Las notas de esa versión explican el impacto y lo que tiene que hacer el usuario.

Si el fallo implica pérdida de datos o exposición de información, se aplica además el procedimiento de
[respuesta a incidentes](../legal/governance/security-policy.md).

## 5. Soporte

Recibe correcciones de seguridad la última versión publicada.
