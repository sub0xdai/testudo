-- JNL-SYNC-01 CP-3: Raw fills table for pull-based journal pipeline.
-- Stores one row per exchange execution fill, idempotent on (user_id, exchange, exec_id).
-- JournalSyncer upserts fills here, then calls reconstruct_trades to project round trips.

CREATE TABLE raw_fills (
    user_id     UUID            NOT NULL,
    exchange    TEXT            NOT NULL,
    exec_id     TEXT            NOT NULL,
    symbol      TEXT            NOT NULL,
    side        TEXT            NOT NULL,  -- 'Buy' | 'Sell'
    price       NUMERIC(40,18)  NOT NULL,
    qty         NUMERIC(40,18)  NOT NULL,
    fee         NUMERIC(40,18)  NOT NULL DEFAULT 0,
    fee_asset   TEXT            NOT NULL DEFAULT '',
    exec_time   TIMESTAMPTZ     NOT NULL,
    order_id    TEXT            NULL,
    raw_json    JSONB           NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, exchange, exec_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_raw_fills_user_exchange_time
    ON raw_fills(user_id, exchange, exec_time DESC);

CREATE INDEX idx_raw_fills_symbol
    ON raw_fills(user_id, exchange, symbol);
