-- TS-01 / UC-3: rollback journal note attribution.
--
-- Constraints first: dropping a column a constraint depends on would take the
-- constraint with it and make the remaining drops ambiguous.

ALTER TABLE journal_trades
    DROP CONSTRAINT IF EXISTS journal_trades_judgment_range_check,
    DROP CONSTRAINT IF EXISTS journal_trades_judgment_attribution_check,
    DROP CONSTRAINT IF EXISTS journal_trades_judgment_status_check;

ALTER TABLE journal_trades
    DROP COLUMN IF EXISTS judgment_exit_discipline,
    DROP COLUMN IF EXISTS judgment_rule_break_noul,
    DROP COLUMN IF EXISTS judgment_setup_confidence,
    DROP COLUMN IF EXISTS judgment_setup_tag,
    DROP COLUMN IF EXISTS judgment_at,
    DROP COLUMN IF EXISTS judgment_model,
    DROP COLUMN IF EXISTS judgment_status;
