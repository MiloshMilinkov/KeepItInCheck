//! Everything around the panel window: docking, drag-to-snap, auto-hide, backdrop, global
//! hotkey, tray and startup registration.

use crate::dock::{self, Rect};
use crate::sampler::Shared;
use crate::settings::{Backdrop, Settings};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, Window, WindowEvent, Wry};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const MAIN: &str = "main";
const AUTO_HIDE_AFTER: Duration = Duration::from_millis(900);

#[derive(Default)]
pub struct DockState {
    /// Where we last put the window and that monitor's scale (read by the pointer watcher, so
    /// it never has to ask the UI thread for the window rect).
    placed: Mutex<Option<(Rect, f64)>>,
    dragging: AtomicBool,
    watching: AtomicBool,
}

struct TrayHandles {
    pause: CheckMenuItem<Wry>,
}

/// Payload of the `toggle` event: the UI owns the expand/collapse animation.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Toggle {
    /// `None` flips the current state.
    pub expand: Option<bool>,
    pub focus: bool,
}

fn shared(app: &AppHandle) -> Arc<Shared> {
    app.state::<Arc<Shared>>().inner().clone()
}

fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN)
}

/// Resize/move the window for the current dock settings.
pub fn layout(app: &AppHandle, expanded: bool, focus: bool) {
    let Some(win) = main_window(app) else { return };
    let settings = shared(app).settings();
    *app.state::<DockState>().placed.lock().unwrap() = dock::apply(&win, &settings, expanded);
    if expanded && focus {
        let _ = win.set_focus();
    }
    lower_webview_memory(&win, !expanded);
    if expanded {
        watch_pointer(app.clone());
    }
}

/// Auto-hide support. While the panel is open, check the cursor 5 times a second and emit
/// `auto-hide` once it has been away from the panel for a moment. Done natively because the
/// webview never sees the cursor when the panel was opened from the screen edge (the cursor
/// sits in the gap between panel and edge) or by hotkey. The thread ends when the panel closes.
fn watch_pointer(app: AppHandle) {
    if app.state::<DockState>().watching.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut outside_since: Option<Instant> = None;
        let mut last_emit: Option<Instant> = None;
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let sh = shared(&app);
            if !sh.expanded.load(Ordering::Relaxed) {
                break;
            }
            let ds = app.state::<DockState>();
            if !sh.settings.read().unwrap().auto_hide || ds.dragging.load(Ordering::Relaxed) {
                outside_since = None;
                continue;
            }
            let (Some((r, scale)), Some((cx, cy))) = (*ds.placed.lock().unwrap(), dock::cursor_pos()) else {
                continue;
            };
            // The margin between panel and screen edge counts as inside.
            let pad = (dock::MARGIN * scale).ceil() as i32 + 2;
            let inside = cx >= r.x - pad && cx < r.x + r.w + pad && cy >= r.y - pad && cy < r.y + r.h + pad;
            if inside || dock::mouse_down() {
                outside_since = None;
                continue;
            }
            let since = *outside_since.get_or_insert_with(Instant::now);
            // Repeat while outside: the UI ignores it while a sheet is open, then acts once closed.
            if since.elapsed() >= AUTO_HIDE_AFTER && last_emit.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
                last_emit = Some(Instant::now());
                let _ = app.emit("auto-hide", ());
            }
        }
        app.state::<DockState>().watching.store(false, Ordering::SeqCst);
    });
}

/// The user pressed on the title bar: let them drag the panel, then snap it to the nearest edge
/// of whichever monitor they dropped it on. Only an explicit drag re-docks, so monitor changes
/// or OS window moves never overwrite the user's chosen edge.
pub fn begin_drag(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    let ds = app.state::<DockState>();
    if ds.dragging.swap(true, Ordering::SeqCst) {
        return;
    }
    let start = win.outer_position().ok();
    if win.start_dragging().is_err() {
        ds.dragging.store(false, Ordering::SeqCst);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let t0 = Instant::now();
        while dock::mouse_down() && t0.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(60));
        }
        if let Some(win) = main_window(&app) {
            // A click without movement is not a drag: leave the dock alone.
            let moved = win.outer_position().ok() != start;
            if moved {
                if let Some((edge, monitor)) = dock::snap_target(&win) {
                    let sh = shared(&app);
                    if let Ok((old, new)) = sh.update_settings(|s| {
                        s.edge = edge;
                        s.monitor = monitor;
                    }) {
                        apply_settings(&app, &old, &new);
                    }
                }
                layout(&app, true, false);
            }
        }
        app.state::<DockState>().dragging.store(false, Ordering::SeqCst);
    });
}

