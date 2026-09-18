#!/usr/bin/env bash
# Testudo Production Deploy Script
# Usage: ssh root@YOUR_DROPLET_IP 'bash -s' < scripts/deploy.sh
#   or:  copy to droplet and run directly

set -euo pipefail

TESTUDO_DIR="/opt/testudo"
JOURNAL_DIST="/var/www/testudo-journal"

echo "=== TESTUDO DEPLOY ==="
echo ""

# 1. Pull latest code
echo "[1/6] Pulling latest code..."
cd "$TESTUDO_DIR"
git checkout -- testudo-journal/dist/ testudo-journal/bun.lock 2>/dev/null || true

# One-time: if testudo-exchange is still an old untracked submodule directory
# (not a proper git-tracked directory), blast it so the merge succeeds.
if [ -d "$TESTUDO_DIR/testudo-exchange" ] && ! git ls-files --error-unmatch testudo-exchange/Cargo.toml >/dev/null 2>&1; then
  echo "  ↳ removing old untracked testudo-exchange directory"
  rm -rf "$TESTUDO_DIR/testudo-exchange"
fi

git pull --ff-only

# Pull submodules that are proper repos (testudo-exchange is now part of main repo)
for sub in testudo-journal testudo-cex safe-cex-sub0; do
  if [ -d "$TESTUDO_DIR/$sub/.git" ]; then
    echo "  ↳ pulling $sub"
    cd "$TESTUDO_DIR/$sub" && git pull --ff-only
  fi
done

# 2. Build Rust backend
echo ""
echo "[2/6] Building Rust backend..."
cd "$TESTUDO_DIR/testudo-exchange"

# Patch SDK: ClassTransfer field usd_size -> usdc.
#
# Hyperliquid expects the wire field "usdc", but the struct derives camelCase
# from `usd_size`, so it sends "usdSize" and the API rejects the action. The
# rename breaks the struct's own construction site, which builds it with
# field-init shorthand, so the field and that call site have to move together.
# Renaming only the field is a hard compile error (E0560), not a silent bug.
SDK_ROOT=$(find /root/.cargo/registry/src -maxdepth 2 -type d \
  -name 'hyperliquid-sdk-rs-*' | sort | head -1)
if [ -z "$SDK_ROOT" ]; then
  echo "  x hyperliquid-sdk-rs not found in the cargo registry; cannot patch"
  exit 1
fi
SDK_ACTIONS="$SDK_ROOT/src/types/actions.rs"
SDK_EXCHANGE="$SDK_ROOT/src/providers/exchange/mod.rs"

sed -i 's/pub usd_size: u64,/pub usdc: u64,/' "$SDK_ACTIONS"
sed -i 's/ClassTransfer { usd_size, to_perp }/ClassTransfer { usdc: usd_size, to_perp }/' \
  "$SDK_EXCHANGE"

# Verify both halves. A silent no-op (SDK version bump, registry path change)
# would otherwise surface as a confusing compile error, or worse as a build
# that puts "usdSize" on the wire and fails at trade time.
grep -q 'pub usdc: u64,' "$SDK_ACTIONS" \
  || { echo "  x usdc field rename did not apply"; exit 1; }
grep -q 'ClassTransfer { usdc: usd_size, to_perp }' "$SDK_EXCHANGE" \
  || { echo "  x ClassTransfer patch did not apply"; exit 1; }
echo "  > SDK patched: ClassTransfer.usd_size -> usdc (field + call site)"

find crates -name "*.rs" -exec touch {} + 2>/dev/null || true  # bust cargo cache after git pull
cargo build --release 2>&1 | tail -3

# 3. Build safe-cex (if needed)
echo ""
echo "[3/6] Building safe-cex..."
cd "$TESTUDO_DIR/safe-cex-sub0"
if [ ! -d "dist" ] || [ "$(git rev-parse HEAD)" != "$(cat dist/.git-rev 2>/dev/null || echo '')" ]; then
  bun install --frozen-lockfile 2>/dev/null || bun install
  bun x tsc
  git rev-parse HEAD > dist/.git-rev
  # Copy built dist into cex sidecar's node_modules
  rm -rf "$TESTUDO_DIR/testudo-cex/node_modules/safe-cex/dist"
  cp -r dist "$TESTUDO_DIR/testudo-cex/node_modules/safe-cex/"
  echo "  ↳ rebuilt and copied to cex sidecar"
else
  echo "  ↳ unchanged, skipping"
fi

# 4. Install CEX sidecar deps
echo ""
echo "[4/6] CEX sidecar deps..."
cd "$TESTUDO_DIR/testudo-cex"
bun install 2>/dev/null || bun install

# 5. Build and deploy journal (Desk)
echo ""
echo "[5/6] Building journal..."
cd "$TESTUDO_DIR/testudo-journal"
bun install --frozen-lockfile 2>/dev/null || bun install
VITE_BASE_PATH=/ VITE_API_URL=https://api.testudo.vip bun run build
rm -rf "$JOURNAL_DIST"/*
cp -r dist/* "$JOURNAL_DIST/"
echo "  ↳ deployed to $JOURNAL_DIST"

# 6. Restart services
echo ""
echo "[6/6] Restarting services..."
systemctl restart testudo-api testudo-ws testudo-cex
sleep 2

# Verify
echo ""
echo "=== VERIFICATION ==="
API=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/api/v1/health)
CEX=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3100/health 2>/dev/null || echo "down")
WS=$(systemctl is-active testudo-ws)

echo "  API:  $API"
echo "  CEX:  $CEX"
echo "  WS:   $WS"

if [ "$API" = "200" ] && [ "$WS" = "active" ]; then
  echo ""
  echo "=== DEPLOY COMPLETE ==="
else
  echo ""
  echo "=== DEPLOY FAILED — check logs ==="
  echo "  journalctl -u testudo-api --no-pager -n 20"
  echo "  journalctl -u testudo-ws --no-pager -n 20"
  echo "  journalctl -u testudo-cex --no-pager -n 20"
  exit 1
fi
