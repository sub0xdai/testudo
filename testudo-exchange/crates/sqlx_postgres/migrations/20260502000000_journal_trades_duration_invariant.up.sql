-- JNL-DUR-01: Make duration_secs structurally consistent with (opened_at, closed_at).
--
-- Problem: duration_secs was stored alongside its inputs and could drift, including
-- becoming negative when a writer produced closed_at < opened_at. The chart layer
-- (DurationScatter) plotted these directly, surfacing impossible "negative time" points.
--
-- Fix: enforce closed_at >= opened_at via CHECK constraint, then derive duration_secs
-- as a GENERATED column. The invalid state becomes unrepresentable; the column can no
-- longer drift from its inputs.
--
-- Pre-flight: scripts/audit_negative_durations.sql must show zero chronology_violations.
-- If any remain, fix or delete them before applying this migration. The CHECK constraint
-- below will fail to validate otherwise.

BEGIN;

-- 1. Lock-in the chronological invariant. NOT VALID first to avoid a long ACCESS
-- EXCLUSIVE under load; VALIDATE after to enforce on existing rows.
ALTER TABLE journal_trades
    ADD CONSTRAINT journal_trades_chronology
    CHECK (closed_at >= opened_at) NOT VALID;

ALTER TABLE journal_trades
    VALIDATE CONSTRAINT journal_trades_chronology;

-- 2. Replace the stored duration with a generated column derived from the timestamps.
-- Single source of truth: (opened_at, closed_at). Recomputed automatically per row.
ALTER TABLE journal_trades DROP COLUMN duration_secs;

ALTER TABLE journal_trades
    ADD COLUMN duration_secs INTEGER
    GENERATED ALWAYS AS (EXTRACT(EPOCH FROM (closed_at - opened_at))::INTEGER) STORED;

COMMIT;
