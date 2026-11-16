-- V11: la ubicación de un archivo deja de ser una ruta absoluta.
--
-- # El defecto que corrige
--
-- Hasta esta migración, `files.path` guardaba la ruta absoluta del archivo, en
-- los dos modos de almacenamiento, y la exportación la llevaba tal
-- cual. La consecuencia es una pérdida:
--
--   * El usuario exporta su biblioteca.
--   * La restaura en otro equipo, o en el mismo tras reinstalar el sistema con
--     otro nombre de usuario, o con el disco externo montado en otro punto.
--   * **Todos** los archivos quedan como no disponibles a la vez.
--
-- El hash de contenido conserva la identidad del archivo; nada
-- conservaba su ubicación. La regla es "ninguna pérdida", y esto
-- era un escenario de pérdida.
--
-- # El modelo
--
-- Una ubicación se compone de dos partes: una **raíz de biblioteca**, que es un
-- lugar declarado con identificador propio, y una **ruta relativa** dentro de
-- ella. La ruta absoluta de la raíz vive en un solo sitio, y es la única que hay
-- que corregir al cambiar de equipo.
--
-- La biblioteca administrada es **una raíz más**, no un caso especial, para que
-- el modelo tenga una sola forma. Su ruta se guarda, pero **se reescribe en cada
-- arranque** con el directorio de biblioteca de esta instalación: es una caché,
-- nunca una fuente de verdad. Así resolver una ubicación no necesita contexto
-- externo, y mover o reinstalar la aplicación se corrige solo. La exportación la
-- omite, porque es un dato de la instalación y no del catálogo.
--
-- Una raíz **referenciada** con ruta `NULL` significa otra cosa: **no
-- localizada**. Es el estado en que quedan las raíces tras restaurar en otro
-- equipo, y el usuario la vuelve a apuntar una vez.
--
-- # Cómo se reparten los archivos que ya existían
--
-- Los administrados van a la raíz administrada, con la ruta relativa que ya
-- tenían dentro de la carpeta de biblioteca.
--
-- Los referenciados reciben **una raíz por directorio padre distinto**. No es un
-- apaño de migración: es exactamente la regla que rige al importar un archivo
-- referenciado nuevo, aplicada hacia atrás. Si el resultado son varias raíces
-- que el usuario prefiere ver como una, existe la operación de fusión, que es
-- una operación normal del producto y no una limpieza posterior.
--
-- El nombre del directorio padre se obtiene con la fórmula habitual de SQLite:
-- `rtrim(p, replace(p, '/', ''))` deja fuera todos los caracteres finales que no
-- son separador, es decir, el nombre del archivo. Antes se normaliza la barra
-- invertida a barra, para que una biblioteca creada en Windows migre igual; las
-- rutas con barra normal las acepta Windows sin problema.
--
-- Contrato: docs/architecture/storage.md, sección 2.1.

CREATE TABLE library_roots (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- Nombre legible. Es lo que el usuario ve al reapuntar una raíz, así que
    -- tiene que significar algo para él y no para nosotros.
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('managed', 'referenced')),
    -- Administrada: la carpeta de biblioteca de esta instalación, reescrita al
    -- arrancar. Referenciada: la ruta absoluta, o NULL si no está localizada.
    path       TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(name)) > 0),
    CHECK (path IS NULL OR length(trim(path)) > 0)
) STRICT;

-- Solo puede haber una raíz administrada: es la carpeta de biblioteca de esta
-- instalación, y no hay dos.
CREATE UNIQUE INDEX idx_library_roots_managed
    ON library_roots(kind) WHERE kind = 'managed';

-- Identificador fijo para la raíz administrada. Fijo a propósito: la exportación
-- y la importación tienen que poder reconocerla entre equipos sin depender de un
-- aleatorio distinto en cada instalación.
INSERT INTO library_roots (id, name, kind, path)
VALUES ('00000000000000000000000000000001', 'Biblioteca administrada', 'managed', NULL);

