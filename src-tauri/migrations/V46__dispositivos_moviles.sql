-- V46: teléfonos emparejados, qué recibe cada uno y qué cambió desde la última vez.
--
-- Contrato: docs/architecture/mobile-contract.md y ADR-016.
--
-- Un teléfono no recibe la base entera: recibe lo que cambió desde la última
-- versión que aplicó, y solo de lo que eligió. Las dos cosas se resuelven aquí
-- y no en el código porque las dos dependen de escrituras que hace media
-- aplicación (importar, editar metadatos, anotar, repasar), y un cambio que el
-- código olvida anunciar es un teléfono que se queda con un dato viejo para
-- siempre sin que ninguna prueba lo vea.

-- ---------------------------------------------------------------------------
-- Teléfonos emparejados.
--
-- Del token solo se guarda su SHA-256: la base viaja en las copias de
-- seguridad, y una copia no puede servir para hacerse pasar por un teléfono.
-- Revocar no borra la fila: deja constancia de que existió y cierra la puerta.
-- ---------------------------------------------------------------------------
CREATE TABLE dispositivos (
    id               TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    nombre           TEXT NOT NULL,
    plataforma       TEXT NOT NULL CHECK (plataforma IN ('android', 'ios')),
    huella_del_token TEXT NOT NULL UNIQUE,
    emparejado_en    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    visto_en         TEXT,
    revocado_en      TEXT,

    CHECK (length(trim(nombre)) > 0),
    CHECK (length(huella_del_token) = 64)
) STRICT;

-- ---------------------------------------------------------------------------
-- Qué recibe cada teléfono.
--
-- Una fila por elección, y se combinan: unas colecciones, más un autor, más
-- libros sueltos. `todo` excluye a las demás; sin ninguna fila, el teléfono no
-- recibe nada. Las estanterías son las de V24: autor, serie y materia, por su
-- clave.
-- ---------------------------------------------------------------------------
CREATE TABLE seleccion_de_dispositivos (
    dispositivo_id TEXT NOT NULL REFERENCES dispositivos(id) ON DELETE CASCADE,
    tipo           TEXT NOT NULL
                   CHECK (tipo IN ('todo', 'coleccion', 'autor', 'serie', 'materia', 'edicion')),
    -- Vacía en `todo`; el identificador o la clave de la estantería en el resto.
    clave          TEXT NOT NULL DEFAULT '',

    PRIMARY KEY (dispositivo_id, tipo, clave),
    CHECK ((tipo = 'todo') = (clave = ''))
) STRICT, WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- Dónde va cada teléfono en cada libro.
--
-- Aparte de `progress`, que es la posición de este equipo: una posición por
-- dispositivo, y ninguna pisa a otra. El teléfono la sube; el PC la enseña como
-- "ir a la posición del teléfono" y nunca la toma por suya.
-- ---------------------------------------------------------------------------
CREATE TABLE progreso_de_dispositivos (
    dispositivo_id TEXT NOT NULL REFERENCES dispositivos(id) ON DELETE CASCADE,
    edition_id     TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- El Locator, serializado. Nunca un entero: ver ADR-004.
    locator        TEXT NOT NULL,
    actualizado_en TEXT NOT NULL,

    PRIMARY KEY (dispositivo_id, edition_id),
    CHECK (json_valid(locator) AND json_type(locator) = 'object')
) STRICT, WITHOUT ROWID;

-- ---------------------------------------------------------------------------
-- Registro de repasos.
--
-- El estado de una tarjeta (vencimiento, intervalo, facilidad) se deriva: es el
-- resultado de aplicar SM-2 a sus repasos en orden. Si dos dispositivos
-- repasan la misma tarjeta sin verse, sincronizar el estado haría ganar a uno y
-- perder al otro; sincronizar los repasos y recalcular da el mismo estado en
-- los dos, sin elegir a nadie.
-- ---------------------------------------------------------------------------
CREATE TABLE registro_de_repasos (
    id           TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    card_id      TEXT NOT NULL REFERENCES study_cards(id) ON DELETE CASCADE,
    calificacion TEXT NOT NULL CHECK (calificacion IN ('again', 'hard', 'good', 'easy')),
    repasado_en  TEXT NOT NULL,
    -- Nulo cuando se repasó en este equipo.
    dispositivo_id TEXT REFERENCES dispositivos(id) ON DELETE SET NULL
) STRICT;

