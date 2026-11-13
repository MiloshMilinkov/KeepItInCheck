//! User settings, persisted as JSON in the app config directory.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    Ram,
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Backdrop {
    /// Mica/Acrylic blur on Windows 11, solid elsewhere.
    Glass,
    Solid,
}

/// Dead-weight thresholds. See docs/dead-weight-score.md for how each one is used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Thresholds {
    /// Average CPU (% of the whole machine) below which an app counts as idle.
    pub idle_cpu_pct: f32,
    /// Window over which CPU and I/O are averaged, in minutes.
    pub idle_window_mins: u32,
    /// A windowed app counts as unused after this many minutes out of the foreground.
    pub unfocused_mins: u32,
    /// Apps using less RAM than this are ignored.
    pub min_ram_mb: u32,
    /// Average disk + network I/O below this counts as negligible, in KB/s.
    pub max_io_kbps: f32,
    /// Minimum score for "Safe to close".
    pub safe_score: u8,
}

impl Thresholds {
    pub fn window_secs(&self) -> u64 {
        self.idle_window_mins as u64 * 60
    }
    pub fn unfocused_secs(&self) -> u64 {
        self.unfocused_mins as u64 * 60
    }
    pub fn min_ram_bytes(&self) -> u64 {
        self.min_ram_mb as u64 * 1024 * 1024
    }
    pub fn max_io_bps(&self) -> f64 {
        self.max_io_kbps as f64 * 1024.0
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            idle_cpu_pct: 0.3,
            idle_window_mins: 10,
            unfocused_mins: 30,
            min_ram_mb: 50,
            max_io_kbps: 20.0,
            safe_score: 65,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Polling interval while the panel is open, in ms.
    pub refresh_visible_ms: u32,
    /// Polling interval while collapsed to the edge strip, in ms.
    pub refresh_hidden_ms: u32,
    pub edge: Edge,
    /// Monitor name to dock on; `None` means the primary monitor.
    pub monitor: Option<String>,
    pub auto_hide: bool,
    pub compact: bool,
    pub theme: Theme,
    pub backdrop: Backdrop,
    pub hotkey: String,
    pub launch_at_startup: bool,
    pub dry_run: bool,
    /// Protect every Microsoft-signed binary, not just the ones inside the Windows folder.
    pub protect_all_microsoft: bool,
    pub thresholds: Thresholds,
    /// Lower-cased exe names or paths the user never wants touched.
    pub exclusions: Vec<String>,
    /// Lower-cased group key -> unix seconds until which suggestions are snoozed.
    pub snoozed: std::collections::BTreeMap<String, u64>,
    pub sort_by: SortBy,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_visible_ms: 1000,
            refresh_hidden_ms: 5000,
            edge: Edge::Right,
            monitor: None,
            auto_hide: true,
            compact: false,
            theme: Theme::Dark,
            backdrop: Backdrop::Glass,
            hotkey: "Ctrl+Alt+R".into(),
            launch_at_startup: false,
            dry_run: false,
            protect_all_microsoft: true,
            thresholds: Thresholds::default(),
            exclusions: Vec::new(),
            snoozed: Default::default(),
            sort_by: SortBy::Ram,
        }
    }
}

impl Settings {
    /// Clamp values a hand-edited file could break.
    pub fn sanitized(mut self) -> Self {
        self.refresh_visible_ms = self.refresh_visible_ms.clamp(500, 10_000);
        self.refresh_hidden_ms = self.refresh_hidden_ms.clamp(1_000, 60_000);
        let t = &mut self.thresholds;
        t.idle_cpu_pct = t.idle_cpu_pct.clamp(0.05, 10.0);
        t.idle_window_mins = t.idle_window_mins.clamp(2, 120);
        t.unfocused_mins = t.unfocused_mins.clamp(1, 600);
        t.min_ram_mb = t.min_ram_mb.clamp(1, 8192);
        t.max_io_kbps = t.max_io_kbps.clamp(0.0, 100_000.0);
        t.safe_score = t.safe_score.clamp(1, 100);
        for e in &mut self.exclusions {
            *e = e.trim().to_lowercase();
        }
        let mut seen = std::collections::HashSet::new();
        self.exclusions.retain(|e| !e.is_empty() && seen.insert(e.clone()));
        self
    }

    pub fn load(path: &Path) -> Self {
        crate::util::read_json_or_default::<Settings>(path).sanitized()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::util::write_json_atomic(path, self, true)
    }

    /// True when this exe name or path is on the user's "never touch" list.
    pub fn is_excluded(&self, name: &str, exe: Option<&str>) -> bool {
        if self.exclusions.is_empty() {
            return false;
        }
        let name = name.to_lowercase();
        let exe = exe.map(str::to_lowercase);
        self.exclusions
            .iter()
            .any(|e| *e == name || exe.as_deref() == Some(e.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"edge":"left"}"#).unwrap();
        assert_eq!(s.edge, Edge::Left);
        assert_eq!(s.thresholds.min_ram_mb, 50);
        assert_eq!(s.hotkey, "Ctrl+Alt+R");
    }

    #[test]
    fn load_tolerates_a_bom() {
        let dir = std::env::temp_dir().join(format!("kic-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("settings.json");
        std::fs::write(&p, "\u{feff}{\"edge\":\"top\"}").unwrap();
        assert_eq!(Settings::load(&p).edge, Edge::Top);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sanitize_clamps_and_normalises() {
        let mut s = Settings::default();
        s.refresh_visible_ms = 1;
        s.exclusions = vec!["  Spotify.EXE ".into(), "".into(), "a.exe".into(), "spotify.exe".into()];
        let s = s.sanitized();
        assert_eq!(s.refresh_visible_ms, 500);
        assert_eq!(s.exclusions, vec!["spotify.exe".to_string(), "a.exe".to_string()]);
        assert!(s.is_excluded("SPOTIFY.exe", None));
    }
}
