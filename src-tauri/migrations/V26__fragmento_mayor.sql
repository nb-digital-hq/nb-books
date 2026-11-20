-- V26: el fragmento indexado pasa de 480 a 1.920 caracteres.
--
-- # Por qué se vacía el índice en lugar de convertirlo
--
-- Un fragmento no se puede "juntar" con el siguiente: su locator apunta al
-- principio de lo que contiene, y unir cuatro daría un texto correcto con un
-- locator que ya no dice dónde empieza la mitad de él. Lo que hay que hacer es
-- volver a trocear desde el documento, y eso es exactamente lo que hace la
-- indexación de fondo.
--
-- Así que aquí se vacía, y la cola se ceba sola: `enqueue_missing` encola lo que
-- no tiene fragmentos, así que en el siguiente arranque vuelven a entrar todos
-- sin que nadie tenga que llevar un cursor. Es la misma propiedad que hace que
-- una indexación interrumpida se reanude, y por eso no hace falta nada más.
--
-- # Lo que esto cuesta, dicho
--
-- Volver a indexar lo que ya estaba indexado. Sobre una biblioteca grande son
-- horas de trabajo de fondo, que se hacen con la aplicación abierta, cediendo
-- por batería y sin bloquear nada. A cambio, cada fragmento deja de costar unos
-- 300 bytes de locator repetidos cuatro veces de más.
--
-- El estado de búsqueda se pone en pendiente y no en "no buscable": lo que se
-- ha perdido no es la capacidad de buscar en esos documentos, es el índice, y
-- decir "no se puede buscar" sería mentir mientras se rehace.
--
-- Contrato: docs/architecture/search.md.

DELETE FROM text_chunks;

UPDATE search_status SET state = 'pending', reason = NULL WHERE state <> 'not_searchable';
