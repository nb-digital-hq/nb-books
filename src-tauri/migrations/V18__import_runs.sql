-- V18: la importación que estabas haciendo, y lo que dejó fuera.
--
-- # Qué se perdía
--
-- Una importación de ciento cincuenta mil archivos dura horas, y hasta aquí la
-- aplicación **no recordaba que la estabas haciendo**. Reanudar era gratis por la
-- identidad de contenido —lo ya importado se reconoce y se salta— pero solo si te
-- acordabas de volver a lanzarla sobre la misma carpeta. Y el informe de lo que
-- se rechazó vivía en el panel de esa ejecución y en ningún sitio más: cerrarlo
-- era perderlo.
--
-- Las dos son la pregunta J12, y las dos son alcance que nadie había decidido, no
-- defectos de lo que se hizo.
--
-- # Por qué una fila abierta es la señal
--
-- Una ejecución con `finished_at` nulo significa "se empezó y no se terminó". No
-- hace falta ninguna bandera aparte, y sobre todo **funciona si el proceso muere**:
-- nadie tiene que acordarse de escribir nada al morir, que es justo lo que no
-- ocurre cuando algo se cae. Terminar, cancelar y fallar cierran la fila; morir la
-- deja abierta, que es la verdad.
--
-- # Qué no se guarda
--
-- El contenido de nada. Una fila de rechazo lleva la ruta del archivo que no
-- entró y el código canónico del motivo, que es lo que hace falta para poder ir a
-- mirarlo. Nunca el título, ni un fragmento, ni el mensaje de error en prosa: los
-- mensajes se traducen en la interfaz desde el código, que es la regla de
-- `error.rs`.
--
-- Contrato: docs/architecture/storage.md.

CREATE TABLE import_runs (
    id                 TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    -- La carpeta que se eligió. Es lo que hay que ofrecer para continuar.
    root               TEXT NOT NULL,
    -- Referenciada o administrada: reanudar tiene que hacer lo mismo que se
    -- pidió, no lo que esté puesto por defecto ahora.
    storage_mode       TEXT NOT NULL,
    started_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Nulo mientras corre. Es la única señal de "esto quedó a medias".
    finished_at        TEXT,
    processed          INTEGER NOT NULL DEFAULT 0,
    imported           INTEGER NOT NULL DEFAULT 0,
    already_in_library INTEGER NOT NULL DEFAULT 0,
    -- Cuántos se rechazaron **en total**, que puede ser más de las filas que se
    -- guarden en `import_rejections`: el recuento no miente aunque la lista se
    -- acote.
    rejected           INTEGER NOT NULL DEFAULT 0,
    cancelled          INTEGER NOT NULL DEFAULT 0,

    CHECK (length(trim(root)) > 0),
    CHECK (storage_mode IN ('referenced', 'managed')),
    CHECK (cancelled IN (0, 1)),
    CHECK (processed >= 0 AND imported >= 0 AND already_in_library >= 0 AND rejected >= 0)
) STRICT;

-- Para encontrar la que quedó abierta, y para listar por fecha.
CREATE INDEX idx_import_runs_abiertas ON import_runs(started_at DESC) WHERE finished_at IS NULL;
CREATE INDEX idx_import_runs_fecha ON import_runs(started_at DESC);

CREATE TABLE import_rejections (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id     TEXT NOT NULL REFERENCES import_runs(id) ON DELETE CASCADE,
    -- La ruta del archivo que no entró, para poder ir a mirarlo.
    path       TEXT NOT NULL,
    -- El código canónico de `error.rs`. Nunca el mensaje: se traduce al pintarlo.
    code       TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    CHECK (length(trim(path)) > 0),
    CHECK (length(trim(code)) > 0)
) STRICT;

CREATE INDEX idx_import_rejections_run ON import_rejections(run_id, id);
