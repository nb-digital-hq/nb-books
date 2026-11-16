-- V7: inmutabilidad de las anotaciones.
--
-- Una anotación no se edita. Editarla es **darla de baja y crear otra**, y la
-- anterior se conserva.
--
-- Por qué, si es más trabajo: una anotación es del usuario, y una edición que
-- sobrescribe pierde lo que había sin dejar rastro. Con este modelo, "me he
-- cargado la nota buena" tiene arreglo, la sincronización entre dispositivos no
-- tiene que resolver quién escribió último sobre la misma fila, y exportar el
-- historial es exportar filas en lugar de reconstruir un pasado que no se
-- guardó.
--
-- La regla no vive solo en Rust: los disparadores de abajo la hacen cumplir
-- también por SQL directo. Una regla que solo vive en una capa depende de que
-- nadie entre por otra puerta, y es la misma decisión que con las colecciones
-- institucionales y los secretos de las preferencias.
--
-- Contratos: docs/architecture/api/schemas.md y
-- docs/architecture/reader-contracts.md.

-- Baja lógica. Nula mientras la anotación está viva.
ALTER TABLE annotations ADD COLUMN deleted_at TEXT;

-- La versión anterior de esta anotación, si es la edición de otra.
--
-- Sin `ON DELETE CASCADE` a propósito: la cadena de versiones no se borra en
-- cascada porque borrar una edición ya arrastra la anotación entera por su
-- `edition_id`, y una cascada por aquí solo podría borrar de más.
ALTER TABLE annotations ADD COLUMN supersedes TEXT REFERENCES annotations(id);

CREATE INDEX idx_annotations_supersedes ON annotations(supersedes);
CREATE INDEX idx_annotations_vivas ON annotations(edition_id) WHERE deleted_at IS NULL;

-- ---------------------------------------------------------------------------
-- Lo que hace cumplir la inmutabilidad.
--
-- Se enumeran las columnas en lugar de prohibir todo `UPDATE` porque hay dos
-- que sí se mueven: `deleted_at`, que es la baja, y `updated_at`, que mantiene
-- el disparador de la V2.
-- ---------------------------------------------------------------------------
CREATE TRIGGER trg_annotations_inmutable
BEFORE UPDATE OF edition_id, kind, body, target, created_at, supersedes ON annotations
BEGIN
    SELECT RAISE(ABORT,
        'una anotación es inmutable: editarla es darla de baja y crear otra');
END;

-- Una baja no se deshace escribiendo NULL encima. Reaparecer sin rastro es
-- justo lo que este modelo evita: si el usuario quiere recuperarla, se crea una
-- versión nueva que la suceda, y entonces queda registrado que ocurrió.
CREATE TRIGGER trg_annotations_baja_definitiva
BEFORE UPDATE OF deleted_at ON annotations
WHEN OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NULL
BEGIN
    SELECT RAISE(ABORT, 'una anotación dada de baja no se revive en sitio');
END;
