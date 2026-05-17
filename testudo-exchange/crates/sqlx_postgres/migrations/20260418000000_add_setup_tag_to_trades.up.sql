-- RSK-02: Optional setup tag captured at Alt+X entry time.
-- Persisted on the live managed_positions record and on the closed journal_trades record.

ALTER TABLE journal_trades ADD COLUMN setup_tag TEXT NULL;

CREATE INDEX idx_journal_trades_user_setup
    ON journal_trades(user_id, setup_tag)
    WHERE setup_tag IS NOT NULL;

ALTER TABLE managed_positions ADD COLUMN setup_tag TEXT NULL;
