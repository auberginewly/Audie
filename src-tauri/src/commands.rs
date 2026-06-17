// Tauri commands for settings persistence (PROJECT_SPEC.md §3.5).
//
// P0.5 scope: hotkey only (microphone selection deferred to the future Settings
// page). Settings live in a tauri-plugin-store JSON file — NO manager owns them;
// the store plugin is the persistence layer per the P0.5 plan. Secrets never go
// here (those are P1 keychain, §6.6).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::error::AppError;
use crate::platform::Platform;

const STORE_FILE: &str = "settings.json";
const KEY_HOTKEY: &str = "hotkey";
const KEY_ASR_PROVIDER: &str = "asr_provider";
const KEY_LLM_PROVIDER: &str = "llm_provider";
const KEY_ENHANCE_ENABLED: &str = "enhance_enabled";
const KEY_ENHANCE_PROMPT: &str = "enhance_prompt";
const KEY_WHISPER_CPP_MODEL_PATH: &str = "whisper_cpp_model_path";
const KEY_OPENAI_COMPATIBLE_BASE_URL: &str = "openai_compatible_base_url";
const KEY_OPENAI_COMPATIBLE_MODEL: &str = "openai_compatible_model";

pub const DEFAULT_HOTKEY: &str = "Ctrl+Shift+Space";
pub const DEFAULT_ASR_PROVIDER: &str = "groq";
pub const DEFAULT_LLM_PROVIDER: &str = "openai_compatible";
pub const DEFAULT_OPENAI_COMPATIBLE_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_COMPATIBLE_MODEL: &str = "gpt-4o-mini";
pub const DEFAULT_ENHANCE_PROMPT: &str =
    "去掉口水话，修正明显口误，补充标点和换行；不要改原意，不要添加信息，不要翻译。";

/// The only hotkeys the UI lets you pick in P0.5. A free-form key recorder is
/// P3 Settings-page work; presets keep this slice small and parseable by
/// `tauri-plugin-global-shortcut`.
pub const HOTKEY_PRESETS: &[&str] = &["Ctrl+Shift+Space", "Alt+Space", "Ctrl+Alt+Space"];

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub hotkey: String,
    pub asr_provider: String,
    pub llm_provider: String,
    pub enhance_enabled: bool,
    pub enhance_prompt: String,
    pub whisper_cpp_model_path: Option<String>,
    pub openai_compatible_base_url: String,
    pub openai_compatible_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            asr_provider: DEFAULT_ASR_PROVIDER.to_string(),
            llm_provider: DEFAULT_LLM_PROVIDER.to_string(),
            enhance_enabled: false,
            enhance_prompt: DEFAULT_ENHANCE_PROMPT.to_string(),
            whisper_cpp_model_path: None,
            openai_compatible_base_url: DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_string(),
            openai_compatible_model: DEFAULT_OPENAI_COMPATIBLE_MODEL.to_string(),
        }
    }
}

#[derive(Deserialize)]
pub struct SettingsPatch {
    pub hotkey: Option<String>,
    pub asr_provider: Option<String>,
    pub llm_provider: Option<String>,
    pub enhance_enabled: Option<bool>,
    pub enhance_prompt: Option<String>,
    pub whisper_cpp_model_path: Option<String>,
    pub openai_compatible_base_url: Option<String>,
    pub openai_compatible_model: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ProviderMetadata {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub engine: String,
    pub default_model: Option<String>,
    pub requires_key: bool,
    pub tags: Vec<String>,
}

/// Read the persisted hotkey, falling back to the default when the store is
/// empty or holds something unexpected. Called at startup (lib.rs) and by
/// `get_settings`.
pub fn load_hotkey(app: &AppHandle) -> String {
    let stored = app
        .store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(KEY_HOTKEY))
        .and_then(|value| value.as_str().map(str::to_string));

    match stored {
        Some(hotkey) if HOTKEY_PRESETS.contains(&hotkey.as_str()) => hotkey,
        _ => DEFAULT_HOTKEY.to_string(),
    }
}

