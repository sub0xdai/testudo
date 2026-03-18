-- Seed script for testudo-journal development
-- Usage: psql $DATABASE_URL -f seed-dev-data.sql
-- Requires: a valid user_id in the users table
--
-- Replace YOUR_USER_ID below with your actual user UUID from the users table.
-- To find your user_id: SELECT id, email FROM users;

DO $$
DECLARE
    uid UUID;
    i INTEGER;
    sym TEXT;
    exch TEXT;
    side TEXT;
    entry_p NUMERIC;
    exit_p NUMERIC;
    qty NUMERIC;
    pnl NUMERIC;
    fee NUMERIC;
    net NUMERIC;
    stop NUMERIC;
    target NUMERIC;
    risk NUMERIC;
    r_mult NUMERIC;
    open_ts TIMESTAMPTZ;
    close_ts TIMESTAMPTZ;
    dur INTEGER;
    symbols TEXT[] := ARRAY['BTC/USDT', 'ETH/USDT', 'SOL/USDT', 'DOGE/USDT', 'AVAX/USDT', 'LINK/USDT', 'ARB/USDT', 'OP/USDT'];
    exchanges TEXT[] := ARRAY['hyperliquid', 'woo'];
    base_prices NUMERIC[] := ARRAY[87000, 3200, 140, 0.18, 35, 15, 1.2, 2.5];
    cur_date DATE;
    daily_pnl NUMERIC;
    daily_wins INTEGER;
    daily_losses INTEGER;
    daily_gross_profit NUMERIC;
    daily_gross_loss NUMERIC;
    daily_fees NUMERIC;
    cumul_pnl NUMERIC := 0;
    peak_pnl NUMERIC := 0;
