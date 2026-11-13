import { AnimatePresence, motion } from "framer-motion";
import { memo, useLayoutEffect, useMemo, useRef, useState } from "react";
import { reducedMotion } from "../hooks/useTicker";
import { api } from "../lib/api";
import { formatBytes, formatPct } from "../lib/format";
import type { AppView, SortBy } from "../lib/types";
import { VERDICT } from "../lib/verdict";
import { AppIcon } from "./AppIcon";
import { IconBan, IconShield } from "./icons";
import { Segmented } from "./ui";

interface Props {
  apps: AppView[];
  memTotal: number;
  sortBy: SortBy;
  onSort: (s: SortBy) => void;
  onExclude: (app: AppView) => void;
  limit?: number;
  title: string;
}

/**
 * Reorder animation (FLIP) that costs nothing while the order is stable. Positions are measured
 * only when the visible order or an expanded row changes; moved rows then glide to their new
 * slot with a compositor-only transform animation. A generic layout-animation library measures
 * every row on every render, which at one update per second was the single biggest CPU cost.
 */
function useFlip(listRef: React.RefObject<HTMLUListElement | null>, signature: string) {
  const tops = useRef(new Map<string, number>());
  const last = useRef("");
  useLayoutEffect(() => {
    if (signature === last.current) return;
    last.current = signature;
    const ul = listRef.current;
    if (!ul) return;
    const next = new Map<string, number>();
    for (const li of Array.from(ul.children) as HTMLElement[]) {
      const key = li.dataset.key!;
      const top = li.offsetTop;
      next.set(key, top);
      const prev = tops.current.get(key);
      if (!reducedMotion && prev !== undefined && prev !== top) {
        li.animate([{ transform: `translateY(${prev - top}px)` }, { transform: "none" }], {
          duration: 320,
          easing: "cubic-bezier(0.22, 1, 0.36, 1)",
        });
      }
    }
    tops.current = next;
  });
}

/** 2 % steps: tiny wobbles should not start a new bar transition every second. */
const quantize = (v: number) => Math.round(Math.max(0.01, Math.min(1, v)) * 50) / 50;

/** Re-render a row only when something the user can see has changed. */
function sameRow(a: RowProps, b: RowProps) {
  return (
    a.open === b.open &&
    a.share === b.share &&
    a.hot === b.hot &&
    a.sortBy === b.sortBy &&
    a.app.excluded === b.app.excluded &&
    a.onToggle === b.onToggle &&
    a.app.name === b.app.name &&
    a.app.exe === b.app.exe &&
    formatPct(a.app.cpu) === formatPct(b.app.cpu) &&
    formatBytes(a.app.mem) === formatBytes(b.app.mem) &&
    a.app.procCount === b.app.procCount &&
    a.app.verdict.class === b.app.verdict.class &&
    a.app.verdict.reason === b.app.verdict.reason &&
    (!a.open || a.app.procs === b.app.procs)
  );
}

export function AppList({ apps, memTotal, sortBy, onSort, onExclude, limit, title }: Props) {
  const [open, setOpen] = useState<string | null>(null);
  const openRef = useRef(open);
  openRef.current = open;
  const toggles = useRef(new Map<string, () => void>());
  const toggleFor = (key: string) => {
    let f = toggles.current.get(key);
    if (!f) {
      f = () => {
        const next = openRef.current === key ? null : key;
        setOpen(next);
        api.setDetail(next); // only the expanded row gets per-process detail
      };
      toggles.current.set(key, f);
    }
    return f;
  };
  const [showAll, setShowAll] = useState(false);

  const sorted = useMemo(() => {
    // CPU is sorted in 0.5 % steps (RAM breaks ties) so near-equal apps don't swap every second.
    // The back-end already sends apps sorted by RAM.
    const cpuKey = (a: AppView) => Math.round(a.cpu * 2);
    const list = sortBy === "cpu" ? [...apps].sort((a, b) => cpuKey(b) - cpuKey(a) || b.mem - a.mem) : apps;
    // Hide zero-footprint noise (System Idle etc.) from the list.
    return list.filter((a) => a.mem > 0);
  }, [apps, sortBy]);

  const cap = limit ?? (showAll ? sorted.length : 30);
  const visible = sorted.slice(0, cap);
  const top = sortBy === "cpu" ? Math.max(1, visible[0]?.cpu ?? 1) : Math.max(1, visible[0]?.mem ?? 1);
  const listRef = useRef<HTMLUListElement>(null);
  useFlip(listRef, `${open}|${visible.map((a) => a.key).join("|")}`);

  return (
    <section className="flex flex-col" aria-label={title}>
      <header className="flex items-center justify-between px-4 pb-1.5 pt-3">
        <h2 className="m-0 text-[12px] font-medium text-ink-2">{title}</h2>
        <Segmented
          value={sortBy}
          onChange={onSort}
          options={[
            { value: "ram", label: "RAM" },
            { value: "cpu", label: "CPU" },
          ]}
          label="Sort by"
        />
      </header>
      <ul ref={listRef} className="m-0 list-none px-2 pb-2">
        {visible.map((a) => (
          <li key={a.key} data-key={a.key} className="relative">
            <Row
              app={a}
              open={open === a.key}
              onToggle={toggleFor(a.key)}
              share={quantize(sortBy === "cpu" ? a.cpu / top : a.mem / top)}
              hot={a.cpu >= 25 || a.mem / memTotal >= 0.25}
              sortBy={sortBy}
              onExclude={onExclude}
            />
          </li>
        ))}
      </ul>
      {limit === undefined && sorted.length > 30 && (
        <button
          onClick={() => setShowAll((s) => !s)}
          className="mx-4 mb-3 rounded-md py-1.5 text-[12px] text-ink-2 hover:bg-hover hover:text-ink"
        >
          {showAll ? "Show top 30" : `Show all ${sorted.length} apps`}
        </button>
      )}
    </section>
  );
}

