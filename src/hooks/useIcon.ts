import { useEffect, useState } from "react";
import { api } from "../lib/api";

// Session cache: each exe's icon is extracted once.
const cache = new Map<string, Promise<string | null>>();

export function useIcon(exe: string | null): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!exe) return;
    let alive = true;
    let p = cache.get(exe);
    if (!p) {
      p = api.icon(exe).catch(() => null);
      cache.set(exe, p);
    }
    p.then((u) => alive && setUrl(u));
    return () => {
      alive = false;
    };
  }, [exe]);
  return url;
}