fn read_string_setting(app: &AppHandle, key: &str, default: &str) -> String {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn read_bool_setting(app: &AppHandle, key: &str, default: bool) -> bool {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

pub fn load_settings(app: &AppHandle) -> Settings {
    Settings {
        hotkey: load_hotkey(app),
        asr_provider: read_provider_setting(
            app,
            KEY_ASR_PROVIDER,
            DEFAULT_ASR_PROVIDER,
            &available_asr_providers(),
        ),
        llm_provider: read_provider_setting(
            app,
            KEY_LLM_PROVIDER,
            DEFAULT_LLM_PROVIDER,
            &available_llm_providers(),
        ),
        enhance_enabled: read_bool_setting(app, KEY_ENHANCE_ENABLED, false),
        enhance_prompt: read_string_setting(app, KEY_ENHANCE_PROMPT, DEFAULT_ENHANCE_PROMPT),
        whisper_cpp_model_path: read_optional_string_setting(app, KEY_WHISPER_CPP_MODEL_PATH),
        openai_compatible_base_url: read_string_setting(
            app,
            KEY_OPENAI_COMPATIBLE_BASE_URL,
            DEFAULT_OPENAI_COMPATIBLE_BASE_URL,
        ),
        openai_compatible_model: read_string_setting(
            app,
            KEY_OPENAI_COMPATIBLE_MODEL,
            DEFAULT_OPENAI_COMPATIBLE_MODEL,
        ),
    }
}

fn read_optional_string_setting(app: &AppHandle, key: &str) -> Option<String> {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|value| value.as_str().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_provider_setting(
    app: &AppHandle,
    key: &str,
    default: &str,
    providers: &[ProviderMetadata],
) -> String {
    let stored = read_string_setting(app, key, default);
    if providers.iter().any(|provider| provider.id == stored) {
        stored
    } else {
        default.to_string()
    }
}

fn provider_metadata(
    id: &str,
    title: &str,
    kind: &str,
    engine: &str,
    default_model: Option<&str>,
    requires_key: bool,
    tags: &[&str],
) -> ProviderMetadata {
    ProviderMetadata {
        id: id.to_string(),
        title: title.to_string(),
        kind: kind.to_string(),
        engine: engine.to_string(),
        default_model: default_model.map(str::to_string),
        requires_key,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
    }
}

pub fn available_asr_providers() -> Vec<ProviderMetadata> {
    vec![
        provider_metadata(
            "groq",
            "Groq",
            "asr",
            "Remote ASR",
            Some("whisper-large-v3-turbo"),
            true,
            &["Remote", "Fast", "Whisper"],
        ),
        provider_metadata(
            "openai",
            "OpenAI",
            "asr",
            "Remote ASR",
            Some("whisper-1"),
            true,
            &["Remote", "Multilingual"],
        ),
        provider_metadata(
            "whisper_cpp",
            "Whisper.cpp",
            "asr",
            "Local ASR",
            None,
            false,
            &["Local"],
        ),
    ]
}

pub fn available_llm_providers() -> Vec<ProviderMetadata> {
    vec![provider_metadata(
        "openai_compatible",
        "OpenAI Compatible",
        "llm",
        "Remote LLM",
        None,
        true,
        &["Remote", "Configurable"],
    )]
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, AppError> {
    Ok(load_settings(&app))
}

/// Persist a new hotkey and apply it live: unregister the old combo, register
/// the new one (rebuilding the press/release callback), then write the store.
/// Re-register before persist so a registration failure leaves the store
/// untouched (no "saved but not active" mismatch).
#[tauri::command]
pub fn update_settings(app: AppHandle, patch: SettingsPatch) -> Result<Settings, AppError> {
    let current = load_settings(&app);
    let next_hotkey = patch.hotkey.unwrap_or(current.hotkey);
    let next_asr_provider = patch.asr_provider.unwrap_or(current.asr_provider);
    let next_llm_provider = patch.llm_provider.unwrap_or(current.llm_provider);
    let next_enhance_enabled = patch.enhance_enabled.unwrap_or(current.enhance_enabled);
    let next_enhance_prompt = patch.enhance_prompt.unwrap_or(current.enhance_prompt);
    let next_whisper_cpp_model_path = patch
        .whisper_cpp_model_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or(current.whisper_cpp_model_path);
    let next_openai_compatible_base_url = patch
        .openai_compatible_base_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(current.openai_compatible_base_url);
    let next_openai_compatible_model = patch
        .openai_compatible_model
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(current.openai_compatible_model);

    if !HOTKEY_PRESETS.contains(&next_hotkey.as_str()) {
        return Err(AppError::Internal(format!(
            "unsupported hotkey: {next_hotkey}"
        )));
    }
    if !available_asr_providers()
        .iter()
        .any(|provider| provider.id == next_asr_provider)
    {
        return Err(AppError::Internal(format!(
            "unsupported ASR provider: {next_asr_provider}"
        )));
    }
    if !available_llm_providers()
        .iter()
        .any(|provider| provider.id == next_llm_provider)
    {
        return Err(AppError::Internal(format!(
            "unsupported LLM provider: {next_llm_provider}"
        )));
    }

    if next_hotkey != load_hotkey(&app) {
        let platform = app.state::<Arc<dyn Platform>>();
        platform.unregister_all_hotkeys(&app)?;
        platform.register_hotkey(&app, &next_hotkey, crate::build_hotkey_callback(&app))?;
    }

    let store = app
        .store(STORE_FILE)
        .map_err(|err| AppError::Internal(format!("open store: {err}")))?;
    store.set(KEY_HOTKEY, next_hotkey);
    store.set(KEY_ASR_PROVIDER, next_asr_provider);
    store.set(KEY_LLM_PROVIDER, next_llm_provider);
    store.set(KEY_ENHANCE_ENABLED, next_enhance_enabled);
    store.set(KEY_ENHANCE_PROMPT, next_enhance_prompt);
    store.set(
        KEY_OPENAI_COMPATIBLE_BASE_URL,
        next_openai_compatible_base_url,
    );
    store.set(KEY_OPENAI_COMPATIBLE_MODEL, next_openai_compatible_model);
    if let Some(model_path) = next_whisper_cpp_model_path {
        store.set(KEY_WHISPER_CPP_MODEL_PATH, model_path);
    } else {
        store.delete(KEY_WHISPER_CPP_MODEL_PATH);
    }
    store
        .save()
        .map_err(|err| AppError::Internal(format!("save store: {err}")))?;

    Ok(load_settings(&app))
}

#[tauri::command]
pub fn list_asr_providers() -> Vec<ProviderMetadata> {
    available_asr_providers()
}

#[tauri::command]
pub fn list_llm_providers() -> Vec<ProviderMetadata> {
    available_llm_providers()
}

#[tauri::command]
pub fn set_secret(app: AppHandle, key_id: String, value: String) -> Result<(), AppError> {
    validate_secret_key_id(&key_id)?;
    if value.is_empty() {
        return Err(AppError::Provider("secret value cannot be empty".into()));
    }

    let platform = app.state::<Arc<dyn Platform>>();
    platform.store_secret(&key_id, &value)
}

#[tauri::command]
pub fn has_secret(app: AppHandle, key_id: String) -> Result<bool, AppError> {
    validate_secret_key_id(&key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    platform_has_secret(platform.inner().as_ref(), &key_id)
}

#[tauri::command]
pub fn delete_secret(app: AppHandle, key_id: String) -> Result<(), AppError> {
    validate_secret_key_id(&key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    platform.delete_secret(&key_id)
}

pub(crate) fn validate_secret_key_id(key_id: &str) -> Result<(), AppError> {
    if key_id.is_empty()
        || !key_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(AppError::Internal(format!(
            "unsupported secret key id: {key_id}"
        )));
    }
    Ok(())
}

fn platform_has_secret(platform: &dyn Platform, key_id: &str) -> Result<bool, AppError> {
    match platform.read_secret(key_id) {
        Ok(_) => Ok(true),
        Err(AppError::Provider(_)) => Ok(false),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_include_p1_provider_fields() {
        let settings = Settings::default();

        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert_eq!(settings.asr_provider, "groq");
        assert_eq!(settings.llm_provider, "openai_compatible");
        assert!(!settings.enhance_enabled);
        assert!(!settings.enhance_prompt.trim().is_empty());
        assert_eq!(settings.whisper_cpp_model_path, None);
        assert_eq!(
            settings.openai_compatible_base_url,
            DEFAULT_OPENAI_COMPATIBLE_BASE_URL
        );
        assert_eq!(
            settings.openai_compatible_model,
            DEFAULT_OPENAI_COMPATIBLE_MODEL
        );
    }

    #[test]
    fn provider_lists_expose_voxt_style_metadata() {
        let asr = available_asr_providers();
        let llm = available_llm_providers();

        assert_eq!(
            asr.iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["groq", "openai", "whisper_cpp",]
        );
        assert_eq!(
            llm.iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["openai_compatible",]
        );

        let groq = asr.iter().find(|provider| provider.id == "groq").unwrap();
        assert_eq!(groq.title, "Groq");
        assert_eq!(groq.engine, "Remote ASR");
        assert_eq!(
            groq.default_model.as_deref(),
            Some("whisper-large-v3-turbo")
        );
        assert!(groq.requires_key);
        assert!(groq.tags.contains(&"Remote".to_string()));
        assert!(groq.tags.contains(&"Fast".to_string()));

        let whisper_cpp = asr
            .iter()
            .find(|provider| provider.id == "whisper_cpp")
            .unwrap();
        assert_eq!(whisper_cpp.engine, "Local ASR");
        assert!(!whisper_cpp.requires_key);
        assert!(whisper_cpp.tags.contains(&"Local".to_string()));

        let openai_compatible = &llm[0];
        assert_eq!(openai_compatible.title, "OpenAI Compatible");
        assert_eq!(openai_compatible.engine, "Remote LLM");
        assert!(openai_compatible.requires_key);
        assert!(openai_compatible.tags.contains(&"Configurable".to_string()));
    }

    #[test]
    fn secret_key_ids_are_strict_identifiers() {
        assert!(validate_secret_key_id("groq_api_key").is_ok());
        assert!(validate_secret_key_id("openai_compatible_api_key").is_ok());
        assert!(validate_secret_key_id("").is_err());
        assert!(validate_secret_key_id("GroqApiKey").is_err());
        assert!(validate_secret_key_id("../groq_api_key").is_err());
        assert!(validate_secret_key_id("groq-api-key").is_err());
    }

    #[test]
    fn has_secret_maps_missing_secret_to_false() {
        struct MissingSecretPlatform;

        impl Platform for MissingSecretPlatform {
            fn register_hotkey(
                &self,
                _app: &AppHandle,
                _combo: &str,
                _callback: crate::platform::HotkeyCallback,
            ) -> crate::error::AppResult<()> {
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

            fn read_secret(&self, _key: &str) -> crate::error::AppResult<String> {
                Err(AppError::Provider("secret not found".into()))
            }

            fn delete_secret(&self, _key: &str) -> crate::error::AppResult<()> {
                unreachable!()
            }
        }

        assert!(!platform_has_secret(&MissingSecretPlatform, "groq_api_key").unwrap());
    }
}
