# Testudo UI - Clarity & Requirements

This document outlines the vision, requirements, and key architectural decisions for the Testudo frontend, **Imperium**. It is a living document designed to guide the refactoring and development process.

## Part A: Synthesized Requirements (The Current State)

This section summarizes the known requirements derived from the project's existing backend architecture, documentation, and frontend components.

### 1. UI Philosophy & Core Principles

-   **Core Mission:** The UI must serve as a "Command Center" that enforces disciplined trading by automating position sizing and transparently communicating risk.
-   **Guiding Principles:** The interface must embody the project's Roman military principles of *Disciplina* (Discipline), *Formatio* (System), *Prudentia* (Prudence), and *Imperium* (Command).
-   **Aesthetic:** The visual style will be a "minimalist" and "slick terminal" experience, building upon the "Roman Glass" and Monochromatic, with some neon or deep jewel toned accented color palette established in the `RomanShieldLanding` component.

### 2. Known Functional Requirements

-   **Authentication:** A complete authentication flow (Login, Signup, Google/GitHub) is required, using the `RomanShieldLanding` as the entry point.
-   **Onboarding:** A mandatory, one-time wizard after first login must capture and validate the user's CEX API keys and allow them to select a risk profile (e.g., Conservative, Standard).
-   **Charting:** The core of the terminal will be the **TradingView Lightweight Charts** library.
-   **Position Sizing Interaction:** A **drag-and-drop tool** on the chart is the primary method for setting entry, stop-loss, and take-profit levels.
-   **Real-Time Sizing:** As the user manipulates the chart tool, the UI must continuously fetch and display the **automatically calculated position size** from the `disciplina` backend engine.
-   **Risk Communication:** The UI must clearly display the final risk assessment from the `prudentia` engine before execution. It must also visualize global system states, such as an active **circuit breaker**.

### 3. Known Non-Functional Requirements

-   **Performance:** The application must be desktop-first and meet strict latency targets: **< 200ms** for the entire trade execution loop (click-to-confirmation) and **< 100ms** for UI updates from real-time data.
-   **Security:** The frontend must **never** store sensitive data (API keys, JWTs) in `localStorage` or `sessionStorage`. All authenticated API calls must use a JWT held in memory.
-   **Technology:** The UI will be a **React** application. State management will be handled by **TanStack Query** (server state) and **Zustand** (global client state). API communication will use **REST** for commands and **WebSockets** for real-time data streams.

---

## Part B: Requirements Discovery Questionnaire (Action Required)

The following questions are designed to clarify the remaining ambiguities and finalize the technical direction for the UI refactor.

### Phase 1: Architecture & Framework

**1.1. Framework Selection:**
**Question:** The existing code uses Vite-specific syntax, but a full framework like Next.js is also an option. Given the need for a high-performance, "application-like" terminal, which path should we commit to?

Decision: We will commit to using the Leptos framework for the frontend.

Justification: While a Vite+React SPA is a viable option, Leptos aligns perfectly with Testudo's core principles of Disciplina, Performance, and Precision.

    `Performance: Leptos leverages WebAssembly (WASM) to achieve the highest possible runtime performance, which is critical for a real-time trading interface with demanding latency targets.

    End-to-End Type Safety: By using Rust across the entire stack (Axum backend, Leptos frontend), we can share code and data models. This guarantees mathematical precision in financial calculations and eliminates an entire class of bugs that can arise from language mismatches.

    Manageable Trade-offs: The primary drawback of a WASM-based framework is the initial load time. However, for our target audience of dedicated traders using a desktop-first application, this one-time cost is an acceptable trade-off for the significant gains in performance and reliability.`


**1.2. State Management Confirmation:**
**Question:** The proposed state management stack is `TanStack Query` for server data and `Zustand` for global UI state. Do you confirm this choice, or do you have other preferences (e.g., Redux, Jotai)?

Decision: The proposed stack of TanStack Query and Zustand is not compatible with our decision to use the Leptos framework. We will instead use Leptos's built-in, native state management primitives, which are more performant and align with our goal of a unified Rust technology stack.

Justification:

Leptos provides its own powerful and fine-grained reactive system for managing both server and UI state. Adopting external JavaScript libraries would add unnecessary complexity and performance overhead via WASM-JS interop, contradicting our core principles.

    For Server Data (Replaces TanStack Query): We will use Leptos's built-in create_resource. This primitive is designed specifically for asynchronous data fetching. It automatically handles loading states, errors, and re-fetching when dependent signals change, providing all the necessary functionality for managing our real-time API data.

    For Global UI State (Replaces Zustand): We will use Leptos's core reactive primitive, create_signal. For global access, these signals will be provided through Leptos's context system, making state available throughout the component tree as needed. This approach is extremely lightweight and minimalist, perfectly matching the project's philosophy.

