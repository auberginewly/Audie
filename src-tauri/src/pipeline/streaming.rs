use super::*;
pub(super) fn start_streaming_session(
    app: &AppHandle,
    transcription: &TranscriptionManager,
    streaming: &ActiveStreamingSession,
) -> Option<mpsc::SyncSender<AppResult<crate::asr::AudioChunk>>> {
    let config = doubao_streaming_config(app)?;
    let (chunks_tx, chunks_rx) = mpsc::sync_channel(STREAMING_AUDIO_QUEUE_CAPACITY);
    match transcription.transcribe_stream(chunks_rx, &config) {
        Ok(transcripts) => {
            *streaming.lock() = Some(transcripts);
            Some(chunks_tx)
        }
        Err(err) => {
            log::warn!("doubao streaming unavailable, falling back to batch: {err:?}");
            None
        }
    }
}

pub(super) fn clear_streaming_session(streaming: &ActiveStreamingSession) {
    let _ = streaming.lock().take();
}

pub(super) fn take_streaming_session(
    streaming: &ActiveStreamingSession,
) -> Option<TranscriptStream> {
    streaming.lock().take()
}

pub(super) fn streaming_result_after_capture(
    streaming_result: Option<TranscriptStream>,
    streaming_queue_overflowed: bool,
) -> Option<TranscriptStream> {
    if streaming_queue_overflowed {
        log::warn!("streaming audio queue overflowed; falling back to batch ASR");
        None
    } else {
        streaming_result
    }
}
