-- Revert ENG-01a dignitas schema.

ALTER TABLE users DROP COLUMN IF EXISTS dignitas_pill_hidden;
DROP TABLE IF EXISTS dignitas_config;
DROP INDEX IF EXISTS idx_dignitas_history_user_date;
DROP TABLE IF EXISTS dignitas_history;
