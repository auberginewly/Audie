// Tauri commands for settings persistence (PROJECT_SPEC.md §3.5).
//
// Settings live in a human-editable TOML file (`settings.toml` in the app config
// dir) — NO manager owns them. The legacy tauri-plugin-store JSON is read once,
// directly (no store plugin), to migrate existing installs into TOML. Secrets never
// go here (those are P1 keychain, §6.6).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;
use crate::platform::{HotkeySlot, Platform};

/// Legacy tauri-plugin-store filename, read once to migrate into TOML (P3 cleanup).
const LEGACY_SETTINGS_JSON: &str = "settings.json";

pub const DEFAULT_HOTKEY: &str = "Fn";
pub const DEFAULT_ASR_PROVIDER: &str = "groq";
pub const DEFAULT_LLM_PROVIDER: &str = "openai_compatible";
pub const DEFAULT_OPENAI_COMPATIBLE_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_OPENAI_COMPATIBLE_MODEL: &str = "gpt-4o-mini";
// Legacy shared LLM key id — the default so pre-4b installs keep reading their key
// until the user re-picks a provider (which writes a per-provider key id).
pub const DEFAULT_LLM_API_KEY_ID: &str = "openai_compatible_api_key";
pub const DEFAULT_HISTORY_RETENTION: &str = "forever";
pub const DEFAULT_UI_LANGUAGE: &str = "zh-Hans";
const HISTORY_RETENTION_IDS: &[&str] = &["never", "day", "week", "month", "forever"];
const UI_LANGUAGE_IDS: &[&str] = &["zh-Hans", "zh-Hant", "en"];
// The factory-default enhance prompt is data, not source: it lives in
// prompts/enhance_default.md and is pulled into Settings::default() via include_str!
// (no prompt string in .rs). Edit that file to change the default; the user owns the
// value after (settings.toml / 润色提示词 box). Sent + main language at enhance time.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)] // missing TOML keys fall back to Default (hand-edited / old files)
pub struct Settings {
    pub hotkey: String,
    pub asr_provider: String,
    /// ASR model id within the chosen provider (e.g. groq whisper-large-v3 vs
    /// -turbo). Empty = use each adapter's built-in default const, so old TOML
    /// files and untouched installs keep working. Doubao ignores it (豆包 uses
    /// resource_id, not model — left blank for it, see normalize note).
    pub asr_model: String,
    pub llm_provider: String,
    /// 「AI 润色」总开关。默认 true（配了 LLM 即自动润色）；关掉 = 即使配了 key 也只插入
    /// 语音转写原文（纯转写），给只想要原始文字的人选择权。compose / rewrite 不受它管。
    pub enhance_enabled: bool,
    pub enhance_prompt: String,
    pub openai_compatible_base_url: String,
    pub openai_compatible_model: String,
    /// Keychain key id holding the active LLM provider's API key. All cloud LLM
    /// cards drive one backend slot (openai_compatible) but each stores its own
    /// key (deepseek_api_key / kimi_api_key / …), so switching providers doesn't
    /// reuse another's key. Empty = key-optional local provider (Ollama / LM
    /// Studio). Defaults to the legacy shared id so existing installs keep working
    /// until the user re-picks a provider.
    pub llm_api_key_id: String,
    pub doubao_endpoint: String,
    pub doubao_resource_id: String,
    /// Manually selected input device name (matches `cpal` device.name()). Empty
    /// string = automatic (P0.7 picks a reliable mic). Not `Option` so the patch
    /// can express "clear back to auto" via an empty string.
    pub input_device: String,
    /// Whether first-run onboarding has been completed (P3.12). Default false so a
    /// fresh install auto-opens the SetupWizard; set true when the user finishes it.
    pub onboarding_completed: bool,
    /// User's main language; lib.rs prepends it as a line to the enhance prompt at
    /// send time. Empty string = follow the system locale (like `input_device`'s
    /// empty = automatic). Resolved at enhance time (lib.rs).
    pub primary_language: String,
    /// How long dictation history is kept on disk (History screen). One of
    /// `never | day | week | month | forever`; `never` skips recording entirely,
    /// the rest prune older rows. `normalize_settings` clamps anything else.
    pub history_retention: String,
    pub ui_language: String,
    pub show_in_dock: bool,
    /// 写作模式（compose）独立触发键。空串 = 未配置 = 写作不启用（配了键即启用）。文法同主 hotkey。
    pub compose_hotkey: String,
    /// 写作模式提示词出厂默认（数据文件，源码零 prompt，同 enhance_prompt 经 include_str! 读）。
    pub compose_prompt: String,
    /// 改写模式（rewrite）提示词出厂默认（数据文件）。改写按口述指令改写选中文字（逻辑见片2）。
    pub rewrite_prompt: String,
    /// Per-provider LLM model the user chose, keyed by the front-end card id
    /// (deepseek / lmstudio / …). All LLM cards share one backend slot, so this
    /// lets 选用 restore each provider's own model instead of clearing it. Backend
    /// treats it as an opaque string→string map. MUST stay the last field: a TOML
    /// table has to follow all scalar keys at the same level.
    #[serde(default)]
    pub llm_models: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.to_string(),
            asr_provider: DEFAULT_ASR_PROVIDER.to_string(),
            asr_model: String::new(),
            llm_provider: DEFAULT_LLM_PROVIDER.to_string(),
            // 默认开：配了 LLM 就自动润色（沿用 f87b551 后的行为）；用户可关掉只要纯转写。
            enhance_enabled: true,
            enhance_prompt: include_str!("../prompts/enhance_default.md")
                .trim_end()
                .to_string(),
            openai_compatible_base_url: DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_string(),
            openai_compatible_model: DEFAULT_OPENAI_COMPATIBLE_MODEL.to_string(),
            llm_api_key_id: DEFAULT_LLM_API_KEY_ID.to_string(),
            doubao_endpoint: crate::asr::doubao::config::DEFAULT_ENDPOINT.to_string(),
            doubao_resource_id: crate::asr::doubao::config::DEFAULT_RESOURCE_ID.to_string(),
            input_device: String::new(),
            onboarding_completed: false,
            primary_language: String::new(),
            history_retention: DEFAULT_HISTORY_RETENTION.to_string(),
            ui_language: DEFAULT_UI_LANGUAGE.to_string(),
            show_in_dock: true,
            compose_hotkey: String::new(),
            compose_prompt: include_str!("../prompts/compose_default.md")
                .trim_end()
                .to_string(),
            rewrite_prompt: include_str!("../prompts/rewrite_default.md")
                .trim_end()
                .to_string(),
            llm_models: HashMap::new(),
        }
    }
}

