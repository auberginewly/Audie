use super::*;

#[test]
fn rewrite_history_raw_pairs_instruction_with_quoted_source() {
    // 改写历史的「原文」= 口述指令 + 引用的选中内容（两段带小标题，给人看）。
    let raw = rewrite_history_raw("翻译成英文", "你好世界");
    assert_eq!(raw, "指令：翻译成英文\n\n引用：\n你好世界");
}

#[test]
fn last_take_clone_shares_audio_buffer() {
    let audio = Arc::new(AudioData {
        samples: vec![0.1, 0.2, 0.3],
        sample_rate: 16_000,
        channels: 1,
    });
    let take = LastTake {
        audio: audio.clone(),
        transcript: None,
        duration_ms: 0,
        generation: 1,
        mode: DictationMode::Polish,
        prompt: None,
        rewrite_source: None,
    };

    let cloned = take.clone();

    assert!(Arc::ptr_eq(&take.audio, &cloned.audio));
}

#[test]
fn clearing_last_take_drops_terminal_audio() {
    let slot: LastTakeSlot = Arc::new(parking_lot::Mutex::new(Some(LastTake {
        audio: Arc::new(AudioData {
            samples: vec![0.1],
            sample_rate: 16_000,
            channels: 1,
        }),
        transcript: None,
        duration_ms: 0,
        generation: 1,
        mode: DictationMode::Polish,
        prompt: None,
        rewrite_source: None,
    })));

    clear_last_take(&slot);

    assert!(slot.lock().is_none());
}

#[test]
fn streaming_queue_overflow_falls_back_to_batch_resolution() {
    let (_tx, rx) = mpsc::channel();

    let selected = streaming_result_after_capture(Some(rx), true);

    assert!(selected.is_none());
}

#[test]
fn streaming_result_kept_when_queue_did_not_overflow() {
    let (_tx, rx) = mpsc::channel();

    let selected = streaming_result_after_capture(Some(rx), false);

    assert!(selected.is_some());
}
