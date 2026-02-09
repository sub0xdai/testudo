import { test as base, chromium, type BrowserContext } from "@playwright/test";
import path from "path";
import fs from "fs";

// Path to the built extension
const EXTENSION_DIR = path.resolve(__dirname, "../../../dist/chrome");

/**
 * Patch the built manifest to also inject content scripts on localhost,
 * so the mock TradingView page gets the content script.
 */
const TEST_PATTERNS = ["*://localhost/*", "*://127.0.0.1/*"];

function patchManifestForTesting(): void {
  const manifestPath = path.join(EXTENSION_DIR, "manifest.json");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));

  const contentScripts = manifest.content_scripts || [];
  for (const cs of contentScripts) {
    if (cs.matches) {
      for (const p of TEST_PATTERNS) {
        if (!cs.matches.includes(p)) cs.matches.push(p);
      }
    }
  }

  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
}

/**
 * Restore original manifest after tests.
 */
function restoreManifest(): void {
  const manifestPath = path.join(EXTENSION_DIR, "manifest.json");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));

  const contentScripts = manifest.content_scripts || [];
  for (const cs of contentScripts) {
    if (cs.matches) {
      cs.matches = cs.matches.filter((m: string) => !TEST_PATTERNS.includes(m));
    }
  }

  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
}

export type ExtensionFixtures = {
  context: BrowserContext;
  extensionId: string;
};

export const test = base.extend<ExtensionFixtures>({
  // eslint-disable-next-line no-empty-pattern
  context: async ({}, use) => {
    // Ensure extension is built
    if (!fs.existsSync(path.join(EXTENSION_DIR, "manifest.json"))) {
      throw new Error(
        "Extension not built. Run `bun run build:chrome` before E2E tests."
      );
    }

    patchManifestForTesting();

    const context = await chromium.launchPersistentContext("", {
      headless: false,
      args: [
        `--disable-extensions-except=${EXTENSION_DIR}`,
        `--load-extension=${EXTENSION_DIR}`,
        "--no-first-run",
        "--no-default-browser-check",
      ],
    });

    // Wait for the service worker to initialize
    let sw = context.serviceWorkers()[0];
    if (!sw) {
      sw = await context.waitForEvent("serviceworker");
    }

    await use(context);

    restoreManifest();
    await context.close();
  },

  extensionId: async ({ context }, use) => {
    let sw = context.serviceWorkers()[0];
    if (!sw) {
      sw = await context.waitForEvent("serviceworker");
    }
    // Service worker URL: chrome-extension://<id>/background.js
    const url = sw.url();
    const id = url.split("/")[2];
    await use(id);
  },
});

export const expect = test.expect;

/**
 * Path to the mock TradingView HTML fixture.
 */
export const MOCK_TV_PATH = path.resolve(__dirname, "mock-tradingview.html");