#[derive(Deserialize)]
pub struct SettingsPatch {
    pub hotkey: Option<String>,
    pub asr_provider: Option<String>,
    pub asr_model: Option<String>,
    pub llm_provider: Option<String>,
    pub enhance_enabled: Option<bool>,
    pub enhance_prompt: Option<String>,
    pub openai_compatible_base_url: Option<String>,
    pub openai_compatible_model: Option<String>,
    pub llm_api_key_id: Option<String>,
    pub doubao_endpoint: Option<String>,
    pub doubao_resource_id: Option<String>,
    pub input_device: Option<String>,
    pub onboarding_completed: Option<bool>,
    pub primary_language: Option<String>,
    pub history_retention: Option<String>,
    pub ui_language: Option<String>,
    pub show_in_dock: Option<bool>,
    pub compose_hotkey: Option<String>,
    pub compose_prompt: Option<String>,
    pub rewrite_prompt: Option<String>,
    pub llm_models: Option<HashMap<String, String>>,
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

/// The persisted hotkey, for startup (lib.rs) + `apply_hotkey_if_changed`. Derived
/// from the full settings load so there's a single source of truth.
pub fn load_hotkey(app: &AppHandle) -> String {
    load_settings(app).hotkey
}

/// Path to the human-editable settings file (TOML), in the platform config dir.
fn settings_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.toml"))
        .map_err(|err| AppError::Internal(format!("resolve config dir: {err}")))
}

fn write_settings_toml(path: &Path, settings: &Settings) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| AppError::Internal(format!("create config dir: {err}")))?;
    }
    let text = toml::to_string_pretty(settings)
        .map_err(|err| AppError::Internal(format!("serialize settings: {err}")))?;
    std::fs::write(path, text)
        .map_err(|err| AppError::Internal(format!("write settings.toml: {err}")))
}

/// Load user settings from `settings.toml`. The first run after the TOML switch the
/// file is absent, so we migrate the legacy tauri-plugin-store JSON (which yields
/// defaults when empty) and write the TOML once. Parse failures degrade to defaults
/// rather than wedging settings. Always normalized so callers get valid values.
pub fn load_settings(app: &AppHandle) -> Settings {
    let path = match settings_path(app) {
        Ok(path) => path,
        Err(err) => {
            log::error!("{err}");
            return Settings::default();
        }
    };

    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => return normalize_settings(settings),
                Err(err) => log::error!("parse settings.toml, using defaults: {err}"),
            },
            Err(err) => log::error!("read settings.toml: {err}"),
        }
        return normalize_settings(Settings::default());
    }

    let migrated = normalize_settings(migrate_from_legacy_json(app));
    if let Err(err) = write_settings_toml(&path, &migrated) {
        log::error!("write settings.toml during migration: {err}");
    }
    migrated
}

