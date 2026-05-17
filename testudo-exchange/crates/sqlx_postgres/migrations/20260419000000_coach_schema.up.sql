-- RSK-03: AI Trade Coach — weekly behavioral reports.
-- Adds opt-in + banner-viewed tracking to users, and a write-once per-week
-- coach_reports archive.

ALTER TABLE users ADD COLUMN coach_enabled BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users ADD COLUMN coach_banner_last_viewed_at TIMESTAMPTZ NULL;

CREATE TABLE coach_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    week_start TIMESTAMPTZ NOT NULL,
    week_end TIMESTAMPTZ NOT NULL,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    model_used TEXT NOT NULL,
    headline TEXT NULL,
    narrative_sections_json JSONB NULL,
    digest_json JSONB NOT NULL,
    cache_hit_ratio NUMERIC(4, 3) NULL,
    banner_dismissed_at TIMESTAMPTZ NULL,
    UNIQUE (user_id, week_start)
);

CREATE INDEX idx_coach_reports_user_generated
    ON coach_reports(user_id, generated_at DESC);
