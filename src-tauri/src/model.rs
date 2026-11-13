//! Plain data types shared by the sampler, classifier and the UI.
//!
//! Everything that crosses the IPC boundary is `camelCase` JSON so the React side can use it
//! directly. Keep these structs small: a snapshot is emitted up to once per second.

use serde::{Deserialize, Serialize};

/// One raw process sample taken from the OS. Cumulative counters are since process start.
#[derive(Debug, Clone, Default)]
pub struct ProcSample {
    pub pid: u32,
    pub ppid: u32,
    /// Executable file name, e.g. `chrome.exe`.
    pub name: String,
    /// Full executable path when we are allowed to read it.
    pub exe: Option<String>,
    /// Process creation time (opaque units, only compared for equality/ordering).
    pub start_time: u64,
    /// Terminal-services session. Session 0 hosts services and system processes.
    pub session_id: Option<u32>,
    /// CPU usage since the previous sample, as % of the *whole machine* (0..=100).
    pub cpu: f32,
    /// Cumulative CPU time (user + kernel) in milliseconds.
    pub cpu_time_ms: u64,
    /// Private working set in bytes: the RAM that is freed when the process exits.
    pub mem: u64,
    /// Cumulative read + write transfer bytes (disk and network I/O on Windows).
    pub io_bytes: u64,
}

/// How a flagged app is classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Class {
    /// High confidence: idle background app with no window. Pre-checked in Clean Up.
    Safe,
    /// Idle but needs a human look (has a window, or a cautious category). Not pre-checked.
    Probable,
    /// Never suggested.
    Keep,
}

/// The classifier's decision for one app group, with a plain-English reason.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub class: Class,
    /// 0..=100, higher means "more dead weight".
    pub score: u8,
    pub reason: String,
    /// True when the app is on the protected list and can never be closed.
    pub protected: bool,
}

/// One process inside an app group (shown when a row is expanded).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcView {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub mem: u64,
}

/// One row in the Top Consumers list: an application with all its helper processes rolled up.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    /// Stable identity across ticks (lower-cased exe path or name).
    pub key: String,
    pub name: String,
    pub exe: Option<String>,
    pub cpu: f32,
    pub mem: u64,
    pub has_window: bool,
    /// On the user's never-touch list (decided here so the UI never re-implements matching).
    pub excluded: bool,
    pub proc_count: u32,
    /// Member processes; only filled for the row the UI has expanded, to keep updates small.
    pub procs: Vec<ProcView>,
    pub verdict: Verdict,
}

/// Everything the UI needs for one frame.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Milliseconds since the Unix epoch.
    pub ts: u64,
    pub cpu: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub cores: u32,
    pub process_count: u32,
    /// Last ~60 s of `[seconds_ago, cpu%, mem%]` for the sparklines.
    pub history: Vec<[f32; 3]>,
    pub apps: Vec<AppView>,
    /// Sum of RAM held by Safe + Probably-safe apps.
    pub reclaimable: u64,
    /// How long monitoring has been running (dead-weight needs time to learn).
    pub observed_secs: u64,
    /// The idle window the classifier needs before it is fully confident.
    pub learning_secs: u64,
    /// KeepItInCheck's own footprint (main process + its WebView2 children).
    pub self_cpu: f32,
    pub self_mem: u64,
}

/// What the edge strip needs while collapsed: only whether the system is under high load.
/// Sent only when that changes, so a collapsed panel normally receives nothing at all.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    pub hot: bool,
}

/// CPU or RAM at or above this percentage is "high load" (red in the UI).
pub const HIGH_LOAD_PCT: f32 = 85.0;
