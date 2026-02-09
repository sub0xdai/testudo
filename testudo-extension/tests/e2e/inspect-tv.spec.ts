/**
 * DOM inspector for TradingView position tool.
 *
 * Usage: npx playwright test inspect-tv --headed
 *
 * 1. Browser opens to TradingView SOLUSDT chart
 * 2. Draw a Long/Short Position tool on the chart
 * 3. Double-click it to open properties dialog
 * 4. Click "Resume" in the Playwright Inspector window to run DOM queries
 * 5. Results print to terminal
 */
import { test } from "@playwright/test";

test("inspect TradingView position tool DOM", async ({ browser }) => {
  test.setTimeout(300_000); // 5 min

  const context = await browser.newContext({ viewport: { width: 1400, height: 900 } });
  const page = await context.newPage();

  await page.goto("https://www.tradingview.com/chart/?symbol=BINANCE:SOLUSDT");
  await page.waitForLoadState("networkidle");

  console.log("\n=== TradingView loaded ===");
  console.log("1. Draw a Long Position tool on the chart");
  console.log("2. Double-click it to open the properties dialog");
  console.log("3. Click RESUME in the Playwright Inspector window\n");

  // Pause — opens Playwright Inspector. Click Resume when ready.
  await page.pause();

  console.log("\n=== Running DOM queries ===\n");

  // Query 1: All input elements
  const inputs = await page.evaluate(() => {
    return Array.from(document.querySelectorAll("input")).map((el) => ({
      type: el.type,
      value: el.value,
      placeholder: el.placeholder,
      parentClass: el.parentElement?.className?.substring(0, 80),
      grandparentClass: el.parentElement?.parentElement?.className?.substring(0, 80),
    }));
  });
  console.log(`--- Inputs (${inputs.length}) ---`);
  inputs.forEach((i) => console.log(JSON.stringify(i)));

  // Query 2: Elements with data-name containing relevant keywords
  const dataNames = await page.evaluate(() => {
    const selectors = [
      '[data-name*="risk"]', '[data-name*="reward"]',
      '[data-name*="position"]', '[data-name*="long"]', '[data-name*="short"]',
      '[data-name*="dialog"]', '[data-name*="properties"]', '[data-name*="setting"]',
    ];
    return Array.from(document.querySelectorAll(selectors.join(","))).map((el) => ({
      tag: el.tagName,
      dataName: el.getAttribute("data-name"),
      cls: (el.className || "").substring(0, 80),
      childCount: el.children.length,
    }));
  });
  console.log(`\n--- data-name matches (${dataNames.length}) ---`);
  dataNames.forEach((d) => console.log(JSON.stringify(d)));

  // Query 3: Floating/dialog/popup elements
  const dialogs = await page.evaluate(() => {
    return Array.from(document.querySelectorAll(
      '[class*="floating"], [class*="dialog"], [class*="popup"], [class*="modal"], [class*="properties"], [role="dialog"]'
    )).map((el) => ({
      tag: el.tagName,
      role: el.getAttribute("role"),
      cls: (el.className || "").substring(0, 100),
      childCount: el.children.length,
      textPreview: el.textContent?.substring(0, 120)?.replace(/\s+/g, " "),
    }));
  });
  console.log(`\n--- Dialogs/floating panels (${dialogs.length}) ---`);
  dialogs.forEach((d) => console.log(JSON.stringify(d)));

  // Query 4: overlay-manager-root contents
  const overlay = await page.evaluate(() => {
    const root = document.getElementById("overlap-manager-root");
    if (!root) return "NOT FOUND";
    return root.innerHTML.substring(0, 2000);
  });
  console.log(`\n--- #overlap-manager-root ---`);
  console.log(overlay);

  // Query 5: Search for price-like values in leaf elements
  const priceElements = await page.evaluate(() => {
    return Array.from(document.querySelectorAll("*"))
      .filter((el) => {
        if (el.children.length > 0) return false;
        const t = el.textContent?.trim() || "";
        return /^\d[\d,.]*$/.test(t) && parseFloat(t.replace(",", "")) > 10;
      })
      .map((el) => ({
        tag: el.tagName,
        cls: (el.className || "").substring(0, 60),
        text: el.textContent?.trim(),
        parentCls: el.parentElement?.className?.substring(0, 60),
        gpCls: el.parentElement?.parentElement?.className?.substring(0, 60),
      }));
  });
  console.log(`\n--- Price-like leaf elements (${priceElements.length}) ---`);
  priceElements.forEach((p) => console.log(JSON.stringify(p)));

  // Query 6: Any shadow roots
  const shadows = await page.evaluate(() => {
    return Array.from(document.querySelectorAll("*"))
      .filter((el) => el.shadowRoot)
      .map((el) => `${el.tagName}#${el.id}.${(el.className || "").substring(0, 40)}`);
  });
  console.log(`\n--- Shadow roots (${shadows.length}) ---`);
  shadows.forEach((s) => console.log(s));

  console.log("\n=== Inspection complete ===\n");

  // Pause again so you can review results before browser closes
  await page.pause();

  await context.close();
});
