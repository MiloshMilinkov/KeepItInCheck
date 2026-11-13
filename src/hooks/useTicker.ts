import { useEffect, useRef } from "react";

export const reducedMotion = typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Eases a number towards `value` and hands every frame to `apply`, which writes straight into
 * the DOM: zero React renders per frame. Every animation frame of this transparent window costs
 * WebView2 a couple of milliseconds, so tweens are short and skipped entirely when `isSame`
 * says the change would not be visible.
 */
export function useTween(
  value: number,
  apply: (v: number) => void,
  { duration = 260, isSame }: { duration?: number; isSame?: (a: number, b: number) => boolean } = {},
) {
  const current = useRef(value);
  const applyRef = useRef(apply);
  applyRef.current = apply;
  const sameRef = useRef(isSame);
  sameRef.current = isSame;

  useEffect(() => {
    const from = current.current;
    if (reducedMotion || !Number.isFinite(from) || from === value || sameRef.current?.(from, value)) {
      current.current = value;
      applyRef.current(value);
      return;
    }
    const start = performance.now();
    let raf = 0;
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      const eased = 1 - Math.pow(1 - t, 3);
      const v = from + (value - from) * eased;
      current.current = v;
      applyRef.current(v);
      if (t < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [value, duration]);
}

/** A number ticker: returns a ref for the element that shows the formatted value. */
export function useTicker<T extends HTMLElement>(value: number, format: (v: number) => string, duration = 260) {
  const ref = useRef<T>(null);
  const fmt = useRef(format);
  fmt.current = format;
  useTween(
    value,
    (v) => {
      if (ref.current) ref.current.textContent = fmt.current(v);
    },
    { duration, isSame: (a, b) => fmt.current(a) === fmt.current(b) },
  );
  return ref;
}
