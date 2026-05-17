DROP INDEX IF EXISTS idx_journal_trades_needs_reconciliation;

ALTER TABLE journal_trades
    DROP COLUMN IF EXISTS needs_reconciliation;
