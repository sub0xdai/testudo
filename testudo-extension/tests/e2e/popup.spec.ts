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
    const backendInput = page.locator("#backend-url");
    await expect(backendInput).toHaveValue("http://localhost:8080");

    // Default WS URL
    const wsInput = page.locator("#ws-url");
    await expect(wsInput).toHaveValue("ws://localhost:4000");

    // Paper mode is active by default
    const paperBtn = page.locator('.toggle-btn[data-mode="paper"]');
    await expect(paperBtn).toHaveClass(/active/);

    // Live mode is NOT active
    const liveBtn = page.locator('.toggle-btn[data-mode="live"]');
    await expect(liveBtn).not.toHaveClass(/active/);

    // Status dot shows disconnected (no WS server running)
    const statusDot = page.locator("#status-dot");
    await expect(statusDot).not.toHaveClass(/connected/);
    await expect(page.locator("#status-text")).toHaveText("Disconnected");

    await page.close();
  });

  test("toggling execution mode persists to storage", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    // Click Live toggle
    const liveBtn = page.locator('.toggle-btn[data-mode="live"]');
    await liveBtn.click();
    await expect(liveBtn).toHaveClass(/active/);

    // Reload popup and verify persistence
    await page.reload();
    await expect(
      page.locator('.toggle-btn[data-mode="live"]')
    ).toHaveClass(/active/);
    await expect(
      page.locator('.toggle-btn[data-mode="paper"]')
    ).not.toHaveClass(/active/);

    // Switch back to paper
    await page.locator('.toggle-btn[data-mode="paper"]').click();
    await page.reload();
    await expect(
      page.locator('.toggle-btn[data-mode="paper"]')
    ).toHaveClass(/active/);

    await page.close();
  });

  test("settings save persists backend URL", async ({
    context,
    extensionId,
  }) => {
    const page = await context.newPage();
    await page.goto(`chrome-extension://${extensionId}/popup/popup.html`);

    const backendInput = page.locator("#backend-url");
    await backendInput.fill("http://my-server:9090");
    // Trigger change event (settings save on change)
    await backendInput.dispatchEvent("change");

    // Save status should briefly appear
    await expect(page.locator("#save-status")).toHaveClass(/visible/);

    // Reload and verify
    await page.reload();
    await expect(page.locator("#backend-url")).toHaveValue(
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
    const loginSection = page.locator("#auth-logged-out");
    await expect(loginSection).toBeVisible();

    // Email and password fields exist
    await expect(page.locator("#login-email")).toBeVisible();
    await expect(page.locator("#login-password")).toBeVisible();
    await expect(page.locator("#login-btn")).toBeVisible();
    await expect(page.locator("#login-btn")).toHaveText("Login");

    // Logged-in section is hidden
    await expect(page.locator("#auth-logged-in")).toBeHidden();

    await page.close();
  });
});
