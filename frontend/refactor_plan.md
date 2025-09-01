# Testudo Trading Terminal - Leptos Implementation Plan

## Current State
✅ **React cleanup complete** - All React/TypeScript components removed
✅ **Assets preserved** - Roman shield, Trajan column images retained  
✅ **Theme preserved** - Nord Arctic Roman Glass theme in globals.css
✅ **Documentation intact** - clarityUI.md requirements and reference materials
✅ **Clean slate ready** - Directory prepared for ground-up Leptos build

Note: `RomanShieldLanding.tsx` kept as reference for CSS techniques (mouse tracking, carousels) for future marketing pages.

## Architecture Decision
- **Framework**: Leptos with Client-Side Rendering (CSR) for SPA
- **Build Tool**: Trunk (optimized for WASM SPAs)  
- **Styling**: Tailwind CSS with terminal-first approach
- **Performance**: Fine-grained reactivity (no Virtual DOM) for <200ms OODA loop targets
- **UI Philosophy**: Bloomberg Terminal inspired - information density over aesthetics

## Terminal-First Design Principles

### 1. **Information Density**
- Every pixel serves a purpose - no wasted screen real estate
- Tabular data displays with minimal padding/margins  
- Compact typography and tight line spacing
- Multiple data points visible simultaneously

### 2. **Performance Over Aesthetics**
- Solid backgrounds for all data panels (no transparency/glass effects)
- High contrast colors for instant readability
- Minimal animations - only for state changes
- Zero glassmorphism in trading areas (reserve for modals/overlays only)

### 3. **Professional Trading UX**
- Keyboard shortcuts for all critical actions
- Click-and-drag position sizing on charts
- No modal interruptions during active trading
- Instant visual feedback for all interactions

### 4. **Roman Military Branding (Subtle)**
- Color palette: Nord Arctic with Roman accents
- Typography: Cinzel for headers, Inter for data
- Terminology: "Command Center", "Legion", etc. in non-critical areas
- Visual assets: Shield/column imagery in branding areas only

## Implementation Plan

### Phase 1: Initialize Leptos Frontend Structure
1. **Create frontend Cargo.toml**:
   - Configure leptos with CSR features
   - Add dependencies: leptos, wasm-bindgen, web-sys, console_error_panic_hook
   - Set up WASM optimizations

2. **Create index.html**:
   - Set up document structure with Testudo - Imperium title
   - Link to Tailwind CSS via Trunk
   - Include Google Fonts (Cinzel, Inter) for Roman theming
   - Mount point for Leptos application

3. **Create Trunk.toml**:
   - Configure watch paths (./src, ./styles)
   - Set dev server (127.0.0.1:8080)
   - Add pre-build hook for Tailwind compilation
   - Add post-build hook for assets copying

4. **Create package.json**:
   - Add tailwindcss and dependencies
   - Configure Tailwind with Rust file scanning

### Phase 2: Build Terminal-First Authentication & UI
1. **Create Terminal Authentication Bar**:
   - `src/components/auth/auth_bar.rs` - Inline authentication status/controls
   - `src/components/auth/login_panel.rs` - Compact login form (not modal)
   - `src/components/auth/user_menu.rs` - User profile dropdown
   - **Design**: Top bar integration, no modal interruptions during trading

2. **Build Core Terminal UI Components**:
   - `src/components/ui/` - Purpose-built trading components
   - High-density data tables, buttons optimized for rapid clicking
   - Dark theme, high contrast, zero transparency in data areas
   - Keyboard navigation and accessibility

3. **Implement OIDC Authentication Flow**:
   - **Primary Strategy**: Integrate the `leptos_oidc` crate as the core authentication system
   - **Functionality**: Handle OIDC redirect flow, manage JWT tokens in memory, provide reactive auth signals  
   - **Terminal Integration**: Seamless auth state in top bar, no modal popups during trading

### Phase 3: Build Terminal-Specific Styling System
1. **Enhance globals.css**:
   - Keep existing Nord Arctic theme variables
   - Add terminal-specific CSS custom properties
   - Ensure standard Tailwind directives
   - Create trading-optimized component classes

2. **Terminal Color System**:
   - High-contrast variants for data display
   - Semantic colors: profit green, loss red, warning amber
   - Accessible color combinations for colorblind users
   - Dark theme optimization for extended screen time

3. **Component Styling Strategy**:
   - Utility-first with Tailwind for rapid development
   - Custom CSS only for complex trading-specific components
   - Roman Glass effects reserved for non-data areas (auth modals, settings)
   - Terminal areas: solid backgrounds, zero transparency

### Phase 4: Implement Trading Terminal
1. **Create three-panel layout**:
   - Central: Chart container for TradingView integration
   - Right: Order panel with Van Tharp calculations
   - Bottom: Positions and status monitoring

2. **TradingView Charts Integration**:
   - Use wasm-bindgen for JS interop
   - Load TradingView Lightweight Charts library
   - Implement drag-and-drop position sizing tool

3. **Real-time data**:
   - WebSocket connection to backend
   - Live price updates and order book
   - Position monitoring

### Phase 5: API Integration
1. **Connect to Imperium backend**:
   - REST API client for commands
   - WebSocket for real-time data
   - Handle JWT validation (tokens managed by leptos_oidc)

2. **Onboarding wizard**:
   - API key capture (Binance)
   - Required permissions validation
   - IP whitelisting instructions
   - Risk profile selection

### Phase 6: Performance & Security
1. **Performance optimizations**:
   - Ensure <200ms OODA loop execution
   - Optimize WASM bundle size
   - Implement efficient re-rendering

2. **Security measures**:
   - No localStorage/sessionStorage for sensitive data
   - JWT tokens in memory only (via leptos_oidc)
   - Secure WebSocket connections

## Current Directory Structure
```
frontend/
├── assets/                 # ✅ Roman military images (preserved)
├── styles/
│   └── globals.css        # ✅ Nord Arctic Roman Glass theme (preserved)
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