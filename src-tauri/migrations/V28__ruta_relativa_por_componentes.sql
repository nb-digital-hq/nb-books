-- V28: la comprobación de travesía de ruta mira componentes, no subcadenas.
--
-- # Qué estaba mal
--
-- V11 rechazaba toda ruta relativa que **contuviera** `..` en cualquier
-- posición. La intención es correcta y sigue vigente: una ruta relativa que se
-- sale de su raíz no es relativa, es una travesía, y comprobarlo en el esquema
-- es mejor que comprobarlo en código porque el código se puede olvidar.
--
-- Pero `..` dentro de un nombre de archivo no es una travesía: es una elipsis.
-- Los títulos de libros las llevan, y bastante.
--
-- Medido el 2026-09-10 sobre el corpus real, 152.080 archivos:
--
--   * 667 los rechazaba esta comprobación, el 0,44 %.
--   * De esos 667, **cero** eran travesías de ruta.
--
-- Cien por cien de falsos positivos: seiscientos sesenta y siete libros que no
-- pueden entrar en la biblioteca de su dueño, y ni una travesía detenida. Un
-- mecanismo que solo rechaza lo legítimo no protege de nada.
--
-- # Qué comprueba ahora
--
-- Que ningún **componente** de la ruta sea `..`. La expresión rodea la ruta de
-- separadores y normaliza la barra invertida, así que un solo `instr` cubre las
-- cuatro posiciones -sola, al principio, en medio y al final- y las dos formas
-- de escribir un separador. `Cuentos.. y otros.epub` entra;
-- `../../etc/passwd` y `..\\..\\Windows` no.
--
-- La barra invertida se normaliza porque el mismo esquema corre en Windows, y
-- allí `..\\` sale de la carpeta igual que `../` aquí.
--
-- # Y el disparador que V11 se dejó
--
-- V11 reconstruyó `files` con `DROP TABLE` y `RENAME`, y eso se llevó por
-- delante `trg_files_updated_at`, que V2 había creado y que V11 no repuso. Desde
-- entonces `files.updated_at` se quedaba con la fecha de alta aunque la fila
-- cambiara. Se repone aquí porque esta migración vuelve a reconstruir la tabla y
-- dejarlo sin reponer sería repetir el mismo descuido a sabiendas.

CREATE TABLE files_nuevo (
    id            TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id    TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
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
    -- Sigue sin poder empezar por separador, en sus dos formas.
    CHECK (substr(relative_path, 1, 1) NOT IN ('/', char(92))),
    -- Y ningún componente puede ser `..`. Ver la cabecera.
    CHECK (instr('/' || replace(relative_path, char(92), '/') || '/', '/../') = 0),
    UNIQUE (root_id, relative_path)
) STRICT;

INSERT INTO files_nuevo (id, edition_id, root_id, relative_path, content_hash, byte_size,
                         media_type, storage_mode, availability, created_at, updated_at)
SELECT id, edition_id, root_id, relative_path, content_hash, byte_size,
       media_type, storage_mode, availability, created_at, updated_at
FROM files;

DROP TABLE files;
ALTER TABLE files_nuevo RENAME TO files;

CREATE INDEX idx_files_edition ON files(edition_id);
CREATE INDEX idx_files_content_hash ON files(content_hash);
CREATE INDEX idx_files_root ON files(root_id);

CREATE TRIGGER trg_files_updated_at AFTER UPDATE ON files
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE files SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
