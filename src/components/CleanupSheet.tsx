import { AnimatePresence, motion } from "framer-motion";
import { memo, useEffect, useMemo, useState } from "react";
import { api, on } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { AppView, LogEntry, Report } from "../lib/types";
import { isClosed, VERDICT } from "../lib/verdict";
import { AppIcon } from "./AppIcon";
import { flaggedApps } from "./DeadWeight";
import { IconCheck, IconClose, IconRelaunch, IconWarn } from "./icons";
import { Checkbox, Toggle } from "./ui";

interface Props {
  apps: AppView[];
  dryRunDefault: boolean;
  onClose: () => void;
}

type Phase = { kind: "preview" } | { kind: "running" } | { kind: "done"; report: Report } | { kind: "error"; message: string };

/**
 * Clean-up preview -> confirmation -> result log. Nothing closes until the user presses the
 * close button. Only "Safe" apps start checked.
 */
export const CleanupSheet = memo(
  function CleanupSheet({ apps, dryRunDefault, onClose }: Props) {
  // Freeze the list when the sheet opens so rows do not jump while the user decides.
  const [items] = useState(() => flaggedApps(apps));
  const [checked, setChecked] = useState<Set<string>>(() => new Set(items.filter((a) => a.verdict.class === "safe").map((a) => a.key)));
  const [dryRun, setDryRun] = useState(dryRunDefault);
  const [phase, setPhase] = useState<Phase>({ kind: "preview" });
  const [progress, setProgress] = useState<Record<string, "closing" | "done">>({});

  useEffect(() => on("cleanup-progress", (p) => setProgress((s) => ({ ...s, [p.key]: p.stage }))), []);

  const selected = items.filter((a) => checked.has(a.key));
  const total = selected.reduce((s, a) => s + a.mem, 0);

  const start = async () => {
    setPhase({ kind: "running" });
    try {
      const report = await api.runCleanup(
        selected.map((a) => a.key),
        dryRun,
      );
      setPhase({ kind: "done", report });
    } catch (e) {
      setPhase({ kind: "error", message: String(e) });
    }
  };

  const toggle = (key: string, v: boolean) =>
    setChecked((s) => {
      const n = new Set(s);
      if (v) n.add(key);
      else n.delete(key);
      return n;
    });

  return (
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-label="Clean up"
      className="absolute inset-0 z-30 flex flex-col"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.18 }}
    >
      <div className="absolute inset-0 bg-bg/60 backdrop-blur-[2px]" onClick={phase.kind === "running" ? undefined : onClose} />
      <motion.div
        className="relative mt-auto flex max-h-[88%] min-h-0 flex-col rounded-t-[14px] border-t border-line bg-raised shadow-[var(--shadow)]"
        initial={{ y: 40 }}
        animate={{ y: 0 }}
        exit={{ y: 40 }}
        transition={{ type: "spring", stiffness: 420, damping: 38 }}
      >
        {phase.kind === "preview" && (
          <>
            <header className="px-4 pt-4 pb-2">
              <h2 className="m-0 text-[15px] font-semibold text-ink">Clean up</h2>
              <p className="m-0 mt-1 text-[12px] leading-snug text-ink-2">
                Checked apps get a normal close request. Apps with a window are never force-closed.
              </p>
            </header>
            <ul className="scroll m-0 min-h-0 flex-1 list-none px-2 py-1">
              {items.map((a) => {
                const v = VERDICT[a.verdict.class === "safe" ? "safe" : "probable"];
                return (
                  <li key={a.key}>
                    <label className="grid cursor-default grid-cols-[16px_20px_minmax(0,1fr)_auto] items-start gap-x-2.5 rounded-lg px-2 py-2 hover:bg-hover">
                      <span className="pt-0.5">
                        <Checkbox checked={checked.has(a.key)} onChange={(v) => toggle(a.key, v)} label={`Close ${a.name}`} tone={v.tone} />
                      </span>
                      <AppIcon exe={a.exe} name={a.name} />
                      <span className="min-w-0">
                        <span className="flex items-center gap-1.5">
                          <span className="truncate text-[13px] text-ink">{a.name}</span>
                          {v.tone === "amber" && <span className={`shrink-0 rounded-full px-1.5 py-px text-[10.5px] font-medium ${v.chip}`}>{v.label}</span>}
                        </span>
                        <span className="mt-0.5 block text-[11.5px] leading-snug text-ink-2">{a.verdict.reason}</span>
                      </span>
                      <span className="pt-0.5 text-[12px] text-ink">{formatBytes(a.mem)}</span>
                    </label>
                  </li>
                );
              })}
            </ul>
            <footer className="border-t border-line px-4 pt-1 pb-4">
              <Toggle checked={dryRun} onChange={setDryRun} label="Dry run" hint="Show what would happen without closing anything." />
              <div className="mt-2 flex items-center gap-2">
                <button onClick={onClose} className="rounded-lg px-3 py-2 text-[13px] text-ink-2 hover:bg-hover hover:text-ink">
                  Cancel
                </button>
                <button
                  onClick={start}
                  disabled={selected.length === 0}
                  className="flex-1 rounded-lg bg-accent py-2 text-[13px] font-semibold text-accent-ink hover:brightness-110 disabled:bg-track disabled:text-ink-3"
                >
                  {selected.length === 0
                    ? "Select apps to close"
                    : dryRun
                      ? `Simulate closing ${selected.length}`
                      : `Close ${selected.length} ${selected.length === 1 ? "app" : "apps"} (${formatBytes(total)})`}
                </button>
              </div>
            </footer>
          </>
        )}

        {phase.kind === "running" && (
          <div className="px-4 py-5">
            <h2 className="m-0 text-[15px] font-semibold text-ink">{dryRun ? "Simulating…" : "Closing apps…"}</h2>
            <p className="m-0 mt-1 text-[12px] text-ink-2">Waiting up to 5 seconds for each app to close on its own.</p>
            <ul className="m-0 mt-3 list-none p-0">
              {selected.map((a) => (
                <li key={a.key} className="flex items-center gap-2.5 py-1.5">
                  <AppIcon exe={a.exe} name={a.name} />
                  <span className="flex-1 truncate text-[13px] text-ink">{a.name}</span>
                  {progress[a.key] === "done" ? <IconCheck className="text-accent" /> : <Spinner />}
                </li>
              ))}
            </ul>
          </div>
        )}

        {phase.kind === "done" && <Result report={phase.report} onClose={onClose} />}

        {phase.kind === "error" && (
          <div className="px-4 py-5">
            <h2 className="m-0 text-[15px] font-semibold text-ink">Clean up didn't run</h2>
            <p className="m-0 mt-1 text-[12px] text-ink-2">{phase.message}</p>
            <button onClick={onClose} className="mt-4 w-full rounded-lg bg-track py-2 text-[13px] text-ink hover:bg-hover">
              Close
            </button>
          </div>
        )}
      </motion.div>
    </motion.div>
  );
},
  // The list is frozen when the sheet opens, so new snapshots (new `apps`) must not re-render it.
  (a, b) => a.dryRunDefault === b.dryRunDefault && a.onClose === b.onClose,
);

