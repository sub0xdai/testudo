#!/usr/bin/env bash
# Sync brand assets from canonical source to all projects
set -euo pipefail

BRAND="assets/brand"
echo "Syncing brand assets from ${BRAND}/"

# Extension icons
cp "$BRAND/icon16.png" testudo-extension/src/icons/
cp "$BRAND/icon48.png" testudo-extension/src/icons/
cp "$BRAND/icon128.png" testudo-extension/src/icons/
cp "$BRAND/shield.svg" testudo-extension/src/popup/images/
cp "$BRAND/hadrian-wall.jpg" testudo-extension/src/popup/images/

# Journal/Desk public
cp "$BRAND/crest.png" testudo-journal/public/
cp "$BRAND/shield.svg" testudo-journal/public/
cp "$BRAND/favicon.svg" testudo-journal/public/

# Web landing public
cp "$BRAND/crest.png" testudo-web/public/
cp "$BRAND/shield.svg" testudo-web/public/
cp "$BRAND/favicon.svg" testudo-web/public/

echo "Done. All brand assets synced."
