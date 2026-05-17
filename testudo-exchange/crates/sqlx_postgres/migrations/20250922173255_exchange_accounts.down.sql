-- Add down migration script here
-- Remove exchange_accounts table and associated triggers/functions

-- Drop the trigger first
DROP TRIGGER IF EXISTS update_exchange_accounts_last_used ON exchange_accounts;

-- Drop the function
DROP FUNCTION IF EXISTS update_exchange_account_last_used();

-- Drop indexes (they will be automatically dropped when table is dropped, but explicit for clarity)
DROP INDEX IF EXISTS idx_exchange_accounts_user_id;
DROP INDEX IF EXISTS idx_exchange_accounts_exchange_name;
DROP INDEX IF EXISTS idx_exchange_accounts_is_active;
DROP INDEX IF EXISTS idx_exchange_accounts_created_at;
DROP INDEX IF EXISTS idx_exchange_accounts_last_used_at;
DROP INDEX IF EXISTS idx_exchange_accounts_user_exchange_active;

-- Drop the table (this will also drop all constraints)
DROP TABLE IF EXISTS exchange_accounts;