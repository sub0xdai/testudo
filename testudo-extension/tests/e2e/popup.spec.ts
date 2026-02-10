import { test, expect } from "./fixtures/extension";

test.describe("Popup UI", () => {
  test("renders with default settings and disconnected status", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Title renders
    await expect(page.locator("h1")).toHaveText("Testudo Sniper");

    // Default backend URL
    const backendInput = page.locator('[data-testid="backend-url"]');
    await expect(backendInput).toHaveValue("http://localhost:8080");

    // Default WS URL
    const wsInput = page.locator('[data-testid="ws-url"]');
    await expect(wsInput).toHaveValue("ws://localhost:4000");

    // Paper mode is active by default (emerald background)
    const paperBtn = page.locator('[data-testid="mode-paper"]');
    await expect(paperBtn).toContainText("Paper");

    // Status shows disconnected (no WS server running)
    await expect(page.locator('[data-testid="status-text"]')).toHaveText("Disconnected");

    await page.close();
  });

  test("toggling execution mode persists to storage", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Click Live toggle
    const liveBtn = page.locator('[data-testid="mode-live"]');
    await liveBtn.click();

    // Reload popup and verify persistence
    await page.reload();
    // Live button should have red background class
    const liveBtnReloaded = page.locator('[data-testid="mode-live"]');
    await expect(liveBtnReloaded).toHaveClass(/bg-red-500/);

    // Switch back to paper
    await page.locator('[data-testid="mode-paper"]').click();
    await page.reload();
    await expect(page.locator('[data-testid="mode-paper"]')).toHaveClass(/bg-emerald-400/);

    await page.close();
  });

  test("settings save persists backend URL", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    const backendInput = page.locator('[data-testid="backend-url"]');
    await backendInput.fill("http://my-server:9090");
    // Trigger change event (settings save on change)
    await backendInput.dispatchEvent("change");

    // Save status should briefly appear
    await expect(page.locator('[data-testid="save-status"]')).toHaveClass(/opacity-100/);

    // Reload and verify
    await page.reload();
    await expect(page.locator('[data-testid="backend-url"]')).toHaveValue(
      "http://my-server:9090"
    );

    // Restore default for other tests
    await backendInput.fill("http://localhost:8080");
    await backendInput.dispatchEvent("change");

    await page.close();
  });

  test("login form renders with email and password fields", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Login form is visible (logged out state)
    const loginSection = page.locator('[data-testid="auth-logged-out"]');
    await expect(loginSection).toBeVisible();

    // Email and password fields exist
    await expect(page.locator('[data-testid="login-email"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-password"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-btn"]')).toBeVisible();
    await expect(page.locator('[data-testid="login-btn"]')).toHaveText("Login");

    // Logged-in section is hidden
    await expect(page.locator('[data-testid="auth-logged-in"]')).not.toBeVisible();

    await page.close();
  });

  test("trade management section renders with default preset", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Trade management section is visible
    const mgmt = page.locator('[data-testid="trade-management"]');
    await expect(mgmt).toBeVisible();

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

  test("trade management settings persist to storage", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Change risk percent
    const riskInput = page.locator('[data-testid="risk-percent"]');
    await riskInput.fill("2.5");
    await riskInput.dispatchEvent("change");

    // Enable trailing stop
    await page.locator('[data-testid="trailing-toggle"]').click();
    await expect(page.locator('[data-testid="trailing-toggle"]')).toHaveText("ON");

    // Reload and verify persistence
    await page.reload();
    await expect(page.locator('[data-testid="risk-percent"]')).toHaveValue("2.5");
    await expect(page.locator('[data-testid="trailing-toggle"]')).toHaveText("ON");

    await page.close();
  });
});
