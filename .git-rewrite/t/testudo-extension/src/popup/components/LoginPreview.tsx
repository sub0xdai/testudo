/**
 * LoginPreview — Static mockup of the main dashboard.
 * Purely decorative: no imports from webextension-polyfill, no API calls,
 * no service worker messages, no storage reads. All values hardcoded.
 */

/** Static arc gauge dots — pre-computed positions for 21 ticks at 18% exposure */
function StaticArcGauge() {
  const TICK_COUNT = 21;
  const RADIUS = 85;
  const CENTER = 100;
  const DOT_RADIUS = 3.5;
  const ACTIVE_INDEX = 4; // ~18% exposure

  function tickColor(t: number): string {
    if (t < 0.5) {
      const p = t / 0.5;
      const r = Math.round(16 + (245 - 16) * p);
      const g = Math.round(185 + (245 - 185) * p);
      const b = Math.round(129 + (245 - 129) * p);
      return `rgb(${r},${g},${b})`;
    }
    const p = (t - 0.5) / 0.5;
    const r = Math.round(245 + (245 - 245) * p);
    const g = Math.round(245 + (245 - 245) * p);
    const b = Math.round(245 + (245 - 245) * p);
    return `rgb(${r},${g},${b})`;
  }

  const dots = Array.from({ length: TICK_COUNT }, (_, i) => {
    const t = i / (TICK_COUNT - 1);
    const angle = Math.PI * (1 - t);
    const x = CENTER + RADIUS * Math.cos(angle);
    const y = CENTER - RADIUS * Math.sin(angle);
    const isNeedle = i === ACTIVE_INDEX;
    const dist = Math.abs(i - ACTIVE_INDEX);
    const opacity = isNeedle ? 1 : dist <= 1 ? 0.8 : dist <= 3 ? 0.5 : 0.25;
    const r = isNeedle ? DOT_RADIUS + 1.5 : DOT_RADIUS;
    return { x, y, r, opacity, fill: tickColor(t) };
  });

  return (
    <div class="relative flex flex-col items-center justify-end h-44 w-full px-10 overflow-hidden mt-2">
      <svg viewBox="0 0 200 110" class="w-full h-full overflow-visible">
        {dots.map((d) => (
          <circle cx={d.x} cy={d.y} r={d.r} fill={d.fill} opacity={d.opacity} />
        ))}
      </svg>
      <div class="absolute bottom-0 flex flex-col items-center mb-1">
        <span class="text-4xl font-bold text-text-primary tracking-tighter font-family-mono">
          18.1%
        </span>
        <span class="text-[11px] uppercase tracking-widest text-text-secondary mt-1">
          Exposure
        </span>
      </div>
    </div>
  );
}

function MockToggleCard(props: { label: string; detail: string }) {
  return (
    <div class="bg-bg-panel border border-border-subtle">
      <div class="flex items-center justify-between px-4 py-3">
        <span class="text-[13px] font-sans font-semibold text-text-primary">{props.label}</span>
        <span class="text-[11px] font-mono font-bold text-signal-green tracking-wider">ON</span>
      </div>
      <div class="px-4 pb-3">
        <span class="text-[12px] font-mono text-text-dim">{props.detail}</span>
      </div>
    </div>
  );
}

export default function LoginPreview() {
  return (
    <div class="flex flex-col h-full bg-bg-core text-text-primary font-mono">
      {/* Mock header bar — UXP-17: white dots instead of signal-green */}
      <div class="flex items-center justify-between px-5 py-2.5">
        <div class="flex items-center gap-2">
          <span class="w-2 h-2 rounded-full bg-text-primary" />
          <span class="w-2 h-2 rounded-full bg-text-primary opacity-50" />
        </div>
        <div class="flex items-center gap-2">
          {/* UXP-17: white badge instead of accent-green */}
          <span class="text-[11px] font-mono font-bold text-text-primary bg-text-primary/10 px-1.5 py-0.5 tracking-wider uppercase">
            WOO
          </span>
          <span class="text-text-dim">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </span>
        </div>
      </div>

      {/* Mock balance panel */}
      <div class="balance-panel">
        <div class="balance-panel-overlay" />
        <div class="flex flex-col items-center pt-5 pb-2">
          <div class="flex items-center gap-2 mb-2">
            <span class="text-[12px] font-medium text-text-primary/70 tracking-widest uppercase">Balance</span>
            {/* UXP-17: white badge instead of accent-green */}
            <span class="text-[10px] font-bold text-text-primary bg-text-primary/10 px-1.5 py-0.5 tracking-wider uppercase">
              WOO
            </span>
          </div>
          <span
            class="text-[42px] font-bold text-text-primary tracking-tight leading-none text-shadow-balance"
          >
            $12,450.00
          </span>
          <div class="flex items-center gap-2 mt-2">
            {/* KEEP: available balance is trading data */}
            <span class="text-[12px] text-signal-green font-medium">$10,200.00 available</span>
            <span class="text-text-dim">&middot;</span>
            <span class="text-[12px] text-text-secondary">$2,250.00 locked</span>
          </div>
        </div>
        <StaticArcGauge />
      </div>

      {/* Mock tab bar */}
      <nav>
        <div class="flex mx-5 my-2 bg-bg-panel p-1">
          <span class="flex-1 py-2 text-[13px] font-sans font-semibold tracking-wide text-center tab-active">Trade</span>
          <span class="flex-1 py-2 text-[13px] font-sans font-semibold tracking-wide text-center tab-inactive">
            Positions
            {/* UXP-17: white count badge */}
            <span class="ml-1.5 text-[11px] font-mono text-text-primary bg-text-primary/10 px-1.5 py-0.5">2</span>
          </span>
          <span class="flex-1 py-2 text-[13px] font-sans font-semibold tracking-wide text-center tab-inactive">Account</span>
        </div>
      </nav>

      {/* Mock trade management cards */}
      <div class="flex-1 px-5 py-3 space-y-2.5">
        <MockToggleCard label="Stop Loss" detail="Trailing: 1.5%" />
        <MockToggleCard label="Take Profit" detail="Partial: 50% @ 2:1 R" />
        <MockToggleCard label="Break Even" detail="Trigger: 1:1 R" />
      </div>
    </div>
  );
}
