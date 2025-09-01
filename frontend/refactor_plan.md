# Testudo Trading Terminal - Leptos Implementation Plan

## Current State
✅ **Fresh Build Initialized** - Project is a ground-up Leptos build, not a refactor.
✅ **Assets preserved** - Roman shield, Trajan column images retained.
✅ **New Theme Adopted** - Monochromatic terminal theme with neon accents is primary (`globals.css`).
✅ **Old Theme Backed Up** - Previous Nord Arctic theme is archived in `globals-nord-backup.css`.
✅ **Documentation Intact** - `clarityUI.md` requirements are up-to-date.

Note: `RomanShieldLanding.tsx` is kept as a reference for potential future marketing pages, but is not part of the active application build.

## Architecture Decision
- **Framework**: Leptos with Client-Side Rendering (CSR) for the Single-Page Application (SPA).
- **Build Tool**: Trunk, optimized for WebAssembly (WASM) SPAs.
- **Styling**: Tailwind CSS, configured for a "terminal-first" design.
- **Performance**: Leptos's fine-grained reactivity (no V-DOM) is critical for meeting the sub-200ms OODA loop performance targets.
- **UI Philosophy**: Inspired by a Bloomberg Terminal, prioritizing information density and performance over purely aesthetic concerns.

## Terminal-First Design Principles

### 1. **Information Density**
- Every pixel must serve a purpose; no wasted screen real estate.
- Data will be displayed in tabular formats with minimal padding and margins.
- Typography will be compact with tight line spacing to maximize data visibility.

### 2. **Performance Over Aesthetics**
- All data panels will use solid, opaque backgrounds. No transparency or glassmorphism.
- The color scheme is high-contrast to ensure instant readability.
- Animations are minimal and used only to indicate a state change.

### 3. **Professional Trading UX**
- Keyboard shortcuts will be available for all critical actions.
- The primary interaction for position sizing will be a click-and-drag tool on the chart.
- The workflow will not be interrupted by modals during active trading.

### 4. **Roman Military Branding (Subtle)**
- **Color Palette**: A professional, monochromatic grayscale aesthetic with subtle, glowing neon accents for profit/loss indicators. Full neon colors are reserved for critical alerts only.
- **Typography**: Cinzel for headers, Inter for data.
- **Terminology**: "Command Center," "Legion," etc., will be used in non-critical areas.
- **Visual Assets**: The shield and column imagery are reserved for branding areas.

## Implementation Plan

### Phase 1: Initialize Leptos Frontend Structure
1.  **Create `frontend/Cargo.toml`**:
    -   Configure `leptos` with CSR features.
    -   Add dependencies: `leptos`, `wasm-bindgen`, `web-sys`, `console_error_panic_hook`.
    -   Set up WASM release optimizations.
2.  **Create `index.html`**:
    -   Set up the document structure with the title "Testudo - Imperium."
    -   Link to the compiled Tailwind CSS.
    -   Include Google Fonts (Cinzel, Inter).
    -   Define the mount point for the Leptos application.
3.  **Create `Trunk.toml`**:
    -   Configure watch paths (`./src`, `./styles`).
    -   Set the dev server to `127.0.0.1:8080`.
    -   Add a pre-build hook for Tailwind compilation.
    -   Add a post-build hook to copy assets.
4.  **Create `package.json`**:
    -   Add `tailwindcss` and its dependencies.
    -   Configure Tailwind to scan Rust files for class discovery.

### Phase 2: Build Terminal-First Authentication & UI
1.  **Create Terminal Authentication Bar**:
    -   `src/components/auth/auth_bar.rs`: An inline component for auth status and controls.
    -   `src/components/auth/login_panel.rs`: A compact login form, not a modal.
    -   `src/components/auth/user_menu.rs`: A dropdown for the user profile.
2.  **Build Core Terminal UI Components**:
    -   `src/components/ui/`: A library of purpose-built trading components.
    -   Focus on high-density data tables, buttons optimized for rapid interaction, and a dark, high-contrast theme.
3.  **Implement OIDC Authentication Flow**:
    -   Integrate the `leptos_oidc` crate as the primary authentication system.
    -   It will handle the OIDC redirect flow, manage JWTs in memory, and provide reactive auth signals to the UI.

### Phase 3: Build Terminal-Specific Styling System
1.  **Enhance `globals.css`**:
    -   Implement the new monochromatic theme with subtle neon accents.
    -   Define terminal-specific CSS custom properties for profit, loss, and active states.
    -   Ensure standard Tailwind directives are in place.
2.  **Terminal Color System**:
    -   Use high-contrast grayscale for data display.
    -   Use semantic colors (subtle green/red glows) for P/L states.
    -   Reserve full neon colors for critical alerts only.
3.  **Component Styling Strategy**:
    -   Employ a utility-first approach with Tailwind for rapid development.
    -   Use "Roman Glass" effects only for non-data surfaces like modals or settings overlays.
    -   All core terminal areas will have solid, opaque backgrounds.

