# Specification: Obsidian Plugin — One-Way Sync of Testudo Data Into a Local Vault

**Spec ID:** OBS-01-obsidian-plugin
**Date:** 2026-04-17
**Status:** Draft
**Class:** Feature / Integration (new product surface)
**Priority:** P2 — optional feature for the Obsidian ∩ Testudo user intersection; unlocks "your data, your AI" positioning without adding complexity to the core app
**Depends on:** None hard. Maximally useful after RSK-01 (risk hub), RSK-02 (setup tags), and RSK-03 (coach reports) ship — each new entity type multiplies the vault graph's value.
**Series:** OBS-01 (first in series; potential future: OBS-02 MCP server over vault, OBS-03 multi-vault support)

---

## Problem Statement

Obsidian users who also trade have already built a personal AI workflow around their vault — Smart Connections, Copilot for Obsidian, Dataview, local MCP servers, Templater, or their own LLM pipelines. Their vault *is* their second brain and their AI substrate. Testudo's trade and journal data, today, is trapped behind the API — visible in the web app but not in the place where those users actually do their thinking.

The gap this spec closes is small but precise: **stream Testudo's structured entities into the user's Obsidian vault as linked markdown files**, so every existing Obsidian AI tool can query the user's full trading history locally without any Testudo-specific integration. A user can ask their local LLM "what did I do the last 3 times I traded a liquidity sweep on BTC?" and get an answer entirely from vault content, not from a Testudo API call.

The design is deliberately narrow: **one-way push, optional, self-contained plugin**. No two-way sync (tarpit), no Testudo-side UI (plugin is a separate product surface), no required setup for non-Obsidian users. The plugin sits entirely inside the user's Obsidian instance, holds a personal access token obtained via a copy-paste flow with zero Testudo frontend changes, and writes Testudo entities into a configurable vault folder with managed-section markers that preserve user annotations across syncs.

This is a niche feature — target audience is the intersection of Obsidian power-users and Testudo users — but the positioning payoff is large: Testudo becomes the first trading journal whose data is natively consumable by the personal-AI-wiki stack. Zero competitors in retail trading tooling offer this.

---

## User Stories

- **As an Obsidian user with Testudo**, I want my trades and journal entries synced as `.md` files with YAML frontmatter, so Dataview can render my own custom dashboards.
- **As a user of local AI tooling** (Smart Connections, Copilot, MCP), I want my coach reports and trades as plain markdown, so my local LLM can read my full history as context without API calls.
- **As a user who annotates trades in Obsidian**, I want my annotations preserved forever across syncs, so I trust the plugin not to lose my notes.
- **As a first-time user**, I want to authenticate the plugin once via a copy-paste token, so I don't need to juggle credentials.
- **As a privacy-minded user**, I want nothing to sync unless I explicitly install the plugin and paste a token, so the default Testudo experience is unaffected.
- **As a Testudo user who doesn't use Obsidian**, I want no new UI in the app, so the feature stays invisible to me.
- **As a user of a self-hosted Testudo**, I want to configure the plugin's API base URL, so I can point it at my own instance.

---

## Functional Requirements

