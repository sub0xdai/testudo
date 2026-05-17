-- Add passphrase_encrypted column for exchanges that require it (OKX, KuCoin)
-- Nullable because most exchanges don't use a passphrase
ALTER TABLE exchange_accounts ADD COLUMN passphrase_encrypted BYTEA;
