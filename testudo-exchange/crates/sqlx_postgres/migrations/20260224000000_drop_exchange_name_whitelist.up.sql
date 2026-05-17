-- EXT-15: Drop hardcoded exchange name whitelist
-- CCXT sidecar validates exchange names dynamically; hardcoded list blocks new exchanges like WOO X
ALTER TABLE exchange_accounts DROP CONSTRAINT IF EXISTS check_exchange_name_supported;
