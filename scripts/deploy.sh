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

# Patch SDK: ClassTransfer.usd_size → usdc (Hyperliquid expects "usdc" not "usdSize")
SDK_ACTIONS=$(find /root/.cargo/registry/src -path '*/hyperliquid-sdk-rs-*/src/types/actions.rs' 2>/dev/null | head -1)
if [ -n "$SDK_ACTIONS" ]; then
  sed -i 's/pub usd_size: u64,/pub usdc: u64,/' "$SDK_ACTIONS"
fi

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