**1.3. Path Alias Configuration:**
**Question:** To improve code clarity and maintainability, I propose setting up path aliases (e.g., `import { Button } from '@/components/ui/button';`). Do you approve this convention?

Decision: Yes

### Phase 2: UX & Workflow Details

**2.1. Onboarding Flow:**
**Question:** For the initial API key setup, what are the exact fields we need to capture from the user for Binance integration? Is it just an API Key and a Secret Key, or are there other parameters like permissions or IP whitelisting instructions we need to account for?

Answer: We need to capture two key fields from the user and provide clear instructions for two mandatory security settings: permissions and IP whitelisting.

### Required Fields

The user interface must have input fields to capture the following:

    API Key: The public identifier for the user's Binance API.

    Secret Key: The private credential for the API. The UI must prominently feature a warning that Binance only displays this key once upon creation.

### Required Permissions

Our application cannot function without specific permissions. The onboarding UI must guide the user to enable the following settings on the Binance website when creating their key:

    ✅ Enable Reading

    ✅ Enable Spot & Margin Trading

For the user's protection, the UI must also give a strong recommendation NOT to enable withdrawals:

    ❌ Enable Withdrawals

Our backend will validate the submitted key to ensure these permissions are active and will reject the key with a helpful error message if they are not.

### IP Whitelisting Instructions

To adhere to security best practices, we will operate from a static IP address. The API setup screen must display this IP to the user with clear instructions.

Example Instructional Text:
"For maximum security, please restrict this API key to our server's IP address: [Enter Testudo's Static IP Address Here]. This ensures all trading activity can only originate from the Testudo platform."

Strategic Note: By standardizing the capture of these credentials, we can feed them directly into our ccxt-rs integration layer. This approach ensures that as we enable support for other exchanges (Kraken, Coinbase, etc.), the user experience for adding new keys will be consistent and familiar.


**2.2. Command Center Layout:**
**Question:** I envision a three-panel layout: a large central chart, a right-hand panel for order/sizing details, and a bottom panel for positions/status. Does this information hierarchy align with your vision for a minimalist trading terminal?

Decision: Yes, the three-panel layout you've envisioned is an excellent foundation for Testudo's Command Center. It establishes a clear and effective information hierarchy that strongly aligns with the minimalist philosophy and the principles of a good trading UX outlined in the article you found.

## Validating the Three-Panel Layout

This layout works because it logically separates the trader's workflow into three distinct zones, reducing cognitive load and aligning with the OODA loop (Observe, Orient, Decide, Act).

    1. Central Panel (The Chart): This is the Observe & Orient zone. As the largest and most central element, it correctly prioritizes market analysis. Our plan to use TradingView's Lightweight Charts will ensure we can retain essential functionality like drawing tools, technical indicators, and multi-timeframe analysis.

    2. Right-Hand Panel (Order & Sizing): This is the Decide & Act zone. It's the perfect place for all trade execution details. It will display the results of our automated Van Tharp position sizing, showing the user their calculated position size, risk-in-dollars, and R-multiple, without them needing to perform any calculations.

    3. Bottom Panel (Positions & Status): This is the Feedback & Monitoring zone. It provides an at-a-glance view of all open positions, live P/L, and account status. This is critical for managing ongoing trades without cluttering the main analysis area.

## Integrating the Order Book

Your suggestion to include an order book (Level 2 data) is a great one for traders who need to see market depth. To maintain the minimalist interface, we can integrate it without adding permanent clutter.

Recommendation: The order book should be an optional, collapsible view within the right-hand (Order) panel. It could be implemented as a tab alongside the primary order entry form. This provides immediate access for those who need it while keeping it hidden for users who prefer a cleaner interface, perfectly balancing utility and minimalism.

**2.3. Displaying Warnings & System Status:**
**Question:** When the `prudentia` backend returns a non-critical **warning** on a trade (e.g., low R:R ratio), how should the UI present this? A dismissible toast notification, a permanent yellow banner in the order panel, or another method?

Decision: Yes, the UI should use a **contextual banner within the order panel**, not a toast notification. This approach is more aligned with Testudo's core principles.

---
## ## The `Prudentia` Protocol: Contextual Warnings

When the backend identifies a non-critical issue with a trade setup, such as a low reward-to-risk ratio, the warning should be displayed directly within the **right-hand order panel**. This method is superior to a toast notification for several key reasons:

