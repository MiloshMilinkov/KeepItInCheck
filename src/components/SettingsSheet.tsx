import { motion } from "framer-motion";
import { memo, useEffect, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import { baseName, formatBytes, formatPct } from "../lib/format";
import type { LogEntry, PlatformInfo, Settings, Thresholds } from "../lib/types";
import { OutcomeIcon } from "./CleanupSheet";
import { IconBack, IconClose } from "./icons";
import { NumberField, Segmented, Toggle } from "./ui";

interface Props {
  settings: Settings;
  update: (patch: Partial<Settings> | ((s: Settings) => Settings)) => void;
  info: PlatformInfo | null;
  /** KeepItInCheck's own footprint, shown under About. */
  selfCpu: number | null;
  selfMem: number | null;
  onClose: () => void;
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="border-b border-line px-4 py-3 last:border-b-0">
      <h3 className="m-0 mb-1 text-[12px] font-medium text-ink-2">{title}</h3>
      {children}
    </section>
  );
}

function Row({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <div className="flex items-center justify-between gap-3 py-1.5">
      <span className="min-w-0">
        <span className="block text-[12.5px] text-ink">{label}</span>
        {hint && <span className="block text-[11.5px] leading-snug text-ink-2">{hint}</span>}
      </span>
      <span className="shrink-0">{children}</span>
    </div>
  );
}

