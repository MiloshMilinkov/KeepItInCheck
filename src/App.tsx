import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppList } from "./components/AppList";
import { CleanupSheet } from "./components/CleanupSheet";
import { DeadWeight } from "./components/DeadWeight";
import { EdgeStrip } from "./components/EdgeStrip";
import { Gauge } from "./components/Gauge";
import { ReportSheet } from "./components/ReportSheet";
import { SettingsSheet } from "./components/SettingsSheet";
import { StartupSheet } from "./components/StartupSheet";
import { TitleBar } from "./components/TitleBar";
import { useSettings } from "./hooks/useSettings";
import { useSnapshot } from "./hooks/useSnapshot";
import { api, native, on } from "./lib/api";
import { baseName, formatBytes, formatPct } from "./lib/format";
import type { AppView, Edge, PlatformInfo, Snapshot, SortBy } from "./lib/types";

type Phase = "starting" | "open" | "closing" | "closed";
type Sheet = "cleanup" | "settings" | "startup" | "report" | null;

/** Where Back / Escape go from each sheet (null closes it). */
const PARENT: Record<Exclude<Sheet, null>, Sheet> = { cleanup: null, settings: null, startup: "settings", report: "settings" };

/** Offset the panel slides in from, towards its docked edge. */
function slide(edge: Edge) {
  const d = 28;
  return edge === "left" ? { x: -d } : edge === "right" ? { x: d } : edge === "top" ? { y: -d } : { y: d };
}

function useWide() {
  const [wide, setWide] = useState(() => window.innerWidth > 600);
  useEffect(() => {
    const f = () => setWide(window.innerWidth > 600);
    window.addEventListener("resize", f);
    return () => window.removeEventListener("resize", f);
  }, []);
  return wide;
}

