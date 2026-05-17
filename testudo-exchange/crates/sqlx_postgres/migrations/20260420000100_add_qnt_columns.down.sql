-- QNT-01a: Rollback calibrated Kelly sizing engine schema.

ALTER TABLE journal_trades DROP COLUMN IF EXISTS kelly_inputs;
DROP TABLE IF EXISTS user_settings;
