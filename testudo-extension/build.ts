import * as esbuild from "esbuild";
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";

const args = process.argv.slice(2);
const buildChrome = args.includes("--chrome") || (!args.includes("--firefox") && !args.includes("--chrome"));
const buildFirefox = args.includes("--firefox") || (!args.includes("--firefox") && !args.includes("--chrome"));
const watch = args.includes("--watch");

const ENTRY_POINTS = [
  { in: "src/content.ts", out: "content" },
  { in: "src/background.ts", out: "background" },
  { in: "src/popup/popup.ts", out: "popup/popup" },
];

async function bundle(outdir: string): Promise<void> {
  await esbuild.build({
    entryPoints: ENTRY_POINTS.map((e) => ({ in: e.in, out: e.out })),
    bundle: true,
    outdir,
    format: "esm",
    target: "es2022",
    sourcemap: true,
    minify: !watch,
    logLevel: "info",
  });
}

function copyStaticFiles(outdir: string): void {
  // Copy popup HTML
  cpSync("src/popup/popup.html", join(outdir, "popup/popup.html"));

  // Copy icons (create placeholder SVGs if real icons don't exist)
  mkdirSync(join(outdir, "icons"), { recursive: true });
  for (const size of [16, 48, 128]) {
    const iconPath = join("src", "icons", `icon${size}.png`);
    const destPath = join(outdir, "icons", `icon${size}.png`);
    try {
      cpSync(iconPath, destPath);
    } catch {
      // Create minimal placeholder PNG (1x1 green pixel)
      createPlaceholderIcon(destPath, size);
    }
  }
}

function createPlaceholderIcon(path: string, _size: number): void {
  // Minimal valid PNG: 1x1 green pixel
  const png = Buffer.from([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, // IDAT chunk
    0x54, 0x08, 0xd7, 0x63, 0x90, 0xc8, 0x60, 0x00, // compressed data
    0x00, 0x00, 0x04, 0x00, 0x01, 0xa3, 0xb1, 0x96, //
    0xa2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, // IEND chunk
    0x44, 0xae, 0x42, 0x60, 0x82,
  ]);
  writeFileSync(path, png);
}

function writeManifest(outdir: string, browser: "chrome" | "firefox"): void {
  const manifest = JSON.parse(readFileSync("manifest.json", "utf-8"));

  if (browser === "firefox") {
    // Firefox uses browser_specific_settings instead of some MV3 Chrome features
    manifest.browser_specific_settings = {
      gecko: {
        id: "testudo-sniper@sub0xdai",
        strict_min_version: "109.0",
      },
    };
    // Firefox MV3 uses "scripts" array in background, not service_worker
    manifest.background = {
      scripts: ["background.js"],
      type: "module",
    };
  }

  writeFileSync(join(outdir, "manifest.json"), JSON.stringify(manifest, null, 2));
}

async function build(): Promise<void> {
  if (buildChrome) {
    const outdir = "dist/chrome";
    mkdirSync(outdir, { recursive: true });
    await bundle(outdir);
    copyStaticFiles(outdir);
    writeManifest(outdir, "chrome");
    console.log("Chrome build complete → dist/chrome/");
  }

  if (buildFirefox) {
    const outdir = "dist/firefox";
    mkdirSync(outdir, { recursive: true });
    await bundle(outdir);
    copyStaticFiles(outdir);
    writeManifest(outdir, "firefox");
    console.log("Firefox build complete → dist/firefox/");
  }
}

build().catch((err) => {
  console.error("Build failed:", err);
  process.exit(1);
});
