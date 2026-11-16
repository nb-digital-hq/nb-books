-- V3: procedencia del metadato y propuestas de cambio.
--
-- Resuelve el defecto que separa un gestor en el que se puede confiar de uno que
-- corrompe el trabajo del usuario en silencio: una actualización automática que
-- pisa el título que alguien corrigió a mano. Aquí la actualización automática
-- **no puede** pisarlo; propone.
--
-- Reglas que gobiernan este archivo:
--   docs/architecture/metadata-model.md, sección 3
--   docs/architecture/api/schemas.md
--
--.

-- ---------------------------------------------------------------------------
-- Procedencia: de dónde salió cada campo de cada entidad.
--
-- Una fila por origen y campo, no una por cambio: el historial completo de
-- ediciones es otra cosa y no hace falta para decidir la precedencia. Con esto
-- se responde a las dos preguntas que la interfaz necesita: de dónde salió el
-- valor que se está viendo, y cuál era el original del archivo para poder
-- revertirlo.
-- ---------------------------------------------------------------------------
CREATE TABLE metadata_provenance (
    -- No hay clave foránea porque la referencia es polimórfica. La integridad
    -- la mantienen los disparadores del final de este archivo, que borran la
    -- procedencia cuando desaparece la entidad.
    entity_type TEXT NOT NULL CHECK (entity_type IN ('work', 'edition', 'contributor')),
    entity_id   TEXT NOT NULL,
    field       TEXT NOT NULL,
    source      TEXT NOT NULL CHECK (source IN ('file', 'online', 'manual')),
    -- Valor que aportó ese origen. Nulo si lo que aportó fue vaciar el campo.
    value       TEXT,
    recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (entity_type, entity_id, field, source),
    CHECK (length(trim(field)) > 0)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_metadata_provenance_entity ON metadata_provenance(entity_type, entity_id);

-- ---------------------------------------------------------------------------
-- Propuestas de cambio.
--
-- Lo que un origen automático quería escribir y no pudo, porque el usuario ya
-- había decidido ese campo. No se descarta: se guarda para que la interfaz
-- pueda ofrecerlo. Descartarlo en silencio sería perder información igual que
-- sobrescribir.
-- ---------------------------------------------------------------------------
CREATE TABLE metadata_suggestions (
    id             TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    entity_type    TEXT NOT NULL CHECK (entity_type IN ('work', 'edition', 'contributor')),
    entity_id      TEXT NOT NULL,
    field          TEXT NOT NULL,
    -- `manual` no aparece: lo que decide el usuario se aplica, no se sugiere.
    source         TEXT NOT NULL CHECK (source IN ('file', 'online')),
    proposed_value TEXT,
    status         TEXT NOT NULL DEFAULT 'pending'
                   CHECK (status IN ('pending', 'accepted', 'rejected')),
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    resolved_at    TEXT,

    CHECK (length(trim(field)) > 0),
    -- Una propuesta resuelta tiene fecha de resolución, y una pendiente no.
    CHECK ((status = 'pending') = (resolved_at IS NULL))
) STRICT;

-- Una sola propuesta viva por campo y origen: consultar el mismo proveedor diez
-- veces produce diez propuestas idénticas y ninguna información nueva.
CREATE UNIQUE INDEX idx_metadata_suggestions_vivas
    ON metadata_suggestions(entity_type, entity_id, field, source)
    WHERE status = 'pending';

CREATE INDEX idx_metadata_suggestions_entity
    ON metadata_suggestions(entity_type, entity_id);

-- ---------------------------------------------------------------------------
-- Integridad de la referencia polimórfica.
--
-- Sin clave foránea, borrar una edición dejaría su procedencia y sus propuestas
-- colgando: filas que nadie vuelve a encontrar y que la exportación arrastraría
-- sin dueño.
-- ---------------------------------------------------------------------------
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
