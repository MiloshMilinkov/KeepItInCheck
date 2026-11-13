// Thin typed wrapper over Tauri IPC. Outside Tauri (plain `npm run dev` in a browser) it falls
// back to a local mock so the UI can be designed and tested without the native shell.

import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { LogEntry, PlatformInfo, Pulse, Report, Settings, Snapshot, Toggle } from "./types";

export const native = isTauri();
// Loaded only in a plain browser, so the Tauri build never runs the mock timer.
const mock = native ? null : import("./mock");

type Events = {
  snapshot: Snapshot;
  pulse: Pulse;
  toggle: Toggle;
  "open-cleanup": null;
  paused: boolean;
  "settings-changed": Settings;
  "cleanup-progress": { key: string; stage: "closing" | "done" };
  "auto-hide": null;
};

export function on<K extends keyof Events>(event: K, cb: (payload: Events[K]) => void): () => void {
  if (mock) {
    let off: (() => void) | undefined;
    let gone = false;
    mock.then((m) => {
      if (!gone) off = m.on(event, cb as (p: unknown) => void);
    });
    return () => {
      gone = true;
      off?.();
    };
  }
  let unlisten: (() => void) | undefined;
  let dead = false;
  listen<Events[K]>(event, (e) => cb(e.payload)).then((u) => {
    if (dead) u();
    else unlisten = u;
  });
  return () => {
    dead = true;
    unlisten?.();
  };
}

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return mock ? (mock.then((m) => m.invoke(cmd, args)) as Promise<T>) : invoke<T>(cmd, args);
}

export const api = {
  snapshot: () => call<Snapshot | null>("get_snapshot"),
  settings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),
  setExpanded: (expanded: boolean, focus: boolean) => call<void>("set_expanded", { expanded, focus }),
  setPaused: (paused: boolean) => call<void>("set_paused", { paused }),
  setDetail: (key: string | null) => call<void>("set_detail", { key }),
  icon: (exe: string) => call<string | null>("app_icon", { exe }),
  runCleanup: (keys: string[], dryRun: boolean) => call<Report>("run_cleanup", { keys, dryRun }),
  forceClose: (key: string) => call<LogEntry>("force_close", { key }),
  relaunch: (exe: string) => call<void>("relaunch", { exe }),
  cleanupLog: () => call<LogEntry[]>("cleanup_log"),
  platformInfo: () => call<PlatformInfo>("platform_info"),
  quit: () => call<void>("quit_app"),
  beginDrag: () => call<void>("begin_drag"),
};
