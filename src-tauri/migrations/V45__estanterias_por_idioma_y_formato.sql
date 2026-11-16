-- V45: estanterías por idioma y por formato (A32).
--
-- Se cuentan al vuelo y no se materializan como V24, pero el filtro que abren
-- tiene que saber cuántas obras hay sin recorrer la tabla: sin índice, contar
-- las obras en español leía las 152.080 ediciones enteras, sinopsis incluida, y
-- tardaba 6 s en frío sobre la base real. El año no lo necesita: entra por el
-- índice de publicación de V44.

CREATE INDEX idx_editions_idioma ON editions(language);
CREATE INDEX idx_files_formato ON files(media_type, edition_id);
