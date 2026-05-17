# Specification: On-Chain Discipline Attestations — Permanent Credentials for Milestones

**Spec ID:** ENG-02-onchain-discipline-attestations
**Date:** 2026-04-17
**Status:** Draft
**Class:** Feature / Backend + Frontend + On-Chain Integration
**Priority:** P2 — identity layer that gives ENG-01's score economic potentiation; ships as a public good without needing external partners
**Depends on:** ENG-01 (Dignitas history + streak data must exist). Soft dependency on RSK-02 for the "100 setup-tagged trades" milestone.
**Series:** ENG-01 through ENG-03 (ENG-03 = Dignitas-gated Morpho lending market — deferred until treasury + user base can anchor it)

---

## Problem Statement

ENG-01 produces a living Dignitas score and a discipline streak that lives inside Testudo. That's enough to be a retention artifact, but it's not *portable*. A 180-day clean streak achieved by a Testudo user cannot be proven to a prop firm, a copy-trading platform, or a DeFi protocol. It's a number on a private dashboard. The identity collateral is real inside Testudo and nowhere else.

This spec closes that gap by minting **permanent on-chain attestations** for disciplined-behavior milestones. Attestations use EAS (Ethereum Attestation Service) on Base, are non-revocable, are written to the user's SIWE-connected wallet address, and cost the user nothing — Testudo pays gas via a backend relayer. A user who achieves a 90-day clean streak on 2026-07-15 has an immutable on-chain record saying so, forever, even if they later regress. Historical achievements are facts; facts don't revoke.

This is the *smallest possible* step that could credibly close the "tokenize an ideology" loop from the user's design insight. The spec ships **no capital layer** — no lending, no yield, no protocol partnerships. Just the credential. ENG-03 is explicitly deferred: once attestations exist, future lending markets (Morpho, Euler, others) can gate on them whenever a partner or treasury anchor emerges. Shipping the credential alone is sufficient to make the belief load-bearing — users don't need a current lending market to exist, they need to *believe one could*.

Critically: **attestations are opt-in, address-bound, and privacy-calibrated.** A user must explicitly enable on-chain minting — default is off. Attestations are public by cryptographic nature (EAS records are queryable by any party), so opt-in is the only honest default.

---

## Non-Goals (Explicit Anti-Scope)

- **No capital features.** No lending, no yield, no fee rebates, no market-making, no partnership offers. ENG-03 concerns.
- **No NFTs.** Attestations ≠ NFTs. Attestations are non-transferable, structured, queryable records. NFTs imply tradability, which corrupts a credential.
- **No soulbound-token gimmickry** — EAS is already the right primitive. Don't reinvent.
- **No attestation revocation on discipline breaks.** "Achieved 90-day streak on 2026-07-15" is a historical fact. It does not retroactively disappear because of a later Concerning flag. The *current* streak is separate and mutable (ENG-01 owns it).
- **No cross-chain minting.** Single chain (Base) for MVP. Multi-chain can be a future spec if demand emerges.
- **No user-payable gas path.** Relayer-only. Making users pay gas on milestones they earned would be hostile UX.
- **No milestone inflation.** Strict MVP list of 5. New milestones require an explicit follow-up spec; adding them cheaply cheapens every existing one.
- **No attestation for activity metrics** (trade count, P&L, win rate). Only for discipline-adherence achievements, consistent with ENG-01's formula design.

---

## User Stories

