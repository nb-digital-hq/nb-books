-- V13: la única puerta por la que algo sale del dispositivo.
--
-- # Qué hace cumplir esta migración
--
-- El innegociable de `AGENTS.md` dice que nada sale del dispositivo sin
-- consentimiento explícito. Hasta hoy ese innegociable **no tenía mecanismo**:
-- lo cumplía la ausencia de código de red, que no es una garantía sino una
-- casualidad que dura hasta la primera petición.
--
-- # Dos tablas, y por qué no una
--
-- El **consentimiento** dice a dónde se puede salir. El **registro** dice a
-- dónde se salió. Son cosas distintas: revocar un consentimiento no puede
-- borrar la historia de lo que ya ocurrió, porque entonces el usuario perdería
-- justo la información que necesita para decidir si revocarlo.
--
-- # El consentimiento es por anfitrión y por propósito
--
-- Autorizar "salir a este servidor" no es lo mismo que autorizar "salir a este
-- servidor para cualquier cosa". Un catálogo al que se le permite servir libros
-- no queda por ello autorizado a recibir el texto de una selección para
-- traducirla. Sin el propósito, un permiso concedido una vez se convierte en un
-- permiso para todo.
--
-- Contrato: docs/audit/privacy.md.

CREATE TABLE network_consents (
    host       TEXT NOT NULL,
    -- Para qué se autoriza. Ver `net::Purpose`.
    purpose    TEXT NOT NULL,
    granted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (host, purpose),
    CHECK (length(trim(host)) > 0),
    CHECK (length(trim(purpose)) > 0)
) STRICT, WITHOUT ROWID;

CREATE TABLE network_log (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    host     TEXT NOT NULL,
    purpose  TEXT NOT NULL,
    -- `allowed`, `denied` (sin consentimiento) u `offline` (interruptor puesto).
    outcome  TEXT NOT NULL CHECK (outcome IN ('allowed', 'denied', 'offline')),
    -- Código de estado, cuando llegó a haber respuesta.
    status   INTEGER,
    bytes    INTEGER NOT NULL DEFAULT 0,

    CHECK (bytes >= 0)
) STRICT;

-- El registro se consulta por fecha, de lo más reciente a lo más antiguo, que
-- es como lo mira una persona.
CREATE INDEX idx_network_log_at ON network_log(at DESC);

-- Nunca se guarda la ruta completa ni la consulta: el anfitrión y el propósito
-- bastan para que el usuario sepa qué salió, y la ruta puede llevar dentro el
-- título de un documento o el texto que seleccionó. Es la misma regla de
-- redacción que ya rige los registros de la aplicación.
