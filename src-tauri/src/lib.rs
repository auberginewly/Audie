// Tauri entry. `main.rs` only calls `run()` — all setup happens here.
//
// P0.1 wiring:
//   - global-shortcut plugin → dispatches into `HotkeyRegistry`
//   - Press → state Idle→Recording → show overlay window
//   - Release → state Recording→Idle → hide overlay window
//   - Overlay window positioned bottom-center, click-through

mod asr;
mod error;
mod managers;
mod platform;
mod state;

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};
use tauri_plugin_global_shortcut::ShortcutState;

use crate::asr::AudioData;
use crate::error::{AppError, AppResult};
use crate::managers::audio::AudioManager;
use crate::managers::transcription::TranscriptionManager;
use crate::platform::{current_platform, HotkeyEvent, HotkeyRegistry, Platform};
use crate::state::{AppState, StateMachine};

const SUCCESS_HOLD_MS: u64 = 150;
const ERROR_HOLD_MS: u64 = 2500;

#[derive(Serialize, Clone)]
struct FinalTranscript {
    text: String,
    duration_ms: u64,
}

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
            let transcription = Arc::new(TranscriptionManager::new());
            let platform = current_platform(registry_for_setup.clone());

            let state_for_cb = state_machine.clone();
            let audio_for_cb = audio.clone();
            let transcription_for_cb = transcription.clone();
            let app_for_cb = app_handle.clone();
            if let Err(err) = platform.register_hotkey(
                &app_handle,
                DEFAULT_HOTKEY,
                Box::new(move |event| {
                    handle_hotkey(
                        &app_for_cb,
                        &state_for_cb,
                        &audio_for_cb,
                        &transcription_for_cb,
                        event,
                    );
                }),
            ) {
                log::error!("register hotkey {DEFAULT_HOTKEY}: {err:?}");
                return Err(Box::new(std::io::Error::other(format!("{err:?}"))));
            }

            log::info!("registered global hotkey {DEFAULT_HOTKEY}");

            // Stash for future managers / commands.
            app.manage(state_machine);
            app.manage(audio);
            app.manage(transcription);
            app.manage(registry_for_setup.clone());
            let platform_arc: Arc<dyn Platform> = Arc::from(platform);
            app.manage(platform_arc);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_hotkey(
    app: &AppHandle,
    state: &Arc<StateMachine>,
    audio: &Arc<AudioManager>,
    transcription: &Arc<TranscriptionManager>,
    event: HotkeyEvent,
) {
    match event {
        HotkeyEvent::Pressed => {
            if state.transition(app, AppState::Recording, Some("hotkey-down")) {
                if let Err(err) = audio.start_capture(app.clone()) {
                    // P0.6 will route this to the UI as an AppError event.
                    // For now we just log; the overlay still shows but bars
                    // will stay flat.
                    log::error!("start capture: {err:?}");
                }
                if let Err(err) = show_overlay(app) {
                    log::error!("show overlay: {err:?}");
                }
            }
        }
        HotkeyEvent::Released => {
            if !state.transition(app, AppState::Processing, Some("hotkey-up")) {
                return;
            }
            if let Err(err) = hide_overlay(app) {
                log::error!("hide overlay: {err:?}");
            }
            match audio.stop_capture() {
                Ok(recorded) => {
                    spawn_transcription(
                        app.clone(),
                        state.clone(),
                        transcription.clone(),
                        recorded,
                    );
                }
                Err(err) => {
                    log::error!("stop capture: {err:?}");
                    enter_error(app.clone(), state.clone(), err);
                }
            }
        }
    }
}

/// Run the (blocking) transcription off the hotkey thread, then drive the
/// pipeline tail: print the text, emit `final-transcript`, flash Success and
/// settle to Idle. Errors flash red (Error) and recover. PROJECT_SPEC.md §3.3.
fn spawn_transcription(
    app: AppHandle,
    state: Arc<StateMachine>,
    transcription: Arc<TranscriptionManager>,
    audio: AudioData,
) {
    thread::spawn(move || {
        let duration_ms = duration_ms(&audio);
        match transcription.transcribe(&audio) {
            Ok(text) => {
                // P0.3 acceptance: the transcript shows up in the console.
                println!("[transcript] {text}");
                log::info!("transcript ({duration_ms} ms): {text}");
                let _ = app.emit("final-transcript", FinalTranscript { text, duration_ms });
                state.transition(&app, AppState::Success, Some("transcribed"));
                thread::sleep(Duration::from_millis(SUCCESS_HOLD_MS));
                state.transition(&app, AppState::Idle, Some("done"));
            }
            Err(err) => {
                log::error!("transcription failed: {err:?}");
                enter_error(app, state, err);
            }
        }
    });
}

/// Emit the error, flash Error, and recover to Idle after a hold. Spawns its own
/// thread so callers on the hotkey path don't block.
fn enter_error(app: AppHandle, state: Arc<StateMachine>, err: AppError) {
    let _ = app.emit("error", &err);
    state.transition(&app, AppState::Error, Some("error"));
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(ERROR_HOLD_MS));
        state.transition(&app, AppState::Idle, Some("recovered"));
    });
}

fn duration_ms(audio: &AudioData) -> u64 {
    let denom = audio.sample_rate as u64 * audio.channels.max(1) as u64;
    if denom == 0 {
        0
    } else {
        audio.samples.len() as u64 * 1000 / denom
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
