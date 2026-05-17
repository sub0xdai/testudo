# Testudo Extension

Overlays a trading panel on TradingView. Draw a position tool → Alt+X → trade is sized and routed through Testudo.

## Install

1. Clone and build:
   ```
   bun install && bun run build
   ```
2. Chrome: `chrome://extensions` → Developer mode → Load unpacked → pick `dist/chrome`
3. Firefox: `about:debugging` → This Firefox → Load Temporary Add-on → pick `dist/firefox/manifest.json`

## Usage

- Draw a **long position** or **short position** tool on TradingView
- Hit **Alt+X** — the panel opens with your take-profit and stop-loss pre-filled from the chart
- Your risk config (account %, max size, leverage) is pulled from Testudo
- Hit submit. The engine validates, sizes, and routes the order to your connected exchange

## Dev

```
bun run build      # Chrome + Firefox
bun run test       # Vitest
```

The extension talks to a Testudo API instance. Set `VITE_API_URL` if your backend isn't at `http://localhost:8080`.

## License

AGPL-3.0. See [LICENSE](LICENSE).
