-- V2: núcleo bibliográfico. Obra, edición y archivo como entidades distintas.
--
-- Es el esquema más caro de cambiar del proyecto.
--
-- La distinción entre los tres niveles no es
-- purismo: sin ella no se puede representar "tengo tres ediciones del mismo
-- libro" ni fusionar dos bibliotecas sin perder información.
--
--   work  1---N  edition  1---N  file
--                    |
--                    +---N  annotation
--
-- Reglas que gobiernan este archivo:
--   docs/architecture/api/schemas.md                                  (esquema objetivo)
--   docs/architecture/metadata-model.md      (por qué de cada campo)
--   docs/architecture/migrations.md          (cómo evoluciona)
--
-- Convenciones de esta migración y de las siguientes:
--
--   * Identificadores: TEXT de 32 caracteres hexadecimales, 128 bits de
--     aleatoriedad, generados por la propia base con `randomblob`. No son
--     enteros autoincrementales porque tienen que sobrevivir a la exportación,
-- la reimportación y la fusión de bibliotecas entre dispositivos; un entero local obliga a reasignar claves en cada fusión.
--     No se usa un UUID con formato canónico para no añadir una dependencia
--     por 36 caracteres.
--   * Marcas de tiempo: TEXT en ISO-8601 UTC con `T` y `Z`, ordenable como
--     cadena. La V1 usó `datetime('now')` para `app_metadata`; una migración
--     publicada no se edita, así que esa tabla conserva su formato y a partir
--     de aquí se usa el explícito.
--   * `STRICT` en todas las tablas: sin él, SQLite acepta una cadena en una
--     columna declarada INTEGER y el error aparece meses después.

-- ---------------------------------------------------------------------------
-- Obra: el concepto, independiente de cualquier publicación concreta.
-- ---------------------------------------------------------------------------
CREATE TABLE works (
    id                TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    canonical_title   TEXT NOT NULL,
    -- Etiqueta BCP-47 del idioma en que se escribió la obra, no el de esta
    -- edición: una traducción es una edición de la misma obra.
    original_language TEXT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(canonical_title)) > 0)
) STRICT;

-- ---------------------------------------------------------------------------
-- Edición: una publicación concreta de una obra.
-- ---------------------------------------------------------------------------
CREATE TABLE editions (
    id                TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    work_id           TEXT NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    -- El título de la edición puede diferir del canónico de la obra: una
    -- traducción y una reedición conmemorativa se titulan distinto.
    title             TEXT NOT NULL,
    subtitle          TEXT,
    publisher         TEXT,
    -- Fecha parcial permitida: muchos catálogos solo conocen el año. Guardar
    -- una fecha completa inventada es peor que guardar la que se sabe.
    published_date    TEXT,
    -- Mención de edición tal y como aparece en la publicación ("2a edicion").
    edition_statement TEXT,
    language          TEXT,
    series_title      TEXT,
    -- Real y no entero: las series numeran con intercalados (1.5) más a menudo
    -- de lo que parece, sobre todo en cómic y en novela por entregas.
    series_index      REAL,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(title)) > 0),
    CHECK (
        published_date IS NULL
        OR published_date GLOB '[0-9][0-9][0-9][0-9]'
        OR published_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]'
        OR published_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
    )
) STRICT;

CREATE INDEX idx_editions_work ON editions(work_id);

-- ---------------------------------------------------------------------------
-- Identificadores externos de una edición.
--
-- Tabla propia y no columnas `isbn`/`doi` porque una misma edición tiene a la
-- vez ISBN-10 e ISBN-13, y a veces DOI. Regla 4 de docs/architecture/api/schemas.md: los
-- identificadores externos no son clave primaria ni son fiables como identidad.
-- ---------------------------------------------------------------------------
CREATE TABLE edition_identifiers (
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Lista cerrada y explícita: un esquema nuevo es una migración, no un
    -- valor libre que nadie sabe interpretar después.
    scheme     TEXT NOT NULL CHECK (scheme IN ('isbn', 'issn', 'doi', 'asin', 'oclc', 'local')),
    value      TEXT NOT NULL,

    PRIMARY KEY (edition_id, scheme, value),
    CHECK (length(trim(value)) > 0)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_edition_identifiers_lookup ON edition_identifiers(scheme, value);

-- ---------------------------------------------------------------------------
-- Archivo: el documento en disco.
--
-- Regla 1 de docs/architecture/api/schemas.md: los archivos no se guardan como BLOB. Aquí
-- viven la ruta y el hash; los bytes viven en el sistema de archivos.
-- ---------------------------------------------------------------------------
CREATE TABLE files (
    id           TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id   TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Absoluta en modo referenciado; relativa a la carpeta de biblioteca en
    -- modo administrado. Quién decide cuál es cada caso lo resuelve la importación.
    path         TEXT NOT NULL UNIQUE,
    -- Identidad por contenido. El cálculo en flujo lo hace `library.rs`; la columna y su
    -- índice están aquí porque añadirlos después obliga a rehashear la
    -- biblioteca entera del usuario.
    content_hash TEXT NOT NULL,
    byte_size    INTEGER NOT NULL,
    -- Tipo de medio detectado por contenido, nunca por extensión.
    media_type   TEXT NOT NULL,
    storage_mode TEXT NOT NULL CHECK (storage_mode IN ('managed', 'referenced')),
    -- Un archivo en un volumen desconectado no se borra del catálogo: se marca.
    -- Perder los metadatos y las anotaciones por desenchufar un disco externo
    -- seria exactamente la clase de pérdida que esta fase existe para impedir.
    availability TEXT NOT NULL DEFAULT 'available'
                 CHECK (availability IN ('available', 'missing', 'unreadable')),
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (byte_size >= 0),
    CHECK (length(trim(path)) > 0),
    CHECK (length(trim(content_hash)) > 0),
    CHECK (length(trim(media_type)) > 0)
) STRICT;

CREATE INDEX idx_files_edition ON files(edition_id);
-- No es UNIQUE a propósito: dos archivos idénticos pueden coexistir
-- legítimamente mientras se migra de modo referenciado a administrado. La
-- política de duplicados la decide la importación sobre este índice, no el esquema.
CREATE INDEX idx_files_content_hash ON files(content_hash);

-- ---------------------------------------------------------------------------
-- Personas y entidades que contribuyen a una edición.
-- ---------------------------------------------------------------------------
CREATE TABLE contributors (
    id        TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- Forma de presentación: "Gabriel Garcia Marquez".
    name      TEXT NOT NULL,
    -- Forma de ordenación: "Garcia Marquez, Gabriel". Separada porque
    -- derivarla del nombre falla en cuanto hay dos apellidos o una particula.
    sort_name TEXT,
    -- Unico identificador externo con columna propia, a diferencia de las
    -- ediciones: una persona tiene como mucho un ORCID, mientras que una
    -- edición tiene varios identificadores a la vez.
    orcid     TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(name)) > 0),
    CHECK (orcid IS NULL OR orcid GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9][0-9][0-9]-[0-9][0-9][0-9][0-9]-[0-9][0-9][0-9][0-9X]')
) STRICT;

