const MB = 1024 * 1024;
const GB = MB * 1024;

/** "312 MB", "1.4 GB", "820 KB". */
export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 MB";
  if (bytes >= GB) return `${(bytes / GB).toFixed(bytes >= 10 * GB ? 0 : 1)} GB`;
  if (bytes >= MB) return `${Math.round(bytes / MB)} MB`;
  return `${Math.max(0, Math.round(bytes / 1024))} KB`;
}

/** "0.4%", "12%". One decimal below 10 so small numbers stay informative. */
export function formatPct(p: number): string {
  if (!Number.isFinite(p) || p <= 0) return "0%";
  if (p < 0.1) return "<0.1%";
  return p < 10 ? `${p.toFixed(1)}%` : `${Math.round(p)}%`;
}

/** "45 s", "12 min", "2 h 5 min". */
export function formatDuration(secs: number): string {
  if (secs < 60) return `${Math.floor(secs)} s`;
  if (secs < 3600) return `${Math.floor(secs / 60)} min`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return m ? `${h} h ${m} min` : `${h} h`;
}

/** "45 min", "2.5 h", "31 h": decimal hours for totals. */
export function formatHours(h: number): string {
  if (h < 1) return `${Math.round(h * 60)} min`;
  return `${h >= 10 ? Math.round(h) : h.toFixed(1)} h`;
}

/** Red is reserved for genuinely high load (matches HIGH_LOAD_PCT in model.rs). */
export function isHigh(pct: number): boolean {
  return pct >= 85;
}

/** File name of a Windows or POSIX path, lower-cased: `C:\Apps\Spotify.exe` -> `spotify.exe`. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop()!.toLowerCase();
}
