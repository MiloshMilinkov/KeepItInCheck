// Small shared controls.

import { AnimatePresence, motion } from "framer-motion";
import { IconBack } from "./icons";
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

/** A bare on/off switch. */
export function Switch({
  checked,
  onChange,
  disabled,
  id,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  id?: string;
  label?: string;
}) {
  return (
    <button
      id={id}
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative mt-0.5 h-[18px] w-[30px] shrink-0 rounded-full transition-colors disabled:opacity-50 ${checked ? "bg-accent" : "bg-track"}`}
    >
      <motion.span
        className="absolute top-[2px] left-[2px] size-[14px] rounded-full bg-white shadow"
        animate={{ x: checked ? 12 : 0 }}
        transition={{ type: "spring", stiffness: 700, damping: 40 }}
      />
    </button>
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
      <Switch id={id} checked={checked} onChange={onChange} disabled={disabled} />
    </div>
  );
}

/** Full-panel sheet with a back button, shared by Settings, Startup apps and the report. */
export function SubSheet({ title, onBack, children }: { title: string; onBack: () => void; children: ReactNode }) {
  return (
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="absolute inset-0 z-30 flex flex-col rounded-[14px] bg-bg"
      initial={{ opacity: 0, x: 16 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 16 }}
      transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
    >
      <header className="flex h-11 shrink-0 items-center gap-1 border-b border-line pr-2 pl-2">
        <button onClick={onBack} aria-label="Back" className="flex size-7 items-center justify-center rounded-md text-ink-2 hover:bg-hover hover:text-ink">
          <IconBack />
        </button>
        <h2 className="m-0 flex-1 text-[13px] font-semibold text-ink">{title}</h2>
      </header>
      <div className="scroll min-h-0 flex-1">{children}</div>
    </motion.div>
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

/** Thin proportional bar along the bottom of a list row. Transform-only, so it animates on the
 *  compositor. `share` is 0..1. */
export function ShareBar({ share, hot = false }: { share: number; hot?: boolean }) {
  return (
    <span className="pointer-events-none absolute right-2 bottom-1 left-[38px] h-[2px] overflow-hidden rounded-full bg-track/60">
      <span
        className={`block h-full origin-left rounded-full transition-transform duration-200 ease-out ${hot ? "bg-danger" : "bg-accent/70"}`}
        style={{ transform: `scaleX(${Math.max(0.01, Math.min(1, share))})` }}
      />
    </span>
  );
}

/** Inline "are you sure?" panel that slides open under a row. */
export function InlineConfirm({
  open,
  message,
  cancelLabel,
  confirmLabel,
  onCancel,
  onConfirm,
  tone = "accent",
  busy,
}: {
  open: boolean;
  message: ReactNode;
  cancelLabel: string;
  confirmLabel: string;
  onCancel: () => void;
  onConfirm: () => void;
  tone?: "accent" | "danger";
  busy?: boolean;
}) {
  const danger = tone === "danger";
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: "auto" }}
          exit={{ opacity: 0, height: 0 }}
          className="overflow-hidden"
        >
          <div className={`mt-2 rounded-lg border p-2.5 ${danger ? "border-danger/40 bg-danger-soft" : "border-line bg-raised"}`}>
            <p className="m-0 text-[12px] leading-snug text-ink">{message}</p>
            <div className="mt-2 flex gap-1.5">
              <button onClick={onCancel} className="rounded-md px-2 py-1 text-[11.5px] whitespace-nowrap text-ink-2 hover:bg-hover">
                {cancelLabel}
              </button>
              <button
                disabled={busy}
                onClick={onConfirm}
                className={`rounded-md px-2 py-1 text-[11.5px] font-semibold whitespace-nowrap hover:brightness-110 ${danger ? "bg-danger text-white" : "bg-accent text-accent-ink"}`}
              >
                {confirmLabel}
              </button>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
