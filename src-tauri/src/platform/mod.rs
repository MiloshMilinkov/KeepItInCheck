//! OS abstraction. Everything platform-specific lives behind [`Platform`] so macOS and Linux
//! back-ends can be added without touching the sampler, classifier or UI.
//!
//! A `Platform` value is owned by one thread (the Windows audio code holds COM objects), so the
//! trait does not require `Send`. Commands that need OS access create their own instance.

use crate::model::ProcSample;
use std::collections::HashSet;
use std::time::Duration;

#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
mod other;

/// Window facts for one tick.
#[derive(Debug, Default, Clone)]
pub struct WindowInfo {
    /// PIDs that own at least one user-facing top-level window (visible or minimised).
    pub windowed_pids: HashSet<u32>,
    /// PID owning the current foreground window.
    pub foreground_pid: Option<u32>,
}

/// Version-resource facts about an executable.
#[derive(Debug, Default, Clone)]
pub struct FileInfo {
    pub description: Option<String>,
    pub product: Option<String>,
    pub company: Option<String>,
}

/// Result of an Authenticode check. Only "is it Microsoft" drives a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    /// Valid, trusted signature issued to Microsoft.
    Microsoft,
    /// Signed by someone else, unsigned, or not checkable.
    Other,
}

pub trait Platform {
    /// Every process in one go. CPU % is left at 0; the sampler derives it from `cpu_time_ms`.
    /// Returns `None` when there is no fast path and the caller should fall back to `sysinfo`.
    fn process_table(&mut self) -> Option<Vec<ProcSample>>;

    /// Full executable path of a process, when readable.
    fn exe_path(&mut self, pid: u32) -> Option<String>;

    /// Which processes own windows, and who owns the foreground.
    fn windows(&mut self) -> WindowInfo;

    /// PIDs with an audio session that is producing sound right now.
    fn audio_pids(&mut self) -> HashSet<u32>;

    /// PIDs hosting a running Windows service (or the platform equivalent).
    fn service_pids(&mut self) -> HashSet<u32>;

    /// Version resource lookup (display name and vendor).
    fn file_info(&mut self, exe: &str) -> FileInfo;

    /// Authenticode check. Slow (tens of ms); call from a worker thread and cache.
    fn signature(&mut self, exe: &str) -> Signature;

    /// Ask a process to close by posting WM_CLOSE to its top-level windows.
    /// With `include_hidden` false only user-facing windows are targeted.
    fn request_close(&mut self, pids: &[u32], include_hidden: bool);

    /// Wait until the given processes exit or `timeout` passes. Returns those still alive.
    /// Each entry is `(pid, start_time)` so a recycled PID is never mistaken for the original.
    fn wait_exit(&mut self, procs: &[(u32, u64)], timeout: Duration) -> Vec<(u32, u64)>;

    /// Terminate a process, but only if it is still the same process (matching start time).
    fn kill(&mut self, pid: u32, start_time: u64) -> Result<(), String>;

    /// The app's icon as PNG bytes.
    fn icon_png(&mut self, exe: &str) -> Option<Vec<u8>>;

    /// Start an executable detached from us.
    fn launch(&mut self, exe: &str) -> Result<(), String>;

    /// True when the OS can draw a blurred (Mica/Acrylic) window backdrop.
    fn supports_glass(&self) -> bool;
}

/// Create the platform implementation for this OS.
pub fn create() -> Box<dyn Platform> {
    #[cfg(windows)]
    {
        Box::new(windows::WinPlatform::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(other::OtherPlatform::new())
    }
}

/// Apply the window corner region / backdrop for the docked panel (Windows only).
#[cfg(windows)]
pub use windows::window_style;
