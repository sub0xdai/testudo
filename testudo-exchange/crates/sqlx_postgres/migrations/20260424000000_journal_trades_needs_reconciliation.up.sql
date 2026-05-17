-- FIX-08: Add needs_reconciliation flag to journal_trades.
-- Rows with this flag set TRUE have a placeholder exit_price (0) and are
-- excluded from all stat aggregations until the async reconciler patches
-- them with the real fill price from the exchange REST API.

ALTER TABLE journal_trades
    ADD COLUMN needs_reconciliation BOOLEAN NOT NULL DEFAULT FALSE;

-- Partial index: reconciliation sweep reads only the hot-set of flagged rows.
CREATE INDEX idx_journal_trades_needs_reconciliation
    ON journal_trades (user_id)
    WHERE needs_reconciliation = TRUE;
