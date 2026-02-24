import { For } from "solid-js";

interface ArcGaugeProps {
  /** Exposure percentage 0-100 */
  exposure: number;
  /** Dollar amount at risk */
  atRisk: number;
  /** Total account balance */
  totalBalance: number;
}

const TICK_COUNT = 21;
const RADIUS = 85;
const CENTER = 100;
const DOT_RADIUS = 3.5;

/** Interpolate color along green → amber → red gradient at position t (0-1) */
function tickColor(t: number): string {
  if (t < 0.5) {
    const p = t / 0.5;
    const r = Math.round(16 + (245 - 16) * p);
    const g = Math.round(185 + (158 - 185) * p);
    const b = Math.round(129 + (11 - 129) * p);
    return `rgb(${r},${g},${b})`;
  }
  const p = (t - 0.5) / 0.5;
  const r = Math.round(245 + (239 - 245) * p);
  const g = Math.round(158 + (68 - 158) * p);
  const b = Math.round(11 + (68 - 11) * p);
  return `rgb(${r},${g},${b})`;
}

/**
 * Semi-circle (180-degree) risk gauge with individual dot ticks.
 * The dot nearest the current exposure glows; other active dots are muted.
 */
export default function ArcGauge(props: ArcGaugeProps) {
  const exposure = () => Math.min(100, Math.max(0, props.exposure || 0));

  // Which tick index is the exposure closest to?
  const activeTickIndex = () => Math.round((exposure() / 100) * (TICK_COUNT - 1));

  const ticks = Array.from({ length: TICK_COUNT }, (_, i) => i);

  return (
    <div class="relative flex flex-col items-center justify-end h-44 w-full px-10 overflow-hidden mt-2" data-testid="arc-gauge">
      <svg viewBox="0 0 200 110" class="w-full h-full overflow-visible">
        <defs>
          <filter id="tick-glow">
            <feGaussianBlur stdDeviation="3" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        <For each={ticks}>
          {(i) => {
            const t = i / (TICK_COUNT - 1);
            const angle = Math.PI * (1 - t);
            const x = CENTER + RADIUS * Math.cos(angle);
            const y = CENTER - RADIUS * Math.sin(angle);

            const isActive = () => i <= activeTickIndex();
            const isNeedle = () => i === activeTickIndex() && exposure() > 0;

            // Distance from active tick (0 = at needle, higher = further away)
            const dist = () => Math.abs(i - activeTickIndex());

            // Active dots near needle: full opacity, far: muted
            const opacity = () => {
              if (!isActive()) return 1;
              if (isNeedle()) return 1;
              const d = dist();
              if (d <= 2) return 0.9;
              return 0.35;
            };

            const color = () => isActive() ? tickColor(t) : "#27272A";
            const r = () => isNeedle() ? DOT_RADIUS + 1.5 : DOT_RADIUS;

            return (
              <circle
                cx={x}
                cy={y}
                r={r()}
                fill={color()}
                opacity={opacity()}
                filter={isNeedle() ? "url(#tick-glow)" : undefined}
                class="transition-all duration-700 ease-out"
              />
            );
          }}
        </For>
      </svg>

      {/* Gauge center text */}
      <div class="absolute bottom-0 flex flex-col items-center mb-1">
        <span class="text-4xl font-bold text-white tracking-tighter" style={{ "font-family": "var(--font-family-mono)" }}>
          {props.exposure.toFixed(1)}%
        </span>
        <span class="text-[11px] uppercase tracking-widest text-zinc-400 mt-1">
          Exposure
        </span>
      </div>
    </div>
  );
}