- **As a Testudo user who achieved a 90-day clean streak**, I want that fact recorded permanently on-chain tied to my wallet, so it's a credential I control and can show to anyone, forever.
- **As a trader applying to a prop firm / pitching my edge**, I want to link an EAS block-explorer URL proving my discipline milestones, so my reputation is verifiable and portable.
- **As a user who broke my streak last week**, I want my *past* 90-day attestation preserved — not revoked — so my historical achievements remain intact even while my current streak is 0.
- **As a user who doesn't care about on-chain**, I want attestations off by default, so Testudo never mints anything unless I opt in.
- **As a Testudo user without crypto-ops fluency**, I want to earn attestations without paying gas, holding ETH, or doing any transaction signing, so discipline milestones feel like a native Testudo feature rather than a DeFi obstacle course.
- **As any third-party protocol (now or future)**, I want Testudo's attestations to use the standard EAS schema, so I can query them programmatically and, if I wish, price them into my own system.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Five MVP milestones eligible for attestation: **30-day clean streak**, **90-day clean streak**, **180-day clean streak**, **100 trades with a setup tag** (requires RSK-02), **first positive Dignitas quarter** (Q of rolling 90 days with net positive score trajectory) | High | db-processor |
| FR-2 | Single EAS schema registered once on **Base mainnet**; schema fields: `milestone_kind` (enum as uint8), `achieved_at` (uint64 unix), `streak_days` (uint32, 0 if n/a), `proof_hash` (bytes32 SHA-256 of computation inputs for verifiability), `version` (uint8, starts at 1) | High | on-chain |
| FR-3 | Daily attestation-check job (piggybacks on ENG-01's daily cron): for each opted-in user, check every milestone, mint on-chain via relayer for any newly-earned + not-yet-attested milestones | High | db-processor |
| FR-4 | Idempotency: database unique constraint on `(user_id, milestone_kind)` prevents re-minting; pre-flight check confirms no existing attestation UID before submission | High | sqlx_postgres + db-processor |
| FR-5 | Relayer pattern: a dedicated Testudo backend wallet (`attester_key`) pays all gas and signs attestations; attestations are `revocable: false`; recipient is the user's SIWE-connected wallet address | High | db-processor |
| FR-6 | Opt-in preference: attestations **default OFF**. User must explicitly enable via Account → Identity → "Enable on-chain attestations" toggle | High | router + journal/frontend |
| FR-7 | Retroactive minting on opt-in: when a user first enables attestations, the daily job mints *all currently-earned* milestones on next run (one catch-up pass), not separate per-milestone events | High | db-processor |
| FR-8 | Public profile renders an "Attestations" list (under existing visibility-toggle gating from ENG-01): each attestation shows milestone name, achieved date, and a link to `https://base.easscan.org/attestation/view/<uid>` for independent verification | High | journal/frontend |
| FR-9 | User's own Dignitas panel shows earned attestations with the same link-out, regardless of public profile visibility settings (user always sees their own) | Medium | journal/frontend |
| FR-10 | Gas budget tracking: every attestation logs `gas_used` + `gas_price` + `tx_hash` in the `attestations` table for cost monitoring | Medium | db-processor |
| FR-11 | Relayer failure handling: on transaction failure (chain congestion, RPC error, insufficient funds in attester wallet), retry with exponential backoff up to 5 attempts across 24h; persist `attestation_status: pending | submitted | failed` in DB; surface `failed` states in backend health metrics | High | db-processor |
| FR-12 | Attester wallet low-balance alert: when attester wallet ETH on Base drops below configurable threshold (default 0.01 ETH), log warning + emit metric. Manual top-up procedure documented — no auto-funding | Medium | db-processor + ops |
| FR-13 | Attestation chain configuration via env: `ATTESTATION_CHAIN_ID`, `ATTESTATION_RPC_URL`, `ATTESTATION_SCHEMA_UID`, `ATTESTER_PRIVATE_KEY` (loaded from secret manager, never in source). Testnet config for CI uses Base Sepolia | High | config |
| FR-14 | Schema versioning: the `version` field in each attestation allows future schema evolution without invalidating old attestations | Medium | on-chain |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Register EAS schema on Base Sepolia (testnet); record schema UID in env; milestone enum + detection logic in `crates/db-processor/src/attestations/milestones.rs` with unit tests | Milestone detection is deterministic and testable off-chain |
| CP-2 | `attester.rs` module submits attestations to Base Sepolia via `alloy` client; integration test end-to-end against testnet with a funded throwaway wallet | On-chain submission works; relayer pattern proven |
| CP-3 | Daily attestation-check job wired into ENG-01's daily cron; idempotency + retroactive-on-opt-in tested with fixture users | Scheduled path works; no duplicate attestations under any invocation pattern |
| CP-4 | Opt-in toggle + `AttestationSettings.tsx` on Account page; backend preference endpoint; user sees current opt-in state | User can control minting behavior |
| CP-5 | Public profile + Dignitas panel render attestation list with EAS explorer links | Discoverable + verifiable from outside |
| CP-6 | Deploy schema to Base **mainnet**; fund attester wallet with 0.05 ETH ($~$150 at current prices — enough for tens of thousands of attestations); flip `ATTESTATION_CHAIN_ID` to 8453 (Base mainnet); validate with one test user | Production cut-over |
| CP-7 | Observability: gas-used metrics, attester balance alert, failed-retry dashboards | Operational readiness |

Each checkpoint is shippable. After CP-5 the feature works on testnet (sufficient for beta with real users sharing their testnet attestation URLs). CP-6 is the commitment to mainnet permanence.

### Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  testudo-exchange/crates/db-processor/src/attestations/          │
│                                                                  │
│   milestones.rs           ← enum MilestoneKind + detection rules │
│     fn eligible_milestones(user_id) -> Vec<MilestoneKind>        │
│                                                                  │
│   attester.rs             ← EAS submission via alloy             │
│     fn submit(user_wallet, milestone, proof_hash)                │
│       -> Result<AttestationUid>                                  │
│                                                                  │
│   schedule.rs             ← piggybacks ENG-01 daily cron         │
│     for each opted-in user, for each eligible milestone,         │
│     if not minted, submit via attester                           │
│                                                                  │
│   types.rs                                                       │
└──────────────────────────────────────────────────────────────────┘
              │
              ▼
       attestations table (PG)
       ├─ user_id, milestone_kind, uid, tx_hash
       ├─ gas_used, gas_price, block_number
       ├─ submitted_at, status (pending|submitted|failed)
       └─ UNIQUE(user_id, milestone_kind)
              │
              ▼
  Base mainnet EAS contract (0x4200000000000000000000000000000000000021)
  ├─ Schema UID (registered once at deploy)
  └─ Attestations indexed by recipient (user wallet) + schema
              │
              ▼
┌──────────────────────────────────────────────────────────────────┐
│  testudo-journal                                                 │
│   components/account/AttestationSettings.tsx  ← opt-in toggle   │
│   components/account/AttestationList.tsx      ← user's own view  │
│   components/coach/AttestationBadge.tsx       ← single item      │
│   pages/PublicProfile.tsx (extend from ENG-01) ← opt-in list    │
└──────────────────────────────────────────────────────────────────┘
```

### Key Types

```rust
// testudo-exchange/crates/db-processor/src/attestations/types.rs

#[derive(Copy, Clone, Debug, sqlx::Type, Serialize, Deserialize)]
#[repr(u8)]
pub enum MilestoneKind {
    CleanStreak30 = 1,
    CleanStreak90 = 2,
    CleanStreak180 = 3,
    SetupTaggedTrades100 = 4,
    FirstPositiveDignitasQuarter = 5,
}

pub struct AttestationRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub milestone: MilestoneKind,
    pub uid: Option<[u8; 32]>,       // EAS attestation UID, null until submitted
    pub tx_hash: Option<[u8; 32]>,
    pub block_number: Option<u64>,
    pub gas_used: Option<u64>,
    pub gas_price: Option<u128>,
    pub status: AttestationStatus,   // Pending | Submitted | Failed
    pub submitted_at: Option<DateTime<Utc>>,
    pub retry_count: u32,
    pub last_error: Option<String>,
    pub proof_hash: [u8; 32],        // SHA-256 of computation inputs
    pub version: u8,                 // schema version
}

