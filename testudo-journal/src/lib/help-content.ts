export const HELP: Record<string, string> = {
  // ── Dignitas Radar ──
  'radar.dignitas': 'Composite score (0–100) rating overall trading quality. Weights: Profit Factor 25%, Drawdown 20%, Avg W/L 20%, Win Rate 15%, Avg R 10%, Activity 10%.',

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
