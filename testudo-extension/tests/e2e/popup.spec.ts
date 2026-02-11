import { test, expect } from "./fixtures/extension";

/** Click "continue without account" to bypass auth gate and reach MainView */
async function bypassAuthGate(page: import("@playwright/test").Page) {
  const paperBtn = page.locator('[data-testid="paper-mode-btn"]');
  await paperBtn.waitFor({ state: "visible", timeout: 5000 });
  await paperBtn.click();
  // Wait for main view to render
  await page.locator('[data-testid="trade-management"]').waitFor({ state: "visible", timeout: 5000 });
}

test.describe("Auth Gate", () => {
  test("shows auth section on fresh install", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Auth section renders with title
    await expect(page.locator("h1")).toHaveText("TESTUDO");

    // Login form visible
    const authSection = page.locator('[data-testid="auth-section"]');
    await expect(authSection).toBeVisible();

    // Email and password fields
    await expect(page.locator('[data-testid="login-email"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-password"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-btn"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-btn"]')).toHaveText("LOGIN");

    // Paper mode bypass button
    await expect(page.locator('[data-testid="paper-mode-btn"]')).toBeVisible();

    await page.close();
  });

  test("continue without account navigates to main view", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    await bypassAuthGate(page);

    // Main view is now visible
    await expect(page.locator('[data-testid="trade-management"]')).toBeVisible();
    await expect(page.locator('[data-testid="active-orders"]')).toBeVisible();
    await expect(page.locator('[data-testid="mode-toggle"]')).toBeVisible();
    await expect(page.locator('[data-testid="status-bar"]')).toBeVisible();

    // Footer shows PAPER ONLY
    await expect(page.locator('[data-testid="footer-paper"]')).toHaveText("PAPER ONLY");

    await page.close();
  });
});

test.describe("Main View", () => {
  test("trade management renders with default preset", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    // Risk % default is 1
    await expect(page.locator('[data-testid="risk-percent"]')).toHaveValue("1");

    // Break-even default is 50
    await expect(page.locator('[data-testid="break-even-at"]')).toHaveValue("50");

    // Trailing stop is OFF by default
    await expect(page.locator('[data-testid="trailing-toggle"]')).toHaveText("OFF");

    // Partial TP is OFF by default
    await expect(page.locator('[data-testid="partial-tp-toggle"]')).toHaveText("OFF");

    await page.close();
  });

  test("trade management settings persist to storage", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    // Change risk percent
    const riskInput = page.locator('[data-testid="risk-percent"]');
    await riskInput.fill("2.5");
    await riskInput.dispatchEvent("change");

    // Enable trailing stop
    await page.locator('[data-testid="trailing-toggle"]').click();
    await expect(page.locator('[data-testid="trailing-toggle"]')).toHaveText("ON");

    // Reload and verify persistence — should skip auth gate (paperOnly stored)
    await page.reload();
    await page.locator('[data-testid="trade-management"]').waitFor({ state: "visible", timeout: 5000 });
    await expect(page.locator('[data-testid="risk-percent"]')).toHaveValue("2.5");
    await expect(page.locator('[data-testid="trailing-toggle"]')).toHaveText("ON");

    await page.close();
  });

  test("status bar shows disconnected state", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    await expect(page.locator('[data-testid="status-text"]')).toHaveText("Disconnected");
    await expect(page.locator('[data-testid="status-dot"]')).toHaveAttribute("data-state", "disconnected");

    await page.close();
  });

  test("balance section renders", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    // Balance section exists
    const balanceSection = page.locator('[data-testid="balance-section"]');
    await expect(balanceSection).toBeVisible();

    // Should show "unavailable" or actual values (no backend = unavailable)
    const sectionText = await balanceSection.textContent();
    expect(sectionText).toContain("Account");

    await page.close();
  });

  test("mode toggle shows PAPER button in paper-only mode", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    // Paper button is visible and active
    const paperBtn = page.locator('[data-testid="mode-paper"]');
    await expect(paperBtn).toBeVisible();
    await expect(paperBtn).toHaveText("PAPER");

    // Live button hidden in paper-only mode
    await expect(page.locator('[data-testid="mode-live"]')).not.toBeVisible();

    await page.close();
  });
});

test.describe("Settings View", () => {
  test("settings accessible via gear icon and shows backend URL", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    // Click settings gear
    await page.locator('[data-testid="settings-btn"]').click();

    // Settings view renders
    await expect(page.locator('[data-testid="backend-url"]')).toBeVisible();
    await expect(page.locator('[data-testid="backend-url"]')).toHaveValue("http://localhost:8080");

    await expect(page.locator('[data-testid="ws-url"]')).toBeVisible();
    await expect(page.locator('[data-testid="ws-url"]')).toHaveValue("ws://localhost:4000");

    // Back button returns to main view
    await page.locator('[data-testid="settings-back"]').click();
    await expect(page.locator('[data-testid="trade-management"]')).toBeVisible();

    await page.close();
  });

  test("settings save persists backend URL", async ({ context, extensionId }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await bypassAuthGate(page);

    // Open settings
    await page.locator('[data-testid="settings-btn"]').click();

    const backendInput = page.locator('[data-testid="backend-url"]');
    await backendInput.fill("http://my-server:9090");
    await backendInput.dispatchEvent("change");

    // Reload and verify persistence
    await page.reload();
    await page.locator('[data-testid="settings-btn"]').waitFor({ state: "visible", timeout: 5000 });
    await page.locator('[data-testid="settings-btn"]').click();
    await expect(page.locator('[data-testid="backend-url"]')).toHaveValue("http://my-server:9090");

    // Restore default
    await page.locator('[data-testid="backend-url"]').fill("http://localhost:8080");
    await page.locator('[data-testid="backend-url"]').dispatchEvent("change");

    await page.close();
  });
});
