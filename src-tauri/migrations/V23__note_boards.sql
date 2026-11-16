-- V23: tableros espaciales sobre las notas que ya existen.
--
-- # Qué guarda esto y qué no
--
-- Guarda **dónde** está cada nota en un tablero. No guarda notas: una nota vive
-- en `notes` desde V12 y aquí solo se la señala. Un tablero es una vista sobre
-- lo que ya hay, y borrarlo no pierde ni una nota.
--
-- # Por qué la posición es fila y columna y no dos números cualesquiera
--
-- Porque el tablero es **finito**. Un lienzo infinito no tiene forma de
-- respetar el presupuesto de 1.500 nodos del documento: siempre cabe una nota
-- más. Con una rejilla acotada, lo que se pinta está acotado por construcción y
-- no por disciplina.
--
-- Y porque una rejilla se recorre con el teclado sin inventar nada: las flechas
-- van de celda a celda. Con coordenadas libres en píxeles, "la siguiente" no
-- significa nada, y el criterio de la ficha exige que todo tablero sea operable
-- solo con teclado.
--
-- El tamaño de la rejilla lo fija el runtime y no el esquema: cambiarlo no debe
-- pedir una migración.
--
-- # Por qué una celda puede llevar otro tablero
--
-- Porque la ficha pide tableros que se enlacen entre sí. Sin eso, la única
-- forma de tener más de una rejilla sería hacer la rejilla más grande, que es
-- volver al lienzo infinito por la puerta de atrás.
--
-- Una celda lleva **una nota o un tablero, nunca las dos ni ninguna**: la
-- restricción está aquí y no en el código, porque una fila con las dos cosas no
-- significa nada y no debe poder existir.
--
-- Contrato: `src-tauri/src/boards.rs`.

CREATE TABLE note_boards (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(title)) > 0)
) STRICT;

CREATE TABLE note_board_cells (
    board_id  TEXT NOT NULL REFERENCES note_boards(id) ON DELETE CASCADE,
    row_index INTEGER NOT NULL,
    col_index INTEGER NOT NULL,
    -- La nota que ocupa la celda, si la ocupa una nota. Si la nota se borra, la
    -- celda se vacía sola: un tablero no puede señalar a lo que ya no está.
    note_id   TEXT REFERENCES notes(id) ON DELETE CASCADE,
    -- El tablero al que lleva, si la celda es un enlace.
    links_to  TEXT REFERENCES note_boards(id) ON DELETE CASCADE,

    PRIMARY KEY (board_id, row_index, col_index),
    CHECK (row_index >= 0 AND col_index >= 0),
    CHECK ((note_id IS NULL) <> (links_to IS NULL))
) STRICT, WITHOUT ROWID;

-- Para responder "en qué tableros está esta nota" sin recorrerlos todos.
CREATE INDEX idx_note_board_cells_note ON note_board_cells(note_id);