pub enum AttestationStatus {
    Pending,
    Submitted,
    Failed,
}
```

### EAS Schema

```
string rawSchema = "uint8 milestone_kind, uint64 achieved_at, uint32 streak_days, bytes32 proof_hash, uint8 version"
bool revocable = false
address resolver = 0x0000000000000000000000000000000000000000  // no custom resolver
```

Schema registered once via EAS SchemaRegistry (0x4200000000000000000000000000000000000020 on Base). UID pinned in config forever.

### Relayer Flow (per attestation)

```
1. Detection job identifies user + newly-earned milestone
2. Compute proof_hash = sha256(user_id || milestone_kind || achieved_at || inputs)
3. Pre-flight check: SELECT FROM attestations WHERE user_id=? AND milestone_kind=? → must be empty
4. Insert row with status=Pending
5. Build AttestationRequest via alloy EAS bindings
6. Sign with ATTESTER_PRIVATE_KEY
7. Submit via RPC
8. On receipt: update row with uid, tx_hash, block_number, gas_used → status=Submitted
9. On failure: increment retry_count; if < 5 schedule retry, else status=Failed + log
```

Attester wallet is hot by necessity (automated gas-paying). Mitigate by:
- **Minimal balance** — keep just enough to cover ~1 month of expected attestations (~0.01–0.05 ETH on Base)
- **Scoped permissions** — wallet has no other role; compromised key lets attacker mint fake attestations but not drain user funds
- **Key rotation procedure** documented — new attester wallet → deploy new schema version → old attestations remain valid under schema v1, new ones under v2
- **Monitoring** — anomalous minting rate triggers alert

### Dependency: `alloy` (Rust Ethereum Toolkit)

```
alloy = { version = "0.x", features = ["full"] }
```

`alloy` is the modern successor to `ethers-rs` (which is being sunset). Provides typed contract bindings, tokio-native async, and EIP-712 typed-data signing out of the box. EAS interaction fits neatly into its contract macro pattern.

### Paved Roads

- **ENG-01's daily cron** — piggyback, don't create a new scheduler.
- **ENG-01's streak computation** — directly consumable; milestone detection is `streak_days >= 30/90/180`.
- **RSK-02's `setup_tag` data** — count of trades with non-null `setup_tag` per user → trivially gives the "100 setup-tagged trades" milestone.
- **SIWE auth session** — already gives us the user's wallet address as the attestation recipient. No additional wallet-connect flow needed.
- **`rust_decimal`, `sqlx`, `chrono`, `uuid`** — already available.
- **EAS on Base** — contracts deployed, schemas supported, block explorer (easscan.org) indexes automatically; no custom indexer needed.
- **`PageSubHeader`, signal colors, `font-mono`** — aesthetic tokens for the frontend components.

### Files

**New (backend):**
- `testudo-exchange/crates/db-processor/src/attestations/mod.rs`
- `testudo-exchange/crates/db-processor/src/attestations/milestones.rs`
- `testudo-exchange/crates/db-processor/src/attestations/attester.rs`
- `testudo-exchange/crates/db-processor/src/attestations/schedule.rs`
- `testudo-exchange/crates/db-processor/src/attestations/types.rs`
- `testudo-exchange/crates/db-processor/tests/attestations_milestones_test.rs`
- `testudo-exchange/crates/db-processor/tests/attestations_idempotency_test.rs`
- `testudo-exchange/crates/db-processor/tests/attestations_testnet_integration_test.rs` (gated by feature flag, requires RPC)
- `testudo-exchange/crates/router/src/routes/attestations.rs` — `GET /api/attestations/me`, `PATCH /api/attestations/preference`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_attestations.sql`