-- ---------------------------------------------------------------------------
-- Reconstrucción de `files`.
--
-- Se reconstruye en lugar de añadir columnas porque `path` era NOT NULL UNIQUE y
-- desaparece, y porque la unicidad pasa a ser por raíz más ruta relativa.
-- ---------------------------------------------------------------------------

CREATE TABLE files_nuevo (
    id            TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id    TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- La ubicación, en sus dos mitades. Ninguna ruta absoluta de archivo vuelve
    -- a guardarse aquí.
    root_id       TEXT NOT NULL REFERENCES library_roots(id) ON DELETE RESTRICT,
    relative_path TEXT NOT NULL,
    content_hash  TEXT NOT NULL,
    byte_size     INTEGER NOT NULL,
    media_type    TEXT NOT NULL,
    storage_mode  TEXT NOT NULL CHECK (storage_mode IN ('managed', 'referenced')),
    availability  TEXT NOT NULL DEFAULT 'available'
                  CHECK (availability IN ('available', 'missing', 'unreadable')),
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (byte_size >= 0),
    CHECK (length(trim(relative_path)) > 0),
    CHECK (length(trim(content_hash)) > 0),
    CHECK (length(trim(media_type)) > 0),
    -- Una ruta relativa que empieza por separador o que contiene `..` no es
    -- relativa: es una forma de salirse de la raíz. Se rechaza en el esquema
    -- porque la comprobación en código se puede olvidar y esta no.
    CHECK (substr(relative_path, 1, 1) <> '/'),
    CHECK (instr(relative_path, '..') = 0),
    -- Dos archivos distintos no pueden ocupar el mismo sitio.
    UNIQUE (root_id, relative_path)
) STRICT;

-- Una raíz referenciada por cada directorio padre distinto de los archivos
-- referenciados que ya existían.
--
-- `normalizadas` calcula de una vez las tres piezas que hacen falta: la ruta con
-- separadores normalizados, su directorio y el nombre del archivo. Hacerlo en
-- una sola vista evita repetir tres veces la misma expresión y equivocarse en
-- una de ellas.
CREATE TEMP VIEW normalizadas AS
SELECT
    f.id            AS file_id,
    f.storage_mode  AS storage_mode,
    replace(f.path, char(92), '/') AS completa,
    rtrim(rtrim(replace(f.path, char(92), '/'),
                replace(replace(f.path, char(92), '/'), '/', '')), '/') AS directorio
FROM files f;

INSERT INTO library_roots (name, kind, path)
SELECT DISTINCT
    -- El nombre por defecto es el del propio directorio, que es lo que el
    -- usuario reconoce al verlo en la lista de raíces.
    replace(directorio, rtrim(directorio, replace(directorio, '/', '')), ''),
    'referenced',
    directorio
FROM normalizadas
WHERE storage_mode = 'referenced' AND directorio <> '';

INSERT INTO files_nuevo (id, edition_id, root_id, relative_path, content_hash, byte_size,
                         media_type, storage_mode, availability, created_at, updated_at)
SELECT
    f.id,
    f.edition_id,
    CASE
        WHEN f.storage_mode = 'managed' OR n.directorio = ''
            THEN '00000000000000000000000000000001'
        ELSE (SELECT r.id FROM library_roots r
              WHERE r.kind = 'referenced' AND r.path = n.directorio)
    END,
    -- Lo que queda tras quitar el directorio y su separador. Es la misma
    -- expresión en los dos modos: en el administrado el directorio es la carpeta
    -- de biblioteca, y lo que queda es el nombre del archivo.
    CASE
        WHEN n.directorio = '' THEN n.completa
        ELSE substr(n.completa, length(n.directorio) + 2)
    END,
    f.content_hash,
    f.byte_size,
    f.media_type,
    f.storage_mode,
    f.availability,
    f.created_at,
    f.updated_at
FROM files f
JOIN normalizadas n ON n.file_id = f.id;

DROP VIEW normalizadas;

DROP TABLE files;
ALTER TABLE files_nuevo RENAME TO files;

CREATE INDEX idx_files_edition ON files(edition_id);
CREATE INDEX idx_files_content_hash ON files(content_hash);
CREATE INDEX idx_files_root ON files(root_id);