export default function App() {
  const snap = useSnapshot();
  const [settings, update] = useSettings();
  const [info, setInfo] = useState<PlatformInfo | null>(null);
  const [phase, setPhase] = useState<Phase>("starting");
  const [sheet, setSheet] = useState<Sheet>(null);
  const [paused, setPaused] = useState(false);
  const wide = useWide();
  const phaseRef = useRef(phase);
  phaseRef.current = phase;

  useEffect(() => {
    api.platformInfo().then((i) => {
      setInfo(i);
      setPhase(i.expanded ? "open" : "closed");
    });
    return on("paused", setPaused);
  }, []);

  // Theme + glass attributes on <html>.
  useEffect(() => {
    if (!settings) return;
    const root = document.documentElement;
    const apply = () => {
      const dark = settings.theme === "system" ? matchMedia("(prefers-color-scheme: dark)").matches : settings.theme === "dark";
      root.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    const mq = matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [settings?.theme]); // eslint-disable-line react-hooks/exhaustive-deps

  // Glass needs both the setting and Windows 11 (the back-end applies the same rule).
  const glass = !!settings && !!info && native && settings.backdrop === "glass" && info.glassSupported;
  useEffect(() => {
    document.documentElement.dataset.glass = String(glass);
    if (!native) document.documentElement.dataset.preview = "";
  }, [glass]);

  // ---- expand / collapse state machine -------------------------------------------------------
  const open = useCallback(async (focus: boolean) => {
    if (phaseRef.current !== "closed") return;
    await api.setExpanded(true, focus);
    setPhase("open");
  }, []);

  const close = useCallback(() => {
    if (phaseRef.current !== "open") return;
    setSheet(null);
    setPhase("closing"); // exit animation runs, then finishClose shrinks the window
  }, []);

  const finishClose = useCallback(async () => {
    await api.setExpanded(false, false);
    setPhase("closed");
  }, []);

  useEffect(() => {
    const a = on("toggle", (t) => {
      const want = t.expand ?? phaseRef.current === "closed";
      if (want) open(t.focus);
      else close();
    });
    const b = on("open-cleanup", async () => {
      await open(true);
      setSheet("cleanup");
    });
    return () => {
      a();
      b();
    };
  }, [open, close]);

  // Auto-hide: the native side watches the cursor and asks; we decline while a sheet is open.
  const locked = sheet !== null || !settings?.autoHide;
  const lockedRef = useRef(locked);
  lockedRef.current = locked;
  useEffect(
    () =>
      on("auto-hide", () => {
        if (!lockedRef.current) close();
      }),
    [close],
  );

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (sheet) setSheet(PARENT[sheet]);
      else if (settings?.autoHide) close();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [sheet, settings?.autoHide, close]);

  // ---- actions ------------------------------------------------------------------------------
  // The back-end decides `app.excluded`; un-excluding removes both the name and full-path forms.
  const toggleExclude = useCallback(
    (a: AppView) =>
      update((s) => {
        const name = baseName(a.exe ?? a.key);
        const path = (a.exe ?? "").toLowerCase();
        const exclusions = a.excluded ? s.exclusions.filter((x) => x !== name && x !== path) : [...s.exclusions, name];
        return { ...s, exclusions };
      }),
    [update],
  );
  const snooze = useCallback(
    (a: AppView) => update((s) => ({ ...s, snoozed: { ...s.snoozed, [a.key]: Math.floor(Date.now() / 1000) + 24 * 3600 } })),
    [update],
  );
  // The back-end owns the paused state and echoes it back through the `paused` event.
  const togglePause = useCallback(() => api.setPaused(!paused), [paused]);
  const closeSheet = useCallback(() => setSheet(null), []);
  const back = useCallback(() => setSheet((s) => (s ? PARENT[s] : null)), []);
  const onSort = useCallback((sortBy: SortBy) => update({ sortBy }), [update]);
  const onCleanUp = useCallback(() => setSheet("cleanup"), []);

  if (!settings || phase === "starting") return null;
  const edge = settings.edge;

  // In a plain browser, mimic the native window geometry so the design can be reviewed.
  const previewStyle: React.CSSProperties | undefined = native
    ? undefined
    : phase === "closed"
      ? { position: "fixed", right: 0, top: "35%", height: "30%", width: 6 }
      : { position: "fixed", right: 8, top: 8, bottom: 8, width: wide && !settings.compact ? 780 : 360, height: settings.compact ? 492 : undefined };

  return (
    <div className="h-full w-full" style={previewStyle}>
      {phase === "closed" ? (
        <EdgeStrip edge={edge} onOpen={() => open(false)} />
      ) : (
        <AnimatePresence onExitComplete={finishClose}>
          {phase === "open" && (
            <motion.main
              key="panel"
              className="panel flex h-full w-full flex-col overflow-hidden"
              initial={{ opacity: 0, ...slide(edge) }}
              animate={{ opacity: 1, x: 0, y: 0 }}
              exit={{ opacity: 0, ...slide(edge) }}
              transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            >
              <TitleBar
                edge={edge}
                paused={paused}
                compact={settings.compact}
                onPause={togglePause}
                onCompact={() => update({ compact: !settings.compact })}
                onSettings={() => setSheet("settings")}
                onHide={close}
              />
              {paused && (
                <div className="mx-3 mb-1 flex items-center justify-between rounded-lg bg-amber-soft px-3 py-1.5 text-[12px] text-amber">
                  Monitoring paused. Numbers are frozen.
                  <button onClick={togglePause} className="rounded-md px-1.5 py-0.5 font-medium hover:bg-hover">
                    Resume
                  </button>
                </div>
              )}
              {snap ? (
                <Body
                  snap={snap}
                  compact={settings.compact}
                  wide={wide}
                  sortBy={settings.sortBy}
                  onSort={onSort}
                  onCleanUp={onCleanUp}
                  onSnooze={snooze}
                  onExclude={toggleExclude}
                />
              ) : (
                <div className="flex flex-1 items-center justify-center text-[12px] text-ink-3">Reading processes…</div>
              )}
              {snap && <Footer snap={snap} />}
              <AnimatePresence>
                {sheet === "cleanup" && snap && <CleanupSheet key="cleanup" apps={snap.apps} dryRunDefault={settings.dryRun} onClose={closeSheet} />}
                {sheet === "settings" && (
                  <SettingsSheet
                    key="settings"
                    settings={settings}
                    update={update}
                    info={info}
                    selfCpu={snap ? Math.round(snap.selfCpu * 10) / 10 : null}
                    selfMem={snap ? Math.round(snap.selfMem / 1048576) * 1048576 : null}
                    onClose={closeSheet}
                    onOpenTool={setSheet}
                  />
                )}
                {sheet === "startup" && <StartupSheet key="startup" onBack={back} />}
                {sheet === "report" && <ReportSheet key="report" onBack={back} />}
              </AnimatePresence>
            </motion.main>
          )}
        </AnimatePresence>
      )}
    </div>
  );
}

interface BodyProps {
  snap: Snapshot;
  compact: boolean;
  wide: boolean;
  sortBy: SortBy;
  onSort: (s: SortBy) => void;
  onCleanUp: () => void;
  onSnooze: (a: AppView) => void;
  onExclude: (a: AppView) => void;
}

function Gauges({ snap }: { snap: Snapshot }) {
  const memPct = snap.memTotal ? (snap.memUsed / snap.memTotal) * 100 : 0;
  const cpuHist = useMemo(() => snap.history.map(([t, c]) => [t, c] as [number, number]), [snap.history]);
  const memHist = useMemo(() => snap.history.map(([t, , m]) => [t, m] as [number, number]), [snap.history]);
  return (
    <div className="flex gap-5 px-5 pt-1 pb-4">
      <Gauge label="CPU" value={snap.cpu} detail={`${snap.cores} threads`} history={cpuHist} />
      <Gauge label="RAM" value={memPct} detail={`${formatBytes(snap.memUsed)} of ${formatBytes(snap.memTotal)}`} history={memHist} />
    </div>
  );
}

function Body({ snap, compact, wide, sortBy, onSort, onCleanUp, onSnooze, onExclude }: BodyProps) {
  const list = (limit?: number) => (
    <AppList
      apps={snap.apps}
      memTotal={snap.memTotal}
      sortBy={sortBy}
      onSort={onSort}
      onExclude={onExclude}
      limit={limit}
      title={limit ? `Top ${limit}` : "Top consumers"}
    />
  );
  const dead = <DeadWeight snap={snap} onCleanUp={onCleanUp} onSnooze={onSnooze} onExclude={onExclude} />;

  if (compact) {
    return (
      <div className="scroll flex min-h-0 flex-1 flex-col">
        <Gauges snap={snap} />
        <div className="mx-4 h-px bg-line" />
        {list(5)}
      </div>
    );
  }
  if (wide) {
    return (
      <div className="grid min-h-0 flex-1 grid-cols-[330px_minmax(0,1fr)]">
        <div className="scroll min-h-0 border-r border-line">
          <Gauges snap={snap} />
          {dead}
        </div>
        <div className="scroll min-h-0">{list()}</div>
      </div>
    );
  }
  return (
    <>
      <Gauges snap={snap} />
      <div className="scroll min-h-0 flex-1 border-t border-line pt-3">
        {dead}
        <div className="mx-4 mt-3 h-px bg-line" />
        {list()}
      </div>
    </>
  );
}

function Footer({ snap }: { snap: Snapshot }) {
  return (
    <footer className="flex h-8 shrink-0 items-center justify-between border-t border-line px-4 text-[11px] text-ink-3">
      <span title="KeepItInCheck's own footprint, including its WebView2 helpers">
        This app: {formatPct(snap.selfCpu)} CPU, {formatBytes(snap.selfMem)}
      </span>
      <span>{snap.processCount} processes</span>
    </footer>
  );
}