interface RowProps {
  app: AppView;
  open: boolean;
  onToggle: () => void;
  share: number;
  hot: boolean;
  sortBy: SortBy;
  onExclude: (app: AppView) => void;
}

const Row = memo(function Row({ app, open, onToggle, share, hot, sortBy, onExclude }: RowProps) {
  const many = app.procCount > 1;
  const cls = app.verdict.class;
  return (
    <>
      <button
        onClick={onToggle}
        aria-expanded={open}
        className="group relative grid w-full grid-cols-[20px_minmax(0,1fr)_48px_60px] items-center gap-x-2.5 rounded-lg px-2 pb-2.5 pt-2 text-left hover:bg-hover"
      >
        <AppIcon exe={app.exe} name={app.name} />
        <span className="flex min-w-0 items-center gap-1.5">
          <span className="truncate text-[13px] text-ink">{app.name}</span>
          {many && <span className="shrink-0 text-[11px] text-ink-3">{app.procCount}</span>}
          {app.verdict.protected && <IconShield size={12} className="shrink-0 text-ink-3" />}
          {cls !== "keep" && <span className={`size-1.5 shrink-0 rounded-full ${VERDICT[cls].dot}`} title={VERDICT[cls].title} />}
        </span>
        <span className={`text-right text-[12px] ${sortBy === "cpu" ? "text-ink" : "text-ink-2"} ${app.cpu >= 25 ? "text-danger" : ""}`}>
          {formatPct(app.cpu)}
        </span>
        <span className={`text-right text-[12px] ${sortBy === "ram" ? "text-ink" : "text-ink-2"}`}>{formatBytes(app.mem)}</span>
        {/* Proportional bar: transform-only so it animates on the compositor. */}
        <span className="pointer-events-none absolute bottom-1 left-[38px] right-2 h-[2px] overflow-hidden rounded-full bg-track/60">
          <span
            className={`block h-full origin-left rounded-full transition-transform duration-200 ease-out ${hot ? "bg-danger" : "bg-accent/70"}`}
            style={{ transform: `scaleX(${share})` }}
          />
        </span>
      </button>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
            className="overflow-hidden"
          >
            <div className="mb-1 ml-[38px] mr-2 border-l border-line pl-3 pt-1">
              <p className="m-0 mb-1.5 text-[11.5px] leading-snug text-ink-2">{app.verdict.reason}</p>
              {app.procs.slice(0, 12).map((p) => (
                <div key={p.pid} className="grid grid-cols-[minmax(0,1fr)_44px_56px] gap-x-2 py-0.5 text-[11.5px] text-ink-2">
                  <span className="truncate">
                    {p.name} <span className="text-ink-3">{p.pid}</span>
                  </span>
                  <span className="text-right">{formatPct(p.cpu)}</span>
                  <span className="text-right">{formatBytes(p.mem)}</span>
                </div>
              ))}
              {app.procs.length === 0 && <div className="py-0.5 text-[11px] text-ink-3">Loading processes…</div>}
              {app.procCount > 12 && app.procs.length > 0 && (
                <div className="py-0.5 text-[11px] text-ink-3">and {app.procCount - 12} more processes</div>
              )}
              {!app.verdict.protected && (
                <button
                  onClick={() => onExclude(app)}
                  className="mt-1.5 mb-1 flex items-center gap-1.5 rounded-md px-2 py-1 text-[11.5px] text-ink-2 hover:bg-hover hover:text-ink"
                >
                  <IconBan size={12} />
                  {app.excluded ? "Allow suggestions for this app" : "Never touch this app"}
                </button>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}, sameRow);
