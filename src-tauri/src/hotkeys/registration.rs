use super::*;
use crate::settings::Settings;
/// Register the 写作 (compose) trigger when enabled + configured. A missing Input
/// Monitoring grant only logs (like the primary trigger) so startup / capture-restore
/// never abort. Reads the persisted settings, so callers must persist before calling
/// (startup + capture-restore both do).
pub(super) fn register_compose_hotkey(app: &AppHandle) {
    let settings = settings::load_settings(app);
    if settings.compose_hotkey.trim().is_empty() {
        return;
    }
    let platform = app.state::<Arc<dyn Platform>>();
    match platform.register_hotkey(
        app,
        HotkeySlot::Compose,
        &settings.compose_hotkey,
        build_hotkey_callback(app, HotkeyRole::Compose),
    ) {
        Ok(()) => log::info!("registered 写作键 {}", settings.compose_hotkey),
        Err(err) => log::warn!(
            "register 写作键 {} failed (grant Input Monitoring then relaunch): {err:?}",
            settings.compose_hotkey
        ),
    }
}

/// Re-register triggers whose binding changed — the primary (fn) key and the 写作键
/// independently. Re-register before persist so a failure leaves the store untouched
/// (no "saved but not active" mismatch). 写作键 also re-registers when its enable
/// toggle flips; disabled / empty leaves the Compose slot unregistered.
pub(crate) fn apply_hotkeys_if_changed(
    app: &AppHandle,
    current: &Settings,
    next: &Settings,
) -> Result<(), AppError> {
    let platform = app.state::<Arc<dyn Platform>>();

    // Primary (fn). On change: re-register and surface failures. Unchanged: just
    // ensure it's live — the Settings recorder's begin_trigger_capture stops ALL
    // triggers, and recording a NEW key returns here via update_settings, so the
    // OTHER slot must be revived. register_hotkey is idempotent when already
    // registered (the common no-op case); a revive failure is ignored so a normal
    // settings save never fails just because Input Monitoring lapsed.
    if next.hotkey != current.hotkey {
        platform.unregister_hotkey(app, HotkeySlot::Primary);
        platform.register_hotkey(
            app,
            HotkeySlot::Primary,
            &next.hotkey,
            build_hotkey_callback(app, HotkeyRole::Primary),
        )?;
    } else {
        let _ = platform.register_hotkey(
            app,
            HotkeySlot::Primary,
            &next.hotkey,
            build_hotkey_callback(app, HotkeyRole::Primary),
        );
    }

    // 写作键 (compose). A non-empty 写作键 = 启用; same revive logic.
    let compose_changed = next.compose_hotkey != current.compose_hotkey;
    let compose_on = !next.compose_hotkey.trim().is_empty();
    if compose_changed {
        platform.unregister_hotkey(app, HotkeySlot::Compose);
        if compose_on {
            platform.register_hotkey(
                app,
                HotkeySlot::Compose,
                &next.compose_hotkey,
                build_hotkey_callback(app, HotkeyRole::Compose),
            )?;
        }
    } else if compose_on {
        let _ = platform.register_hotkey(
            app,
            HotkeySlot::Compose,
            &next.compose_hotkey,
            build_hotkey_callback(app, HotkeyRole::Compose),
        );
    } else {
        platform.unregister_hotkey(app, HotkeySlot::Compose);
    }
    Ok(())
}
