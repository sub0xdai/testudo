# App Performance Optimization Process (via @brotzky)

## 1. Bundle & Cold Load Optimization
* **Library Reduction:** Stripped out heavy third-party libraries in favor of custom, tailored implementations.
* **Asset Delivery:** Implemented lazy splits, `modulepreload`, and inlined CSS to speed up initial rendering.
* **Routing:** Utilized middleware to check cookies for immediate, server-side routing decisions.
* **Result:** Achieved a highly optimized React main bundle size of just 114 KB.

## 2. Aggressive Prefetching
* **Intent-Based:** Preloaded routes and APIs immediately upon login intent, before the session fully resolves.
* **Critical Path:** Preloaded 7 essential APIs and preconnected WebSockets right away.
* **Interaction-Based:** Implemented hover-prefetching for dynamic UI elements (tickers, chat, screener) and pulled heavy external data (charts, financials, Reddit) strictly on component mount.

## 3. Multi-Tiered Caching
* **SWR Strategy:** Configured stale-while-revalidate cache tiers with headers ranging from 30 seconds to 1 day.
* **Hydration:** Synchronized `localStorage` with React Query hydration (e.g., caching prices for 5 minutes).
* **AI Data:** Leveraged Gemini context caching (30m KV) and shared caches for AI-generated summaries and chart data.

## 4. Streaming & Parallelism
* **AI Output:** Ensured real token streaming for AI chats using anti-buffering headers (no artificial chunking).
* **Concurrency:** Executed parallel tool calls and batched AI prompts to reduce latency.
* **Network Requests:** Batched client-side API requests over a single endpoint to minimize overhead.

## 5. Rendering Efficiency
* **Layout Stability:** Used lazy-loaded charts with reserved container heights to completely eliminate layout shift.
* **Component Optimization:** Memoized list items, utilized tab-level lazy loading, and maintained an extremely lightweight DOM structure.
* **Perceived Speed:** Ensured instant paint times on any cache hits.

## 6. Connection & Offline Strategies (WS & SW)
* **WebSocket:** Consolidated to a single, app-wide shared WebSocket with visibility gating, smart reconnects, and prefetching upon connection.
* **Service Worker:** Precached the application shell, implemented a NetworkFirst navigation strategy (with a 3s timeout), cached fonts for 30 days, and deferred registration until the browser was idle.

## 7. Development Methodology ("Vibe Coding")
* **AI-Driven Architecture:** The entire optimization process was executed via AI prompts without manually opening an IDE, touching the file system, or writing raw CSS. 
* **Core Philosophy:** Compounding micro-optimizations driven by a strict adherence to simplicity, lightness, and speed.
