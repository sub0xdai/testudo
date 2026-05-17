-- Wallet-primary users table migration
-- Replaces email/password auth with wallet_address as sole identity

-- Add wallet_address as new identity column
ALTER TABLE users ADD COLUMN wallet_address VARCHAR(42);
CREATE UNIQUE INDEX idx_users_wallet_address ON users(wallet_address);

-- Drop email-based auth columns and infrastructure
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
DROP INDEX IF EXISTS idx_users_email;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_email_not_empty;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_email_format;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_password_hash_not_empty;
ALTER TABLE users DROP COLUMN IF EXISTS email;
ALTER TABLE users DROP COLUMN IF EXISTS password_hash;
ALTER TABLE users DROP COLUMN IF EXISTS email_verified;

-- Make wallet_address NOT NULL after column exists
-- (existing rows must be backfilled or truncated before applying this)
ALTER TABLE users ALTER COLUMN wallet_address SET NOT NULL;

-- Wallet address format constraint (0x + 40 hex chars)
-- Lowercase-only constraint — addresses are normalized to lowercase on insert
ALTER TABLE users ADD CONSTRAINT check_wallet_address_format
    CHECK (wallet_address ~ '^0x[0-9a-f]{40}$');

-- Re-create updated_at trigger (was dropped with email infrastructure)
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
