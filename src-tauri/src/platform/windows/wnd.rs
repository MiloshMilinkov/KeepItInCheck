//! Top-level window enumeration, graceful close requests and panel window styling.

use super::super::WindowInfo;
use std::collections::HashSet;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
    DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, PostMessageW, GWL_EXSTYLE, GW_OWNER, WM_CLOSE,
    WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

/// Alt-Tab style test for "a window the user can see and interact with". Minimised windows and
/// windows on other virtual desktops count; tool windows and owned popups do not. Erring on the
/// side of "has a window" is deliberate: windowed apps are never force-killed.
fn is_user_window(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        let owned = GetWindow(hwnd, GW_OWNER).map(|o| !o.0.is_null()).unwrap_or(false);
        if owned && ex & WS_EX_APPWINDOW.0 == 0 {
            return false;
        }
        // Untitled windows still count (some apps draw their own caption); only zero-size
        // placeholder windows are ignored.
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r).is_ok() && r.right > r.left && r.bottom > r.top
    }
}

/// UWP apps draw inside a frame owned by ApplicationFrameHost.exe; the real app process owns a
/// child CoreWindow. Collect the PIDs of child windows that belong to another process.
fn hosted_pids(frame: HWND, frame_pid: u32, out: &mut HashSet<u32>) {
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let (frame_pid, out) = &mut *(lparam.0 as *mut (u32, &mut HashSet<u32>));
        let pid = pid_of(hwnd);
        if pid != 0 && pid != *frame_pid {
            out.insert(pid);
        }
        BOOL(1)
    }
    let mut ctx: (u32, &mut HashSet<u32>) = (frame_pid, out);
    unsafe {
        let _ = EnumChildWindows(Some(frame), Some(cb), LPARAM(&mut ctx as *mut _ as isize));
    }
}

/// Calls `f` for every top-level window.
fn for_each_window(mut f: impl FnMut(HWND)) {
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let f = &mut *(lparam.0 as *mut &mut dyn FnMut(HWND));
        f(hwnd);
        BOOL(1)
    }
    let mut dyn_f: &mut dyn FnMut(HWND) = &mut f;
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut dyn_f as *mut _ as isize));
    }
}

pub fn window_info() -> WindowInfo {
    let mut windowed = HashSet::new();
    for_each_window(|hwnd| {
        if is_user_window(hwnd) {
            let pid = pid_of(hwnd);
            windowed.insert(pid);
            hosted_pids(hwnd, pid, &mut windowed);
        }
    });
    let fg = unsafe { GetForegroundWindow() };
    let foreground_pid = (!fg.0.is_null()).then(|| pid_of(fg)).filter(|&p| p != 0);
    WindowInfo { windowed_pids: windowed, foreground_pid }
}

/// Posts WM_CLOSE (the same message the title-bar X sends) to the processes' windows.
/// Apps with unsaved work typically answer with a "Save changes?" prompt instead of exiting.
pub fn post_close(pids: &[u32], include_hidden: bool) {
    let set: HashSet<u32> = pids.iter().copied().collect();
    let mut targets = Vec::new();
    for_each_window(|hwnd| {
        if set.contains(&pid_of(hwnd)) && (include_hidden || is_user_window(hwnd)) {
            targets.push(hwnd);
        }
    });
    for hwnd in targets {
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

/// Windows 11 is build 22000+. Read once; it cannot change while we run.
pub fn is_windows_11() -> bool {
    static WIN11: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WIN11.get_or_init(|| {
        sysinfo::System::kernel_version()
            .ok()
            .and_then(|b| b.trim().parse::<u32>().ok())
            .is_some_and(|b| b >= 22000)
    })
}

/// Shapes the panel window with a rounded window region, so a blurred system backdrop gets the
/// same corners as the CSS frame. `radius` 0 removes the region.
pub fn window_style(hwnd: isize, width: i32, height: i32, radius: i32) {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if is_windows_11() {
            // We draw our own corners; stop DWM from rounding at its own (smaller) radius.
            let pref: DWM_WINDOW_CORNER_PREFERENCE = if radius > 0 { DWMWCP_DONOTROUND } else { DWMWCP_ROUND };
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const _,
                std::mem::size_of_val(&pref) as u32,
            );
        }
        if radius > 0 && width > 0 && height > 0 {
            let rgn = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius * 2, radius * 2);
            // The system owns the region after this call; do not delete it.
            SetWindowRgn(hwnd, Some(rgn), true);
        } else {
            SetWindowRgn(hwnd, None, true);
        }
    }
}
