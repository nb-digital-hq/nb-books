-- V19: la clave por la que se ordena por autor, en la propia edición.
--
-- # Qué estaba pasando
--
-- Ordenar la biblioteca por autor costaba unos **60 ms** sobre ciento cincuenta
-- mil obras, contra un presupuesto de fotograma de 16. No por falta de índice: la
-- clave no está en `editions` sino al otro lado de dos uniones —a
-- `edition_contributors` y a `contributors`—, así que el motor tiene que unir
-- primero y ordenar después, y eso construye un árbol temporal se ponga el índice
-- que se ponga. Se probó con un índice sobre el nombre del contribuidor y quedó
-- en 79 ms, peor. Es la pregunta abierta J11.
--
-- Los otros órdenes se arreglaron en V17 con índices de cobertura porque su clave
-- ya vivía en `editions`. Este no tenía esa salida.
--
-- # Por qué se copia y no se deriva
--
-- `contributors.sort_name` ya existe desde V2, y su comentario dice por qué:
-- derivar "Apellido, Nombre" del nombre de presentación **falla en cuanto hay dos
-- apellidos o una partícula**. Así que aquí no se inventa ninguna heurística: se
-- copia `sort_name` si lo hay y el nombre si no, que es la misma decisión que ya
-- estaba tomada, materializada donde se puede indexar.
--
-- # Cuál de los autores
--
-- El primero por `position`, que es el principal. Se toma con `ORDER BY position
-- LIMIT 1` y no con `position = 0` —que es lo que hacía la consulta— porque una
-- edición cuyos autores empiecen en 1, por un importador que numere desde uno, no
-- tiene autor cero: antes se ordenaba como si no tuviera autor, y ahora ordena
-- por el suyo.
--
-- # Los cuatro disparadores, y por qué son cuatro
--
-- Una columna desnormalizada que solo se mantiene en el caso feliz es peor que no
-- tenerla: el orden miente y nadie se entera. Hay que cubrir **alta, baja, cambio
-- de autoría y cambio de nombre de la persona**. Los tres primeros son sobre
-- `edition_contributors`; el cuarto sobre `contributors`, porque renombrar a
-- alguien cambia el orden de todas sus obras y sin él se quedarían con el nombre
-- viejo hasta la próxima reimportación.
--
-- Contrato: docs/architecture/storage.md.

ALTER TABLE editions ADD COLUMN author_sort TEXT;

-- `author_sort` no es una modificación de la edición, así que no marca la fecha.
--
-- # Por qué hay que decírselo al disparador de V2
--
-- `trg_editions_updated_at` se dispara ante **cualquier** `UPDATE` sobre
-- `editions`. Como los disparadores de más abajo escriben en esa tabla, enlazar
-- una autoría pasaría a marcar la edición como modificada. Y hasta ahora **no lo
-- hacía**: enlazar un autor escribe en `edition_contributors` y no tocaba
-- `editions` para nada. Así que esto no es una decisión nueva sobre qué cuenta
-- como modificar una obra; es no cambiarla por accidente al materializar una
-- clave de orden.
--
-- Importa más allá de la fecha: la resolución de conflictos de ADR-006 compara
-- marcas de tiempo, y una exportación que se reimporta tiene que devolver una
-- biblioteca equivalente. Hay una prueba que lo comprueba, y fue la que avisó.
--
-- Se recrea en lugar de editarse: **una migración publicada no se toca**, y V2 lo
-- está. Lo que cambia es la lista de columnas que cuentan; el cuerpo y la guarda
-- contra la recursión son los mismos.
DROP TRIGGER trg_editions_updated_at;

CREATE TRIGGER trg_editions_updated_at
AFTER UPDATE OF work_id, title, subtitle, publisher, published_date,
                edition_statement, language, series_title, series_index, cover_path
ON editions
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE editions SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- El valor de partida para lo que ya está en la biblioteca. Sin esto, ordenar por
-- autor devolvería todo con clave nula hasta que alguien tocara cada obra.
UPDATE editions
   SET author_sort = (
       SELECT COALESCE(c.sort_name, c.name)
         FROM edition_contributors ec
         JOIN contributors c ON c.id = ec.contributor_id
        WHERE ec.edition_id = editions.id AND ec.role = 'author'
        ORDER BY ec.position
        LIMIT 1
   );

