-- TS-01 / UC-3: journal note attribution.
--
-- The raw Jev response lands in `trade_events` as an immutable audit row. These
-- columns are the queryable projection of it, so calibration, Dignitas, and the
-- coach read typed columns on their hot paths instead of parsing JSONB on every
-- run. The event log records what the model said; these columns record the
-- state code acts on.

ALTER TABLE journal_trades
    ADD COLUMN judgment_status TEXT NOT NULL DEFAULT 'not_attempted',
    ADD COLUMN judgment_model TEXT,
    ADD COLUMN judgment_at TIMESTAMPTZ,
    ADD COLUMN judgment_setup_tag TEXT,
    ADD COLUMN judgment_setup_confidence NUMERIC,
    ADD COLUMN judgment_rule_break_noul NUMERIC,
    ADD COLUMN judgment_exit_discipline NUMERIC;

-- The status is a closed set, enforced here rather than re-checked in every
-- reader.
ALTER TABLE journal_trades
    ADD CONSTRAINT journal_trades_judgment_status_check
        CHECK (judgment_status IN ('not_attempted', 'no_note', 'attributed', 'failed'));

-- `judgment_status` is the discriminant for the rest of the columns: they are
-- meaningful only when it says 'attributed' and meaningless otherwise. One
-- constraint ties the two so a partial write cannot leave a half-attributed
-- row that readers would have to defend against.
--
-- `judgment_setup_tag` is deliberately absent from the 'attributed' branch. The
-- trader's existing tags do not have to cover a novel note, and "no listed tag
-- describes this" is itself the answer. When attribution failed, the tag must
-- be NULL, which the second branch enforces.
ALTER TABLE journal_trades
    ADD CONSTRAINT journal_trades_judgment_attribution_check
        CHECK (
            (
                judgment_status = 'attributed'
                AND judgment_model IS NOT NULL
                AND judgment_at IS NOT NULL
                AND judgment_setup_confidence IS NOT NULL
                AND judgment_rule_break_noul IS NOT NULL
                AND judgment_exit_discipline IS NOT NULL
            )
            OR (
                judgment_status <> 'attributed'
                AND judgment_model IS NULL
                AND judgment_at IS NULL
                AND judgment_setup_tag IS NULL
                AND judgment_setup_confidence IS NULL
                AND judgment_rule_break_noul IS NULL
                AND judgment_exit_discipline IS NULL
            )
        );

-- Both probabilities are bounded by definition. The exit-discipline score's
-- upper bound is its level count minus one, which the code owns, so only the
-- floor is fixed here rather than duplicating a code constant in the schema.
ALTER TABLE journal_trades
    ADD CONSTRAINT journal_trades_judgment_range_check
        CHECK (
            (
                judgment_rule_break_noul IS NULL
                OR (judgment_rule_break_noul >= 0 AND judgment_rule_break_noul <= 1)
            )
            AND (
                judgment_setup_confidence IS NULL
                OR (judgment_setup_confidence >= 0 AND judgment_setup_confidence <= 1)
            )
            AND (judgment_exit_discipline IS NULL OR judgment_exit_discipline >= 0)
        );

COMMENT ON COLUMN journal_trades.judgment_status IS
    'TS-01 UC-3: not_attempted | no_note (blank notes, nothing to read)'
    ' | attributed | failed. Discriminant for the judgment_* columns.';
COMMENT ON COLUMN journal_trades.judgment_rule_break_noul IS
    'TS-01 UC-3: raw probability that the note describes a rule break.'
    ' Stored unthresholded; each consumer owns its cutoff.';
COMMENT ON COLUMN journal_trades.judgment_exit_discipline IS
    'TS-01 UC-3: probability-weighted exit discipline across the code-owned'
    ' levels. Non-integer by construction.';