/** Turn a key event into "Ctrl+Alt+R" style text. */
function hotkeyFrom(e: React.KeyboardEvent): string | null {
  const k = e.key;
  if (["Control", "Alt", "Shift", "Meta"].includes(k)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
  if (mods.length === 0) return null;
  const key = k.length === 1 ? k.toUpperCase() : e.code.replace(/^Key|^Digit/, "");
  return [...mods, key].join("+");
}

export const SettingsSheet = memo(function SettingsSheet({ settings: s, update, info, selfCpu, selfMem, onClose }: Props) {
  const [log, setLog] = useState<LogEntry[] | null>(null);
  const [newExclusion, setNewExclusion] = useState("");
  const [capturing, setCapturing] = useState(false);
  const t = s.thresholds;
  const setT = (patch: Partial<Thresholds>) => update((x) => ({ ...x, thresholds: { ...x.thresholds, ...patch } }));

  useEffect(() => {
    api.cleanupLog().then(setLog);
  }, []);

  const snoozed = Object.entries(s.snoozed).filter(([, until]) => until * 1000 > Date.now());

  return (
    <motion.div
      role="dialog"
      aria-modal="true"
      aria-label="Settings"
      className="absolute inset-0 z-30 flex flex-col rounded-[14px] bg-bg"
      initial={{ opacity: 0, x: 16 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 16 }}
      transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
    >
      <header className="flex h-11 shrink-0 items-center gap-1 border-b border-line pr-2 pl-2">
        <button onClick={onClose} aria-label="Back" className="flex size-7 items-center justify-center rounded-md text-ink-2 hover:bg-hover hover:text-ink">
          <IconBack />
        </button>
        <h2 className="m-0 flex-1 text-[13px] font-semibold text-ink">
          Settings
        </h2>
      </header>

      <div className="scroll min-h-0 flex-1">
        <Group title="Appearance">
          <Row label="Theme">
            <Segmented
              label="Theme"
              value={s.theme}
              onChange={(theme) => update({ theme })}
              options={[
                { value: "dark", label: "Dark" },
                { value: "light", label: "Light" },
                { value: "system", label: "System" },
              ]}
            />
          </Row>
          <Row label="Backdrop" hint={info?.glassSupported ? "Glass uses Windows 11 Acrylic blur." : "Glass needs Windows 11."}>
            <Segmented
              label="Backdrop"
              value={info?.glassSupported ? s.backdrop : "solid"}
              onChange={(backdrop) => info?.glassSupported && update({ backdrop })}
              options={[
                { value: "glass", label: "Glass" },
                { value: "solid", label: "Solid" },
              ]}
            />
          </Row>
          <Toggle checked={s.compact} onChange={(compact) => update({ compact })} label="Compact mode" hint="Just the two gauges and the top 5 apps." />
        </Group>

        <Group title="Panel">
          <Row label="Dock edge" hint="You can also drag the panel to any screen edge.">
            <Segmented
              label="Dock edge"
              value={s.edge}
              onChange={(edge) => update({ edge })}
              options={[
                { value: "left", label: "Left" },
                { value: "right", label: "Right" },
                { value: "top", label: "Top" },
                { value: "bottom", label: "Bottom" },
              ]}
            />
          </Row>
          {info && info.monitors.length > 1 && (
            <Row label="Monitor">
              <select
                value={s.monitor ?? ""}
                onChange={(e) => update({ monitor: e.target.value || null })}
                className="max-w-[150px] rounded-md bg-track px-2 py-1 text-[12px] text-ink outline-none"
              >
                <option value="">Primary</option>
                {info.monitors.map((m, i) => (
                  <option key={m} value={m}>
                    Display {i + 1}
                  </option>
                ))}
              </select>
            </Row>
          )}
          <Toggle checked={s.autoHide} onChange={(autoHide) => update({ autoHide })} label="Auto-hide" hint="Collapse to a thin strip on the edge when the mouse leaves." />
          <Row label="Show / hide shortcut">
            <button
              onClick={() => setCapturing(true)}
              onBlur={() => setCapturing(false)}
              onKeyDown={(e) => {
                if (!capturing) return;
                e.preventDefault();
                if (e.key === "Escape") return setCapturing(false);
                const hk = hotkeyFrom(e);
                if (hk) {
                  update({ hotkey: hk });
                  setCapturing(false);
                }
              }}
              className={`min-w-[96px] rounded-md px-2 py-1 text-[12px] ${capturing ? "bg-accent-soft text-accent" : "bg-track text-ink"}`}
            >
              {capturing ? "Press keys…" : s.hotkey}
            </button>
          </Row>
          <Toggle checked={s.launchAtStartup} onChange={(launchAtStartup) => update({ launchAtStartup })} label="Launch at startup" hint="Starts collapsed on the edge." />
        </Group>

        <Group title="Monitoring">
          <Row label="Refresh while open">
            <Segmented
              label="Refresh while open"
              value={String(s.refreshVisibleMs)}
              onChange={(v) => update({ refreshVisibleMs: Number(v) })}
              options={[
                { value: "500", label: "0.5 s" },
                { value: "1000", label: "1 s" },
                { value: "2000", label: "2 s" },
              ]}
            />
          </Row>
          <Row label="Refresh while hidden">
            <Segmented
              label="Refresh while hidden"
              value={String(s.refreshHiddenMs)}
              onChange={(v) => update({ refreshHiddenMs: Number(v) })}
              options={[
                { value: "5000", label: "5 s" },
                { value: "10000", label: "10 s" },
                { value: "30000", label: "30 s" },
              ]}
            />
          </Row>
        </Group>

        <Group title="Dead-weight thresholds">
          <NumberField label="Idle CPU below" value={t.idleCpuPct} min={0.05} max={10} step={0.05} unit="%" onChange={(idleCpuPct) => setT({ idleCpuPct })} />
          <NumberField label="Averaged over" value={t.idleWindowMins} min={2} max={120} unit="min" onChange={(idleWindowMins) => setT({ idleWindowMins })} />
          <NumberField label="Window unused for" value={t.unfocusedMins} min={1} max={600} unit="min" onChange={(unfocusedMins) => setT({ unfocusedMins })} />
          <NumberField label="Ignore apps under" value={t.minRamMb} min={1} max={8192} unit="MB" onChange={(minRamMb) => setT({ minRamMb })} />
          <NumberField label="Negligible I/O below" value={t.maxIoKbps} min={0} max={100000} unit="KB/s" onChange={(maxIoKbps) => setT({ maxIoKbps })} />
          <NumberField label="Safe-to-close score" value={t.safeScore} min={1} max={100} unit="/100" onChange={(safeScore) => setT({ safeScore })} />
          {/* An empty object lets the back-end fill in its own defaults: one source of truth. */}
          <button onClick={() => update((x) => ({ ...x, thresholds: {} as Thresholds }))} className="mt-1 rounded-md px-2 py-1 text-[11.5px] text-ink-2 hover:bg-hover hover:text-ink">
            Reset to defaults
          </button>
        </Group>

        <Group title="Safety">
          <Toggle checked={s.dryRun} onChange={(dryRun) => update({ dryRun })} label="Dry run by default" hint="Clean up shows what would happen and closes nothing." />
          <Toggle
            checked={s.protectAllMicrosoft}
            onChange={(protectAllMicrosoft) => update({ protectAllMicrosoft })}
            label="Protect all Microsoft-signed apps"
            hint="Off: only Microsoft components inside the Windows folder are protected."
          />
          <div className="pt-2">
            <span className="block text-[12.5px] text-ink">Never touch</span>
            <span className="block text-[11.5px] text-ink-2">Exe names or full paths that are never suggested or closed.</span>
            <ul className="m-0 mt-1.5 list-none p-0">
              {s.exclusions.length === 0 && <li className="py-1 text-[11.5px] text-ink-3">No apps excluded yet.</li>}
              {s.exclusions.map((e) => (
                <li key={e} className="flex items-center justify-between gap-2 rounded-md py-0.5 pr-0.5 pl-2 hover:bg-hover">
                  <span className="truncate text-[12px] text-ink">{e}</span>
                  <button
                    aria-label={`Remove ${e}`}
                    onClick={() => update((x) => ({ ...x, exclusions: x.exclusions.filter((y) => y !== e) }))}
                    className="flex size-6 items-center justify-center rounded-md text-ink-3 hover:text-ink"
                  >
                    <IconClose size={12} />
                  </button>
                </li>
              ))}
            </ul>
            <form
              className="mt-1.5 flex gap-1.5"
              onSubmit={(ev) => {
                ev.preventDefault();
                const v = newExclusion.trim().toLowerCase();
                if (v && !s.exclusions.includes(v)) update((x) => ({ ...x, exclusions: [...x.exclusions, v] }));
                setNewExclusion("");
              }}
            >
              <input
                value={newExclusion}
                onChange={(e) => setNewExclusion(e.target.value)}
                placeholder="e.g. spotify.exe"
                className="min-w-0 flex-1 rounded-md bg-track px-2 py-1 text-[12px] text-ink outline-none placeholder:text-ink-3"
              />
              <button className="rounded-md bg-track px-2.5 py-1 text-[12px] text-ink hover:bg-hover">Add</button>
            </form>
          </div>
          {snoozed.length > 0 && (
            <div className="pt-3">
              <span className="block text-[12.5px] text-ink">Snoozed</span>
              <ul className="m-0 mt-1 list-none p-0">
                {snoozed.map(([key, until]) => (
                  <li key={key} className="flex items-center justify-between gap-2 py-0.5 pl-2">
                    <span className="truncate text-[12px] text-ink">{baseName(key)}</span>
                    <span className="shrink-0 text-[11px] text-ink-3">until {new Date(until * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
                    <button
                      aria-label="Unsnooze"
                      onClick={() =>
                        update((x) => {
                          const n = { ...x.snoozed };
                          delete n[key];
                          return { ...x, snoozed: n };
                        })
                      }
                      className="flex size-6 items-center justify-center rounded-md text-ink-3 hover:text-ink"
                    >
                      <IconClose size={12} />
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </Group>

        <Group title="Cleanup history">
          {log === null ? null : log.length === 0 ? (
            <p className="m-0 py-1 text-[11.5px] text-ink-3">Nothing closed yet.</p>
          ) : (
            <ul className="m-0 list-none p-0">
              {log.slice(0, 30).map((e, i) => (
                <li key={i} className="grid grid-cols-[14px_minmax(0,1fr)_auto] items-center gap-x-2 py-1">
                  <OutcomeIcon outcome={e.outcome} />
                  <span className="truncate text-[12px] text-ink">{e.name}</span>
                  <span className="text-[11px] text-ink-3">
                    {new Date(e.at * 1000).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" })}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Group>

        <Group title="About">
          <p className="m-0 text-[12px] leading-relaxed text-ink-2">
            KeepItInCheck {info?.version ?? ""}. Everything stays on this PC: no network, no telemetry.
            {selfCpu !== null && selfMem !== null && (
              <>
                {" "}
                Right now it uses {formatPct(selfCpu)} CPU and {formatBytes(selfMem)} RAM.
              </>
            )}
          </p>
          <button onClick={() => api.quit()} className="mt-2 rounded-md px-2 py-1 text-[12px] text-danger hover:bg-danger-soft">
            Quit KeepItInCheck
          </button>
        </Group>
      </div>
    </motion.div>
  );
});
