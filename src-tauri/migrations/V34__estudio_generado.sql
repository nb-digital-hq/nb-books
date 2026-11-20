-- V34: lo que se guarda de las funciones de estudio.
--
-- # Solo se guarda lo que alguien decide guardar
--
-- Un resumen se genera, se lee y se va: no tiene tabla. Las tarjetas, las
-- preguntas y los conceptos se generan como propuesta en el Lector y **solo
-- entran aquí cuando la persona los acepta**, uno a uno o todos, después de poder
-- editarlos o descartarlos. Una base que guardara todo lo generado se llenaría de
-- lo que nadie quiso.
--
-- # La marca de generado no se puede perder
--
-- `provider` y `template` son obligatorios: dicen quién lo generó y con qué
-- instrucción, y es la regla 4 de `ai/philosophy.md` —toda salida generada es
-- identificable como tal— escrita en el esquema. Editar una tarjeta no la
-- convierte en propia: `edited_at` dice que se tocó, y la procedencia se queda.
--
-- # Las fuentes son locators, dentro de un objeto
--
-- `sources` es `{"locators": [...]}`: los fragmentos de los que sale, para abrir
-- el sitio desde Estudio. Un objeto y no la lista suelta porque así lo exige la
-- regla de anclaje (ADR-004), que comprueba `anclaje_de_posiciones.rs` sobre toda
-- tabla que ancla, y porque deja sitio para añadir campos sin migrar el valor. Sin
-- una sola columna numérica, por la misma regla: la versión de la plantilla va en
-- `template` como `estudio.tarjetas@1`, y la opción correcta, dentro de `choices`.

CREATE TABLE study_cards (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- `flashcard`: anverso y reverso. `question`: enunciado, opciones y explicación.
    kind       TEXT NOT NULL CHECK (kind IN ('flashcard', 'question')),
    front      TEXT NOT NULL,
    -- La respuesta de una tarjeta o la explicación de una pregunta.
    back       TEXT NOT NULL,
    -- `{"options": [...], "correct": n}` en una pregunta; nulo en una tarjeta.
    choices    TEXT,
    sources    TEXT NOT NULL,
    provider   TEXT NOT NULL,
    template   TEXT NOT NULL,
    edited_at  TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(front)) > 0),
    CHECK (kind = 'question' OR length(trim(back)) > 0),
    CHECK ((kind = 'question') = (choices IS NOT NULL)),
    CHECK (choices IS NULL OR (json_valid(choices) AND json_type(choices) = 'object')),
    CHECK (json_valid(sources) AND json_type(sources) = 'object'),
    CHECK (length(trim(provider)) > 0),
    CHECK (length(trim(template)) > 0)
) STRICT;

CREATE INDEX idx_study_cards_edition ON study_cards(edition_id, created_at);

CREATE TABLE concepts (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id  TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    name        TEXT NOT NULL COLLATE NOCASE,
    kind        TEXT NOT NULL
                CHECK (kind IN ('persona', 'lugar', 'obra', 'organizacion', 'fecha', 'concepto')),
    description TEXT NOT NULL DEFAULT '',
    sources     TEXT NOT NULL,
    provider    TEXT NOT NULL,
    template    TEXT NOT NULL,
    edited_at   TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(name)) > 0),
    CHECK (json_valid(sources) AND json_type(sources) = 'object'),
    CHECK (length(trim(provider)) > 0),
    CHECK (length(trim(template)) > 0),
    -- Un concepto es uno por libro: guardarlo otra vez junta sus fuentes.
    UNIQUE (edition_id, name)
) STRICT;

-- Es la base del grafo de conocimiento personal: el mismo nombre en dos libros es
-- la arista, y se busca por nombre.
CREATE INDEX idx_concepts_name ON concepts(name);
