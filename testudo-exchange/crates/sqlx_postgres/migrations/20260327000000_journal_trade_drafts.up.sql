CREATE TABLE journal_trade_drafts (
    trade_group_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_journal_drafts_user ON journal_trade_drafts(user_id);
