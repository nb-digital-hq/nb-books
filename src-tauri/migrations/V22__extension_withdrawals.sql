-- V22: qué extensiones instaladas ha retirado el registro, y por qué.
--
-- # Por qué se guarda, si se podría volver a preguntar
--
-- Porque preguntar exige red y el aviso tiene que sobrevivir a cerrar la
-- aplicación. Una extensión retirada por maliciosa se apaga sola, y si el
-- motivo no se guardara, al volver a abrir aparecería apagada sin explicación:
-- exactamente la pérdida silenciosa que el producto existe para impedir.
--
-- # Por qué una fila por extensión y no un histórico
--
-- Lo que hace falta enseñar es el estado de ahora. Una comprobación reemplaza
-- lo anterior entera —se borra y se vuelve a escribir en la misma
-- transacción—, así que una extensión que el registro deja de tener retirada
-- desaparece de aquí sola. Un histórico obligaría a decidir qué fila manda, y
-- la respuesta siempre sería "la última".
--
-- # Por qué la versión se guarda aquí
--
-- Porque una retirada puede afectar solo a algunas versiones. Guardar cuál
-- estaba instalada al saberlo permite decir "la 2.0.0 que tienes" en lugar de
-- "esta extensión", que es la diferencia entre un aviso accionable y una
-- alarma.
--
-- Contrato: docs/legal/governance/extension-policy.md, sección 6.

CREATE TABLE extension_withdrawals (
    -- El identificador del manifiesto, el mismo del perfil del almacén.
    extension    TEXT NOT NULL,
    -- La versión instalada cuando se supo de la retirada.
    version      TEXT NOT NULL,
    -- malicious, breach o abandoned. Solo la primera apaga la extensión.
    reason       TEXT NOT NULL,
    -- Lo que el registro cuenta, para poder leerlo sin salir a buscarlo.
    detail       TEXT NOT NULL DEFAULT '',
    -- La fecha que declara el registro, no la de esta máquina.
    withdrawn_at TEXT NOT NULL DEFAULT '',
    -- Cuándo se enteró este equipo.
    noticed_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    -- Si la aplicación la apagó al enterarse.
    disabled     INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (extension),
    CHECK (length(trim(extension)) > 0),
    CHECK (length(trim(reason)) > 0),
    CHECK (disabled IN (0, 1))
) STRICT, WITHOUT ROWID;
