# Specification: Rewrite Landing Page as Astro Static Site

**Spec ID:** DESK-02-landing-strip
**Date:** 2026-03-26
**Status:** Draft (Revised — replaces previous React-strip spec)
**Class:** Refactor / Architecture
**Priority:** P1 — After DESK-01 moves all auth and account management to the Desk, testudo-web is a pure marketing site built in React that uses zero React features. Migrating to Astro eliminates ~300KB of unnecessary JavaScript, enables content collections for documentation, and aligns the tech with the purpose.
**Depends on:** DESK-01-unified-dashboard
**Series:** DESK-01 through DESK-02 (unified dashboard migration)

---

## Problem Statement

Once DESK-01 is complete, testudo-web serves a single purpose: marketing landing page with links to the Desk, docs, and extension download. The current implementation uses React 18, Vite, wagmi, RainbowKit, and viem — none of which are needed. The only interactive elements are a theme toggle (useState + localStorage) and a mouse-following spotlight effect (useState + mousemove listener).

The previous version of this spec (DESK-02) proposed stripping Web3 dependencies from the React app. But since React itself provides zero value here — no state management, no dynamic routing, no component lifecycle — stripping Web3 from React just leaves an empty React shell. The right fix is replacing React entirely with Astro, a static site generator purpose-built for content-first marketing sites.

Astro delivers: zero JavaScript by default (theme toggle and spotlight become component islands), first-class content collections for docs and blog posts, superior SEO and first-paint performance, and Solid.js island support for interactive elements — maintaining consistency with the Desk's framework.

---

## User Stories

- **As a visitor**, I want the landing page to load instantly with zero JavaScript overhead, so that I can evaluate Testudo without waiting for a React bundle.
- **As a developer**, I want docs to live as markdown files in the same repo, so that documentation deploys with the site automatically.
- **As a user**, I want the theme toggle and spotlight effect to work exactly as before, so that the visual experience is preserved.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Initialize Astro project in `testudo-web/` (replace existing React app). Configure with `output: 'static'`, Solid.js integration (`@astrojs/solid-js`), and Tailwind CSS. | High | Build |
| FR-2 | Port `LandingPage.tsx` sections to Astro pages/components: `Hero`, `Features`, `Pricing`, `Footer`. All render as static HTML — no client-side JavaScript. | High | Pages |
| FR-3 | Port `Header.tsx` to Astro component. Navigation links: Home, About, Docs, "LAUNCH DESK" (external link to Desk), "INSTALL EXTENSION" (external link to Chrome Web Store). | High | Layout |
| FR-4 | Port theme toggle to a Solid.js island component (`client:load`). Preserve: AMOLED dark / light toggle, `localStorage` persistence, `data-theme` attribute on `<html>`. | High | Islands |
| FR-5 | Port `SpotlightBackground.tsx` to a vanilla `<script>` tag or Solid.js island. Preserve: mouse-following radial gradient, theme-aware opacity adjustment via MutationObserver on `data-theme`. | High | Islands |
| FR-6 | Remove all React dependencies: `react`, `react-dom`, `react-router-dom`, `wagmi`, `@rainbow-me/rainbowkit`, `viem`, `@tanstack/react-query`, `@metamask/sdk`. | High | Dependencies |
| FR-7 | Configure Astro content collections for docs (`src/content/docs/`) with markdown + frontmatter schema. At minimum, create placeholder structure: Getting Started, Extension Setup, API Keys, Troubleshooting. | Medium | Docs |
| FR-8 | Create `/docs` route rendering content collection entries with sidebar navigation, search-friendly markup, and consistent styling with the landing page theme. | Medium | Docs |
| FR-9 | Add static onboarding stepper to landing page (per ONBOARD-01 FR-8). Static HTML showing 4 steps with step 1 highlighted. No interactivity — purely visual marketing element. | Medium | Pages |
| FR-10 | Split CTA on hero section: primary "INSTALL EXTENSION" button (Chrome Web Store link) + secondary "LAUNCH DESK" button (link to Desk URL). | High | Pages |
| FR-11 | Preserve existing Tailwind CSS styling: brutalist dark theme, flicker animations, ticker-pulse keyframes, glassmorphism panels. Port all custom CSS. | High | Styling |
| FR-12 | Configure deployment: static build output (`dist/`), compatible with existing hosting (same build command pattern). | Medium | Build |

---

## Technical Implementation

### Astro Project Structure

```
testudo-web/
├── astro.config.mjs          # Astro config with solid-js + tailwind integrations
├── src/
│   ├── layouts/
│   │   └── Base.astro         # HTML shell, theme script, spotlight
│   ├── pages/
│   │   ├── index.astro        # Landing page (Hero + Features + Pricing + Footer)
│   │   ├── about.astro        # About page
│   │   └── docs/
│   │       └── [...slug].astro # Dynamic docs route from content collection
│   ├── components/
│   │   ├── Header.astro       # Static nav bar
│   │   ├── Hero.astro         # Hero section with split CTA
│   │   ├── Features.astro     # Feature grid
│   │   ├── Pricing.astro      # Pricing tiers
│   │   ├── Footer.astro       # Footer
│   │   ├── Stepper.astro      # Static onboarding preview (4 steps)
│   │   ├── ThemeToggle.tsx     # Solid.js island (client:load)
│   │   └── Spotlight.astro    # Inline <script> for mouse tracking
│   ├── content/
│   │   ├── config.ts          # Content collection schema
│   │   └── docs/
│   │       ├── getting-started.md
│   │       ├── extension-setup.md
│   │       ├── api-keys.md
│   │       └── troubleshooting.md
│   └── styles/
│       └── global.css         # Tailwind directives + custom keyframes
├── public/
│   └── images/                # Static assets (wall background, etc.)
├── package.json
├── tailwind.config.cjs
└── tsconfig.json
```

