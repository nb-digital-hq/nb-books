-- V27: el texto deja de guardarse dentro de la base.
--
-- # Qué se quita y por qué
--
-- `text_chunks.text` guardaba una copia del texto de cada libro para que el
-- índice pudiera enseñar el pasaje de un resultado. Medido el 2026-09-10 sobre
-- EPUB reales, esa copia pesa el **52,1 % de lo que ocupan los documentos**, y es
-- el mayor de todos los gastos derivados: más que el índice invertido, que ronda
-- el 20 %, y más que las portadas, que rondan el 21 %.
--
-- El presupuesto son 20. Con la copia dentro no cabe ninguna combinación, así que
-- se quita, y el pasaje se relee del documento al enseñar el resultado. El lector
-- ya sabe hacer eso: es lo mismo que hace para pintar una página.
--
-- Lo que **no** se quita es el locator. Es lo que abre la posición exacta, y es
-- lo que hace que un resultado lleve a un sitio y no a un capítulo.
--
-- # Por qué el índice pasa a ser "sin contenido"
--
-- Porque un índice de contenido externo lee el texto de la tabla que lo alimenta
-- cuando hace falta -para enseñar un pasaje, o para rehacerse-, y esa tabla ya no
-- lo tiene. `content = ''` le dice a FTS5 que guarde solo el índice invertido y
-- que no espere poder volver al original.
--
-- `contentless_delete = 1` es lo que permite seguir borrando por identificador de
-- fila. Sin él, un índice sin contenido no sabe quitar lo que ya metió, y
-- reindexar un documento dejaría dentro los fragmentos viejos: la búsqueda
-- encontraría pasajes que ya no existen.
--
-- # El disparador de alta desaparece, y eso es una pérdida
--
-- V8 sincronizaba el índice por disparador, y decía por qué: un índice que el
-- código olvida actualizar produce búsquedas que no encuentran lo que está ahí, y
-- ese olvido no lo detecta ninguna prueba de la ruta feliz. Con el texto fuera de
-- la tabla, el disparador de alta ya no tiene de dónde sacarlo, así que la alta
-- pasa a hacerla `search::index_edition_con`, que es el único sitio que escribe
-- fragmentos. El de baja se queda, porque para borrar basta el identificador.
--
-- Se dice en vez de dejarlo pasar: es una garantía que baja de la base al código.
--
-- Contrato: docs/architecture/search.md.

DROP TRIGGER IF EXISTS trg_text_chunks_indexar;
DROP TRIGGER IF EXISTS trg_text_chunks_desindexar;
DROP TABLE IF EXISTS documents_fts;

-- Se vacía porque hay que volver a indexar de todas formas: el índice nuevo no
-- puede heredar nada del viejo.
DELETE FROM text_chunks;

ALTER TABLE text_chunks DROP COLUMN text;

CREATE VIRTUAL TABLE documents_fts USING fts5(
    text,
    content = '',
    contentless_delete = 1,
    tokenize = "unicode61 remove_diacritics 2"
);

-- Un `DELETE` normal y no el comando `'delete'` de FTS5: con
-- `contentless_delete = 1` el segundo se rechaza expresamente, porque la tabla
-- ya sabe borrar por identificador de fila sin que le recuerden el texto.
CREATE TRIGGER trg_text_chunks_desindexar AFTER DELETE ON text_chunks
BEGIN
    DELETE FROM documents_fts WHERE rowid = old.rowid;
END;

-- Y todo vuelve a la cola: lo que no tiene fragmentos se reencola solo.
UPDATE search_status SET state = 'pending', reason = NULL WHERE state <> 'not_searchable';

-- ---------------------------------------------------------------------------
-- La muestra que permite seguir comprobando el índice.
--
-- # Por qué hace falta una tabla para esto
--
-- Porque quitar el texto se lleva por delante la detección de corrupción, y eso
-- no se ve hasta que se mide. Un índice de contenido externo se comprueba
-- comparándolo con el texto del que salió: eso es lo que hace
-- `integrity-check`. Sin contenido no hay con qué comparar, y la comprobación
-- pasa a mirar solo que sus estructuras cuadren consigo mismas.
--
-- Medido el 2026-09-10 sobre SQLite 3.53.4, con seiscientos documentos
-- indexados y los ocho bloques del índice puestos a cero: `integrity-check`
-- **responde que todo está bien**, la búsqueda que devolvía las seiscientas
-- filas devuelve cero **sin dar error**, y la escritura siguiente funciona. Es
-- el peor de los casos posibles: pérdida silenciosa. No es que la detección
-- fuera difícil; es que había dejado de existir.
--
-- # Qué guarda, y por qué no es volver a guardar el texto
--
-- Una palabra por edición, y el identificador de fila donde tiene que
-- encontrarse. Esa palabra **ya está dentro del índice**: es justamente por eso
-- por lo que sirve para comprobarlo. No añade nada que no estuviera, solo lo
-- deja también fuera, donde la corrupción del índice no lo alcanza.
--
-- Cuesta unos veinte bytes por edición: sobre las 152.080 del corpus real, tres
-- megabytes contra los noventa y tres gigabytes de texto que esta migración
-- quita. Una muestra, no una copia.
--
-- Se guarda por edición y no por fragmento a propósito: por fragmento serían
-- unos cuarenta y ocho millones de filas sobre el corpus real -medio gigabyte-
-- para comprobar lo mismo. Ciento cincuenta mil sondas repartidas por todo el
-- índice ya son muchas más de las que se leen en una comprobación.
CREATE TABLE index_probes (
    edition_id  TEXT PRIMARY KEY REFERENCES editions(id) ON DELETE CASCADE,
    -- Sin acentos ni signos: la consulta pasa por el mismo tokenizador, así que
    -- lo que se guarda tiene que ser algo que el tokenizador devuelva igual.
    term        TEXT NOT NULL,
    chunk_rowid INTEGER NOT NULL
) STRICT;

-- La comprobación toma sondas repartidas por el rango de identificadores, que
-- es lo que las reparte por segmentos distintos del índice. Sin este índice esa
-- lectura sería un recorrido entero de la tabla.
CREATE INDEX idx_index_probes_rowid ON index_probes(chunk_rowid);
