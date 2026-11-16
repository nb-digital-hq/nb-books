-- V16: catálogos OPDS que el usuario añade a mano.
--
-- # Por qué ninguno viene puesto
--
-- Porque un catálogo preconfigurado es una recomendación comercial disfrazada de
-- función. La ficha lo dice sin rodeos —"ninguno preconfigurado con
-- ánimo comercial"— y aquí eso significa que la tabla nace vacía y solo la llena
-- el usuario. El puente institucional traerá los suyos por otra vía,
-- y entonces se sabrá de dónde salieron.
--
-- # Qué se guarda, y qué no
--
-- La dirección y el nombre que el usuario le dio. **No se guarda nada de lo que
-- el catálogo devuelve**: ni sus entradas, ni su índice, ni lo que se buscó en
-- él. Un catálogo es una dirección a la que se va, no una copia que se mantiene,
-- y guardar su contenido sería mantener sincronizada una caché de algo que
-- cambia sin avisarnos.
--
-- El registro de qué salió del dispositivo vive en `network_log` (V13), y guarda
-- el anfitrión y el propósito, nunca la ruta ni la consulta.
--
-- Contrato: docs/architecture/metadata-model.md.

CREATE TABLE catalogs (
    id       TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- Nombre legible. Lo pone el usuario, o se toma del título del propio feed
    -- la primera vez que responde.
    title    TEXT NOT NULL,
    -- Dirección de la raíz del catálogo. Única: dos filas con la misma
    -- dirección son la misma cosa dos veces.
    url      TEXT NOT NULL UNIQUE,
    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(title)) > 0),
    -- Solo `https`. En claro, la dirección de lo que alguien descarga queda a la
    -- vista de cualquiera en la red, y no hay ningún uso en este producto que lo
    -- justifique. Es la misma regla que hace cumplir `net::host_de`.
    CHECK (url LIKE 'https://%')
) STRICT;

CREATE INDEX idx_catalogs_added ON catalogs(added_at DESC);
