-- 019f: Append-only trade event log for financial-grade audit trail
CREATE TABLE trade_events (
    seq         BIGSERIAL PRIMARY KEY,
    event_type  TEXT NOT NULL,
    group_id    UUID,
    user_id     UUID NOT NULL,
    symbol      TEXT,
    payload     JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_trade_events_group ON trade_events (group_id, seq);
CREATE INDEX idx_trade_events_user  ON trade_events (user_id, created_at);
