//! Edge docking: where the panel and the collapsed strip go on a monitor.
//!
//! All geometry is in *physical* pixels of the target monitor, computed from its work area (the
//! screen minus the taskbar) and its own scale factor, so docking is correct on mixed-DPI
//! multi-monitor setups. [`layout`] is pure and unit-tested; [`apply`] moves the real window.

use crate::settings::{Edge, Settings};
use tauri::{Monitor, WebviewWindow};

/// Panel width when docked left/right (logical px).
pub const PANEL_W: f64 = 360.0;
/// Compact mode height (logical px).
pub const COMPACT_H: f64 = 492.0;
/// Panel size when docked top/bottom (logical px).
pub const WIDE_W: f64 = 780.0;
pub const WIDE_H: f64 = 380.0;
pub const WIDE_COMPACT_H: f64 = 300.0;
/// Collapsed strip thickness (logical px) and how much of the edge it spans.
pub const STRIP: f64 = 6.0;
pub const STRIP_FRACTION: f64 = 0.3;
/// Gap between the floating panel and the screen edges (logical px).
pub const MARGIN: f64 = 8.0;
/// Corner radius of the panel (logical px).
pub const RADIUS: f64 = 14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

fn px(v: f64, scale: f64) -> i32 {
    (v * scale).round() as i32
}

/// Where the window goes for a given work area, scale, edge and state.
pub fn layout(work: Rect, scale: f64, edge: Edge, expanded: bool, compact: bool) -> Rect {
    let m = px(MARGIN, scale);
    let vertical = matches!(edge, Edge::Left | Edge::Right);
    if !expanded {
        let thick = px(STRIP, scale).max(3);
        return if vertical {
            let h = ((work.h as f64 * STRIP_FRACTION) as i32).max(px(120.0, scale)).min(work.h);
            let x = if edge == Edge::Left { work.x } else { work.x + work.w - thick };
            Rect { x, y: work.y + (work.h - h) / 2, w: thick, h }
        } else {
            let w = ((work.w as f64 * STRIP_FRACTION) as i32).max(px(160.0, scale)).min(work.w);
            let y = if edge == Edge::Top { work.y } else { work.y + work.h - thick };
            Rect { x: work.x + (work.w - w) / 2, y, w, h: thick }
        };
    }
    if vertical {
        let w = px(PANEL_W, scale).min(work.w - 2 * m);
        let full_h = work.h - 2 * m;
        let h = if compact { px(COMPACT_H, scale).min(full_h) } else { full_h };
        let x = if edge == Edge::Left { work.x + m } else { work.x + work.w - w - m };
        let y = work.y + (work.h - h) / 2;
        Rect { x, y, w, h }
    } else {
        let w = px(WIDE_W, scale).min(work.w - 2 * m);
        let h = px(if compact { WIDE_COMPACT_H } else { WIDE_H }, scale).min(work.h - 2 * m);
        let x = work.x + (work.w - w) / 2;
        let y = if edge == Edge::Top { work.y + m } else { work.y + work.h - h - m };
        Rect { x, y, w, h }
    }
}

/// The edge of `work` closest to a point (used to snap after the user drags the panel).
pub fn nearest_edge(work: Rect, x: i32, y: i32) -> Edge {
    let d = [
        (Edge::Left, (x - work.x).abs()),
        (Edge::Right, (work.x + work.w - x).abs()),
        (Edge::Top, (y - work.y).abs()),
        (Edge::Bottom, (work.y + work.h - y).abs()),
    ];
    d.into_iter().min_by_key(|&(_, dist)| dist).map(|(e, _)| e).unwrap_or(Edge::Right)
}

fn work_rect(m: &Monitor) -> Rect {
    let wa = m.work_area();
    Rect { x: wa.position.x, y: wa.position.y, w: wa.size.width as i32, h: wa.size.height as i32 }
}

/// The monitor the user picked, else the primary one.
pub fn target_monitor(win: &WebviewWindow, settings: &Settings) -> Option<Monitor> {
    if let Some(name) = settings.monitor.as_deref() {
        if let Ok(list) = win.available_monitors() {
            if let Some(m) = list.into_iter().find(|m| m.name().map(String::as_str) == Some(name)) {
                return Some(m);
            }
        }
    }
    win.primary_monitor().ok().flatten().or_else(|| win.current_monitor().ok().flatten())
}

/// The monitor containing a physical point.
pub fn monitor_at(win: &WebviewWindow, x: i32, y: i32) -> Option<Monitor> {
    let list = win.available_monitors().ok()?;
    list.into_iter().find(|m| {
        let p = m.position();
        let s = m.size();
        x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
    })
}

