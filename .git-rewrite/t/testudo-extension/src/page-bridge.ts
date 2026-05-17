// EXT-43: Main-World Bridge for TradingView Chart API
// Runs in the PAGE context (MAIN world), injected by content script via <script> tag.
// Accesses window.TradingViewApi / window.ChartApiInstance / window.tvWidget directly.
// Communicates with content script via window.postMessage.

(function () {
  const MSG_REQUEST = "TESTUDO_BRIDGE_REQUEST";
  const MSG_RESPONSE = "TESTUDO_BRIDGE_RESPONSE";
  const MSG_READY = "TESTUDO_BRIDGE_READY";

  // --- Widget Discovery ---

  function findChartWidget(): any | null {
    const w = window as any;

    // Tier 1: Captured by EXT-46 constructor hook (handles closure-stored widgets)
    if (w.__TESTUDO_TV_WIDGET__ && typeof w.__TESTUDO_TV_WIDGET__.activeChart === "function") {
      return w.__TESTUDO_TV_WIDGET__;
    }

    // Tier 2: Known global names (fast path — covers tradingview.com)
    for (const name of ["TradingViewApi", "ChartApiInstance", "tvWidget"]) {
      const val = w[name];
      if (val && typeof val.activeChart === "function") return val;
    }

    // Tier 3: Window property scan (catches any global variable name)
    try {
      for (const key of Object.getOwnPropertyNames(w)) {
        try {
          const val = w[key];
          if (
            val &&
            typeof val === "object" &&
            !Array.isArray(val) &&
            typeof val.activeChart === "function"
          ) {
            return val;
          }
        } catch {
          /* skip cross-origin frames, throwing getters */
        }
      }
    } catch {
      /* getOwnPropertyNames itself can fail on some environments */
    }

    return null;
  }

  // --- Tick Size Calculation ---

  interface TickSizeResult {
    value: number;
    source: string;
  }

  function getTickSize(chart: any, entryPrice: number): TickSizeResult {
    // Try 1: Price formatter internals
    try {
      if (typeof chart.priceFormatter === "function") {
        const fmt = chart.priceFormatter();
        if (fmt) {
          const minMove = fmt._minMove ?? fmt.minMove;
          const priceScale = fmt._priceScale ?? fmt.priceScale;
          if (typeof minMove === "number" && typeof priceScale === "number" && priceScale > 0) {
            return { value: minMove / priceScale, source: "priceFormatter" };
          }
        }
      }
    } catch { /* continue */ }

    // Try 2: Series symbol info
    try {
      if (typeof chart.getSeries === "function") {
        const series = chart.getSeries();
        if (series) {
          const info = typeof series.symbolInfo === "function"
            ? series.symbolInfo()
            : series._symbolInfo || series.symbolInfo;
          if (info) {
            const mm = info.minmov ?? info.minmovement ?? info.min_move;
            const ps = info.pricescale ?? info.price_scale;
            if (typeof mm === "number" && typeof ps === "number" && ps > 0) {
              return { value: mm / ps, source: "symbolInfo" };
            }
          }
        }
      }
    } catch { /* continue */ }

    // Fallback: derive from entry price decimal places
    const str = entryPrice.toString();
    const dot = str.indexOf(".");
    if (dot >= 0) {
      return { value: Math.pow(10, -(str.length - dot - 1)), source: "decimalFallback" };
    }
    return { value: 0.01, source: "hardcodedFallback" };
  }

  // --- Position Tool Extraction ---

  interface PositionToolData {
    entry: number;
    stop: number;
    target: number;
    side: "LONG" | "SHORT";
  }

  function getPositionTool(): PositionToolData | null {
    const widget = findChartWidget();
    if (!widget) return null;

    const chart = widget.activeChart();
    if (!chart || typeof chart.getAllShapes !== "function") return null;

    const shapes: Array<{ id: string; name: string }> = chart.getAllShapes();
    if (!Array.isArray(shapes) || shapes.length === 0) return null;

    // Find position tools — prefer last one (most recently drawn)
    const positionTool = [...shapes]
      .reverse()
      .find((s) => s.name === "long_position" || s.name === "short_position");
    if (!positionTool) return null;

    if (typeof chart.getShapeById !== "function") return null;
    const api = chart.getShapeById(positionTool.id);
    if (!api) return null;

    const side: "LONG" | "SHORT" = positionTool.name === "long_position" ? "LONG" : "SHORT";

    // Get entry price from anchor points
    if (typeof api.getPoints !== "function") return null;
    const points = api.getPoints();
    if (!Array.isArray(points) || points.length === 0) return null;

    const entry = points[0]?.price;
    if (typeof entry !== "number" || entry <= 0) return null;

    // Get stop/target levels from properties
    if (typeof api.getProperties !== "function") return null;
    const props = api.getProperties();
    if (!props || typeof props.stopLevel !== "number" || typeof props.profitLevel !== "number") return null;

    const tick = getTickSize(chart, entry);
    const stopDist = props.stopLevel * tick.value;
    const profitDist = props.profitLevel * tick.value;

    const stop = side === "LONG" ? entry - stopDist : entry + stopDist;
    const target = side === "LONG" ? entry + profitDist : entry - profitDist;

    // Validate prices
    if (stop <= 0 || target <= 0) return null;
    if (side === "LONG" && (stop >= entry || target <= entry)) return null;
    if (side === "SHORT" && (stop <= entry || target >= entry)) return null;

    return { entry, stop, target, side };
  }

  // --- Symbol Extraction ---

  function getSymbol(): string | null {
    const widget = findChartWidget();
    if (!widget) return null;

    try {
      const chart = widget.activeChart();
      if (chart && typeof chart.symbol === "function") {
        let sym = chart.symbol();
        if (typeof sym === "string" && sym.length > 0) {
          // Strip exchange prefix: "BITSTAMP:BTCUSD" → "BTCUSD"
          if (sym.includes(":")) sym = sym.split(":").pop()!;
          // Strip perpetual suffixes
          sym = sym.replace(/\.P$/, "").replace(/PERP$/, "");
          // CEX-08: Strip exchange suffix (.Bybit, .Binance, etc.)
          sym = sym.replace(/\.(Bybit|Binance|OKX|Bitget|Gate|Phemex|BloFin)$/i, "");
          // Strip TradingView continuous contract prefix ".M"
          sym = sym.replace(/^\.M(?=[A-Z]{2,})/, "");
          // Strip remaining leading dots
          sym = sym.replace(/^\.+/, "");
          return sym;
        }
      }
    } catch { /* silent */ }

    return null;
  }

  // --- Message Handler ---

  window.addEventListener("message", (event: MessageEvent) => {
    if (event.source !== window) return;
    if (!event.data || event.data.type !== MSG_REQUEST) return;

    const { action, id } = event.data;
    let data: any = null;

    switch (action) {
      case "probe":
        data = { widgetFound: findChartWidget() !== null };
        break;
      case "getPositionTool":
        data = getPositionTool();
        break;
      case "getSymbol":
        data = getSymbol();
        break;
      default:
        data = { error: `Unknown action: ${action}` };
    }

    window.postMessage({ type: MSG_RESPONSE, id, data }, "*");
  });

  // Signal that bridge is ready
  window.postMessage({ type: MSG_READY }, "*");
})();
