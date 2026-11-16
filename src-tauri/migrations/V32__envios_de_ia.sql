-- V32: el registro local de lo que se envía a un proveedor de IA externo.
--
-- # Por qué existe, si ya hay un registro de salidas
--
-- Porque el de salidas (V13) dice **a dónde** se salió y cuántos bytes, y lo dice
-- a propósito sin la ruta ni el contenido: sirve para cualquier salida, y la ruta
-- de un catálogo puede llevar el título de lo que se busca. Aquí la regla 4 de
-- `ai/privacy.md` pide otra cosa: que el usuario pueda auditar **qué se envió**.
-- Una línea que dijera "salieron 3.412 bytes a api.anthropic.com" no deja comprobar
-- que no se envió el documento entero, que es la regla 1.
--
-- # Por qué guarda el contenido, y por qué eso no choca con la redacción
--
-- `body` es el mensaje entero tal como salió, **sin la credencial**, que va en una
-- cabecera y nunca en el cuerpo. Lleva fragmentos de documentos del usuario, y la
-- regla de redacción prohíbe ponerlos en los **registros de la aplicación** —los
-- que acaban en un informe de fallo— y no en su base, que ya guarda el texto
-- entero de lo indexado. Es historial del usuario, en su equipo, y `ai/privacy.md`
-- sección 3 dice cómo se trata: local, exportable y borrable.
--
-- # Qué no entra
--
-- Lo que no llegó a salir: un envío sin consentimiento o con el modo sin red
-- puesto no tiene contenido que auditar, y ya queda en el registro de salidas como
-- `denied` u `offline`. Tampoco la respuesta: lo que se audita es lo que salió.

CREATE TABLE ai_sends (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    sent_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- `openai` o `anthropic`: a qué protocolo se habló.
    protocol   TEXT NOT NULL CHECK (protocol IN ('openai', 'anthropic')),
    host       TEXT NOT NULL,
    model      TEXT NOT NULL,
    -- Para qué función, con el nombre de `ai::Capability`.
    capability TEXT NOT NULL,
    body       TEXT NOT NULL,
    bytes      INTEGER NOT NULL,
    -- El código de estado de la respuesta; nulo si no llegó a haberla.
    status     INTEGER,

    CHECK (length(trim(host)) > 0),
    CHECK (bytes >= 0)
) STRICT;

-- Se consulta de lo más reciente a lo más antiguo, igual que el de salidas.
CREATE INDEX idx_ai_sends_sent_at ON ai_sends(sent_at DESC);
