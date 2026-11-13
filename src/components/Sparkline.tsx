import { memo, useId } from "react";

const W = 60; // one unit per second
const H = 24;

/** 60-second sparkline. `points` are [secondsAgo, value 0..100]. */
export const Sparkline = memo(function Sparkline({ points, high }: { points: [number, number][]; high: boolean }) {
  const id = useId();
  if (points.length < 2) {
    return <div className="h-6 w-full rounded-sm bg-track/40" aria-hidden />;
  }
  const max = Math.max(25, ...points.map((p) => p[1])) * 1.1;
  const xy = points.map(([ago, v]) => [W - Math.min(W, ago), H - (v / max) * (H - 2) - 1] as const);
  const line = xy.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(2)} ${y.toFixed(2)}`).join("");
  const area = `${line}L${xy[xy.length - 1][0].toFixed(2)} ${H}L${xy[0][0].toFixed(2)} ${H}Z`;
  const color = high ? "var(--danger)" : "var(--accent)";
  return (
    <svg className="h-6 w-full" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" aria-hidden>
      <defs>
        <linearGradient id={`${id}-f`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={color} stopOpacity="0.28" />
          <stop offset="1" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <path d={area} fill={`url(#${id}-f)`} />
      <path d={line} fill="none" stroke={color} strokeWidth="1.5" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
    </svg>
  );
});
