use super::config::prompt_from_snapshot;
use super::context::{
    ActivePromptSlot, DictationMode, HotkeyContext, HotkeyRole, LastTake, RewriteSourceSlot,
};
use super::processing::spawn_transcription;
use super::recovery::enter_error;
use super::streaming::{
    clear_streaming_session, start_streaming_session, streaming_result_after_capture,
    take_streaming_session,
};
use super::{show_overlay, with_hotkey_ctx};
use crate::asr::AudioData;
use crate::error::AppError;
use crate::platform::Platform;
use crate::settings;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub(super) fn handle_hotkey(ctx: &HotkeyContext<'_>, role: HotkeyRole) {
    // toggle 模式两次按键分别开始、结束；处理或终态期间忽略按键，防止重入。
    // 只有开始时需要 role 来决定本轮模式。
    match ctx.state.current() {
        AppState::Idle => start_recording(ctx, role),
        AppState::Recording => finish_recording(ctx),
        _ => {}
    }
}

/// 先确认权限并成功打开音频，再进入 Recording；可用时同时建立流式 ASR 会话。
pub(super) fn start_recording(ctx: &HotkeyContext<'_>, role: HotkeyRole) {
    // 先推进轮次，让旧任务和旧定时器无法影响这次录音。
    let my_gen = {
        let mut cached = ctx.last_take.lock();
        let generation = ctx.take_gen.fetch_add(1, Ordering::SeqCst) + 1;
        *cached = None;
        generation
    };
    let platform = ctx.app.state::<Arc<dyn Platform>>();
    // 必须在选区探测、权限弹窗、胶囊显示之前记住前台 App。
    // 合成 Cmd+C 和最终注入都要回到这个目标，权限弹窗可能改变当前焦点。
    platform.capture_focus_target();

    // 写作键直接进入写作；主键用开始时的选区决定改写或润色。
    // 改写选区此时就冻结，避免处理结束时取到别的内容。
    let mode = match role {
        HotkeyRole::Compose => DictationMode::Compose,
        HotkeyRole::Primary => match platform.read_selection(ctx.app) {
            Some(sel) if !sel.trim().is_empty() => {
                *ctx.app.state::<RewriteSourceSlot>().lock() = Some(sel);
                DictationMode::Rewrite
            }
            _ => DictationMode::Polish,
        },
    };
    if !matches!(mode, DictationMode::Rewrite) {
        *ctx.app.state::<RewriteSourceSlot>().lock() = None;
    }
    *ctx.active_mode.lock() = mode;
    let settings = settings::load_settings(ctx.app);
    *ctx.app.state::<ActivePromptSlot>().0.lock() =
        Some(prompt_from_snapshot(None, &settings, mode));

    // 拒绝麦克风权限时立即反馈，避免静音被 ASR 误识别成文字。

    if !platform.ensure_microphone_permission() {
        let _ = show_overlay(ctx.app);
        enter_error(
            ctx.app.clone(),
            ctx.state.clone(),
            AppError::Permission("请授予麦克风权限".into()),
            my_gen,
        );
        return;
    }
    // 先打开设备再切 Recording：设备失败需要走 Idle → Error，
    // 若先改状态，可能留下没有音频流的 Recording。
    // 同时先停设置页的预览监听，录音独占麦克风。
    ctx.audio.stop_monitor();
    let streaming_start = start_streaming_session(ctx.app, ctx.transcription, ctx.streaming);
    let capture_result = match streaming_start {
        Some(chunk_tx) => ctx.audio.start_capture_streaming(ctx.app.clone(), chunk_tx),
        None => ctx.audio.start_capture(ctx.app.clone()),
    };
    if let Err(err) = capture_result {
        log::error!("start capture: {err:?}");
        clear_streaming_session(ctx.streaming);
        let _ = show_overlay(ctx.app);
        enter_error(ctx.app.clone(), ctx.state.clone(), err, my_gen);
        return;
    }
    if ctx
        .state
        .transition(ctx.app, AppState::Recording, Some("toggle-start"))
    {
        if let Err(err) = show_overlay(ctx.app) {
            log::error!("show overlay: {err:?}");
        }
    } else {
        // 状态切换若被拒绝，释放刚打开的音频流，避免资源泄漏。
        clear_streaming_session(ctx.streaming);
        if let Err(err) = ctx.audio.stop_capture() {
            log::warn!("rollback stop_capture: {err:?}");
        }
    }
}

/// 结束录音后胶囊继续展示处理与终态；统一到 Idle 时才隐藏，避免中途闪退。
pub(super) fn finish_recording(ctx: &HotkeyContext<'_>) {
    let (my_gen, stopped) = {
        // 与取消、恢复、终态清理共用锁，保证进入 Processing 时缓存已就绪。
        // 锁内只做录音交接；网络 provider 调用在释放锁之后执行。
        let mut cached = ctx.last_take.lock();
        if ctx.state.current() != AppState::Recording {
            return;
        }
        let generation = ctx.take_gen.load(Ordering::SeqCst);
        if !ctx
            .state
            .transition(ctx.app, AppState::Processing, Some("toggle-finish"))
        {
            return;
        }
        let streaming = take_streaming_session(ctx.streaming);
        let stopped = ctx.audio.stop_capture().map(|recorded| {
            let streaming =
                streaming_result_after_capture(streaming, recorded.streaming_queue_overflowed);
            let take = recorded_take(ctx, recorded.audio, generation);
            *cached = Some(take.clone());
            (take, streaming)
        });
        if stopped.is_err() {
            *cached = None;
        }
        (generation, stopped)
    };
    match stopped {
        Ok((take, streaming)) => spawn_transcription(
            ctx.app.clone(),
            ctx.state.clone(),
            ctx.transcription.clone(),
            ctx.enhance.clone(),
            ctx.inject.clone(),
            ctx.last_take.clone(),
            ctx.take_gen.clone(),
            my_gen,
            take,
            streaming,
            false,
        ),
        Err(err) => {
            log::error!("stop capture: {err:?}");
            enter_error(ctx.app.clone(), ctx.state.clone(), err, my_gen);
        }
    }
}

pub(super) fn recorded_take(
    ctx: &HotkeyContext<'_>,
    audio: AudioData,
    generation: u64,
) -> LastTake {
    let audio = Arc::new(audio);
    LastTake {
        duration_ms: duration_ms(&audio),
        audio,
        transcript: None,
        generation,
        mode: *ctx.active_mode.lock(),
        prompt: ctx.app.state::<ActivePromptSlot>().0.lock().take(),
        rewrite_source: ctx.app.state::<RewriteSourceSlot>().lock().take(),
    }
}

pub(super) fn is_digital_silence(audio: &AudioData) -> bool {
    const SILENCE_EPS: f32 = 1e-4;
    !audio.samples.iter().any(|s| s.abs() > SILENCE_EPS)
}

pub(super) fn duration_ms(audio: &AudioData) -> u64 {
    let denom = audio.sample_rate as u64 * audio.channels.max(1) as u64;
    (audio.samples.len() as u64 * 1000)
        .checked_div(denom)
        .unwrap_or_default()
}

/// 胶囊确认按钮复用第二次快捷键的结束流程。
pub(crate) fn confirm_recording(app: AppHandle) {
    with_hotkey_ctx(&app, |ctx| {
        if ctx.state.current() == AppState::Recording {
            finish_recording(ctx);
        }
    });
}
