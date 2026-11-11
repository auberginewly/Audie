//! Frontend command surface; business logic belongs to the owning module.
pub(crate) mod audio;
pub(crate) mod dictation;
pub(crate) mod history;
pub(crate) mod hotkeys;
pub(crate) mod permissions;
pub(crate) mod providers;
pub(crate) mod secrets;
pub(crate) mod settings;

// Temporary during the capability migration; removed in the next checkpoint.
pub(crate) use crate::settings::{
    available_asr_providers, available_llm_providers, load_settings, validate_secret_key_id,
    DEFAULT_LLM_PROVIDER,
};
