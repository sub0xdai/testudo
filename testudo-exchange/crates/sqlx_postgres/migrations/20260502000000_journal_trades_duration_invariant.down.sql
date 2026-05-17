-- Revert JNL-DUR-01: restore duration_secs as a plain stored column and drop the
-- chronology constraint. Values are seeded from the current (opened_at, closed_at)
-- pair so the rollback preserves the post-migration values.

BEGIN;

ALTER TABLE journal_trades DROP COLUMN duration_secs;

ALTER TABLE journal_trades ADD COLUMN duration_secs INTEGER;

UPDATE journal_trades
SET duration_secs = EXTRACT(EPOCH FROM (closed_at - opened_at))::INTEGER;

ALTER TABLE journal_trades ALTER COLUMN duration_secs SET NOT NULL;

ALTER TABLE journal_trades DROP CONSTRAINT journal_trades_chronology;

COMMIT;
