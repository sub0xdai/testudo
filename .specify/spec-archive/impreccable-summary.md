Confirmed by screenshots:
  - The ArcGauge looks excellent in practice — the dot glow, gradient interpolation, and the stone texture backdrop create a genuinely distinctive balance panel. The 176px allocation is earned here.
  - Inactive tab text IS very dim — "Quick", "Positions", "Account" in screenshot 1 are barely distinguishable from the background, confirming the text-dim contrast failure (C-7).
  - Position card is well-executed — the red left accent border, SHORT/ACTIVE badges, SL in red, and the compact info density all read clearly. Good visual hierarchy.
  - The "Woo" exchange pill renders green — so accent-green IS resolving via Tailwind fallback, but it's relying on luck rather than an explicit token.
  - Settings gear and Woo pill are visually small — confirms the touch target concerns (M-9).
  - The "BE: OFF" / "TRAIL: OFF" badges are visible and the hardcoded "TRAIL: OFF" bug (L-9) would be noticeable if trailing were actually enabled on a trade.

  New visual note not caught in code review:
  - The balance panel's stone texture is more prominent than the code's 0.92 → 0.65 opacity gradient suggested — it reads as a deliberate design choice, not an accident. The right side of the texture is more visible (0.65 opacity), creating an asymmetric depth effect
  that works well.

  The screenshots validate the analysis. The extension looks solid for a trading tool — the priority is tightening the foundation (tokens, accessibility) rather than redesigning surfaces.
