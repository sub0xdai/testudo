---
# Design System: Testudo Exchange
# A brutalist, high-performance design system inspired by cool marble, 
# architectural stone, and retro-tech instrumentation.

design-tokens:
  colors:
    dark: # AMOLED / Marble Dark
      bg-core: "rgb(9, 10, 13)"
      bg-panel: "rgb(19, 21, 26)"
      bg-elevated: "rgb(26, 28, 35)"
      bg-hover: "rgb(35, 38, 46)"
      border: "rgb(45, 48, 58)"
      border-active: "rgb(237, 237, 237)"
      accent-steel: "rgb(148, 163, 184)"
      accent-primary: "rgb(180, 190, 200)" # Silver Marble
      text-primary: "rgb(235, 237, 242)"
      text-secondary: "rgb(185, 190, 200)"
      text-tertiary: "rgb(115, 120, 130)"
    light: # Marble Light (Blue-shifted, no parchment)
      bg-core: "rgb(241, 243, 247)"
      bg-panel: "rgb(255, 255, 255)"
      bg-elevated: "rgb(234, 236, 242)"
      bg-hover: "rgb(225, 228, 236)"
      border: "rgb(195, 200, 212)"
      border-active: "rgb(15, 20, 35)"
      accent-steel: "rgb(55, 65, 81)"
      accent-primary: "rgb(80, 90, 108)"
      text-primary: "rgb(10, 12, 20)"
      text-secondary: "rgb(55, 62, 78)"
      text-tertiary: "rgb(95, 102, 118)"
    shared:
      signal-green: "rgb(34, 197, 94)"
      signal-red: "rgb(239, 68, 68)"
      signal-amber: "rgb(245, 158, 11)"

  typography:
    font-family:
      display: "Space Grotesk, system-ui, sans-serif"
      mono: "Space Mono, JetBrains Mono, monospace"
    size:
      xs: "0.75rem"
      sm: "0.875rem"
      base: "1rem"
      lg: "1.125rem"
      xl: "1.25rem"
    letter-spacing:
      tight: "-0.025em"
      normal: "0"
      wide: "0.025em"
      widest: "0.1em"
      section: "0.2em"

  spacing:
    container:
      padding-x: "2rem"
      padding-y: "1.5rem"
    radius:
      none: "0"
      sm: "0.125rem"
      default: "0.25rem"
      md: "0.375rem"
      lg: "0.5rem"
      xl: "0.75rem"

  motion:
    flicker: "flicker 4s ease-in-out infinite"
    ticker-pulse: "ticker-pulse 3s ease-in-out infinite"
    transition:
      fast: "150ms cubic-bezier(0.4, 0, 0.2, 1)"
      normal: "300ms cubic-bezier(0.4, 0, 0.2, 1)"

  shadows:
    none: "none"
    brutalist: "4px 4px 0px rgba(0, 0, 0, 0.2)" # Derived from border logic

---

# Design Ethos: Testudo Exchange

## Look & Feel
Testudo is designed as a **Brutalist Modern** exchange. It rejects the soft, rounded gradients of consumer fintech in favor of architectural rigidity, information density, and a "cool marble" aesthetic. 

The interface should feel like a piece of precision machinery carved from stone.

## Visual Identity
- **Monochrome Foundation:** The primary palette is blue-shifted greys and deep blacks (AMOLED). The "Dark" theme is the primary experience, intended to reduce eye strain during long trading sessions while maintaining high contrast for critical signals.
- **Architectural Borders:** Instead of shadows, depth is communicated through distinct borders. In the light theme, these borders are doubled in weight to maintain the brutalist structure without relying on dark backgrounds.
- **Typography as Hierarchy:**
    - **Space Grotesk** is used for UI labels, buttons, and display text, providing a modern but slightly idiosyncratic feel.
    - **Space Mono** is used for all data, prices, and code, ensuring that numerical information is perfectly aligned and readable.
- **Retro-Tech Accents:** Subtle animations like `flicker` and `ticker-pulse`, combined with `scan-line` overlays, evoke the feel of high-end financial terminals and early digital instrumentation.

## Design Intent
1. **Information First:** Layouts are dense. Whitespace is used strategically to separate logical blocks, but never at the expense of seeing the "full picture" of the market.
2. **Signal over Noise:** The only vibrant colors allowed are signal colors (Green/Red/Amber). These are reserved for market movement, order status, and warnings.
3. **Tactile Digitalism:** Buttons and interactive elements should feel "clicky" and responsive, using sharp corners and immediate color transitions rather than slow fades.
4. **The "Crest":** The Testudo crest is a symbol of stability and defense (the "Tortoise"). It is often presented with minimal styling, either inverted or as a high-contrast mark.
