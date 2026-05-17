-- JNL-DUR-01: pre-flight audit before applying
-- 20260502000000_journal_trades_duration_invariant.up.sql.
--
-- Run on a read-only replica (or a session with default_transaction_read_only = on).
-- The migration adds CHECK (closed_at >= opened_at) and converts duration_secs to a
-- generated column. If any rows violate the constraint, VALIDATE will fail. Use
-- the per-source breakdown below to decide whether to fix-by-swap, recompute, or
-- delete the offending rows. Keep the audit output attached to the rollout PR.

\echo == per-source summary ==
SELECT
    COALESCE(source, '(null)')                                AS source,
    exchange,
    COUNT(*)                                                  AS rows,
    COUNT(*) FILTER (WHERE closed_at < opened_at)             AS chronology_violations,
    COUNT(*) FILTER (WHERE closed_at = opened_at)             AS zero_duration,
    MIN(closed_at - opened_at)                                AS most_negative_interval,
    PERCENTILE_DISC(0.50) WITHIN GROUP (ORDER BY closed_at - opened_at) AS p50_interval
FROM journal_trades
GROUP BY source, exchange
ORDER BY chronology_violations DESC, rows DESC;

\echo
\echo == violator detail (top 200, oldest negative first) ==
SELECT id, user_id, source, exchange, symbol, side,
       opened_at, closed_at,
       (closed_at - opened_at)        AS interval,
       trade_group_id, exchange_fill_id, needs_reconciliation
FROM journal_trades
WHERE closed_at < opened_at
ORDER BY (closed_at - opened_at) ASC
LIMIT 200;

\echo
\echo == zero-duration count by source (informational, not blocking) ==
SELECT COALESCE(source, '(null)') AS source, COUNT(*) AS zero_duration_rows
FROM journal_trades
WHERE closed_at = opened_at
GROUP BY source
ORDER BY zero_duration_rows DESC;
