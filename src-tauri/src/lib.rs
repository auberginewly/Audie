// Tauri entry. `main.rs` only calls `run()` — all setup happens here.
//
// P0.1 wiring:
//   - global-shortcut plugin → dispatches into `HotkeyRegistry`
//   - Press → state Idle→Recording → show overlay window
//   - Release → state Recording→Idle → hide overlay window
//   - Overlay window positioned bottom-center, click-through

mod error;
mod managers;
mod platform;
mod state;

use std::sync::Arc;

use tauri::{AppHandle, Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::platform::{current_platform, HotkeyEvent, HotkeyRegistry, Platform};
use crate::state::{AppState, StateMachine};

const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
const OVERLAY_WINDOW_LABEL: &str = "overlay";
const OVERLAY_BOTTOM_MARGIN_PX: f64 = 16.0;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();

    let registry = Arc::new(HotkeyRegistry::default());
    let registry_for_handler = registry.clone();
    let registry_for_setup = registry.clone();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |_app, shortcut, event| {
                    let hk = match event.state() {
                        ShortcutState::Pressed => HotkeyEvent::Pressed,
                        ShortcutState::Released => HotkeyEvent::Released,
                    };
                    registry_for_handler.dispatch(shortcut, hk);
                })
                .build(),
        )
        .setup(move |app| {
            let app_handle = app.handle().clone();

            if let Err(err) = position_overlay(&app_handle) {
                log::error!("position overlay failed: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
            }

            let state_machine = Arc::new(StateMachine::new());
            let audio = Arc::new(AudioManager::new());
            let platform = current_platform(registry_for_setup.clone());

            let state_for_cb = state_machine.clone();
            let audio_for_cb = audio.clone();
            let app_for_cb = app_handle.clone();
            if let Err(err) = platform.register_hotkey(
                &app_handle,
                DEFAULT_HOTKEY,
                Box::new(move |event| {
                    handle_hotkey(&app_for_cb, &state_for_cb, &audio_for_cb, event);
                }),
            ) {
                log::error!("register hotkey {DEFAULT_HOTKEY}: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
            }

            log::info!("registered global hotkey {DEFAULT_HOTKEY}");

            // Stash for future managers / commands.
            app.manage(state_machine);
            app.manage(audio);
            app.manage(registry_for_setup.clone());
            let platform_arc: Arc<dyn Platform> = Arc::from(platform);
            app.manage(platform_arc);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_hotkey(app: &AppHandle, state: &StateMachine, audio: &AudioManager, event: HotkeyEvent) {
    match event {
        HotkeyEvent::Pressed => {
            if state.transition(app, AppState::Recording, Some("hotkey-down")) {
                if let Err(err) = audio.start_capture(app.clone()) {
                    // P0.7 will route this to the UI as an AppError event.
                    // For P0.2 we just log; the overlay still shows but bars
                    // will stay flat.
                    log::error!("start capture: {err:?}");
                }
                if let Err(err) = show_overlay(app) {
                    log::error!("show overlay: {err:?}");
                }
            }
        }
        HotkeyEvent::Released => {
            // P0.1 short-circuit: no transcription pipeline yet, so we go
            // Recording → Idle directly. P0.4+ will go Recording → Processing.
            if state.transition(app, AppState::Idle, Some("hotkey-up")) {
                if let Err(err) = audio.stop_capture() {
                    log::error!("stop capture: {err:?}");
                }
                if let Err(err) = hide_overlay(app) {
                    log::error!("hide overlay: {err:?}");
                }
            }
        }
    }
}

fn position_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;

    let monitor = overlay
        .primary_monitor()
        .map_err(|err| AppError::Internal(format!("primary_monitor: {err}")))?
        .ok_or_else(|| AppError::Internal("no primary monitor".into()))?;

    let monitor_size = monitor.size();
    let scale = monitor.scale_factor();

    let win_size = overlay
        .outer_size()
        .map_err(|err| AppError::Internal(format!("outer_size: {err}")))?;

    let bottom_margin_px = (OVERLAY_BOTTOM_MARGIN_PX * scale).round() as i32;
    let x = (monitor_size.width as i32 - win_size.width as i32) / 2;
    let y = monitor_size.height as i32 - win_size.height as i32 - bottom_margin_px;

    overlay
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|err| AppError::Internal(format!("set_position: {err}")))?;

    // Click-through — the capsule must not steal events from the underlying app.
    overlay
        .set_ignore_cursor_events(true)
        .map_err(|err| AppError::Internal(format!("set_ignore_cursor_events: {err}")))?;

    Ok(())
}

fn show_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;
    overlay
        .show()
        .map_err(|err| AppError::Internal(format!("show overlay: {err}")))?;
    Ok(())
}

fn hide_overlay(app: &AppHandle) -> AppResult<()> {
    let overlay = app
        .get_webview_window(OVERLAY_WINDOW_LABEL)
        .ok_or_else(|| AppError::Internal("overlay window not found".into()))?;
    overlay
        .hide()
        .map_err(|err| AppError::Internal(format!("hide overlay: {err}")))?;
    Ok(())
}
