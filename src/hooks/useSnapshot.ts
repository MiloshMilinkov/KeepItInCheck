import { useEffect, useState } from "react";
import { api, on } from "../lib/api";
import type { Snapshot } from "../lib/types";

/** Latest snapshot pushed by the sampler (about once per second while the panel is open). */
export function useSnapshot(): Snapshot | null {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  useEffect(() => {
    let alive = true;
    api.snapshot().then((s) => alive && s && setSnap((prev) => prev ?? s));
    const off = on("snapshot", (s) => setSnap(s));
    return () => {
      alive = false;
      off();
    };
  }, []);
  return snap;
}

/** True while the system is under high load (tints the collapsed edge strip red). */
export function useHighLoad(): boolean {
  const [hot, setHot] = useState(false);
  useEffect(() => on("pulse", (p) => setHot(p.hot)), []);
  return hot;
}