**New (frontend):**
- `testudo-journal/src/components/account/AttestationSettings.tsx` — opt-in toggle + status
- `testudo-journal/src/components/account/AttestationList.tsx` — renders user's own attestations
- `testudo-journal/src/components/AttestationBadge.tsx` — single attestation display (reusable in profile + panel)

**Modified:**
- `testudo-journal/src/pages/PublicProfile.tsx` (from ENG-01) — render attestation list under visibility gating
- `testudo-journal/src/components/DignitasPanel.tsx` (from ENG-01) — include attestations section
- `testudo-journal/src/components/account/IdentitySettings.tsx` (from ENG-01) — add attestation opt-in toggle alongside public-profile toggles
- `testudo-journal/src/api/client.ts` — `fetchMyAttestations`, `setAttestationPreference`
- `testudo-journal/src/lib/help-content.ts` — explanations for each milestone + EAS concepts
- `testudo-exchange/crates/db-processor/Cargo.toml` — add `alloy`
- `testudo-exchange/crates/router/src/routes/mod.rs` — wire attestation routes
- `testudo-exchange/crates/db-processor/src/dignitas/schedule.rs` (ENG-01) — call into attestation job after daily snapshot

### Dependencies Added

- `alloy = "0.x"` with `full` features — Rust Ethereum toolkit

