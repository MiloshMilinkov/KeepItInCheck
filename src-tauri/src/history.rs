//! Rolling per-app history used by the dead-weight classifier.
//!
//! Instead of storing every sample, we take a checkpoint every ~15 s and record how much CPU time
//! and I/O each app group consumed in that interval (from cumulative OS counters). Averages over
//! the idle window are then exact no matter how often the sampler ran (1 s visible, 5 s hidden).
//! Memory cost is tiny: ~40 small records per app for a 10 minute window.

use crate::grouping::Group;
use crate::model::ProcSample;
use crate::settings::Thresholds;
use std::collections::{HashMap, HashSet, VecDeque};

pub const CHECKPOINT_SECS: u64 = 15;
/// Groups that disappear are forgotten after this long (an app restart starts fresh).
const FORGET_AFTER_SECS: u64 = 60;

#[derive(Debug, Clone, Copy)]
struct PidBase {
    start_time: u64,
    cpu_ms: u64,
    io: u64,
}

#[derive(Debug, Clone, Copy)]
struct Rec {
    t: u64,
    dur: u64,
    cpu_ms: u64,
    io: u64,
}

#[derive(Debug, Clone)]
struct GroupHist {
    first_seen: u64,
    last_seen: u64,
    last_active: u64,
    last_foreground: Option<u64>,
    last_audio: Option<u64>,
    records: VecDeque<Rec>,
}

/// What the classifier needs to know about one app's recent behaviour.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GroupStats {
    /// Average CPU over the idle window, % of the whole machine.
    pub avg_cpu_pct: f32,
    /// Average I/O over the idle window, bytes per second.
    pub io_bps: f64,
    /// Seconds of checkpointed data inside the idle window.
    pub covered_secs: u64,
    /// Seconds since this app first appeared while we were watching.
    pub observed_secs: u64,
    /// Seconds since it last used CPU/I-O above the thresholds, had focus or played audio.
    pub idle_secs: u64,
    /// Seconds since it last owned the foreground window (None: never while observed).
    pub since_foreground: Option<u64>,
    /// Seconds since it last played audio (None: never while observed).
    pub since_audio: Option<u64>,
}

#[derive(Debug)]
pub struct History {
    cores: u32,
    last_checkpoint: Option<u64>,
    pids: HashMap<u32, PidBase>,
    groups: HashMap<String, GroupHist>,
}

/// Per-tick signals that do not come from cumulative counters.
pub struct TickSignals<'a> {
    pub foreground_key: Option<&'a str>,
    pub audio_keys: &'a HashSet<String>,
}

impl History {
    pub fn new(cores: u32) -> Self {
        Self { cores: cores.max(1), last_checkpoint: None, pids: HashMap::new(), groups: HashMap::new() }
    }

    /// Feed one sampler tick. `now` is unix seconds.
    pub fn update(&mut self, now: u64, procs: &[ProcSample], groups: &[Group], signals: &TickSignals, t: &Thresholds) {
        for g in groups {
            // `get_mut` first: the key is only cloned for a group seen for the first time.
            if !self.groups.contains_key(&g.key) {
                self.groups.insert(
                    g.key.clone(),
                    GroupHist {
                        first_seen: now,
                        last_seen: now,
                        // Conservative start: assume everything was in use right before we started.
                        last_active: now,
                        last_foreground: None,
                        last_audio: None,
                        records: VecDeque::new(),
                    },
                );
            }
            let h = self.groups.get_mut(&g.key).expect("inserted above");
            h.last_seen = now;
            if signals.foreground_key == Some(g.key.as_str()) {
                h.last_foreground = Some(now);
                h.last_active = now;
            }
            if signals.audio_keys.contains(&g.key) {
                h.last_audio = Some(now);
                h.last_active = now;
            }
        }

        let due = self
            .last_checkpoint
            .is_none_or(|t| now.saturating_sub(t) >= CHECKPOINT_SECS);
        if due {
            self.checkpoint(now, procs, groups, t);
        }

        self.groups
            .retain(|_, h| now.saturating_sub(h.last_seen) <= FORGET_AFTER_SECS);
    }

    fn checkpoint(&mut self, now: u64, procs: &[ProcSample], groups: &[Group], t: &Thresholds) {
        let prev = self.last_checkpoint;
        let mut next = HashMap::with_capacity(procs.len());
        // Per-process deltas since the previous checkpoint.
        let mut delta: Vec<(u64, u64)> = Vec::with_capacity(procs.len());
        for p in procs {
            let d = match (self.pids.get(&p.pid), prev) {
                (Some(b), _) if b.start_time == p.start_time => (
                    p.cpu_time_ms.saturating_sub(b.cpu_ms),
                    p.io_bytes.saturating_sub(b.io),
                ),
                // Born since the last checkpoint: its whole life happened in this interval.
                (_, Some(t)) if p.start_time >= t => (p.cpu_time_ms, p.io_bytes),
                // First time we see a long-running process: no baseline yet.
                _ => (0, 0),
            };
            delta.push(d);
            next.insert(
                p.pid,
                PidBase { start_time: p.start_time, cpu_ms: p.cpu_time_ms, io: p.io_bytes },
            );
        }
        self.pids = next;
        self.last_checkpoint = Some(now);

        let Some(prev) = prev else { return };
        let dur = now.saturating_sub(prev).max(1);
        let keep_after = now.saturating_sub(t.window_secs() + CHECKPOINT_SECS);
        let max_io_bps = t.max_io_bps();
        for g in groups {
            let (cpu_ms, io) = g
                .members
                .iter()
                .fold((0u64, 0u64), |(c, i), &m| (c + delta[m].0, i + delta[m].1));
            let Some(h) = self.groups.get_mut(&g.key) else { continue };
            let cpu_pct = machine_pct(cpu_ms as f64, dur as f64 * 1000.0, self.cores);
            let io_bps = io as f64 / dur as f64;
            if cpu_pct >= t.idle_cpu_pct || io_bps >= max_io_bps {
                h.last_active = now;
            }
            h.records.push_back(Rec { t: now, dur, cpu_ms, io });
            while h.records.front().is_some_and(|r| r.t < keep_after) {
                h.records.pop_front();
            }
        }
    }

