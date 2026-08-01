# Optimization Target: bundle-size

## Goal
Reduce the size of `content.js` (360K) injected into every TradingView page. Smaller injection = faster page load, smaller extension package. Secondary target: `background.js` (340K).

"Better" means: fewer bytes in `dist/chrome/content.js` with zero functionality loss.

## Target Files
- `testudo-extension/src/content.ts`
- `testudo-extension/src/modal.tsx`
- `testudo-extension/src/scraper.ts`
- `testudo-extension/build.ts`

## Constraints
- Do NOT modify test files
- Do NOT remove any user-facing functionality
- Do NOT change the manifest.json content script declarations
- Do NOT add new dependencies
- Do NOT modify `background.ts` or popup components
- The modal must still render correctly inside Shadow DOM on TradingView
- The scraper must still extract position data from TradingView DOM

## Strategy Hints
- Audit which Zod schemas are imported by content.ts — it may pull in the full schemas.ts when only a subset is needed
- Check if the modal's inline CSS (MODAL_STYLES, ~250 lines) can be reduced or deduplicated
- Evaluate whether scraper strategies that are never hit can be pruned
- Check esbuild tree-shaking effectiveness — are dead exports being bundled?
- Consider lazy-loading the modal (dynamic import) so it's not in the initial content.js payload

## Verification
```bash
cd testudo-extension && bun run build
```

## Metric
- METRIC_DIRECTION=MINIMIZE
- Benchmark: .specify/optimize/bundle-size/benchmark.sh
- BENCHMARK_RUNS=1
