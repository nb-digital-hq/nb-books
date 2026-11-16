-- V38: un autor, una fila.
--
-- # Qué pasaba
--
-- La importación daba de alta al autor de cada obra otra vez, aunque ya
-- existiera: en la biblioteca real, **207.282 filas de `contributors` para 61.187
-- nombres**. Las estanterías y la fila "porque estás leyendo" de Inicio lo
-- esquivaban agrupando por nombre, pero cada consulta que preguntara por un autor
-- de verdad -sus obras, su orden, lo que se busca de él- tropezaba con tantas
-- filas como libros.
--
-- # Qué es el mismo autor
--
-- Criterio: el mismo nombre **sin distinguir mayúsculas ni
-- tildes**. "Allan pease" y "Allan Pease", "Slavoj Zizek" y "Slavoj Žižek" son
-- uno. Dos personas homónimas quedan juntas, como en Calibre: sin otro dato no se
-- distinguen, y el único que existe para eso, el ORCID, queda fuera de la fusión.
--
-- # Por qué una columna generada y un índice único
--
-- Porque así **la base** impide la segunda fila, venga la escritura de donde
-- venga: la importación, una restauración o una migración futura. La clave es
-- SQL puro -`lower` y una cadena de `replace`- y no una función registrada desde
-- Rust, para que cualquier `sqlite3` pueda leer la tabla. La misma expresión vive
-- en `db::catalog::clave_de_nombre_sql`, y una prueba exige que sea la misma
-- cadena.
--
-- Contrato: docs/architecture/metadata-model.md.

ALTER TABLE contributors ADD COLUMN name_key TEXT
    GENERATED ALWAYS AS (lower(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(trim(name), '  ', ' '), '  ', ' '), 'á', 'a'), 'à', 'a'), 'â', 'a'), 'ä', 'a'), 'ã', 'a'), 'å', 'a'), 'ā', 'a'), 'ą', 'a'), 'ă', 'a'), 'Á', 'a'), 'À', 'a'), 'Â', 'a'), 'Ä', 'a'), 'Ã', 'a'), 'Å', 'a'), 'Ā', 'a'), 'Ą', 'a'), 'Ă', 'a'), 'é', 'e'), 'è', 'e'), 'ê', 'e'), 'ë', 'e'), 'ē', 'e'), 'ę', 'e'), 'ě', 'e'), 'ė', 'e'), 'É', 'e'), 'È', 'e'), 'Ê', 'e'), 'Ë', 'e'), 'Ē', 'e'), 'Ę', 'e'), 'Ě', 'e'), 'Ė', 'e'), 'í', 'i'), 'ì', 'i'), 'î', 'i'), 'ï', 'i'), 'ī', 'i'), 'į', 'i'), 'ı', 'i'), 'Í', 'i'), 'Ì', 'i'), 'Î', 'i'), 'Ï', 'i'), 'Ī', 'i'), 'Į', 'i'), 'İ', 'i'), 'ó', 'o'), 'ò', 'o'), 'ô', 'o'), 'ö', 'o'), 'õ', 'o'), 'ø', 'o'), 'ō', 'o'), 'ő', 'o'), 'Ó', 'o'), 'Ò', 'o'), 'Ô', 'o'), 'Ö', 'o'), 'Õ', 'o'), 'Ø', 'o'), 'Ō', 'o'), 'Ő', 'o'), 'ú', 'u'), 'ù', 'u'), 'û', 'u'), 'ü', 'u'), 'ū', 'u'), 'ů', 'u'), 'ű', 'u'), 'ų', 'u'), 'Ú', 'u'), 'Ù', 'u'), 'Û', 'u'), 'Ü', 'u'), 'Ū', 'u'), 'Ů', 'u'), 'Ű', 'u'), 'Ų', 'u'), 'ç', 'c'), 'ć', 'c'), 'č', 'c'), 'Ç', 'c'), 'Ć', 'c'), 'Č', 'c'), 'ñ', 'n'), 'ń', 'n'), 'ň', 'n'), 'Ñ', 'n'), 'Ń', 'n'), 'Ň', 'n'), 'ś', 's'), 'š', 's'), 'ş', 's'), 'ș', 's'), 'Ś', 's'), 'Š', 's'), 'Ş', 's'), 'Ș', 's'), 'ź', 'z'), 'ż', 'z'), 'ž', 'z'), 'Ź', 'z'), 'Ż', 'z'), 'Ž', 'z'), 'ý', 'y'), 'ÿ', 'y'), 'Ý', 'y'), 'Ÿ', 'y'), 'ł', 'l'), 'ľ', 'l'), 'ĺ', 'l'), 'Ł', 'l'), 'Ľ', 'l'), 'Ĺ', 'l'), 'ř', 'r'), 'ŕ', 'r'), 'Ř', 'r'), 'Ŕ', 'r'), 'ď', 'd'), 'đ', 'd'), 'Ď', 'd'), 'Đ', 'd'), 'ť', 't'), 'ţ', 't'), 'ț', 't'), 'Ť', 't'), 'Ţ', 't'), 'Ț', 't'), 'ğ', 'g'), 'Ğ', 'g'), 'ß', 'ss'), 'æ', 'ae'), 'Æ', 'ae'), 'œ', 'oe'), 'Œ', 'oe'))) VIRTUAL;

