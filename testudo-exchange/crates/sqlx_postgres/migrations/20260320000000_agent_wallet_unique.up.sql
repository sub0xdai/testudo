-- FR-5: Prevent multiple active agent wallets per user+wallet at the DB level.
-- Partial unique index: only one active agent wallet per (user_id, wallet_address).
CREATE UNIQUE INDEX IF NOT EXISTS idx_unique_active_agent_wallet
ON exchange_accounts(user_id, wallet_address)
WHERE auth_mode = 'agent_wallet' AND is_active = true;
