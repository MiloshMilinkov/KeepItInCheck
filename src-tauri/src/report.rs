//! Weekly report: what each app cost over the last 7 days.
//!
//! Every 15 s checkpoint adds one interval per app: how long it ran, its RAM, the CPU time it
//! used, and whether it was idle. Totals are bucketed per local calendar day and kept for 8 days
//! in `usage.json` next to the settings. Nothing leaves this PC.

use crate::cleanup::{self, LogEntry};
use crate::util;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

pub const DAY: u64 = 86_400;
const KEEP_DAYS: u64 = 8;
/// Apps seen for less than this over the week are left out of the report.
const MIN_SEEN_SECS: u64 = 300;
const GB: f64 = 1024.0 * 1024.0 * 1024.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AppDay {
    name: String,
    seen: u64,
    /// RAM x seconds (bytes*s), so averages stay exact whatever the sampling rate.
    ram_bs: u128,
    peak: u64,
    cpu_ms: u64,
    idle_secs: u64,
    /// RAM x seconds while idle: memory held for nothing.
    idle_ram_bs: u128,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SysDay {
    seen: u64,
    /// CPU % x seconds and RAM % x seconds, for daily averages.
    cpu_s: f64,
    ram_s: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Day {
    /// Keyed by app group key (lower-cased exe path, or the process name).
    apps: HashMap<String, AppDay>,
    sys: SysDay,
}

/// One app's activity in one checkpoint interval.
pub struct AppSample<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub mem: u64,
    pub cpu_ms: u64,
    pub idle: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Local day number (days since 1970-01-01 in local time) -> totals.
    days: BTreeMap<u64, Day>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    path: PathBuf,
}

/// One app in the report.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportApp {
    pub key: String,
    pub name: String,
    pub exe: Option<String>,
    /// Hours it was running during the week.
    pub hours: f32,
    pub avg_ram: u64,
    pub peak_ram: u64,
    /// RAM-hours: average RAM x hours running, in GB*h. The "memory" ranking.
    pub ram_gbh: f32,
    /// CPU time used, in hours of one core.
    pub cpu_hours: f32,
    /// Average CPU while running, % of the whole machine.
    pub avg_cpu: f32,
    /// RAM held while idle, in GB*h. The "dead weight" ranking.
    pub idle_gbh: f32,
    /// Share of its running time spent idle (0..1).
    pub idle_share: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPoint {
    /// Unix seconds at local midnight.
    pub start: u64,
    pub observed_secs: u64,
    pub avg_cpu: f32,
    pub avg_ram_pct: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyReport {
    pub days: Vec<DayPoint>,
    pub apps: Vec<ReportApp>,
    pub cleanups: u32,
    pub apps_closed: u32,
    pub ram_returned: u64,
}

impl Usage {
    pub fn load(path: PathBuf) -> Self {
        let mut u: Usage = util::read_json_or_default(&path);
        u.path = path;
        u
    }

    /// Persist if anything changed since the last save.
    pub fn save_if_dirty(&mut self) {
        if self.dirty && util::write_json_atomic(&self.path, self, false).is_ok() {
            self.dirty = false;
        }
    }

    /// Add one checkpoint interval: the system averages plus every app's sample.
    pub fn record<'a>(&mut self, now: u64, dur: u64, cpu: f32, ram_pct: f32, apps: impl IntoIterator<Item = AppSample<'a>>) {
        self.dirty = true;
        let today = util::local_day(now);
        if !self.days.contains_key(&today) {
            self.days.retain(|&d, _| d + KEEP_DAYS > today);
        }
        let day = self.days.entry(today).or_default();
        day.sys.seen += dur;
        day.sys.cpu_s += cpu as f64 * dur as f64;
        day.sys.ram_s += ram_pct as f64 * dur as f64;
        for s in apps {
            // Only allocate the key and name the first time an app shows up that day.
            if !day.apps.contains_key(s.key) {
                day.apps.insert(s.key.to_string(), AppDay { name: s.name.to_string(), ..Default::default() });
            }
            let a = day.apps.get_mut(s.key).expect("inserted above");
            let ram = s.mem as u128 * dur as u128;
            a.seen += dur;
            a.ram_bs += ram;
            a.peak = a.peak.max(s.mem);
            a.cpu_ms += s.cpu_ms;
            if s.idle {
                a.idle_secs += dur;
                a.idle_ram_bs += ram;
            }
        }
    }

    /// The last 7 local days, today included. The UI does the ranking.
    pub fn report(&self, now: u64, cores: u32, log: &[LogEntry]) -> WeeklyReport {
        let today = util::local_day(now);
        let first = today.saturating_sub(6);
        let offset = util::local_offset_secs();
        let midnight = |d: u64| (d * DAY) as i64 - offset;
        let mut totals: HashMap<&str, AppDay> = HashMap::new();
        let mut days = Vec::with_capacity(7);
        for d in first..=today {
            let day = self.days.get(&d);
            let sys = day.map(|x| x.sys.clone()).unwrap_or_default();
            let per = |v: f64| if sys.seen > 0 { (v / sys.seen as f64) as f32 } else { 0.0 };
            days.push(DayPoint {
                start: midnight(d).max(0) as u64,
                observed_secs: sys.seen,
                avg_cpu: per(sys.cpu_s),
                avg_ram_pct: per(sys.ram_s),
            });
            for (key, a) in day.map(|x| &x.apps).into_iter().flatten() {
                let t = totals.entry(key.as_str()).or_default();
                t.name.clone_from(&a.name);
                t.seen += a.seen;
                t.ram_bs += a.ram_bs;
                t.peak = t.peak.max(a.peak);
                t.cpu_ms += a.cpu_ms;
                t.idle_secs += a.idle_secs;
                t.idle_ram_bs += a.idle_ram_bs;
            }
        }

        let apps = totals
            .into_iter()
            .filter(|(_, a)| a.seen >= MIN_SEEN_SECS)
            .map(|(key, a)| {
                let seen = a.seen as f64;
                ReportApp {
                    // Keys are lower-cased exe paths (Windows paths are case-insensitive).
                    exe: key.contains(":\\").then(|| key.to_string()),
                    key: key.to_string(),
                    name: a.name,
                    hours: (seen / 3600.0) as f32,
                    avg_ram: (a.ram_bs as f64 / seen) as u64,
                    peak_ram: a.peak,
                    ram_gbh: (a.ram_bs as f64 / 3600.0 / GB) as f32,
                    cpu_hours: (a.cpu_ms as f64 / 3_600_000.0) as f32,
                    avg_cpu: crate::history::machine_pct(a.cpu_ms as f64, seen * 1000.0, cores),
                    idle_gbh: (a.idle_ram_bs as f64 / 3600.0 / GB) as f32,
                    idle_share: (a.idle_secs as f64 / seen) as f32,
                }
            })
            .collect();

        let t = cleanup::totals_since(log, midnight(first).max(0) as u64);
        WeeklyReport { days, apps, cleanups: t.runs, apps_closed: t.apps_closed, ram_returned: t.ram_returned }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1024 * 1024;

    fn sample<'a>(key: &'a str, mem: u64, cpu_ms: u64, idle: bool) -> AppSample<'a> {
        AppSample { key, name: key, mem, cpu_ms, idle }
    }

    #[test]
    fn averages_and_rankings() {
        let mut u = Usage::default();
        let now = 20 * DAY + 12 * 3600;
        // "big" holds 1 GB for 2 h, idle the whole time; "busy" holds 100 MB for 1 h using a full core.
        for i in 0..480 {
            u.record(now - i * 15, 15, 10.0, 50.0, [sample("big", 1024 * MB, 0, true)]);
        }
        for i in 0..240 {
            u.record(now - i * 15, 15, 10.0, 50.0, [sample("busy", 100 * MB, 15_000, false)]);
        }
        u.record(now, 15, 10.0, 50.0, [sample("blip", 10 * MB, 0, true)]);
        let r = u.report(now, 4, &[]);
        assert_eq!(r.apps.len(), 2, "apps seen < 5 min are left out");
        let big = r.apps.iter().find(|a| a.key == "big").unwrap();
        assert!((big.ram_gbh - 2.0).abs() < 0.01);
        assert!((big.idle_gbh - 2.0).abs() < 0.01);
        assert_eq!(big.avg_ram, 1024 * MB);
        assert!((big.idle_share - 1.0).abs() < 1e-6);
        let busy = r.apps.iter().find(|a| a.key == "busy").unwrap();
        assert!((busy.cpu_hours - 1.0).abs() < 0.01);
        assert!((busy.avg_cpu - 25.0).abs() < 0.1, "one core of four");
        assert_eq!(busy.idle_gbh, 0.0);
        assert_eq!(busy.exe, None, "only path keys carry an exe");
    }

    #[test]
    fn only_the_last_seven_days_count_and_old_days_are_pruned() {
        let mut u = Usage::default();
        let today = 100;
        for d in (today - 9)..=today {
            // Midday, so the local-time offset never moves it to another day.
            u.record(d * DAY + 12 * 3600, 600, 10.0, 40.0, [sample(r"c:\a\a.exe", 50 * MB, 0, false)]);
        }
        assert!(u.days.len() <= KEEP_DAYS as usize);
        let r = u.report(today * DAY + 12 * 3600, 4, &[]);
        assert_eq!(r.days.len(), 7);
        assert_eq!(r.apps[0].hours, (7.0 * 600.0 / 3600.0) as f32);
        assert_eq!(r.apps[0].exe.as_deref(), Some(r"c:\a\a.exe"));
        assert!((r.days[6].avg_ram_pct - 40.0).abs() < 1e-3);
    }
}
