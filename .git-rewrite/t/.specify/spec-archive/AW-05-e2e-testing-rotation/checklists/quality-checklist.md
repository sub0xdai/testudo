# Quality Checklist — AW-05 E2E Testing, Migration & Agent Rotation

**Spec ID:** AW-05-e2e-testing-rotation
**Date:** 2026-03-16

## Implementation

- [ ] Integration test: full init → approve → trade → cancel lifecycle
- [ ] Integration test: query_address dispatch verification
- [ ] Integration test: revocation prevents trading
- [ ] Migration endpoint: direct-key → agent-wallet conversion
- [ ] Migration: AuthCache invalidation on migration
- [ ] Revocation endpoint: deactivate + record timestamp
- [ ] `AgentRotationService` TTL tracking
- [ ] WebSocket notification for approaching TTL
- [ ] Feature flag: `HYPERLIQUID_AGENT_WALLET_ENABLED` gating
- [ ] `ExchangeAccountResponse` updated with auth_mode + wallet_address
- [ ] Migrate/revoke request/response types
- [ ] Frontend: revoke button + migration prompt

## Verification

- [ ] `cargo clippy --all-targets` passes with zero errors
- [ ] `cargo test` passes with zero failures
- [ ] `cd testudo-web && bun run build` passes
- [ ] Integration tests compile (run manually with testnet credentials)
- [ ] Feature flag off = agent-wallet routes return 404