/// Clamp loaded settings to valid values — the validation the per-field store readers
/// used to do, applied once after deserialize. Unknown asr/llm providers reset to
/// default (but `doubao_stream` stays — streaming-only, absent from the batch list);
/// legacy doubao resource id migrates; empty required strings fall back to defaults;
/// a blank whisper model path becomes None.
fn normalize_settings(mut settings: Settings) -> Settings {
    if settings.asr_provider != "doubao_stream"
        && !available_asr_providers()
            .iter()
            .any(|provider| provider.id == settings.asr_provider)
    {
        settings.asr_provider = DEFAULT_ASR_PROVIDER.to_string();
    }
    if !available_llm_providers()
        .iter()
        .any(|provider| provider.id == settings.llm_provider)
    {
        settings.llm_provider = DEFAULT_LLM_PROVIDER.to_string();
    }
    settings.doubao_resource_id = normalize_doubao_resource_id(settings.doubao_resource_id);
    // asr_model is free-form (each adapter validates / defaults at request time);
    // empty = built-in default. Only trim hand-edited whitespace, never reset.
    settings.asr_model = settings.asr_model.trim().to_string();

    if settings.hotkey.trim().is_empty() {
        settings.hotkey = DEFAULT_HOTKEY.to_string();
    }
    if settings.enhance_prompt.trim().is_empty() {
        settings.enhance_prompt = Settings::default().enhance_prompt;
    }
    if settings.openai_compatible_base_url.trim().is_empty() {
        settings.openai_compatible_base_url = DEFAULT_OPENAI_COMPATIBLE_BASE_URL.to_string();
    }
    // No empty→default reset for the model: an empty model is a meaningful "not yet
    // chosen" state (picking a provider no longer seeds a hardcoded/stale model id —
    // the user fetches the live list or types one). build_provider surfaces a clear
    // "model 未配置" error when enhance runs without one.
    settings.openai_compatible_model = settings.openai_compatible_model.trim().to_string();
    if settings.doubao_endpoint.trim().is_empty() {
        settings.doubao_endpoint = crate::asr::doubao::config::DEFAULT_ENDPOINT.to_string();
    }
    if settings.doubao_resource_id.trim().is_empty() {
        settings.doubao_resource_id = crate::asr::doubao::config::DEFAULT_RESOURCE_ID.to_string();
    }

    if !HISTORY_RETENTION_IDS.contains(&settings.history_retention.as_str()) {
        settings.history_retention = DEFAULT_HISTORY_RETENTION.to_string();
    }
    if !UI_LANGUAGE_IDS.contains(&settings.ui_language.as_str()) {
        settings.ui_language = DEFAULT_UI_LANGUAGE.to_string();
    }
    if settings.compose_prompt.trim().is_empty() {
        settings.compose_prompt = Settings::default().compose_prompt;
    }
    if settings.rewrite_prompt.trim().is_empty() {
        settings.rewrite_prompt = Settings::default().rewrite_prompt;
    }

    settings
}

fn normalize_doubao_resource_id(stored: String) -> String {
    if stored == crate::asr::doubao::config::LEGACY_RESOURCE_ID {
        crate::asr::doubao::config::DEFAULT_RESOURCE_ID.to_string()
    } else {
        stored
    }
}

/// One-time migration: read the legacy tauri-plugin-store JSON directly with serde
/// (a flat key→value object whose keys match Settings fields), so we no longer
/// depend on the store plugin. Unknown legacy keys are ignored and missing fields
/// use Default (container `serde(default)`); an absent/unreadable/corrupt file
/// yields Default.
fn migrate_from_legacy_json(app: &AppHandle) -> Settings {
    let Ok(dir) = app.path().app_config_dir() else {
        return Settings::default();
    };
    match std::fs::read_to_string(dir.join(LEGACY_SETTINGS_JSON)) {
        Ok(text) => serde_json::from_str::<Settings>(&text).unwrap_or_else(|err| {
            log::warn!("parse legacy settings.json, using defaults: {err}");
            Settings::default()
        }),
        Err(_) => Settings::default(),
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
            "glm",
            "智谱 GLM ASR",
            "asr",
            "Remote ASR",
            Some(crate::asr::glm::DEFAULT_MODEL),
            true,
            &["Remote", "中文"],
        ),
        provider_metadata(
            "aliyun_fun",
            "通义 Paraformer ASR",
            "asr",
            "Remote ASR",
            Some(crate::asr::aliyun::config::DEFAULT_MODEL),
            true,
            &["Remote", "实时", "中文"],
        ),
        provider_metadata(
            "stepfun",
            "StepFun ASR",
            "asr",
            "Remote ASR",
            Some(crate::asr::stepfun::config::DEFAULT_MODEL),
            true,
            &["Remote", "中文"],
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
    let next = settings_from_patch(current.clone(), patch)?;

    apply_hotkeys_if_changed(&app, &current, &next)?;
    apply_dock_visibility_if_changed(&app, &current, &next)?;
    persist_settings(&app, next)?;

    let updated = load_settings(&app);
    if let Err(err) = app.emit("settings-updated", &updated) {
        log::warn!("emit settings-updated failed: {err}");
    }
    Ok(updated)
}

/// Re-register triggers whose binding changed — the primary (fn) key and the 写作键
/// independently. Re-register before persist so a failure leaves the store untouched
/// (no "saved but not active" mismatch). 写作键 also re-registers when its enable
/// toggle flips; disabled / empty leaves the Compose slot unregistered.
fn apply_hotkeys_if_changed(
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
            crate::build_hotkey_callback(app, crate::HotkeyRole::Primary),
        )?;
    } else {
        let _ = platform.register_hotkey(
            app,
            HotkeySlot::Primary,
            &next.hotkey,
            crate::build_hotkey_callback(app, crate::HotkeyRole::Primary),
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
                crate::build_hotkey_callback(app, crate::HotkeyRole::Compose),
            )?;
        }
    } else if compose_on {
        let _ = platform.register_hotkey(
            app,
            HotkeySlot::Compose,
            &next.compose_hotkey,
            crate::build_hotkey_callback(app, crate::HotkeyRole::Compose),
        );
    } else {
        platform.unregister_hotkey(app, HotkeySlot::Compose);
    }
    Ok(())
}

