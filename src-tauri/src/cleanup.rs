//! Safe cleanup.
//!
//! Order of operations for every selected app:
//! 1. Re-check it against fresh data. Anything that became protected or "Keep" is skipped.
//! 2. Graceful close: WM_CLOSE to its windows (all top-level windows for background apps).
//! 3. Wait up to 5 s for it to exit.
//! 4. Still running *and* no visible window: force-terminate. Still running *with* a window:
//!    leave it alone and say why (it is probably asking to save work). Force-closing a windowed
//!    app is a separate, explicit per-app action ([`force_close`]).
//! 5. Measure the RAM actually freed and append everything to the log.

use crate::model::{AppView, Class};
use crate::platform::{self, Platform};
use crate::sampler::Shared;
use crate::util::{unix_secs, write_json_atomic};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const GRACE: Duration = Duration::from_secs(5);
const LOG_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Exited on its own after the close request.
    Closed,
    /// Background process that ignored the request and was terminated.
    ForceKilled,
    /// Windowed app that did not close; left running.
    StillRunning,
    /// Not touched (no longer safe, already gone, ...).
    Skipped,
    Failed,
    /// Dry run: what would have happened.
    WouldClose,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub key: String,
    pub name: String,
    pub exe: Option<String>,
    pub mem: u64,
    pub outcome: Outcome,
    pub detail: String,
    pub dry_run: bool,
    /// Unix seconds.
    pub at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub entries: Vec<LogEntry>,
    /// Measured change in available RAM.
    pub freed: u64,
    /// Sum of RAM of the apps that closed (what we expected to free).
    pub estimated: u64,
    pub dry_run: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress<'a> {
    key: &'a str,
    stage: &'a str,
}

/// An app selected for cleanup, as seen in the freshest snapshot.
struct Target {
    app: AppView,
    procs: Vec<(u32, u64)>,
}

fn available_memory() -> u64 {
    sysinfo::System::new()
        .map(|mut s| {
            s.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
            s.available_memory()
        })
        .unwrap_or(0)
}

fn entry(app: &AppView, outcome: Outcome, detail: impl Into<String>, dry_run: bool) -> LogEntry {
    LogEntry {
        key: app.key.clone(),
        name: app.name.clone(),
        exe: app.exe.clone(),
        mem: app.mem,
        outcome,
        detail: detail.into(),
        dry_run,
        at: unix_secs(),
    }
}

/// Terminate every listed process (start-time checked). Returns how many could not be ended.
fn kill_all(plat: &mut Box<dyn Platform>, procs: &[(u32, u64)]) -> usize {
    procs.iter().filter(|(pid, st)| plat.kill(*pid, *st).is_err()).count()
}

fn access_denied(failed: usize) -> String {
    format!("Could not end {failed} process(es): access denied")
}

/// Look up apps by key in a fresh snapshot.
fn fresh_targets(shared: &Shared, keys: &[String]) -> Vec<Result<Target, String>> {
    shared.fresh_tick(Duration::from_secs(3));
    let latest = shared.latest.lock().unwrap().clone();
    let members = shared.members.lock().unwrap().clone();
    keys.iter()
        .map(|key| {
            let app = latest.as_ref().and_then(|s| s.apps.iter().find(|a| &a.key == key));
            match (app, members.get(key)) {
                (Some(app), Some(procs)) => Ok(Target { app: app.clone(), procs: procs.clone() }),
                _ => Err(key.clone()),
            }
        })
        .collect()
}

/// Clears the "cleanup running" flag even if we bail out early.
struct RunningGuard<'a>(&'a Shared);
impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.0.cleanup_running.store(false, Ordering::SeqCst);
    }
}

