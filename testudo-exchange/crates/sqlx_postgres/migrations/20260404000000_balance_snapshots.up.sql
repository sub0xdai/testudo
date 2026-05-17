-- JNL-13: Balance snapshots for true equity curve and accurate max drawdown.
-- Captures account equity at trade boundaries via exchange API balance fetch.

CREATE TABLE IF NOT EXISTS balance_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    exchange_account_id UUID NOT NULL REFERENCES exchange_accounts(id) ON DELETE CASCADE,
    equity NUMERIC NOT NULL,
    available NUMERIC NOT NULL DEFAULT 0,
    snapshot_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_balance_snapshots_user_time
    ON balance_snapshots(user_id, snapshot_at DESC);
CREATE INDEX idx_balance_snapshots_account
    ON balance_snapshots(exchange_account_id, snapshot_at DESC);

-- Add optional starting_balance to exchange_accounts for fallback equity calculation.
-- When no balance snapshots exist, equity = starting_balance + cumulative_pnl.
ALTER TABLE exchange_accounts
    ADD COLUMN IF NOT EXISTS starting_balance NUMERIC;
