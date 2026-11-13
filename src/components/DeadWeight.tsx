import { AnimatePresence, motion } from "framer-motion";
import { useMemo, useState } from "react";
import { useTicker } from "../hooks/useTicker";
import { formatBytes, formatDuration } from "../lib/format";
import type { AppView, Snapshot } from "../lib/types";
import { VERDICT } from "../lib/verdict";
import { AppIcon } from "./AppIcon";
import { IconDots } from "./icons";

interface Props {
  snap: Snapshot;
  onCleanUp: () => void;
  onSnooze: (app: AppView) => void;
  onExclude: (app: AppView) => void;
}

/** Flagged apps shown inline; the rest are in the Clean up sheet. */
const MAX_SHOWN = 4;

export function flaggedApps(apps: AppView[]): AppView[] {
  return apps
    .filter((a) => a.verdict.class !== "keep")
    .sort((a, b) => (a.verdict.class === b.verdict.class ? b.mem - a.mem : a.verdict.class === "safe" ? -1 : 1));
}

/** The "dead weight" summary: what is idle, why, and how much RAM it holds. */
export function DeadWeight({ snap, onCleanUp, onSnooze, onExclude }: Props) {
  const flagged = useMemo(() => flaggedApps(snap.apps), [snap.apps]);
  const reclaim = useTicker<HTMLSpanElement>(snap.reclaimable, (v) => formatBytes(v));
  const learning = snap.observedSecs < snap.learningSecs;
  const shown = flagged.slice(0, MAX_SHOWN);

  return (
    <section aria-label="Dead weight" className="px-2">
      <header className="flex items-end justify-between px-2 pb-2">
        <div>
          <h2 className="m-0 text-[12px] font-medium text-ink-2">Dead weight</h2>
          <p className="m-0 mt-0.5 text-[11.5px] text-ink-3">
            {flagged.length === 0
              ? "No idle apps found"
              : `${flagged.length} idle ${flagged.length === 1 ? "app" : "apps"}`}
          </p>
        </div>
        <div className="text-right">
          <span ref={reclaim} className={`block text-[22px] font-semibold leading-none tracking-[-0.02em] ${snap.reclaimable > 0 ? "text-ink" : "text-ink-3"}`}>
            {formatBytes(snap.reclaimable)}
          </span>
          <span className="text-[11px] text-ink-2">reclaimable</span>
        </div>
      </header>

      {flagged.length === 0 ? (
        <div className="mx-2 mb-1 rounded-lg border border-dashed border-line px-3 py-3 text-[12px] leading-snug text-ink-2">
          {learning ? (
            <>
              <span className="text-ink">Learning what's idle.</span>{" "}
              {snap.learningSecs > 120
                ? `First suggestions appear after 2 min and firm up after ${formatDuration(snap.learningSecs)}.`
                : "First suggestions appear after 2 min."}
              <span className="mt-2 block h-[3px] overflow-hidden rounded-full bg-track">
                <span
                  className="block h-full origin-left bg-accent"
                  style={{ transform: `scaleX(${Math.min(1, snap.observedSecs / snap.learningSecs)})` }}
                />
              </span>
            </>
          ) : (
            <>Everything running has been used recently or is protected. Nothing to clean up.</>
          )}
        </div>
      ) : (
        <ul className="m-0 list-none p-0">
          <AnimatePresence initial={false}>
            {shown.map((a) => (
              <motion.li
                key={a.key}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, height: 0 }}
                transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
              >
                <FlaggedRow app={a} onSnooze={onSnooze} onExclude={onExclude} />
              </motion.li>
            ))}
          </AnimatePresence>
        </ul>
      )}

      {flagged.length > shown.length && (
        <p className="m-0 px-2 pt-1 text-[11.5px] text-ink-3">and {flagged.length - shown.length} more in Clean up</p>
      )}

      <div className="px-2 pt-2.5 pb-1">
        <button
          onClick={onCleanUp}
          disabled={flagged.length === 0}
          className="w-full rounded-lg bg-accent py-2 text-[13px] font-semibold text-accent-ink transition-[filter,opacity] hover:brightness-110 active:brightness-95 disabled:bg-track disabled:text-ink-3"
        >
          Clean up…
        </button>
      </div>
    </section>
  );
}

function FlaggedRow({ app, onSnooze, onExclude }: { app: AppView; onSnooze: (a: AppView) => void; onExclude: (a: AppView) => void }) {
  const [menu, setMenu] = useState(false);
  const v = VERDICT[app.verdict.class === "safe" ? "safe" : "probable"];
  return (
    <div className="group relative rounded-lg px-2 py-2 hover:bg-hover" onMouseLeave={() => setMenu(false)}>
      <div className="grid grid-cols-[20px_minmax(0,1fr)_auto_auto] items-center gap-x-2.5">
        <AppIcon exe={app.exe} name={app.name} />
        <span className="truncate text-[13px] text-ink">{app.name}</span>
        <span className={`rounded-full px-1.5 py-px text-[10.5px] font-medium ${v.chip}`} title={v.title}>
          {v.label}
        </span>
        <button
          aria-label={`More options for ${app.name}`}
          onClick={() => setMenu((m) => !m)}
          className="flex size-6 items-center justify-center rounded-md text-ink-3 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-hover hover:text-ink focus-visible:opacity-100"
        >
          <IconDots size={14} />
        </button>
      </div>
      <p className="m-0 mt-0.5 pl-[30px] text-[11.5px] leading-snug text-ink-2">{app.verdict.reason}</p>
      <AnimatePresence>
        {menu && (
          <motion.div
            initial={{ opacity: 0, scale: 0.96, y: -4 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96 }}
            transition={{ duration: 0.12 }}
            className="absolute top-9 right-2 z-20 w-48 origin-top-right rounded-lg border border-line bg-raised p-1 shadow-[var(--shadow)]"
          >
            <MenuItem onClick={() => (setMenu(false), onSnooze(app))}>Snooze suggestions for 24 h</MenuItem>
            <MenuItem onClick={() => (setMenu(false), onExclude(app))}>Never touch this app</MenuItem>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function MenuItem({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button onClick={onClick} className="block w-full rounded-md px-2.5 py-1.5 text-left text-[12px] text-ink hover:bg-hover">
      {children}
    </button>
  );
}