BEGIN
    -- Get the first user from the users table
    SELECT id INTO uid FROM users LIMIT 1;
    IF uid IS NULL THEN
        RAISE EXCEPTION 'No users found. Create a user first via testudo-web login.';
    END IF;
    RAISE NOTICE 'Seeding journal data for user %', uid;

    -- Clean existing journal data for this user
    DELETE FROM journal_trade_tags WHERE trade_id IN (SELECT id FROM journal_trades WHERE user_id = uid);
    DELETE FROM journal_entries WHERE user_id = uid;
    DELETE FROM journal_tags WHERE user_id = uid;
    DELETE FROM journal_daily_stats WHERE user_id = uid;
    DELETE FROM journal_trades WHERE user_id = uid;

    -- Create tags
    INSERT INTO journal_tags (user_id, name, color) VALUES
        (uid, 'breakout', '#22C55E'),
        (uid, 'fade', '#EF4444'),
        (uid, 'scalp', '#F59E0B'),
        (uid, 'swing', '#3B82F6'),
        (uid, 'news', '#A855F7');

    -- Generate 120 trades over the past 90 days
    FOR i IN 1..120 LOOP
        -- Pick random symbol and exchange
        sym := symbols[1 + floor(random() * array_length(symbols, 1))::int];
        exch := exchanges[1 + floor(random() * array_length(exchanges, 1))::int];
        side := CASE WHEN random() > 0.45 THEN 'long' ELSE 'short' END;

        -- Base price for the symbol
        entry_p := base_prices[array_position(symbols, sym)];
        -- Add noise: +/- 5%
        entry_p := entry_p * (0.95 + random() * 0.10);

        -- Win/loss: 55% win rate
        IF random() < 0.55 THEN
            -- Win: 0.5% to 4% move in favorable direction
            IF side = 'long' THEN
                exit_p := entry_p * (1.005 + random() * 0.035);
            ELSE
                exit_p := entry_p * (0.965 + random() * 0.035);
            END IF;
        ELSE
            -- Loss: 0.3% to 2.5% adverse move
            IF side = 'long' THEN
                exit_p := entry_p * (0.975 + random() * 0.025);
            ELSE
                exit_p := entry_p * (1.003 + random() * 0.025);
            END IF;
        END IF;

        -- Quantity based on ~$2000 notional
        qty := round(2000.0 / entry_p, 4);
        IF qty < 0.001 THEN qty := 0.001; END IF;

        -- P&L
        IF side = 'long' THEN
            pnl := (exit_p - entry_p) * qty;
        ELSE
            pnl := (entry_p - exit_p) * qty;
        END IF;
        pnl := round(pnl, 2);

        -- Fees: 0.04% of notional each way
        fee := round(entry_p * qty * 0.0004 * 2, 2);
        net := pnl - fee;

        -- Stop and target
        IF side = 'long' THEN
            stop := round(entry_p * 0.985, 2);
            target := round(entry_p * 1.03, 2);
        ELSE
            stop := round(entry_p * 1.015, 2);
            target := round(entry_p * 0.97, 2);
        END IF;

        risk := round(abs(entry_p - stop) * qty, 2);
        IF risk > 0 THEN
            r_mult := round(net / risk, 2);
        ELSE
            r_mult := 0;
        END IF;

        -- Timing: spread across last 90 days, random hour
        open_ts := NOW() - ((random() * 89 + 1)::int || ' days')::interval
                        - ((random() * 12)::int || ' hours')::interval;
        -- Duration: 5 minutes to 48 hours
        dur := (300 + floor(random() * 172800))::int;
        close_ts := open_ts + (dur || ' seconds')::interval;

        INSERT INTO journal_trades (
            user_id, exchange, symbol, side,
            entry_price, exit_price, quantity, leverage,
            realized_pnl, realized_pnl_pct, fees, net_pnl,
            stop_price, target_price, risk_amount, r_multiple,
            opened_at, closed_at, duration_secs, notes
        ) VALUES (
            uid, exch, sym, side,
            round(entry_p, 2), round(exit_p, 2), qty, CASE WHEN random() > 0.5 THEN 5 ELSE 10 END,
            pnl, round(pnl / (entry_p * qty) * 100, 2), fee, net,
            stop, target, risk, r_mult,
            open_ts, close_ts, dur,
            CASE WHEN random() > 0.7 THEN 'Good setup, clean execution.' ELSE NULL END
        );
    END LOOP;

    -- Populate journal_daily_stats from the trades
    FOR cur_date IN
        SELECT DISTINCT date(closed_at)
        FROM journal_trades
        WHERE user_id = uid
        ORDER BY date(closed_at)
    LOOP
        FOR exch IN SELECT DISTINCT t.exchange FROM journal_trades t WHERE t.user_id = uid AND date(t.closed_at) = cur_date
        LOOP
            SELECT
                COUNT(*),
                COALESCE(COUNT(*) FILTER (WHERE net_pnl > 0), 0),
                COALESCE(COUNT(*) FILTER (WHERE net_pnl <= 0), 0),
                COALESCE(SUM(net_pnl) FILTER (WHERE net_pnl > 0), 0),
                COALESCE(ABS(SUM(net_pnl) FILTER (WHERE net_pnl <= 0)), 0),
                COALESCE(SUM(fees), 0),
                COALESCE(SUM(net_pnl), 0)
            INTO
                daily_wins, -- reusing as trade_count temporarily
                daily_losses, -- reusing...
                i, -- throwaway
                daily_gross_profit,
                daily_gross_loss,
                daily_fees,
                daily_pnl
            FROM journal_trades
            WHERE user_id = uid AND date(closed_at) = cur_date AND exchange = exch;

            cumul_pnl := cumul_pnl + daily_pnl;
            IF cumul_pnl > peak_pnl THEN
                peak_pnl := cumul_pnl;
            END IF;

            INSERT INTO journal_daily_stats (
                user_id, stat_date, exchange,
                trade_count, win_count, loss_count,
                gross_profit, gross_loss, net_pnl, fees,
                cumulative_pnl, peak_cumulative_pnl, drawdown, drawdown_pct
            ) VALUES (
                uid, cur_date, exch,
                daily_wins, daily_losses, i,
                daily_gross_profit, daily_gross_loss, daily_pnl, daily_fees,
                cumul_pnl, peak_pnl,
                cumul_pnl - peak_pnl,
                CASE WHEN peak_pnl > 0 THEN round((cumul_pnl - peak_pnl) / peak_pnl * 100, 2) ELSE 0 END
            );
        END LOOP;
    END LOOP;

    -- Link some trades to tags randomly
    INSERT INTO journal_trade_tags (trade_id, tag_id)
    SELECT t.id, tag.id
    FROM journal_trades t
    CROSS JOIN journal_tags tag
    WHERE t.user_id = uid AND tag.user_id = uid AND random() < 0.15
    ON CONFLICT DO NOTHING;

    RAISE NOTICE 'Seeded % trades, % daily stats rows, and random tag links',
        (SELECT COUNT(*) FROM journal_trades WHERE user_id = uid),
        (SELECT COUNT(*) FROM journal_daily_stats WHERE user_id = uid);
END $$;
