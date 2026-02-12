interface ArcGaugeProps {
  /** Exposure percentage 0-100 */
  exposure: number;
  /** Dollar amount at risk */
  atRisk: number;
  /** Total account balance */
  totalBalance: number;
}

function formatMoney(value: number): string {
  if (value >= 1000) {
    return "$" + value.toLocaleString("en-US", { minimumFractionDigits: 0, maximumFractionDigits: 0 });
  }
  return "$" + value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/** Color interpolation: green (0%) → orange (50%) → red (100%) */
function exposureColor(pct: number): string {
  const clamped = Math.max(0, Math.min(100, pct));
  if (clamped <= 50) {
    const t = clamped / 50;
    const r = Math.round(0 + t * 255);
    const g = Math.round(210 - t * 70);
    const b = Math.round(106 - t * 40);
    return `rgb(${r}, ${g}, ${b})`;
  }
  const t = (clamped - 50) / 50;
  const r = 255;
  const g = Math.round(140 - t * 69);
  const b = Math.round(66 - t * (66 - 87));
  return `rgb(${r}, ${g}, ${b})`;
}

/**
 * SVG arc gauge showing account exposure.
 * 220° sweep arc, centered, with hero number inside.
 */
export default function ArcGauge(props: ArcGaugeProps) {
  const cx = 130;
  const cy = 115;
  const r = 90;

  // Arc sweep: 220 degrees (-200° to +20° from bottom)
  const startAngle = -200;
  const endAngle = 20;
  const totalSweep = endAngle - startAngle; // 220°

  function polarToCartesian(angle: number): { x: number; y: number } {
    const rad = ((angle - 90) * Math.PI) / 180;
    return {
      x: cx + r * Math.cos(rad),
      y: cy + r * Math.sin(rad),
    };
  }

  function arcPath(startDeg: number, endDeg: number): string {
    const start = polarToCartesian(startDeg);
    const end = polarToCartesian(endDeg);
    const sweep = endDeg - startDeg;
    const largeArc = sweep > 180 ? 1 : 0;
    return `M ${start.x} ${start.y} A ${r} ${r} 0 ${largeArc} 1 ${end.x} ${end.y}`;
  }

  const fillAngle = () => {
    const clamped = Math.max(0, Math.min(100, props.exposure));
    return startAngle + (clamped / 100) * totalSweep;
  };

  // Dashed tick marks along the arc
  const ticks = () => {
    const count = 40;
    const tickPaths: { x1: number; y1: number; x2: number; y2: number; active: boolean }[] = [];
    for (let i = 0; i <= count; i++) {
      const angle = startAngle + (i / count) * totalSweep;
      const rad = ((angle - 90) * Math.PI) / 180;
      const innerR = r - 4;
      const outerR = r + 4;
      tickPaths.push({
        x1: cx + innerR * Math.cos(rad),
        y1: cy + innerR * Math.sin(rad),
        x2: cx + outerR * Math.cos(rad),
        y2: cy + outerR * Math.sin(rad),
        active: (i / count) * 100 <= props.exposure,
      });
    }
    return tickPaths;
  };

  return (
    <div class="flex flex-col items-center py-3" data-testid="arc-gauge">
      <svg width="260" height="160" viewBox="0 0 260 160">
        {/* Background arc (track) */}
        <path
          d={arcPath(startAngle, endAngle)}
          fill="none"
          stroke="var(--color-bg-elevated)"
          stroke-width="8"
          stroke-linecap="square"
        />

        {/* Tick marks */}
        {ticks().map((tick) => (
          <line
            x1={tick.x1}
            y1={tick.y1}
            x2={tick.x2}
            y2={tick.y2}
            stroke={tick.active ? exposureColor(props.exposure) : "var(--color-border-grid)"}
            stroke-width="1.5"
            opacity={tick.active ? 0.8 : 0.3}
          />
        ))}

        {/* Filled arc (exposure) */}
        {props.exposure > 0 && (
          <path
            d={arcPath(startAngle, fillAngle())}
            fill="none"
            stroke={exposureColor(props.exposure)}
            stroke-width="8"
            stroke-linecap="square"
            style={{ filter: `drop-shadow(0 0 6px ${exposureColor(props.exposure)}40)` }}
          />
        )}

        {/* Hero number */}
        <text
          x={cx}
          y={cy - 10}
          text-anchor="middle"
          class="arc-gauge-text"
          font-size="36"
        >
          {props.exposure.toFixed(1)}%
        </text>

        {/* Label */}
        <text
          x={cx}
          y={cy + 12}
          text-anchor="middle"
          class="arc-gauge-label"
          font-size="10"
        >
          ACCOUNT EXPOSURE
        </text>

        {/* Sub info */}
        <text
          x={cx}
          y={cy + 30}
          text-anchor="middle"
          class="arc-gauge-sub"
          font-size="11"
        >
          {formatMoney(props.atRisk)} at risk of {formatMoney(props.totalBalance)}
        </text>
      </svg>
    </div>
  );
}
