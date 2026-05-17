DROP INDEX IF EXISTS idx_unique_pull_sync_trade;

ALTER TABLE journal_trades
    DROP COLUMN IF EXISTS source_fills_hash;
