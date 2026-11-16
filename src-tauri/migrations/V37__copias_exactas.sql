-- V37: las copias exactas que encontró una importación.
--
-- # Qué se callaba
--
-- La identidad por contenido ya distinguía tres cosas al importar: el
-- mismo archivo visto otra vez, el mismo archivo movido, y **otra copia del mismo
-- contenido en otra ruta que sigue existiendo**. Las tres terminaban en el mismo
-- recuento, "ya estaban", así que quien importaba una carpeta con el mismo libro
-- dos veces no se enteraba nunca de que lo tenía dos veces. No se creaban dos
-- entradas, que es lo que importa, pero tampoco se decía, y eso es justo lo que
-- Komga resuelve con hashes y Calibre no.
--
-- # Qué se guarda
--
-- El recuento total en la ejecución, que no depende de ningún tope, y las
-- primeras copias con dos datos: la ruta de la copia que no entró, relativa a la
-- carpeta elegida como las de los rechazos, y el archivo registrado del que es
-- copia. Del original no se copia nada: se consulta, así que si después sale de
-- la biblioteca, su fila se va con él por la clave en cascada y la lista no dice
-- que algo es copia de lo que ya no está.
--
-- Contrato: docs/architecture/storage.md.

ALTER TABLE import_runs ADD COLUMN duplicates INTEGER NOT NULL DEFAULT 0 CHECK (duplicates >= 0);

CREATE TABLE import_duplicates (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id     TEXT NOT NULL REFERENCES import_runs(id) ON DELETE CASCADE,
    -- La ruta de la copia que no entró, para poder ir a mirarla.
    path       TEXT NOT NULL,
    -- El archivo que ya estaba y tiene el mismo contenido.
    file_id    TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(path)) > 0)
) STRICT;

CREATE INDEX idx_import_duplicates_run ON import_duplicates(run_id, id);
-- Sin él, borrar un archivo de la biblioteca recorrería la tabla entera para
-- cumplir la cascada.
CREATE INDEX idx_import_duplicates_file ON import_duplicates(file_id);
