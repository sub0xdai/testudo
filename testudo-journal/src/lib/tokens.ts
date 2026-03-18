// Single source of truth for all design tokens used in JS chart configs
// CSS variables in app.css mirror these for Tailwind utility access

// Signal colors
export const SIGNAL_GREEN = '#00FF41'
export const SIGNAL_RED = '#FF003C'
export const SIGNAL_AMBER = '#F59E0B'

// Derived rgba variants for chart fills
export const signalGreenAlpha = (a: number) => `rgba(0, 255, 65, ${a})`
export const signalRedAlpha = (a: number) => `rgba(255, 0, 60, ${a})`

// Chart background
export const CHART_BG = '#111111'

// Tag color palette — used by TagBadge, TagManager, SymbolDonut
export const TAG_PALETTE = [
  '#00FF41', '#FF003C', '#3B82F6', '#F59E0B',
  '#8B5CF6', '#EC4899', '#06B6D4', '#10B981',
]

// Entry type colors
export const ENTRY_TYPE_COLORS: Record<string, string> = {
  'note': '#94A3B8',
  'pre-trade': '#F59E0B',
  'post-trade': '#22C55E',
  'daily-review': '#888888',
  'weekly-review': '#888888',
}

// Animation timing
export const CLOSE_ANIMATION_MS = 200
