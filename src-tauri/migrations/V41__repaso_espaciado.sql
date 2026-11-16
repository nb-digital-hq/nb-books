-- V41: Repaso espaciado de tarjetas y preguntas de estudio.
--
-- # Por qué en la base
--
-- Cada tarjeta guarda cuándo toca repasarla, su intervalo en días y su facilidad
-- acumulada. Repasar no es un evento efímero del webview: es el progreso de
-- aprendizaje del usuario y viaja con él al exportar la biblioteca.
--
-- # Compatibilidad hacia atrás
--
-- Las tarjetas existentes reciben como fecha de repaso su propia fecha de creación,
-- para que queden disponibles para repasar hoy sin perder nada.

ALTER TABLE study_cards ADD COLUMN due_at TEXT NOT NULL DEFAULT '';
UPDATE study_cards SET due_at = created_at WHERE due_at = '';
ALTER TABLE study_cards ADD COLUMN interval_days INTEGER NOT NULL DEFAULT 0 CHECK (interval_days >= 0);
ALTER TABLE study_cards ADD COLUMN ease_factor REAL NOT NULL DEFAULT 2.5 CHECK (ease_factor >= 1.3);
ALTER TABLE study_cards ADD COLUMN repetitions INTEGER NOT NULL DEFAULT 0 CHECK (repetitions >= 0);
ALTER TABLE study_cards ADD COLUMN last_reviewed_at TEXT;

CREATE INDEX idx_study_cards_due ON study_cards(due_at);

