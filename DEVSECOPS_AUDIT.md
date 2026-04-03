# DevSecOps Audit Report

**Project:** `/home/m0xu/1-projects/testudo`
**Scan Date:** 2026-04-03T00:16:58Z
**Detected Stacks:** Rust (testudo-exchange), Node.js (testudo-extension, testudo-web)

---

## Emerging Threats (Latest CVEs)

### Rust — testudo-exchange (12 vulnerabilities + 9 unmaintained warnings)

| Advisory ID | CVSS | Severity | Crate | Installed | Patched | Description |
|-------------|------|----------|-------|-----------|---------|-------------|
| RUSTSEC-2025-0073 | 7.5 | **HIGH** | `alloy-dyn-abi` | 0.7.7 | >=0.8.26 | DoS via `TypedData` hashing |
| RUSTSEC-2026-0048 | 7.4 | **HIGH** | `aws-lc-sys` | 0.38.0 | >=0.39.0 | CRL Distribution Point scope check logic error |
| RUSTSEC-2026-0044 | — | **HIGH** | `aws-lc-sys` | 0.38.0 | >=0.39.0 | X.509 Name Constraints bypass via Wildcard/Unicode CN |
| RUSTSEC-2026-0007 | — | **HIGH** | `bytes` | 1.7.1 | >=1.11.1 | Integer overflow in `BytesMut::reserve` |
| RUSTSEC-2024-0421 | — | **HIGH** | `idna` | 0.4.0, 0.5.0 | >=1.0.0 | Punycode labels accepted without non-ASCII output |
| RUSTSEC-2024-0437 | — | **HIGH** | `protobuf` | 2.28.0 | >=3.7.2 | Crash via uncontrolled recursion |
| RUSTSEC-2026-0001 | — | **MEDIUM** | `rkyv` | 0.7.45 | >=0.7.46 | UB in `Arc<T>`/`Rc<T>` `from_value` on OOM |
| RUSTSEC-2023-0071 | 5.9 | **MEDIUM** | `rsa` | 0.9.6 | **NO FIX** | Marvin Attack: key recovery via timing sidechannel |
| RUSTSEC-2026-0049 | — | **MEDIUM** | `rustls-webpki` | 0.103.9 | >=0.103.10 | CRL not considered authoritative due to faulty matching |
| RUSTSEC-2026-0009 | 6.8 | **MEDIUM** | `time` | 0.3.36 | >=0.3.47 | Denial of Service via stack exhaustion |
| RUSTSEC-2025-0055 | — | **MEDIUM** | `tracing-subscriber` | 0.3.18 | >=0.3.20 | Log poisoning via ANSI escape sequences |

#### Unmaintained Crates (warnings)

| Advisory ID | Crate | Installed | Note |
|-------------|-------|-----------|------|
| RUSTSEC-2025-0056 | `adler` | 1.0.2 | Use `adler2` instead |
| RUSTSEC-2024-0388 | `derivative` | 2.2.0 | Consider `educe` or `derive_more` |
| RUSTSEC-2024-0436 | `paste` | 1.0.15 | No longer maintained |
| RUSTSEC-2024-0370 | `proc-macro-error` | 1.0.4 | Use `proc-macro-error2` or `manyhow` |
| RUSTSEC-2025-0134 | `rustls-pemfile` | 1.0.4, 2.2.0 | Unmaintained |
| RUSTSEC-2026-0002 | `lru` | 0.12.5 | `IterMut` violates Stacked Borrows |
| RUSTSEC-2025-0023 | `tokio` | 1.40.0 | Broadcast channel clone without `Sync` |

### Node.js — testudo-extension (3 vulnerabilities)

| Advisory | Severity | Package | Installed | Patched | Description |
|----------|----------|---------|-----------|---------|-------------|
| GHSA-67mh-4wv8-2f99 | **MODERATE** | `esbuild` | <=0.24.2 | >=0.28.0 (breaking) | Dev server allows any website to read responses |
| GHSA-3v7f-55p6-f55p | **HIGH** | `picomatch` | 4.0.0–4.0.3 | `npm audit fix` | Method injection in POSIX character classes |
| GHSA-mw96-cpmx-2vgc | **HIGH** | `rollup` | 4.0.0–4.58.0 | `npm audit fix` | Arbitrary file write via path traversal |

