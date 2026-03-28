# Specification: Extension Production URLs

**Spec ID:** DEPLOY-03-extension-prod-urls
**Date:** 2026-03-28
**Status:** Complete
**Class:** Configuration
**Priority:** P1 — Extension can't connect to production backend.
**Depends on:** DEPLOY-01-production-hosting (deployed)

---

## Problem Statement

The browser extension has hardcoded localhost URLs in `src/utils.ts`:

```typescript
export const DEFAULT_SETTINGS: Settings = {
  backendUrl: "http://localhost:8080",
  wsUrl: "ws://127.0.0.1:4000",
};

export const WEB_APP_URL = "http://localhost:3001";
```

The extension needs to target production URLs (`api.testudo.vip`, `ws.testudo.vip`, `testudo.vip`) when built for Chrome Web Store distribution, while keeping localhost defaults for local development.

---

## Implementation

### T1: Build-time environment variables

Update `src/utils.ts` to use build-time env vars with localhost fallbacks:

```typescript
export const DEFAULT_SETTINGS: Settings = {
  backendUrl: process.env.BACKEND_URL || "http://localhost:8080",
  wsUrl: process.env.WS_URL || "ws://127.0.0.1:4000",
};

export const WEB_APP_URL = process.env.WEB_APP_URL || "http://localhost:3001";
```

### T2: Configure esbuild define

Update `esbuild.config.js` (or equivalent build config) to inject env vars at build time:

```javascript
define: {
  'process.env.BACKEND_URL': JSON.stringify(process.env.BACKEND_URL || ''),
  'process.env.WS_URL': JSON.stringify(process.env.WS_URL || ''),
  'process.env.WEB_APP_URL': JSON.stringify(process.env.WEB_APP_URL || ''),
}
```

### T3: Add production build script

Add to `package.json`:
```json
"build:prod": "BACKEND_URL=https://api.testudo.vip WS_URL=wss://ws.testudo.vip WEB_APP_URL=https://testudo.vip bun run build"
```

### T4: Update tests

Update test files that assert against localhost URLs to use the same env var pattern or match expected defaults.

### T5: Verify

- `bun run build` (dev) — defaults to localhost
- `bun run build:prod` — uses production URLs
- Load unpacked extension from prod build, verify it connects to `api.testudo.vip`

---

## Acceptance Criteria

- [ ] Dev build (`bun run build`) uses localhost defaults
- [ ] Prod build (`bun run build:prod`) uses production URLs
- [ ] Extension connects to `api.testudo.vip` with prod build
- [ ] WebSocket connects to `wss://ws.testudo.vip` with prod build
- [ ] "LAUNCH DESK" / web app links point to `testudo.vip`
- [ ] `bun run build` passes for both modes
