use super::processing::rewrite_history_raw;
use crate::asr::{AudioData, TranscriptStream};
use crate::managers::audio::AudioManager;
use crate::managers::enhance::EnhanceManager;
use crate::managers::inject::InjectManager;
use crate::managers::transcription::TranscriptionManager;
use crate::state::StateMachine;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tauri::AppHandle;

pub(super) type ActiveStreamingSession = Arc<parking_lot::Mutex<Option<TranscriptStream>>>;

/// 终态保留上一轮音频和上下文，让取消后撤销、失败后重试仍沿用原模式。
/// ASR 返回后才填 transcript；未获得转写时保留音频供恢复。
#[derive(Clone)]
pub(super) struct LastTake {
    pub(super) audio: Arc<AudioData>,
    pub(super) transcript: Option<String>,
    pub(super) duration_ms: u64,
    pub(super) generation: u64,
    pub(super) mode: DictationMode,
    pub(super) prompt: Option<String>,
    pub(super) rewrite_source: Option<String>,
}

impl LastTake {
    pub(super) fn history_raw(&self, text: &str) -> String {
        match (self.mode, self.rewrite_source.as_deref()) {
            (DictationMode::Rewrite, Some(source)) => rewrite_history_raw(text, source),
            _ => text.to_string(),
        }
    }
}

pub(super) type LastTakeSlot = Arc<parking_lot::Mutex<Option<LastTake>>>;

/// 单调递增的录音轮次。取消会推进轮次，让后台任务即使晚返回也不能继续粘贴。
pub(super) type TakeGen = Arc<AtomicU64>;

/// 润色清理转写；改写执行口述指令处理选区；写作根据口述要点生成内容。
/// 润色开关只约束 Polish，另外两种模式需要显式调用 LLM。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DictationMode {
    Polish,
    Rewrite,
    Compose,
}

impl DictationMode {
    /// 历史记录使用稳定字符串，避免 Rust 枚举名变动影响已有数据。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            DictationMode::Polish => "polish",
            DictationMode::Rewrite => "rewrite",
            DictationMode::Compose => "compose",
        }
    }
}

/// 这里只表示哪个快捷键触发；是否有选区要到录音开始时判断，快捷键层不接触录音上下文。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HotkeyRole {
    Primary,
    Compose,
}

/// toggle 的开始和结束是两次独立调用，模式必须保存到本轮结束。
pub(super) type ActiveModeSlot = Arc<parking_lot::Mutex<DictationMode>>;

/// 冻结本轮提示词，避免修改设置后重试改变原意。
// Tauri 按具体类型区分 state；若与 RewriteSourceSlot 使用相同的类型别名，
// 两份数据会共用存储，所以提示词使用独立包装类型。
pub(super) struct ActivePromptSlot(pub(super) parking_lot::Mutex<Option<String>>);

/// 开始时保存选区，结束后作为 LLM 的改写来源；不再探测可能已变化的当前选区。
pub(super) type RewriteSourceSlot = Arc<parking_lot::Mutex<Option<String>>>;

pub(super) struct HotkeyContext<'a> {
    pub(super) app: &'a AppHandle,
    pub(super) state: &'a Arc<StateMachine>,
    pub(super) audio: &'a Arc<AudioManager>,
    pub(super) transcription: &'a Arc<TranscriptionManager>,
    pub(super) enhance: &'a Arc<EnhanceManager>,
    pub(super) inject: &'a Arc<InjectManager>,
    pub(super) streaming: &'a ActiveStreamingSession,
    pub(super) last_take: &'a LastTakeSlot,
    pub(super) take_gen: &'a TakeGen,
    pub(super) active_mode: &'a ActiveModeSlot,
}

/// 区分未启用、成功润色、失败回退：三者均可注入，但只有成功润色才存 enhanced_text。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum EnhanceOutcome {
    Disabled,
    Enhanced,
    Failed(String),
}
