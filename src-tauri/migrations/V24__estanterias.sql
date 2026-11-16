-- V24: el recuento de cada estantería, materializado.
--
-- # Qué estaba pasando
--
-- Abrir el panel de autores congelaba la ventana. Medido el 2026-09-10 sobre la
-- biblioteca real -151.411 ediciones, 55.740 autores distintos-, pidiendo las
-- veinte estanterías mayores:
--
--   autor    4.141 ms   SCAN + SEARCH + TEMP B-TREE para GROUP BY y para ORDER BY
--   serie      330 ms   SCAN + TEMP B-TREE para GROUP BY y para ORDER BY
--   materia    994 ms   SCAN + SEARCH + TEMP B-TREE para ORDER BY
--
-- Contra un presupuesto de fotograma de 16 ms. Y son la mitad del coste: el
-- panel pide además `count(*)` sobre la misma agrupación para saber cuántas hay,
-- así que el trabajo se hace dos veces.
--
-- El `LIMIT 20` recorta el resultado, **no el trabajo**: para saber cuáles son
-- las veinte mayores hay que contar las 55.740. Es exactamente el caso de
-- `author_sort` en V19 con otra forma, y la salida es la misma que allí: no otro
-- índice, sino materializar lo que se necesita ordenar. Un índice no puede
-- evitar un GROUP BY.
--
-- # Qué se materializa
--
-- Una fila por estantería, con su recuento al día. La clave de agrupación es la
-- que ya usaba la consulta y **no se cambia**: el autor agrupa por nombre -dos
-- filas de `contributors` con el mismo nombre son una sola estantería, que es lo
-- que quiere quien mira- y la materia por identificador, porque su etiqueta
-- puede cambiar sin que la materia sea otra.
--
-- # La baja cuelga de la cascada, y eso se comprobó en lugar de suponerse
--
-- `edition_contributors` y `edition_subjects` cuelgan de `editions` con
-- ON DELETE CASCADE. La documentación de SQLite dice que una acción en cascada
-- dispara los disparadores **solo** si `recursive_triggers` está activo, y aquí
-- no lo está. Con eso, borrar una obra habría dejado los recuentos intactos, así
-- que la primera versión de esta migración restaba desde arriba, con un BEFORE
-- DELETE sobre `editions`.
--
-- Se midió: en SQLite 3.53.4, que es la que se compila aquí, **la cascada sí los
-- dispara**. El resultado fue que se restaba dos veces y el recuento se iba a
-- cero. La afirmación de la documentación no se cumple en esta versión, así que
-- la que manda es la medida y los disparadores de baja viven donde tienen que
-- vivir: sobre las tablas de enlace.
--
-- Lo que la comprobación de una dependencia no puede hacer es quedarse en un
-- comentario: `tests/estanterias.rs` borra una obra y comprueba que sus
-- estanterías bajan de dos a uno, y `tests/recorrido_biblioteca.rs` comprueba
-- que la premisa siga siendo cierta. Si una versión futura de SQLite deja de
-- disparar en cascada, eso falla y dice por qué, en lugar de dejar el panel
-- diciendo que un autor tiene doce libros cuando le quedan once.
--
-- Contrato: docs/architecture/data-flows.md.

CREATE TABLE shelf_counts (
    -- 'author', 'series' o 'subject'. Lista cerrada: una clase nueva se añade
    -- por migración, como los roles de V2.
    kind      TEXT NOT NULL CHECK (kind IN ('author', 'series', 'subject')),
    -- Lo que identifica la estantería. Nombre para el autor y la serie,
    -- identificador para la materia.
    shelf_key TEXT NOT NULL,
    -- Lo que se enseña. Coincide con la clave salvo en las materias.
    label     TEXT NOT NULL,
    -- Cuántas ediciones hay dentro. No se llama `count` para no chocar con la
    -- función del mismo nombre dentro de los propios disparadores.
    tally     INTEGER NOT NULL,

    PRIMARY KEY (kind, shelf_key),
    CHECK (tally >= 0)
) STRICT, WITHOUT ROWID;

