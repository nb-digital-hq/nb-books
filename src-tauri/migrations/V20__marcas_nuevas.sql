-- V20: el subrayado y el trazo libre, dos clases de marca más.
--
-- # Qué faltaba
--
-- La superficie de lectura pinta sobre el texto lo que ya existía —resaltado, nota y marcador— y
-- dejó dicho lo que no: el **subrayado** de palabras sueltas, que es el gesto de
-- "esta la quiero buscar o traducir", y el **trazo libre**, que es el único que no
-- tenía ningún precedente en el modelo. Es la otra mitad.
--
-- Decisión: el trazo libre entra como anotación, con las demás, y el
-- subrayado es clase propia y no una variante del resaltado. Son gestos distintos
-- y tienen que poder convivir sobre las mismas palabras.
--
-- # Por qué se reconstruye la tabla, y por qué eso era peligroso
--
-- Porque `kind` está acotado por un `CHECK` y SQLite no permite alterarlo. Y
-- reconstruir una tabla con las claves ajenas activas ejecuta un `DELETE`
-- implícito al soltar la vieja, que **dispara las cascadas de quien la
-- referencia**: aquí, `note_annotation_links`, los enlaces entre las notas propias de
-- quien lee y los pasajes que citan. Medido: la reconstrucción entera dentro de
-- una transacción confirma sin error y deja las citas en cero.
--
-- Por eso el ejecutor de `db/migrations.rs` apaga las claves ajenas alrededor de
-- las migraciones y comprueba `foreign_key_check` al terminar, que es el
-- procedimiento que documenta SQLite. Sin esas dos cosas, esta migración pierde
-- datos en silencio. Era la pregunta J14.
--
-- # El trazo libre necesita algo donde vivir
--
-- `target` dice **dónde** está la marca; para un trazo no basta, porque hace falta
-- la geometría. Va en `payload`, un objeto JSON aparte, y no dentro de `body`:
-- `body` es el texto que escribe una persona, y meter coordenadas ahí obligaría a
-- adivinar cuál de las dos cosas es al leerlo. Un trazo sin geometría no es un
-- trazo, así que hay un `CHECK` que lo exige.
--
-- Contrato: docs/architecture/api/schemas.md.

CREATE TABLE annotations_nueva (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Cinco clases. `underline` marca palabras y `drawing` es tinta libre.
    kind       TEXT NOT NULL CHECK (kind IN ('highlight', 'note', 'bookmark', 'underline', 'drawing')),
    -- Texto de la nota. Nulo en un resaltado sin comentario y en un trazo.
    body       TEXT,
    -- Selectores serializados como objeto JSON. La comprobación no es
    -- decorativa: `json_type(...) = 'object'` rechaza en la propia base un
    -- entero de progreso, que es exactamente el modelo que ADR-004 descarta.
    target     TEXT NOT NULL,
    -- Geometría del trazo, cuando la marca es un trazo. Objeto JSON.
    payload    TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    deleted_at TEXT,
    supersedes TEXT REFERENCES annotations(id),

    CHECK (json_valid(target) AND json_type(target) = 'object'),
    CHECK (payload IS NULL OR (json_valid(payload) AND json_type(payload) = 'object')),
    -- Un trazo sin geometría no es un trazo: sería una marca invisible que
    -- ocupa sitio y no se puede pintar.
    CHECK (kind <> 'drawing' OR payload IS NOT NULL)
) STRICT;

INSERT INTO annotations_nueva
    (id, edition_id, kind, body, target, created_at, updated_at, deleted_at, supersedes)
SELECT id, edition_id, kind, body, target, created_at, updated_at, deleted_at, supersedes
  FROM annotations;

DROP TABLE annotations;
ALTER TABLE annotations_nueva RENAME TO annotations;

-- Los índices y los disparadores se van con la tabla vieja, así que se rehacen
-- los tres y los tres. Son los mismos de V2 y V7, con `payload` añadido a la
-- lista de columnas inmutables: la geometría de un trazo tampoco se edita en
-- sitio, se da de baja y se crea otra.
CREATE INDEX idx_annotations_edition ON annotations(edition_id);
CREATE INDEX idx_annotations_supersedes ON annotations(supersedes);
CREATE INDEX idx_annotations_vivas ON annotations(edition_id) WHERE deleted_at IS NULL;

CREATE TRIGGER trg_annotations_updated_at AFTER UPDATE ON annotations
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE annotations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trg_annotations_inmutable
BEFORE UPDATE OF edition_id, kind, body, target, payload, created_at, supersedes ON annotations
BEGIN
    SELECT RAISE(ABORT,
        'una anotación es inmutable: editarla es darla de baja y crear otra');
END;

CREATE TRIGGER trg_annotations_baja_definitiva
BEFORE UPDATE OF deleted_at ON annotations
WHEN OLD.deleted_at IS NOT NULL AND NEW.deleted_at IS NULL
BEGIN
    SELECT RAISE(ABORT, 'una anotación dada de baja no se revive en sitio');
END;
