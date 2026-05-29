/** @anchor infra:journal-lib:theme-observer
 * @tags infra */

// Shared MutationObserver for data-theme changes on <html>
// Notifies all subscribers when theme switches so charts can re-init

type ThemeCallback = (theme: string | null) => void

const listeners = new Set<ThemeCallback>()
let observer: MutationObserver | undefined

function ensureObserver() {
  if (observer || typeof document === 'undefined') return

  observer = new MutationObserver((mutations) => {
    for (const m of mutations) {
      if (m.type === 'attributes' && m.attributeName === 'data-theme') {
        const theme = document.documentElement.getAttribute('data-theme')
        listeners.forEach((cb) => cb(theme))
      }
    }
  })

  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme'],
  })
}

/**
 * Subscribe to theme changes. Returns an unsubscribe function.
 * The callback receives the new theme name (or null for default/amoled).
 */
export function onThemeChange(cb: ThemeCallback): () => void {
  ensureObserver()
  listeners.add(cb)
  return () => {
    listeners.delete(cb)
    if (listeners.size === 0 && observer) {
      observer.disconnect()
      observer = undefined
    }
  }
}

/** Read the current theme from the DOM */
export function getCurrentTheme(): string {
  if (typeof document === 'undefined') return 'amoled'
  return document.documentElement.getAttribute('data-theme') ?? 'amoled'
}
