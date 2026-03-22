#!/bin/bash
# Measures content.js bundle size in bytes
set -euo pipefail
cd testudo-extension && bun run build > /dev/null 2>&1
BYTES=$(wc -c < dist/chrome/content.js)
echo "METRIC=$BYTES"
