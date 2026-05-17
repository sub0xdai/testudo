-- ENG-01a: Dignitas Score as living artifact.
-- Adds daily snapshot table, tunable weight config, and pill-hidden user preference.

-- Per-user daily score snapshots (upsert-safe via UNIQUE constraint).
CREATE TABLE dignitas_history (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date          DATE        NOT NULL,
    score         NUMERIC(5, 2) NOT NULL,          -- 0.00 .. 100.00
    -- Raw [0..1] input contributions stored for transparency page.
    drawdown_adherence           NUMERIC(5, 4) NOT NULL DEFAULT 0,
    risk_per_trade_consistency   NUMERIC(5, 4) NOT NULL DEFAULT 0,
    setup_adherence              NUMERIC(5, 4) NOT NULL DEFAULT 0,
    coach_severity_penalty       NUMERIC(5, 4) NOT NULL DEFAULT 0,
    journal_consistency          NUMERIC(5, 4) NOT NULL DEFAULT 0,
    -- True while fewer than 7 days of input data exist for this user.
    cold_start    BOOLEAN     NOT NULL DEFAULT FALSE,
    UNIQUE (user_id, date)
);

CREATE INDEX idx_dignitas_history_user_date
    ON dignitas_history(user_id, date DESC);

-- Tunable formula weights (FR-6): changing a row takes effect on next daily run.
-- Forward-only: existing snapshots retain the weights in effect at snapshot time.
CREATE TABLE dignitas_config (
    key         TEXT           PRIMARY KEY,
    value       NUMERIC(6, 4)  NOT NULL,
    description TEXT           NOT NULL
);

-- Seed default weights (must sum to 1.0 when all five axes are active).
-- When coach axis has no data, the four remaining weights sum to 0.80 and are
-- renormalized at snapshot time — these rows are NOT mutated for that user.
INSERT INTO dignitas_config (key, value, description) VALUES
    ('weight_drawdown_adherence',         0.2500, 'Fraction of daily score assigned to drawdown-limit adherence'),
    ('weight_risk_per_trade_consistency', 0.2000, 'Fraction assigned to risk-per-trade deviation from configured %'),
    ('weight_setup_adherence',            0.2000, 'Fraction assigned to setup-tag presence on trades (RSK-02)'),
    ('weight_coach_severity_penalty',     0.2000, 'Fraction assigned to coach-report severity (RSK-03); renormalized when absent'),
    ('weight_journal_consistency',        0.1500, 'Fraction assigned to journaling completeness (notes or linked entries)'),
    ('cold_start_min_days',               7.0000, 'Minimum days of input data before cold_start lifts and score replaces 50');

-- User preference: hide the Dignitas pill from the top nav.
ALTER TABLE users ADD COLUMN dignitas_pill_hidden BOOLEAN NOT NULL DEFAULT FALSE;
