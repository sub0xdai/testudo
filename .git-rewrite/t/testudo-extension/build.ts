import * as esbuild from "esbuild";
import { solidPlugin } from "esbuild-plugin-solid";
import { cpSync, mkdirSync, readFileSync, writeFileSync, existsSync } from "fs";
import { join } from "path";
import { execSync } from "child_process";

const args = process.argv.slice(2);
const buildChrome = args.includes("--chrome") || (!args.includes("--firefox") && !args.includes("--chrome"));
const buildFirefox = args.includes("--firefox") || (!args.includes("--firefox") && !args.includes("--chrome"));
const watch = args.includes("--watch");
const isProduction = !watch;

// Build-time URL injection — defaults to empty string (falls back to localhost in utils.ts)
const envDefine = {
  "process.env.BACKEND_URL": JSON.stringify(process.env.BACKEND_URL || ""),
  "process.env.WS_URL": JSON.stringify(process.env.WS_URL || ""),
  "process.env.WEB_APP_URL": JSON.stringify(process.env.WEB_APP_URL || ""),
  "process.env.DESK_URL": JSON.stringify(process.env.DESK_URL || ""),
};

// Background service worker uses ESM (manifest declares "type": "module")
const ESM_ENTRIES = [
  { in: "src/background.ts", out: "background" },
];

// Content scripts and popup are classic scripts — must use IIFE
// Content script imports modal.tsx (Solid), popup uses Solid components
const IIFE_ENTRIES = [
  { in: "src/content.ts", out: "content" },
  { in: "src/popup/index.tsx", out: "popup/popup" },
];

// Page bridge and widget hook run in MAIN world — plain IIFE, no Solid
const BRIDGE_ENTRIES = [
  { in: "src/page-bridge.ts", out: "page-bridge" },
  { in: "src/widget-hook.ts", out: "widget-hook" },
];

async function bundle(outdir: string): Promise<void> {
  await Promise.all([
    // ESM build: background worker (no framework, no Solid plugin needed)
    esbuild.build({
      entryPoints: ESM_ENTRIES.map((e) => ({ in: e.in, out: e.out })),
      bundle: true,
      outdir,
      format: "esm",
      target: "es2022",
      sourcemap: true,
      minify: !watch,
      drop: isProduction ? ["console"] : [],
      define: envDefine,
      logLevel: "info",
    }),
    // IIFE build: content script + popup (Solid.js JSX compilation)
    esbuild.build({
      entryPoints: IIFE_ENTRIES.map((e) => ({ in: e.in, out: e.out })),
      bundle: true,
      outdir,
      format: "iife",
      target: "es2022",
      sourcemap: true,
      minify: !watch,
      drop: isProduction ? ["console"] : [],
      define: envDefine,
      logLevel: "info",
      plugins: [solidPlugin()],
      jsx: "automatic",
    }),
    // IIFE build: page bridge (MAIN world script, no Solid, no polyfill)
    esbuild.build({
      entryPoints: BRIDGE_ENTRIES.map((e) => ({ in: e.in, out: e.out })),
      bundle: true,
      outdir,
      format: "iife",
      target: "es2022",
      sourcemap: false,
      minify: !watch,
      drop: isProduction ? ["console"] : [],
      define: envDefine,
      logLevel: "info",
    }),
  ]);
}

function buildTailwindCSS(outdir: string): void {
  const inputCss = "src/popup/popup.css";
  const outputCss = join(outdir, "popup/popup.css");
  mkdirSync(join(outdir, "popup"), { recursive: true });

  try {
    execSync(
      `npx @tailwindcss/cli -i ${inputCss} -o ${outputCss} --minify`,
      { stdio: "pipe" },
    );
  } catch (err) {
    const error = err as { stderr?: Buffer };
    console.error("Tailwind CSS build failed:", error.stderr?.toString());
    throw err;
  }
}

function copyStaticFiles(outdir: string): void {
  // Copy popup HTML + theme init script
  cpSync("src/popup/popup.html", join(outdir, "popup/popup.html"));
  cpSync("src/popup/theme-init.js", join(outdir, "popup/theme-init.js"));

  // Copy bundled fonts
  const fontsDir = join(outdir, "popup/fonts");
  mkdirSync(fontsDir, { recursive: true });
  for (const font of ["space-grotesk-variable.woff2", "space-mono-regular.woff2", "space-mono-bold.woff2"]) {
    const src = join("src", "fonts", font);
    if (existsSync(src)) {
      cpSync(src, join(fontsDir, font));
    }
  }

  // Copy images
  const imagesDir = join(outdir, "popup/images");
  mkdirSync(imagesDir, { recursive: true });
  const srcImagesDir = join("src", "popup", "images");
  if (existsSync(srcImagesDir)) {
    cpSync(srcImagesDir, imagesDir, { recursive: true });
  }

  // Copy icons (create placeholder SVGs if real icons don't exist)
  mkdirSync(join(outdir, "icons"), { recursive: true });
  for (const size of [16, 48, 128]) {
    const iconPath = join("src", "icons", `icon${size}.png`);
    const destPath = join(outdir, "icons", `icon${size}.png`);
    try {
      cpSync(iconPath, destPath);
    } catch {
      createPlaceholderIcon(destPath, size);
    }
  }
}

function createPlaceholderIcon(path: string, _size: number): void {
  const png = Buffer.from([
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
    0x54, 0x08, 0xd7, 0x63, 0x90, 0xc8, 0x60, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x01, 0xa3, 0xb1, 0x96,
    0xa2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
  ]);
  writeFileSync(path, png);
}

function writeManifest(outdir: string, browser: "chrome" | "firefox"): void {
  const manifest = JSON.parse(readFileSync("manifest.json", "utf-8"));

  if (browser === "firefox") {
    manifest.browser_specific_settings = {
      gecko: {
        id: "testudo-sniper@sub0xdai",
        strict_min_version: "112.0",
        data_collection_permissions: {
          required: ["none"],
        },
      },
    };
    manifest.background = {
      scripts: ["background.js"],
      type: "module",
    };
    // EXT-46: Widget hook injected via <script> tag from content.ts — works on all browsers.
  }

  writeFileSync(join(outdir, "manifest.json"), JSON.stringify(manifest, null, 2));
}

async function build(): Promise<void> {
  if (buildChrome) {
    const outdir = "dist/chrome";
    mkdirSync(outdir, { recursive: true });
    await bundle(outdir);
    buildTailwindCSS(outdir);
    copyStaticFiles(outdir);
    writeManifest(outdir, "chrome");
    console.log("Chrome build complete → dist/chrome/");
  }

  if (buildFirefox) {
    const outdir = "dist/firefox";
    mkdirSync(outdir, { recursive: true });
    await bundle(outdir);
    buildTailwindCSS(outdir);
    copyStaticFiles(outdir);
    writeManifest(outdir, "firefox");
    console.log("Firefox build complete → dist/firefox/");
  }
}

build().catch((err) => {
  console.error("Build failed:", err);
  process.exit(1);
});