### Env Configuration

```
ATTESTATIONS_ENABLED=true
ATTESTATION_CHAIN_ID=8453                # Base mainnet (84532 for Sepolia)
ATTESTATION_RPC_URL=https://mainnet.base.org
ATTESTATION_SCHEMA_UID=0x...             # registered once, then hardcoded
ATTESTER_PRIVATE_KEY=<from-secrets>
ATTESTER_MIN_BALANCE_ETH=0.01            # alert threshold
ATTESTATION_RETRY_MAX=5
```

---

## Acceptance Criteria

- [ ] EAS schema registered on Base mainnet; UID pinned in env
- [ ] Attester wallet funded; balance-alert metric live
- [ ] All 5 MVP milestones detected correctly via unit-tested fixtures
- [ ] Daily job mints new milestones for opted-in users; idempotency holds under repeated runs
- [ ] Retroactive catch-up: a user who opts in after already earning 30d + 90d streaks gets both attestations minted on the next job run (but not more)
- [ ] Opt-in defaults to **off**; enabling is explicit via `AttestationSettings` toggle
- [ ] Historical attestations survive a streak reset: user with 90d attestation who then has Concerning flag and streak resets to 0 still shows 90d attestation in profile + panel
- [ ] Public profile renders attestations with correct easscan.org links; each link resolves to the actual attestation on-chain
- [ ] Gas usage logged per attestation; average cost < $0.005 on Base at typical gas levels
- [ ] Failed-submission retry: injecting an RPC failure causes up to 5 retries with exponential backoff, then marks `status=failed`
- [ ] Attester key rotation procedure documented in `testudo-exchange/docs/ops/attester-key-rotation.md` (or equivalent)
- [ ] Third-party verifiability: an outside party given a user's wallet address can query EAS and see their Testudo milestone attestations without any Testudo API access
- [ ] Backend: `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] Frontend: `cd testudo-journal && bun run build`
- [ ] Testnet integration test (Base Sepolia) passes in CI with a funded test attester wallet

---

## Risks

1. **Attester key compromise.** If the relayer private key leaks, an attacker can mint fake attestations (but cannot drain user funds, since the wallet only signs `attest()` calls and holds only gas-tier ETH). *Mitigation:* wallet held in secret manager, never on developer laptops; scoped permissions (no other role); anomalous-mint-rate monitoring; documented rotation procedure (new wallet → schema v2 → old attestations remain valid). Smart-contract-level permission scoping is not possible in EAS's current design — a stolen attester can mint anything — so operational discipline is the only defense.
2. **Chain outage or RPC provider failure.** Base or the RPC provider could have downtime. *Mitigation:* attestation minting is not time-critical — retry with exponential backoff; 24h window before marking failed; attestations aren't user-facing the instant they're earned, they appear once confirmed. Degraded state surfaces in metrics, not users.
3. **Milestone gaming.** Could a user manipulate trades to earn milestones they don't deserve? *Mitigation:* milestones derive entirely from server-side Dignitas data (ENG-01) and trade metadata (RSK-02). Users cannot manipulate their own streak counter; they can only *live* through a 90-day period without a Concerning flag. The gaming surface is ENG-01's formula, which is already discipline-only by design.
4. **Re-mint bugs producing duplicates.** A backend bug could trigger the detection logic to try to mint the same milestone twice. *Mitigation:* `(user_id, milestone_kind)` UNIQUE constraint at the DB layer is the hard gate — even if application logic fails, the DB rejects the duplicate insert. Pre-flight check provides early warning.
5. **User loses access to their wallet after attestation.** The attestation is permanently bound to the address; if the user loses the key, the credential is orphaned. *Mitigation:* this is a fundamental property of on-chain credentials, not a bug. Document it in help copy: "your attestations are permanently tied to the wallet you used when achieving the milestone." A future spec could add a "bind new wallet" flow but it's inherently weaker than key security.
6. **Credential inflation dilutes meaning.** If ENG-02 later expands to 20 milestones, each one means less. *Mitigation:* non-goal explicitly bans milestone inflation; adding milestones requires a new spec + justification; the MVP list of 5 is designed to be sparse.
7. **Privacy: attestations are public.** Anyone watching Base can see which addresses earn Testudo milestones on which dates. *Mitigation:* opt-in default OFF is the fundamental mitigation. Help copy is explicit: "on-chain attestations are publicly visible to anyone. Do not enable if you want your Testudo usage private." Power users can create a dedicated "trading identity" wallet that they use only for Testudo SIWE.
8. **Regulatory uncertainty.** In some jurisdictions, issuing on-chain records tied to financial behavior could be argued to constitute an unregistered financial instrument. *Mitigation:* attestations carry no economic value in this spec (ENG-03 capital tie-in is explicitly deferred), no transferability, no redemption rights. They're credentials, not tokens. Legal review before mainnet deploy (CP-6) is a must.
9. **Proof_hash doesn't fully prove correctness.** The `proof_hash` lets a verifier reproduce a computation if they have the inputs, but Testudo controls the inputs — a malicious Testudo could mint any attestation it wanted. *Mitigation:* this is an honest limitation. The attestation is trust-in-Testudo-scoped, not trustless. Third parties trusting the attestations are trusting Testudo's computation, just as they'd trust a CA signing a certificate. Document clearly.
10. **Schema evolution breaks old attestations' semantic meaning.** Adding a new milestone or changing an existing one mid-flight. *Mitigation:* `version` field in schema allows future versions; old attestations remain valid under v1 semantics; new milestones in v2 are additive. Never reuse `milestone_kind` enum values.

---

## Completion Signal

This spec is complete when:

1. All FR-1 through FR-14 implemented and tested
2. All acceptance criteria checked off
3. Schema deployed on Base mainnet with UID pinned in production env
4. Attester wallet funded, balance monitoring live
5. At least 3 test users have opted in and earned at least one attestation each on Base mainnet; all 3 attestations independently verifiable via easscan.org
6. Key rotation procedure documented and rehearsed once (dry-run) before production use
7. Legal review of attestation semantics complete before mainnet cut-over (CP-6 gate)
8. `cd testudo-exchange && cargo clippy --all-targets && cargo test` passes
9. `cd testudo-journal && bun run build` succeeds
10. Overview + Account pages visually unchanged aside from explicit additions (AttestationSettings, AttestationList)
11. Committed: `feat(eng-02): on-chain discipline attestations via EAS on Base`
12. ENG-03 scoped as fast-follow with concrete criteria for "when to build the Morpho market" (e.g., "≥ 200 users with ≥ one on-chain attestation" and "treasury capable of seeding ≥ $25k")
