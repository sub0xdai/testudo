-- ENG-01c: Dignitas streak — days without a Concerning coach flag.
--
-- One row per user. `days_clean` is the active streak; `longest_ever` is
-- the trophy of the best run ever (updated only on reset).
-- `last_concerning_flag_at` gates idempotency: we never react twice to the
-- same Concerning flag.

CREATE TABLE dignitas_streak (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    days_clean BIGINT NOT NULL DEFAULT 0,
    longest_ever BIGINT NOT NULL DEFAULT 0,
    last_concerning_flag_at TIMESTAMPTZ NULL,
    streak_started_at TIMESTAMPTZ NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (days_clean >= 0),
    CHECK (longest_ever >= 0)
);
