#!/bin/bash
# Measures total line count of background.ts (proxy for boilerplate reduction)
set -euo pipefail
LINES=$(wc -l < testudo-extension/src/background.ts)
echo "METRIC=$LINES"
