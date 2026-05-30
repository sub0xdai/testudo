-- AGENT-07 CP-4: Add agent_key_id audit trail columns.
-- Links trades and journal entries to the specific agent key that created them.

ALTER TABLE trade_groups ADD COLUMN IF NOT EXISTS agent_key_id UUID REFERENCES agent_keys(id);
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS agent_key_id UUID REFERENCES agent_keys(id);

CREATE INDEX IF NOT EXISTS idx_trade_groups_agent_key ON trade_groups(agent_key_id);
CREATE INDEX IF NOT EXISTS idx_journal_entries_agent_key ON journal_entries(agent_key_id);
