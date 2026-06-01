# CLI-07-onboarding-distribution — Implementation Plan

## Current State Summary

The testudo CLI harness is fully built (165 tests, 0 clippy warnings). Four gaps block the full user story:

1. **No `install.sh`** — no curl-pipe-bash acquisition path.
2. **Only Anthropic works as LLM** — `create_client()` panics on anything except `"anthropic"`. OpenAI, Gemini, and Ollama are 4-line stubs.
3. **Init skips LLM config** — `run_init()` never prompts for provider, API key, or model.
4. **No strategy creation wizard** — users must hand-write TOML.

## Checkpoints

### CP-1: install.sh ✅
- **Touches**: `install.sh` (NEW, repo root)
- **Tasks**:
  1. Create `install.sh` with OS/arch detection, GitHub Releases download, install to `~/.local/bin`
  2. Shell detection via `$SHELL`, append `export PATH` to rc file (.zshrc/.bashrc/config.fish/.profile)
  3. Idempotency: overwrite binary, guard against duplicate PATH entries
  4. "Next steps" message pointing to `testudo init`
- **Verification**: `bash install.sh` on Linux x86_64 downloads binary, appends PATH, prints next steps. Re-run overwrites cleanly.
- **Commit message**: `feat: install.sh with OS/arch detection and PATH append`
- Completed 2026-06-01 by /skill:vox build

### CP-2: OpenAI-compatible client + factory ✅
- **Touches**: `testudo-cli/src/llm/openai.rs` (rewrite), `testudo-cli/src/llm/client.rs` (modify), `testudo-cli/src/config.rs` (modify)
- **Tasks**:
  1. Add `base_url: Option<String>` to `LlmConfig` in `config.rs`
  2. Implement `OpenAiClient` with `send_message()` — translate LlmMessage → OpenAI format, tools → function calling, parse response
  3. Add provider defaults map (provider key → base_url, default_model)
  4. Update `create_client()` factory: match anthropic (native), gemini (native in CP-3), everything else → `OpenAiClient` with correct default URL
  5. `base_url` in config overrides provider default
  6. Add unit tests for message/tool translation and URL resolution
- **Verification**: `cargo test` passes. `create_client("deepseek", config)` produces OpenAiClient pointed at `https://api.deepseek.com/v1/chat/completions`. Custom `base_url` overrides correctly.
- **Commit message**: `feat: OpenAI-compatible client supporting 8 providers`
- Completed 2026-06-01 by /skill:vox build

### CP-3: Gemini client ✅
- **Touches**: `testudo-cli/src/llm/gemini.rs` (rewrite), `testudo-cli/src/llm/client.rs` (modify)
- **Tasks**:
  1. Implement `GeminiClient` with `send_message()` — translate to Gemini format, tools → functionDeclarations, parse candidates response
  2. Add `"gemini"` arm to `create_client()` factory
  3. Add unit tests for Gemini message/tool translation
- **Verification**: `cargo test` passes. `create_client("gemini", config)` produces GeminiClient.
- **Commit message**: `feat: Google Gemini LLM provider`
- Completed 2026-06-01 by /skill:vox build

### CP-4: testudo init — LLM step with all providers ✅
- **Touches**: `testudo-cli/src/cmd.rs` (modify `run_init`), `testudo-cli/tests/init_tests.rs` (new tests)
- **Tasks**:
  1. Insert LLM step between exchange (step 3) and risk (step 4): 5 steps → 6 steps
  2. Present 12 options: 11 named providers + Custom
  3. Each provider shows its default model and base URL
  4. API key input with non-empty validation
  5. Model input with provider-specific default
  6. Wire `provider`, `api_key`, `model`, and `base_url` (if custom) into Config builder
  7. Add piped-input tests for Anthropic, DeepSeek, Gemini, and Custom providers
- **Verification**: `cargo test` passes. Piped init for `deepseek` produces config with `provider = "deepseek"`, `api_key = "sk-test"`, `model = "deepseek-chat"`.
- **Commit message**: `feat: multi-provider LLM selection in testudo init`
- Completed 2026-06-01 by /skill:vox build

### CP-5: testudo strategy create wizard ✅
- **Touches**: `testudo-cli/src/cmd.rs` (add `run_strategy_create`, add `Create` to `StrategyAction`), `testudo-cli/src/strategies/registry.rs` (collision check), `testudo-cli/src/main.rs` (wire handler), `testudo-cli/src/lib.rs` (export), `testudo-cli/tests/strategies_tests.rs` (new tests)
- **Tasks**:
  1. Add `Create` variant to `StrategyAction`
  2. Implement wizard: name → desc → leverage → symbols → loop config → tools → $EDITOR prompt
  3. $EDITOR fallback: `$EDITOR` → `vim` → `nano` → `vi` → inline input
  4. Add collision check to `StrategyRegistry.add()` (reject if builtin or existing user strategy)
  5. Validate: kebab-case name, non-empty prompt (after comment stripping), at least one symbol
  6. Save to `~/.config/testudo/strategies/<name>.toml`
  7. Wire `StrategyAction::Create` in `main.rs`
  8. Add tests
- **Verification**: `cargo test` passes. `strategy create` with piped input → `strategy validate` passes → `strategy show` displays correctly. Duplicate name rejected.
- **Commit message**: `feat: interactive strategy creation wizard`
- Completed 2026-06-01 by /skill:vox build

---

## Risks

1. **OpenAI-compatible quirks** — some providers need specific headers (Mistral: `X-Api-Key`, others `Authorization: Bearer`). Mitigation: each provider's default header style is in the defaults map. Custom base_url uses `Authorization: Bearer`.
2. **Gemini key in URL** — API key passed as query param, visible in logs. Mitigation: documented tradeoff; production could use header auth with service accounts.
3. **install.sh needs CI releases** — out of scope for this spec. Script is written and tested locally.

Plan ready: 5 checkpoints, ~8-10 hours total. Run `/skill:vox build CLI-07-onboarding-distribution` to start CP-1.