fn apply_dock_visibility_if_changed(
    app: &AppHandle,
    current: &Settings,
    next: &Settings,
) -> Result<(), AppError> {
    if next.show_in_dock == current.show_in_dock {
        return Ok(());
    }
    let platform = app.state::<Arc<dyn Platform>>();
    platform.set_dock_visible(app, next.show_in_dock)
}

fn settings_from_patch(current: Settings, patch: SettingsPatch) -> Result<Settings, AppError> {
    let next = Settings {
        hotkey: patch.hotkey.unwrap_or(current.hotkey),
        asr_provider: patch.asr_provider.unwrap_or(current.asr_provider),
        // Empty is meaningful (= adapter built-in default), so like input_device we
        // keep an empty patch value rather than filtering it back to current.
        asr_model: patch
            .asr_model
            .map(|value| value.trim().to_string())
            .unwrap_or(current.asr_model),
        llm_provider: patch.llm_provider.unwrap_or(current.llm_provider),
        enhance_enabled: patch.enhance_enabled.unwrap_or(current.enhance_enabled),
        enhance_prompt: patch.enhance_prompt.unwrap_or(current.enhance_prompt),
        openai_compatible_base_url: patch
            .openai_compatible_base_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.openai_compatible_base_url),
        // Empty is meaningful (= no model chosen yet; provider pick no longer seeds
        // a hardcoded one), so keep an explicit empty patch value rather than
        // filtering it back to current.
        openai_compatible_model: patch
            .openai_compatible_model
            .map(|value| value.trim().to_string())
            .unwrap_or(current.openai_compatible_model),
        // Empty is meaningful (= key-optional local provider), so keep an empty
        // patch value rather than filtering it back to current (like input_device).
        llm_api_key_id: patch
            .llm_api_key_id
            .map(|value| value.trim().to_string())
            .unwrap_or(current.llm_api_key_id),
        doubao_endpoint: patch
            .doubao_endpoint
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.doubao_endpoint),
        doubao_resource_id: patch
            .doubao_resource_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.doubao_resource_id),
        // Empty is meaningful here (= automatic), so unlike the others we keep an
        // empty patch value instead of filtering it back to the current value.
        input_device: patch
            .input_device
            .map(|value| value.trim().to_string())
            .unwrap_or(current.input_device),
        onboarding_completed: patch
            .onboarding_completed
            .unwrap_or(current.onboarding_completed),
        // Empty is meaningful (= follow system locale), so like input_device we keep
        // an empty patch value rather than filtering it back to current.
        primary_language: patch.primary_language.unwrap_or(current.primary_language),
        // Retention is an enum-ish id (never/day/week/month/forever) — a blank patch
        // keeps current; normalize_settings clamps anything unknown.
        history_retention: patch
            .history_retention
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.history_retention),
        ui_language: patch
            .ui_language
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or(current.ui_language),
        show_in_dock: patch.show_in_dock.unwrap_or(current.show_in_dock),
        // 空串有意义（= 写作键未配置），故保留空 patch 值，不 filter 回 current（同 input_device）。
        compose_hotkey: patch
            .compose_hotkey
            .map(|value| value.trim().to_string())
            .unwrap_or(current.compose_hotkey),
        compose_prompt: patch.compose_prompt.unwrap_or(current.compose_prompt),
        rewrite_prompt: patch.rewrite_prompt.unwrap_or(current.rewrite_prompt),
        // Whole-map replace when provided (the front-end sends the merged map).
        llm_models: patch.llm_models.unwrap_or(current.llm_models),
    };

    validate_settings(&next)?;
    Ok(next)
}

