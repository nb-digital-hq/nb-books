-- V10: búsqueda de catálogo.
--
-- Buscar en el texto de los libros y buscar **un** libro son dos cosas
-- distintas. La primera recorre gigabytes y devuelve párrafos; la segunda
-- recorre unos cientos de miles de filas cortas y devuelve documentos, y tiene
-- que responder mientras se escribe (`performance-budgets.md`: menos de 50 ms
-- sobre mil libros).
--
-- Por eso son dos índices y no uno. Meter el título en el mismo índice que el
-- texto haría que buscar "Cien años de soledad" devolviera doscientos párrafos
-- que mencionan la soledad antes que el libro.
--
-- # Por qué se mantiene con disparadores
--
-- Porque la alternativa es acordarse. Un índice de catálogo que el código olvida
-- actualizar produce un libro que existe y no aparece al buscarlo, y ese olvido
-- no lo detecta ninguna prueba de la ruta feliz: el libro está en la
-- biblioteca, se abre, se lee, y solo no se encuentra.
--
-- Los disparadores reconstruyen la fila entera de una edición cuando cambia
-- cualquiera de las tres cosas que la componen. Reconstruir es más caro que
-- actualizar un campo y es lo correcto: un `UPDATE` parcial exigiría saber cuál
-- de los tres cambió, y ahí es donde se cuelan los defectos.
--
-- Contratos: docs/architecture/search.md, sección 1, y
-- docs/architecture/api/schemas.md.

CREATE VIRTUAL TABLE catalog_fts USING fts5(
    edition_id UNINDEXED,
    title,
    authors,
    subjects,
    tokenize = "unicode61 remove_diacritics 2"
);

-- ---------------------------------------------------------------------------
-- Reconstrucción de una fila.
--
-- Las tres consultas van juntas en cada disparador porque FTS5 no admite un
-- `UPDATE ... FROM`, así que la fila se borra y se vuelve a escribir entera.
-- ---------------------------------------------------------------------------
CREATE TRIGGER trg_catalog_fts_edicion_alta AFTER INSERT ON editions
BEGIN
    INSERT INTO catalog_fts (edition_id, title, authors, subjects)
    VALUES (
        new.id,
        coalesce(new.title, '') || ' ' || coalesce(new.subtitle, '')
            || ' ' || coalesce(new.series_title, ''),
        coalesce((SELECT group_concat(c.name, ' ')
                    FROM edition_contributors ec
                    JOIN contributors c ON c.id = ec.contributor_id
                   WHERE ec.edition_id = new.id), ''),
        coalesce((SELECT group_concat(s.label, ' ')
                    FROM edition_subjects es
                    JOIN subjects s ON s.id = es.subject_id
                   WHERE es.edition_id = new.id), '')
    );
END;

CREATE TRIGGER trg_catalog_fts_edicion_cambio
AFTER UPDATE OF title, subtitle, series_title ON editions
BEGIN
    DELETE FROM catalog_fts WHERE edition_id = new.id;
    INSERT INTO catalog_fts (edition_id, title, authors, subjects)
    VALUES (
        new.id,
        coalesce(new.title, '') || ' ' || coalesce(new.subtitle, '')
            || ' ' || coalesce(new.series_title, ''),
        coalesce((SELECT group_concat(c.name, ' ')
                    FROM edition_contributors ec
                    JOIN contributors c ON c.id = ec.contributor_id
                   WHERE ec.edition_id = new.id), ''),
        coalesce((SELECT group_concat(s.label, ' ')
                    FROM edition_subjects es
                    JOIN subjects s ON s.id = es.subject_id
                   WHERE es.edition_id = new.id), '')
    );
END;

CREATE TRIGGER trg_catalog_fts_edicion_baja AFTER DELETE ON editions
BEGIN
    DELETE FROM catalog_fts WHERE edition_id = old.id;
END;

CREATE TRIGGER trg_catalog_fts_autor_alta AFTER INSERT ON edition_contributors
BEGIN
    DELETE FROM catalog_fts WHERE edition_id = new.edition_id;
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
      FROM editions e WHERE e.id = new.edition_id;
END;

CREATE TRIGGER trg_catalog_fts_autor_baja AFTER DELETE ON edition_contributors
BEGIN
    DELETE FROM catalog_fts WHERE edition_id = old.edition_id;
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
      FROM editions e WHERE e.id = old.edition_id;
END;

CREATE TRIGGER trg_catalog_fts_materia_alta AFTER INSERT ON edition_subjects
BEGIN
    DELETE FROM catalog_fts WHERE edition_id = new.edition_id;
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
      FROM editions e WHERE e.id = new.edition_id;
END;

CREATE TRIGGER trg_catalog_fts_materia_baja AFTER DELETE ON edition_subjects
BEGIN
    DELETE FROM catalog_fts WHERE edition_id = old.edition_id;
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
      FROM editions e WHERE e.id = old.edition_id;
END;
