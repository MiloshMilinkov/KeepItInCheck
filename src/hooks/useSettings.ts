import { useCallback, useEffect, useRef, useState } from "react";
import { api, on } from "../lib/api";
import type { Settings } from "../lib/types";

export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const ref = useRef<Settings | null>(null);
  ref.current = settings;

  useEffect(() => {
    api.settings().then(setSettings);
    return on("settings-changed", setSettings);
  }, []);

  /** Optimistic update; the back-end sanitises and echoes the stored value. */
  const update = useCallback((patch: Partial<Settings> | ((s: Settings) => Settings)) => {
    const prev = ref.current;
    if (!prev) return;
    const next = typeof patch === "function" ? patch(prev) : { ...prev, ...patch };
    ref.current = next;
    setSettings(next);
    api.saveSettings(next).then(setSettings).catch(() => setSettings(prev));
  }, []);

  return [settings, update] as const;
}
