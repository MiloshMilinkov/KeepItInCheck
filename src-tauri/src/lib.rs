/*
 * KeepItInCheck
 * Copyright (c) 2026 Miloš Milinkov. All rights reserved.
 * Proprietary and confidential. Use, copying, modification or distribution
 * without written permission is prohibited. See the LICENSE file.
 */

//! KeepItInCheck: an edge-docked monitor that shows which apps eat RAM and CPU, finds the ones
//! adding nothing, and closes them safely.
//!
//! Module map:
//! - `sampler`    background thread: OS data -> grouped, classified [`model::Snapshot`]
//! - `grouping`   processes -> applications
//! - `history`    rolling per-app CPU / I/O / focus / audio history
//! - `classifier` dead-weight score and Safe / Probable / Keep verdicts (unit-tested)
//! - `protect`    protected and cautious process lists
//! - `cleanup`    graceful close, 5 s grace, background-only force kill, log, relaunch
//! - `platform`   OS abstraction (Windows implementation + stubs for other OSes)
//! - `dock`/`shell` edge docking, tray, hotkey, backdrop, autostart
//! - `commands`   IPC surface for the React UI

mod classifier;
mod cleanup;
mod commands;
mod dock;
mod grouping;
mod history;
mod model;
mod platform;
mod protect;
mod sampler;
mod settings;
mod shell;
mod util;

use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch just opens the panel of the running instance.
            let _ = app.emit("toggle", shell::Toggle { expand: Some(true), focus: true });
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| shell::hotkey_pressed(app, event.state))
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let settings_path = config_dir.join("settings.json");
            let settings = settings::Settings::load(&settings_path);

            let shared = sampler::Shared::new(settings.clone(), settings_path);
            // Started from Windows startup: begin collapsed on the edge.
            let start_collapsed = std::env::args().any(|a| a == "--autostart");
            shared.expanded.store(!start_collapsed, Ordering::Relaxed);
            app.manage(shared.clone());
            app.manage(shell::DockState::default());

            shell::setup_tray(&handle)?;
            shell::register_hotkey(&handle, None, &settings.hotkey);
            shell::apply_backdrop(&handle, &settings);
            shell::layout(&handle, !start_collapsed, false);
            if let Some(win) = app.get_webview_window(shell::MAIN) {
                let _ = win.show();
            }
            sampler::spawn(handle, shared);
            Ok(())
        })
        .on_window_event(shell::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::get_settings,
            commands::save_settings,
            commands::set_expanded,
            commands::set_paused,
            commands::set_detail,
            commands::begin_drag,
            commands::app_icon,
            commands::run_cleanup,
            commands::force_close,
            commands::relaunch,
            commands::cleanup_log,
            commands::platform_info,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KeepItInCheck");
}
