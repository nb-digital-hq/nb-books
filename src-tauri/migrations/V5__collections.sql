-- V5: colecciones y etiquetas.
--
-- Dos formas de agrupar que se parecen y no son lo mismo:
--
--   * Una **colección manual** es una lista que el usuario mantiene a mano.
--   * Una **colección por regla** no guarda miembros: guarda un criterio, y sus
--     miembros son los que lo cumplen en cada momento. Por eso una colección por
--     regla se actualiza sola al importar un documento que la cumple, sin
--     trabajo en segundo plano y sin poder quedarse obsoleta.
--
-- Contratos: docs/architecture/api/schemas.md y
-- docs/architecture/storage.md.

CREATE TABLE collections (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name       TEXT NOT NULL,
    -- `manual` guarda miembros en collection_items; `rule` no guarda ninguno.
    kind       TEXT NOT NULL CHECK (kind IN ('manual', 'rule')),
    -- De quién es la colección.
    origin     TEXT NOT NULL DEFAULT 'personal'
               CHECK (origin IN ('personal')),
    -- Criterio serializado, solo en las colecciones por regla. Su forma la fija
    -- `src-tauri/src/collections.rs`; aquí solo se exige que sea un objeto, por
    -- el mismo motivo que en `annotations.target`.
    rule       TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(name)) > 0),
    -- Una colección por regla sin regla no tiene miembros posibles, y una manual
    -- con regla tendría dos fuentes de verdad sobre quién pertenece.
    CHECK ((kind = 'rule') = (rule IS NOT NULL)),
    CHECK (rule IS NULL OR (json_valid(rule) AND json_type(rule) = 'object'))
) STRICT;

CREATE UNIQUE INDEX idx_collections_nombre ON collections(origin, name);

CREATE TABLE collection_items (
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    edition_id    TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Orden explícito: una bibliografía tiene un orden que importa, y dejarlo al
    -- criterio de la consulta lo convierte en accidental.
    position      INTEGER NOT NULL DEFAULT 0,
    added_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (collection_id, edition_id),
    CHECK (position >= 0)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_collection_items_edition ON collection_items(edition_id);

CREATE TRIGGER trg_collections_updated_at AFTER UPDATE ON collections
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE collections SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
