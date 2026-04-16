export const HELP: Record<string, string> = {
  // ── Overview Hero ──
  'hero.netPnl': 'Total profit minus losses and all fees — your true bottom line.',
  'hero.balance': 'Current account value across all connected exchanges.',

  // ── Account Stats ──
  'stat.totalPnl': 'Sum of all realized profits and losses before fees.',
  'stat.netPnl': 'Total P&L after subtracting all trading fees.',
  'stat.fees': 'Total exchange fees paid across all trades.',
  'stat.trades': 'Total number of closed trades.',

  // ── Performance Stats ──
  'stat.winRate': 'Percentage of trades that closed in profit.',
  'stat.profitFactor': 'Gross profit ÷ gross loss. Above 1.0 = profitable. Above 2.0 = strong.',
  'stat.expectancy': 'Average P&L per trade. Positive means your strategy has an edge.',
  'stat.rMultiple': 'Average return in units of risk. 1R = made what you risked. 2R = double.',
  'stat.tradesPerDay': 'Average trades closed per day over the selected period.',

  // ── Risk Stats ──
  'stat.maxDd': 'Largest peak-to-trough decline in account value. Lower is better.',
  'stat.worstDay': 'Biggest single-day loss in the selected period.',
  'stat.worstWeek': 'Biggest single-week loss in the selected period.',
  'stat.streak': 'Current consecutive wins (+) or losses (−).',
  'stat.bestStreak': 'Longest consecutive winning streak achieved.',

  // ── Dignitas Radar ──
  'radar.dignitas': 'Composite score (0–100) rating overall trading quality. Weights: Profit Factor 25%, Drawdown 20%, Avg W/L 20%, Win Rate 15%, Avg R 10%, Activity 10%.',

  // ── P&L Calendar ──
  'calendar': 'Daily P&L heatmap. Color intensity shows magnitude. Click a day to view its trades.',

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

  // ── Trade Table Columns ──
  'col.r': 'R-Multiple: profit measured in units of initial risk. +2R = gained twice what you risked.',
  'col.duration': 'Time elapsed from entry fill to exit fill.',
  'col.netPnl': 'Profit or loss after all exchange fees.',
  'col.exch': 'Exchange where the trade executed.',

  // ── Trade Detail ──
  'detail.stop': 'Price where your stop-loss order limits downside.',
  'detail.target': 'Price where your take-profit order locks in gains.',
  'detail.rMultiple': 'Net P&L ÷ risk amount. How many R this trade returned.',
  'detail.return': 'Percentage return on the position.',
  'detail.leverage': 'Position size multiplier. 10× means $100 controls $1,000 of asset.',

  // ── Account Page ──
  'account.cexDex': 'CEX = centralized exchange (API keys). DEX = decentralized (wallet signature).',
  'account.reauth': 'Agent wallet authorization expired. Re-sign to resume trading.',
  'account.agentWallet': 'A sub-wallet authorized to trade on your behalf without exposing your main keys.',
}
