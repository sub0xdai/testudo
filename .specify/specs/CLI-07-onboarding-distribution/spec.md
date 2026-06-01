# Specification: One-Line Install + Multi-Provider LLM + Strategy Creation Wizard

**Spec ID:** CLI-07-onboarding-distribution
**Date:** 2026-06-01
**Status:** Draft
**Class:** Feature / Distribution
**Priority:** P0 — without `install.sh` there's no user acquisition path; without LLM providers, the agent loop can't think
**Depends on:** CLI-01 through CLI-06 (all complete)
**Series:** CLI-07 (Onboarding & Distribution)

---

## Problem Statement

The testudo CLI harness is fully built (165 tests, 0 clippy warnings) but four gaps block the user story of "curl install → pick any LLM → create strategies → trade":

1. **No install script.** Users must clone the repo and `cargo build --release`. The vision is `curl -fsSL https://testudo.vip/install.sh | bash`.

2. **Only Anthropic works.** The `LlmClient` factory panics on anything except `"anthropic"`. OpenAI, Gemini, and Ollama are 4-line stubs. Users can't use DeepSeek, Groq, OpenRouter, Qwen, or any OpenAI-compatible provider.

3. **Init doesn't configure the LLM.** The 5-step wizard never asks about provider, API key, or model. A fresh user finishes init and `agent start` panics because `[llm].api_key` is empty.

4. **No guided strategy creation.** Users must hand-write TOML files to use `strategy add --from`.

---

## User Stories

- **As a new user**, I run `curl ... | bash` and within 5 seconds I have `testudo` on my PATH, ready for `testudo init`.
- **As a user with a DeepSeek API key**, I pick "DeepSeek" in the init wizard, paste my key, and my agent runs on DeepSeek-V3.
- **As a user running Ollama locally**, I pick "Ollama" and my agent uses my local Llama model with zero API costs.
- **As a trader with an idea**, I run `testudo strategy create` and it walks me through building a strategy TOML interactively.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `install.sh` detects OS/arch, downloads binary from GitHub releases, installs to `~/.local/bin`, appends to PATH. | High | Distribution |
| FR-2 | `install.sh` is idempotent — re-running overwrites binary, no duplicate PATH entries. | Medium | Distribution |
| FR-3 | `OpenAiClient` implements `LlmClient` via OpenAI Chat Completions API. Covers OpenAI, DeepSeek, Groq, Together, xAI, Mistral, OpenRouter, Ollama, Qwen, and any custom endpoint. | High | LLM |
| FR-4 | `GeminiClient` implements `LlmClient` via Google Generative AI API (`generateContent`). | High | LLM |
| FR-5 | `LlmConfig` gains `base_url: Option<String>` — when set, overrides the provider's default endpoint. | High | Config |
| FR-6 | `create_client()` factory matches anthropic, openai, deepseek, groq, together, xai, mistral, openrouter, gemini, ollama, qwen — each with correct default base URL and default model. | High | LLM |
| FR-7 | `testudo init` asks which LLM provider to use from the full list, prompts for API key, and offers provider-specific default model. | High | Init |
| FR-8 | `testudo init` saves all LLM config to `[llm]` section of `config.toml`. | High | Init |
| FR-9 | `testudo strategy create` starts an interactive wizard: name → desc → constraints → tools → `$EDITOR` for system prompt → validates → saves. | High | Strategy |
| FR-10 | `testudo strategy create` validates name uniqueness (no collision with builtins or existing user strategies). | Medium | Strategy |

---

## Technical Implementation

### Architecture

