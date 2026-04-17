export const HELP: Record<string, string> = {
  // ── Page Explainers ──
  'page.overview': 'Your trading dashboard. Performance stats, P&L calendar, and analytical charts — all filtered by exchange, symbol, and time period.',
  'page.journal': 'Trade history with sorting, filtering, and detailed breakdowns. Click any trade to view full metrics, add notes, and tag for review.',
  'page.entries': 'Write pre-trade plans, post-trade reviews, and daily/weekly reflections. Link entries to specific trades to build a complete trading journal.',
  'page.account': 'Manage exchange connections, import trade history, and configure agent wallets for automated execution.',

  // ── Dignitas Radar ──
  'radar.dignitas': 'Your standing as a trader — measured across six virtues. Higher score, stronger formation.',

  // ── Risk Hub ──
  'risk.exposure': 'Sum of absolute notional across all open positions. Answers: how much capital is at work right now?',
  'risk.leverage': 'Net exposure divided by total margin across every venue. 2x+ amber, 5x+ red.',
  'risk.margin': 'Free margin available across all connected exchanges. The dry powder you can still deploy.',
  'risk.long_short': 'Share of notional biased long vs short. Balanced numbers = market-neutral; lopsided = directional bet.',
  'risk.pulse': 'Live aggregate of your positions across every venue. Click to jump to the Account hub.',
  'risk.positions_by_venue': 'Open positions grouped by exchange. Surfaces exchange-side state directly — covers trades placed via Testudo and manually on the venue.',
  'risk.margin_by_venue': 'Free capital per venue, sorted from most to least available. Use it to pick where to deploy your next position.',
  'risk.correlation': 'Positions grouped by asset family and direction — surfaces directional stacking (e.g., three longs on BTC/ETH/SOL trading as one BTC-beta bet).',

  // ── Charts ──
  'chart.symbol': 'Trade count by trading pair — shows where you trade most.',
  'chart.treemap': 'Rectangle area = P&L magnitude per symbol. Green = profit, red = loss.',
  'chart.expectancy': 'Average P&L per trade for each symbol. Identifies best and worst pairs.',
  'chart.daily-pnl': 'Day-by-day profit and loss bars over time.',
  'chart.cumulative': 'Running total of P&L — a rising line means consistent profitability.',
  'chart.drawdown': 'How far your account dropped from its peak at each point. Shallower = better.',
  'chart.holding': 'How trade duration affects profitability. Find your optimal holding period.',
  'chart.market': 'Your returns vs buy-and-hold. Measures if active trading adds value.',
  'chart.duration': 'Each trade plotted by duration vs return. Shows if holding longer helps.',
  'chart.return': 'Histogram of trade returns — shows the shape of your P&L distribution.',
  'chart.heatmap': 'When you trade most by hour and day of week.',
}
