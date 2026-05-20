-- AGENT-01 CP-2: Add agent attribution columns to journal_trades.
-- reasoning: free-text explanation of the agent's signal rationale.
-- confidence: 0.00–1.00 for future Kelly criterion calibration.
-- source already exists (added in 20260326000000_add_import_fields).

ALTER TABLE journal_trades ADD COLUMN reasoning TEXT;
ALTER TABLE journal_trades ADD COLUMN confidence NUMERIC(3,2);