-- De cobertura y en el orden exacto en el que se pide: clase, tamaño
-- descendente, etiqueta y clave. Con él, la consulta del panel no ordena nada y
-- no toca la tabla; sin él, vuelve el árbol temporal por otro camino.
CREATE INDEX idx_shelf_counts_orden ON shelf_counts(kind, tally DESC, label, shelf_key);

-- ---------------------------------------------------------------------------
-- El valor de partida. Son las tres consultas que hacía el panel, una sola vez.
-- ---------------------------------------------------------------------------

INSERT INTO shelf_counts (kind, shelf_key, label, tally)
SELECT 'author', c.name, c.name, count(*)
  FROM edition_contributors ec
  JOIN contributors c ON c.id = ec.contributor_id
 WHERE ec.role = 'author'
 GROUP BY c.name;

INSERT INTO shelf_counts (kind, shelf_key, label, tally)
SELECT 'series', e.series_title, e.series_title, count(*)
  FROM editions e
 WHERE e.series_title IS NOT NULL AND trim(e.series_title) <> ''
 GROUP BY e.series_title;

INSERT INTO shelf_counts (kind, shelf_key, label, tally)
SELECT 'subject', s.id, s.label, count(*)
  FROM edition_subjects es
  JOIN subjects s ON s.id = es.subject_id
 GROUP BY s.id, s.label;

-- ---------------------------------------------------------------------------
-- Autores.
-- ---------------------------------------------------------------------------

-- El alta es el caso caliente: una importación pasa por aquí una vez por
-- documento. Por eso suma uno en lugar de recontar, que sería volver a hacer
-- justamente lo que esta migración existe para no hacer.
CREATE TRIGGER estanteria_al_anadir_autoria
AFTER INSERT ON edition_contributors
WHEN NEW.role = 'author'
BEGIN
    INSERT INTO shelf_counts (kind, shelf_key, label, tally)
    SELECT 'author', c.name, c.name, 1
      FROM contributors c WHERE c.id = NEW.contributor_id
    ON CONFLICT (kind, shelf_key) DO UPDATE SET tally = tally + 1;
END;

CREATE TRIGGER estanteria_al_quitar_autoria
AFTER DELETE ON edition_contributors
WHEN OLD.role = 'author'
BEGIN
    UPDATE shelf_counts SET tally = tally - 1
     WHERE kind = 'author'
       AND shelf_key = (SELECT c.name FROM contributors c WHERE c.id = OLD.contributor_id)
       AND tally > 0;
    DELETE FROM shelf_counts WHERE kind = 'author' AND tally = 0;
END;

-- Cambiar el rol o la persona es una baja y un alta a la vez. Es raro -no ocurre
-- durante una importación- y se escribe entero para que no pueda equivocarse.
CREATE TRIGGER estanteria_al_cambiar_autoria
AFTER UPDATE OF role, contributor_id ON edition_contributors
BEGIN
    UPDATE shelf_counts SET tally = tally - 1
     WHERE OLD.role = 'author'
       AND kind = 'author'
       AND shelf_key = (SELECT c.name FROM contributors c WHERE c.id = OLD.contributor_id)
       AND tally > 0;

    INSERT INTO shelf_counts (kind, shelf_key, label, tally)
    SELECT 'author', c.name, c.name, 1
      FROM contributors c
     WHERE NEW.role = 'author' AND c.id = NEW.contributor_id
    ON CONFLICT (kind, shelf_key) DO UPDATE SET tally = tally + 1;

    DELETE FROM shelf_counts WHERE kind = 'author' AND tally = 0;
END;

