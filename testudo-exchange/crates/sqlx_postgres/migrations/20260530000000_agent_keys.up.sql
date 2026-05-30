-- AGENT-07: Create agent_keys table for scoped agent API keys.
-- Keys are SHA-256 hashed at rest — raw key is never stored.
-- Permissions are stored as JSONB for flexible schema evolution.

CREATE TABLE agent_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    key_hash VARCHAR(64) NOT NULL UNIQUE,
    key_prefix VARCHAR(16) NOT NULL,
    permissions JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ,
    is_revoked BOOLEAN NOT NULL DEFAULT false,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX idx_agent_keys_user ON agent_keys(user_id);
CREATE INDEX idx_agent_keys_hash ON agent_keys(key_hash);
