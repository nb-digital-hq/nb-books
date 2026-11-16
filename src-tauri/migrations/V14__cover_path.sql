-- Portada extraída de un documento.
--
-- # Por qué una columna y no mirar el disco
--
-- La portada se extrae al importar y se guarda en la caché como
-- `<edition_id>.<extensión>`. La extensión depende de lo que traiga el
-- documento, así que averiguar el nombre desde fuera obliga a probar una lista
-- de extensiones o a recorrer el directorio. Con ciento cincuenta mil documentos
-- eso es un recorrido de disco por cada pantalla de biblioteca, y la lista se
-- recorre sesenta veces por tirón.
--
-- # Por qué solo el nombre y no la ruta
--
-- La caché cambia de sitio: entre equipos, entre versiones del sistema, y cuando
-- alguien mueve su perfil. Una ruta absoluta guardada aquí sería una ruta rota en
-- cuanto pase cualquiera de esas tres cosas. El nombre se resuelve contra el
-- directorio de caché de **esta** instalación, que es quien sabe dónde está.
--
-- Nulo significa que el documento no traía portada, que es lo normal en un PDF
-- y no es un error.
ALTER TABLE editions ADD COLUMN cover_path TEXT;

CREATE INDEX idx_editions_cover ON editions(cover_path) WHERE cover_path IS NOT NULL;
