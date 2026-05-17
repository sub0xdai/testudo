-- Allow multiple accounts per exchange (e.g., two Binance sub-accounts)
-- Drop the unique constraint on (user_id, exchange_name)
ALTER TABLE exchange_accounts DROP CONSTRAINT IF EXISTS exchange_accounts_user_id_exchange_name_key;
