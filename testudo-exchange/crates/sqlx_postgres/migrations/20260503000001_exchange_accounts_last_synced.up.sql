-- JNL-SYNC-01 CP-3: Watermark column for incremental fill polling.
-- NULL means "never synced"; first sync pulls 90 days. Subsequent syncs are
-- incremental from this timestamp.

ALTER TABLE exchange_accounts
    ADD COLUMN last_synced_exec_time TIMESTAMPTZ NULL;
