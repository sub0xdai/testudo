-- ENG-01a follow-up: replace day-count cold-start gate with trade-count signal.
--
-- The original cold-start rule (`existing_count < 7` rows in dignitas_history)
-- was a weak proxy for "is this user's score statistically meaningful yet?".
-- A user with 1 trade in 30 days has more signal noise than a user with 20
-- trades in 5 days, but the day-count rule treated them identically. Worse,
-- when the daily scheduler missed midnight, the cold-start window stretched
-- artificially.
--
-- This migration:
--   1. Persists `trade_count_30d` per snapshot so the API can return it without
--      recomputing on every /dignitas/me request (high-traffic, every page).
--   2. Adds `cold_start_min_trades` (default 10) — n=10 puts standard error on
--      mean-fraction inputs at ~±15 percentage points; below that the score
--      moves more from sample noise than from behavior.
--
-- Existing rows backfill to 0; they will be flagged cold_start until the next
-- scheduler run rewrites them with the real trade count.

ALTER TABLE dignitas_history
    ADD COLUMN trade_count_30d INTEGER NOT NULL DEFAULT 0;

INSERT INTO dignitas_config (key, value, description) VALUES
    ('cold_start_min_trades', 10.0000, 'Minimum closed trades in trailing 30d before cold_start lifts')
ON CONFLICT (key) DO NOTHING;
