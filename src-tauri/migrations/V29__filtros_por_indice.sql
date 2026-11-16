-- V29: filtrar la biblioteca entra por un índice, no recorre el catálogo entero.
--
-- # Qué pasaba
--
-- Con una biblioteca real, abrir una estantería de autor tardaba demasiado en
-- aparecer. Medido sobre la base real,
-- 152.080 ediciones, con la estantería de M. L. Estefanía (1.248 obras):
--
--   * contar las obras del filtro: 0,91 s
--   * decidir qué página se pinta:  0,62 s
--
-- Un segundo y medio por cada cambio de página, y otro tanto para adelantar la
-- siguiente. El plan lo decía: `SCAN e` y, por cada una de las 152.080
-- ediciones, una subconsulta correlacionada contra sus autores. Y dentro de
-- ella, `contributors.name` **no tenía índice**, así que no había forma de
-- empezar por el autor aunque la consulta se escribiera al revés.
--
-- Lo mismo con "No disponibles": 0,41 s para contar cero, recorriendo todos los
-- archivos para encontrar los que no están.
--
-- # Qué cambia
--
-- Dos índices, y la consulta se reescribe en `browse.rs` para entrar por ellos
-- (`e.id IN (...)` en lugar de `EXISTS` correlacionado). Medido sobre una copia
-- de las mismas tablas: el filtro de autor pasa de 1,5 s a **7 ms**, y contar lo
-- no disponible de 0,41 s a 0,1 ms.
--
-- El índice de lo no disponible es **parcial**: solo guarda los archivos que no
-- están, que en una biblioteca sana son cero. Un índice entero sobre una columna
-- que vale lo mismo en el 100 % de las filas ocuparía sitio para no distinguir
-- nada.

CREATE INDEX idx_contributors_nombre ON contributors(name);

CREATE INDEX idx_files_no_disponibles ON files(edition_id)
    WHERE availability <> 'available';
