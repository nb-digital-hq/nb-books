-- V35: el audio preparado de un capítulo o de un libro.
--
-- # Por qué hay una tabla y no solo archivos
--
-- Porque el archivo solo dice cómo suena, y hacen falta tres cosas más que no
-- caben dentro de él: **con qué voz y con qué versión del modelo se hizo** —para
-- no mezclar dos voces en un mismo libro y para saber qué habría que rehacer si
-- el modelo cambia—, **cuánto ocupa** —porque cuenta contra el tope de lo
-- derivado, y un tope que no sabe qué hay no es un tope— y **por dónde va el
-- texto mientras suena**, que es el criterio de seguimiento visual.
--
-- # Las marcas anclan con Locator, como todo lo demás
--
-- `anchor` es el locator serializado del fragmento que se está diciendo, dentro
-- de un objeto de selectores: ADR-004 y la prueba `anclaje_de_posiciones.rs`. Los
-- milisegundos **no son una posición de lectura**: son el tiempo dentro del
-- archivo de audio, que es una coordenada del sonido y no del documento. Por eso
-- son números y por eso están declarados como admitidos en esa prueba.
--
-- # Por qué la ruta es relativa
--
-- Porque la carpeta de datos se muda -`portability.rs` exporta e importa la
-- biblioteca entera- y una ruta absoluta dentro de la base convierte cada mudanza
-- en una migración de datos. Se guarda el nombre y quien lee compone la ruta con
-- la carpeta de esta instalación, igual que hacen las portadas.

CREATE TABLE audio_sections (
    id            TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id    TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Qué parte del libro: los recursos del capítulo unidos por `|`, o vacío
    -- cuando es el libro entero.
    scope_key     TEXT NOT NULL,
    -- Cómo se llama lo preparado para quien lo ve en la biblioteca.
    title         TEXT NOT NULL DEFAULT '',
    voice         TEXT NOT NULL,
    -- La revisión del modelo con que se hizo. Cambiar de modelo no borra nada:
    -- deja dicho con cuál está hecho lo que hay.
    model_rev     TEXT NOT NULL,
    -- Nombre del archivo dentro de la carpeta de audio de esta instalación.
    file_name     TEXT NOT NULL,
    bytes         INTEGER NOT NULL,
    duration_ms   INTEGER NOT NULL,
    -- Cuántos fragmentos se dijeron. Sirve para saber si quedó a medias sin
    -- recorrer las marcas.
    fragments     INTEGER NOT NULL DEFAULT 0,
    -- `ready` cuando está entero; `partial` cuando se canceló y lo hecho sirve.
    status        TEXT NOT NULL DEFAULT 'ready' CHECK (status IN ('ready', 'partial')),
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(voice)) > 0),
    CHECK (length(trim(model_rev)) > 0),
    CHECK (length(trim(file_name)) > 0),
    CHECK (bytes >= 0),
    CHECK (duration_ms >= 0),
    -- Una sola preparación por parte, voz y versión del modelo: preparar otra vez
    -- lo mismo sustituye, no acumula.
    UNIQUE (edition_id, scope_key, voice, model_rev)
) STRICT;

-- Lo que la biblioteca pregunta: qué libros están preparados para escuchar.
CREATE INDEX idx_audio_sections_edition ON audio_sections(edition_id, created_at);

CREATE TABLE audio_marks (
    section_id TEXT NOT NULL REFERENCES audio_sections(id) ON DELETE CASCADE,
    -- En qué orden se dicen. No es una posición del documento: es el orden dentro
    -- del audio, y por eso es un número.
    ordinal    INTEGER NOT NULL,
    start_ms   INTEGER NOT NULL,
    end_ms     INTEGER NOT NULL,
    -- El locator del fragmento, dentro de un objeto: ADR-004.
    anchor     TEXT NOT NULL,

    PRIMARY KEY (section_id, ordinal),
    CHECK (start_ms >= 0 AND end_ms >= start_ms),
    CHECK (json_valid(anchor) AND json_type(anchor) = 'object')
) STRICT;