CREATE UNIQUE INDEX idx_contributors_orcid ON contributors(orcid) WHERE orcid IS NOT NULL;

CREATE TABLE edition_contributors (
    edition_id     TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    contributor_id TEXT NOT NULL REFERENCES contributors(id) ON DELETE CASCADE,
    -- Lista cerrada: un rol nuevo se añade por migración, con su significado
    -- documentado, no escribiendo una cadena distinta en cada importación.
    role           TEXT NOT NULL CHECK (role IN ('author', 'editor', 'translator', 'illustrator')),
    -- Orden de aparición dentro del rol. Una obra con tres autores tiene un
    -- orden que la cita bibliográfica debe respetar.
    position       INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (edition_id, contributor_id, role),
    CHECK (position >= 0)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_edition_contributors_contributor ON edition_contributors(contributor_id);

-- ---------------------------------------------------------------------------
-- Materias y clasificación.
-- ---------------------------------------------------------------------------
CREATE TABLE subjects (
    id     TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- El esquema de clasificación al que pertenece la etiqueta. `local` es la
    -- materia que introduce el usuario y no viene de ningún vocabulario.
    scheme TEXT NOT NULL DEFAULT 'local',
    label  TEXT NOT NULL,

    UNIQUE (scheme, label),
    CHECK (length(trim(label)) > 0),
    CHECK (length(trim(scheme)) > 0)
) STRICT;

CREATE TABLE edition_subjects (
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    subject_id TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,

    PRIMARY KEY (edition_id, subject_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_edition_subjects_subject ON edition_subjects(subject_id);

-- ---------------------------------------------------------------------------
-- Anotación.
--
-- Se ancla a la edición, no al archivo: cambiar el archivo de sitio, o
-- sustituirlo por otra copia del mismo texto, no puede desanclar las notas.
--
-- `target` guarda los selectores serializados. La forma exacta la fija el Locator
-- (ADR-004) y la inmutabilidad de la anotación la impone V7;
-- esta migración crea la tabla y la restricción que impide la regresión que
-- ADR-004 existe para evitar.
-- ---------------------------------------------------------------------------
CREATE TABLE annotations (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL CHECK (kind IN ('highlight', 'note', 'bookmark')),
    -- Texto de la nota. Nulo en un resaltado sin comentario.
    body       TEXT,
    -- Selectores serializados como objeto JSON. La comprobación no es
    -- decorativa: `json_type(...) = 'object'` rechaza en la propia base un
    -- entero de progreso, que es exactamente el modelo que ADR-004 descarta y
    -- que el esquema anterior del prototipo usaba.
    target     TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (json_valid(target) AND json_type(target) = 'object')
) STRICT;

CREATE INDEX idx_annotations_edition ON annotations(edition_id);

-- ---------------------------------------------------------------------------
-- Mantenimiento de `updated_at`.
--
-- Por disparador y no desde el código de Rust: una marca de tiempo que el
-- código olvida actualizar produce un dato en el que después no se puede
-- confiar, y el olvido no lo detecta ninguna prueba.
--
-- La guarda `WHEN NEW.updated_at = OLD.updated_at` cumple dos funciones: impide
-- la recursión si alguien activa `recursive_triggers`, y deja pasar un
-- `updated_at` fijado a propósito, que es lo que necesita una importación o una
-- restauración desde copia de seguridad para conservar la marca original.
-- ---------------------------------------------------------------------------
CREATE TRIGGER trg_works_updated_at AFTER UPDATE ON works
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE works SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trg_editions_updated_at AFTER UPDATE ON editions
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE editions SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trg_files_updated_at AFTER UPDATE ON files
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE files SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trg_contributors_updated_at AFTER UPDATE ON contributors
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE contributors SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

CREATE TRIGGER trg_annotations_updated_at AFTER UPDATE ON annotations
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE annotations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
