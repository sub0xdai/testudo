-- AUTH-03: Support Solana (base58) addresses alongside EVM (0x hex) addresses
-- Solana addresses are 32-44 chars base58, EVM are 42 chars hex

-- 1. Widen column to accommodate both address formats
ALTER TABLE users ALTER COLUMN wallet_address TYPE VARCHAR(48);

-- 2. Replace EVM-only constraint with multi-chain validation
-- Accepts: 0x-prefixed 40-char lowercase hex (EVM) OR 32-44 char base58 (Solana)
ALTER TABLE users DROP CONSTRAINT IF EXISTS check_wallet_address_format;
ALTER TABLE users ADD CONSTRAINT check_wallet_address_format
    CHECK (
        wallet_address ~ '^0x[0-9a-f]{40}$'  -- EVM: 0x + 40 hex
        OR wallet_address ~ '^[1-9A-HJ-NP-Za-km-z]{32,44}$'  -- Solana: base58, 32-44 chars
    );
