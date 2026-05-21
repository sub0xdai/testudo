export const HELP: Record<string, string> = {
  // ── Page Explainers ──
  'page.overview': 'Your trading dashboard. Performance stats, P&L calendar, and analytical charts — all filtered by exchange, symbol, and time period.',
  'page.journal': 'Trade history with sorting, filtering, and detailed breakdowns. Click any trade to view full metrics, add notes, and tag for review.',
  'page.entries': 'Write pre-trade plans, post-trade reviews, and daily/weekly reflections. Link entries to specific trades to build a complete trading journal.',
  'page.account': 'Manage exchange connections, import trade history, and configure agent wallets for automated execution.',
  'page.coach': 'Weekly behavioral review. Deterministic pattern detection over your trades plus a narrated summary with trade-ID citations. Unlocks after 30 trades.',

  // ── Coach ──
  'coach.narrative': 'LLM-written analysis of this week\'s patterns. Every claim must cite a specific trade [T-xxx] — unsupported reports are rejected and replaced with stats only.',
  'coach.citations': 'Each [T-xxx] links to the trade it references. Click to inspect the exact trade the coach is calling out.',
  'coach.provider': 'Your digest (aggregated stats + flagged trades) is sent to an OpenAI-compatible provider for narration. Disable the coach to stop all outbound analysis.',
  'coach.patterns.sizing_drift': 'Position sizes after losing trades are trending larger than your baseline — a classic tell of revenge sizing.',
  'coach.patterns.frequency_spike': 'Trade frequency this week exceeded your rolling 30-day 6-hour window p90 — unusual concentration of activity.',
  'coach.patterns.session_anomaly': 'Multiple trades outside your typical active hours (top 4 UTC hours in the last 30 days).',
  'coach.patterns.setup_fatigue': 'A tagged setup\'s trailing-10 R-multiple has degraded to less than half its all-time average for that setup.',
  'coach.patterns.correlation_stack': 'Three or more same-direction positions in the same asset family held concurrently for more than 4 hours.',
  'coach.patterns.streak_risk': 'Three or more consecutive losses, or five or more consecutive wins with non-decreasing position size (pyramiding on a hot run).',

  // ── Agent Setup ──
  'coach.agent_setup': 'AI agents (Hermes, Claude, OpenClaw) can trade autonomously on Testudo. This link opens the full agent onboarding guide — authentication, signal submission, WebSocket monitoring, journal-as-memory queries, and the complete autonomous trading loop. Start in shadow (paper) mode.',

  // ── Dignitas Radar ──
  'radar.dignitas': 'Five discipline axes: drawdown, sizing, setup, coach, and journal. Higher score = stronger formation. Tap an axis on the breakdown page for its full definition.',

  // ── Dignitas Score ──
  'dignitas.pill': 'Your Dignitas score — a discipline-adherence index computed daily from five behavioral inputs. Trade frequency, P&L, and win rate are not inputs. Only how well you follow your own rules.',
  'dignitas.transparency': 'Each of the five Dignitas inputs, your current value for it, its weight in the formula, and how many points it contributes to your score.',
  'dignitas.cold_start': 'Your score is preliminary while fewer than 10 closed trades exist in the trailing 30 days. The score is computed from real inputs but flagged as preliminary because mean-fraction inputs (setup adherence, risk consistency) carry too much sample noise below n=10. Place more trades and the score firms up automatically.',
  'dignitas.inputs.drawdown': 'Fraction of trading days in the trailing 30d where daily drawdown stayed within your configured limit. Measures capital preservation discipline.',
  'dignitas.inputs.risk_consistency': 'How closely each trade\'s actual risk percentage matched your configured risk-per-trade target. Deviation from your own plan is the signal — win/loss outcome is not.',
  'dignitas.inputs.setup_adherence': 'Fraction of closed trades in the trailing 30d tagged with a named setup. Untagged trades signal reactive, plan-absent entries that bypass your own framework.',
  'dignitas.inputs.coach_alignment': 'Absence of behavioral flags in the last 4 weekly coach reports. Notable flags cost 0.5×; Concerning flags cost 1.0×. Zero flags = 100%. Silently neutral until coach reports exist.',
  'dignitas.inputs.journal_consistency': 'Fraction of closed trades with trade notes or linked journal entries. Captures execution-reflection discipline independent of outcome.',

  // ── Risk Hub ──
  'risk.exposure': 'Sum of absolute notional across all open positions. Answers: how much capital is at work right now?',
  'risk.leverage': 'Net exposure divided by total margin across every venue. 2x+ amber, 5x+ red.',
  'risk.margin': 'Free margin available across all connected exchanges. The dry powder you can still deploy.',
  'risk.correlation': 'Positions grouped by asset family and direction — surfaces directional stacking (e.g., three longs on BTC/ETH/SOL trading as one BTC-beta bet).',

  // ── Kelly Sizing ──
  'kelly.badge': 'This trade was sized by Calibrated Kelly — the position size was adjusted based on your setup\'s historical win rate and R-ratios vs your global baseline.',
  'kelly.edge_multiplier': 'How much the Calibrated Kelly engine scaled your baseline risk for this setup. Values above 1× mean the setup\'s edge is stronger than your baseline; below 1× means weaker. Clamped to [0.25×, 2.0×].',
  'kelly.n_setup': 'Number of closed trades recorded for this specific setup tag at the time of sizing. More trades → less weight given to the global prior.',
  'kelly.n_global': 'Total tagged closed trades in your global baseline at the time of sizing. This is the reference pool the setup is compared against.',
  'kelly.p_eff': 'Blended effective win rate used in the Kelly formula — a weighted average of the setup\'s raw win rate and your global prior, with heavier weight on the prior when setup sample size is small.',

  // ── Charts ──
  'chart.symbol': 'Trade count by trading pair — shows where you trade most.',
  'chart.treemap': 'Rectangle area = P&L magnitude per symbol. Green = profit, red = loss.',
  'chart.expectancy': 'Average P&L per trade for each symbol. Identifies best and worst pairs.',
  'chart.setup': 'Per-setup trade count, win rate, average R-multiple, and expectancy. Untagged trades grouped separately.',
  'chart.daily-pnl': 'Day-by-day profit and loss bars over time.',
  'chart.cumulative': 'Running total of P&L — a rising line means consistent profitability.',
  'chart.drawdown': 'How far your account dropped from its peak at each point. Shallower = better.',
  'chart.holding': 'How trade duration affects profitability. Find your optimal holding period.',
  'chart.market': 'Your returns vs buy-and-hold. Measures if active trading adds value.',
  'chart.duration': 'Each trade plotted by duration vs return. Shows if holding longer helps.',
  'chart.return': 'Histogram of trade returns — shows the shape of your P&L distribution.',
  'chart.heatmap': 'When you trade most by hour and day of week.',
}