fn validate_settings(settings: &Settings) -> Result<(), AppError> {
    // The trigger string's real gate is parse_trigger at register time (platform
    // layer); here we only reject an empty value, so the recorder can pick fn /
    // function keys / combos freely (SPEC §5.8 P3.9).
    if settings.hotkey.trim().is_empty() {
        return Err(AppError::Internal("trigger key must not be empty".into()));
    }
    // 润色/改写键与写作键不能相同（前端 HotkeyRecorder 已实时拦，这里兜底防手改 toml）。
    let compose = settings.compose_hotkey.trim();
    if !compose.is_empty() && compose == settings.hotkey.trim() {
        return Err(AppError::Internal(
            "写作触发键不能和润色/改写触发键相同".into(),
        ));
    }
    // `doubao_stream` is a real, selectable ASR choice (the model picker writes it)
    // but it's streaming-only, not a batch provider, so it stays out of
    // `available_asr_providers` / `list_asr_providers`. Accept it explicitly here.
    if settings.asr_provider != "doubao_stream"
        && !available_asr_providers()
            .iter()
            .any(|provider| provider.id == settings.asr_provider)
    {
        return Err(AppError::Internal(format!(
            "unsupported ASR provider: {}",
            settings.asr_provider
        )));
    }
    if !available_llm_providers()
        .iter()
        .any(|provider| provider.id == settings.llm_provider)
    {
        return Err(AppError::Internal(format!(
            "unsupported LLM provider: {}",
            settings.llm_provider
        )));
    }
    if settings.enhance_prompt.trim().is_empty() {
        return Err(AppError::Internal("enhance prompt cannot be empty".into()));
    }
    if settings.openai_compatible_base_url.trim().is_empty() {
        return Err(AppError::Internal(
            "OpenAI-compatible base URL cannot be empty".into(),
        ));
    }
    // openai_compatible_model may be empty: picking a provider seeds no model (ids go
    // stale), the user fetches/types one. build_provider errors only when enhance runs.
    if settings.doubao_endpoint.trim().is_empty() {
        return Err(AppError::Internal("Doubao endpoint cannot be empty".into()));
    }
    if settings.doubao_resource_id.trim().is_empty() {
        return Err(AppError::Internal(
            "Doubao resource id cannot be empty".into(),
        ));
    }
    Ok(())
}

fn persist_settings(app: &AppHandle, settings: Settings) -> Result<(), AppError> {
    let path = settings_path(app)?;
    write_settings_toml(&path, &settings)
}

/// Enumerate input devices for the Settings device picker. A cheap CoreAudio/cpal
/// query (not the hot path), so it stays a plain sync command like the provider
/// lists. Returns names that `input_device` / device resolution match on.
#[tauri::command]
pub fn list_microphones() -> Vec<crate::managers::audio::MicrophoneInfo> {
    crate::managers::audio::list_input_devices()
}

/// Name of the device the automatic path would open (P0.7's override or the
/// system default), so the picker's "自动" row can show what it resolves to.
#[tauri::command]
pub fn auto_input_device(app: AppHandle) -> Option<String> {
    let platform = app.state::<Arc<dyn Platform>>();
    let preferred = platform.preferred_input_device_name();
    crate::managers::audio::auto_input_device_name(preferred)
}

/// Start the Settings mic-preview monitor on `device` (None / "" = automatic).
/// Emits `mic-monitor-level` until `stop_mic_monitor`, so the picker can show the
/// chosen mic is actually picking up sound. Called when the device section mounts
/// and on every selection change.
#[tauri::command]
pub fn start_mic_monitor(app: AppHandle, device: Option<String>) -> Result<(), AppError> {
    let audio = app.state::<Arc<crate::managers::audio::AudioManager>>();
    audio.start_monitor(app.clone(), device)
}

/// Stop the Settings mic-preview monitor (section unmounted / dialog closed).
#[tauri::command]
pub fn stop_mic_monitor(app: AppHandle) -> Result<(), AppError> {
    let audio = app.state::<Arc<crate::managers::audio::AudioManager>>();
    audio.stop_monitor();
    Ok(())
}

/// Dictation history for the History screen (newest first, capped). Reads straight
/// from the HistoryManager's SQLite store (§6.1).
#[tauri::command]
pub fn list_history(
    app: AppHandle,
) -> Result<Vec<crate::managers::history::HistoryEntry>, AppError> {
    app.state::<Arc<crate::managers::history::HistoryManager>>()
        .list()
}

