# Build Instructions

## Requirements

- **OS**: Linux, macOS, or Windows
- **Bun**: v1.2+ (https://bun.sh — `curl -fsSL https://bun.sh/install | bash`)
- **Node.js**: v20+ (only needed if not using Bun)

## Steps

```bash
# 1. Install dependencies
bun install

# 2. Build the extension (development)
bun run build

# 3. Build the extension (production — minified, no console logs)
bun run build:prod
```

## Output

- `dist/chrome/` — Chrome/Chromium build (Manifest V3)
- `dist/firefox/` — Firefox build (Manifest V3 with gecko settings)

## Build System

- **Bundler**: esbuild (via `build.ts`)
- **CSS**: Tailwind CSS v4 CLI
- **JSX**: Solid.js (via esbuild-plugin-solid)
- **Format**: IIFE for content scripts and popup, ESM for background service worker

## Verification

Load the unpacked extension from `dist/chrome/` or `dist/firefox/` in the respective browser's extension developer mode.