-- De cobertura, con el desempate por `id` dentro: el orden real termina siempre
-- en `e.id` para que dos autores iguales no se intercambien entre dos consultas,
-- y un índice que no lo lleve deja al motor ordenando el último tramo a mano.
CREATE INDEX idx_editions_autor ON editions(author_sort COLLATE NOCASE, id);

-- El alta es el caso caliente: una importación de ciento cincuenta mil archivos
-- pasa por aquí una vez por documento. Por eso no recalcula como los demás.
--
-- `WHEN NEW.role = 'author'` corta de raíz los editores, traductores e
-- ilustradores, que no cambian esta clave y hasta la primera versión de esta
-- migración disparaban un recálculo entero para nada.
--
-- Y cuando sí es autor, no hace falta volver a buscar quién es el principal:
-- basta con saber si el que entra se pone por delante de los que ya había. Si no
-- hay ninguno con posición menor, es él.
--
-- Medido sobre veinte mil autorías, **con una pasada de control sin disparadores
-- para restar el coste de la propia inserción**, que es lo que hace que la cifra
-- signifique algo: insertarlas cuesta 77,7 s de por sí. Con el disparador
-- ingenuo, 98,4 s —1,03 ms por fila—; con este, 85,2 —0,37 ms—. Sobre una
-- importación de ciento cincuenta mil documentos eso son 56 segundos en lugar de
-- dos minutos y medio, sobre un trabajo que dura horas.
CREATE TRIGGER author_sort_al_anadir_autoria
AFTER INSERT ON edition_contributors
WHEN NEW.role = 'author'
  AND NOT EXISTS (
      SELECT 1 FROM edition_contributors ec
       WHERE ec.edition_id = NEW.edition_id
         AND ec.role = 'author'
         AND ec.position < NEW.position
  )
BEGIN
    UPDATE editions
       SET author_sort = (
           SELECT COALESCE(c.sort_name, c.name)
             FROM contributors c WHERE c.id = NEW.contributor_id
       )
     WHERE id = NEW.edition_id;
END;

-- Quitar, cambiar y renombrar son raros —no ocurren durante una importación— así
-- que se quedan con el recálculo entero, que es el que no puede equivocarse.
CREATE TRIGGER author_sort_al_quitar_autoria
AFTER DELETE ON edition_contributors
WHEN OLD.role = 'author'
BEGIN
    UPDATE editions
       SET author_sort = (
           SELECT COALESCE(c.sort_name, c.name)
             FROM edition_contributors ec
             JOIN contributors c ON c.id = ec.contributor_id
            WHERE ec.edition_id = OLD.edition_id AND ec.role = 'author'
            ORDER BY ec.position
            LIMIT 1
       )
     WHERE id = OLD.edition_id;
END;

-- Cambiar la posición o el rol cambia quién es el autor principal. Y la fila
-- puede mudarse de edición, así que se recalculan las dos.
CREATE TRIGGER author_sort_al_cambiar_autoria
AFTER UPDATE ON edition_contributors
WHEN OLD.role = 'author' OR NEW.role = 'author'
BEGIN
    UPDATE editions
       SET author_sort = (
           SELECT COALESCE(c.sort_name, c.name)
             FROM edition_contributors ec
             JOIN contributors c ON c.id = ec.contributor_id
            WHERE ec.edition_id = editions.id AND ec.role = 'author'
            ORDER BY ec.position
            LIMIT 1
       )
     WHERE id IN (OLD.edition_id, NEW.edition_id);
END;

-- Renombrar a una persona cambia el orden de todas sus obras. Sin esto se
-- quedarían con el nombre viejo hasta que alguien las reimportara, que es la
-- clase de mentira silenciosa que una columna desnormalizada trae si nadie la
-- mantiene entera.
CREATE TRIGGER author_sort_al_renombrar_persona
AFTER UPDATE OF name, sort_name ON contributors
BEGIN
    UPDATE editions
       SET author_sort = (
           SELECT COALESCE(c.sort_name, c.name)
             FROM edition_contributors ec
             JOIN contributors c ON c.id = ec.contributor_id
            WHERE ec.edition_id = editions.id AND ec.role = 'author'
            ORDER BY ec.position
            LIMIT 1
       )
     WHERE id IN (
         SELECT edition_id FROM edition_contributors
          WHERE contributor_id = NEW.id AND role = 'author'
     );
END;