### Theme Toggle Island

```tsx
// src/components/ThemeToggle.tsx — Solid.js island
import { createSignal, onMount } from "solid-js";

export default function ThemeToggle() {
  const [theme, setTheme] = createSignal<"dark" | "light">("dark");

  onMount(() => {
    const saved = localStorage.getItem("theme") || "dark";
    setTheme(saved as "dark" | "light");
    document.documentElement.setAttribute("data-theme", saved);
  });

  const toggle = () => {
    const next = theme() === "dark" ? "light" : "dark";
    setTheme(next);
    localStorage.setItem("theme", next);
    document.documentElement.setAttribute("data-theme", next);
  };

  return (
    <button onClick={toggle} class="theme-toggle" aria-label="Toggle theme">
      {theme() === "dark" ? /* sun icon */ : /* moon icon */}
    </button>
  );
}
```

Usage in Header: `<ThemeToggle client:load />`

### Spotlight Effect

Port as inline `<script>` in `Base.astro` layout — no framework needed for a mousemove listener:

```html
<script>
  const bg = document.querySelector('.spotlight-bg');
  if (bg) {
    document.addEventListener('mousemove', (e) => {
      bg.style.background = `radial-gradient(circle at ${e.clientX}px ${e.clientY}px, ...)`;
    });
  }
</script>
```

### Content Collections Config

```typescript
// src/content/config.ts
import { defineCollection, z } from "astro:content";

const docs = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.string().optional(),
  }),
});

export const collections = { docs };
```

### Migration Checklist

1. `rm -rf src/ node_modules/` (current React app)
2. `bun create astro@latest . --template minimal`
3. `bun add @astrojs/solid-js @astrojs/tailwind solid-js`
4. Port Tailwind config and custom CSS
5. Port each section component (JSX → Astro/HTML)
6. Port theme toggle as Solid.js island
7. Port spotlight as inline script
8. Set up content collections for docs
9. Verify build and visual parity

### Files

**Deleted (entire React app):**
- `src/main.tsx`, `src/App.tsx`, `src/vite-env.d.ts`
- `src/pages/LandingPage.tsx`, `src/pages/AccountPage.tsx`, `src/pages/AboutPage.tsx`
- `src/context/AuthContext.tsx`, `src/context/ThemeContext.tsx`
- `src/components/ui/Header.tsx`, `src/components/ui/SpotlightBackground.tsx`
- `src/components/sections/Hero.tsx`, `Features.tsx`, `Pricing.tsx`, `Footer.tsx`
- `src/components/WalletConnect.tsx`, `ExchangeCard.tsx`, `AddExchangeCard.tsx`, `ExtensionPairingBanner.tsx`
- `vite.config.ts`, `index.html`

**New (Astro app):**
- `astro.config.mjs`, `src/layouts/Base.astro`
- `src/pages/index.astro`, `src/pages/about.astro`, `src/pages/docs/[...slug].astro`
- `src/components/*.astro` (Header, Hero, Features, Pricing, Footer, Stepper, Spotlight)
- `src/components/ThemeToggle.tsx` (Solid.js island)
- `src/content/config.ts`, `src/content/docs/*.md`

### Dependencies Added

- `astro` — Static site generator
- `@astrojs/solid-js` — Solid.js island integration
- `@astrojs/tailwind` — Tailwind CSS integration
- `solid-js` — Already a project dependency (shared with testudo-journal)

### Dependencies Removed

- `react`, `react-dom`, `@types/react`, `@types/react-dom`
- `react-router-dom`
- `wagmi`, `@rainbow-me/rainbowkit`, `viem`
- `@tanstack/react-query`
- `@metamask/sdk`, `@reown/appkit-*`
- `vite`, `@vitejs/plugin-react`

---

## Acceptance Criteria

- [ ] Landing page renders identically to current design (visual parity)
- [ ] Theme toggle works: persists to localStorage, switches AMOLED dark / light
- [ ] Spotlight mouse-follow effect works with theme-aware opacity
- [ ] Zero JavaScript shipped on pages without islands (verify with browser DevTools)
- [ ] `/docs` route renders markdown content collection with sidebar navigation
- [ ] Split CTA: "INSTALL EXTENSION" + "LAUNCH DESK" buttons on hero
- [ ] Static onboarding stepper renders on landing page
- [ ] All React, wagmi, RainbowKit, viem dependencies removed from package.json
- [ ] `bun run build` (Astro) succeeds with no errors
- [ ] Flicker animations, ticker-pulse, and custom CSS keyframes preserved
- [ ] Page loads in < 1s on throttled connection (no React bundle)

---

## Risks

1. **Visual parity** — Porting Tailwind classes and custom CSS from React components to Astro templates may introduce subtle differences. Mitigation: Side-by-side visual comparison before merging. Screenshot diffing.
2. **Spotlight MutationObserver** — The current spotlight observes `data-theme` attribute changes via MutationObserver. This must work with Astro's hydration timing. Mitigation: Inline script runs synchronously, MutationObserver attaches on DOMContentLoaded.
3. **Build pipeline change** — Switching from Vite to Astro changes the build command and output structure. Mitigation: Update any CI/CD references. Astro's output dir (`dist/`) matches Vite's default.

---

## Completion Signal

This spec is complete when:
1. testudo-web is an Astro static site with zero React dependencies
2. Visual parity confirmed with current landing page
3. Theme toggle and spotlight effects work identically
4. Docs content collection renders at `/docs`
5. All acceptance criteria met
6. `bun run build` passes
7. Code committed to master
