-- V30: un catálogo se puede guardar con una dirección en claro.
--
-- # Qué cambia, y quién lo decidió
--
-- V16 solo admitía `https://`. Hay catálogos que se
-- sirven únicamente en claro, y se admiten **con su propio
-- consentimiento**: salir hacia uno pide el propósito `catalog-cleartext`, que
-- dice exactamente qué deja a la vista -la dirección, lo que se busca y lo que se
-- descarga-, y tener permiso para el mismo anfitrión por `https` no lo concede.
-- Ver `net::Purpose::CatalogCleartext` y `docs/audit/privacy.md`.
--
-- El resto de salidas no cambia: la voz, las consultas y las extensiones siguen
-- sin poder hablar en claro hacia internet. La excepción es de catálogos, que no
-- mandan nada del usuario más allá de qué catálogo mira.
--
-- # Cómo
--
-- SQLite no permite alterar un `CHECK`, así que la tabla se reconstruye. No la
-- referencia ninguna otra, y las claves ajenas van apagadas alrededor de las
-- migraciones (`db::migrations::apply`).

CREATE TABLE catalogs_nueva (
    id       TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    title    TEXT NOT NULL,
    url      TEXT NOT NULL UNIQUE,
    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(title)) > 0),
    CHECK (url LIKE 'https://%' OR url LIKE 'http://%')
) STRICT;

INSERT INTO catalogs_nueva (id, title, url, added_at)
SELECT id, title, url, added_at FROM catalogs;

DROP TABLE catalogs;
ALTER TABLE catalogs_nueva RENAME TO catalogs;

CREATE INDEX idx_catalogs_added ON catalogs(added_at DESC);
