-- V15: el vocabulario consultado.
--
-- # Qué guarda, y por qué sale gratis
--
-- Cada consulta de diccionario que **encuentra algo** deja aquí la palabra, el
-- documento donde apareció y el punto exacto. No hay nada que calcular ni que
-- pedirle al usuario: es apuntar lo que ya ocurrió. Ese es el motivo de que esta
-- el vocabulario esté en el núcleo y no junto a las tarjetas de estudio:
-- las tarjetas necesitan que exista esta lista; esta lista no necesita las
-- tarjetas.
--
-- # Por qué se guardan dos formas de la palabra
--
-- `term` es lo que el usuario seleccionó y `headword` es el lema con el que
-- respondió el diccionario. Casi nunca coinciden —se selecciona "corriendo" y se
-- encuentra "correr"—, y las dos hacen falta: el lema es lo que se estudia, y la
-- forma seleccionada es lo que permite volver a encontrarla en el texto.
--
-- # Por qué ancla, y por eso guarda un Locator
--
-- El criterio de aceptación dice que desde cada entrada se salta al
-- punto exacto donde apareció la palabra. Eso es anclar, así que se aplica
-- ADR-004 entera: la posición es un Locator serializado y la base rechaza
-- cualquier otra cosa. `tests/anclaje_de_posiciones.rs` lo comprueba sobre el
-- esquema vivo, no leyendo este archivo.
--
-- # Por qué cuelga de la edición y no del archivo
--
-- El mismo motivo que el progreso en la V6: el archivo puede moverse, volver a
-- importarse o existir en dos copias, y en los tres casos la palabra se consultó
-- en la misma obra. La edición es lo que no cambia.
--
-- # Por qué la misma palabra puede aparecer varias veces
--
-- Porque consultarla dos veces en dos libros distintos son dos hechos distintos,
-- y el segundo es justamente el que dice que la palabra cuesta. Un `UNIQUE` por
-- lema convertiría el vocabulario en un conjunto y perdería esa información. Lo
-- que sí se evita es la repetición inmediata: consultar dos veces seguidas la
-- misma palabra en el mismo sitio es un rebote, no un dato, y lo descarta
-- `vocabulary::record`.
--
-- Contrato: docs/architecture/api/schemas.md.

CREATE TABLE vocabulary (
    id          TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- El lema con el que respondió el diccionario.
    headword    TEXT NOT NULL,
    -- La forma que el usuario seleccionó en el texto.
    term        TEXT NOT NULL,
    edition_id  TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Quién respondió. Sirve para la procedencia, igual que en las anotaciones:
    -- una palabra encontrada en un diccionario bilingüe no es lo mismo que una
    -- encontrada en uno monolingüe.
    provider_id TEXT NOT NULL,
    -- El Locator, serializado. Nunca un entero: ver ADR-004.
    locator     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(headword)) > 0),
    CHECK (length(trim(term)) > 0),
    CHECK (json_valid(locator) AND json_type(locator) = 'object')
) STRICT;

-- Se lista de lo más reciente a lo más antiguo, que es como lo mira una persona,
-- y se filtra por obra al abrir un documento.
CREATE INDEX idx_vocabulary_created ON vocabulary(created_at DESC);
CREATE INDEX idx_vocabulary_edition ON vocabulary(edition_id);