```
LLM Provider Architecture:

  LlmClient trait (send_message)
       │
       ├── AnthropicClient  ──▶  POST https://api.anthropic.com/v1/messages
       │                           (already implemented)
       │
       ├── OpenAiClient     ──▶  POST {base_url}/chat/completions
       │                           Covers: OpenAI, DeepSeek, Groq, Together,
       │                           xAI, Mistral, OpenRouter, Ollama, Qwen,
       │                           and any custom endpoint via base_url
       │
       └── GeminiClient     ──▶  POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent

  create_client(config) ──▶  match config.provider:
       "anthropic"   → AnthropicClient (api.anthropic.com)
       "openai"      → OpenAiClient (api.openai.com)
       "deepseek"    → OpenAiClient (api.deepseek.com)
       "groq"        → OpenAiClient (api.groq.com)
       "together"    → OpenAiClient (api.together.xyz)
       "xai"         → OpenAiClient (api.x.ai)
       "mistral"     → OpenAiClient (api.mistral.ai)
       "openrouter"  → OpenAiClient (openrouter.ai/api/v1)
       "qwen"        → OpenAiClient (dashscope.aliyuncs.com/compatible-mode/v1)
       "ollama"      → OpenAiClient (localhost:11434/v1)
       "gemini"      → GeminiClient (generativelanguage.googleapis.com)
```

### Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | `install.sh` — OS/arch detection, binary download, PATH append, idempotent | Script runs on Linux x86_64 |
| CP-2 | `OpenAiClient` — OpenAI Chat Completions impl + `base_url` in config + factory for all 8 OpenAI-compatible providers | `cargo test` passes, all 8 providers resolve to correct default URLs |
| CP-3 | `GeminiClient` — Google Generative AI impl + factory integration | `cargo test` passes, Gemini client formats requests correctly |
| CP-4 | Expand `testudo init` — full provider menu (Anthropic, OpenAI, DeepSeek, Groq, Together, xAI, Mistral, OpenRouter, Qwen, Gemini, Ollama) with defaults | Piped-input test produces valid `[llm]` config for each provider |
| CP-5 | `testudo strategy create` wizard — interactive TOML builder with `$EDITOR`, collision check | Wizard completes → `strategy validate` passes |

### CP-1: install.sh

Same as previously specified. Shell detection via `$SHELL`, PATH append to rc file, idempotent re-run.

### CP-2: OpenAI-Compatible Client

**Files:**
- `testudo-cli/src/llm/openai.rs` — **REWRITE** from stub to full implementation
- `testudo-cli/src/llm/client.rs` — **MODIFY** factory to support all providers
- `testudo-cli/src/config.rs` — **MODIFY** add `base_url` to `LlmConfig`

**Provider defaults:**

| Provider key | Default base_url | Default model |
|-------------|------------------|---------------|
| `anthropic` | `https://api.anthropic.com` | `claude-sonnet-4-20250514` |
| `openai` | `https://api.openai.com` | `gpt-4o` |
| `deepseek` | `https://api.deepseek.com` | `deepseek-chat` |
| `groq` | `https://api.groq.com` | `llama-3.3-70b-versatile` |
| `together` | `https://api.together.xyz` | `meta-llama/Llama-3.3-70B-Instruct-Turbo` |
| `xai` | `https://api.x.ai` | `grok-2` |
| `mistral` | `https://api.mistral.ai` | `mistral-large-latest` |
| `openrouter` | `https://openrouter.ai/api/v1` | `anthropic/claude-sonnet-4` |
| `qwen` | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `qwen-max` |
| `gemini` | `https://generativelanguage.googleapis.com` | `gemini-2.5-flash` |
| `ollama` | `http://localhost:11434/v1` | `llama3` |

OpenAI-compatible providers (all except anthropic and gemini) use the `OpenAiClient` with their respective base URLs.

**OpenAiClient key design:**
- Translates `LlmMessage` → OpenAI messages format (role, content, tool_calls, tool_call_id)
- Translates tools → OpenAI `tools[].function` format
- Parses `choices[0].message` back to `LlmResponse`
- Handles `X-Api-Key` header for Mistral, `Authorization: Bearer` for others
- `base_url` in `LlmConfig` overrides the provider default (for custom endpoints)

### CP-3: Gemini Client