    pub fn stats(&self, key: &str, now: u64, window_secs: u64) -> GroupStats {
        let Some(h) = self.groups.get(key) else { return GroupStats::default() };
        let from = now.saturating_sub(window_secs);
        let (mut cpu, mut io, mut dur) = (0u64, 0u64, 0u64);
        for r in h.records.iter().filter(|r| r.t > from) {
            cpu += r.cpu_ms;
            io += r.io;
            dur += r.dur;
        }
        GroupStats {
            avg_cpu_pct: if dur == 0 { 0.0 } else { machine_pct(cpu as f64, dur as f64 * 1000.0, self.cores) },
            io_bps: if dur == 0 { 0.0 } else { io as f64 / dur as f64 },
            covered_secs: dur.min(window_secs),
            observed_secs: now.saturating_sub(h.first_seen),
            idle_secs: now.saturating_sub(h.last_active),
            since_foreground: h.last_foreground.map(|t| now.saturating_sub(t)),
            since_audio: h.last_audio.map(|t| now.saturating_sub(t)),
        }
    }
}

/// CPU time as a percentage of the whole machine over `wall_ms` (Task Manager style).
pub fn machine_pct(cpu_ms: f64, wall_ms: f64, cores: u32) -> f32 {
    let capacity = wall_ms * cores.max(1) as f64;
    if capacity <= 0.0 { 0.0 } else { (cpu_ms / capacity * 100.0).min(100.0) as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grouping;

    fn p(pid: u32, cpu_ms: u64, io: u64) -> ProcSample {
        ProcSample {
            pid,
            ppid: 0,
            name: format!("app{pid}.exe"),
            exe: Some(format!(r"C:\Apps\app{pid}.exe")),
            start_time: 1,
            cpu_time_ms: cpu_ms,
            io_bytes: io,
            ..Default::default()
        }
    }

    fn tick(h: &mut History, now: u64, procs: &[ProcSample], fg: Option<&str>) {
        let groups = grouping::group(procs, &grouping::pid_index(procs));
        let audio = HashSet::new();
        let sig = TickSignals { foreground_key: fg, audio_keys: &audio };
        h.update(now, procs, &groups, &sig, &Thresholds::default());
    }

    #[test]
    fn averages_cpu_over_the_window() {
        let mut h = History::new(4);
        let key = r"c:\apps\app1.exe";
        // 4 cores. 15 s interval = 60_000 ms capacity. 600 ms of CPU = 1 %.
        let mut cpu = 0;
        for i in 0..=40 {
            tick(&mut h, 1000 + i * 15, &[p(1, cpu, 0)], None);
            cpu += 600;
        }
        let s = h.stats(key, 1000 + 40 * 15, 600);
        assert!((s.avg_cpu_pct - 1.0).abs() < 0.01, "got {}", s.avg_cpu_pct);
        assert_eq!(s.covered_secs, 600);
        assert_eq!(s.idle_secs, 0, "1 % is above the idle threshold");
    }

    #[test]
    fn idle_app_accumulates_idle_time() {
        let mut h = History::new(4);
        let key = r"c:\apps\app2.exe";
        for i in 0..=40 {
            tick(&mut h, 1000 + i * 15, &[p(2, 5, 0)], None);
        }
        let s = h.stats(key, 1600, 600);
        assert_eq!(s.avg_cpu_pct, 0.0);
        assert_eq!(s.idle_secs, 600);
        assert_eq!(s.observed_secs, 600);
        assert_eq!(s.since_foreground, None);
    }

    #[test]
    fn foreground_resets_idle() {
        let mut h = History::new(4);
        let key = r"c:\apps\app3.exe";
        for i in 0..=40 {
            let fg = (i == 30).then_some(key);
            tick(&mut h, 1000 + i * 15, &[p(3, 0, 0)], fg);
        }
        let s = h.stats(key, 1600, 600);
        assert_eq!(s.since_foreground, Some(150));
        assert_eq!(s.idle_secs, 150);
    }

    #[test]
    fn io_counts_as_activity() {
        let mut h = History::new(4);
        let key = r"c:\apps\app4.exe";
        let mut io = 0;
        for i in 0..=8 {
            tick(&mut h, 1000 + i * 15, &[p(4, 0, io)], None);
            io += 15 * 100 * 1024; // 100 KB/s
        }
        let s = h.stats(key, 1120, 600);
        assert!(s.io_bps > 90_000.0);
        assert_eq!(s.idle_secs, 0);
    }

    #[test]
    fn vanished_groups_are_forgotten() {
        let mut h = History::new(4);
        tick(&mut h, 1000, &[p(5, 0, 0)], None);
        tick(&mut h, 1100, &[], None);
        assert_eq!(h.stats(r"c:\apps\app5.exe", 1100, 600), GroupStats::default());
    }
}
