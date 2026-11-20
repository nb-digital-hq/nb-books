-- V4: preferencias del usuario. SQLite es la fuente de verdad.
--
-- Hasta ahora vivían en el almacenamiento del webview, que se pierde al limpiar
-- los datos del navegador, no entra en el respaldo ni en la exportación y no se
-- puede sincronizar. Las preferencias que más importa preservar son las de
-- accesibilidad, y son justamente las más expuestas a perderse así.
--
-- Contrato: docs/adr/010-settings-persistence.md.
--.

CREATE TABLE settings (
    key           TEXT PRIMARY KEY,
    -- Escalar JSON, no texto plano: así `false` no se confunde con la cadena
    -- "false" ni 1.0 con "1.0". El tipo forma parte del valor.
    value         TEXT NOT NULL,
    -- Versión del esquema **de este valor**, no de la tabla. Permite cambiar la
    -- representación de una preferencia concreta (por ejemplo, la escala
    -- tipográfica de porcentaje entero a factor decimal) sin adivinar qué
    -- significa cada fila antigua. ADR-010 lo exige explícitamente.
    value_version INTEGER NOT NULL DEFAULT 1,
    updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(key)) > 0),
    CHECK (json_valid(value)),
    CHECK (value_version >= 1)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER trg_settings_updated_at AFTER UPDATE ON settings
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE settings SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE key = NEW.key;
END;

-- ---------------------------------------------------------------------------
-- Los secretos no entran aquí.
--
-- Regla 7 de docs/architecture/api/schemas.md y regla 4 de ADR-010: los tokens y las claves
-- de API viven en el almacén de credenciales del sistema operativo, que llega
-- con la. La superficie de Rust ya lo impide, porque las
-- claves son una enumeración cerrada; estos disparadores cubren la otra puerta,
-- la del SQL escrito a mano, para que la regla no dependa de por dónde se entre.
-- ---------------------------------------------------------------------------
CREATE TRIGGER trg_settings_sin_secretos_insert BEFORE INSERT ON settings
WHEN NEW.key LIKE '%token%' OR NEW.key LIKE '%secret%' OR NEW.key LIKE '%password%'
  OR NEW.key LIKE '%api_key%' OR NEW.key LIKE '%credential%' OR NEW.key LIKE '%passphrase%'
BEGIN
    SELECT RAISE(ABORT, 'los secretos no se guardan en la base: ver ADR-010');
END;

CREATE TRIGGER trg_settings_sin_secretos_update BEFORE UPDATE ON settings
WHEN NEW.key LIKE '%token%' OR NEW.key LIKE '%secret%' OR NEW.key LIKE '%password%'
  OR NEW.key LIKE '%api_key%' OR NEW.key LIKE '%credential%' OR NEW.key LIKE '%passphrase%'
BEGIN
    SELECT RAISE(ABORT, 'los secretos no se guardan en la base: ver ADR-010');
END;
