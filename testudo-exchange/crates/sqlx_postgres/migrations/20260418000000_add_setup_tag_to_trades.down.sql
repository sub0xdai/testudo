-- RSK-02: Rollback optional setup tag columns.

ALTER TABLE managed_positions DROP COLUMN IF EXISTS setup_tag;
DROP INDEX IF EXISTS idx_journal_trades_user_setup;
ALTER TABLE journal_trades DROP COLUMN IF EXISTS setup_tag;
