-- One-time backfill: populate journal_trades from managed_positions (closed trades)
-- Usage: psql 'postgres://root:root@localhost:5000/exchange-db' -f testudo-journal/backfill-from-positions.sql
--
-- Since managed_positions doesn't store exact exit price, we infer it from current_stop.
-- All 21 positions closed via stop-loss (no TP fill events recorded), so current_stop
-- is a reasonable proxy for exit price.

DO $$
DECLARE
    pos RECORD;
    exch TEXT;
    pnl NUMERIC;
    pnl_pct NUMERIC;
    margin NUMERIC;
    fee NUMERIC;
    net NUMERIC;
    risk NUMERIC;
    r_mult NUMERIC;
    dur INTEGER;
    exit_p NUMERIC;
    inserted INTEGER := 0;
    skipped INTEGER := 0;
BEGIN
    FOR pos IN
        SELECT mp.*, ea.exchange_name
        FROM managed_positions mp
        LEFT JOIN exchange_accounts ea ON ea.id = mp.exchange_account_id
        WHERE mp.state = 'closed'
        ORDER BY mp.created_at
    LOOP
        -- Determine exchange name
        exch := COALESCE(pos.exchange_name, 'hyperliquid');

        -- Skip if already backfilled (idempotent via trade_group_id = position id)
        IF EXISTS (SELECT 1 FROM journal_trades WHERE trade_group_id = pos.id) THEN
            skipped := skipped + 1;
            CONTINUE;
        END IF;

        -- Exit price: use current_stop as proxy (all positions closed via SL)
        exit_p := pos.current_stop;

        -- P&L calculation
        IF UPPER(pos.side) = 'LONG' THEN
            pnl := (exit_p - pos.entry_price) * pos.quantity;
        ELSE
            pnl := (pos.entry_price - exit_p) * pos.quantity;
        END IF;
        pnl := round(pnl, 8);

        -- Margin and PnL %
        IF pos.leverage > 0 THEN
            margin := (pos.entry_price * pos.quantity) / pos.leverage;
        ELSE
            margin := pos.entry_price * pos.quantity;
        END IF;
        IF margin > 0 THEN
            pnl_pct := round((pnl / margin) * 100, 4);
        ELSE
            pnl_pct := 0;
        END IF;

        -- Fees: estimate 0.04% of notional each way (entry + exit)
        fee := round(pos.entry_price * pos.quantity * 0.0004 * 2, 8);
        net := pnl - fee;

        -- Risk amount
        risk := abs(pos.entry_price - pos.stop_price) * pos.quantity;
        IF risk > 0 THEN
            r_mult := round(net / risk, 4);
        ELSE
            r_mult := NULL;
        END IF;

        -- Duration in seconds
        dur := EXTRACT(EPOCH FROM (pos.updated_at - pos.created_at))::INTEGER;

        INSERT INTO journal_trades (
            user_id, exchange, symbol, side,
            entry_price, exit_price, quantity, leverage,
            realized_pnl, realized_pnl_pct, fees, net_pnl,
            stop_price, target_price, risk_amount, r_multiple,
            opened_at, closed_at, duration_secs,
            trade_group_id, notes
        ) VALUES (
            pos.user_id, exch, pos.symbol, UPPER(pos.side),
            pos.entry_price, exit_p, pos.quantity, pos.leverage,
            pnl, pnl_pct, fee, net,
            pos.stop_price, pos.target_price, round(risk, 8), r_mult,
            pos.created_at, pos.updated_at, dur,
            pos.id, 'Backfilled from managed_positions'
        );

        inserted := inserted + 1;
    END LOOP;

    RAISE NOTICE 'Backfill complete: % inserted, % skipped (already existed)', inserted, skipped;

    -- Rebuild daily stats from backfilled trades
    DELETE FROM journal_daily_stats WHERE user_id IN (
        SELECT DISTINCT user_id FROM journal_trades WHERE notes = 'Backfilled from managed_positions'
    );

    INSERT INTO journal_daily_stats (
        user_id, stat_date, exchange,
        trade_count, win_count, loss_count,
        gross_profit, gross_loss, net_pnl, fees,
        cumulative_pnl, peak_cumulative_pnl, drawdown, drawdown_pct
    )
    SELECT
        user_id,
        date(closed_at) as stat_date,
        exchange,
        COUNT(*) as trade_count,
        COUNT(*) FILTER (WHERE net_pnl > 0) as win_count,
        COUNT(*) FILTER (WHERE net_pnl <= 0) as loss_count,
        COALESCE(SUM(net_pnl) FILTER (WHERE net_pnl > 0), 0) as gross_profit,
        COALESCE(ABS(SUM(net_pnl) FILTER (WHERE net_pnl <= 0)), 0) as gross_loss,
        SUM(net_pnl) as net_pnl,
        SUM(fees) as fees,
        0, 0, 0, 0  -- cumulative fields recomputed below
    FROM journal_trades
    GROUP BY user_id, date(closed_at), exchange
    ORDER BY stat_date;

    -- Recompute cumulative fields per user+exchange (two-step to avoid nested window)
    WITH step1 AS (
        SELECT id, user_id, exchange, stat_date,
            SUM(net_pnl) OVER (PARTITION BY user_id, exchange ORDER BY stat_date) as cum_pnl
        FROM journal_daily_stats
    ),
    step2 AS (
        SELECT id, cum_pnl,
            MAX(cum_pnl) OVER (PARTITION BY user_id, exchange ORDER BY stat_date) as running_peak
        FROM step1
    )
    UPDATE journal_daily_stats jds SET
        cumulative_pnl = s.cum_pnl,
        peak_cumulative_pnl = s.running_peak,
        drawdown = s.cum_pnl - s.running_peak,
        drawdown_pct = CASE
            WHEN s.running_peak > 0
            THEN (s.cum_pnl - s.running_peak) / s.running_peak * 100
            ELSE 0 END
    FROM step2 s
    WHERE jds.id = s.id;

    RAISE NOTICE 'Daily stats rebuilt: % rows',
        (SELECT COUNT(*) FROM journal_daily_stats);
END $$;
