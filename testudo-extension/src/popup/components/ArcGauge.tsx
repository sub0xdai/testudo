interface ArcGaugeProps {
  /** Exposure percentage 0-100 */
  exposure: number;
  /** Dollar amount at risk */
  atRisk: number;
  /** Total account balance */
  totalBalance: number;
}

/**
 * Semi-circle (180-degree) risk gauge with spaced tick nodes.
 * Green (low risk) → Amber (mid) → Red (high risk) gradient.
 */
export default function ArcGauge(props: ArcGaugeProps) {
  const radius = 85;
  const strokeWidth = 5;
  const center = 100;

  // Semi-circle circumference: PI * r
  const circumference = Math.PI * radius;

  // Tight ticks: 3px dash, 10px gap = precision dial
  const dashArray = "3 10";

  // stroke-dashoffset: full circumference = 0%, 0 = 100%
  const offset = () => circumference - ((Math.min(100, Math.max(0, props.exposure)) || 0) / 100) * circumference;

  return (
    <div class="relative flex flex-col items-center justify-end h-44 w-full px-10 overflow-hidden mt-2" data-testid="arc-gauge">
      <svg viewBox="0 0 200 110" class="w-full h-full overflow-visible">
        <defs>
          <linearGradient id="risk-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stop-color="#10B981" />
            <stop offset="50%" stop-color="#F59E0B" />
            <stop offset="100%" stop-color="#EF4444" />
          </linearGradient>
        </defs>

        {/* Background track (dark ticks) */}
        <path
          d={`M ${center - radius} ${center} A ${radius} ${radius} 0 0 1 ${center + radius} ${center}`}
          fill="none"
          stroke="#27272A"
          stroke-width={strokeWidth}
          stroke-linecap="round"
          stroke-dasharray={dashArray}
        />

        {/* Active track (green → amber → red ticks) */}
        <path
          d={`M ${center - radius} ${center} A ${radius} ${radius} 0 0 1 ${center + radius} ${center}`}
          fill="none"
          stroke="url(#risk-gradient)"
          stroke-width={strokeWidth}
          stroke-linecap="round"
          stroke-dasharray={dashArray}
          stroke-dashoffset={offset()}
          class="transition-all duration-1000 ease-out"
        />
      </svg>

      {/* Gauge center text */}
      <div class="absolute bottom-0 flex flex-col items-center mb-1">
        <span class="text-4xl font-bold text-white tracking-tighter" style={{ "font-family": "var(--font-family-mono)" }}>
          {props.exposure.toFixed(1)}%
        </span>
        <span class="text-[10px] uppercase tracking-widest text-text-secondary mt-1">
          Exposure
        </span>
      </div>
    </div>
  );
}
