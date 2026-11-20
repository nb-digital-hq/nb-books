-- V31: los tableros de notas pasan de rejilla a columnas con nombre.
--
-- # Qué cambia, y quién lo decidió
--
-- V23 guardaba cada nota en una celda de una rejilla de 6x8. Una rejilla
-- sin nombres no dice qué significa estar en una celda y no en otra, así que
-- colocar una nota no ordenaba nada. Se eligen **columnas con nombre**: cada
-- columna dice qué agrupa, las notas son tarjetas dentro de ella, y mover una
-- tarjeta de columna es decir algo de esa nota.
--
-- # Lo que no cambia
--
-- * **Un tablero sigue siendo una vista sobre las notas.** Aquí solo se señala
--   una nota; borrar el tablero, una columna o una tarjeta no pierde ninguna, y
--   borrar una nota la quita de los tableros sin dejar una tarjeta rota.
-- * **Sigue siendo finito.** Cuántas columnas y tarjetas caben lo fija el runtime
--   (`boards::MAXIMO_DE_COLUMNAS` y `boards::MAXIMO_DE_TARJETAS`) y no el
--   esquema, por lo mismo que en V23: cambiarlo no debe pedir una migración.
-- * **Una tarjeta lleva una nota o un tablero, nunca las dos ni ninguna.**
--
-- # Lo nuevo que impone el esquema
--
-- **Una nota está una sola vez en cada tablero.** En una rejilla, la misma nota
-- en dos celdas era raro pero significaba algo; en columnas, estar en dos a la vez
-- contradice lo que la columna dice. Lo mismo un enlace al mismo tablero.
--
-- # Cómo se trae lo que había
--
-- Cada columna usada de la rejilla pasa a ser una columna con nombre, "Columna
-- N" con su número de antes, en el mismo orden; sus celdas, de arriba abajo, son
-- sus tarjetas. Es lo que conserva el sitio de cada cosa: lo que estaba a la
-- izquierda sigue a la izquierda y lo que estaba arriba sigue arriba. Si una nota
-- estaba en dos celdas del mismo tablero se queda la primera en orden de lectura.
--
-- El nombre va en español porque la base no conoce el idioma de la interfaz; es
-- un nombre del usuario desde este momento y se renombra como cualquier otro.
-- Medido el 2026-09-12: la instalación real no tiene ningún tablero.

CREATE TABLE note_board_columns (
    id       TEXT PRIMARY KEY,
    board_id TEXT NOT NULL REFERENCES note_boards(id) ON DELETE CASCADE,
    title    TEXT NOT NULL,
    position INTEGER NOT NULL,

    CHECK (length(trim(title)) > 0),
    CHECK (position >= 0)
) STRICT;

CREATE INDEX idx_note_board_columns_board ON note_board_columns(board_id, position);

CREATE TABLE note_board_cards (
    id        TEXT PRIMARY KEY,
    -- Repetido desde la columna para poder exigir la unicidad por tablero con un
    -- índice, que es donde SQLite puede exigirla.
    board_id  TEXT NOT NULL REFERENCES note_boards(id) ON DELETE CASCADE,
    column_id TEXT NOT NULL REFERENCES note_board_columns(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL,
    note_id   TEXT REFERENCES notes(id) ON DELETE CASCADE,
    links_to  TEXT REFERENCES note_boards(id) ON DELETE CASCADE,

    CHECK (position >= 0),
    CHECK ((note_id IS NULL) <> (links_to IS NULL))
) STRICT;

CREATE INDEX idx_note_board_cards_column ON note_board_cards(column_id, position);
-- Para responder "en qué tableros está esta nota" sin recorrerlos todos.
CREATE INDEX idx_note_board_cards_note ON note_board_cards(note_id);
CREATE UNIQUE INDEX idx_note_board_cards_nota_unica
    ON note_board_cards(board_id, note_id) WHERE note_id IS NOT NULL;
CREATE UNIQUE INDEX idx_note_board_cards_enlace_unico
    ON note_board_cards(board_id, links_to) WHERE links_to IS NOT NULL;

-- Cada columna usada de la rejilla, con su identificador nuevo.
CREATE TEMP TABLE columnas_de_rejilla AS
SELECT board_id,
       col_index,
       lower(hex(randomblob(16))) AS column_id,
       (SELECT count(DISTINCT antes.col_index)
          FROM note_board_cells antes
         WHERE antes.board_id = usadas.board_id
           AND antes.col_index < usadas.col_index) AS position
  FROM (SELECT DISTINCT board_id, col_index FROM note_board_cells) usadas;

INSERT INTO note_board_columns (id, board_id, title, position)
SELECT column_id, board_id, 'Columna ' || (col_index + 1), position
  FROM columnas_de_rejilla;

-- Las celdas, como tarjetas en orden de fila. Una nota o un enlace repetidos en
-- el mismo tablero se quedan en su primera celda en orden de lectura.
INSERT INTO note_board_cards (id, board_id, column_id, position, note_id, links_to)
SELECT lower(hex(randomblob(16))),
       celda.board_id,
       columna.column_id,
       (SELECT count(*)
          FROM note_board_cells arriba
         WHERE arriba.board_id = celda.board_id
           AND arriba.col_index = celda.col_index
           AND arriba.row_index < celda.row_index
           AND NOT EXISTS (
               SELECT 1 FROM note_board_cells previa
                WHERE previa.board_id = arriba.board_id
                  AND (previa.row_index, previa.col_index) < (arriba.row_index, arriba.col_index)
                  AND (previa.note_id = arriba.note_id OR previa.links_to = arriba.links_to)
           )),
       celda.note_id,
       celda.links_to
  FROM note_board_cells celda
  JOIN columnas_de_rejilla columna
    ON columna.board_id = celda.board_id AND columna.col_index = celda.col_index
 WHERE NOT EXISTS (
     SELECT 1 FROM note_board_cells previa
      WHERE previa.board_id = celda.board_id
        AND (previa.row_index, previa.col_index) < (celda.row_index, celda.col_index)
        AND (previa.note_id = celda.note_id OR previa.links_to = celda.links_to)
 );

DROP TABLE columnas_de_rejilla;
DROP TABLE note_board_cells;
