-- V1: metadatos de la propia instalación.
--
-- Deliberadamente mínima. El esquema del dominio (obras, ediciones, archivos,
-- anotaciones) se define en la.; esta migración existe para que
-- el mecanismo de migraciones tenga algo real que aplicar y pueda probarse.
--
-- Reglas que gobiernan este directorio:
--   docs/architecture/migrations.md
--   docs/architecture/versioning.md

CREATE TABLE app_metadata (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

INSERT INTO app_metadata (key, value) VALUES
    ('created_by_version', '1.0.0'),
    ('install_id', lower(hex(randomblob(16))));