### Node.js — testudo-web

No vulnerabilities found.

### OSV Scanner — Cross-stack

No additional issues beyond those already flagged by native tools.

---

## Dependency Mitigation

### `testudo-exchange/Cargo.toml` (transitive dependencies — update via `cargo update`)

Most Rust vulnerabilities are in **transitive** dependencies pulled through `alloy`, `rustls`, `reqwest`, and `tokio`. Direct `Cargo.toml` edits are not possible for most — instead:

```bash
cd testudo-exchange && cargo update
```

This will pull the latest compatible patch versions for:
- `bytes` 1.7.1 → >=1.11.1
- `aws-lc-sys` 0.38.0 → >=0.39.0
- `rustls-webpki` 0.103.9 → >=0.103.10
- `time` 0.3.36 → >=0.3.47
- `tracing-subscriber` 0.3.18 → >=0.3.20
- `rkyv` 0.7.45 → >=0.7.46
- `tokio` 1.40.0 → latest 1.x

**Cannot be fixed by `cargo update` alone** (require major version bumps):

```diff
# alloy-dyn-abi requires alloy ecosystem upgrade
- alloy = "0.1.4"
+ alloy = "0.8.x"  # or 1.x — check hyperliquid-sdk-rs compatibility

# protobuf requires major bump
- protobuf = "2.28.0"
+ protobuf = "3.7.2"  # check rethinkdb/fred compatibility

# idna requires major bump
- idna = "0.4.0" / "0.5.0"
+ idna = "1.0.0"  # transitive via url/reqwest — wait for upstream

# rsa — NO FIX AVAILABLE (Marvin Attack)
# Monitor RUSTSEC-2023-0071 for future patches
```

### `testudo-extension/package.json`

```bash
cd testudo-extension && npm audit fix
```

This fixes `picomatch` and `rollup`. For `esbuild` (breaking change):

```diff
- "esbuild": "^0.24.x"
+ "esbuild": "^0.28.0"
```

**Note:** esbuild 0.28.0 is a breaking change — verify the build pipeline after upgrading.

---

## Static Analysis Warnings

### cargo clippy (testudo-exchange)

**File:** `crates/router/src/services/cex_client.rs:644`
**Rule:** `clippy::useless_conversion`
**Severity:** Warning

```rust
// Before (flagged)
.send(WsMessage::Text(msg_text.into()))

// After (hardened)
.send(WsMessage::Text(msg_text))
```

**Explanation:** `msg_text` is already `String` — `.into()` is a no-op conversion.

---

**File:** `crates/engine/src/shadow/actor.rs:1835`
**Rule:** `unused_variables`
**Severity:** Warning

```rust
// Before (flagged)
let placed = handle.place_order(user_id, order).await.unwrap();

// After (hardened)
let _placed = handle.place_order(user_id, order).await.unwrap();
```

**Explanation:** Variable `placed` is never read. Prefix with `_` to signal intentional discard.

---

**File:** `crates/router/src/services/trade_manager/evaluator.rs:188`
**Rule:** `clippy::manual_contains`
**Severity:** Warning (test code)

```rust
// Before (flagged)
assert!(!actions.iter().any(|a| *a == ManagementAction::MoveStopToEntry));

// After (hardened)
assert!(!actions.contains(&ManagementAction::MoveStopToEntry));
```

**Explanation:** `contains()` is more efficient and idiomatic than `iter().any()` for equality checks.

---

## Missing Tools

All required tools are installed. No gaps.

---

## Summary

- **Critical:** 0
- **High:** 8 (6 Rust + 2 Node.js)
- **Medium:** 4 (Rust)
- **Moderate:** 1 (Node.js)
- **Low:** 0
- **Unmaintained warnings:** 9 (Rust)
- **Clippy warnings:** 3

### Top Priority Actions

1. **Run `cargo update` in testudo-exchange** — patches 7 of 11 Rust advisories immediately (bytes, aws-lc-sys, rustls-webpki, time, tracing-subscriber, rkyv, tokio)
2. **Run `npm audit fix` in testudo-extension** — patches picomatch and rollup (2 HIGH)
3. **Evaluate alloy ecosystem upgrade** — alloy-dyn-abi 0.7.7 has a 7.5 CVSS DoS vulnerability; requires coordinating with hyperliquid-sdk-rs compatibility
