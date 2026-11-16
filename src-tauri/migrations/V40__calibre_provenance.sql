-- V40: procedencia de Calibre.
--
-- Al traer una biblioteca de Calibre, sus metadatos se importan con la procedencia
-- "calibre". Esto asegura que una extracción posterior de metadatos internos del
-- archivo no pise lo que el usuario corrigió en Calibre, y que una consulta en
-- línea proponga en lugar de sobrescribir en silencio.
--
-- Contrato: docs/architecture/metadata-model.md, sección 3.

DROP TRIGGER IF EXISTS trg_works_borran_su_metadato;
DROP TRIGGER IF EXISTS trg_editions_borran_su_metadato;
DROP TRIGGER IF EXISTS trg_contributors_borran_su_metadato;

CREATE TABLE metadata_provenance_v40 (
    entity_type TEXT NOT NULL CHECK (entity_type IN ('work', 'edition', 'contributor')),
    entity_id   TEXT NOT NULL,
    field       TEXT NOT NULL,
    source      TEXT NOT NULL CHECK (source IN ('file', 'online', 'manual', 'calibre')),
    value       TEXT,
    recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (entity_type, entity_id, field, source),
    CHECK (length(trim(field)) > 0)
) STRICT, WITHOUT ROWID;

INSERT INTO metadata_provenance_v40 (entity_type, entity_id, field, source, value, recorded_at)
SELECT entity_type, entity_id, field, source, value, recorded_at FROM metadata_provenance;

DROP TABLE metadata_provenance;
ALTER TABLE metadata_provenance_v40 RENAME TO metadata_provenance;

CREATE INDEX idx_metadata_provenance_entity ON metadata_provenance(entity_type, entity_id);

CREATE TABLE metadata_suggestions_v40 (
    id             TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    entity_type    TEXT NOT NULL CHECK (entity_type IN ('work', 'edition', 'contributor')),
    entity_id      TEXT NOT NULL,
    field          TEXT NOT NULL,
    source         TEXT NOT NULL CHECK (source IN ('file', 'online', 'calibre')),
    proposed_value TEXT,
    status         TEXT NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'accepted', 'rejected')),
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    resolved_at    TEXT,

    CHECK (length(trim(field)) > 0),
    CHECK ((status = 'pending') = (resolved_at IS NULL))
) STRICT;

INSERT INTO metadata_suggestions_v40 (id, entity_type, entity_id, field, source, proposed_value, status, created_at, resolved_at)
SELECT id, entity_type, entity_id, field, source, proposed_value, status, created_at, resolved_at FROM metadata_suggestions;

DROP TABLE metadata_suggestions;
ALTER TABLE metadata_suggestions_v40 RENAME TO metadata_suggestions;

CREATE UNIQUE INDEX idx_metadata_suggestions_vivas
    ON metadata_suggestions(entity_type, entity_id, field, source)
    WHERE status = 'pending';

CREATE INDEX idx_metadata_suggestions_entity
    ON metadata_suggestions(entity_type, entity_id);

CREATE TRIGGER trg_works_borran_su_metadato AFTER DELETE ON works
BEGIN
    DELETE FROM metadata_provenance WHERE entity_type = 'work' AND entity_id = OLD.id;
    DELETE FROM metadata_suggestions WHERE entity_type = 'work' AND entity_id = OLD.id;
END;

CREATE TRIGGER trg_editions_borran_su_metadato AFTER DELETE ON editions
BEGIN
    DELETE FROM metadata_provenance WHERE entity_type = 'edition' AND entity_id = OLD.id;
    DELETE FROM metadata_suggestions WHERE entity_type = 'edition' AND entity_id = OLD.id;
END;

CREATE TRIGGER trg_contributors_borran_su_metadato AFTER DELETE ON contributors
BEGIN
    DELETE FROM metadata_provenance WHERE entity_type = 'contributor' AND entity_id = OLD.id;
    DELETE FROM metadata_suggestions WHERE entity_type = 'contributor' AND entity_id = OLD.id;
END;

