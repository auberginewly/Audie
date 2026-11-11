//! Persisted settings, defaults and validation. No command handlers or OS effects.
mod catalog;
mod defaults;
mod normalize;
mod patch;
mod persistence;
mod types;
mod validation;

use crate::error::AppError;
use crate::platform::should_migrate_windows_fn;
pub(crate) use catalog::{available_asr_providers, available_llm_providers, ProviderMetadata};
pub(crate) use defaults::*;
use normalize::normalize_settings;
pub(crate) use patch::settings_from_patch;
pub(crate) use persistence::{load_hotkey, load_settings, persist_settings};
pub(crate) use types::{Settings, SettingsPatch};
pub(crate) use validation::validate_secret_key_id;
use validation::validate_settings;

#[cfg(test)]
mod tests;