### Plugin (Obsidian side)

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1 | Plugin is a standalone Obsidian plugin, distributed as its own repository (`testudo-obsidian` — new submodule or separate repo), published via BRAT initially, then submitted to the Obsidian community plugin registry | High |
| FR-2 | Plugin settings tab exposes: API base URL, personal access token, vault root folder (default `Testudo/`), per-content-type sync toggles (Entries / Trades / Coach / Setups / Daily), auto-interval toggle + minutes | High |
| FR-3 | Sync triggers: on Obsidian startup (if token present) + manual "Sync now" command + optional auto-interval (default disabled, minimum 15 min, default 60 min when enabled) | High |
| FR-4 | Delta sync: plugin persists `last_synced_at` per entity type; each sync calls `GET /api/plugin/<entity>?since=<ts>`; no full resync unless user clicks "Resync all" in settings | High |
| FR-5 | File-per-entity layout under configurable root folder: `Testudo/Entries/`, `Testudo/Trades/`, `Testudo/Coach/`, `Testudo/Setups/`, `Testudo/Daily/` | High |
| FR-6 | Each file has YAML frontmatter (managed — Testudo owns it) + managed content section between `<!-- testudo:start -->` and `<!-- testudo:end -->` markers (managed) + everything outside those markers (user-owned, preserved across syncs) | High |
| FR-7 | Missing-marker protection: if a file exists but lacks both markers, plugin logs a warning to Obsidian's notice + console and **skips the file** — never overwrites without markers present | High |
| FR-8 | Wikilinks between entities: Entry → Trade, Trade → Setup, Trade → Daily, Coach report → flagged Trades, Daily → all Trades + Entries + Coach report for that day | High |
| FR-9 | Frontmatter schema tuned for Dataview queries — consistent property names: `testudo_id`, `testudo_type`, `testudo_synced_at`, `symbol`, `side`, `venue`, `r_multiple`, `pnl`, `setup`, `opened_at`, `closed_at`, etc. | High |
| FR-10 | Progress notice during sync showing "Syncing N trades, M entries…" + completion toast | Medium |
| FR-11 | Large initial sync paginated with "Loading page X of Y" indicator; does not block Obsidian UI | Medium |
| FR-12 | All network calls use `requestUrl` from the Obsidian API (CORS-safe, works in Electron) | High |
| FR-13 | Error states surface as Obsidian Notice + plugin setting diagnostic section (last sync time, last error) | Medium |
| FR-14 | Plugin stores token using `saveData` (plaintext in plugin data file by default) with a documented note recommending filesystem-level encryption for users with stricter privacy needs | Medium |

