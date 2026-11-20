-- V43: a igual fecha de alta o autor, por título de la A a la Z (A15).
--
-- # Por qué
--
-- Toda la importación inicial comparte fecha de alta, así que "Añadido" se
-- desempataba por el identificador, que es un hash, y la biblioteca parecía un
-- orden alfabético al revés a partir de la primera fila. Ahora el desempate es el
-- título, siempre de la A a la Z.
--
-- # Por qué dos índices por orden
--
-- Ordenar sin árbol temporal es un guardián (`recorrido_biblioteca.rs`): con él,
-- la página 1.500 costaba 145,8 ms; sin él, 4,0. La fecha va en la dirección que
-- se elija y el título siempre ascendente, y un índice solo sirve recorrido en
-- su orden o al revés del todo: hace falta uno para cada dirección. Los nuevos
-- empiezan por la misma columna que los de V17 y V19, así que los sustituyen.

DROP INDEX IF EXISTS idx_editions_alta;
DROP INDEX IF EXISTS idx_editions_autor;

CREATE INDEX idx_editions_alta_asc
    ON editions(created_at ASC, title COLLATE NOCASE ASC, id ASC);
CREATE INDEX idx_editions_alta_desc
    ON editions(created_at DESC, title COLLATE NOCASE ASC, id ASC);
CREATE INDEX idx_editions_autor_asc
    ON editions(author_sort COLLATE NOCASE ASC, title COLLATE NOCASE ASC, id ASC);
CREATE INDEX idx_editions_autor_desc
    ON editions(author_sort COLLATE NOCASE DESC, title COLLATE NOCASE ASC, id ASC);
