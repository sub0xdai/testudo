-- AGENT-07 CP-4: rollback agent_key_id audit trail columns.
-- Mirrors the guard in the up migration: `trade_groups` does not exist in
-- every deployed database, so an unguarded DROP aborts the revert.

ALTER TABLE journal_entries DROP COLUMN IF EXISTS agent_key_id;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'trade_groups') THEN
        ALTER TABLE trade_groups DROP COLUMN IF EXISTS agent_key_id;
    END IF;
END $$;
