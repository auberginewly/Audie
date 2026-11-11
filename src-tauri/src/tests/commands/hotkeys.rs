use super::*;

#[cfg(target_os = "windows")]
#[test]
fn windows_normalize_migrates_legacy_fn_to_platform_default() {
    let settings = normalize_settings(Settings {
        hotkey: " fn ".into(),
        ..Settings::default()
    });

    assert_eq!(settings.hotkey, "Ctrl+Shift+Space");
    assert!(should_migrate_windows_fn("Fn"));
}

#[test]
fn validate_rejects_compose_hotkey_equal_to_primary() {
    let settings = Settings {
        hotkey: "F13".into(),
        compose_hotkey: "F13".into(),
        ..Settings::default()
    };
    assert!(validate_settings(&settings).is_err());
}

#[test]
fn validate_allows_distinct_or_empty_compose_hotkey() {
    let distinct = Settings {
        hotkey: DEFAULT_HOTKEY.into(),
        compose_hotkey: "F13".into(),
        ..Settings::default()
    };
    assert!(validate_settings(&distinct).is_ok());

    let empty_compose = Settings {
        hotkey: DEFAULT_HOTKEY.into(),
        compose_hotkey: String::new(),
        ..Settings::default()
    };
    assert!(validate_settings(&empty_compose).is_ok());
}

#[cfg(target_os = "windows")]
#[test]
fn validate_rejects_fn_for_windows_hotkeys() {
    let primary = Settings {
        hotkey: "Fn".into(),
        ..Settings::default()
    };
    assert!(validate_settings(&primary).is_err());

    let compose = Settings {
        compose_hotkey: "fn".into(),
        ..Settings::default()
    };
    assert!(validate_settings(&compose).is_err());
}
