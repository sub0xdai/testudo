-- Revert to EVM-only wallet address format
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_wallet_address_format;
ALTER TABLE users ALTER COLUMN wallet_address TYPE VARCHAR(42);
ALTER TABLE users ADD CONSTRAINT check_wallet_address_format
    CHECK (wallet_address ~ '^0x[0-9a-f]{40}$');
