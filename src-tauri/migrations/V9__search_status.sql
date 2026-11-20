-- V9: si un documento se puede buscar, y por qué no.
--
-- # Por qué hace falta guardarlo
--
-- Un índice vacío no distingue entre "este documento no tiene texto" y "este
-- documento todavía no se ha indexado". Sin esa distinción, la interfaz solo
-- puede callarse, y presentar un documento como buscable cuando no lo es genera
-- desconfianza en la función completa: el usuario busca una frase que sabe que
-- está en el libro, no la encuentra, y a partir de ahí no se fía de ninguna
-- búsqueda.
--
-- # Por qué el motivo importa tanto como el estado
--
-- "No se puede buscar" son cuatro cosas distintas y la interfaz dice cuatro
-- cosas distintas:
--
--   * `IMAGE_ONLY`      un cómic. No tiene texto y nunca lo tendrá.
--   * `NO_TEXT_LAYER`   un escaneo sin OCR. Tiene arreglo, y es el único caso en
--                       el que ofrecer OCR significa algo.
--   * `FILE_UNAVAILABLE` el volumen está desconectado. Vuelve solo.
--   * `NO_ENGINE`       esta instalación no puede abrir ese formato.
--
-- Un solo estado "no buscable" obligaría a ofrecer OCR sobre un cómic.
--
-- Contratos: docs/architecture/search.md, sección 4, y
-- docs/architecture/api/schemas.md.

CREATE TABLE search_status (
    edition_id TEXT PRIMARY KEY REFERENCES editions(id) ON DELETE CASCADE,
    state      TEXT NOT NULL
               CHECK (state IN ('pending', 'searchable', 'not_searchable')),
    -- Código canónico, nunca un mensaje: el texto lo pone la interfaz, en el
    -- idioma del usuario. Nulo cuando el estado no necesita explicación.
    reason     TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    -- Un estado buscable no lleva motivo, y uno no buscable siempre lo lleva.
    -- Sin esto, "no buscable" sin motivo llegaría a la interfaz y no habría nada
    -- que decirle al usuario, que es exactamente lo que esta tabla evita.
    CHECK ((state = 'not_searchable') = (reason IS NOT NULL))
) STRICT;

CREATE INDEX idx_search_status_state ON search_status(state);
