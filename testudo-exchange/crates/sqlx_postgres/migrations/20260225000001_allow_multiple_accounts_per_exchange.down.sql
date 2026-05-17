-- Re-add unique constraint (will fail if duplicate data exists)
ALTER TABLE exchange_accounts ADD CONSTRAINT exchange_accounts_user_id_exchange_name_key UNIQUE (user_id, exchange_name);
