//! IPC commands called from the React UI.

use crate::cleanup::{self, LogEntry, Report};
use crate::model::Snapshot;
use crate::platform::{self, StartupItem};
use crate::report::WeeklyReport;
use crate::sampler::Shared;
use crate::settings::Settings;
use crate::shell;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, State};

type Sh<'a> = State<'a, Arc<Shared>>;

#[tauri::command]
pub fn get_snapshot(state: Sh<'_>) -> Option<Snapshot> {
    state.latest.lock().unwrap().as_deref().cloned()
}

#[tauri::command]
pub fn get_settings(state: Sh<'_>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: Sh<'_>, settings: Settings) -> Result<Settings, String> {
    let (old, new) = state.update_settings(|s| *s = settings).map_err(|e| e.to_string())?;
    shell::apply_settings(&app, &old, &new);
    Ok(new)
}

/// The user pressed on the title bar to move the panel.
#[tauri::command]
pub fn begin_drag(app: AppHandle) {
    shell::begin_drag(&app);
}

/// Open or collapse the panel. `focus` is true for keyboard/tray triggers, false for hover, so
/// hovering the edge never steals focus from the app you are typing in.
#[tauri::command]
pub fn set_expanded(app: AppHandle, state: Sh<'_>, expanded: bool, focus: bool) {
    state.expanded.store(expanded, Ordering::Relaxed);
    shell::layout(&app, expanded, focus);
    if expanded {
        state.wake();
    }
}

/// Which row is expanded in the list; only that app's processes are sent each tick.
#[tauri::command]
pub fn set_detail(state: Sh<'_>, key: Option<String>) {
    *state.detail_key.lock().unwrap() = key;
    state.wake();
}

#[tauri::command]
pub fn set_paused(app: AppHandle, paused: bool) {
    shell::set_paused(&app, paused);
}

/// App icon as a PNG data URL (cached by the UI).
#[tauri::command]
pub async fn app_icon(exe: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let png = platform::create().icon_png(&exe)?;
        Some(format!("data:image/png;base64,{}", base64(&png)))
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
pub async fn run_cleanup(app: AppHandle, state: Sh<'_>, keys: Vec<String>, dry_run: bool) -> Result<Report, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || cleanup::run(&app, &shared, &keys, dry_run))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn force_close(state: Sh<'_>, key: String) -> Result<LogEntry, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || cleanup::force_close(&shared, &key))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn relaunch(state: Sh<'_>, exe: String) -> Result<(), String> {
    cleanup::relaunch(&state, &exe)
}

#[tauri::command]
pub fn cleanup_log(state: Sh<'_>) -> Vec<LogEntry> {
    let mut log = cleanup::read_log(&cleanup::log_path(&state));
    log.reverse();
    log
}

/// True for our own "Launch at startup" entry, which is managed in Settings instead.
fn is_own_entry(item: &StartupItem) -> bool {
    let own = std::env::current_exe().ok().map(|p| p.to_string_lossy().to_lowercase());
    own.is_some() && item.exe.as_ref().map(|e| e.to_lowercase()) == own
}

/// Programs that start with Windows (registry + Startup folders).
#[tauri::command]
pub async fn startup_items() -> Vec<StartupItem> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut items = platform::create().startup_items();
        items.retain(|i| !is_own_entry(i));
        items
    })
    .await
    .unwrap_or_default()
}

/// Enable or disable one startup entry (reversible; nothing is deleted).
#[tauri::command]
pub async fn set_startup_enabled(id: String, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut plat = platform::create();
        if plat.startup_items().iter().any(|i| i.id == id && is_own_entry(i)) {
            return Err("KeepItInCheck's own startup setting is in Settings".to_string());
        }
        plat.set_startup_enabled(&id, enabled)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Top offenders over the last 7 days.
#[tauri::command]
pub fn weekly_report(state: Sh<'_>) -> WeeklyReport {
    let log = cleanup::read_log(&cleanup::log_path(&state));
    // Same core count the live view uses, so averages agree.
    let cores = state.latest.lock().unwrap().as_ref().map(|s| s.cores).unwrap_or(1);
    state.usage.lock().unwrap().report(crate::util::unix_secs(), cores, &log)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub glass_supported: bool,
    pub monitors: Vec<String>,
    pub version: String,
    pub expanded: bool,
}

#[tauri::command]
pub fn platform_info(app: AppHandle, state: Sh<'_>) -> PlatformInfo {
    PlatformInfo {
        expanded: state.expanded.load(Ordering::Relaxed),
        glass_supported: platform::create().supports_glass(),
        monitors: shell::monitor_names(&app),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    shell::quit(&app);
}

/// Standard base64 (no external crate needed for one call site).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }
}
