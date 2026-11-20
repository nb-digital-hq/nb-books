-- V17: los índices por los que la biblioteca se ordena.
--
-- # Qué estaba pasando
--
-- Ninguno de los órdenes de la biblioteca tenía índice, así que **cada cambio de
-- página volvía a ordenar la biblioteca entera**. No es una hipótesis: medido
-- sobre una copia con 157.783 ediciones, ordenando por título y con cincuenta
-- por página, el plan decía `SCAN e` más `USE TEMP B-TREE FOR ORDER BY`, y una
-- página costaba:
--
--     página 1        21,2 ms
--     página 300      61,0 ms
--     página 1.500   145,8 ms
--     última         184,9 ms
--
-- Contra un presupuesto de fotograma de 16 ms. Con los índices de aquí, el plan
-- pasa a `SCAN e USING COVERING INDEX` y las mismas cuatro páginas cuestan 2,7,
-- 4,5, 4,0 y 9,3 ms.
--
-- # Por qué el coste crecía con el número de página
--
-- Porque ordenar sin índice cuesta lo mismo siempre —la biblioteca entera— y
-- encima `OFFSET` obliga a recorrer y descartar todo lo que queda por delante.
-- Con el índice, el motor entra por él y salta: lo que queda es el propio salto,
-- que es barato y por eso la última página sigue costando menos de diez.
--
-- # Por qué el desempate va dentro del índice
--
-- Porque el orden real termina siempre en `e.id`, para que dos títulos iguales
-- no se intercambien entre dos consultas. Un índice que no lo lleve deja al
-- motor ordenando el último tramo a mano, y entonces vuelve el árbol temporal.
--
-- # Lo que no arregla, y se dice
--
-- Ordenar **por autor** sigue costando unos 60 ms, porque su clave no está en
-- esta tabla sino al otro lado de dos uniones, y ningún índice sobre
-- `contributors` lo evita: se probó, y quedó en 79 ms. Eso pide una columna de
-- ordenación desnormalizada en `editions`, que es una decisión de modelo y no un
-- índice. Queda como pregunta abierta J11.

-- Título: es el orden por defecto de la biblioteca.
CREATE INDEX IF NOT EXISTS idx_editions_titulo
    ON editions(title COLLATE NOCASE, id);

-- Fecha de alta: es el orden de "lo último que añadí".
CREATE INDEX IF NOT EXISTS idx_editions_alta
    ON editions(created_at, id);

-- Serie: dos claves, porque dentro de una serie manda el número.
CREATE INDEX IF NOT EXISTS idx_editions_serie
    ON editions(series_title COLLATE NOCASE, series_index, id);
