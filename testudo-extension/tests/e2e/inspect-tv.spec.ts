/**
 * TradingView API inspector for position tool data extraction.
 *
 * Usage: npx playwright test inspect-tv --headed
 *
 * 1. Browser opens to TradingView SOLUSDT chart
 * 2. API detection runs automatically on load
 * 3. Draw a Long/Short Position tool on the chart
 * 4. Single-click to SELECT it (blue handles visible)
 * 5. Click "Resume" in the Playwright Inspector window
 * 6. API queries extract shape data → results print to terminal
 */
import { test } from "@playwright/test";

test("inspect TradingView position tool API", async ({ browser }) => {
  test.setTimeout(300_000); // 5 min

  const context = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();

  await page.goto("https://www.tradingview.com/chart/?symbol=BINANCE:SOLUSDT");
  await page.waitForLoadState("networkidle");

  // --- Phase 1: API detection (runs immediately on load) ---

  const apiDetection = await page.evaluate(() => {
    const w = window as any;
    const result: Record<string, any> = {};

    // Find widget with activeChart
    result.windowKeys = Object.keys(w).filter((k) =>
      /chart|trading|widget|tv/i.test(k)
    );

    // Get chart via TradingViewApi
    try {
      const widget = w.TradingViewApi || w.ChartApiInstance;
      if (!widget || typeof widget.activeChart !== "function") {
        result.error = "No widget with activeChart found";
        return result;
      }
      result.widgetSource = w.TradingViewApi ? "TradingViewApi" : "ChartApiInstance";

      const chart = widget.activeChart();
      result.hasChart = !!chart;

      // Get ALL chart methods (walk prototype chain)
      const allMethods: string[] = [];
      let proto = chart;
      let depth = 0;
      while (proto && depth < 5) {
        proto = Object.getPrototypeOf(proto);
        if (!proto) break;
        const methods = Object.getOwnPropertyNames(proto).filter(
          (m) => typeof chart[m] === "function" && m !== "constructor"
        );
        allMethods.push(...methods);
        depth++;
      }
      result.allChartMethods = [...new Set(allMethods)].sort();
      result.hasGetAllShapes = typeof chart.getAllShapes === "function";
      result.hasGetShapeById = typeof chart.getShapeById === "function";
      result.hasChartModel = typeof chart.chartModel === "function";

      // Pre-draw: shapes should be empty
      if (result.hasGetAllShapes) {
        result.shapesOnLoad = chart.getAllShapes();
      }
    } catch (e) {
      result.error = String(e);
    }

    return result;
  });
  console.log("\n=== Phase 1: API Detection (on load) ===");
  console.log(JSON.stringify(apiDetection, null, 2));

  console.log("\n--- Instructions ---");
  console.log("1. Draw a Long Position tool on the chart");
  console.log("2. Single-click to SELECT it (blue handles visible)");
  console.log("3. Click RESUME in the Playwright Inspector window\n");

  await page.pause();

  // --- Phase 2: Read position tool data via API ---

  console.log("\n=== Phase 2: Reading Shape Data ===\n");

  const shapeData = await page.evaluate(() => {
    const w = window as any;
    const result: Record<string, any> = {};

    try {
      const chart = w.TradingViewApi.activeChart();

      // Get all shapes
      const shapes = chart.getAllShapes();
      result.shapes = shapes;
      result.shapeCount = shapes.length;

      // Find position tools specifically
      const positionTools = shapes.filter((s: any) =>
        /risk|reward|position/i.test(s.name || "")
      );
      result.positionTools = positionTools;

      // For each shape, try getShapeById → getProperties
      result.shapeDetails = shapes.map((s: any) => {
        const detail: Record<string, any> = { id: s.id, name: s.name };

        try {
          if (typeof chart.getShapeById !== "function") {
            detail.error = "getShapeById not available";
            return detail;
          }

          const api = chart.getShapeById(s.id);
          if (!api) {
            detail.error = "getShapeById returned null";
            return detail;
          }

          // Dump all methods on the shape API
          const methods: string[] = [];
          let proto = api;
          let depth = 0;
          while (proto && depth < 5) {
            proto = Object.getPrototypeOf(proto);
            if (!proto) break;
            methods.push(...Object.getOwnPropertyNames(proto).filter(
              (m) => typeof api[m] === "function" && m !== "constructor"
            ));
            depth++;
          }
          detail.methods = [...new Set(methods)].sort();

          // Try getProperties
          if (typeof api.getProperties === "function") {
            const props = api.getProperties();
            // Serialize, handling any non-serializable values
            detail.properties = JSON.parse(JSON.stringify(props, (_, v) => {
              if (typeof v === "function") return "[function]";
              if (typeof v === "bigint") return v.toString();
              return v;
            }));
          }

          // Try getPoints
          if (typeof api.getPoints === "function") {
            detail.points = api.getPoints();
          }

        } catch (e) {
          detail.error = String(e);
        }

        return detail;
      });

    } catch (e) {
      result.error = String(e);
    }

    return result;
  });
  console.log("--- Shape Data ---");
  console.log(JSON.stringify(shapeData, null, 2));

  // Also try chartModel() for deeper access
  const modelData = await page.evaluate(() => {
    const w = window as any;
    const result: Record<string, any> = {};

    try {
      const chart = w.TradingViewApi.activeChart();

      if (typeof chart.chartModel !== "function") {
        result.error = "chartModel not available";
        return result;
      }

      const model = chart.chartModel();
      if (!model) {
        result.error = "chartModel returned null";
        return result;
      }

      // Get model methods
      const methods: string[] = [];
      let proto = model;
      let depth = 0;
      while (proto && depth < 3) {
        proto = Object.getPrototypeOf(proto);
        if (!proto) break;
        methods.push(...Object.getOwnPropertyNames(proto).filter(
          (m) => typeof model[m] === "function" && m !== "constructor"
        ));
        depth++;
      }

      // Filter for drawing/shape related methods
      result.drawingMethods = methods.filter((m) =>
        /draw|shape|line|tool|source|entity|selection/i.test(m)
      );

      // Try to get data sources (drawings are data sources in TradingView)
      if (typeof model.dataSources === "function") {
        const sources = model.dataSources();
        result.dataSourceCount = sources?.length;
        result.dataSourceTypes = sources?.map((s: any) => ({
          type: s?.constructor?.name,
          toolName: s?.toolname?.(),
          hasProperties: typeof s?.properties === "function",
        })).filter((s: any) => s.toolName);
      }

      // Try selection
      if (typeof model.selection === "function") {
        const sel = model.selection();
        result.selectionType = typeof sel;
        if (sel) {
          const selMethods = Object.getOwnPropertyNames(
            Object.getPrototypeOf(sel) || {}
          ).filter((m) => typeof sel[m] === "function");
          result.selectionMethods = selMethods;
        }
      }

    } catch (e) {
      result.error = String(e);
    }

    return result;
  });
  console.log("\n--- Chart Model Data ---");
  console.log(JSON.stringify(modelData, null, 2));

  // Check floating toolbar for auto-click fallback
  const toolbar = await page.evaluate(() => {
    const tb = document.querySelector('[data-name="floating-toolbar"], .tv-floating-toolbar');
    if (!tb) return { found: false };
    return {
      found: true,
      buttons: Array.from(tb.querySelectorAll("button, [role='button']")).map((b) => ({
        dataName: b.getAttribute("data-name"),
        title: b.getAttribute("title") || b.getAttribute("aria-label"),
      })),
    };
  });
  console.log("\n--- Floating Toolbar ---");
  console.log(JSON.stringify(toolbar, null, 2));

  console.log("\n=== Inspection complete ===\n");

  await page.pause();
  await context.close();
});
