import { memo, useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { formatBytes, formatDuration, formatHours, formatPct } from "../lib/format";
import type { DayPoint, ReportApp, WeeklyReport } from "../lib/types";
import { AppIcon } from "./AppIcon";
import { Segmented, ShareBar, SubSheet } from "./ui";

type Tab = "idle" | "ram" | "cpu";

const TABS: { value: Tab; label: string }[] = [
  { value: "idle", label: "Idle RAM" },
  { value: "ram", label: "Memory" },
  { value: "cpu", label: "CPU" },
];

const metric: Record<Tab, (a: ReportApp) => number> = {
  idle: (a) => a.idleGbh,
  ram: (a) => a.ramGbh,
  cpu: (a) => a.cpuHours,
};

function gbh(v: number) {
  return `${v >= 10 ? Math.round(v) : v.toFixed(1)} GB·h`;
}

// Created once: the report re-renders on every live snapshot while open.
const weekdayShort = new Intl.DateTimeFormat([], { weekday: "short" });
const weekdayNarrow = new Intl.DateTimeFormat([], { weekday: "narrow" });

function headline(tab: Tab, a: ReportApp): [string, string] {
  switch (tab) {
    case "idle":
      return [gbh(a.idleGbh), `idle ${Math.round(a.idleShare * 100)}% of its ${formatHours(a.hours)} running, about ${formatBytes(a.avgRam)}`];
    case "ram":
      return [gbh(a.ramGbh), `${formatBytes(a.avgRam)} on average over ${formatHours(a.hours)}, peak ${formatBytes(a.peakRam)}`];
    case "cpu":
      return [formatHours(a.cpuHours), `of CPU time, ${formatPct(a.avgCpu)} of the machine on average`];
  }
}

/** Top offenders over the last 7 days, from KeepItInCheck's own local history. */
export const ReportSheet = memo(function ReportSheet({ onBack }: { onBack: () => void }) {
  const [report, setReport] = useState<WeeklyReport | null>(null);
  const [tab, setTab] = useState<Tab>("idle");

  useEffect(() => {
    api.weeklyReport().then(setReport);
  }, []);

  const ranked = useMemo(() => {
    if (!report) return [];
    return [...report.apps]
      .sort((a, b) => metric[tab](b) - metric[tab](a))
      .filter((a) => metric[tab](a) > 0)
      .slice(0, 10);
  }, [report, tab]);

  const observed = report ? report.days.reduce((s, d) => s + d.observedSecs, 0) : 0;
  const enough = observed >= 3600;
  const top = ranked.length ? metric[tab](ranked[0]) : 1;

  return (
    <SubSheet title="Weekly report" onBack={onBack}>
      {!report ? (
        <p className="m-0 px-4 py-3 text-[12px] text-ink-3">Reading history…</p>
      ) : (
        <>
          <section className="px-4 pt-3">
            <p className="m-0 text-[12px] text-ink-2">
              Last 7 days, based on {formatDuration(observed)} of monitoring.
            </p>
            {report.appsClosed > 0 && (
              <p className="m-0 mt-1 text-[13px] leading-snug text-ink">
                {report.cleanups} {report.cleanups === 1 ? "cleanup" : "cleanups"} closed {report.appsClosed}{" "}
                {report.appsClosed === 1 ? "app" : "apps"} that held about {formatBytes(report.ramReturned)}.
              </p>
            )}
            <DayChart days={report.days} />
          </section>

          {!enough ? (
            <p className="mx-4 mt-2 rounded-lg border border-dashed border-line px-3 py-3 text-[12px] leading-snug text-ink-2">
              <span className="text-ink">Not enough history yet.</span> The report fills in while KeepItInCheck runs, even
              when it is hidden on the edge. Check back after a few hours.
            </p>
          ) : (
            <section aria-label="Top offenders" className="pb-3">
              <header className="flex items-center justify-between px-4 pt-3 pb-1.5">
                <h3 className="m-0 text-[12px] font-medium text-ink-2">Top offenders</h3>
                <Segmented value={tab} onChange={setTab} options={TABS} label="Rank by" />
              </header>
              {tab === "idle" && (
                <p className="m-0 px-4 pb-1 text-[11.5px] leading-snug text-ink-3">
                  Memory held while doing nothing. 1 GB·h is 1 GB kept for an hour.
                </p>
              )}
              <ol className="m-0 list-none px-2">
                {ranked.map((a) => {
                  const [value, detail] = headline(tab, a);
                  return (
                    <li key={a.key} className="relative rounded-lg px-2 pt-2 pb-2.5 hover:bg-hover">
                      <div className="grid grid-cols-[20px_minmax(0,1fr)_auto] items-center gap-x-2.5">
                        <AppIcon exe={a.exe} name={a.name} />
                        <span className="truncate text-[13px] text-ink">{a.name}</span>
                        <span className="text-[12px] text-ink">{value}</span>
                      </div>
                      <p className="m-0 mt-0.5 pl-[30px] text-[11.5px] leading-snug text-ink-2">{detail}</p>
                      <ShareBar share={metric[tab](a) / top} />
                    </li>
                  );
                })}
                {ranked.length === 0 && <li className="px-2 py-2 text-[12px] text-ink-3">Nothing to rank yet.</li>}
              </ol>
            </section>
          )}
        </>
      )}
    </SubSheet>
  );
});

/**
 * Average RAM in use per day: one series, one hue, thin bars rising from a shared baseline,
 * a hover tooltip per bar, and days without data drawn as an empty track.
 */
function DayChart({ days }: { days: DayPoint[] }) {
  const [hover, setHover] = useState<number | null>(null);
  const H = 64;
  const h = hover !== null ? days[hover] : null;
  return (
    <figure className="m-0 mt-3">
      <figcaption className="flex items-baseline justify-between text-[11.5px]">
        <span className="font-medium text-ink-2">Average RAM in use per day</span>
        <span className="text-ink-3" aria-live="polite">
          {h
            ? h.observedSecs > 0
              ? `${Math.round(h.avgRamPct)}% RAM, ${Math.round(h.avgCpu)}% CPU, ${formatDuration(h.observedSecs)} watched`
              : "Not running that day"
            : "Hover a day"}
        </span>
      </figcaption>
      <div className="mt-2 grid grid-cols-7 gap-[2px]" style={{ height: H }} role="list">
        {days.map((d, i) => {
          const has = d.observedSecs > 0;
          const pct = Math.max(2, Math.min(100, d.avgRamPct));
          const label = weekdayShort.format(d.start * 1000);
          return (
            <div
              key={d.start}
              role="listitem"
              aria-label={has ? `${label}: ${Math.round(d.avgRamPct)}% RAM on average` : `${label}: no data`}
              className="flex h-full cursor-default flex-col justify-end px-[3px]"
              onMouseEnter={() => setHover(i)}
              onMouseLeave={() => setHover(null)}
            >
              <div
                className={`w-full rounded-t-[4px] transition-colors ${has ? (hover === i ? "bg-accent" : "bg-accent/70") : "bg-track"}`}
                style={{ height: has ? `${pct}%` : 3 }}
              />
            </div>
          );
        })}
      </div>
      <div className="mt-1 h-px bg-line" />
      <div className="mt-1 grid grid-cols-7 text-center text-[10.5px] text-ink-3">
        {days.map((d) => (
          <span key={d.start}>{weekdayNarrow.format(d.start * 1000)}</span>
        ))}
      </div>
    </figure>
  );
}
