//! Placeholder back-end for macOS and Linux.
//!
//! The sampler falls back to `sysinfo` for the process table when `process_table` returns
//! `None`. Window, audio and service detection return empty sets, which makes the classifier
//! conservative (see the TODOs). Implement these to bring the dead-weight detector to a new OS.

use super::{FileInfo, Platform, Signature, WindowInfo};
use crate::model::ProcSample;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

pub struct OtherPlatform {
    sys: Option<System>,
}

impl OtherPlatform {
    pub fn new() -> Self {
        Self { sys: System::new().ok() }
    }

    fn proc_alive(&mut self, pid: u32, start_time: u64) -> bool {
        let Some(sys) = self.sys.as_mut() else { return false };
        let p = Pid::from_u32(pid);
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), true, ProcessRefreshKind::nothing());
        sys.process(p).is_some_and(|pr| pr.start_time() == start_time)
    }
}

impl Platform for OtherPlatform {
    fn process_table(&mut self) -> Option<Vec<ProcSample>> {
        None
    }
    fn exe_path(&mut self, _pid: u32) -> Option<String> {
        None
    }
    fn windows(&mut self) -> WindowInfo {
        // TODO(macOS): CGWindowListCopyWindowInfo; TODO(Linux): X11 _NET_CLIENT_LIST / Wayland.
        WindowInfo::default()
    }
    fn audio_pids(&mut self) -> HashSet<u32> {
        // TODO: CoreAudio process objects / PipeWire streams.
        HashSet::new()
    }
    fn service_pids(&mut self) -> HashSet<u32> {
        // TODO: launchd / systemd units.
        HashSet::new()
    }
    fn file_info(&mut self, _exe: &str) -> FileInfo {
        FileInfo::default()
    }
    fn signature(&mut self, _exe: &str) -> Signature {
        // TODO(macOS): SecStaticCodeCheckValidity.
        Signature::Other
    }
    fn request_close(&mut self, _pids: &[u32], _include_hidden: bool) {}
    fn wait_exit(&mut self, procs: &[(u32, u64)], timeout: Duration) -> Vec<(u32, u64)> {
        let deadline = Instant::now() + timeout;
        let mut left: Vec<(u32, u64)> = procs.to_vec();
        while !left.is_empty() && Instant::now() < deadline {
            left.retain(|&(p, s)| self.proc_alive(p, s));
            std::thread::sleep(Duration::from_millis(150));
        }
        left
    }
    fn kill(&mut self, pid: u32, start_time: u64) -> Result<(), String> {
        let sys = self.sys.as_mut().ok_or("sysinfo unavailable")?;
        let p = Pid::from_u32(pid);
        sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[p]), true, ProcessRefreshKind::nothing());
        match sys.process(p) {
            Some(pr) if pr.start_time() == start_time => {
                if pr.kill() { Ok(()) } else { Err("kill failed".into()) }
            }
            _ => Err("process already exited".into()),
        }
    }
    fn icon_png(&mut self, _exe: &str) -> Option<Vec<u8>> {
        None
    }
    fn launch(&mut self, exe: &str) -> Result<(), String> {
        std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    fn supports_glass(&self) -> bool {
        false
    }
}
