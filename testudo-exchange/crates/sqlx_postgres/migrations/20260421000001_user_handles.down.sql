DROP INDEX IF EXISTS idx_user_handles_handle_lower;
DROP TABLE IF EXISTS user_handles;
ALTER TABLE users DROP COLUMN IF EXISTS last_handle_change_at;
