// Browser-only stand-in for the Rust back-end. Never shipped in the Tauri build's code path
// (api.ts only calls it when `isTauri()` is false).

import type { AppView, Class, LogEntry, Report, Settings, Snapshot } from "./types";

const MB = 1024 * 1024;
const GB = 1024 * MB;

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();

export function on(event: string, cb: Listener) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(cb);
  return () => listeners.get(event)?.delete(cb);
}

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((cb) => cb(payload));
}

let settings: Settings = {
  refreshVisibleMs: 1000,
  refreshHiddenMs: 5000,
  edge: "right",
  monitor: null,
  autoHide: false,
  compact: false,
  theme: "dark",
  backdrop: "solid",
  hotkey: "Ctrl+Alt+R",
  launchAtStartup: false,
  dryRun: false,
  protectAllMicrosoft: true,
  thresholds: { idleCpuPct: 0.3, idleWindowMins: 10, unfocusedMins: 30, minRamMb: 50, maxIoKbps: 20, safeScore: 65 },
  exclusions: [],
  snoozed: {},
  sortBy: "ram",
};

const seed: [string, string, number, number, Class, string, boolean, number][] = [
  ["Google Chrome", "chrome.exe", 1.9 * GB, 4.2, "keep", "In use: focused 1 min ago", true, 23],
  ["Visual Studio Code", "Code.exe", 1.1 * GB, 2.6, "keep", "In use: focused 4 min ago", true, 11],
  ["Discord", "Discord.exe", 612 * MB, 0.02, "safe", "Idle for 47 min, no window, using 612 MB", false, 6],
  ["Microsoft Teams", "ms-teams.exe", 540 * MB, 0.1, "keep", "Protected: Microsoft-signed component", true, 5],
  ["Spotify", "Spotify.exe", 318 * MB, 0.0, "probable", "Idle for 52 min, window unused for 52 min, using 318 MB", true, 5],
  ["Steam", "steam.exe", 287 * MB, 0.05, "probable", "Idle for 31 min, no window, using 287 MB · may sync or work in the background", false, 4],
  ["Figma Agent", "figma_agent.exe", 96 * MB, 0.01, "safe", "Idle for 2 h 5 min, no window, using 96 MB", false, 1],
  ["Windows Explorer", "explorer.exe", 210 * MB, 0.3, "keep", "Protected: core Windows process", true, 1],
  ["Slack", "slack.exe", 402 * MB, 0.9, "keep", "Active: 0.9% CPU on average", true, 5],
  ["Desktop Window Manager", "dwm.exe", 140 * MB, 1.6, "keep", "Protected: core Windows process", false, 1],
  ["Node.js", "node.exe", 260 * MB, 3.1, "keep", "Active: 3.1% CPU on average", false, 2],
  ["Adobe Creative Cloud", "Creative Cloud.exe", 188 * MB, 0.0, "safe", "Idle for 1 h 12 min, no window, using 188 MB", false, 3],
  ["OneDrive", "OneDrive.exe", 74 * MB, 0.02, "keep", "Protected: Microsoft-signed component", false, 1],
  ["Antimalware Service", "MsMpEng.exe", 230 * MB, 0.4, "keep", "Protected: security software", false, 1],
];

const t0 = Date.now();
const history: [number, number, number][] = [];

function jitter(v: number, amt: number) {
  return Math.max(0, v + (Math.random() - 0.5) * amt);
}

function snapshot(): Snapshot {
  const apps: AppView[] = seed.map(([name, exe, mem, cpu, cls, reason, win, n], i) => ({
    key: `c:\\apps\\${exe}`.toLowerCase(),
    name,
    exe: `C:\\Apps\\${exe}`,
    cpu: cls === "keep" ? jitter(cpu, cpu * 0.8) : cpu,
    mem: jitter(mem, mem * 0.02),
    hasWindow: win,
    excluded: settings.exclusions.includes(exe.toLowerCase()),
    procCount: n,
    procs: Array.from({ length: n }, (_, j) => ({
      pid: 1000 + i * 40 + j,
      name: exe,
      cpu: cpu / n,
      mem: mem / n / (j + 1),
    })),
    verdict: { class: cls, score: cls === "safe" ? 88 : cls === "probable" ? 61 : 0, reason, protected: reason.startsWith("Protected") },
  }));
  const cpu = jitter(18, 14);
  const memTotal = 32 * GB;
  const memUsed = jitter(19.4 * GB, 0.3 * GB);
  history.push([Date.now(), cpu, (memUsed / memTotal) * 100]);
  while (history.length > 60) history.shift();
  const flagged = apps.filter((a) => a.verdict.class !== "keep");
  return {
    ts: Date.now(),
    cpu,
    memUsed,
    memTotal,
    cores: 16,
    processCount: 312,
    history: history.map(([t, c, m]) => [(Date.now() - t) / 1000, c, m]),
    apps,
    reclaimable: flagged.reduce((s, a) => s + a.mem, 0),
    observedSecs: (Date.now() - t0) / 1000 + 3600,
    learningSecs: 600,
    selfCpu: 0.08,
    selfMem: 41 * MB,
  };
}

let latest = snapshot();
setInterval(() => {
  latest = snapshot();
  emit("snapshot", latest);
}, 1000);

const log: LogEntry[] = [];

export async function invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "get_snapshot":
      return latest;
    case "get_settings":
      return settings;
    case "save_settings":
      settings = args!.settings as Settings;
      emit("settings-changed", settings);
      return settings;
    case "platform_info":
      return { glassSupported: true, monitors: ["\\\\.\\DISPLAY1", "\\\\.\\DISPLAY2"], version: "0.1.0", expanded: true };
    case "app_icon":
      return null;
    case "cleanup_log":
      return [...log].reverse();
    case "run_cleanup": {
      const keys = args!.keys as string[];
      const dryRun = args!.dryRun as boolean;
      await new Promise((r) => setTimeout(r, dryRun ? 300 : 2200));
      const entries: LogEntry[] = keys.map((k) => {
        const a = latest.apps.find((x) => x.key === k)!;
        const stuck = a.hasWindow && !dryRun;
        return {
          key: k,
          name: a.name,
          exe: a.exe,
          mem: a.mem,
          outcome: dryRun ? "wouldClose" : stuck ? "stillRunning" : "closed",
          detail: dryRun
            ? "Would ask it to close, then end it after 5 s if it doesn't respond."
            : stuck
              ? "Didn't close within 5 s. It may be asking to save your work, so it was left running."
              : "Closed gracefully",
          dryRun,
          at: Date.now() / 1000,
        };
      });
      if (!dryRun) log.push(...entries);
      const estimated = entries.filter((e) => e.outcome === "closed" || e.outcome === "wouldClose").reduce((s, e) => s + e.mem, 0);
      return { entries, freed: dryRun ? 0 : estimated * 0.92, estimated, dryRun } satisfies Report;
    }
    default:
      return null;
  }
}
