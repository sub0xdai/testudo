# Quality Checklist — CEX-02 Scaffold testudo-cex

**Spec ID:** CEX-02-scaffold-testudo-cex
**Date:** 2026-03-15

## Implementation

- [ ] `testudo-ccxt` archived via `git mv`
- [ ] `testudo-cex/` directory created with full scaffold
- [ ] `package.json` has correct dependencies (express, ws, prom-client)
- [ ] `tsconfig.json` configured for Bun runtime
- [ ] `src/server.ts` creates Express + WS server on port 3100
- [ ] `GET /health` returns `{ok: true}`
- [ ] All stub files created (gateway, handlers, ws-fills, reconciler, types, symbols, metrics)

## Verification

- [ ] `bun install` succeeds
- [ ] `bun run start` launches without errors
- [ ] `curl http://127.0.0.1:3100/health` returns `{"ok": true}`
- [ ] Git history preserved for archived sidecar
