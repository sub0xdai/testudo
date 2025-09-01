# Frontend Refactoring Plan: React to Leptos Migration (Final)

## Architecture Decision
- **Framework**: Leptos with Client-Side Rendering (CSR) for SPA
- **Build Tool**: Trunk (optimized for WASM SPAs)
- **Styling**: Tailwind CSS with existing Roman Glass theme
- **Performance**: Fine-grained reactivity (no Virtual DOM) for <200ms latency targets

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

### Phase 2: Port Authentication Components (Refined)
1. **Convert React Components to Leptos**:
   - `RomanShieldLanding.tsx` → `src/components/auth/roman_shield_landing.rs`
   - `AuthModal.tsx` → `src/components/auth/auth_modal.rs`
   - `LoginForm.tsx` → `src/components/auth/login_form.rs`
   - `SignUpForm.tsx` → `src/components/auth/signup_form.rs`

2. **Port UI Library**:
   - Convert the shadcn/ui components (Button, Card, etc.) to Leptos component equivalents in `src/components/ui/`
   - Preserve all Tailwind CSS classes and styling variants

3. **Implement OIDC Authentication Flow**:
   - **Primary Strategy**: Integrate the `leptos_oidc` crate as the core of the authentication system
   - **Functionality**: Use `leptos_oidc` to handle the entire OIDC redirect flow, manage token state (storing them securely in memory), and provide reactive signals for authentication status (e.g., `is_authenticated`)
   - **UI Integration**: The Login and Logout buttons in the UI will trigger the respective functions provided by the `leptos_oidc` library

### Phase 3: Extract and Refactor Styles
1. **Move inline styles**:
   - Extract large `<style>` block from RomanShieldLanding
   - Create `styles/components/roman_shield.css`
   - Use CSS modules approach

2. **Clean up globals.css**:
   - Keep existing Nord Arctic theme variables
   - Ensure standard Tailwind directives
   - Maintain Roman Glass glassmorphism effects

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

## File Structure
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
│   │   ├── auth/          # Authentication components
│   │   ├── trading/       # Trading terminal components
│   │   └── ui/            # Reusable UI components
│   └── lib.rs
├── styles/
│   ├── globals.css        # Global styles with Tailwind
│   └── components/        # Component-specific styles
└── assets/                # Images and static files
```

## Implementation Steps
1. Set up Leptos CSR skeleton with Trunk
2. Configure Tailwind CSS pipeline
3. Port authentication flow using leptos_oidc (priority)
4. Implement basic terminal layout
5. Integrate TradingView charts
6. Connect backend APIs
7. Add real-time WebSocket data
8. Performance testing & optimization
9. Security hardening

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