function Spinner() {
  return (
    <span aria-label="In progress" className="size-3.5 animate-spin rounded-full border-[1.5px] border-track border-t-accent" />
  );
}

const outcomeLabel: Record<LogEntry["outcome"], string> = {
  closed: "Closed",
  forceKilled: "Ended",
  stillRunning: "Still running",
  skipped: "Skipped",
  failed: "Failed",
  wouldClose: "Would close",
};

export function OutcomeIcon({ outcome }: { outcome: LogEntry["outcome"] }) {
  if (isClosed(outcome) || outcome === "wouldClose") return <IconCheck size={14} className="text-accent" />;
  if (outcome === "stillRunning") return <IconWarn size={14} className="text-amber" />;
  return <IconClose size={14} className="text-ink-3" />;
}

function Result({ report, onClose }: { report: Report; onClose: () => void }) {
  const [entries, setEntries] = useState(report.entries);
  const freed = report.dryRun ? report.estimated : Math.max(report.freed, 0);
  const closedCount = useMemo(() => entries.filter((e) => isClosed(e.outcome)).length, [entries]);
  return (
    <>
      <header className="px-4 pt-4 pb-2">
        <p className="m-0 text-[12px] text-ink-2">{report.dryRun ? "Dry run: nothing was closed" : `${closedCount} closed`}</p>
        <h2 className="m-0 mt-0.5 text-[24px] font-semibold tracking-[-0.02em] text-ink">
          {report.dryRun ? `Would free ~${formatBytes(freed)}` : `Freed ${formatBytes(freed)}`}
        </h2>
        {!report.dryRun && report.estimated > 0 && (
          <p className="m-0 mt-0.5 text-[11.5px] text-ink-3">Measured from available RAM. The closed apps held {formatBytes(report.estimated)}.</p>
        )}
      </header>
      <ul className="scroll m-0 min-h-0 flex-1 list-none px-2 py-1">
        {entries.map((e, i) => (
          <ResultRow key={e.key + i} entry={e} onUpdate={(n) => setEntries((list) => list.map((x, j) => (j === i ? n : x)))} />
        ))}
      </ul>
      <footer className="border-t border-line px-4 py-3">
        <button onClick={onClose} className="w-full rounded-lg bg-track py-2 text-[13px] font-medium text-ink hover:bg-hover">
          Done
        </button>
      </footer>
    </>
  );
}

