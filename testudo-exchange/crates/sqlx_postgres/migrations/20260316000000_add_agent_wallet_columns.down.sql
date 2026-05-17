-- Reverse AW-01: Remove agent wallet columns from exchange_accounts

ALTER TABLE exchange_accounts
  DROP CONSTRAINT IF EXISTS check_agent_wallet_has_address,
  DROP CONSTRAINT IF EXISTS check_auth_mode;

ALTER TABLE exchange_accounts
  DROP COLUMN IF EXISTS wallet_address,
  DROP COLUMN IF EXISTS auth_mode;