* **Supports `Prudentia` (Risk-Awareness):** A persistent banner forces the trader to consciously acknowledge the warning. Unlike a toast notification, it can't be casually dismissed without thought, promoting more disciplined decision-making.
* **Maintains Minimalism:** The warning is placed contextually alongside the trade parameters it relates to. It doesn't pop up over the main chart area, avoiding visual clutter and keeping the trader's focus on their analysis.
* **Reduces Cognitive Load:** By integrating the warning into the natural workflow of setting up a trade, it becomes part of the process rather than an unexpected interruption.

### ### Recommended Implementation

To make the warning even more effective, we should combine the banner with a change in the state of the primary action button.

1.  **Contextual Banner:** A clear, non-dismissible banner appears at the top of the order panel with a concise explanation (e.g., "⚠️ **Warning:** R:R ratio is below 1.5").
2.  **Action Button State Change:** The "Execute Trade" button changes its appearance. The color could shift from standard green/blue to a cautionary yellow or orange, and the text could change to "**Execute Anyway**."



This two-factor approach ensures the user not only sees the warning but must also make a deliberate, conscious choice to proceed, perfectly embodying the disciplined nature of the Testudo platform.

**2.4. Theming and Styling:**
**Question:** The "Roman Glass" theme is a strong starting point. Should this glassmorphism effect be applied to all panels, including the main chart background, or should it be reserved for modals and elevated surfaces to maintain focus on the chart data?

Decision: While the "Roman Glass" theme is stylistically compelling, clarity and focus must be the top priorities for a trading terminal. Applying a glassmorphism effect universally would compromise the readability of the chart, which is unacceptable.

The best approach is to use the effect sparingly and strategically, reserving it for modals and elevated surfaces only.

## The "Roman Stone & Glass" Approach

To balance our aesthetic goals with functional requirements, we will adopt a "Roman Stone & Glass" theme. This means our foundational UI will be solid and opaque ("Stone") for maximum clarity, while the glass effect will be used only for transient or secondary elements ("Glass").

### Solid "Stone" Surfaces (High Priority)

These surfaces must have solid, high-contrast, opaque backgrounds to ensure data is instantly legible and free from distraction.

    The Main Chart Background: This is the most critical element. It must be a solid color to ensure price action, indicators, and drawings are perfectly clear.

    The Order Panel: Readability of numbers and text in this panel is essential for accurate trade execution.

    The Positions Panel: Live P/L and position data must be unambiguous.

### Translucent "Glass" Surfaces (Low Priority)

The glassmorphism effect can be applied to surfaces that temporarily overlay the main interface, creating a sense of depth without interfering with core trading tasks.

    Modals: Such as the settings menu, API key setup, or confirmation dialogs.

    Notifications/Toasts: If used, these are good candidates for the glass effect.

This disciplined approach ensures the UI serves the trader first by prioritizing focus and readability, while still allowing for a modern and thematically consistent aesthetic on less critical components. It perfectly aligns with the principles of Disciplina and Prudentia.

### Phase 3: Refactoring & Implementation

**3.1. CSS Refactoring:**
**Question:** I will refactor the large inline `<style>` block in `RomanShieldLanding.tsx` into a separate, scoped CSS file and clean up `globals.css` to use standard Tailwind directives. Please confirm this course of action.

Decision: Yes, this is the correct course of action. This is a crucial and highly recommended refactoring that aligns with modern web development best practices.

## Refactoring RomanShieldLanding.tsx

Moving the large inline <style> block into a separate, scoped CSS file (e.g., RomanShieldLanding.module.css) is the right move. This improves the project by:

    Increasing Maintainability: It separates styling from logic, making the component's code easier to read and manage.

    Preventing Style Conflicts: Using a scoped CSS file ensures that these styles will not accidentally affect other components.

    Improving Readability: The TSX component will be significantly cleaner, focusing only on structure and logic.

## Cleaning up globals.css

Refactoring globals.css to use the standard Tailwind directives (@tailwind base;, @tailwind components;, @tailwind utilities;) is the correct way to set up the project's styling foundation. This ensures that:

    Tailwind's Engine Works Correctly: It allows Tailwind's Just-In-Time (JIT) compiler to scan your files, purge unused styles, and keep the final CSS bundle as small as possible.

    Styling is Predictable: It establishes the proper cascade of styles, preventing unexpected overrides and making the application's overall styling more consistent.

This cleanup is essential for maintaining a healthy and scalable codebase. Your proposed changes are confirmed as best practices. 👍

## Implementation in Leptos

