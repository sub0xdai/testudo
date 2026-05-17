-- Revert wallet-primary users table migration

-- Drop wallet infrastructure
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_wallet_address_format;
DROP INDEX IF EXISTS idx_users_wallet_address;
ALTER TABLE users DROP COLUMN IF EXISTS wallet_address;

-- Restore email-based auth columns
ALTER TABLE users ADD COLUMN email VARCHAR(255);
ALTER TABLE users ADD COLUMN password_hash VARCHAR(255);
ALTER TABLE users ADD COLUMN email_verified BOOLEAN DEFAULT FALSE;
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- Re-create updated_at trigger
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
