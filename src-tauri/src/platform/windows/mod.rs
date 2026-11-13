//! Windows back-end.
//!
//! The process table comes from a single `NtQuerySystemInformation(SystemProcessInformation)`
//! call per tick, the same source Task Manager uses. It returns PID, parent, session, CPU times,
//! private working set and I/O counters for every process at once, so the monitor costs one
//! syscall per tick instead of several per process.

mod audio;
mod icon;
mod services;
mod signature;
mod version;
mod wnd;

pub use wnd::window_style;

use super::{FileInfo, Platform, Signature, WindowInfo};
use crate::model::ProcSample;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, STATUS_INFO_LENGTH_MISMATCH, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// Full x64/x86 layout of `SYSTEM_PROCESS_INFORMATION` (the SDK header hides most fields as
/// `Reserved`). Stable since Windows Vista; used by Task Manager and System Informer.
#[repr(C)]
struct SpInfo {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name_len: u16,
    image_name_max: u16,
    image_name_buf: *const u16,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_page_count: usize,
    read_operation_count: i64,
    write_operation_count: i64,
    other_operation_count: i64,
    read_transfer_count: i64,
    write_transfer_count: i64,
    other_transfer_count: i64,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<SpInfo>() == 0x100);

/// FILETIME (100 ns since 1601) to unix seconds.
pub(crate) fn filetime_to_unix(ft: i64) -> u64 {
    const EPOCH_DIFF_SECS: i64 = 11_644_473_600;
    (ft / 10_000_000 - EPOCH_DIFF_SECS).max(0) as u64
}

/// Null-terminated UTF-16 for Win32 calls.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Closes a HANDLE when dropped.
pub(crate) struct Owned(pub HANDLE);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Opens a process and checks it is still the instance that started at `start_time`.
fn open_same_process(pid: u32, start_time: u64, access: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS) -> Option<Owned> {
    unsafe {
        let h = Owned(OpenProcess(access | PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        GetProcessTimes(h.0, &mut c, &mut e, &mut k, &mut u).ok()?;
        let created = ((c.dwHighDateTime as i64) << 32) | c.dwLowDateTime as i64;
        (filetime_to_unix(created) == start_time).then_some(h)
    }
}

pub struct WinPlatform {
    buf: Vec<u8>,
    audio: audio::AudioProbe,
}

impl WinPlatform {
    /// Cheap to create: the process-table buffer is only allocated by the sampler's first tick,
    /// so one-off instances (icons, relaunch, cleanup) cost nothing.
    pub fn new() -> Self {
        Self { buf: Vec::new(), audio: audio::AudioProbe::default() }
    }
}

impl Platform for WinPlatform {
    fn process_table(&mut self) -> Option<Vec<ProcSample>> {
        let mut needed = 0u32;
        if self.buf.is_empty() {
            self.buf.resize(512 * 1024, 0);
        }
        // Grow the buffer until the whole table fits (it is reused across ticks).
        for _ in 0..8 {
            let status = unsafe {
                NtQuerySystemInformation(
                    SystemProcessInformation,
                    self.buf.as_mut_ptr().cast(),
                    self.buf.len() as u32,
                    &mut needed,
                )
            };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                let grow = (needed as usize).max(self.buf.len()) + 64 * 1024;
                self.buf.resize(grow, 0);
                continue;
            }
            if status.is_err() {
                return None;
            }
            break;
        }

        let mut out = Vec::with_capacity(400);
        let mut offset = 0usize;
        loop {
            if offset + std::mem::size_of::<SpInfo>() > self.buf.len() {
                break;
            }
            // SAFETY: the kernel wrote a linked list of SpInfo records into `buf`.
            let info = unsafe { &*(self.buf.as_ptr().add(offset) as *const SpInfo) };
            let pid = info.unique_process_id as u32;
            let name = if info.image_name_buf.is_null() || info.image_name_len == 0 {
                match pid {
                    0 => "System Idle Process".to_string(),
                    _ => "System".to_string(),
                }
            } else {
                let len = info.image_name_len as usize / 2;
                let slice = unsafe { std::slice::from_raw_parts(info.image_name_buf, len) };
                String::from_utf16_lossy(slice)
            };
            out.push(ProcSample {
                pid,
                ppid: info.inherited_from_unique_process_id as u32,
                name,
                exe: None,
                start_time: filetime_to_unix(info.create_time),
                session_id: Some(info.session_id),
                cpu: 0.0,
                cpu_time_ms: ((info.user_time + info.kernel_time).max(0) / 10_000) as u64,
                mem: info.working_set_private_size.max(0) as u64,
                io_bytes: (info.read_transfer_count + info.write_transfer_count + info.other_transfer_count)
                    .max(0) as u64,
            });
            if info.next_entry_offset == 0 {
                break;
            }
            offset += info.next_entry_offset as usize;
        }
        // Drop the idle pseudo-process: it is not an app and its "CPU" is free capacity.
        out.retain(|p| p.pid != 0);
        Some(out)
    }

    fn exe_path(&mut self, pid: u32) -> Option<String> {
        unsafe {
            let h = Owned(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
            Some(String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    fn windows(&mut self) -> WindowInfo {
        wnd::window_info()
    }

    fn audio_pids(&mut self) -> HashSet<u32> {
        self.audio.playing_pids()
    }

    fn service_pids(&mut self) -> HashSet<u32> {
        services::service_pids()
    }

    fn file_info(&mut self, exe: &str) -> FileInfo {
        version::file_info(exe)
    }

    fn signature(&mut self, exe: &str) -> Signature {
        signature::check(exe)
    }

    fn request_close(&mut self, pids: &[u32], include_hidden: bool) {
        wnd::post_close(pids, include_hidden);
    }

    fn wait_exit(&mut self, procs: &[(u32, u64)], timeout: Duration) -> Vec<(u32, u64)> {
        // Hold one SYNCHRONIZE handle per process; a process we can no longer open has exited.
        let mut pending: Vec<((u32, u64), Owned)> = procs
            .iter()
            .filter_map(|&(pid, st)| open_same_process(pid, st, PROCESS_SYNCHRONIZE).map(|h| ((pid, st), h)))
            .collect();
        let deadline = Instant::now() + timeout;
        loop {
            pending.retain(|(_, h)| unsafe { WaitForSingleObject(h.0, 0) } != WAIT_OBJECT_0);
            if pending.is_empty() || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        pending.into_iter().map(|(p, _)| p).collect()
    }

    fn kill(&mut self, pid: u32, start_time: u64) -> Result<(), String> {
        let h = open_same_process(pid, start_time, PROCESS_TERMINATE)
            .ok_or_else(|| "process already exited or access denied".to_string())?;
        unsafe { TerminateProcess(h.0, 1) }.map_err(|e| e.message().to_string())
    }

    fn icon_png(&mut self, exe: &str) -> Option<Vec<u8>> {
        icon::icon_png(exe)
    }

    fn launch(&mut self, exe: &str) -> Result<(), String> {
        let file = wide(exe);
        let dir = std::path::Path::new(exe)
            .parent()
            .map(|d| wide(&d.to_string_lossy()))
            .unwrap_or_else(|| vec![0]);
        let r = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(file.as_ptr()),
                PCWSTR::null(),
                PCWSTR(dir.as_ptr()),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute returns a value > 32 on success.
        if r.0 as isize > 32 {
            Ok(())
        } else {
            Err(format!("could not start {exe}"))
        }
    }

    fn supports_glass(&self) -> bool {
        wnd::is_windows_11()
    }
}
