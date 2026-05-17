-- JNL-01: Drop journal tables in reverse dependency order
DROP TABLE IF EXISTS journal_trade_tags;
DROP TABLE IF EXISTS journal_daily_stats;
DROP TABLE IF EXISTS journal_entries;
DROP TABLE IF EXISTS journal_tags;
DROP TABLE IF EXISTS journal_trades;
