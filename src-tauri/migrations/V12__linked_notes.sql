-- V12: notas propias, con enlaces y retroenlaces.
--
-- # Por qué existen, además de las anotaciones
--
-- Una anotación cuelga de un documento y se ancla a un punto de él. Una nota no:
-- es del usuario y existe por sí sola. Sin ellas, lo único que NB Books sabe
-- guardar es lo que alguien escribió *sobre* un libro, y no lo que piensa a
-- partir de varios.
--
-- Hay además un motivo que no es de simetría. La investigación de
-- docs/design/reading-experience/README.md, sección 1.1, es concluyente en
-- dos puntos: resaltar por sí solo no mejora el aprendizaje, y quien lee en
-- digital casi no subraya. Lo que funciona es escribir algo propio. Un producto
-- que solo facilita resaltar está optimizando la conducta que no sirve.
--
-- # Por qué una nota sí se edita y una anotación no
--
-- Una anotación es inmutable (V7) porque ancla a un texto que puede moverse, y
-- porque una edición que sobrescribe pierde el trabajo de lectura sin rastro.
-- Una nota no ancla a nada: es un escrito propio, y un escrito propio se
-- reescribe. Imponerle inmutabilidad convertiría cada corrección de una errata
-- en una versión nueva, y en un mes el usuario tendría cuarenta versiones de la
-- misma nota. Es la misma pregunta con dos respuestas distintas porque son dos
-- cosas distintas.
--
-- # Los retroenlaces no se guardan
--
-- Un enlace es una fila. El retroenlace es esa misma fila leída del otro lado.
-- Guardarlo aparte abriría la posibilidad de que las dos direcciones dejaran de
-- coincidir, que es la clase de incoherencia que nadie detecta hasta que ya está
-- extendida. Con `ON DELETE CASCADE`, borrar una nota se lleva sus enlaces en
-- los dos sentidos y no queda ningún retroenlace apuntando al vacío.
--
-- Contrato: docs/architecture/api/schemas.md.

CREATE TABLE notes (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- El título es también el nombre con el que otras notas la enlazan, así que
    -- no puede estar vacío ni repetirse: dos notas con el mismo nombre hacen
    -- ambiguo cualquier enlace escrito a mano.
    title      TEXT NOT NULL UNIQUE,
    body       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(title)) > 0)
) STRICT;

CREATE TRIGGER trg_notes_updated_at
AFTER UPDATE ON notes
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE notes SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- ---------------------------------------------------------------------------
-- Enlaces entre notas.
-- ---------------------------------------------------------------------------
CREATE TABLE note_links (
    source_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    target_id  TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (source_id, target_id),
    -- Una nota enlazada a sí misma no aporta nada y ensucia todo retroenlace.
    CHECK (source_id <> target_id)
) STRICT, WITHOUT ROWID;

-- El índice por destino es lo que hace que un retroenlace sea una consulta
-- barata y no un recorrido de toda la tabla.
CREATE INDEX idx_note_links_target ON note_links(target_id);

-- ---------------------------------------------------------------------------
-- Enlaces de una nota a una anotación anclada.
--
-- Tabla aparte y no una columna polimórfica con un `target_kind`: una clave
-- foránea polimórfica no la puede hacer cumplir la base, y este proyecto
-- prefiere que las reglas vivan en el esquema. Ver la misma decisión en la V7.
-- ---------------------------------------------------------------------------
CREATE TABLE note_annotation_links (
    note_id       TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    annotation_id TEXT NOT NULL REFERENCES annotations(id) ON DELETE CASCADE,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (note_id, annotation_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_note_annotation_links_annotation ON note_annotation_links(annotation_id);
