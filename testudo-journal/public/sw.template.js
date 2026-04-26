/* eslint-disable */
/**
 * Testudo journal service worker (template).
 *
 * Build pipeline: scripts/inject-sw-shell.ts reads this file at Vite
 * writeBundle and substitutes the CACHE / SHELL placeholders below with
 * the deploy version and the actual built asset paths. The resulting
 * dist/sw.js is what the browser registers.
 *
 * Strategy (mirrors src/lib/sw-route.ts — keep in sync):
 *   ?nosw=1            -> bypass (FR-12)
 *   /api/*             -> NetworkFirst + 3s timeout, fallback to cache
 *                         with sw-fallback: stale header (FR-9)
 *   *.woff2            -> CacheFirst with 30-day TTL via cached-at header (FR-10)
 *   navigate requests  -> CacheFirst against the precached shell (FR-8)
 *   anything else      -> passthrough
 *
 * Versioned cache name + skipWaiting + clients.claim ensure stale workers
 * self-evict on the next deploy (FR-13).
 */

const CACHE = '__CACHE_NAME__'
const SHELL = "[__SHELL__]"
const API_TIMEOUT_MS = 3000
const FONT_TTL_MS = 30 * 24 * 60 * 60 * 1000

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE)
      .then((cache) => cache.addAll(SHELL))
      .then(() => self.skipWaiting()),
  )
})

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  )
})

self.addEventListener('fetch', (event) => {
  const req = event.request
  const kind = classifyRequest(req.url, req.mode)
  if (kind === 'bypass' || kind === 'passthrough') return
  if (kind === 'api') {
    event.respondWith(networkFirstWithTimeout(req, API_TIMEOUT_MS))
    return
  }
  if (kind === 'font') {
    event.respondWith(cacheFirstWithTtl(req, FONT_TTL_MS))
    return
  }
  if (kind === 'navigate') {
    event.respondWith(cacheFirstShell(req))
    return
  }
})

/** Mirror of src/lib/sw-route.ts — keep in sync. */
function classifyRequest(rawUrl, mode) {
  let url
  try {
    url = new URL(rawUrl)
  } catch (_e) {
    return 'passthrough'
  }
  if (url.searchParams.has('nosw')) return 'bypass'
  if (url.pathname.startsWith('/api/')) return 'api'
  if (/\.woff2$/.test(url.pathname)) return 'font'
  if (mode === 'navigate') return 'navigate'
  return 'passthrough'
}

/**
 * NetworkFirst with timeout — race fetch vs setTimeout.
 * On success: write to cache, return fresh response.
 * On timeout / network error: serve cached response with `sw-fallback: stale`
 * header so the SPA cache layer can flag the data as stale (FR-9).
 */
async function networkFirstWithTimeout(request, timeoutMs) {
  const cache = await caches.open(CACHE)
  let timeoutId
  try {
    const fresh = await Promise.race([
      fetch(request),
      new Promise((_resolve, reject) => {
        timeoutId = setTimeout(() => reject(new Error('sw-timeout')), timeoutMs)
      }),
    ])
    clearTimeout(timeoutId)
    if (fresh && fresh.ok && request.method === 'GET') {
      // Only cache GETs — POST/PUT/DELETE responses must not be replayed.
      try { await cache.put(request, fresh.clone()) } catch (_e) { /* opaque/cors edge cases */ }
    }
    return fresh
  } catch (_err) {
    clearTimeout(timeoutId)
    const cached = await cache.match(request)
    if (cached) return withStaleHeader(cached)
    // No cache fallback — re-throw network error semantics
    return new Response('Service Worker: offline and no cached fallback', { status: 503 })
  }
}

/**
 * CacheFirst with TTL — return cached response if `cached-at` header is
 * within ttl; otherwise refetch + re-cache. Used for fonts (FR-10).
 */
async function cacheFirstWithTtl(request, ttlMs) {
  const cache = await caches.open(CACHE)
  const cached = await cache.match(request)
  if (cached) {
    const cachedAt = Number(cached.headers.get('cached-at') || '0')
    if (cachedAt && Date.now() - cachedAt < ttlMs) return cached
  }
  try {
    const fresh = await fetch(request)
    if (fresh && fresh.ok && request.method === 'GET') {
      const stamped = await stampCachedAt(fresh.clone())
      try { await cache.put(request, stamped) } catch (_e) { /* opaque resp */ }
    }
    return fresh
  } catch (_err) {
    if (cached) return cached
    return new Response('Service Worker: font fetch failed', { status: 503 })
  }
}

/**
 * CacheFirst against the precached shell — never revalidate during a page
 * navigation (FR-8). Falls through to the network if the cache miss happens
 * (e.g. a path we did not precache).
 */
async function cacheFirstShell(request) {
  const cache = await caches.open(CACHE)
  const cached = await cache.match(request) || await cache.match('/index.html') || await cache.match('/')
  if (cached) return cached
  try {
    return await fetch(request)
  } catch (_err) {
    return new Response('Service Worker: shell unavailable', { status: 503 })
  }
}

/** Clone a response with an additional `sw-fallback: stale` header. */
function withStaleHeader(response) {
  const headers = new Headers(response.headers)
  headers.set('sw-fallback', 'stale')
  return response.blob().then((body) => new Response(body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  }))
}

/** Clone a response with a `cached-at: <ms>` header for TTL bookkeeping. */
async function stampCachedAt(response) {
  const headers = new Headers(response.headers)
  headers.set('cached-at', String(Date.now()))
  const body = await response.blob()
  return new Response(body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}
