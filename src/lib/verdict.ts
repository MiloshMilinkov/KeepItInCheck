import type { Class, Outcome } from "./types";

/** How each flagged class is shown everywhere (chip, dot, checkbox tone, tooltip). */
export const VERDICT: Record<Exclude<Class, "keep">, { label: string; title: string; chip: string; dot: string; tone: "accent" | "amber" }> = {
  safe: {
    label: "Safe",
    title: "Safe to close: idle background app",
    chip: "bg-accent-soft text-accent",
    dot: "bg-accent",
    tone: "accent",
  },
  probable: {
    label: "Review",
    title: "Probably safe: review before closing",
    chip: "bg-amber-soft text-amber",
    dot: "bg-amber",
    tone: "amber",
  },
};

/** The app is gone after this outcome (so Relaunch makes sense). */
export function isClosed(o: Outcome): boolean {
  return o === "closed" || o === "forceKilled";
}
