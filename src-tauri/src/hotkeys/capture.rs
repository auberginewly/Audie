use super::*;
/// P3.10 trigger recorder: while the Settings recorder is open, stop the live
/// trigger and start the platform capture session. macOS runs a native listen-only
/// tap because the webview cannot see fn; Windows reads keydown in the focused
/// recorder after this command has suspended the global shortcuts.
pub(crate) fn begin_trigger_capture(app: AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.unregister_all_hotkeys(&app)?;
    if let Err(err) = platform.start_trigger_capture(&app) {
        if let Err(restore_err) = restore_configured_hotkeys(&app) {
            log::error!("restore hotkeys after capture start failure: {restore_err}");
        }
        return Err(err);
    }
    Ok(())
}

pub(crate) fn end_trigger_capture(app: AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.stop_trigger_capture();
    restore_configured_hotkeys(&app)
}

pub(crate) fn restore_configured_hotkeys(app: &AppHandle) -> AppResult<()> {
    let platform = app.state::<Arc<dyn Platform>>();
    let hotkey = settings::load_hotkey(app);
    platform.register_hotkey(
        app,
        HotkeySlot::Primary,
        &hotkey,
        build_hotkey_callback(app, HotkeyRole::Primary),
    )?;
    register_compose_hotkey(app);
    Ok(())
}