function ResultRow({ entry, onUpdate }: { entry: LogEntry; onUpdate: (e: LogEntry) => void }) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const canRelaunch = !entry.dryRun && !!entry.exe && isClosed(entry.outcome);
  const canForce = !entry.dryRun && entry.outcome === "stillRunning";

  const relaunch = async () => {
    setBusy(true);
    try {
      await api.relaunch(entry.exe!);
      setNote("Relaunched");
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  const force = async () => {
    setBusy(true);
    try {
      onUpdate(await api.forceClose(entry.key));
      setConfirm(false);
    } catch (e) {
      setNote(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className="rounded-lg px-2 py-2">
      <div className="grid grid-cols-[14px_20px_minmax(0,1fr)_auto] items-center gap-x-2.5">
        <OutcomeIcon outcome={entry.outcome} />
        <AppIcon exe={entry.exe} name={entry.name} />
        <span className="truncate text-[13px] text-ink">{entry.name}</span>
        <span className="text-[11.5px] text-ink-2">{outcomeLabel[entry.outcome]}</span>
      </div>
      <p className="m-0 mt-0.5 pl-[54px] text-[11.5px] leading-snug text-ink-2">{note ?? entry.detail}</p>
      {(canRelaunch || canForce) && !note && (
        <div className="mt-1.5 flex gap-1.5 pl-[54px]">
          {canRelaunch && (
            <button disabled={busy} onClick={relaunch} className="flex items-center gap-1.5 rounded-md bg-track px-2 py-1 text-[11.5px] text-ink hover:bg-hover">
              <IconRelaunch size={12} /> Relaunch
            </button>
          )}
          {canForce && !confirm && (
            <button onClick={() => setConfirm(true)} className="rounded-md px-2 py-1 text-[11.5px] text-danger hover:bg-danger-soft">
              Force close…
            </button>
          )}
        </div>
      )}
      <AnimatePresence>
        {confirm && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="overflow-hidden pl-[54px]"
          >
            <div className="mt-2 rounded-lg border border-danger/40 bg-danger-soft p-2.5">
              <p className="m-0 text-[12px] leading-snug text-ink">
                Force close {entry.name}? Anything unsaved in its windows will be lost.
              </p>
              <div className="mt-2 flex gap-1.5">
                <button onClick={() => setConfirm(false)} className="rounded-md px-2 py-1 text-[11.5px] whitespace-nowrap text-ink-2 hover:bg-hover">
                  Keep running
                </button>
                <button disabled={busy} onClick={force} className="rounded-md bg-danger px-2 py-1 text-[11.5px] font-semibold whitespace-nowrap text-white">
                  Force close
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </li>
  );
}
