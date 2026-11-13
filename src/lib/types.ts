// Mirrors the Rust types in src-tauri/src/model.rs, settings.rs and cleanup.rs.

export type Class = "safe" | "probable" | "keep";

export interface Verdict {
  class: Class;
  score: number;
  reason: string;
  protected: boolean;
}

export interface ProcView {
  pid: number;
  name: string;
  cpu: number;
  mem: number;
}

export interface AppView {
  key: string;
  name: string;
  exe: string | null;
  cpu: number;
  mem: number;
  hasWindow: boolean;
  /** On the never-touch list (decided by the back-end). */
  excluded: boolean;
  procCount: number;
  /** Only filled for the expanded row. */
  procs: ProcView[];
  verdict: Verdict;
}

export interface Snapshot {
  ts: number;
  cpu: number;
  memUsed: number;
  memTotal: number;
  cores: number;
  processCount: number;
  /** [secondsAgo, cpu%, mem%] */
  history: [number, number, number][];
  apps: AppView[];
  reclaimable: number;
  observedSecs: number;
  learningSecs: number;
  selfCpu: number;
  selfMem: number;
}

/** Sent to the collapsed strip only when high load starts or ends. */
export interface Pulse {
  hot: boolean;
}

export type SortBy = "ram" | "cpu";

export type Edge = "left" | "right" | "top" | "bottom";
export type Theme = "dark" | "light" | "system";
export type Backdrop = "glass" | "solid";

export interface Thresholds {
  idleCpuPct: number;
  idleWindowMins: number;
  unfocusedMins: number;
  minRamMb: number;
  maxIoKbps: number;
  safeScore: number;
}

export interface Settings {
  refreshVisibleMs: number;
  refreshHiddenMs: number;
  edge: Edge;
  monitor: string | null;
  autoHide: boolean;
  compact: boolean;
  theme: Theme;
  backdrop: Backdrop;
  hotkey: string;
  launchAtStartup: boolean;
  dryRun: boolean;
  protectAllMicrosoft: boolean;
  thresholds: Thresholds;
  exclusions: string[];
  snoozed: Record<string, number>;
  sortBy: SortBy;
}

export type Outcome = "closed" | "forceKilled" | "stillRunning" | "skipped" | "failed" | "wouldClose";

export interface LogEntry {
  key: string;
  name: string;
  exe: string | null;
  mem: number;
  outcome: Outcome;
  detail: string;
  dryRun: boolean;
  at: number;
}

export interface Report {
  entries: LogEntry[];
  freed: number;
  estimated: number;
  dryRun: boolean;
}

export interface PlatformInfo {
  glassSupported: boolean;
  monitors: string[];
  version: string;
  /** False when the app started collapsed (launched at Windows startup). */
  expanded: boolean;
}

export interface Toggle {
  expand: boolean | null;
  focus: boolean;
}
