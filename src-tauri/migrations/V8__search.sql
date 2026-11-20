-- V8: índice de texto completo.
--
-- Dos tablas que son una sola cosa: `text_chunks` guarda el texto con **el
-- locator de dónde empieza**, y `documents_fts` es el índice invertido de ese
-- texto. La segunda no duplica el contenido: es una tabla FTS5 de contenido
-- externo, así que apunta a la primera y no guarda una copia.
--
-- Por qué importa que no duplique: una biblioteca de ciento cincuenta mil libros
-- son varios gigabytes de texto, y tenerlo dos veces convertiría el índice en el
-- archivo más grande del usuario.
--
-- # Esto es caché, no datos
--
-- Todo lo de aquí se puede regenerar leyendo los documentos otra vez. Perderlo
-- cuesta tiempo de CPU, no información. Por eso `text_chunks` puede vaciarse y
-- rehacerse sin transacción de rescate, y por eso la reconstrucción es
-- una operación normal y no una recuperación de desastre.
--
-- # Sin una sola columna numérica
--
-- La posición de cada fragmento vive en su `locator` y solo ahí. Un
-- `char_start INTEGER` habría sido cómodo y habría sido el entero de posición
-- que ADR-004 descarta, con la excusa de que "esto es solo el índice".
--
-- Contratos: docs/architecture/search.md y docs/architecture/api/schemas.md.
--.

CREATE TABLE text_chunks (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Dónde empieza este fragmento, con todos sus selectores. Es lo que permite
    -- que un resultado abra la posición exacta y no el principio del capítulo.
    locator    TEXT NOT NULL,
    text       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (json_valid(locator) AND json_type(locator) = 'object')
) STRICT;

CREATE INDEX idx_text_chunks_edition ON text_chunks(edition_id);

-- ---------------------------------------------------------------------------
-- El índice invertido.
--
-- `remove_diacritics 2` no es un detalle de comodidad: en español, buscar
-- "analisis" tiene que encontrar "análisis", y quien escribe deprisa o desde un
-- teclado sin acentos es la mayoría. La variante 2 y no la 1 porque la 1 deja
-- fuera los diacríticos que no caben en un solo punto de código.
-- ---------------------------------------------------------------------------
CREATE VIRTUAL TABLE documents_fts USING fts5(
    text,
    content = 'text_chunks',
    content_rowid = 'rowid',
    tokenize = "unicode61 remove_diacritics 2"
);

-- ---------------------------------------------------------------------------
-- Sincronización.
--
-- Por disparador y no desde Rust, por el mismo motivo que `updated_at`: un
-- índice que el código olvida actualizar produce búsquedas que no encuentran lo
-- que está ahí, y ese olvido no lo detecta ninguna prueba de la ruta feliz.
--
-- No hay disparador de UPDATE porque un fragmento no se edita: es caché, y
-- rehacerlo es borrarlo y volver a insertarlo.
-- ---------------------------------------------------------------------------
CREATE TRIGGER trg_text_chunks_indexar AFTER INSERT ON text_chunks
BEGIN
    INSERT INTO documents_fts(rowid, text) VALUES (new.rowid, new.text);
END;

CREATE TRIGGER trg_text_chunks_desindexar AFTER DELETE ON text_chunks
BEGIN
    INSERT INTO documents_fts(documents_fts, rowid, text)
    VALUES ('delete', old.rowid, old.text);
END;
