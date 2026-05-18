# CLN-01-dependency-hardening — Implementation Plan

## Current State Summary

`cargo audit` reveals 4 vulnerabilities (2 HIGH, 1 unrated, 1 MEDIUM) and 7 unmaintained/unsound warnings in `testudo-exchange`. The HIGH `alloy-dyn-abi` (CVSS 7.5) is the most critical and requires an alloy ecosystem upgrade from 0.1.4 to >=0.8.26. Other Rust vulns (`idna`, `protobuf`, `rsa`) are transitive through `validator`, `prometheus`, and `sqlx-mysql`. The 7 unmaintained warnings are mostly transitive through alloy 0.1.4 and `async-openai`.

`npm audit` in `testudo-extension` shows 2 vulnerabilities (1 HIGH vite, 1 MODERATE postcss), both fixable via `npm audit fix`.

The spec's original FR-2 (esbuild upgrade) is no longer present — the `picomatch`, `rollup`, and `esbuild` advisories mentioned in the original spec appear to have been previously resolved.

## Checkpoints

### CP-1: Transitive Dependency Bumps (`cargo update` + `npm audit fix`) ✅
- Completed 2026-05-17 by /skill:vox build
- **Touches**: `testudo-exchange/Cargo.lock`, `testudo-extension/package-lock.json`
- **Tasks**:
  1. `cd testudo-exchange && cargo update` — bump transitive deps
  2. `cd testudo-extension && npm audit fix` — patch vite and postcss
- **Verification**: `cd testudo-exchange && cargo audit` shows reduced HIGH advisories (non-alloy ones resolved); `cd testudo-extension && npm audit` shows 0 vulnerabilities
- **Commit message**: `chore: cargo update + npm audit fix transitive deps`

### CP-2: Upgrade alloy Ecosystem (0.1.4 → >=0.8.26) ✅
- Completed 2026-05-17 by /skill:vox build
- **Outcome**: Alloy 0.8 upgrade ATTEMPTED and BLOCKED by hyperliquid-sdk-rs@0.1.2 type coupling. Documented exception in `.cargo/audit.toml` per spec risk mitigation #1.

### CP-3: Address Remaining Rust Vulns and Unmaintained Crates ✅
- Completed 2026-05-17 by /skill:vox build
- **Resolved**: `protobuf` (prometheus 0.13→0.14), `idna` (validator 0.16→0.20), `backoff`/`instant` (async-openai 0.34→0.38), `rustls-pemfile` (reqwest 0.11→0.12). All remaining advisories documented in `.cargo/audit.toml`.

### CP-4: Full CI Verification (clippy + tests + build) ✅
- Completed 2026-05-17 by /skill:vox build
- All three commands pass. Acceptance criteria met.

## Risks & Open Questions

1. **hyperliquid-sdk-rs@0.1.2 pins alloy 0.1.4** — This is the primary blocker for CP-2. If the SDK hasn't been updated, we must either fork it and upgrade, use `[patch]` section to override alloy versions (may cause ABI incompatibility), or document the exception. Most unmaintained/unsound warnings also come from alloy 0.1.4 transitive deps, so CP-2 resolves many at once.

2. **`rsa 0.9.10` Marvin Attack has no fix** — This is a MEDIUM vulnerability with no available fix. The only option is to allow-list it and rely on compensating controls (network isolation, minimal attack surface for MySQL connections).

3. **`validator 0.16.1` brings `idna 0.4.0` and `proc-macro-error`** — Both are transitive. `validator` 0.18+ may resolve `idna`; `proc-macro-error` requires `validator` to migrate to `proc-macro-error2` or `manyhow`.

4. **`prometheus 0.13.4` brings `protobuf 2.28.0`** — `prometheus` 0.14+ may resolve this; needs investigation.
