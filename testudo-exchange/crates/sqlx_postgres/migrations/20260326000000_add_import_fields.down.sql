DROP INDEX IF EXISTS idx_unique_import_fill;
ALTER TABLE journal_trades DROP COLUMN IF EXISTS exchange_fill_id;
ALTER TABLE journal_trades DROP COLUMN IF EXISTS source;
