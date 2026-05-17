-- JNL-SYNC-01 CP-6: One-time heal — clear any rows stuck with needs_reconciliation = TRUE
-- before we drop the filter clauses. Prevents reconciling rows from appearing in
-- aggregations the moment the WHERE filters are removed.
UPDATE journal_trades
   SET needs_reconciliation = FALSE
 WHERE needs_reconciliation = TRUE;
