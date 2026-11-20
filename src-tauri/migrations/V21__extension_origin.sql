-- V21: quién quiere salir, además de a dónde y para qué.
--
-- # Por qué no basta con anfitrión y propósito
--
-- V13 fijó que el consentimiento es por anfitrión **y** por propósito, porque
-- autorizar un servidor no es autorizarlo para cualquier cosa. Con las
-- extensiones aparece una tercera pregunta que las dos anteriores no responden:
-- **quién**.
--
-- Sin el origen, conceder a una extensión salida a `openlibrary.org` la
-- concedería a la vez a todas las demás, presentes y futuras, porque el permiso
-- viviría pegado al anfitrión y no a quien lo pidió. Instalar una segunda
-- extensión heredaría en silencio lo que el usuario aprobó para la primera, que
-- es exactamente la clase de permiso que nadie recuerda haber dado.
--
-- # Qué significa el origen vacío
--
-- La propia aplicación. Se guarda como cadena vacía y no como `NULL` para que
-- pueda formar parte de la clave primaria sin las reglas de comparación de
-- `NULL`, que en SQL no es igual ni a sí mismo.
--
-- Los consentimientos que ya existían pasan a origen vacío, que es lo que eran:
-- permisos de la aplicación. Nadie pierde nada y nadie gana nada.
--
-- Contrato: docs/architecture/security/authorization.md.

CREATE TABLE network_consents_v21 (
    -- Quién sale. Vacío es la propia aplicación; si no, el identificador de la
    -- extensión, tal y como lo declara su manifiesto.
    origin     TEXT NOT NULL DEFAULT '',
    host       TEXT NOT NULL,
    purpose    TEXT NOT NULL,
    granted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),

    PRIMARY KEY (origin, host, purpose),
    CHECK (length(trim(host)) > 0),
    CHECK (length(trim(purpose)) > 0)
) STRICT, WITHOUT ROWID;

INSERT INTO network_consents_v21 (origin, host, purpose, granted_at)
SELECT '', host, purpose, granted_at FROM network_consents;

DROP TABLE network_consents;
ALTER TABLE network_consents_v21 RENAME TO network_consents;

-- El registro solo gana una columna: no cambia de clave y no hay que
-- reconstruirlo. Lo que ya está anotado sigue siendo de la aplicación.
ALTER TABLE network_log ADD COLUMN origin TEXT NOT NULL DEFAULT '';
