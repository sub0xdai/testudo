-- JNL-01: Journal database schema
-- Creates journal_trades, journal_entries, journal_tags, journal_trade_tags, journal_daily_stats

CREATE TABLE journal_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    exchange TEXT NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL,
    entry_price NUMERIC NOT NULL,
    exit_price NUMERIC NOT NULL,
    quantity NUMERIC NOT NULL,
    leverage INTEGER NOT NULL DEFAULT 1,

    -- P&L
    realized_pnl NUMERIC NOT NULL,
    realized_pnl_pct NUMERIC NOT NULL,
    fees NUMERIC NOT NULL DEFAULT 0,
    net_pnl NUMERIC NOT NULL,

    -- Risk metrics
    stop_price NUMERIC,
    target_price NUMERIC,
    risk_amount NUMERIC,
    r_multiple NUMERIC,

    -- Timing
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ NOT NULL,
    duration_secs INTEGER NOT NULL,

    -- Linkage
    trade_group_id UUID,
    exchange_order_ids TEXT[],

    -- Metadata
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journal_trades_user_time ON journal_trades(user_id, closed_at DESC);
CREATE INDEX idx_journal_trades_user_symbol ON journal_trades(user_id, symbol);
CREATE INDEX idx_journal_trades_user_exchange ON journal_trades(user_id, exchange);

CREATE TABLE journal_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    trade_id UUID REFERENCES journal_trades(id) ON DELETE SET NULL,
    entry_date DATE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    entry_type TEXT NOT NULL DEFAULT 'note',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journal_entries_user ON journal_entries(user_id, created_at DESC);
CREATE INDEX idx_journal_entries_trade ON journal_entries(trade_id);

CREATE TABLE journal_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    color TEXT,
    UNIQUE(user_id, name)
);

CREATE TABLE journal_trade_tags (
    trade_id UUID NOT NULL REFERENCES journal_trades(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES journal_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (trade_id, tag_id)
);

CREATE TABLE journal_daily_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    stat_date DATE NOT NULL,
    exchange TEXT,

    trade_count INTEGER NOT NULL DEFAULT 0,
    win_count INTEGER NOT NULL DEFAULT 0,
    loss_count INTEGER NOT NULL DEFAULT 0,
    gross_profit NUMERIC NOT NULL DEFAULT 0,
    gross_loss NUMERIC NOT NULL DEFAULT 0,
    net_pnl NUMERIC NOT NULL DEFAULT 0,
    fees NUMERIC NOT NULL DEFAULT 0,

    -- Running totals (for equity curve)
    cumulative_pnl NUMERIC NOT NULL DEFAULT 0,
    peak_cumulative_pnl NUMERIC NOT NULL DEFAULT 0,
    drawdown NUMERIC NOT NULL DEFAULT 0,
    drawdown_pct NUMERIC NOT NULL DEFAULT 0,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, stat_date, exchange)
);

CREATE INDEX idx_journal_daily_user ON journal_daily_stats(user_id, stat_date);
