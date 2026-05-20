-- AGENT-02 CP-4: Add agent_approved_at timestamp to exchange_accounts.
-- Tracks when an agent wallet was last approved, enabling expiry detection.
-- Agent wallet approval has a 30-day window on Hyperliquid L1.

ALTER TABLE exchange_accounts ADD COLUMN agent_approved_at TIMESTAMPTZ;