Here's how we'll apply your plan in our new Leptos stack:

    Component-Specific Styles: Instead of RomanShieldLanding.tsx, we'll have roman_shield_landing.rs. The large <style> block will be moved to a separate CSS file (e.g., roman_shield_landing.css) and linked in the main index.html or processed by the build tool. For general styling, we'll favor using Tailwind's utility classes directly within the Leptos view! macro for speed and colocation.

    Global Styles: This part of the plan is unchanged. We will still have a globals.css file, and it will contain the standard @tailwind base;, @tailwind components;, and @tailwind utilities; directives. This is a core part of the Tailwind CSS workflow, regardless of the frontend framework.

**3.2. Future-Proofing for Trade Journal:**
**Question:** The `clarity.md` document mentions a future FxBlue-style trade journal. Should the initial UI shell (e.g., the main navigation or sidebar) include a disabled or placeholder link for the "Journal" to bake its future location into the core design?

Decision: Yes, absolutely. Adding a disabled placeholder link for the "Journal" is a smart, forward-thinking design choice that I fully endorse.

It's a low-effort, high-impact decision that aligns perfectly with a strategic product rollout.

## Architectural Foresight

By including the placeholder from the start, you are baking the information architecture into the core design. This "reserves the real estate" for one of the product's most critical future features. When the journal is ready to launch, it will slot into a natural and familiar location, rather than feeling like a feature that was awkwardly bolted on later, which avoids disrupting the user's established workflow.

## Setting User Expectations

A disabled link does more than just fill a space; it acts as a a part of your product's roadmap that is visible to the user. It communicates that Testudo is not just a trade execution tool, but a complete platform for disciplined traders. It builds anticipation and signals to early adopters that a key component for performance review and analysis is a planned priority.

## Recommended Implementation

To make it effective, the placeholder should be more than just a dead link.

    Visual State: The link should be styled in a "disabled" or "ghosted" state (e.g., greyed out, with lower opacity) to indicate it's not yet active.

    Tooltip on Hover: When a user hovers over the link, a tooltip should appear with an informative and encouraging message like "Trade Journal (Coming Soon)" or "Performance Analytics (Planned for Q4 2025)."

This simple interaction turns a non-functional element into a positive preview of the platform's future, reinforcing user confidence in the product's vision.


## Confirmation of Hybrid Approach

Your findings confirm that the most effective way to build Testudo is through a hybrid approach. There is currently no single, native Rust library that provides the rich, interactive charting experience required by the PRD. Therefore, our strategy will be to leverage each ecosystem for its strengths.

    Rust (Backend & Logic): For its performance, safety, and correctness in handling data and financial calculations.

    JavaScript (Frontend Charting - Lightweight charts tradingview library): For its mature, feature-rich libraries for interactive web-based visualization.

## Refined Technical Stack

Based on this summary, I can refine our proposed technical stack with specific library choices that are now confirmed to be the best fit.

    Backend Data Integration: We should adopt ccxt-rs. Even though the PRD starts with only Binance, using this library from the beginning is a strategic advantage. It provides a unified API, which directly fulfills our secondary objective to "Support multiple cryptocurrency exchanges" and will make adding Kraken, Coinbase, etc., vastly simpler in the future.

    Backend Data Processing: We will use RustTI for calculating all technical indicators. Its comprehensive and configurable nature is perfect for our needs.

    Frontend Framework: Our choice of Leptos remains solid, providing the Rust+WASM foundation.

    Frontend Charting: Our plan to integrate TradingView's Lightweight Charts via JS interop is confirmed as the most practical path to achieving the required UI/UX.

This refined plan is robust, pragmatic, and leverages the best tools available to meet Testudo's ambitious requirements.

# the question of Authentication
## The Perfect Client-Side Solution

You're right, leptos_oidc is the simplest and most flexible option. It perfectly bridges our Leptos frontend with a dedicated authentication server like Keycloak.

The benefits you listed are exactly why this is the right choice:

    It handles the complexity: The library manages the difficult parts of the OIDC flow, such as redirects, token handling, and state management, so we don't have to build it from scratch.

    It's framework-native: Because it's designed for Leptos, its built-in components and reactive helpers will integrate seamlessly into our UI.

    It's flexible: By using the OIDC standard, it keeps our options open. While we'll start with a self-hosted provider, we could switch to another OIDC-compliant service in the future with minimal code changes.

## Updated Authentication Plan

This solidifies our path forward. Here is the complete, three-part plan for authentication in Testudo:

    Auth Server: We will deploy a self-hosted, OIDC-compliant provider. Keycloak remains the top recommendation for its power and feature set.

    Frontend (Leptos): We will use the leptos_oidc crate to manage all user-facing authentication flows. This includes the login/logout buttons and protecting client-side routes.

    Backend (Axum): Our Axum server will be configured to validate the JWTs issued by Keycloak on every API request, ensuring our backend endpoints are secure.