-- ---------------------------------------------------------------------------
-- Quién se queda: de cada grupo, el que más obras tiene; a igualdad, el más
-- antiguo. Es la forma escrita que más se ve, y la que menos cambia.
-- ---------------------------------------------------------------------------
CREATE TEMP TABLE fusion_de_autores AS
WITH usos AS (
    SELECT c.id, c.name, c.name_key, c.created_at,
           (SELECT count(*) FROM edition_contributors ec WHERE ec.contributor_id = c.id) AS obras
      FROM contributors c
     WHERE c.orcid IS NULL
),
elegidos AS (
    SELECT id, name,
           first_value(id) OVER grupo AS superviviente,
           first_value(name) OVER grupo AS nombre_superviviente
      FROM usos
    WINDOW grupo AS (PARTITION BY name_key ORDER BY obras DESC, created_at, id)
)
SELECT id AS duplicado, superviviente, name AS nombre, nombre_superviviente
  FROM elegidos
 WHERE id <> superviviente;

CREATE INDEX temp.idx_fusion_de_autores ON fusion_de_autores(duplicado);

-- Las ediciones cuyo autor cambia de forma escrita: su texto del catálogo de
-- búsqueda lleva el nombre, y no hay disparador de cambio que lo rehaga.
CREATE TEMP TABLE ediciones_con_autor_renombrado AS
SELECT DISTINCT ec.edition_id
  FROM edition_contributors ec
  JOIN fusion_de_autores f ON f.duplicado = ec.contributor_id
 WHERE f.nombre <> f.nombre_superviviente;

-- ---------------------------------------------------------------------------
-- Cada vínculo pasa al superviviente. Con `UPDATE` y no borrando e insertando:
-- los disparadores de cambio de autoría mueven la cuenta de la estantería de un
-- nombre al otro y rehacen `author_sort`, y ningún vínculo pierde su papel ni su
-- posición. Si la edición ya tenía al superviviente con ese papel, el vínculo
-- repetido sobra y se borra, que también descuenta su estantería.
-- ---------------------------------------------------------------------------
UPDATE OR IGNORE edition_contributors
   SET contributor_id = (SELECT f.superviviente FROM fusion_de_autores f
                          WHERE f.duplicado = edition_contributors.contributor_id)
 WHERE contributor_id IN (SELECT duplicado FROM fusion_de_autores);

DELETE FROM edition_contributors
 WHERE contributor_id IN (SELECT duplicado FROM fusion_de_autores);

DELETE FROM contributors
 WHERE id IN (SELECT duplicado FROM fusion_de_autores);

-- El catálogo de búsqueda, rehecho para las ediciones que cambiaron de nombre de
-- autor, con la misma forma que le dan sus disparadores.
DELETE FROM catalog_fts
 WHERE edition_id IN (SELECT edition_id FROM ediciones_con_autor_renombrado);

INSERT INTO catalog_fts (edition_id, title, authors, subjects)
SELECT e.id,
       coalesce(e.title, '') || ' ' || coalesce(e.subtitle, '')
           || ' ' || coalesce(e.series_title, ''),
       coalesce((SELECT group_concat(c.name, ' ')
                   FROM edition_contributors ec
                   JOIN contributors c ON c.id = ec.contributor_id
                  WHERE ec.edition_id = e.id), ''),
       coalesce((SELECT group_concat(s.label, ' ')
                   FROM edition_subjects es
                   JOIN subjects s ON s.id = es.subject_id
                  WHERE es.edition_id = e.id), '')
  FROM editions e
 WHERE e.id IN (SELECT edition_id FROM ediciones_con_autor_renombrado);

DROP TABLE ediciones_con_autor_renombrado;
DROP TABLE fusion_de_autores;

-- ---------------------------------------------------------------------------
-- Y desde aquí, una sola fila por autor. El ORCID sigue siendo único por su
-- propio índice, y quien lo tiene no choca con un homónimo que no.
-- ---------------------------------------------------------------------------
CREATE UNIQUE INDEX idx_contributors_name_key
    ON contributors(name_key) WHERE orcid IS NULL;