CREATE INDEX idx_registro_de_repasos_tarjeta ON registro_de_repasos(card_id, repasado_en, id);

-- El punto de partida de cada tarjeta. Con el registro, el estado es
-- `base + repasos en orden`; las tarjetas que ya tenían repasos antes de
-- existir el registro parten de su estado de hoy.
ALTER TABLE study_cards ADD COLUMN base_repetitions INTEGER NOT NULL DEFAULT 0 CHECK (base_repetitions >= 0);
ALTER TABLE study_cards ADD COLUMN base_interval_days INTEGER NOT NULL DEFAULT 0 CHECK (base_interval_days >= 0);
ALTER TABLE study_cards ADD COLUMN base_ease_factor REAL NOT NULL DEFAULT 2.5 CHECK (base_ease_factor >= 1.3);
ALTER TABLE study_cards ADD COLUMN base_due_at TEXT NOT NULL DEFAULT '';
ALTER TABLE study_cards ADD COLUMN base_last_reviewed_at TEXT;

UPDATE study_cards
   SET base_repetitions = repetitions,
       base_interval_days = interval_days,
       base_ease_factor = ease_factor,
       base_due_at = due_at,
       base_last_reviewed_at = last_reviewed_at;

-- Una tarjeta nueva parte del estado con el que nace.
CREATE TRIGGER trg_study_cards_base AFTER INSERT ON study_cards
WHEN NEW.base_due_at = ''
BEGIN
    UPDATE study_cards
       SET base_repetitions = NEW.repetitions,
           base_interval_days = NEW.interval_days,
           base_ease_factor = NEW.ease_factor,
           base_due_at = NEW.due_at,
           base_last_reviewed_at = NEW.last_reviewed_at
     WHERE id = NEW.id;
END;

-- ---------------------------------------------------------------------------
-- Revisión de las notas.
--
-- El teléfono propone un cambio diciendo qué revisión vio. Si la nota cambió
-- después, el cambio no la pisa: entra como nota nueva.
-- ---------------------------------------------------------------------------
ALTER TABLE notes ADD COLUMN revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0);

CREATE TRIGGER trg_notes_revision AFTER UPDATE OF title, body ON notes
WHEN NEW.revision = OLD.revision
BEGIN
    UPDATE notes SET revision = OLD.revision + 1 WHERE id = NEW.id;
END;

-- ---------------------------------------------------------------------------
-- Registro de cambios.
--
-- Una fila por cosa cambiada, con la versión del último cambio: cada disparador
-- borra la fila anterior de esa cosa e inserta una nueva al final, así que la
-- tabla no crece con cada edición sino con cada cosa distinta. Se borra e
-- inserta, y no se usa `INSERT OR REPLACE`, porque dentro de un disparador la
-- política de conflicto de la sentencia que lo dispara manda sobre la suya: un
-- `INSERT OR IGNORE` de la aplicación convertiría el reemplazo en un silencio. La
-- versión es el `rowid` de AUTOINCREMENT, que nunca se reutiliza aunque se
-- borren filas: un teléfono que pide "desde la 812" nunca se salta un cambio.
--
-- Lo que cambia de una obra (su título, sus autores, su archivo, sus materias)
-- se registra como cambio de la **edición**: el teléfono pide la ficha entera
-- de nuevo, y una lista de campos tocados solo serviría para que alguien
-- decidiera no actualizar lo que sí cambió.
-- ---------------------------------------------------------------------------
CREATE TABLE registro_de_cambios (
    version  INTEGER PRIMARY KEY AUTOINCREMENT,
    entidad  TEXT NOT NULL CHECK (entidad IN (
                 'edicion', 'coleccion', 'anotacion', 'progreso', 'sesion',
                 'vocabulario', 'tarjeta', 'repaso', 'nota')),
    clave    TEXT NOT NULL,
    borrado  INTEGER NOT NULL DEFAULT 0 CHECK (borrado IN (0, 1)),

    UNIQUE (entidad, clave)
) STRICT;

-- Ediciones y lo que cuelga de ellas.
CREATE TRIGGER trg_cambios_edicion_alta AFTER INSERT ON editions BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('edicion', NEW.id);
END;
CREATE TRIGGER trg_cambios_edicion AFTER UPDATE ON editions BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('edicion', NEW.id);
END;
CREATE TRIGGER trg_cambios_edicion_baja AFTER DELETE ON editions BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = OLD.id;
    INSERT INTO registro_de_cambios (entidad, clave, borrado) VALUES ('edicion', OLD.id, 1);
