-- AW-01: Add agent wallet support columns to exchange_accounts
-- auth_mode distinguishes CEX API-key accounts from Hyperliquid agent-wallet accounts
-- wallet_address stores the user's main ETH wallet address (for info queries + WS subscriptions)

ALTER TABLE exchange_accounts
  ADD COLUMN auth_mode VARCHAR(20) NOT NULL DEFAULT 'api_key',
  ADD COLUMN wallet_address VARCHAR(42);

-- Constraint: auth_mode must be 'api_key' or 'agent_wallet'
ALTER TABLE exchange_accounts
  ADD CONSTRAINT check_auth_mode
  CHECK (auth_mode IN ('api_key', 'agent_wallet'));

-- Constraint: agent_wallet mode requires wallet_address
ALTER TABLE exchange_accounts
  ADD CONSTRAINT check_agent_wallet_has_address
  CHECK (auth_mode != 'agent_wallet' OR wallet_address IS NOT NULL);
