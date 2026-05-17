-- JNL-12 FR-1: B-tree index on trade_group_id for O(log n) idempotency lookups
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_journal_trades_group_id
    ON journal_trades(trade_group_id);
