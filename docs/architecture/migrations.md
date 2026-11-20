# Migraciones de Base de Datos

> **Estado:** Implementado.
> Módulo: `src-tauri/src/db/migrations.rs`. Migraciones: `src-tauri/migrations/`.
> En una aplicación de escritorio, la base de datos (SQLite) vive en la máquina del usuario. No tienes acceso a ella. Las migraciones deben ser a prueba de balas, atómicas y ejecutarse antes de que la UI se levante.

## 1. El Riesgo de las Migraciones en Desktop

A diferencia de la web, donde controlas cuándo se actualiza el esquema PostgreSQL del servidor, en desktop un usuario puede pasar de la versión `0.1.0` directamente a la `1.5.0` omitiendo todas las intermedias.

Si la migración falla, el usuario pierde toda su biblioteca.

## 2. Herramienta y estrategia

NB Books usa un **módulo de migración propio y embebido**, no una biblioteca externa.

La decisión sigue la [política de dependencias](dependency-policy.md): el módulo son unas cien líneas y, sobre
todo, necesita hacer tres cosas que las bibliotecas habituales no hacen por sí solas y que aquí no son
opcionales: crear un respaldo antes de migrar, negarse a operar sobre una base más nueva que la aplicación, y
registrar el fallo con la redacción de datos personales del proyecto.

### 2.1 Archivos de migración embebidos
Las migraciones son archivos `.sql` en `src-tauri/migrations/`, nombrados `V<n>__<nombre>.sql`:

* `V1__app_metadata.sql` — metadatos de la instalación
* `V2__bibliographic_core.sql` — obra, edición, archivo, contribuidores, materias y anotaciones
* `V3__metadata_provenance.sql` — procedencia por campo y propuestas de cambio
* `V4__settings.sql` — preferencias del usuario, con los secretos vetados por disparador
* `V5__collections.sql` — colecciones manuales y por regla, con su origen

La extracción de metadatos escribe en las tablas que ya existían, y las portadas viven en la caché, no en la
base.

Se incrustan en el binario con `include_str!` en tiempo de compilación. La aplicación nunca depende de archivos
externos, y la lista ordenada de migraciones vive en una constante del módulo.

### 2.1.1 Convenciones vigentes desde la V2

| Aspecto | Convención | Por qué |
|---|---|---|
| Identificadores | `TEXT` de 32 caracteres hexadecimales, generados por la base con `randomblob(16)` | Sobreviven a exportar, reimportar y fusionar bibliotecas entre dispositivos; un entero local obliga a reasignar claves en cada fusión |
| Marcas de tiempo | `TEXT` en ISO-8601 UTC (`strftime(%Y-%m-%dT%H:%M:%fZ)`) | Ordenar como cadena es lo mismo que ordenar como instante |
| Tipos | `STRICT` en todas las tablas | Sin él SQLite acepta una cadena en una columna `INTEGER`, y el error aparece meses después |
| `updated_at` | Disparador `AFTER UPDATE` con guarda `WHEN NEW.updated_at = OLD.updated_at` | Una marca que el código olvida actualizar es un dato en el que no se puede confiar, y el olvido no lo detecta ninguna prueba |
| Listas de valores | `CHECK (col IN (...))` con lista cerrada, reflejada en un `enum` de Rust | Los dos sitios tienen que decir lo mismo, y añadir un valor obliga a tocar ambos |

La V1 usó `datetime(now)` para `app_metadata.updated_at`. Una migración publicada no se edita, así que esa tabla
conserva su formato y la convención rige a partir de la V2.

La versión aplicada se guarda en el `PRAGMA user_version` de SQLite, sin tabla auxiliar.

**Nunca se edita una migración ya publicada.** Corregir un error en una migración aplicada significa añadir una
migración nueva.

### 2.2 Ejecución al arranque
`bootstrap` abre la base de datos y aplica las migraciones pendientes **antes** de que Tauri levante la
ventana. Cada migración corre dentro de su propia transacción: se aplica entera o no se aplica.

```rust
let db = Database::open(&paths.database)?; // migra o falla; nunca deja estado intermedio
```

Si la migración falla, el arranque se detiene y el respaldo previo queda en disco junto a la base, con el sufijo
`.v<n>.backup`. **Desde la implementación el respaldo lleva todo salvo el índice del texto**, que va vacío y se rehace al
restaurarlo: qué tablas, por qué esas y lo que ocupa está en [storage.md](storage.md) sección 1.3. El diálogo nativo de modo de recuperación que sustituirá al fallo actual por una explicación
comprensible llega.

### 2.3 Negativa a degradar
Si `user_version` es mayor que la versión que la compilación conoce, la aplicación devuelve `SCHEMA_MISMATCH` y
**no abre la base**. Es el caso de quien instala una versión antigua después de haber usado una nueva: leerla la
corrompería.

## 3. Reglas Estrictas de Modificación

1. **Up-Only:** No existen scripts "Down" (rollback). En aplicaciones desktop, hacer un downgrade de esquema casi siempre resulta en pérdida de datos. Los errores se arreglan iterando hacia adelante (Roll-forward).
2. **No borrar columnas:** En lugar de `DROP COLUMN`, se marca la columna como obsoleta (con sufijo `_deprecated`) en el esquema de Rust.
3. **Transacciones Atómicas:** Toda migración (ej. V4) corre dentro de un `BEGIN TRANSACTION; ... COMMIT;`. Si falla a la mitad, se revierte sin corromper la BD.
