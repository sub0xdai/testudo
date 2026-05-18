# Specification: Git History Secret Scan and Sanitization

**Spec ID:** CLN-08-secret-history-scan
**Date:** 2026-05-15
**Status:** Draft
**Class:** Infrastructure / Security
**Priority:** P0 — secrets in git history are a dealbreaker for open-source release
**Depends on:** None (parallel-safe — can run alongside CLN-01 through CLN-07)
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

Before open-sourcing, the entire git history must be scanned for secrets. The `.gitignore` is comprehensive (blocks `.env`, `*.pem`, `*.key`, Docker data dirs), but that only protects new commits — not historical ones.

Risk areas:
1. **`.env` files with real credentials** committed before `.env` was added to `.gitignore`
2. **API keys or tokens** in source code (extension code paths, CCXT sidecar configs)
3. **Docker Hub credentials** referenced in CI (`secrets.DOCKER_USERNAME` / `DOCKER_PASSWORD` — these are GitHub Secrets, not in code, but verify)
4. **Sealed secrets** in `testudo-ops/backend/sealed-secret.yml` — these are Kubernetes sealed-secrets (designed to be committed), but verify they're sealed, not plaintext
5. **Private keys** (`*.pem`, `*.key`) ever committed
6. **Wallet addresses or private keys** in test fixtures or constants
7. **JWT secrets, AES keys, SIWE signing keys** in config or source

The `.env.example` file uses `root:root` as example credentials — that's fine (it's an example). But if the real `.env` with production credentials was ever committed, it must be purged.

---

## User Stories

- **As the project owner**, I want absolute certainty that no real credentials exist in git history, so that open-sourcing doesn't expose production infrastructure.
- **As a security-conscious contributor**, I want to trust that the repo has been audited, so that I can contribute without stumbling on leaked secrets.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Run `git filter-repo` or equivalent scan for API key patterns across entire git history | High | Git |
| FR-2 | Run `trufflehog` or `gitleaks` scan on the repository | High | Git |
| FR-3 | Verify `sealed-secret.yml` is a valid sealed-secret (not plaintext) | High | testudo-ops |
| FR-4 | Audit `.env.example` and `sample-secret.md` for accidental real credentials | High | Root |
| FR-5 | Run `git log -p` grep for common secret patterns: `sk-`, `api_key`, `AES_KEY`, `JWT_SECRET`, `0x[a-fA-F0-9]{64}` | High | Git |
| FR-6 | Document the scan methodology and results | Medium | Security |
| FR-7 | If secrets are found: purge with `git filter-repo`, rotate exposed credentials, force-push | High | Git/infra |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Automated scan with `gitleaks` or `trufflehog` | Report of potential secrets |
| CP-2 | Manual grep for common patterns across full history | No hits, or all hits are false positives |
| CP-3 | Verify sealed-secret validity and audit `.env.example` | No plaintext secrets in repo |
| CP-4 | If secrets found: purge + rotate + document | Clean history, rotated credentials |

### Scan Commands

#### Option A: `gitleaks` (fast, purpose-built)

```bash
# Install
brew install gitleaks  # macOS
# or: go install github.com/gitleaks/gitleaks/v8@latest

# Scan entire git history with verbose output
cd /home/m0xu/1-projects/testudo
gitleaks detect --source . --verbose --report-format json --report-path gitleaks-report.json

# Review findings
gitleaks detect --source . --verbose
```

#### Option B: `trufflehog` (deeper, scans more patterns)

```bash
# Install
brew install trufflehog  # macOS
# or: docker run --rm -v "$PWD:/pwd" trufflesecurity/trufflehog:latest git file:///pwd

# Scan git history
cd /home/m0xu/1-projects/testudo
trufflehog git file://. --only-verified
```

#### Manual Grep Patterns

