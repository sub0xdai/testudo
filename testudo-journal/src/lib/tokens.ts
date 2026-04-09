// Single source of truth for all design tokens used in JS chart configs
// Reads from CSS custom properties at runtime, with fallbacks for SSR/testing
// Fallbacks synced to landing page (testudo-web) values

// ── Helpers ──────────────────────────────────────────────────────────

/** Read a CSS var and return rgb(r, g, b) string */
function getCSSVarRGB(name: string, fallback: string): string {
  if (typeof document === 'undefined') return `rgb(${fallback})`
  const val = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  if (!val) return `rgb(${fallback})`
  const parts = val.split(' ').map(Number)
  if (parts.length === 3 && parts.every((n) => !isNaN(n))) {
    return `rgb(${parts[0]}, ${parts[1]}, ${parts[2]})`
  }
  return `rgb(${fallback})`
}

/** Read a CSS var and return raw space-separated channels (for alpha compositing) */
function getCSSVarRaw(name: string, fallback: string): string {
  if (typeof document === 'undefined') return fallback
  const val = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return val || fallback
}

// ── Signal colors ────────────────────────────────────────────────────

export function getSignalGreen(): string {
  return getCSSVarRGB('--signal-green', '34, 197, 94')
}

export function getSignalRed(): string {
  return getCSSVarRGB('--signal-red', '239, 68, 68')
}

export function getSignalAmber(): string {
  return getCSSVarRGB('--signal-amber', '245, 158, 11')
}

// ── Accent colors ───────────────────────────────────────────────────

export function getAccentPrimary(): string {
  return getCSSVarRGB('--accent-primary', '196, 115, 90')
}

export function accentPrimaryAlpha(a: number): string {
  const raw = getCSSVarRaw('--accent-primary', '196 115 90')
  const parts = raw.split(' ').map(Number)
  if (parts.length === 3 && parts.every((n) => !isNaN(n))) {
    return `rgba(${parts[0]}, ${parts[1]}, ${parts[2]}, ${a})`
  }
  return `rgba(196, 115, 90, ${a})`
}

// ── Background colors ────────────────────────────────────────────────

export function getChartBg(): string {
  return getCSSVarRGB('--bg-elevated', '30, 30, 30')
}

export function getBgCore(): string {
  return getCSSVarRGB('--bg-core', '10, 10, 10')
}

export function getBgPanel(): string {
  return getCSSVarRGB('--bg-panel', '22, 22, 22')
}

export function getBgHover(): string {
  return getCSSVarRGB('--bg-hover', '40, 40, 40')
}

// ── Text colors ──────────────────────────────────────────────────────

export function getTextPrimary(): string {
  return getCSSVarRGB('--text-primary', '237, 237, 237')
}

export function getTextSecondary(): string {
  return getCSSVarRGB('--text-secondary', '190, 190, 190')
}

export function getTextTertiary(): string {
  return getCSSVarRGB('--text-tertiary', '120, 120, 120')
}

// ── Border / accent ──────────────────────────────────────────────────

export function getBorder(): string {
  return getCSSVarRGB('--border', '50, 50, 50')
}

export function getAccentSteel(): string {
  return getCSSVarRGB('--accent-steel', '148, 163, 184')
}

// ── Alpha variants (read raw channels for rgba composition) ──────────

export function signalGreenAlpha(a: number): string {
  const raw = getCSSVarRaw('--signal-green', '34 197 94')
  const parts = raw.split(' ').map(Number)
  if (parts.length === 3 && parts.every((n) => !isNaN(n))) {
    return `rgba(${parts[0]}, ${parts[1]}, ${parts[2]}, ${a})`
  }
  return `rgba(34, 197, 94, ${a})`
}

export function signalRedAlpha(a: number): string {
  const raw = getCSSVarRaw('--signal-red', '239 68 68')
  const parts = raw.split(' ').map(Number)
  if (parts.length === 3 && parts.every((n) => !isNaN(n))) {
    return `rgba(${parts[0]}, ${parts[1]}, ${parts[2]}, ${a})`
  }
  return `rgba(239, 68, 68, ${a})`
}

// ── Tag color palette ────────────────────────────────────────────────

export function getTagPalette(): string[] {
  return [
    getSignalGreen(), getSignalRed(), '#3B82F6', getSignalAmber(),
    '#8B5CF6', '#EC4899', '#06B6D4', '#10B981',
  ]
}

// ── Entry type colors ────────────────────────────────────────────────

export function getEntryTypeColors(): Record<string, string> {
  return {
    'note': getAccentSteel(),
    'pre-trade': getSignalAmber(),
    'post-trade': getSignalGreen(),
    'daily-review': getTextSecondary(),
    'weekly-review': getTextSecondary(),
  }
}

// ── Grid / axis colors for lightweight-charts ────────────────────────

export function getGridLineColor(): string {
  return getCSSVarRGB('--bg-hover', '40, 40, 40')
}

export function getCrosshairColor(): string {
  return getBorder()
}

// ── Animation timing ─────────────────────────────────────────────────

export const CLOSE_ANIMATION_MS = 200