END;
CREATE TRIGGER trg_cambios_obra AFTER UPDATE ON works BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave IN (SELECT id FROM editions WHERE work_id = NEW.id);
    INSERT INTO registro_de_cambios (entidad, clave)
    SELECT 'edicion', id FROM editions WHERE work_id = NEW.id;
END;
CREATE TRIGGER trg_cambios_archivo_alta AFTER INSERT ON files BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = NEW.edition_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('edicion', NEW.edition_id);
END;
-- Solo lo que el teléfono ve del archivo; el tamaño cambia con la huella, así
-- que basta con ella. La ruta en el disco del PC no le incumbe, y re-enviar
-- 150 000 fichas por mover una carpeta sería absurdo.
CREATE TRIGGER trg_cambios_archivo AFTER UPDATE OF content_hash, media_type, availability ON files BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = NEW.edition_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('edicion', NEW.edition_id);
END;
CREATE TRIGGER trg_cambios_archivo_baja AFTER DELETE ON files BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave IN (SELECT OLD.edition_id WHERE EXISTS (SELECT 1 FROM editions WHERE id = OLD.edition_id));
    INSERT INTO registro_de_cambios (entidad, clave)
    SELECT 'edicion', OLD.edition_id WHERE EXISTS (SELECT 1 FROM editions WHERE id = OLD.edition_id);
END;
CREATE TRIGGER trg_cambios_autoria_alta AFTER INSERT ON edition_contributors BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = NEW.edition_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('edicion', NEW.edition_id);
END;
CREATE TRIGGER trg_cambios_autoria_baja AFTER DELETE ON edition_contributors BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave IN (SELECT OLD.edition_id WHERE EXISTS (SELECT 1 FROM editions WHERE id = OLD.edition_id));
    INSERT INTO registro_de_cambios (entidad, clave)
    SELECT 'edicion', OLD.edition_id WHERE EXISTS (SELECT 1 FROM editions WHERE id = OLD.edition_id);
END;
CREATE TRIGGER trg_cambios_autor AFTER UPDATE OF name ON contributors BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave IN (SELECT edition_id FROM edition_contributors WHERE contributor_id = NEW.id);
    INSERT INTO registro_de_cambios (entidad, clave)
    SELECT 'edicion', edition_id FROM edition_contributors WHERE contributor_id = NEW.id;
END;
CREATE TRIGGER trg_cambios_materia_alta AFTER INSERT ON edition_subjects BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave = NEW.edition_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('edicion', NEW.edition_id);
END;
CREATE TRIGGER trg_cambios_materia_baja AFTER DELETE ON edition_subjects BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'edicion' AND clave IN (SELECT OLD.edition_id WHERE EXISTS (SELECT 1 FROM editions WHERE id = OLD.edition_id));
    INSERT INTO registro_de_cambios (entidad, clave)
    SELECT 'edicion', OLD.edition_id WHERE EXISTS (SELECT 1 FROM editions WHERE id = OLD.edition_id);
END;

-- Colecciones: la colección y su contenido viajan juntos.
CREATE TRIGGER trg_cambios_coleccion_alta AFTER INSERT ON collections BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'coleccion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('coleccion', NEW.id);
END;
CREATE TRIGGER trg_cambios_coleccion AFTER UPDATE ON collections BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'coleccion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('coleccion', NEW.id);
END;
CREATE TRIGGER trg_cambios_coleccion_baja AFTER DELETE ON collections BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'coleccion' AND clave = OLD.id;
    INSERT INTO registro_de_cambios (entidad, clave, borrado) VALUES ('coleccion', OLD.id, 1);
END;
CREATE TRIGGER trg_cambios_miembro_alta AFTER INSERT ON collection_items BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'coleccion' AND clave = NEW.collection_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('coleccion', NEW.collection_id);
END;
CREATE TRIGGER trg_cambios_miembro AFTER UPDATE ON collection_items BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'coleccion' AND clave = NEW.collection_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('coleccion', NEW.collection_id);
END;
CREATE TRIGGER trg_cambios_miembro_baja AFTER DELETE ON collection_items BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'coleccion' AND clave IN (SELECT OLD.collection_id WHERE EXISTS (SELECT 1 FROM collections WHERE id = OLD.collection_id));
    INSERT INTO registro_de_cambios (entidad, clave)
    SELECT 'coleccion', OLD.collection_id WHERE EXISTS (SELECT 1 FROM collections WHERE id = OLD.collection_id);
