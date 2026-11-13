import { api } from "../lib/api";
import type { Edge } from "../lib/types";
import { IconCompact, IconExpand, IconGear, IconHide, IconPause, IconPlay } from "./icons";
import { IconButton } from "./ui";

interface Props {
  edge: Edge;
  paused: boolean;
  compact: boolean;
  onPause: () => void;
  onCompact: () => void;
  onSettings: () => void;
  onHide: () => void;
}

/** The "hide to edge" arrow points at the edge the panel is docked to. */
const HIDE_ROTATION = { right: 0, bottom: 90, left: 180, top: 270 } as const;

/** Drag handle + controls. Drag the panel anywhere and it snaps to the nearest screen edge. */
export function TitleBar({ edge, paused, compact, onPause, onCompact, onSettings, onHide }: Props) {
  return (
    <header
      className="flex h-11 shrink-0 items-center gap-0.5 pr-2 pl-4"
      onPointerDown={(e) => {
        // Drag from anywhere on the bar except its buttons; release snaps to the nearest edge.
        if (e.button === 0 && !(e.target as HTMLElement).closest("button")) api.beginDrag();
      }}
    >
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden className="mr-2 shrink-0">
        <circle cx="8" cy="8" r="6" fill="none" stroke="var(--track)" strokeWidth="2.2" />
        <circle
          cx="8"
          cy="8"
          r="6"
          fill="none"
          stroke="var(--accent)"
          strokeWidth="2.2"
          strokeLinecap="round"
          strokeDasharray="28 38"
          transform="rotate(-90 8 8)"
        />
      </svg>
      <h1 className="m-0 flex-1 truncate text-[13px] font-semibold tracking-[-0.01em] text-ink">
        KeepItInCheck
      </h1>
      <IconButton label={paused ? "Resume monitoring" : "Pause monitoring"} onClick={onPause} active={paused}>
        {paused ? <IconPlay /> : <IconPause />}
      </IconButton>
      <IconButton label={compact ? "Full view" : "Compact view"} onClick={onCompact}>
        {compact ? <IconExpand /> : <IconCompact />}
      </IconButton>
      <IconButton label="Settings" onClick={onSettings}>
        <IconGear />
      </IconButton>
      <IconButton label="Hide to edge" onClick={onHide}>
        <span style={{ transform: `rotate(${HIDE_ROTATION[edge]}deg)` }} className="flex">
          <IconHide />
        </span>
      </IconButton>
    </header>
  );
}
