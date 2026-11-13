import { useEffect, useRef } from "react";
import { useHighLoad } from "../hooks/useSnapshot";
import type { Edge } from "../lib/types";

/**
 * The collapsed state: a thin glowing line on the screen edge. Hovering it (with a short
 * intent delay so a passing cursor does not trigger it) slides the panel out.
 */
export function EdgeStrip({ edge, onOpen }: { edge: Edge; onOpen: () => void }) {
  const hot = useHighLoad();
  const timer = useRef<number>(0);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const vertical = edge === "left" || edge === "right";

  return (
    <div
      className="flex h-full w-full items-center justify-center"
      onMouseEnter={() => {
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(onOpen, 140);
      }}
      onMouseLeave={() => window.clearTimeout(timer.current)}
      onClick={onOpen}
      role="button"
      aria-label="Show KeepItInCheck"
    >
      <span
        className="block rounded-full transition-[background,box-shadow] duration-700"
        style={{
          width: vertical ? 3 : "100%",
          height: vertical ? "100%" : 3,
          background: hot
            ? "var(--danger)"
            : `linear-gradient(${vertical ? "180deg" : "90deg"}, transparent, var(--accent) 22%, var(--accent-2) 78%, transparent)`,
          boxShadow: `0 0 8px ${hot ? "var(--danger)" : "var(--accent)"}`,
        }}
      />
    </div>
  );
}
