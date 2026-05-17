-- QNT-01a: Calibrated Kelly sizing engine.
-- Adds per-user settings JSONB (forward-compat for QNT-01b/c preferences)
-- and per-trade kelly_inputs JSONB snapshot (audit trail for dynamic-mode trades).

CREATE TABLE IF NOT EXISTS user_settings (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    settings JSONB NOT NULL DEFAULT '{"dynamic_risk_enabled": false, "dynamic_risk_unlocked_at": null}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE journal_trades
    ADD COLUMN kelly_inputs JSONB NULL;
