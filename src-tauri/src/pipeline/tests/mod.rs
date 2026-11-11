use super::config::{
    doubao_streaming_config_from_settings, enhance_config_from_settings, prompt_from_snapshot,
    transcription_config_from_settings,
};
use super::context::{
    ActivePromptSlot, DictationMode, EnhanceOutcome, LastTake, LastTakeSlot, RewriteSourceSlot,
};
use super::processing::{cache_transcript, finish_pipeline_tail, rewrite_history_raw};
use super::recovery::{clear_last_take, settle_take};
use super::streaming::streaming_result_after_capture;
use crate::asr::doubao::config as doubao_config;
use crate::asr::AudioData;
use crate::error::AppError;
use crate::managers::enhance::EnhanceConfig;
use crate::settings;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;
mod asr_config;
mod enhance_config;
mod recovery;
mod take_and_stream;
