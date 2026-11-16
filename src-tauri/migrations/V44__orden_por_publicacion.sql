-- V44: ordenar por fecha de publicación (A45).
--
-- La biblioteca solo se ordenaba por alta, título, autor, serie y progreso, y la
-- fecha de publicación estaba en 151.755 ediciones sin servir para nada. Un
-- índice para cada dirección, con el título siempre de la A a la Z como
-- desempate, por el mismo motivo que V43: ordenar sin árbol temporal.

CREATE INDEX idx_editions_publicacion_asc
    ON editions(published_date ASC, title COLLATE NOCASE ASC, id ASC);
CREATE INDEX idx_editions_publicacion_desc
    ON editions(published_date DESC, title COLLATE NOCASE ASC, id ASC);
