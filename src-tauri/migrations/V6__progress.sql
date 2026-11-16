-- V6: progreso de lectura y sesiones medibles.
--
-- Las dos tablas guardan **dónde estaba leyendo alguien**, y por eso las dos
-- anclan con un Locator serializado y ninguna con un número. Es la regla de
-- ADR-004 y la comprueba `tests/anclaje_de_posiciones.rs` recorriendo el
-- esquema vivo, no leyendo este archivo.
--
-- Por qué el progreso cuelga de la **edición** y no del archivo: el archivo
-- puede moverse, volver a importarse o existir en dos copias, y en los tres
-- casos el usuario espera seguir donde estaba. La edición es lo que no cambia.
--
-- Contratos: docs/architecture/api/schemas.md y
-- docs/architecture/reader-contracts.md, sección 6.

CREATE TABLE progress (
    id         TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    -- Un perfil por persona que use la misma instalación. Hoy solo existe
    -- 'default'; la columna está desde ahora porque añadirla después obligaría a
    -- migrar una tabla que para entonces tendría el progreso de toda una
    -- biblioteca, y el valor por defecto hace que no cueste nada tenerla.
    profile    TEXT NOT NULL DEFAULT 'default',
    -- El Locator, serializado. Nunca un entero: ver ADR-004.
    locator    TEXT NOT NULL,
    -- Con qué precisión se resolvió la última vez que se restauró.
    --
    -- Se guarda para que al reabrir se pueda avisar sin volver a resolver: si la
    -- última restauración fue aproximada, el usuario tiene derecho a saberlo
    -- aunque hoy no toque el documento.
    precision  TEXT NOT NULL DEFAULT 'exact'
               CHECK (precision IN ('exact', 'quote', 'approximate')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    -- Un solo progreso vivo por edición y perfil. Sin esto, el antirrebote
    -- podría dejar dos filas y ninguna sería "la" posición.
    UNIQUE (edition_id, profile),

    CHECK (json_valid(locator) AND json_type(locator) = 'object')
) STRICT;

CREATE INDEX idx_progress_edition ON progress(edition_id);

-- ---------------------------------------------------------------------------
-- Sesiones de lectura.
--
-- Una sesión es "estuve leyendo esto, desde aquí hasta aquí, entre estas dos
-- horas". Es la base de la analítica local, y por eso guarda las dos posiciones
-- como Locator y no un número de páginas leídas: las páginas dependen de la
-- tipografía y de la ventana, así que "leí 30 páginas" no significa lo mismo el
-- martes que el jueves.
--
-- No hay columna de duración. La duración se calcula de las dos marcas de
-- tiempo, y guardarla además obligaría a mantener sincronizadas dos versiones
-- del mismo dato. El **tiempo efectivo** (descontando lo que el documento estuvo
-- abierto sin que nadie leyera) es otra cosa, es analítica, y llega con la
-- módulo que la decida.
-- ---------------------------------------------------------------------------
CREATE TABLE reading_sessions (
    id            TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    edition_id    TEXT NOT NULL REFERENCES editions(id) ON DELETE CASCADE,
    profile       TEXT NOT NULL DEFAULT 'default',
    started_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Nulo mientras la sesión sigue abierta, y también si la aplicación se cerró
    -- de golpe. Una sesión sin cierre es información: dice que hubo un cierre
    -- inesperado, y borrarla lo escondería.
    ended_at      TEXT,
    start_locator TEXT NOT NULL,
    -- Nulo hasta que la sesión se cierra.
    end_locator   TEXT,

    CHECK (json_valid(start_locator) AND json_type(start_locator) = 'object'),
    CHECK (end_locator IS NULL
           OR (json_valid(end_locator) AND json_type(end_locator) = 'object'))
) STRICT;

CREATE INDEX idx_reading_sessions_edition ON reading_sessions(edition_id, started_at);

CREATE TRIGGER trg_progress_updated_at AFTER UPDATE ON progress
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE progress SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
