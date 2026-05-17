-- Add down migration script here
-- Remove users table and associated triggers/functions

-- Drop the trigger first
DROP TRIGGER IF EXISTS update_users_updated_at ON users;

-- Drop the function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop indexes (they will be automatically dropped when table is dropped, but explicit for clarity)
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_users_active;
DROP INDEX IF EXISTS idx_users_created_at;

-- Drop the table
DROP TABLE IF EXISTS users;