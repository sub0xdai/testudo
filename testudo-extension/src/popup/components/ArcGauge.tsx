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

/** Read a CSS custom property value from :root, with fallback */
const getColor = (name: string, fallback: string) => {
  if (typeof document === 'undefined') return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
};

/** Interpolate color along green -> amber -> red gradient at position t (0-1) */
function tickColor(t: number): string {
  const green = getColor('--color-signal-green', '#22C55E');
  const orange = getColor('--color-signal-orange', '#f59e0b');
  const red = getColor('--color-signal-red', '#EF4444');

  // Parse hex/named colors to RGB for interpolation
  function parseColor(c: string): [number, number, number] {
    const ctx = typeof document !== 'undefined' ? document.createElement('canvas').getContext('2d') : null;
    if (ctx) {
      ctx.fillStyle = c;
      const hex = ctx.fillStyle;
      const m = hex.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
      if (m) return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
    }
    // Fallback: try direct hex parse
    const m = c.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
    if (m) return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
    return [128, 128, 128];
  }

  const [gr, gg, gb] = parseColor(green);
  const [or, og, ob] = parseColor(orange);
  const [rr, rg, rb] = parseColor(red);

  if (t < 0.5) {
    const p = t / 0.5;
    const r = Math.round(gr + (or - gr) * p);
    const g = Math.round(gg + (og - gg) * p);
    const b = Math.round(gb + (ob - gb) * p);
    return `rgb(${r},${g},${b})`;
  }
  const p = (t - 0.5) / 0.5;
  const r = Math.round(or + (rr - or) * p);
  const g = Math.round(og + (rg - og) * p);
  const b = Math.round(ob + (rb - ob) * p);
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

            const isNeedle = () => i === activeTickIndex() && exposure() > 0;
            const dist = () => Math.abs(i - activeTickIndex());

            // All dots show gradient color; muted baseline, bright near needle
            // Light theme uses higher base opacity for visibility on beige
            const isLightTheme = () =>
              typeof document !== 'undefined' &&
              document.documentElement.getAttribute('data-theme') === 'light';

            const opacity = () => {
              const base = isLightTheme() ? 0.45 : 0.25;
              if (isNeedle()) return 1;
              const d = dist();
              if (d <= 1) return 0.9;
              if (d <= 3) return 0.65;
              return base;
            };

            const r = () => isNeedle() ? DOT_RADIUS + 1.5 : DOT_RADIUS;

            return (
              <circle
                cx={x}
                cy={y}
                r={r()}
                fill={tickColor(t)}
                opacity={opacity()}
                filter={isNeedle() ? "url(#tick-glow)" : undefined}
                style={{ transition: "r 700ms ease-out, opacity 700ms ease-out" }}
              />
            );
          }}
        </For>
      </svg>

      {/* Gauge center text */}
      <div class="absolute bottom-0 flex flex-col items-center mb-1">
        <span class="text-4xl font-bold text-text-primary tracking-tighter font-family-mono">
          {props.exposure.toFixed(1)}%
        </span>
        <span class="text-[11px] uppercase tracking-widest text-text-secondary mt-1">
          Exposure
        </span>
      </div>
    </div>
  );
}
