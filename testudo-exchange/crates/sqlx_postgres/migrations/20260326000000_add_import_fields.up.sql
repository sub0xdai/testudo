-- HIST-01: Add source tracking and deduplication fields for trade history import

ALTER TABLE journal_trades ADD COLUMN source TEXT NOT NULL DEFAULT 'testudo';
ALTER TABLE journal_trades ADD COLUMN exchange_fill_id BIGINT;

-- Partial unique index for import deduplication
-- Only applies to rows with a non-null exchange_fill_id (imported trades)
CREATE UNIQUE INDEX idx_unique_import_fill
    ON journal_trades(user_id, exchange, exchange_fill_id)
    WHERE exchange_fill_id IS NOT NULL;
