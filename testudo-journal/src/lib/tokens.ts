// Single source of truth for all design tokens used in JS chart configs
// Reads from CSS custom properties at runtime, with fallbacks for SSR/testing

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
  return getCSSVarRGB('--signal-green', '52, 211, 153')
}

export function getSignalRed(): string {
  return getCSSVarRGB('--signal-red', '251, 113, 133')
}

export function getSignalAmber(): string {
  return getCSSVarRGB('--signal-amber', '251, 191, 36')
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
  return getCSSVarRGB('--bg-elevated', '39, 39, 42')
}

export function getBgCore(): string {
  return getCSSVarRGB('--bg-core', '9, 9, 11')
}

export function getBgPanel(): string {
  return getCSSVarRGB('--bg-panel', '24, 24, 27')
}

export function getBgHover(): string {
  return getCSSVarRGB('--bg-hover', '52, 52, 56')
}

// ── Text colors ──────────────────────────────────────────────────────

export function getTextPrimary(): string {
  return getCSSVarRGB('--text-primary', '250, 250, 250')
}

export function getTextSecondary(): string {
  return getCSSVarRGB('--text-secondary', '161, 161, 170')
}

export function getTextTertiary(): string {
  return getCSSVarRGB('--text-tertiary', '113, 113, 122')
}

// ── Border / accent ──────────────────────────────────────────────────

export function getBorder(): string {
  return getCSSVarRGB('--border', '39, 39, 42')
}

export function getAccentSteel(): string {
  return getCSSVarRGB('--accent-steel', '161, 161, 170')
}

// ── Alpha variants (read raw channels for rgba composition) ──────────

export function signalGreenAlpha(a: number): string {
  const raw = getCSSVarRaw('--signal-green', '52 211 153')
  const parts = raw.split(' ').map(Number)
  if (parts.length === 3 && parts.every((n) => !isNaN(n))) {
    return `rgba(${parts[0]}, ${parts[1]}, ${parts[2]}, ${a})`
  }
  return `rgba(52, 211, 153, ${a})`
}

export function signalRedAlpha(a: number): string {
  const raw = getCSSVarRaw('--signal-red', '251 113 133')
  const parts = raw.split(' ').map(Number)
  if (parts.length === 3 && parts.every((n) => !isNaN(n))) {
    return `rgba(${parts[0]}, ${parts[1]}, ${parts[2]}, ${a})`
  }
  return `rgba(251, 113, 133, ${a})`
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
  return getCSSVarRGB('--bg-hover', '52, 52, 56')
}

export function getCrosshairColor(): string {
  return getBorder()
}

// ── Animation timing ─────────────────────────────────────────────────

export const CLOSE_ANIMATION_MS = 200
