-- JNL-SYNC-01 CP-3: Idempotency key for pull-sync projected trades.
-- source_fills_hash = sha256(sorted exec_ids joined by ':').
-- Partial unique index coexists with idx_unique_import_fill (HIST-02) and live-trade rows
-- (both of which have source_fills_hash IS NULL).

ALTER TABLE journal_trades
    ADD COLUMN source_fills_hash TEXT NULL;

CREATE UNIQUE INDEX idx_unique_pull_sync_trade
    ON journal_trades(user_id, exchange, source_fills_hash)
    WHERE source_fills_hash IS NOT NULL;