```bash
cd /home/m0xu/1-projects/testudo

# API key patterns
git log -p --all | grep -iE '(api[_-]?key|apikey|secret[_-]?key|private[_-]?key)\s*[:=]\s*["'"'"']?[a-zA-Z0-9_\-]{20,}' | head -20

# JWT / encryption keys
git log -p --all | grep -iE '(AES_KEY|JWT_SECRET|ENCRYPTION_KEY|SIGNING_KEY)\s*[:=]' | head -20

# Ethereum private keys (64 hex chars)
git log -p --all | grep -oP '0x[a-fA-F0-9]{64}' | sort -u | head -20

# AWS / GCP keys
git log -p --all | grep -iE '(AKIA[0-9A-Z]{16}|GOOG[A-Z0-9]{20,}|sk-[a-zA-Z0-9]{32,})' | head -20

# Generic password assignments
git log -p --all | grep -iE '(password|passwd|pwd)\s*[:=]\s*["'"'"'][^"'"'"']{4,}' | grep -v 'example' | grep -v 'your_' | head -20
```

### Sealed-Secret Verification

```bash
# Check if the sealed-secret is valid Kubernetes format
cat testudo-ops/backend/sealed-secret.yml | head -5

# A valid sealed-secret has:
#   kind: SealedSecret
#   spec.encryptedData (NOT spec.stringData or spec.data)
# If spec.stringData or spec.data is present with readable values, it's PLAINTEXT
```

### `.env.example` Audit

```bash
cat .env.example
# All values should be examples: root, localhost, your_*, etc.
# If any value looks like a real credential, flag it.
```

### If Secrets Are Found: Purge Procedure

1. **Document** every secret found (path, line, commit hash)
2. **Rotate** the exposed credential in production immediately
3. **Purge** using `git filter-repo`:
   ```bash
   git filter-repo --path .env --invert-paths --force
   # Or for content-based removal:
   git filter-repo --replace-text <(echo 'REAL_SECRET_VALUE==>REDACTED')
   ```
4. **Force push** to remote (coordinate with any collaborators)
5. **Verify** with a fresh clone that secrets are gone

### Paved Roads

- `.gitignore` already covers `.env`, `*.pem`, `*.key`, `*.crt`, Docker data dirs
- `secrets.DOCKER_USERNAME` in CI — these are GitHub Secrets, not in repo code
- Standard practice: `gitleaks` in CI pre-commit hook going forward

### Files

- `gitleaks-report.json` — scan output (do NOT commit if it contains real secrets)
- `CLN-08-scan-results.md` — **NEW**, documents methodology and findings (commit this)
- `.pre-commit-config.yaml` — **NEW** (optional), add gitleaks hook for future protection

### Dependencies Added

- `gitleaks` or `trufflehog` — dev tool only, not a project dependency

---

## Acceptance Criteria

- [ ] `gitleaks` or `trufflehog` scan completes with zero verified secrets
- [ ] Manual grep for API keys, private keys, wallet keys returns zero real hits
- [ ] `sealed-secret.yml` verified as properly sealed (not plaintext)
- [ ] `.env.example` contains only example values
- [ ] Scan methodology documented in `CLN-08-scan-results.md`
- [ ] If secrets were purged: affected credentials rotated in production
- [ ] If secrets were purged: `git log` confirms they're no longer in history

---

## Risks

1. **Force-pushing rewrites shared history.** If other collaborators have clones, they must re-clone. Mitigation: coordinate with any team members before force-push.
2. **Sealed-secret may be plaintext.** The `sealed-secret.yml` should be encrypted — if it's not, the Bitnami sealed-secrets controller in the Kubernetes cluster was misconfigured. Mitigation: re-seal with `kubeseal` and rotate the underlying secret.
3. **CI workflows reference sealed secrets.** `deploy.yml` references `secrets.DOCKER_USERNAME` and `secrets.DOCKER_PASSWORD` — these are GitHub Encrypted Secrets (safe). Verify they're not hardcoded in the workflow file.

---

## Completion Signal

This spec is complete when:
1. Automated secret scan reports zero findings
2. Manual grep across git history finds zero real credentials
3. Scan results documented
4. Any found secrets are purged and rotated
5. Code committed (and force-pushed if history was rewritten)