### Phase 4: Implement Trading Terminal
1.  **Create Three-Panel Layout**:
    -   **Central**: A container for the TradingView chart.
    -   **Right**: The order panel, displaying real-time Van Tharp calculations.
    -   **Bottom**: A panel for monitoring positions and system status.
2.  **TradingView Charts Integration**:
    -   Use `wasm-bindgen` for JavaScript interop to control the library.
    -   Implement the drag-and-drop position sizing tool.
3.  **Real-Time Data**:
    -   Establish a WebSocket connection to the backend for live price updates and order book data.

### Phase 5: API Integration
1.  **Connect to Imperium Backend**:
    -   Build a REST API client for sending commands.
    -   Handle JWT validation for all requests (tokens managed by `leptos_oidc`).
2.  **Onboarding Wizard**:
    -   Create the UI for API key capture (Binance), permissions validation, and IP whitelisting instructions.

### Phase 6: Performance & Security
1.  **Performance Optimizations**:
    -   Benchmark and ensure the sub-200ms OODA loop target is met.
    -   Optimize the final WASM bundle size.
2.  **Security Measures**:
    -   Ensure no sensitive data is ever stored in `localStorage` or `sessionStorage`.
    -   Use secure WebSockets (`wss://`).

## Current Directory Structure
```
frontend/
├── assets/                 # ✅ Roman military images (preserved)
├── styles/
│   ├── globals.css        # ✅ NEW: Monochromatic terminal theme
│   └── globals-nord-backup.css # ✅ OLD: Nord theme backup
├── clarityUI.md           # ✅ Complete requirements documentation
├── refactor_plan.md       # ✅ This implementation plan
├── README.md              # Documentation of previous React state
└── RomanShieldLanding.tsx # Reference only (CSS techniques for future use)
```

## Target File Structure (After Implementation)
```
frontend/
├── Cargo.toml              # Leptos dependencies
├── Trunk.toml              # Build configuration
├── index.html              # Entry point
├── package.json            # Tailwind dependencies
├── tailwind.config.js      # Tailwind configuration
├── src/
│   ├── main.rs            # Application entry
│   ├── app.rs             # Root component
│   ├── components/
│   │   ├── auth/          # Terminal auth bar & user menu
│   │   ├── trading/       # Three-panel terminal layout
│   │   └── ui/            # High-density UI components
│   └── lib.rs
├── styles/
│   ├── globals.css        # Enhanced terminal themes
│   └── components/        # Component-specific styles
├── assets/                # Roman imagery for branding
├── clarityUI.md           # Requirements (preserved)
├── refactor_plan.md       # This plan (preserved)
└── RomanShieldLanding.tsx # CSS reference (preserved)
```

## Implementation Steps
1. Set up Leptos CSR skeleton with Trunk
2. Configure Tailwind CSS pipeline  
3. **Build** authentication bar using leptos_oidc (priority)
4. **Create** three-panel terminal layout from scratch
5. **Integrate** TradingView charts via JS interop
6. **Connect** to imperium backend APIs
7. **Implement** real-time WebSocket data streams
8. **Build** onboarding wizard for API keys
9. Performance testing & optimization
10. Security hardening

## Key Technical Details

### Proposed frontend/index.html
```html
<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Testudo - Imperium</title>

    <!-- Link to the compiled Tailwind CSS -->
    <link data-trunk rel="css" href="styles/globals.css" />

    <!-- Link to the Google Fonts used in the landing page -->
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link
      href="https://fonts.googleapis.com/css2?family=Cinzel:wght@400;700&family=Inter:wght@300;400;500&display=swap"
      rel="stylesheet"
    />
  </head>
  <body>
    <!-- The Leptos application will be mounted here -->
  </body>
</html>
```

### Proposed frontend/Trunk.toml
```toml
[watch]
# Paths to watch for changes
watch = ["./src", "./styles"]

[serve]
# The address to serve the application on
address = "127.0.0.1"
port = 8080
# Open the browser automatically
open = true

[[hooks]]
# This hook ensures Tailwind CSS is re-compiled whenever our source files change
stage = "pre_build"
command = "sh"
command_arguments = ["-c", "npx tailwindcss -i ./styles/globals.css -o ./dist/tailwind.css"]

[[hooks]]
# This hook copies the assets directory to the output directory
stage = "post_build"
command = "sh"
command_arguments = ["-c", "cp -r ./assets ./dist/"]
```

## Notes
- This approach maintains the existing Roman military aesthetic while transitioning to a high-performance Rust/WASM architecture
- The Leptos CSR approach with fine-grained reactivity aligns with Testudo's performance requirements
- OIDC authentication via leptos_oidc provides a secure, standardized authentication flow
- The three-panel layout follows trading terminal best practices for information hierarchy