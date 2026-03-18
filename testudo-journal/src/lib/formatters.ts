export function formatCurrency(value: string | number): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(num)) return '$0.00'
  const sign = num >= 0 ? '' : '-'
  return `${sign}$${Math.abs(num).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`
}

export function formatPercent(value: string | number): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(num)) return '0.0%'
  return `${num.toFixed(1)}%`
}

export function formatNumber(value: string | number, decimals = 2): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(num)) return '0'
  return num.toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
}

export function formatInteger(value: number): string {
  return value.toLocaleString('en-US')
}

export function pnlColor(value: string | number): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (num > 0) return 'text-signal-green'
  if (num < 0) return 'text-signal-red'
  return 'text-text-secondary'
}

export function streakSign(value: number): string {
  if (value > 0) return `+${value}`
  return `${value}`
}
