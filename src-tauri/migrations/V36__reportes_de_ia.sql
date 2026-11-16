-- V36: lo que el usuario marca como incorrecto de una salida generada.
--
-- # Por qué existe
--
-- Porque la rúbrica automática mide lo que se puede medir sin una persona -que una
-- tarjeta cite, que lo citado sostenga lo que dice- y **no puede medir lo que solo
-- ve quien conoce el libro**: que la respuesta sea falsa aunque esté bien citada,
-- que la pregunta no venga a cuento, que el resumen se deje lo importante. La
-- sección 6 de `ai/evaluation.md` lo pide, y sin una tabla el
-- "marcar como incorrecta" sería un botón que no lleva a ninguna parte.
--
-- # Qué guarda, y sobre todo qué no
--
-- Guarda **lo que hace falta para reproducir el fallo sin el libro delante**: qué
-- función era, con qué plantilla y versión, qué proveedor y qué modelo, qué dijo
-- el usuario que estaba mal, y la salida generada. La salida es del modelo, no del
-- documento.
--
-- **No guarda el texto del documento.** Ni el fragmento citado, ni el título, ni la
-- ruta: solo el identificador de la edición, que fuera de este equipo no dice nada
-- de qué libro es. Es la regla -"sin incluir el texto del documento salvo
-- autorización explícita"- puesta en el esquema, que es donde no se puede olvidar:
-- si la columna no existe, ninguna pantalla puede llenarla por descuido.
--
-- # Local por defecto
--
-- Nada de esto sale del equipo por sí solo. `ai/privacy.md` sección 3: local,
-- exportable y borrable. Exportar produce un archivo que el usuario mira antes de
-- decidir si lo manda a alguna parte, y mandarlo es cosa suya.

CREATE TABLE ai_reports (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- La función de estudio, con el nombre de `estudio::Funcion`.
    function      TEXT NOT NULL,
    template_id   TEXT NOT NULL,
    template_ver  INTEGER NOT NULL,
    provider_id   TEXT NOT NULL,
    provider_label TEXT NOT NULL,
    -- Si el texto salió del dispositivo para generar lo que se reporta.
    left_device   INTEGER NOT NULL DEFAULT 0 CHECK (left_device IN (0, 1)),
    -- La edición, sin título ni ruta: identifica sin decir qué libro es.
    edition_id    TEXT,
    -- Qué está mal, de la lista cerrada que ofrece la interfaz.
    reason        TEXT NOT NULL CHECK (reason IN ('falso', 'no_viene_a_cuento', 'incompleto', 'mal_escrito', 'otro')),
    -- Lo que el usuario quiera añadir. Puede estar vacío.
    note          TEXT NOT NULL DEFAULT '',
    -- La salida generada, que es del modelo y no del documento.
    output        TEXT NOT NULL,

    CHECK (length(trim(function)) > 0),
    CHECK (length(trim(template_id)) > 0),
    CHECK (template_ver >= 0)
) STRICT;

-- Se consulta de lo más reciente a lo más antiguo, igual que el registro de envíos.
CREATE INDEX idx_ai_reports_created_at ON ai_reports(created_at DESC);
