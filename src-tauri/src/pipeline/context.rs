use super::*;
pub(super) type ActiveStreamingSession = Arc<parking_lot::Mutex<Option<TranscriptStream>>>;

/// The last utterance, kept so a terminal toast can resume it: 撤销操作 (after a
/// cancel), 重试 (after an error), or 插入原文. Stored the moment recording
/// finishes; `transcript` fills in once ASR returns (None if it never got there).
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

/// Monotonic id of the take that may still inject. A worker captures its id when
/// it spawns and only injects while the shared counter still equals it; tapping ✕
/// mid-Processing bumps the counter, superseding the worker without racing it.
pub(super) type TakeGen = Arc<AtomicU64>;

/// What to do with the current take. `Polish` cleans the transcript (default fn key;
/// 「AI 润色」开关开且配了 LLM 才润色，否则纯转写); `Compose` generates prose from spoken points (写作键).
/// `Rewrite` applies spoken instructions to the captured selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DictationMode {
    Polish,
    Rewrite,
    Compose,
}

impl DictationMode {
    /// Stable history `mode` column value.
    pub(super) fn as_str(self) -> &'static str {
        match self {
            DictationMode::Polish => "polish",
            DictationMode::Rewrite => "rewrite",
            DictationMode::Compose => "compose",
        }
    }
}

/// Which trigger fired — set on the press that starts a take. `start_recording`
/// resolves it to a `DictationMode` from the captured selection. Hotkey wiring
/// shares this type without accessing the in-flight context.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HotkeyRole {
    Primary,
    Compose,
}

/// The in-flight take's mode, set when recording starts (which trigger fired) and
/// read by the pipeline tail. A toggle keeps it across the start→finish gap.
pub(super) type ActiveModeSlot = Arc<parking_lot::Mutex<DictationMode>>;

/// Keep the mode's original prompt even if settings change before a retry.
// Tauri keys state by concrete type: a type alias of RewriteSourceSlot would
// share its storage, so the prompt needs a distinct wrapper.
pub(super) struct ActivePromptSlot(pub(super) parking_lot::Mutex<Option<String>>);

/// 改写模式抓到的选中文字：start_recording 探选中时存，finish 拼进 LLM 输入后用。
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

/// What happened to the transcript on the enhance step — distinguishes "polishing
/// was off" from "polished" (both inject the text, but only the latter has an
/// enhanced version worth storing) from "polish failed → raw fallback".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnhanceOutcome {
    Disabled,
    Enhanced,
    Failed,
}