END;

-- Lo que la persona hace al leer.
CREATE TRIGGER trg_cambios_anotacion_alta AFTER INSERT ON annotations BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'anotacion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('anotacion', NEW.id);
END;
CREATE TRIGGER trg_cambios_anotacion AFTER UPDATE ON annotations BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'anotacion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('anotacion', NEW.id);
END;
CREATE TRIGGER trg_cambios_anotacion_baja AFTER DELETE ON annotations BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'anotacion' AND clave = OLD.id;
    INSERT INTO registro_de_cambios (entidad, clave, borrado) VALUES ('anotacion', OLD.id, 1);
END;
CREATE TRIGGER trg_cambios_progreso_alta AFTER INSERT ON progress BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'progreso' AND clave = NEW.edition_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('progreso', NEW.edition_id);
END;
CREATE TRIGGER trg_cambios_progreso AFTER UPDATE ON progress BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'progreso' AND clave = NEW.edition_id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('progreso', NEW.edition_id);
END;
CREATE TRIGGER trg_cambios_sesion_alta AFTER INSERT ON reading_sessions BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'sesion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('sesion', NEW.id);
END;
CREATE TRIGGER trg_cambios_sesion AFTER UPDATE ON reading_sessions BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'sesion' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('sesion', NEW.id);
END;
CREATE TRIGGER trg_cambios_vocabulario_alta AFTER INSERT ON vocabulary BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'vocabulario' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('vocabulario', NEW.id);
END;
CREATE TRIGGER trg_cambios_vocabulario_baja AFTER DELETE ON vocabulary BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'vocabulario' AND clave = OLD.id;
    INSERT INTO registro_de_cambios (entidad, clave, borrado) VALUES ('vocabulario', OLD.id, 1);
END;
CREATE TRIGGER trg_cambios_tarjeta_alta AFTER INSERT ON study_cards BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'tarjeta' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('tarjeta', NEW.id);
END;
-- El estado de repaso se deriva del registro de repasos, que viaja aparte; aquí
-- solo cuenta lo que se edita de la tarjeta.
CREATE TRIGGER trg_cambios_tarjeta AFTER UPDATE OF front, back, choices, edited_at ON study_cards BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'tarjeta' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('tarjeta', NEW.id);
END;
CREATE TRIGGER trg_cambios_tarjeta_baja AFTER DELETE ON study_cards BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'tarjeta' AND clave = OLD.id;
    INSERT INTO registro_de_cambios (entidad, clave, borrado) VALUES ('tarjeta', OLD.id, 1);
END;
CREATE TRIGGER trg_cambios_repaso_alta AFTER INSERT ON registro_de_repasos BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'repaso' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('repaso', NEW.id);
END;
CREATE TRIGGER trg_cambios_nota_alta AFTER INSERT ON notes BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'nota' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('nota', NEW.id);
END;
CREATE TRIGGER trg_cambios_nota AFTER UPDATE OF title, body ON notes BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'nota' AND clave = NEW.id;
    INSERT INTO registro_de_cambios (entidad, clave) VALUES ('nota', NEW.id);
END;
CREATE TRIGGER trg_cambios_nota_baja AFTER DELETE ON notes BEGIN
    DELETE FROM registro_de_cambios WHERE entidad = 'nota' AND clave = OLD.id;
    INSERT INTO registro_de_cambios (entidad, clave, borrado) VALUES ('nota', OLD.id, 1);
END;

-- Lo que ya existe entra como un primer cambio, para que el primer teléfono
-- emparejado lo reciba sin un caso especial. El orden (catálogo antes que lo
-- que cuelga de él) es el orden en que el teléfono lo puede aplicar.
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'edicion', id FROM editions;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'coleccion', id FROM collections;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'nota', id FROM notes;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'tarjeta', id FROM study_cards;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'anotacion', id FROM annotations;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'progreso', edition_id FROM progress;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'sesion', id FROM reading_sessions;
INSERT INTO registro_de_cambios (entidad, clave) SELECT 'vocabulario', id FROM vocabulary;
