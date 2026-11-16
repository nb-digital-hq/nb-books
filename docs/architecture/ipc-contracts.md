# Contratos IPC (Frontera Rust / Webview)

> **Estado:** Implementado, con un guardián automático por regla (sección "Mecanismos"). Módulos: `src-tauri/src/ipc.rs`, `src-tauri/src/security.rs`, `src/lib/core/ipc/`.
> El puente IPC es simultáneamente el cuello de botella de rendimiento y la frontera de seguridad del sistema.
> Ver [api/services.md](api/services.md) para el catálogo de comandos vigente.

## 1. Modelo de confianza

El webview es una **zona no confiable**. Ejecuta HTML y JavaScript, renderiza contenido derivado de documentos
importados y puede alojar extensiones. El núcleo Rust es la zona confiable y el único con acceso al sistema.

Consecuencia operativa: **todo dato que cruza hacia Rust se valida en Rust**. La validación en TypeScript es
experiencia de usuario, nunca seguridad.

## 2. Reglas de diseño de comandos

1. **Identificadores, no rutas.** Un comando opera sobre `doc_id`. Las rutas del sistema de archivos solo entran
   al sistema a través del diálogo nativo de archivos y quedan asociadas a un identificador.
2. **Canonicalización obligatoria.** Cuando un comando deba aceptar una ruta, se resuelve canónicamente y se
   verifica que quede dentro de un ámbito permitido, rechazando enlaces simbólicos que escapen.
3. **Errores tipados.** Rust devuelve un enum de dominio serializado con un código estable (`FILE_NOT_FOUND`,
   `FORMAT_UNSUPPORTED`), no una cadena libre. El frontend traduce el código; nunca muestra el mensaje interno.
   Los códigos son **diecinueve**, y una
   prueba los fija con una tabla independiente del código: añadir una variante no compila hasta que alguien decide
   conscientemente su código. `NETWORK_DISABLED`, `CONSENT_REQUIRED` y `NETWORK_FAILURE` existen porque piden acciones
   distintas de la interfaz: apagar el modo sin red, conceder un permiso, o reintentar. Reutilizar
   `COMPONENT_UNAVAILABLE` para los tres habría dejado al frontend sin poder distinguirlas.

   El decimonoveno, **`STORAGE_BUDGET_REACHED`**, dice que lo derivado llegó a su tope de disco. No es
   `STORAGE_FAILURE` -el disco responde- ni `CANCELLED` -nadie canceló nada-: es una decisión del producto, y lo
   que hay que ofrecer es liberar sitio o subir el tope en Ajustes. Es además **el mismo nombre con que el
   indexador publica su pausa**, atado por una prueba: la misma condición la alcanzan dos
   subsistemas, y con dos nombres la interfaz tendría que traducir dos veces lo mismo.

   Los de extensiones salen del mismo criterio: `EXTENSION_MANIFEST_INVALID`, `EXTENSION_API_UNSUPPORTED`,
   `EXTENSION_LIMIT_EXCEEDED`, `EXTENSION_ROLLBACK_UNAVAILABLE` y `EXTENSION_SIGNATURE_MISMATCH`. No hay uno para
   "permiso denegado": `CONSENT_REQUIRED` ya lo dice.
4. **Sin secretos de vuelta.** Ningún comando devuelve tokens, claves ni rutas absolutas que revelen el nombre de
   usuario del sistema.
5. **Un comando que no toca la ventana es asíncrono.** En Tauri un comando sin `async` corre en el hilo
   principal, que es el que atiende la ventana, así que esto no es una optimización: es la diferencia entre una
   interfaz viva y una congelada. Todos los comandos son asíncronos, y `scripts/check-comandos.mjs` rechaza el que
   no lo sea.

   **La excepción es exacta y no se amplía.** Lo que toca **ventana, diálogo nativo, menú o bandeja del
   sistema** se queda en el hilo principal: en Linux la biblioteca gráfica no es segura entre hilos, y llamarla
   desde otro hilo no da lentitud, da un fallo. Acogerse a ella exige las dos cosas a la vez -declararlo con la
   línea `Se queda en el hilo principal:` y su motivo, y usar de verdad una de esas cuatro cosas-, porque una
   excepción que se concede sola deja de ser una excepción.
