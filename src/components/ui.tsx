// Small shared controls.

import { motion } from "framer-motion";
import { useId, type ReactNode } from "react";

export function Segmented<T extends string>({
  value,
  onChange,
  options,
  label,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  label: string;
}) {
  const index = Math.max(0, options.findIndex((o) => o.value === value));
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="relative grid rounded-md bg-track p-0.5"
      style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}
    >
      {/* Sliding pill: a CSS transform, so it costs nothing until the value changes. */}
      <span
        aria-hidden
        className="absolute top-0.5 bottom-0.5 left-0.5 rounded-[5px] bg-raised shadow-[0_1px_2px_rgb(0_0_0/0.25)] transition-transform duration-200 ease-out"
        style={{ width: `calc((100% - 4px) / ${options.length})`, transform: `translateX(${index * 100}%)` }}
      />
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            role="radio"
            aria-checked={active}
            onClick={() => onChange(o.value)}
            className={`relative z-10 rounded-[5px] px-2 py-0.5 text-[11px] font-medium whitespace-nowrap transition-colors ${active ? "text-ink" : "text-ink-2 hover:text-ink"}`}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export function Toggle({ checked, onChange, label, hint, disabled }: { checked: boolean; onChange: (v: boolean) => void; label: string; hint?: ReactNode; disabled?: boolean }) {
  const id = useId();
  return (
    <div className={`flex items-start justify-between gap-3 py-2 ${disabled ? "opacity-50" : ""}`}>
      <label htmlFor={id} className="min-w-0">
        <span className="block text-[12.5px] text-ink">{label}</span>
        {hint && <span className="mt-0.5 block text-[11.5px] leading-snug text-ink-2">{hint}</span>}
      </label>
      <button
        id={id}
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative mt-0.5 h-[18px] w-[30px] shrink-0 rounded-full transition-colors ${checked ? "bg-accent" : "bg-track"}`}
      >
        <motion.span
          className="absolute top-[2px] left-[2px] size-[14px] rounded-full bg-white shadow"
          animate={{ x: checked ? 12 : 0 }}
          transition={{ type: "spring", stiffness: 700, damping: 40 }}
        />
      </button>
    </div>
  );
}

export function Checkbox({ checked, onChange, label, tone = "accent" }: { checked: boolean; onChange: (v: boolean) => void; label: string; tone?: "accent" | "amber" }) {
  return (
    <button
      role="checkbox"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`flex size-4 shrink-0 items-center justify-center rounded-[4px] border transition-colors ${
        checked ? (tone === "amber" ? "border-amber bg-amber" : "border-accent bg-accent") : "border-ink-3 bg-transparent"
      }`}
    >
      {checked && (
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
          <path d="M2 5.2l2 2 4-4.4" fill="none" stroke="var(--accent-ink)" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
    </button>
  );
}

export function NumberField({
  label,
  value,
  onChange,
  min,
  max,
  step = 1,
  unit,
  hint,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  step?: number;
  unit: string;
  hint?: string;
}) {
  const id = useId();
  return (
    <div className="flex items-center justify-between gap-3 py-1.5">
      <label htmlFor={id} className="min-w-0">
        <span className="block text-[12.5px] text-ink">{label}</span>
        {hint && <span className="block text-[11.5px] leading-snug text-ink-2">{hint}</span>}
      </label>
      <span className="flex shrink-0 items-center gap-1.5 rounded-md bg-track px-2 py-1">
        <input
          id={id}
          type="number"
          value={value}
          min={min}
          max={max}
          step={step}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, v)));
          }}
          className="w-14 bg-transparent text-right text-[12.5px] text-ink outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
        />
        <span className="text-[11px] text-ink-2">{unit}</span>
      </span>
    </div>
  );
}

export function IconButton({ label, onClick, children, active }: { label: string; onClick: () => void; children: ReactNode; active?: boolean }) {
  return (
    <button
      aria-label={label}
      title={label}
      onClick={onClick}
      className={`flex size-7 items-center justify-center rounded-md transition-colors hover:bg-hover hover:text-ink ${active ? "text-accent" : "text-ink-2"}`}
    >
      {children}
    </button>
  );
}
