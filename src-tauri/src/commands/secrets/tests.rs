use super::*;

#[test]
fn secret_key_ids_are_strict_identifiers() {
    assert!(validate_secret_key_id("openai_compatible_api_key").is_ok());
    assert!(validate_secret_key_id("").is_err());
    assert!(validate_secret_key_id("OpenAiKey").is_err());
    assert!(validate_secret_key_id("../openai_api_key").is_err());
    assert!(validate_secret_key_id("openai-api-key").is_err());
}

#[test]
fn has_secret_maps_missing_secret_to_false() {
    struct MissingSecretPlatform;

    impl Platform for MissingSecretPlatform {
        fn register_hotkey(
            &self,
            _app: &AppHandle,
            _slot: crate::platform::HotkeySlot,
            _combo: &str,
            _callback: crate::platform::HotkeyCallback,
        ) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn unregister_hotkey(&self, _app: &AppHandle, _slot: crate::platform::HotkeySlot) {
            unreachable!()
        }

        fn unregister_all_hotkeys(&self, _app: &AppHandle) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn inject_text(&self, _app: &AppHandle, _text: &str) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn ensure_microphone_permission(&self) -> bool {
            unreachable!()
        }

        fn store_secret(&self, _key: &str, _value: &str) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn has_secret(&self, _key: &str) -> crate::error::AppResult<bool> {
            Ok(false)
        }

        fn read_secret(&self, _key: &str) -> crate::error::AppResult<String> {
            Err(AppError::Provider("secret not found".into()))
        }

        fn delete_secret(&self, _key: &str) -> crate::error::AppResult<()> {
            unreachable!()
        }
    }

    assert!(!platform_has_secret(&MissingSecretPlatform, "openai_api_key").unwrap());
}

#[test]
fn has_secret_uses_presence_check_without_reading_secret_value() {
    struct PresentSecretPlatform;

    impl Platform for PresentSecretPlatform {
        fn register_hotkey(
            &self,
            _app: &AppHandle,
            _slot: crate::platform::HotkeySlot,
            _combo: &str,
            _callback: crate::platform::HotkeyCallback,
        ) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn unregister_hotkey(&self, _app: &AppHandle, _slot: crate::platform::HotkeySlot) {
            unreachable!()
        }

        fn unregister_all_hotkeys(&self, _app: &AppHandle) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn inject_text(&self, _app: &AppHandle, _text: &str) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn ensure_microphone_permission(&self) -> bool {
            unreachable!()
        }

        fn store_secret(&self, _key: &str, _value: &str) -> crate::error::AppResult<()> {
            unreachable!()
        }

        fn has_secret(&self, _key: &str) -> crate::error::AppResult<bool> {
            Ok(true)
        }

        fn read_secret(&self, _key: &str) -> crate::error::AppResult<String> {
            panic!("has_secret must not read the secret value")
        }

        fn delete_secret(&self, _key: &str) -> crate::error::AppResult<()> {
            unreachable!()
        }
    }

    assert!(platform_has_secret(&PresentSecretPlatform, "openai_api_key").unwrap());
}

#[test]
fn explicit_reveal_returns_saved_secret_value() {
    let secret = secret_for_explicit_reveal(
        "openai_api_key",
        |_| Ok(true),
        |key_id| {
            assert_eq!(key_id, "openai_api_key");
            Ok("saved-openai-key".into())
        },
    )
    .unwrap();

    assert_eq!(secret.as_deref(), Some("saved-openai-key"));
}

#[test]
fn explicit_reveal_skips_data_read_when_secret_is_missing() {
    let secret = secret_for_explicit_reveal(
        "openai_api_key",
        |_| Ok(false),
        |_| panic!("missing secret must not read Keychain data"),
    )
    .unwrap();

    assert_eq!(secret, None);
}
