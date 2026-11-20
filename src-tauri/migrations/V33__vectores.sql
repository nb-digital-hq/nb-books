-- V33: los vectores de los fragmentos, y qué no se vectoriza.
--
-- # Por qué una tabla y no una columna de `text_chunks`
--
-- ADR-007 decidió los vectores "en una columna de la base que ya existe, junto al
-- fragmento y su localizador", y lo que importa de esa frase se cumple: van en la
-- misma base, cuantizados a ocho bits y atados al fragmento. Van en una tabla
-- aparte por tres razones que la frase no podía prever:
--
-- * **Casi ningún fragmento tiene vector.** El alcance por defecto es el documento
--   abierto: unos cientos de vectores frente a millones de fragmentos léxicos.
--   Una columna vacía en cada fila de `text_chunks` no ocupa casi nada, pero no
--   dice de qué modelo es cada vector, y sin eso cambiar de modelo mezclaría
--   vectores que no se pueden comparar.
-- * **Vaciar el índice es borrar una tabla**, que es lo que ADR-007 pide que se
--   pueda hacer sin miedo: es caché.
-- * **Un vector no puede sobrevivir a su fragmento.** La clave ajena con borrado en
--   cascada hace que reindexar una edición —que borra y rehace sus fragmentos— se
--   lleve sus vectores en la misma transacción, sin que el código tenga que
--   acordarse.
--
-- # Y por qué las exclusiones van aquí también
--
-- Son, que va en la misma subfase. Cada migración copia la base entera
-- antes de aplicarse, y en la instalación real son gigas: dos
-- migraciones seguidas son dos copias.

CREATE TABLE chunk_vectors (
    chunk_id   TEXT PRIMARY KEY REFERENCES text_chunks(id) ON DELETE CASCADE,
    -- Repetida del fragmento para poder buscar por edición sin cruzar con
    -- `text_chunks`, que tiene millones de filas.
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- El identificador del modelo del catálogo que lo calculó. Un vector solo se
    -- compara con consultas del mismo modelo.
    model      TEXT NOT NULL,
    -- Lo que hay que multiplicar para volver del entero de ocho bits al valor.
    scale      REAL NOT NULL,
    -- Un byte con signo por dimensión.
    vector     BLOB NOT NULL,

    CHECK (scale > 0),
    CHECK (length(vector) > 0)
) STRICT;

CREATE INDEX idx_chunk_vectors_edition ON chunk_vectors(edition_id, model);
-- Sin índice por modelo solo: buscar en todo lo vectorizado lee el vector de cada
-- fila de todos modos, así que recorrer la tabla cuesta lo mismo, y el índice
-- ocupaba 28 bytes más por vector: medido, 648 con él y 620 sin él.

-- Lo que el usuario no quiere que se vectorice nunca: una edición, o
-- todas las de una colección. Dos tablas y no una con un campo de tipo, para que
-- cada una tenga su clave ajena y desaparezca con lo que excluye.
CREATE TABLE vector_excluded_editions (
    edition_id TEXT PRIMARY KEY REFERENCES editions(id) ON DELETE CASCADE,
    excluded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE vector_excluded_collections (
    collection_id TEXT PRIMARY KEY REFERENCES collections(id) ON DELETE CASCADE,
    excluded_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
