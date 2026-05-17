-- Add up migration script here
-- Create exchange_accounts table for CEX API key management system

-- Ensure UUID extension is available (should already be enabled from users migration)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS exchange_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    exchange_name VARCHAR(50) NOT NULL,
    api_key_encrypted BYTEA NOT NULL,
    api_secret_encrypted BYTEA NOT NULL,
    permissions JSONB DEFAULT '{}',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,

    -- Ensure one account per user per exchange
    UNIQUE(user_id, exchange_name)
);

-- Create indexes for performance optimization
CREATE INDEX IF NOT EXISTS idx_exchange_accounts_user_id ON exchange_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_exchange_accounts_exchange_name ON exchange_accounts(exchange_name);
CREATE INDEX IF NOT EXISTS idx_exchange_accounts_is_active ON exchange_accounts(is_active);
CREATE INDEX IF NOT EXISTS idx_exchange_accounts_created_at ON exchange_accounts(created_at);
CREATE INDEX IF NOT EXISTS idx_exchange_accounts_last_used_at ON exchange_accounts(last_used_at);

-- Composite index for common lookup pattern (user + exchange + active status)
CREATE INDEX IF NOT EXISTS idx_exchange_accounts_user_exchange_active
    ON exchange_accounts(user_id, exchange_name, is_active);

-- Add constraints for data integrity
ALTER TABLE exchange_accounts ADD CONSTRAINT check_exchange_name_not_empty
    CHECK (length(trim(exchange_name)) > 0);

ALTER TABLE exchange_accounts ADD CONSTRAINT check_exchange_name_lowercase
    CHECK (exchange_name = lower(exchange_name));

ALTER TABLE exchange_accounts ADD CONSTRAINT check_exchange_name_supported
    CHECK (exchange_name IN (
        'binance', 'coinbase', 'coinbase_pro', 'kraken', 'bitstamp',
        'bitfinex', 'huobi', 'okx', 'kucoin', 'bybit'
    ));

ALTER TABLE exchange_accounts ADD CONSTRAINT check_api_key_not_empty
    CHECK (length(api_key_encrypted) > 0);

ALTER TABLE exchange_accounts ADD CONSTRAINT check_api_secret_not_empty
    CHECK (length(api_secret_encrypted) > 0);

ALTER TABLE exchange_accounts ADD CONSTRAINT check_permissions_is_object
    CHECK (jsonb_typeof(permissions) = 'object');

-- Create trigger to automatically update last_used_at when credentials are accessed
-- Note: This would typically be updated by application logic, but having the infrastructure ready
CREATE OR REPLACE FUNCTION update_exchange_account_last_used()
RETURNS TRIGGER AS $$
BEGIN
    -- Only update last_used_at if the encrypted fields are being read (simulated by any update)
    -- In practice, this would be triggered by application-level credential access
    IF TG_OP = 'UPDATE' AND OLD.last_used_at IS DISTINCT FROM NEW.last_used_at THEN
        -- Allow explicit last_used_at updates from application
        RETURN NEW;
    END IF;

    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create trigger for the function (placeholder for future enhancement)
CREATE TRIGGER update_exchange_accounts_last_used
    BEFORE UPDATE ON exchange_accounts
    FOR EACH ROW
    EXECUTE FUNCTION update_exchange_account_last_used();