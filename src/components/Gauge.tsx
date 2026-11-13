import { memo, useId, useRef } from "react";
import { useTween } from "../hooks/useTicker";
import { isHigh } from "../lib/format";
import { Sparkline } from "./Sparkline";

interface Props {
  label: string;
  /** 0..100 */
  value: number;
  /** Secondary line under the label, e.g. "19.4 of 32 GB". */
  detail: string;
  /** [secondsAgo, value] pairs for the last 60 s. */
  history: [number, number][];
}

const STROKE = 7;
const SIZE = 92;

/**
 * Ring gauge: the arc and the number in the middle ease together in one short tween that writes
 * straight to the DOM, plus a 60-second sparkline underneath. Turns red only above 85 %.
 */
export const Gauge = memo(function Gauge({ label, value, detail, history }: Props) {
  const size = SIZE;
  const id = useId();
  const r = (size - STROKE) / 2;
  const c = 2 * Math.PI * r;
  // Half-point steps: sub-point jitter does not start a new arc animation.
  const v = Math.round(Math.max(0, Math.min(100, value)) * 2) / 2;
  const offset = (x: number) => c * (1 - x / 100);
  const high = isHigh(v);
  const num = useRef<HTMLSpanElement>(null);
  const arc = useRef<SVGCircleElement>(null);
  const halo = useRef<SVGCircleElement>(null);
  useTween(v, (x) => {
    const o = String(offset(x));
    arc.current?.setAttribute("stroke-dashoffset", o);
    halo.current?.setAttribute("stroke-dashoffset", o);
    if (num.current) num.current.textContent = String(Math.round(x));
  });

  return (
    <figure className="m-0 flex min-w-0 flex-1 flex-col items-center gap-2" aria-label={`${label} ${Math.round(v)} percent`}>
      <div className="relative" style={{ width: size, height: size }}>
        <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="-rotate-90">
          <defs>
            <linearGradient id={`${id}-g`} x1="0" y1="0" x2="1" y2="1">
              <stop offset="0" stopColor="var(--accent)" />
              <stop offset="1" stopColor="var(--accent-2)" />
            </linearGradient>
          </defs>
          <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--track)" strokeWidth={STROKE} />
          {/* Soft halo under the arc instead of a CSS filter: cheaper to animate. */}
          <circle
            ref={halo}
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke={high ? "var(--danger)" : `url(#${id}-g)`}
            strokeOpacity={0.18}
            strokeWidth={STROKE + 6}
            strokeLinecap="round"
            strokeDasharray={c}
            strokeDashoffset={offset(v)}
          />
          <circle
            ref={arc}
            cx={size / 2}
            cy={size / 2}
            r={r}
            fill="none"
            stroke={high ? "var(--danger)" : `url(#${id}-g)`}
            strokeWidth={STROKE}
            strokeLinecap="round"
            strokeDasharray={c}
            strokeDashoffset={offset(v)}
          />
        </svg>
        <div className="absolute inset-0 flex items-center justify-center">
          <span className={`text-[26px] font-semibold tracking-[-0.03em] ${high ? "text-danger" : "text-ink"}`}>
            <span ref={num}>{Math.round(v)}</span>
            <span className="ml-px text-[13px] font-medium text-ink-2">%</span>
          </span>
        </div>
      </div>
      <figcaption className="flex w-full flex-col items-center gap-1.5">
        <div className="flex w-full items-baseline justify-between px-1">
          <span className="text-[12px] font-medium text-ink">{label}</span>
          <span className="truncate pl-2 text-[11px] text-ink-2">{detail}</span>
        </div>
        <Sparkline points={history} high={high} />
      </figcaption>
    </figure>
  );
});
