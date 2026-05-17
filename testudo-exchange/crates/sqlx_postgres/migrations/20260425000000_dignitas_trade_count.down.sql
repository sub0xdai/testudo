DELETE FROM dignitas_config WHERE key = 'cold_start_min_trades';
ALTER TABLE dignitas_history DROP COLUMN trade_count_30d;
