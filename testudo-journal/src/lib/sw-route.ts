/** @anchor infra:journal-lib:sw-route
 * @tags infra */

/**
 * Pure URL-routing classifier for the journal service worker.
 *
 * The SW (`public/sw.template.js`) inlines a copy of this exact logic — service
 * workers cannot cleanly `import` from npm without an extra bundler pass, so
 * the cost of a 10-line duplicate is lower than the build complexity that
 * sharing would introduce. Keep the two copies in lock-step on changes.
 *
 * Branches (in priority order):
 *   - `?nosw=1`              → 'bypass'      — escape hatch, FR-12
 *   - pathname `/api/*`      → 'api'         — NetworkFirst+timeout, FR-9
 *   - pathname `*.woff2`     → 'font'        — CacheFirst 30d, FR-10
 *   - request mode 'navigate'→ 'navigate'    — CacheFirst shell, FR-8
 *   - else                   → 'passthrough' — no SW interception
 */

export type RequestKind = 'bypass' | 'api' | 'font' | 'navigate' | 'passthrough'

export function classifyRequest(url: string, mode: RequestMode): RequestKind {
  // Tolerant URL parsing — relative URLs from the SW need a base, but the
  // service worker always sees absolute URLs at runtime. The fallback exists
  // so unit tests can pass relative paths.
  let parsed: URL
  try {
    parsed = new URL(url, 'http://localhost')
  } catch {
    return 'passthrough'
  }
  if (parsed.searchParams.has('nosw')) return 'bypass'
  if (parsed.pathname.startsWith('/api/')) return 'api'
  if (/\.woff2$/.test(parsed.pathname)) return 'font'
  if (mode === 'navigate') return 'navigate'
  return 'passthrough'
}
