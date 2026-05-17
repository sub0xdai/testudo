-- ENG-01b: Dignitas public profile — handles + shareable identity.
-- Adds user_handles table for opt-in public profile claims.

CREATE TABLE user_handles (
    user_id         UUID        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    handle          TEXT        NOT NULL,
    bio             TEXT        NULL,
    show_score      BOOLEAN     NOT NULL DEFAULT FALSE,
    show_sparkline  BOOLEAN     NOT NULL DEFAULT FALSE,
    allow_indexing  BOOLEAN     NOT NULL DEFAULT FALSE,
    claimed_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Case-insensitive uniqueness — handles are normalized to lowercase on insert,
-- but the index guards against any bypass via raw SQL.
CREATE UNIQUE INDEX idx_user_handles_handle_lower ON user_handles (lower(handle));

-- 30-day rate-limit window for handle changes. Lives on users so the window
-- persists across claim → release → reclaim cycles (row deletion would reset it).
ALTER TABLE users ADD COLUMN last_handle_change_at TIMESTAMPTZ NULL;
