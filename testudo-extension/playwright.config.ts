import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "tests/e2e",
  timeout: 30_000,
  retries: 0,
  workers: 1, // Extensions share browser state; run serially
  use: {
    headless: false, // Extensions require headed Chromium
  },
});
