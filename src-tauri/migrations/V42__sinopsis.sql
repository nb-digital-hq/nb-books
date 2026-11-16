-- V42: la sinopsis de cada edición (A9).
--
-- # Por qué
--
-- El EPUB la trae en `dc:description` y nadie la leía: la ficha no podía decir de
-- qué va un libro. Va en la edición y no en la obra porque cada edición trae la
-- suya, y una traducción puede describirse distinto que el original.
--
-- La rellena para lo ya importado una pasada en segundo plano que solo lee el
-- paquete de cada EPUB; lo que entre a partir de ahora la trae al importar.

ALTER TABLE editions ADD COLUMN description TEXT;
