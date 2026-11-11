//! Tauri composition root: plugins, command registration and manager startup.
mod asr;
mod commands;
mod error;
mod hotkeys;
mod llm;
mod managers;
mod pipeline;
mod platform;
mod provider_test;
mod settings;
mod state;

use crate::managers::{
    audio::AudioManager, enhance::EnhanceManager, history::HistoryManager, inject::InjectManager,
    transcription::TranscriptionManager,
};
use crate::platform::{current_platform, Platform};
use crate::state::StateMachine;
use std::{sync::Arc, thread, time::Duration};
use tauri::Manager;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut log_targets = vec![tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::LogDir {
            file_name: Some("audie".into()),
        },
    )];
    #[cfg(debug_assertions)]
    log_targets.push(tauri_plugin_log::Target::new(
        tauri_plugin_log::TargetKind::Stdout,
    ));

    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(log_targets)
                .level(log_level)
                .max_file_size(5 * 1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Frontend → Rust goes through commands. The hot path itself is not
            // command-driven: global-shortcut events below enter `handle_hotkey`,
            // while settings/keychain/provider-test stay here as explicit UI calls.
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::providers::list_asr_providers,
            commands::providers::list_llm_providers,
            commands::audio::list_microphones,
            commands::audio::auto_input_device,
            commands::history::list_history,
            commands::history::delete_history_entry,
            commands::history::clear_history,
            commands::history::get_usage_stats,
            commands::history::get_daily_usage,
            commands::audio::start_mic_monitor,
            commands::audio::stop_mic_monitor,
            commands::secrets::set_secret,
            commands::secrets::has_secret,
            commands::secrets::get_secret_for_settings,
            commands::secrets::delete_secret,
            #[cfg(debug_assertions)]
            commands::providers::test_doubao_streaming,
            #[cfg(debug_assertions)]
            commands::permissions::start_trigger_probe,
            #[cfg(debug_assertions)]
            commands::permissions::stop_trigger_probe,
            commands::permissions::get_input_monitoring_status,
            commands::permissions::request_input_monitoring_permission,
            commands::permissions::get_microphone_permission_status,
            commands::permissions::request_microphone_permission,
            commands::permissions::get_accessibility_permission_status,
            commands::permissions::request_accessibility_permission,
            commands::permissions::restart_app,
            commands::hotkeys::begin_trigger_capture,
            commands::hotkeys::end_trigger_capture,
            provider_test::connection::test_provider,
            provider_test::models::list_provider_models,
            provider_test::models::discover_local_llm,
            commands::providers::test_doubao_connection,
            // Overlay capsule controls (fe.8b / fe.8c).
            commands::dictation::confirm_recording,
            commands::dictation::cancel_recording,
            commands::dictation::undo_last,
            commands::dictation::retry_last,
            commands::dictation::insert_raw_last,
            commands::dictation::open_main_window,
            commands::history::reenhance_history_entry,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            let platform: Arc<dyn Platform> = Arc::from(current_platform());
            platform.initialize_overlay(app)?;

            let state_machine = Arc::new(StateMachine::new());
            let audio = Arc::new(AudioManager::new());
            let transcription = Arc::new(TranscriptionManager::new());
            let enhance = Arc::new(EnhanceManager::new());
            let history = Arc::new(HistoryManager::new(&app_handle));
            let inject = Arc::new(InjectManager::new(platform.clone()));

            // This is the backend object graph for the P1 pipeline:
            // StateMachine owns legal UI states; Audio captures samples; ASR and
            // LLM managers choose providers from settings; Inject delegates the
            // OS-specific paste/keychain work to Platform.
            // Manage first so `build_hotkey_callback` can resolve managers off
            // the app state — the same callback gets rebuilt when the hotkey
            // changes (commands::settings::update_settings).
            app.manage(state_machine);
            app.manage(audio);
            app.manage(transcription);
            app.manage(enhance);
            app.manage(history);
            app.manage(inject);
            app.manage(platform.clone());
            pipeline::manage_state(app);

            let presentation_app = app_handle.clone();
            let presentation_platform = platform.clone();
            let show_in_dock = settings::load_settings(&app_handle).show_in_dock;
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(150));
                if let Err(err) = presentation_platform.apply_app_icon(&presentation_app) {
                    log::warn!("apply app icon failed: {err:?}");
                }
                if !show_in_dock {
                    if let Err(err) =
                        presentation_platform.set_dock_visible(&presentation_app, false)
                    {
                        log::error!("apply Dock visibility failed: {err:?}");
                    }
                }
            });

            hotkeys::register_configured(&app_handle);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
