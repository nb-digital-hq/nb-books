-- V25: toda edición tiene estado de búsqueda desde que entra.
--
-- # Qué estaba pasando
--
-- La fila de `search_status` la escribía **el indexador**, cuando le tocaba el
-- turno a ese documento. Hasta entonces la edición no tenía fila, y una edición
-- sin fila no está "pendiente": **no existe para esa maquinaria**. La interfaz
-- no podía decir "aún no" porque no había nada que decir.
--
-- Medido el 2026-09-09 sobre la instalación real: **130.724 ediciones sin fila**,
-- de 151.411. Es decir, sobre el 86 % del catálogo la aplicación no sabía ni
-- siquiera que le faltaba indexar.
--
-- # Por qué un disparador y no una línea en la importación
--
-- Porque hay más de un camino que crea una edición: la importación de un
-- archivo, la descarga desde un catálogo OPDS, y lo que traiga cualquier función
-- futura. Poner la línea en uno de ellos deja los demás fuera, y el defecto
-- vuelve por donde nadie está mirando: es exactamente lo que pasó con la
-- transacción diferida de V24. Con el disparador, la garantía es de la base y no
-- de que alguien se acuerde.
--
-- `INSERT OR IGNORE` y no `INSERT`: si alguna vía ya escribió el estado -el
-- indexador puede llegar antes de que termine la transacción de importación-, el
-- suyo manda. Lo que este disparador garantiza es que **haya** fila, no cuál.
--
-- Contrato: docs/architecture/search.md, sección 4.2.-- Contrato: docs/architecture/search.md.

-- Lo que ya está dentro. `pending` y no otra cosa porque es la verdad: nadie las
-- ha mirado todavía.
INSERT OR IGNORE INTO search_status (edition_id, state)
SELECT id, 'pending' FROM editions;

CREATE TRIGGER search_status_al_crear_edicion
AFTER INSERT ON editions
BEGIN
    INSERT OR IGNORE INTO search_status (edition_id, state) VALUES (NEW.id, 'pending');
END;