pub fn run(app: &AppHandle, shared: &Shared, keys: &[String], dry_run: bool) -> Result<Report, String> {
    if shared.cleanup_running.swap(true, Ordering::SeqCst) {
        return Err("A cleanup is already running".into());
    }
    let _guard = RunningGuard(shared);

    // Fresh data: the user may have switched to one of these apps since the preview opened.
    let mut entries = Vec::new();
    let mut targets = Vec::new();
    for t in fresh_targets(shared, keys) {
        match t {
            Err(key) => entries.push(LogEntry {
                name: key.rsplit(['\\', '/']).next().unwrap_or(&key).to_string(),
                key,
                exe: None,
                mem: 0,
                outcome: Outcome::Skipped,
                detail: "Already closed".into(),
                dry_run,
                at: unix_secs(),
            }),
            Ok(t) if t.app.verdict.class == Class::Keep => entries.push(entry(
                &t.app,
                Outcome::Skipped,
                format!("No longer suggested: {}", t.app.verdict.reason),
                dry_run,
            )),
            Ok(t) => targets.push(t),
        }
    }

    if dry_run {
        for t in &targets {
            let detail = if t.app.has_window {
                "Would ask it to close. It would be left running if it doesn't close."
            } else {
                "Would ask it to close, then end it after 5 s if it doesn't respond."
            };
            entries.push(entry(&t.app, Outcome::WouldClose, detail, true));
        }
        let estimated = targets.iter().map(|t| t.app.mem).sum();
        return Ok(Report { entries, freed: 0, estimated, dry_run: true });
    }

    let before = available_memory();
    let mut plat = platform::create();

    // 1) Graceful close for everything at once, so the 5 s grace period runs in parallel.
    for t in &targets {
        let _ = app.emit("cleanup-progress", Progress { key: &t.app.key, stage: "closing" });
        let pids: Vec<u32> = t.procs.iter().map(|p| p.0).collect();
        plat.request_close(&pids, !t.app.has_window);
    }
    let all: Vec<(u32, u64)> = targets.iter().flat_map(|t| t.procs.iter().copied()).collect();
    let mut alive: HashSet<(u32, u64)> = plat.wait_exit(&all, GRACE).into_iter().collect();
    // wait_exit cannot watch processes it is not allowed to open; ask the process table instead
    // so an access-denied process is never reported as closed.
    if let Some(table) = plat.process_table() {
        let present: HashSet<(u32, u64)> = table.iter().map(|p| (p.pid, p.start_time)).collect();
        alive.extend(all.iter().copied().filter(|p| present.contains(p)));
    }

    // 2) Decide per app. Window facts are re-read: a close request can open a "Save?" dialog.
    let windowed_now = plat.windows().windowed_pids;
    for t in &targets {
        let left: Vec<(u32, u64)> = t.procs.iter().copied().filter(|p| alive.contains(p)).collect();
        let e = if left.is_empty() {
            entry(&t.app, Outcome::Closed, "Closed gracefully", false)
        } else if left.iter().any(|(pid, _)| windowed_now.contains(pid)) {
            let why = if t.app.has_window {
                "Didn't close within 5 s. It may be asking to save your work, so it was left running."
            } else {
                "Opened a window when asked to close (maybe a save prompt), so it was left running."
            };
            entry(&t.app, Outcome::StillRunning, why, false)
        } else {
            // Only windowless processes remain: safe to end them.
            match (kill_all(&mut plat, &left), t.app.has_window) {
                (0, false) => entry(
                    &t.app,
                    Outcome::ForceKilled,
                    "Didn't respond in 5 s; ended (background process, no window)",
                    false,
                ),
                (0, true) => entry(
                    &t.app,
                    Outcome::Closed,
                    format!("Closed; ended {} leftover background helper(s)", left.len()),
                    false,
                ),
                (n, _) => entry(&t.app, Outcome::Failed, access_denied(n), false),
            }
        };
        entries.push(e);
        let _ = app.emit("cleanup-progress", Progress { key: &t.app.key, stage: "done" });
    }

    // Give the memory manager a moment before measuring.
    std::thread::sleep(Duration::from_millis(1200));
    let after = available_memory();
    let estimated = entries
        .iter()
        .filter(|e| matches!(e.outcome, Outcome::Closed | Outcome::ForceKilled))
        .map(|e| e.mem)
        .sum();
    append_log(&log_path(shared), &entries);
    shared.wake();
    Ok(Report { entries, freed: after.saturating_sub(before), estimated, dry_run: false })
}

/// Explicit, per-app force close for a windowed app the user confirmed separately.
pub fn force_close(shared: &Shared, key: &str) -> Result<LogEntry, String> {
    let t = fresh_targets(shared, &[key.to_string()])
        .pop()
        .and_then(Result::ok)
        .ok_or("That app is no longer running")?;
    if t.app.verdict.protected {
        return Err(format!("Refusing: {}", t.app.verdict.reason));
    }
    let e = match kill_all(&mut platform::create(), &t.procs) {
        0 => entry(&t.app, Outcome::ForceKilled, "Force closed at your request", false),
        n => entry(&t.app, Outcome::Failed, access_denied(n), false),
    };
    append_log(&log_path(shared), std::slice::from_ref(&e));
    shared.wake();
    Ok(e)
}

pub fn log_path(shared: &Shared) -> PathBuf {
    shared.settings_path.with_file_name("cleanup-log.json")
}

pub fn read_log(path: &Path) -> Vec<LogEntry> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn append_log(path: &Path, new: &[LogEntry]) {
    let mut log = read_log(path);
    log.extend(new.iter().cloned());
    if log.len() > LOG_LIMIT {
        log.drain(..log.len() - LOG_LIMIT);
    }
    let _ = write_json_atomic(path, &log, false);
}

/// Relaunch only executables that appear in our own cleanup log, never arbitrary paths.
pub fn relaunch(shared: &Shared, exe: &str) -> Result<(), String> {
    let known = read_log(&log_path(shared))
        .iter()
        .any(|e| e.exe.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(exe)));
    if !known {
        return Err("Only apps closed by KeepItInCheck can be relaunched".into());
    }
    platform::create().launch(exe)
}