/// Move/resize the real window. Returns the rect and the monitor scale used.
pub fn apply(win: &WebviewWindow, settings: &Settings, expanded: bool) -> Option<(Rect, f64)> {
    let mon = target_monitor(win, settings)?;
    let scale = mon.scale_factor();
    let r = layout(work_rect(&mon), scale, settings.edge, expanded, settings.compact);
    set_bounds(win, r);
    let radius = if expanded { px(RADIUS, scale) } else { r.w.min(r.h) / 2 };
    style(win, r, radius);
    Some((r, scale))
}

#[cfg(windows)]
fn set_bounds(win: &WebviewWindow, r: Rect) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE};
    if let Ok(hwnd) = win.hwnd() {
        // One call for size + position avoids a visible two-step jump.
        unsafe {
            let _ = SetWindowPos(HWND(hwnd.0), Some(HWND_TOPMOST), r.x, r.y, r.w, r.h, SWP_NOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
fn set_bounds(win: &WebviewWindow, r: Rect) {
    use tauri::{PhysicalPosition, PhysicalSize};
    let _ = win.set_size(PhysicalSize::new(r.w as u32, r.h as u32));
    let _ = win.set_position(PhysicalPosition::new(r.x, r.y));
}

#[cfg(windows)]
fn style(win: &WebviewWindow, r: Rect, radius: i32) {
    if let Ok(hwnd) = win.hwnd() {
        crate::platform::window_style(hwnd.0 as isize, r.w, r.h, radius);
    }
}

#[cfg(not(windows))]
fn style(_win: &WebviewWindow, _r: Rect, _radius: i32) {}

/// Current cursor position in physical pixels.
#[cfg(windows)]
pub fn cursor_pos() -> Option<(i32, i32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

#[cfg(not(windows))]
pub fn cursor_pos() -> Option<(i32, i32)> {
    None
}

/// True while the left mouse button is held (a drag is still in progress).
#[cfg(windows)]
pub fn mouse_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
pub fn mouse_down() -> bool {
    false
}

/// Pick the edge and monitor nearest to where the user dropped the panel.
pub fn snap_target(win: &WebviewWindow) -> Option<(Edge, Option<String>)> {
    let (x, y) = cursor_pos().or_else(|| {
        let p = win.outer_position().ok()?;
        let s = win.outer_size().ok()?;
        Some((p.x + s.width as i32 / 2, p.y + s.height as i32 / 2))
    })?;
    let mon = monitor_at(win, x, y).or_else(|| win.current_monitor().ok().flatten())?;
    Some((nearest_edge(work_rect(&mon), x, y), mon.name().cloned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect { x: 0, y: 0, w: 1920, h: 1032 };

    #[test]
    fn right_dock_full_height_at_100_percent() {
        let r = layout(WORK, 1.0, Edge::Right, true, false);
        assert_eq!(r, Rect { x: 1920 - 360 - 8, y: 8, w: 360, h: 1032 - 16 });
    }

    #[test]
    fn left_dock_scales_with_dpi() {
        let r = layout(WORK, 1.5, Edge::Left, true, false);
        assert_eq!(r.w, 540);
        assert_eq!(r.x, 12);
    }

    #[test]
    fn collapsed_strip_hugs_the_edge() {
        let r = layout(WORK, 1.0, Edge::Right, false, false);
        assert_eq!(r.x + r.w, 1920, "flush with the right edge");
        assert_eq!(r.w, 6);
        assert!(r.h >= 120 && r.h < WORK.h);
        let t = layout(WORK, 1.0, Edge::Top, false, false);
        assert_eq!(t.y, 0);
        assert_eq!(t.h, 6);
    }

    #[test]
    fn secondary_monitor_offsets_are_respected() {
        // A monitor to the left of the primary has negative coordinates.
        let work = Rect { x: -2560, y: -200, w: 2560, h: 1400 };
        let r = layout(work, 1.0, Edge::Left, true, false);
        assert_eq!(r.x, -2560 + 8);
        assert_eq!(r.y, -200 + 8);
    }

    #[test]
    fn compact_is_shorter_and_centered() {
        let r = layout(WORK, 1.0, Edge::Left, true, true);
        assert_eq!(r.h, 492);
        assert_eq!(r.y, (1032 - 492) / 2);
    }

    #[test]
    fn top_and_bottom_are_wide() {
        let r = layout(WORK, 1.0, Edge::Bottom, true, false);
        assert_eq!(r.w, 780);
        assert_eq!(r.y + r.h, 1032 - 8);
    }

    #[test]
    fn nearest_edge_picks_closest() {
        assert_eq!(nearest_edge(WORK, 5, 500), Edge::Left);
        assert_eq!(nearest_edge(WORK, 1900, 500), Edge::Right);
        assert_eq!(nearest_edge(WORK, 900, 3), Edge::Top);
        assert_eq!(nearest_edge(WORK, 900, 1020), Edge::Bottom);
    }
}
