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

export function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m`
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h >= 24) {
    const d = Math.floor(h / 24)
    const rem = h % 24
    return rem > 0 ? `${d}d ${rem}h` : `${d}d`
  }
  return m > 0 ? `${h}h ${m}m` : `${h}h`
}

export function formatPrice(value: string | number): string {
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (isNaN(num)) return '0'
  if (num >= 1000) return num.toLocaleString('en-US', { minimumFractionDigits: 0, maximumFractionDigits: 0 })
  if (num >= 1) return num.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  return num.toLocaleString('en-US', { minimumFractionDigits: 4, maximumFractionDigits: 4 })
}

export function formatDate(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export function formatDateFull(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

export function rColor(value: string | number | null): string {
  if (value === null) return 'text-text-secondary'
  const num = typeof value === 'string' ? parseFloat(value) : value
  if (num >= 1) return 'text-signal-green'
  if (num < 0) return 'text-signal-red'
  return 'text-signal-amber'
}

export function sideColor(side: string): string {
  return side.toLowerCase() === 'long' ? 'text-signal-green' : 'text-signal-red'
}