/// While collapsed the UI is a 6 px strip, so ask WebView2 to trim its memory.
#[cfg(windows)]
fn lower_webview_memory(win: &WebviewWindow, low: bool) {
    let _ = win.with_webview(move |wv| unsafe {
        use webview2_com::Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
        };
        use windows::core::Interface;
        if let Ok(core) = wv.controller().CoreWebView2() {
            if let Ok(core19) = core.cast::<ICoreWebView2_19>() {
                let level = if low {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                } else {
                    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                };
                let _ = core19.SetMemoryUsageTargetLevel(level);
            }
        }
    });
}

#[cfg(not(windows))]
fn lower_webview_memory(_win: &WebviewWindow, _low: bool) {}

/// Apply (or clear) the Acrylic backdrop.
pub fn apply_backdrop(app: &AppHandle, settings: &Settings) {
    let Some(win) = main_window(app) else { return };
    if settings.backdrop == Backdrop::Glass && crate::platform::create().supports_glass() {
        let _ = win.set_effects(EffectsBuilder::new().effect(Effect::Acrylic).build());
    } else {
        let _ = win.set_effects(None);
    }
}

pub fn register_hotkey(app: &AppHandle, old: Option<&str>, new: &str) {
    let gs = app.global_shortcut();
    if let Some(old) = old.and_then(|s| s.parse::<Shortcut>().ok()) {
        let _ = gs.unregister(old);
    }
    if let Ok(sc) = new.parse::<Shortcut>() {
        if let Err(e) = gs.register(sc) {
            eprintln!("hotkey {new} not registered: {e}");
        }
    }
}

pub fn hotkey_pressed(app: &AppHandle, state: ShortcutState) {
    if state == ShortcutState::Pressed {
        let _ = app.emit("toggle", Toggle { expand: None, focus: true });
    }
}

pub fn set_autostart(app: &AppHandle, on: bool) {
    let al = app.autolaunch();
    let _ = if on { al.enable() } else { al.disable() };
}

/// React to a settings change (after `Shared::update_settings` stored it).
pub fn apply_settings(app: &AppHandle, old: &Settings, new: &Settings) {
    if old.hotkey != new.hotkey {
        register_hotkey(app, Some(&old.hotkey), &new.hotkey);
    }
    if old.launch_at_startup != new.launch_at_startup {
        set_autostart(app, new.launch_at_startup);
    }
    if old.backdrop != new.backdrop {
        apply_backdrop(app, new);
    }
    if old.edge != new.edge || old.monitor != new.monitor || old.compact != new.compact {
        let expanded = shared(app).expanded.load(Ordering::Relaxed);
        layout(app, expanded, false);
    }
    let _ = app.emit("settings-changed", new);
}

/// The one place monitoring is paused or resumed (tray and UI both call this).
pub fn set_paused(app: &AppHandle, paused: bool) {
    let sh = shared(app);
    sh.paused.store(paused, Ordering::Relaxed);
    if let Some(h) = app.try_state::<TrayHandles>() {
        let _ = h.pause.set_checked(paused);
    }
    let _ = app.emit("paused", paused);
    sh.wake();
}

pub fn quit(app: &AppHandle) {
    shared(app).quit();
    app.exit(0);
}

pub fn monitor_names(app: &AppHandle) -> Vec<String> {
    main_window(app)
        .and_then(|w| w.available_monitors().ok())
        .map(|ms| ms.iter().filter_map(|m| m.name().cloned()).collect())
        .unwrap_or_default()
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let clean = MenuItem::with_id(app, "cleanup", "Clean Up…", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause monitoring", true, false, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &clean, &pause, &sep, &quit_item])?;
    app.manage(TrayHandles { pause });

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("KeepItInCheck")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                let _ = app.emit("toggle", Toggle { expand: Some(true), focus: true });
            }
            "cleanup" => {
                let _ = app.emit("open-cleanup", ());
            }
            "pause" => set_paused(app, !shared(app).paused.load(Ordering::Relaxed)),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let _ = tray.app_handle().emit("toggle", Toggle { expand: None, focus: true });
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Re-dock after a DPI change (moving between monitors with different scaling).
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::ScaleFactorChanged { .. } = event {
        let app = window.app_handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            let expanded = shared(&app).expanded.load(Ordering::Relaxed);
            layout(&app, expanded, false);
        });
    }
}
