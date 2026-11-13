import { memo, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { StartupItem, StartupSource } from "../lib/types";
import { AppIcon } from "./AppIcon";
import { InlineConfirm, SubSheet, Switch } from "./ui";

const SOURCE_LABEL: Record<StartupSource, string> = {
  userRun: "Your account",
  machineRun: "All users",
  machineRun32: "All users",
  userFolder: "Your Startup folder",
  commonFolder: "All users' Startup folder",
};

/**
 * Programs that start with Windows. Turning one off flips the same switch Task Manager uses,
 * so nothing is deleted and it can be turned back on at any time. Turning off asks first.
 */
export const StartupSheet = memo(function StartupSheet({ onBack }: { onBack: () => void }) {
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.startupItems().then(setItems).catch((e) => setError(String(e)));
  }, []);

  const apply = async (item: StartupItem, enabled: boolean) => {
    setConfirm(null);
    setError(null);
    try {
      await api.setStartupEnabled(item.id, enabled);
      setItems((list) => list?.map((x) => (x.id === item.id ? { ...x, enabled } : x)) ?? null);
    } catch (e) {
      setError(String(e));
    }
  };

  const on = items?.filter((i) => i.enabled).length ?? 0;

  return (
    <SubSheet title="Startup apps" onBack={onBack}>
      <p className="m-0 px-4 pt-3 pb-1 text-[12px] leading-snug text-ink-2">
        {items === null
          ? "Reading startup entries…"
          : `${on} of ${items.length} start with Windows. Turning one off uses the same switch as Task Manager: nothing is deleted.`}
      </p>
      {error && <p className="mx-4 my-1 rounded-md bg-danger-soft px-2.5 py-1.5 text-[12px] text-danger">{error}</p>}
      <ul className="m-0 list-none px-2 pb-3">
        {items?.map((item) => (
          <li key={item.id} className="rounded-lg px-2 py-2 hover:bg-hover">
            <div className="grid grid-cols-[20px_minmax(0,1fr)_auto] items-center gap-x-2.5">
              <AppIcon exe={item.exe} name={item.name} />
              <span className="min-w-0">
                <span className={`block truncate text-[13px] ${item.enabled ? "text-ink" : "text-ink-2"}`}>{item.name}</span>
                <span className="block truncate text-[11.5px] text-ink-3" title={item.command}>
                  {item.publisher ?? SOURCE_LABEL[item.source]}
                  {!item.canChange && ", needs admin to change"}
                </span>
              </span>
              <Switch
                checked={item.enabled}
                disabled={!item.canChange}
                label={`${item.name} starts with Windows`}
                onChange={(v) => (v ? apply(item, true) : setConfirm(item.id))}
              />
            </div>
            <div className="pl-[30px]">
              <InlineConfirm
                open={confirm === item.id}
                message={`Stop ${item.name} from starting with Windows? It stays installed, and you can turn it back on here.`}
                cancelLabel="Keep it"
                confirmLabel="Don't start it"
                onCancel={() => setConfirm(null)}
                onConfirm={() => apply(item, false)}
              />
            </div>
          </li>
        ))}
        {items?.length === 0 && <li className="px-2 py-2 text-[12px] text-ink-3">Nothing is set to start with Windows.</li>}
      </ul>
    </SubSheet>
  );
});
