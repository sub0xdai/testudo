# Testudo Extension

Overlays a trading panel on TradingView. Draw a position tool → Alt+X → trade is sized and routed through Testudo.

## Install

```sh
bun install && bun run build
```

- Chrome: `chrome://extensions` → Developer mode → Load unpacked → pick `dist/chrome`
- Firefox: `about:debugging` → This Firefox → Load Temporary Add-on → pick `dist/firefox/manifest.json`

## Usage

1. Draw a long position or short position tool on TradingView
2. Hit Alt+X — the panel opens with take-profit and stop-loss pre-filled
3. Your risk config is pulled from Testudo (account %, max size, leverage)
4. Submit. The engine validates, sizes, and routes the order.

## Dev

```sh
bun run build      # Chrome + Firefox
bun run test       # Vitest
```

Set `VITE_API_URL` if your backend isn't at `http://localhost:8080`.

## License

AGPL-3.0. See [LICENSE](LICENSE).