#[tauri::command]
pub fn delete_history_entry(app: AppHandle, id: i64) -> Result<(), AppError> {
    let history = app.state::<Arc<crate::managers::history::HistoryManager>>();
    history.delete_entry(&app, id)
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Result<(), AppError> {
    let history = app.state::<Arc<crate::managers::history::HistoryManager>>();
    history.clear(&app)
}

/// All-time usage totals for the Home dashboard (§5.4 / release-v1 #6). The
/// frontend derives the four cards (time / words / time-saved / speed) from these.
#[tauri::command]
pub fn get_usage_stats(app: AppHandle) -> Result<crate::managers::history::UsageStats, AppError> {
    app.state::<Arc<crate::managers::history::HistoryManager>>()
        .usage_stats()
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

/// Return a saved secret only for the Settings eye-button Reveal action.
/// Opening the dialog uses `has_secret` instead, so it never requests Keychain data.
#[tauri::command]
pub fn get_secret_for_settings(app: AppHandle, key_id: String) -> Result<Option<String>, AppError> {
    validate_secret_key_id(&key_id)?;

    let platform = app.state::<Arc<dyn Platform>>();
    secret_for_explicit_reveal(
        &key_id,
        |key_id| platform.has_secret(key_id),
        |key_id| platform.read_secret(key_id),
    )
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
    platform.has_secret(key_id)
}

fn secret_for_explicit_reveal(
    key_id: &str,
    has_secret: impl FnOnce(&str) -> Result<bool, AppError>,
    read_secret: impl FnOnce(&str) -> Result<String, AppError>,
) -> Result<Option<String>, AppError> {
    if has_secret(key_id)? {
        read_secret(key_id).map(Some)
    } else {
        Ok(None)
    }
}

/// Assemble the Doubao streaming config from settings (endpoint / resource_id) +
/// keychain (app_id optional — blank = new-console single-key mode; access token
/// required). Reads secrets synchronously and returns an owned config, so callers
/// never hold the Tauri State across an await. Shared by the dev streaming probe
/// and the production connection test.
fn doubao_config_from_settings(
    app: &AppHandle,
) -> Result<crate::asr::doubao::client::DoubaoStreamConfig, AppError> {
    use crate::asr::doubao::{client, config};

    // endpoint / resource_id come from the (normalized) TOML settings; secrets from
    // keychain. Read both before any await so no Tauri State is held across it.
    let settings = load_settings(app);
    let platform = app.state::<Arc<dyn Platform>>();
    let app_id = platform
        .read_secret(config::SECRET_APP_ID)
        .unwrap_or_default();
    let api_key_or_access_token = platform
        .read_secret(config::SECRET_API_KEY_OR_ACCESS_TOKEN)
        .map_err(|_| AppError::Provider("豆包 API Key / Access Token 未配置".into()))?;

    Ok(client::DoubaoStreamConfig {
        endpoint: settings.doubao_endpoint,
        auth: client::DoubaoAuth::from_settings(app_id, api_key_or_access_token),
        resource_id: settings.doubao_resource_id,
    })
}

/// Production connectivity test for Doubao streaming ASR — drives the model config
/// dialog's 测试 button. Doubao is WebSocket-only (no /models endpoint), so this
/// can't go through `test_provider`; it opens the WS + handshake and checks one
/// frame for an auth/config error (no audio sent). See `client::test_connection`.
#[tauri::command]
pub async fn test_doubao_connection(
    app: AppHandle,
) -> Result<crate::provider_test::ProviderTestResult, AppError> {
    let cfg = doubao_config_from_settings(&app)?;
    crate::asr::doubao::client::test_connection(&cfg).await?;
    Ok(crate::provider_test::ProviderTestResult {
        ok: true,
        message: "连接测试通过".into(),
    })
}

/// Dev-only streaming probe for the Doubao ASR (P2.5). Reads a local 16k/mono/16-bit
/// wav and streams it to Doubao, logging partial/final text. Not registered in
/// release builds — the recording hot path lands P2.6.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn test_doubao_streaming(app: AppHandle, wav_path: String) -> Result<String, AppError> {
    use crate::asr::doubao::client;

    let pcm16 = read_wav_pcm16(&wav_path)?;
    let cfg = doubao_config_from_settings(&app)?;
    log::info!(
        "test_doubao_streaming: {} PCM bytes from {wav_path}",
        pcm16.len()
    );
    let text = client::transcribe_pcm16(&cfg, &pcm16).await?;
    log::info!("test_doubao_streaming final text: {text}");
    Ok(text)
}

/// Dev-only trigger-key probe (P3.8). Starts a listen-only CGEventTap so we can
/// verify fn + custom single/combo keys reach us before P3.9 swaps the real
/// trigger; key events surface as the `trigger-probe-key` event. SPEC §5.8.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn start_trigger_probe(app: AppHandle) -> Result<(), AppError> {
    app.state::<Arc<dyn Platform>>().start_trigger_probe(&app)
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn stop_trigger_probe(app: AppHandle) -> Result<(), AppError> {
    app.state::<Arc<dyn Platform>>().stop_trigger_probe()
}

/// P3.9 — Input Monitoring permission (macOS). The default trigger (fn) needs it.
/// `get` reads status without prompting; `request` shows the system prompt then
/// returns the (possibly still-false) status — a fresh grant only applies after
/// relaunch (SPEC §5.8 P3.9).
#[tauri::command]
pub fn get_input_monitoring_status(app: AppHandle) -> bool {
    app.state::<Arc<dyn Platform>>().input_monitoring_status()
}

#[tauri::command]
pub fn request_input_monitoring_permission(app: AppHandle) -> bool {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.request_input_monitoring();
    platform.input_monitoring_status()
}

/// P3.12 — Microphone permission (macOS TCC). `get` reads status without prompting
/// (the onboarding wizard polls it on focus); `request` shows the system prompt then
/// returns the (possibly still-false) status.
#[tauri::command]
pub fn get_microphone_permission_status(app: AppHandle) -> bool {
    app.state::<Arc<dyn Platform>>().microphone_status()
}

#[tauri::command]
pub fn request_microphone_permission(app: AppHandle) -> bool {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.request_microphone();
    platform.microphone_status()
}

/// P3.12 — Accessibility permission (macOS, post-event access). Injection's synthetic
/// Cmd+V needs it. `get` preflights without prompting; `request` shows the prompt.
#[tauri::command]
pub fn get_accessibility_permission_status(app: AppHandle) -> bool {
    app.state::<Arc<dyn Platform>>().accessibility_status()
}

#[tauri::command]
pub fn request_accessibility_permission(app: AppHandle) -> bool {
    let platform = app.state::<Arc<dyn Platform>>();
    platform.request_accessibility();
    platform.accessibility_status()
}

/// Decode a 16k/mono/16-bit wav into little-endian PCM16 bytes (hound decoder).
#[cfg(debug_assertions)]
fn read_wav_pcm16(path: &str) -> Result<Vec<u8>, AppError> {
    use crate::asr::doubao::config;

    let mut reader =
        hound::WavReader::open(path).map_err(|err| AppError::Device(format!("open wav: {err}")))?;
    let spec = reader.spec();
    if spec.sample_rate != config::STREAMING_SAMPLE_RATE
        || spec.channels != config::STREAMING_CHANNELS
        || spec.bits_per_sample != config::STREAMING_BITS_PER_SAMPLE
    {
        return Err(AppError::Device(format!(
            "expect 16k mono 16-bit wav, got {} Hz / {} ch / {} bit",
            spec.sample_rate, spec.channels, spec.bits_per_sample
        )));
    }

    let mut bytes = Vec::new();
    for sample in reader.samples::<i16>() {
        let sample = sample.map_err(|err| AppError::Device(format!("read wav sample: {err}")))?;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_include_p1_provider_fields() {
        let settings = Settings::default();

        assert_eq!(settings.hotkey, DEFAULT_HOTKEY);
        assert_eq!(settings.asr_provider, "groq");
        // Empty = each adapter uses its built-in default model (no behavior change).
        assert_eq!(settings.asr_model, "");
        assert_eq!(settings.llm_provider, "openai_compatible");
        assert!(settings.enhance_enabled);
        assert!(!settings.onboarding_completed);
        assert_eq!(settings.history_retention, DEFAULT_HISTORY_RETENTION);
        assert!(!settings.enhance_prompt.trim().is_empty());
        assert_eq!(
            settings.openai_compatible_base_url,
            DEFAULT_OPENAI_COMPATIBLE_BASE_URL
        );
        assert_eq!(
            settings.openai_compatible_model,
            DEFAULT_OPENAI_COMPATIBLE_MODEL
        );
        assert_eq!(
            settings.doubao_endpoint,
            crate::asr::doubao::config::DEFAULT_ENDPOINT
        );
        assert_eq!(
            settings.doubao_resource_id,
            crate::asr::doubao::config::DEFAULT_RESOURCE_ID
        );
    }

    #[test]
    fn legacy_doubao_resource_id_migrates_to_seed_asr_2_default() {
        assert_eq!(
            normalize_doubao_resource_id(crate::asr::doubao::config::LEGACY_RESOURCE_ID.into()),
            crate::asr::doubao::config::DEFAULT_RESOURCE_ID
        );
    }

    #[test]
    fn settings_round_trip_through_toml() {
        let original = Settings::default();
        let text = toml::to_string_pretty(&original).expect("serialize");
        assert_eq!(
            original,
            toml::from_str::<Settings>(&text).expect("deserialize")
        );

        // A non-empty asr_model is a plain string field (no skip_serializing_if) —
        // it survives the round trip just like asr_provider.
        let with_model = Settings {
            asr_model: "whisper-large-v3".into(),
            ..Settings::default()
        };
        let text = toml::to_string_pretty(&with_model).expect("serialize");
        assert_eq!(
            with_model,
            toml::from_str::<Settings>(&text).expect("deserialize")
        );

        // A populated per-provider model map serializes as the trailing [llm_models]
        // table (it MUST be the last field — a TOML table follows all scalar keys).
        let with_llm_models = Settings {
            llm_models: HashMap::from([
                ("lmstudio".to_string(), "qwen3-14b".to_string()),
                ("deepseek".to_string(), "deepseek-chat".to_string()),
            ]),
            ..Settings::default()
        };
        let text = toml::to_string_pretty(&with_llm_models).expect("serialize");
        assert_eq!(
            with_llm_models,
            toml::from_str::<Settings>(&text).expect("deserialize")
        );
    }

    #[test]
    fn patch_sets_asr_model_and_empty_clears_to_adapter_default() {
        // Setting a model overrides current; an empty patch value is meaningful
        // (= adapter built-in default), so it clears rather than keeping current.
        let current = Settings {
            asr_model: "gpt-4o-transcribe".into(),
            ..Settings::default()
        };
        let patched = settings_from_patch(
            current.clone(),
            SettingsPatch {
                hotkey: None,
                asr_provider: None,
                asr_model: Some("whisper-large-v3".into()),
                llm_provider: None,
                enhance_enabled: None,
                enhance_prompt: None,
                openai_compatible_base_url: None,
                openai_compatible_model: None,
                llm_api_key_id: None,
                doubao_endpoint: None,
                doubao_resource_id: None,
                input_device: None,
                onboarding_completed: None,
                primary_language: None,
                history_retention: None,
                ui_language: None,
                show_in_dock: None,
                compose_hotkey: None,
                compose_prompt: None,
                rewrite_prompt: None,
                llm_models: None,
            },
        )
        .expect("patch with model");
        assert_eq!(patched.asr_model, "whisper-large-v3");

        let cleared = settings_from_patch(
            current,
            SettingsPatch {
                hotkey: None,
                asr_provider: None,
                asr_model: Some(String::new()),
                llm_provider: None,
                enhance_enabled: None,
                enhance_prompt: None,
                openai_compatible_base_url: None,
                openai_compatible_model: None,
                llm_api_key_id: None,
                doubao_endpoint: None,
                doubao_resource_id: None,
                input_device: None,
                onboarding_completed: None,
                primary_language: None,
                history_retention: None,
                ui_language: None,
                show_in_dock: None,
                compose_hotkey: None,
                compose_prompt: None,
                rewrite_prompt: None,
                llm_models: None,
            },
        )
        .expect("patch clearing model");
        assert_eq!(cleared.asr_model, "");
    }

    #[test]
    fn partial_toml_fills_missing_fields_from_default() {
        // A hand-edited file with only a few keys must not error — container
        // serde(default) backfills the rest.
        let parsed: Settings = toml::from_str("hotkey = \"F13\"\nonboarding_completed = true\n")
            .expect("deserialize partial");
        assert_eq!(parsed.hotkey, "F13");
        assert!(parsed.onboarding_completed);
        assert_eq!(parsed.asr_provider, DEFAULT_ASR_PROVIDER);
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
            hotkey: "Fn".into(),
            compose_hotkey: "F13".into(),
            ..Settings::default()
        };
        assert!(validate_settings(&distinct).is_ok());

        let empty_compose = Settings {
            hotkey: "Fn".into(),
            compose_hotkey: String::new(),
            ..Settings::default()
        };
        assert!(validate_settings(&empty_compose).is_ok());
    }

    #[test]
    fn normalize_clamps_invalid_and_keeps_doubao_stream() {
        let clamped = normalize_settings(Settings {
            asr_provider: "bogus".into(),
            ..Settings::default()
        });
        assert_eq!(clamped.asr_provider, DEFAULT_ASR_PROVIDER);

        let kept = normalize_settings(Settings {
            asr_provider: "doubao_stream".into(),
            ..Settings::default()
        });
        assert_eq!(kept.asr_provider, "doubao_stream");

        let retention = normalize_settings(Settings {
            history_retention: "bogus".into(),
            ..Settings::default()
        });
        assert_eq!(retention.history_retention, DEFAULT_HISTORY_RETENTION);

        let fixed = normalize_settings(Settings {
            openai_compatible_base_url: "  ".into(),
            doubao_resource_id: crate::asr::doubao::config::LEGACY_RESOURCE_ID.into(),
            ..Settings::default()
        });
        assert_eq!(
            fixed.openai_compatible_base_url,
            DEFAULT_OPENAI_COMPATIBLE_BASE_URL
        );
        assert_eq!(
            fixed.doubao_resource_id,
            crate::asr::doubao::config::DEFAULT_RESOURCE_ID
        );

        // An empty LLM model is NOT reset to a default — picking a provider seeds no
        // hardcoded model; the user fetches/types one (build_provider errors if blank).
        let no_model = normalize_settings(Settings {
            openai_compatible_model: "  ".into(),
            ..Settings::default()
        });
        assert_eq!(no_model.openai_compatible_model, "");
    }

    #[test]
    fn legacy_json_migrates_into_settings() {
        // The store wrote a flat JSON object: serde maps known keys, ignores unknown
        // (e.g. the retired doubao_streaming_preview_enabled), defaults the missing.
        let json = r#"{"hotkey":"F13","asr_provider":"doubao_stream","doubao_streaming_preview_enabled":true,"onboarding_completed":true}"#;
        let parsed: Settings = serde_json::from_str(json).expect("deserialize legacy json");
        assert_eq!(parsed.hotkey, "F13");
        assert_eq!(parsed.asr_provider, "doubao_stream");
        assert!(parsed.onboarding_completed);
        assert_eq!(parsed.llm_provider, DEFAULT_LLM_PROVIDER);
    }

    #[test]
    fn provider_lists_expose_voxt_style_metadata() {
        let asr = available_asr_providers();
        let llm = available_llm_providers();

        assert_eq!(
            asr.iter()
                .map(|provider| provider.id.as_str())
                .collect::<Vec<_>>(),
            ["groq", "openai", "glm", "aliyun_fun", "stepfun"]
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

        assert!(!platform_has_secret(&MissingSecretPlatform, "groq_api_key").unwrap());
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

        assert!(platform_has_secret(&PresentSecretPlatform, "groq_api_key").unwrap());
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
            "groq_api_key",
            |_| Ok(false),
            |_| panic!("missing secret must not read Keychain data"),
        )
        .unwrap();

        assert_eq!(secret, None);
    }
}