-- Renombrar a alguien puede **fundir** dos estanterías en una, así que aquí no
-- vale sumar y restar: se recuentan las dos afectadas, la que pierde y la que
-- gana. Ocurre al corregir metadatos, de una en una.
CREATE TRIGGER estanteria_al_renombrar_autor
AFTER UPDATE OF name ON contributors
WHEN NEW.name <> OLD.name
BEGIN
    DELETE FROM shelf_counts
     WHERE kind = 'author' AND shelf_key IN (OLD.name, NEW.name);

    INSERT INTO shelf_counts (kind, shelf_key, label, tally)
    SELECT 'author', c.name, c.name, count(*)
      FROM edition_contributors ec
      JOIN contributors c ON c.id = ec.contributor_id
     WHERE ec.role = 'author' AND c.name IN (OLD.name, NEW.name)
     GROUP BY c.name;
END;

-- ---------------------------------------------------------------------------
-- Series. Viven en la propia edición, así que no hay cascada que esquivar.
-- ---------------------------------------------------------------------------

CREATE TRIGGER estanteria_al_anadir_serie
AFTER INSERT ON editions
WHEN NEW.series_title IS NOT NULL AND trim(NEW.series_title) <> ''
BEGIN
    INSERT INTO shelf_counts (kind, shelf_key, label, tally)
    VALUES ('series', NEW.series_title, NEW.series_title, 1)
    ON CONFLICT (kind, shelf_key) DO UPDATE SET tally = tally + 1;
END;

CREATE TRIGGER estanteria_al_cambiar_serie
AFTER UPDATE OF series_title ON editions
WHEN NEW.series_title IS NOT OLD.series_title
BEGIN
    UPDATE shelf_counts SET tally = tally - 1
     WHERE kind = 'series'
       AND OLD.series_title IS NOT NULL AND trim(OLD.series_title) <> ''
       AND shelf_key = OLD.series_title
       AND tally > 0;

    INSERT INTO shelf_counts (kind, shelf_key, label, tally)
    SELECT 'series', NEW.series_title, NEW.series_title, 1
     WHERE NEW.series_title IS NOT NULL AND trim(NEW.series_title) <> ''
    ON CONFLICT (kind, shelf_key) DO UPDATE SET tally = tally + 1;

    DELETE FROM shelf_counts WHERE kind = 'series' AND tally = 0;
END;

-- ---------------------------------------------------------------------------
-- Materias.
-- ---------------------------------------------------------------------------

CREATE TRIGGER estanteria_al_anadir_materia
AFTER INSERT ON edition_subjects
BEGIN
    INSERT INTO shelf_counts (kind, shelf_key, label, tally)
    SELECT 'subject', s.id, s.label, 1
      FROM subjects s WHERE s.id = NEW.subject_id
    ON CONFLICT (kind, shelf_key) DO UPDATE SET tally = tally + 1;
END;

CREATE TRIGGER estanteria_al_quitar_materia
AFTER DELETE ON edition_subjects
BEGIN
    UPDATE shelf_counts SET tally = tally - 1
     WHERE kind = 'subject' AND shelf_key = OLD.subject_id AND tally > 0;
    DELETE FROM shelf_counts WHERE kind = 'subject' AND tally = 0;
END;

-- La etiqueta cambia sin que la materia sea otra: se actualiza lo que se enseña
-- y el recuento se queda como está, que es la ventaja de que la clave sea el
-- identificador y no el texto.
CREATE TRIGGER estanteria_al_renombrar_materia
AFTER UPDATE OF label ON subjects
WHEN NEW.label <> OLD.label
BEGIN
    UPDATE shelf_counts SET label = NEW.label
     WHERE kind = 'subject' AND shelf_key = NEW.id;
END;

-- La serie es lo único de una obra que no cuelga de ninguna tabla de enlace, así
-- que su baja no la cubre ninguna cascada: va aquí. Lo de los autores y las
-- materias lo restan sus propios disparadores cuando la cascada se lleva la
-- fila de enlace.
CREATE TRIGGER estanteria_al_borrar_edicion
AFTER DELETE ON editions
WHEN OLD.series_title IS NOT NULL AND trim(OLD.series_title) <> ''
BEGIN
    UPDATE shelf_counts SET tally = tally - 1
     WHERE kind = 'series' AND shelf_key = OLD.series_title AND tally > 0;
    DELETE FROM shelf_counts WHERE kind = 'series' AND tally = 0;
END;