**Files:**
- `testudo-cli/src/llm/gemini.rs` — **REWRITE** from stub to full implementation
- `testudo-cli/src/llm/client.rs` — **MODIFY** add gemini match arm

Gemini uses `POST /v1beta/models/{model}:generateContent?key={api_key}` format with API key as query param (simplest auth method). Tool declarations follow Gemini's `functionDeclarations` schema. Response parsing extracts `candidates[0].content.parts[]`.

### CP-4: Init Wizard — Full Provider Menu

Expanded provider selection:

```
── Step 4/6: LLM Configuration ─────────────────

  1. Anthropic (Claude)        api.anthropic.com
  2. OpenAI (GPT)              api.openai.com
  3. DeepSeek                  api.deepseek.com
  4. Groq                      api.groq.com
  5. Together AI               api.together.xyz
  6. xAI (Grok)                api.x.ai
  7. Mistral                   api.mistral.ai
  8. OpenRouter                openrouter.ai
  9. Qwen (Alibaba)            dashscope.aliyuncs.com
 10. Google (Gemini)           generativelanguage.googleapis.com
 11. Ollama (local)            localhost:11434
 12. Custom (enter base URL)   any OpenAI-compatible endpoint

Provider [1]:
```

Each provider has its correct default model. "Custom" prompts for base URL + model + API key.

### CP-5: Strategy Create Wizard

Same as previously specified. Name → description → constraints → symbols → tools → `$EDITOR` for prompt → validate → save.

### Files Summary

| File | Action |
|------|--------|
| `install.sh` | **NEW** |
| `testudo-cli/src/llm/openai.rs` | **REWRITE** (from stub) |
| `testudo-cli/src/llm/gemini.rs` | **REWRITE** (from stub) |
| `testudo-cli/src/llm/ollama.rs` | **DELETE** (ollama uses OpenAiClient) |
| `testudo-cli/src/llm/client.rs` | **MODIFY** — full factory |
| `testudo-cli/src/config.rs` | **MODIFY** — add `base_url` field |
| `testudo-cli/src/cmd.rs` | **MODIFY** — init LLM step, strategy create |
| `testudo-cli/src/strategies/registry.rs` | **MODIFY** — collision check |
| `testudo-cli/src/main.rs` | **MODIFY** — wire Create handler |

---

## Acceptance Criteria

- [ ] `install.sh` downloads binary, installs to PATH, idempotent
- [ ] `OpenAiClient` sends correct OpenAI-format requests and parses responses
- [ ] All 8 OpenAI-compatible providers resolve to correct default URLs
- [ ] Custom `base_url` in config overrides provider default
- [ ] `GeminiClient` sends correct Gemini-format requests and parses responses
- [ ] `create_client("deepseek", ...)` returns a working `OpenAiClient` pointed at `api.deepseek.com`
- [ ] `create_client("gemini", ...)` returns a working `GeminiClient`
- [ ] `testudo init` lists all 11 named providers + custom option
- [ ] `testudo init` with piped input produces valid `[llm]` section for each provider
- [ ] `testudo strategy create` produces valid TOML, rejects duplicate names
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **OpenAI-compatible providers may diverge** — some (Qwen, Together) have slight API differences. Mitigation: `base_url` override lets users point at any compatible endpoint. If a specific provider needs quirks, add a `compat` flags field later.
2. **Gemini uses API key in query param** — less secure than headers but simplest to implement. Mitigation: documented as known; users can use a scoped API key.
3. **CI binary for install.sh** — release tarballs need CI setup (out of scope). Script 404s gracefully.

---

## Completion Signal

1. `install.sh` functional on Linux x86_64
2. 10 LLM providers working (1 Anthropic native, 8 OpenAI-compatible, 1 Gemini native)
3. `testudo init` configures any provider
4. `testudo strategy create` produces valid strategies
5. `cargo clippy --all-targets && cargo test` passes
6. Code committed