6. **Payloads acotados.** No se envían colecciones sin límite. Toda lista se pagina con `offset` y `limit`, y el
   contrato declara el máximo, 500 filas por respuesta (`MAXIMO_FILAS` en `ipc.rs`). Lo que está
   acotado por construcción y no por parámetro **declara su cota** con la línea `Cota: <cuál>`, y
   `scripts/check-listas.mjs` rechaza al que no haga ni lo uno ni lo otro.
7. **Binarios fuera del canal JSON.** Las imágenes renderizadas y los archivos se sirven por protocolo de assets
   o por ruta temporal controlada, no serializados en el mensaje IPC.
8. **Idempotencia.** Los comandos de mutación deben poder repetirse sin duplicar estado.

## 3. Comandos peligrosos y su tratamiento

| Comando | Riesgo | Mitigación exigida |
|---|---|---|
| Importar archivo | Ruta arbitraria desde el webview | Origen exclusivo en diálogo nativo; canonicalización |
| Renderizar página | Ruta arbitraria; consumo de memoria | Resolución por `doc_id`; límites de tamaño y de páginas en caché |
| Eliminar documento | Borrado no deseado | Confirmación en la interfaz; no borra el archivo original salvo indicación explícita |
| Limpiar caché de render | Borrado fuera de ámbito | Ruta fija bajo el directorio de caché de la aplicación, nunca parametrizable |

## 4. Superficie mínima

Cada comando expuesto aumenta la superficie de ataque. Regla de proyecto: un comando nuevo requiere justificación
en el PR y revisión de seguridad. Los comandos que solo usa una ventana concreta se exponen únicamente a esa
ventana mediante capabilities de Tauri (ver [security/authorization.md](security/authorization.md)).

## 5. Tipos generados, no escritos a mano

Los tipos que cruzan el puente se generan desde Rust hacia TypeScript. El CI falla si los bindings generados
difieren de los versionados. Ver [testing/contract.md](../developer/testing/contract.md).

## 6. Evolución del contrato

Las reglas de compatibilidad (añadir campos opcionales, deprecar antes de eliminar, versionar eventos) están en
[versioning.md](versioning.md). Un cambio incompatible en el contrato IPC solo es aceptable dentro de la misma
versión de la aplicación, porque frontend y backend se distribuyen juntos; **no** lo es para las extensiones, que
se distribuyen por separado.

## 7. Cómo se hacen cumplir estas reglas

Ninguna de las reglas anteriores depende de que alguien las recuerde en una revisión:

| Regla | Mecanismo que la hace cumplir |
|---|---|
| Nadie invoca el puente fuera de la capa de acceso | Lint del proyecto (`noRestrictedImports`); un `import` de `@tauri-apps/api` fuera de `src/lib/core/ipc/` falla el build. **La capa es una carpeta.5**: `index.ts` tiene lo que la aplicación necesita al arrancar y los módulos de al lado, lo que solo usa una pantalla o un panel, para no ocupar el paquete de arranque. `check-puente.mjs` los lee todos |
| Los tipos no se escriben a mano | Generación con `ts-rs`; CI falla si los bindings quedan desactualizados |
| Los errores no viajan como cadenas | El tipo de dominio no lo permite; una prueba verifica que el payload no filtra detalles internos |
| Las rutas se canonicalizan y se acotan | `security::resolve_within`, con pruebas de recorrido de directorios y de enlaces simbólicos |
| Las entradas de archivos comprimidos se validan | `security::validate_archive_entry`, probada contra el corpus hostil |
| Ninguna respuesta trae más filas de las que se le piden | `scripts/check-listas.mjs`, en `pnpm verify` y en el gancho de pre-commit. Exige un parámetro `limit` o una cota declarada. **Lo que no hace, y se dice**: verificar que una cota declarada sea cierta, porque "las extensiones instaladas" es una afirmación sobre el mundo y no sobre el texto. Lo que garantiza es que esté escrita, que es lo que la hace revisable |
| Ningún comando corre en el hilo que atiende la ventana | `scripts/check-comandos.mjs`, en `pnpm verify` y en el gancho de pre-commit. Y `src-tauri/tests/hilo_de_la_ventana.rs`, que comprueba contra la versión instalada de Tauri que `(async)` sigue significando lo que la regla da por supuesto: un guardián que vigila una palabra vacía pasaría en verde con la ventana congelada |

Aceptar rutas del sistema de archivos directamente desde el frontend o devolver errores como cadenas sin tipar son
los dos defectos clásicos de esta frontera; los mecanismos de la tabla anterior existen para que no puedan
introducirse.