### Backend (Testudo) — minimal surface additions

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-15 | New endpoint `GET /api/plugin/token` — requires a valid browser session (SIWE cookie). Returns a plain-text response (no HTML) with a newly generated long-lived PAT. User copies directly from browser address bar view. | High |
| FR-16 | New table `plugin_tokens` (id, user_id, token_hash, created_at, last_used_at, revoked_at). Tokens hashed at rest. | High |
| FR-17 | Plugin-compatible API endpoints under `/api/plugin/*` namespace, accept `Authorization: Bearer <pat>` auth, return JSON: | High |
|      | - `GET /api/plugin/me` (token sanity check, returns user identifier) | |
|      | - `GET /api/plugin/entries?since=<iso>` | |
|      | - `GET /api/plugin/trades?since=<iso>` | |
|      | - `GET /api/plugin/coach-reports?since=<iso>` | |
|      | - `GET /api/plugin/setups?since=<iso>` | |
|      | - `GET /api/plugin/daily?since=<iso>` | |
| FR-18 | Token revocation endpoint `DELETE /api/plugin/token/:id` + listing `GET /api/plugin/tokens` exist but are **not surfaced in any frontend UI** — reserved for a future `/desk/tokens` page if/when needed | Medium |
| FR-19 | All `/api/plugin/*` endpoints update `plugin_tokens.last_used_at` for auditability | Medium |
| FR-20 | **Zero Testudo frontend changes** — no nav entry, no Account-page row, no setting toggle. Plugin discovery is entirely plugin-side (README, Obsidian community listing) | High |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Backend: `plugin_tokens` migration + `GET /api/plugin/token` endpoint + `GET /api/plugin/me` + PAT auth middleware | Token issuance and auth seam work without any frontend changes |
| CP-2 | Plugin scaffold (manifest, main.ts, settings tab), "Paste token → test connection" flow calling `/api/plugin/me`, "Sync now" command writes a single hardcoded test file | End-to-end plumbing: user can authenticate the plugin and trigger a dummy sync |
| CP-3 | `GET /api/plugin/entries?since=` backend + plugin Entries sync with managed markers + missing-marker protection | First real entity type syncs correctly; conflict-safe write established |
| CP-4 | `GET /api/plugin/trades?since=` + plugin Trades sync + wikilink from Entries to Trades | Cross-entity graph links work |
| CP-5 | Setups sync (from RSK-02's `setup_tag` data) | Per-setup notes file enables Dataview queries |
| CP-6 | Coach reports sync (from RSK-03) | Reports become LLM-readable context in vault |
| CP-7 | Daily notes sync (computed aggregate per day: net P&L, trades list, exposure snapshot) | Hooks into Obsidian's daily-note-centric AI workflows |
| CP-8 | Auto-interval option + progress notices + error surfacing | UX polish, production-readiness |
| CP-9 | BRAT release + README + screenshots; submission to Obsidian community registry | Public beta, then stable |

Each checkpoint is independently shippable. CP-1 + CP-2 + CP-3 alone is a usable private beta (syncs entries only); the rest layers incremental value.

### File Format — Managed Region Contract

```md
---
# frontmatter is managed — Testudo overwrites this block on each sync
testudo_id: trade-01HXYZ...
testudo_type: trade
testudo_synced_at: 2026-04-17T18:00:00Z
symbol: BTC
side: long
venue: bybit
entry_price: 67000
exit_price: 68200
r_multiple: 1.8
pnl_usd: 12.40
setup: "[[btc-4h-breakout]]"
opened_at: 2026-04-17T13:22:00Z
closed_at: 2026-04-17T15:41:00Z
---

<!-- testudo:start — do not edit this block; it is overwritten on each sync -->
## Trade — BTC LONG

- **Venue:** Bybit
- **Entry:** 67,000 @ 2026-04-17 13:22 UTC
- **Exit:**  68,200 @ 2026-04-17 15:41 UTC
- **R-multiple:** +1.8R
- **PnL:** $12.40
- **Setup:** [[btc-4h-breakout]]
- **Daily note:** [[2026-04-17]]
<!-- testudo:end -->

## My notes

Felt rushed on entry, didn't wait for the retest. Size was fine.
Next time: require 2nd candle close above level before pulling trigger.
```

**Sync algorithm:**

```
for each entity from /api/plugin/<type>?since=<last_synced>:
  path = vault_root / type_folder / filename(entity)
  if path exists:
    content = read(path)
    if not has_markers(content):
      log_warning("skipping — no markers", path)
      continue
    new_content = splice_managed_regions(content, render(entity))
    write(path, new_content)
  else:
    write(path, render(entity))  # fresh file has markers from the start
update last_synced_at[type] = now()
```

### Key Types (plugin TypeScript)

```typescript
// testudo-obsidian/src/types.ts
export interface TestudoEntity {
  testudo_id: string
  testudo_type: 'entry' | 'trade' | 'coach' | 'setup' | 'daily'
  testudo_synced_at: string
  updated_at: string
  // entity-specific payload
  [k: string]: unknown
}

export interface SyncState {
  last_synced_at: { [entity_type: string]: string | null }
  token: string | null
  base_url: string
}

export interface TestudoPluginSettings {
  base_url: string
  token: string
  vault_root: string          // default "Testudo"
  sync_entries: boolean
  sync_trades: boolean
  sync_coach: boolean
  sync_setups: boolean
  sync_daily: boolean
  auto_sync_enabled: boolean
  auto_sync_minutes: number   // default 60
}
```

### Backend Token Endpoint Shape

```rust
// testudo-exchange/crates/router/src/routes/plugin.rs
// GET /api/plugin/token
// Requires SIWE session cookie (middleware handles)
// Generates a PAT, inserts hashed version, returns plain text

pub async fn issue_plugin_token(user: AuthedUser, db: &PgPool) -> impl Responder {
    let pat = format!("testudo_pat_{}", generate_url_safe_random(32));
    let hash = sha256(&pat);
    sqlx::query!(
        "INSERT INTO plugin_tokens (user_id, token_hash) VALUES ($1, $2)",
        user.id, hash
    ).execute(db).await?;

    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body(pat)
}
```

User flow: browse to `https://<testudo-host>/api/plugin/token` while logged into Testudo → browser renders the PAT as plain text → copy → paste into Obsidian plugin settings → done.

### Paved Roads

- **Obsidian plugin sample** — `github.com/obsidianmd/obsidian-sample-plugin` as scaffold template.
- **testudo-extension build system** (esbuild + TypeScript + Zod) is a direct template for the plugin build chain.
- **Zod schemas** — runtime validation pattern carries over to plugin.
- **Existing JWT middleware** — PAT auth middleware is a minor variant of the existing bearer-token path.
- **`rust_decimal` + existing trade types** — `/api/plugin/trades` response derives from the same DB views as the journal API.
- **Managed-marker pattern** — well-established in Readwise Official, Omnivore, Hypothesis plugins; no novel design risk.

### Files

**New (plugin, separate repo or `testudo-obsidian/` submodule):**
- `testudo-obsidian/manifest.json`
- `testudo-obsidian/main.ts` — plugin class, commands, sync orchestration
- `testudo-obsidian/settings.ts` — settings tab UI
- `testudo-obsidian/syncer.ts` — sync loop, delta tracking, API calls
- `testudo-obsidian/formatters/` — per-entity markdown generators (one file each: entry.ts, trade.ts, coach.ts, setup.ts, daily.ts)
- `testudo-obsidian/markers.ts` — managed-region splice/parse logic
- `testudo-obsidian/types.ts`
- `testudo-obsidian/esbuild.config.mjs`
- `testudo-obsidian/README.md` — setup instructions, managed-marker contract, screenshots
- `testudo-obsidian/styles.css`

**New (backend):**
- `testudo-exchange/crates/router/src/routes/plugin.rs`
- `testudo-exchange/crates/router/src/middleware/pat_auth.rs`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_plugin_tokens.sql`
- `testudo-exchange/crates/router/tests/plugin_token_test.rs`
- `testudo-exchange/crates/router/tests/plugin_api_test.rs`

**Modified (backend, minimal):**
- `testudo-exchange/crates/router/src/routes/mod.rs` — wire plugin routes
- `testudo-exchange/crates/router/src/main.rs` — register

**Modified (frontend): NONE** (per FR-20)

### Dependencies Added

- **Plugin:** `obsidian` (peer), `esbuild`, `zod`, `yaml` (for frontmatter), `tslib`
- **Backend:** none new — `sha2`, `rand` already available

---

## Acceptance Criteria

- [ ] Backend `GET /api/plugin/token` returns a plain-text PAT when called with a valid session cookie
- [ ] PAT auth middleware accepts `Authorization: Bearer <pat>` and rejects invalid tokens
- [ ] All 5 plugin-compatible endpoints (`/me`, `/entries`, `/trades`, `/coach-reports`, `/setups`, `/daily`) exist with `?since=` support
- [ ] Plugin installs via BRAT, settings tab renders all FR-2 fields
- [ ] First-run flow: paste token → click "Test connection" → green check + user identifier displayed
- [ ] "Sync now" pulls all enabled entity types and writes files under configured vault root
- [ ] Subsequent sync with no changes produces **zero file writes** (delta sync working)
- [ ] Subsequent sync with changes produces writes only for changed entities
- [ ] User-edited content outside `<!-- testudo:start/end -->` markers is preserved verbatim across a sync that updates the file
- [ ] File with missing markers is **skipped** with a warning notice
- [ ] Disabling "Sync Trades" toggle → no trade files are created or updated on next sync
- [ ] Dataview query `TABLE r_multiple, setup FROM "Testudo/Trades"` returns expected rows
- [ ] Auto-interval at 15 minutes pulls without blocking Obsidian UI
- [ ] Plugin passes Obsidian community plugin review guidelines (no arbitrary network without user consent, no unsafe DOM, no bundled secrets)
- [ ] Zero new files or changes in `testudo-journal/` or `testudo-web/` beyond this spec's explicit backend changes
- [ ] Backend verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`

---

## Risks

1. **Obsidian community plugin review rejects.** Review requires no arbitrary CDN loads, clean error handling, no DOM mutation outside own scope. *Mitigation:* submit via BRAT for beta rollout first, harden based on early feedback, only submit to official registry once production-hardened. Worst case, stay on BRAT indefinitely — power users are comfortable with it.
2. **Initial full sync performance on large trade history.** A user with 2,000 trades × avg 500B per markdown file = 1MB write burst. *Mitigation:* paginate API responses (100 entities per page), show progress notice, chunk writes with `yield` between pages to keep the UI responsive.
3. **User writes notes inside managed markers, loses them on next sync.** Even with documentation, this will happen. *Mitigation:* clear comment text in the markers themselves (`do not edit this block`), detect via hash whether managed region was modified mid-session and log a diagnostic notice so sophisticated users can spot it, document recovery in README.
4. **Testudo API breaking changes break all deployed plugin versions.** *Mitigation:* version the namespace — `/api/plugin/v1/*`. Plugin's manifest declares compatible API version. On mismatch, show clear "plugin needs updating" notice. Breaking changes bump version.
5. **Tokens stored in plaintext in plugin data file.** Obsidian doesn't give plugins keychain access. *Mitigation:* document in README; users concerned about secrets at rest can encrypt the vault or plugin data directory. Accept this as the Obsidian plugin norm.
6. **Large tokens (400+ chars) awkward for copy-paste.** *Mitigation:* PATs use URL-safe base64 of 32 random bytes = ~43 chars, trivially copyable.
7. **Plugin data location for tokens differs between desktop and mobile.** *Mitigation:* Obsidian's `saveData` API abstracts this; don't assume paths.
8. **Token endpoint XSS / CSRF.** A malicious site linking to `/api/plugin/token` could trick a logged-in user into generating a token. *Mitigation:* endpoint is `GET` for copy-paste UX, so CSRF protections need extra care — require a custom request header (`X-Testudo-Intent: issue-plugin-token`) that only the plugin sets, blocking casual browser navigation. Document the hurdle in README (user browses to the URL *after* the plugin directs them, with instructions to include the header — alternatively shift to a form-POST page served at a benign URL). Re-evaluate during CP-1 if the header approach is too awkward.
9. **Pseudo-PII leaves the user's machine via HTTP.** Not a change from today (the web app already does this) but worth documenting. *Mitigation:* README + settings page clarify trade data is sent to the user's own Testudo instance only, no third party.

---

## Completion Signal

This spec is complete when:
1. Plugin installable via BRAT from `<org>/testudo-obsidian` repository
2. End-to-end flow works for a real user with ≥ 100 existing trades: paste token → sync → Dataview query returns rows → annotate one trade → resync → annotation preserved
3. All FR-1 through FR-20 implemented
4. All acceptance criteria checked off
5. Backend `cargo clippy --all-targets && cargo test` passes with the new plugin routes
6. Plugin's README has a ≥ 5-step setup guide with screenshots, explicit "managed marker contract" documentation, and a troubleshooting section
7. Testudo frontend is **byte-identical** to pre-spec state (verified by git diff on `testudo-journal/` and `testudo-web/` excluding unrelated changes)
8. Plugin submitted to Obsidian community registry (acceptance pending review is OK — BRAT install remains the distribution floor)
9. Spec OBS-02 scoped (e.g., MCP server exposing the vault to LLMs) based on lessons from OBS-01 rollout
